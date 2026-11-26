
static sv4_t llg_net_compute(llg_net_t* net);
static void force_recompute_target(sv4_t* target, llg_net_t* net);
static void llg_net_alias_refresh_all(llg_net_t* net);
static void llg_net_cancel_propagation(llg_net_t* net);
static void llg_net_propagation_sync(llg_net_t* net);
static void llg_net_strength_publish(llg_net_t* net);
static void llg_switch_net_changed(llg_net_t* net);

static llg_force_watch_fn force_watch_hook;
static LLG_OUTLYING void redrive(void* cell);

void llg_force_watch_hook(llg_force_watch_fn hook) { force_watch_hook = hook; }

// Grow one live-binding table to hold at least `needed` entries. The grown copy
// is completed before it replaces the old table, so an allocation failure
// aborts without a partially rebound registry. Only `used` entries are copied.
static void force_table_reserve(int32_t needed) {
    if (needed <= g.force_capacity) return;
    int32_t capacity = llg_registry_capacity(g.force_capacity, needed);
    llg_force_entry_t* grown = (llg_force_entry_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "force table");
    if (g.force_table)
        memcpy(grown, g.force_table, (size_t)g.force_count * sizeof(*grown));
    free(g.force_table);
    g.force_table = grown;
    g.force_capacity = capacity;
}

// Is `sig` currently covered by a packed force part? Procedural writes keep
// the forced bits of such a signal (`force_write_unforced`); net driver slots
// remain writable so their current resolved value can be exposed on release.
static int32_t llg_is_forced(sv4_t* sig) {
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int32_t j = 0; j < entry->n_parts; j++)
            if (entry->parts[j].target == sig && sv4_to_bool(entry->masks[j])) return 1;
    }
    return 0;
}

static uint64_t force_mask_word(sv4_t* target, size_t word);

// Do forces cover every bit of `target`?
static int32_t force_covers_all(sv4_t* target) {
    uint32_t width = llg_sv4_width(*target);
    size_t words = ((size_t)width + 63u) / 64u;
    for (size_t word = 0; word < words; word++) {
        uint64_t all = word + 1u == words && width % 64u
            ? UINT64_MAX >> (64u - width % 64u) : UINT64_MAX;
        if ((force_mask_word(target, word) & all) != all) return 0;
    }
    return 1;
}

// Give `*value`, a new value of `target` at its width, the target's current
// bits wherever a force covers them.
static LLG_OUTLYING void force_keep_forced_bits(sv4_t* target, sv4_t* value) {
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int32_t j = 0; j < entry->n_parts; j++)
            if (entry->parts[j].target == target)
                llg_sv4_masked_merge(value, *target, entry->masks[j]);
    }
}

// A write of `value` to `target` while forces cover some of its bits (a
// member of an array element held as one packed image, or a cell of a record
// leaf, is forced in part): a forced bit keeps its forced value (SV 10.6.2)
// and the others take the write. A target whose every bit is forced keeps its
// value. Called only for a target `llg_is_forced` reports; `value` is
// borrowed.
static LLG_OUTLYING void force_write_unforced(sv4_t* target, sv4_t value) {
    if (force_covers_all(target)) return;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* merged = llg_value_scope_values(scope);
    sv4_replace(merged, sv4_cast(value, llg_sv4_width(*target), llg_sv4_signed(*target)));
    force_keep_forced_bits(target, merged);
    sig_write(target, *merged);
    llg_value_scope_end(scope);
}

static int32_t llg_is_real_forced(double* target) {
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) return 1;
    }
    return 0;
}

// Counts (`add`) or uncounts the entry's reads on their rows, so that a check
// of what it reads evaluates it again (force_dependency_changed).
static LLG_OUTLYING void force_reads_watch(const llg_force_entry_t* entry, int32_t add) {
    for (int32_t i = 0; i < entry->n_reads; i++) {
        const llg_force_read_t* read = &entry->reads[i];
        const void* key = read->is_real ? (const void*)read->real : (const void*)read->sig;
        if (!key) continue;
        if (add) {
            llg_wait_source_t* source = wait_source_get(key);
            if (source->checks == UINT32_MAX) {
                llg_msg(LLG_MSG_INTERNAL, NULL, "force dependency count overflow");
                abort();
            }
            wait_source_keep(source);
            source->force_reads++;
            source->checks++;
        } else {
            llg_wait_source_t* source = wait_source_find(key);
            if (!source || !source->force_reads) continue;
            source->force_reads--;
            source->checks--;
            wait_source_unkeep(source);
        }
    }
}

static void force_free_entry(llg_force_entry_t* entry) {
    if (entry->reads) force_reads_watch(entry, 0);
    sv4_destroy(&entry->value);
    sv4_destroy_array(entry->masks, entry->masks ? (size_t)entry->n_parts : 0);
    free(entry->parts);
    free(entry->masks);
    free(entry->reads);
    memset(entry, 0, sizeof(*entry));
}

static sv4_t force_part_mask(const llg_force_part_t* part) {
    if (!part->target || llg_sv4_width(*part->target) >= LLG_SUPPORTED_WIDTH_LIMIT ||
        part->width >= LLG_SUPPORTED_WIDTH_LIMIT) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid force target or width");
        abort();
    }
    sv4_t mask = sv4_from_u64(0, llg_sv4_width(*part->target), 0);
    sv4_t ones = sv4_fill(1, part->width, 0);
    sv4_part_select_set(&mask, part->left, part->right, ones);
    sv4_destroy(&ones);
    return mask;
}

// Force replacement and release affect target bits, not the shape of the
// original LHS or its RHS offsets. Overwritten forces never become active again.
static void force_remove_coverage(const llg_force_part_t* parts, int32_t n_parts) {
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        int32_t remains = 0;
        for (int32_t j = 0; j < entry->n_parts; j++) {
            sv4_t* mask = &entry->masks[j];
            for (int32_t k = 0; k < n_parts; k++) {
                if (entry->parts[j].target != parts[k].target) continue;
                sv4_t removed = force_part_mask(&parts[k]);
                llg_sv4_mask_remove(mask, removed);
                sv4_destroy(&removed);
            }
            remains |= sv4_to_bool(*mask);
        }
        if (!remains) force_free_entry(entry);
    }
}

static int32_t force_find_free_slot(void) {
    for (int32_t i = 0; i < g.force_count; i++)
        if (!g.force_table[i].active) return i;
    if (g.force_count == INT_MAX) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "force entry count overflow");
        abort();
    }
    force_table_reserve(g.force_count + 1);
    return g.force_count++;
}

static llg_force_entry_t* force_prepare_packed(
    const llg_force_part_t* parts, int32_t n_parts, uint32_t stream_slice,
    int32_t stream_right_to_left, llg_force_eval_fn eval,
    const llg_force_read_t* reads, int32_t n_reads) {
    if (!parts || n_parts <= 0 || n_reads < 0) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid packed force descriptor");
        abort();
    }
    force_remove_coverage(parts, n_parts);
    int32_t slot = force_find_free_slot();
    llg_force_entry_t* entry = &g.force_table[slot];
    memset(entry, 0, sizeof(*entry));
    entry->parts = (llg_force_part_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->parts), "force parts");
    memcpy(entry->parts, parts, (size_t)n_parts * sizeof(*parts));
    entry->masks = (sv4_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->masks), "force coverage masks");
    for (int32_t i = 0; i < n_parts; i++) entry->masks[i] = force_part_mask(&parts[i]);
    if (n_reads > 0) {
        if (!reads) {
            llg_msg(LLG_MSG_INTERNAL, NULL, "force dependency count has no descriptor array");
            abort();
        }
        entry->reads = (llg_force_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(*entry->reads), "force dependencies");
        memcpy(entry->reads, reads, (size_t)n_reads * sizeof(*reads));
    }
    entry->active = 1;
    entry->is_real = 0;
    entry->n_parts = n_parts;
    entry->stream_slice = stream_slice;
    entry->stream_right_to_left = stream_right_to_left;
    entry->eval = eval;
    entry->real_eval = NULL;
    entry->n_reads = n_reads;
    force_reads_watch(entry, 1);
    return entry;
}

static int32_t force_read_matches(const llg_force_entry_t* entry, sv4_t* sig,
                              double* real, int32_t is_real) {
    for (int32_t i = 0; i < entry->n_reads; i++) {
        const llg_force_read_t* read = &entry->reads[i];
        if (read->is_real == is_real &&
            (is_real ? read->real == real : read->sig == sig))
            return 1;
    }
    return 0;
}

static void force_apply_part(sv4_t* target, const llg_force_part_t* part,
                             const sv4_t* mask, const sv4_t* value) {
    if (part->width == 0 || !sv4_to_bool(*mask)) return;
    uint64_t high = (uint64_t)part->value_lsb + part->width - 1;
    sv4_t selected = sv4_part_select(*value, (int64_t)high, (int64_t)part->value_lsb);
    if (part->two_state) sv4_replace(&selected, sv4_to_two_state(selected));
    sv4_t updated = sv4_clone(target);
    sv4_part_select_set(&updated, part->left, part->right, selected);
    llg_sv4_masked_merge(target, updated, *mask);
    sv4_destroy(&updated);
    sv4_destroy(&selected);
}

// Union of the active packed force masks on one target for one 64-bit word.
// Strength views report forced bits at strong strength.
static uint64_t force_mask_word(sv4_t* target, size_t word) {
    uint64_t forced = 0;
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int32_t j = 0; j < entry->n_parts; j++) {
            if (entry->parts[j].target != target) continue;
            forced |= llg_sv4_word(entry->masks[j], word, LLG_SV4_BITS) &
                ~(llg_sv4_word(entry->masks[j], word, LLG_SV4_X) |
                  llg_sv4_word(entry->masks[j], word, LLG_SV4_Z));
        }
    }
    return forced;
}

static llg_net_t* force_net_for_target(sv4_t* target, llg_net_t* fallback) {
    if (fallback) return fallback;
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int32_t j = 0; j < entry->n_parts; j++) {
            if (entry->parts[j].target == target && entry->parts[j].net)
                return entry->parts[j].net;
        }
    }
    return NULL;
}

static void force_recompute_target(sv4_t* target, llg_net_t* net) {
    net = force_net_for_target(target, net);
    if (net) llg_net_cancel_propagation(net);
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (net) {
        sv4_replace(value, llg_net_compute(net));
    } else {
        sv4_copy(value, target);
    }
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        sv4_t streamed = entry->stream_slice
            ? sv4_stream(entry->value, entry->stream_slice, entry->stream_right_to_left)
            : sv4_clone(&entry->value);
        for (int32_t j = 0; j < entry->n_parts; j++) {
            llg_force_part_t* part = &entry->parts[j];
            if (part->target == target)
                force_apply_part(value, part, &entry->masks[j], &streamed);
        }
        sv4_destroy(&streamed);
    }
    sig_write(target, *value);
    if (net) {
        llg_net_propagation_sync(net);
        llg_net_alias_refresh_all(net);
        llg_net_strength_publish(net);
    }
    llg_value_scope_end(scope);
}

static void force_entry_targets(const llg_force_entry_t* entry) {
    for (int32_t i = 0; i < entry->n_parts; i++) {
        sv4_t* target = entry->parts[i].target;
        int32_t seen = 0;
        for (int32_t j = 0; j < i; j++)
            if (entry->parts[j].target == target) seen = 1;
        if (seen) continue;
        llg_net_t* net = force_net_for_target(target, entry->parts[i].net);
        force_recompute_target(target, net);
        // A forced switch network node is a fixed source of its network.
        if (net && net->switch_net) llg_switch_net_changed(net);
    }
}

static void force_evaluate_entry(llg_force_entry_t* entry) {
    if (!entry->active || entry->evaluating) return;
    entry->evaluating = 1;
    if (entry->is_real) {
        if (entry->real_eval) entry->real_eval(&entry->real_value);
        real_write(entry->real_target, entry->real_value);
    } else {
        if (entry->eval) {
            llg_value_scope_t* scope = llg_value_scope_begin(1);
            sv4_t* evaluated = llg_value_scope_values(scope);
            entry->eval(evaluated);
            sv4_move(&entry->value, evaluated);
            llg_value_scope_end(scope);
        }
        force_entry_targets(entry);
    }
    entry->evaluating = 0;
}

int32_t llg_force_entry_view(int32_t index, const llg_force_part_t** parts, int32_t* n_parts,
                         double** real) {
    *parts = NULL;
    *n_parts = 0;
    *real = NULL;
    if (index < 0 || index >= g.force_count) return 0;
    const llg_force_entry_t* entry = &g.force_table[index];
    // vpi_put_value forces (no evaluator) are not force statements.
    if (!entry->active || (!entry->eval && !entry->real_eval)) return 1;
    if (entry->is_real) {
        *real = entry->real_target;
    } else {
        *parts = entry->parts;
        *n_parts = entry->n_parts;
    }
    return 1;
}

int32_t llg_force_source_active(const sv4_t* sig, const double* real) {
    int32_t is_real = real != NULL;
    for (int32_t i = 0; i < g.force_count; i++) {
        const llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active &&
            force_read_matches(entry, (sv4_t*)sig, (double*)real, is_real))
            return 1;
    }
    return 0;
}

// The check of storage that live force expressions read (`force_reads` on
// its row): each such expression is evaluated again once per check that sees
// a change, and its targets are updated at once (T1-D11).
static LLG_OUTLYING void force_dependency_changed(sv4_t* sig, double* real, int32_t is_real) {
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && force_read_matches(entry, sig, real, is_real))
            force_evaluate_entry(entry);
    }
}

// A part with a NULL target is an array element that the statement's index
// selected none of (SV 7.4.6: an invalid index writes nothing). Returns the
// parts that name storage: `parts` itself when all do, else a compacted copy
// in `*owned` that the caller frees; `*n_present` may become 0.
static LLG_OUTLYING const llg_force_part_t* force_present_parts(
    const llg_force_part_t* parts, int32_t n_parts, int32_t* n_present, llg_force_part_t** owned) {
    *owned = NULL;
    *n_present = n_parts;
    if (!parts || n_parts <= 0) return parts;
    int32_t present = 0;
    for (int32_t i = 0; i < n_parts; i++) present += parts[i].target != NULL;
    if (present == n_parts) return parts;
    *n_present = present;
    if (!present) return parts;
    *owned = (llg_force_part_t*)llg_checked_malloc(
        (size_t)present, sizeof(**owned), "selected force parts");
    for (int32_t i = 0, j = 0; i < n_parts; i++)
        if (parts[i].target) (*owned)[j++] = parts[i];
    return *owned;
}

void llg_force_expr_parts(const llg_force_part_t* parts, int32_t n_parts,
                          uint32_t stream_slice, int32_t stream_right_to_left,
                          llg_force_eval_fn eval,
                          const llg_force_read_t* reads, int32_t n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    llg_force_part_t* owned;
    parts = force_present_parts(parts, n_parts, &n_parts, &owned);
    if (n_parts == 0) return;
    llg_force_entry_t* entry = force_prepare_packed(
        parts, n_parts, stream_slice, stream_right_to_left, eval, reads, n_reads);
    force_evaluate_entry(entry);
    change_check();
    if (LLG_CO_UNLIKELY(force_watch_hook != NULL)) force_watch_hook(0, parts, n_parts, NULL);
    free(owned);
}

void llg_force_real(double* target, llg_force_real_eval_fn eval,
                    const llg_force_read_t* reads, int32_t n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    if (!target || !eval || n_reads < 0 || (n_reads > 0 && !reads)) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid real force descriptor");
        abort();
    }
    llg_force_entry_t* entry = NULL;
    for (int32_t i = 0; i < g.force_count; i++) {
        if (g.force_table[i].active && g.force_table[i].is_real &&
            g.force_table[i].real_target == target) {
            entry = &g.force_table[i];
            force_reads_watch(entry, 0);
            free(entry->reads);
            entry->reads = NULL;
            break;
        }
    }
    if (!entry) {
        int32_t slot = force_find_free_slot();
        entry = &g.force_table[slot];
        memset(entry, 0, sizeof(*entry));
    }
    if (n_reads > 0) {
        entry->reads = (llg_force_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(*entry->reads), "real force dependencies");
        memcpy(entry->reads, reads, (size_t)n_reads * sizeof(*reads));
    }
    entry->active = 1;
    entry->is_real = 1;
    entry->real_target = target;
    entry->real_eval = eval;
    entry->eval = NULL;
    entry->n_reads = n_reads;
    force_reads_watch(entry, 1);
    force_evaluate_entry(entry);
    change_check();
    if (LLG_CO_UNLIKELY(force_watch_hook != NULL)) force_watch_hook(0, NULL, 0, target);
}

void llg_release_parts(const llg_force_part_t* parts, int32_t n_parts,
                       uint32_t stream_slice, int32_t stream_right_to_left) {
    if (!region_can_mutate("force scheduling")) return;
    llg_force_part_t* owned;
    parts = force_present_parts(parts, n_parts, &n_parts, &owned);
    if (!parts || n_parts <= 0) return;
    (void)stream_slice;
    (void)stream_right_to_left;
    // Preserve net metadata before removing the final force on a target.
    llg_net_t** nets = llg_checked_malloc(
        (size_t)n_parts, sizeof(*nets), "released force nets");
    for (int32_t i = 0; i < n_parts; i++)
        nets[i] = force_net_for_target(parts[i].target, parts[i].net);
    force_remove_coverage(parts, n_parts);
    for (int32_t i = 0; i < n_parts; i++) {
        sv4_t* target = parts[i].target;
        int32_t seen = 0;
        for (int32_t j = 0; j < i; j++)
            if (parts[j].target == target) seen = 1;
        if (seen) continue;
        force_recompute_target(target, nets[i]);
        if (nets[i] && nets[i]->switch_net) llg_switch_net_changed(nets[i]);
        // A net took its drivers' value above; a variable takes that of its
        // continuous drivers here.
        if (!nets[i]) redrive(target);
    }
    free(nets);
    change_check();
    if (LLG_CO_UNLIKELY(force_watch_hook != NULL)) force_watch_hook(1, parts, n_parts, NULL);
    free(owned);
}

void llg_release_real(double* target) {
    if (!region_can_mutate("force scheduling")) return;
    for (int32_t i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) {
            // A procedural real variable retains the forced value; no saved
            // value exists to restore.
            force_free_entry(entry);
            break;
        }
    }
    redrive(target);
    if (LLG_CO_UNLIKELY(force_watch_hook != NULL)) force_watch_hook(1, NULL, 0, target);
}

void llg_force(sv4_t* sig, sv4_t value) {
    if (!region_can_mutate("force scheduling")) return;
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)llg_sv4_width(*sig) - 1, 0, llg_sv4_width(*sig), 0, 0
    };
    llg_force_entry_t* entry = force_prepare_packed(&part, 1, 0, 0, NULL, NULL, 0);
    sv4_copy(&entry->value, &value);
    force_entry_targets(entry);
    change_check();
    if (LLG_CO_UNLIKELY(force_watch_hook != NULL)) force_watch_hook(0, &part, 1, NULL);
}

// Constant-value force of explicit storage parts (vpi_put_value with
// vpiForceFlag, 38.34). `value` is borrowed; part `value_lsb` offsets select
// from it exactly as for an expression force.
void llg_force_value_parts(const llg_force_part_t* parts, int32_t n_parts, sv4_t value) {
    if (!region_can_mutate("force scheduling")) return;
    llg_force_entry_t* entry = force_prepare_packed(parts, n_parts, 0, 0, NULL, NULL, 0);
    sv4_copy(&entry->value, &value);
    force_entry_targets(entry);
    change_check();
}

// Constant-value force of a real variable; replaces an existing real force.
void llg_force_real_value(double* target, double value) {
    if (!region_can_mutate("force scheduling")) return;
    if (!target) return;
    llg_force_entry_t* entry = NULL;
    for (int32_t i = 0; i < g.force_count; i++) {
        if (g.force_table[i].active && g.force_table[i].is_real &&
            g.force_table[i].real_target == target) {
            entry = &g.force_table[i];
            force_reads_watch(entry, 0);
            free(entry->reads);
            entry->reads = NULL;
            break;
        }
    }
    if (!entry) {
        int32_t slot = force_find_free_slot();
        entry = &g.force_table[slot];
        memset(entry, 0, sizeof(*entry));
    }
    entry->active = 1;
    entry->is_real = 1;
    entry->real_target = target;
    entry->real_eval = NULL;
    entry->real_value = value;
    entry->eval = NULL;
    entry->n_reads = 0;
    force_evaluate_entry(entry);
    change_check();
}

void llg_release(sv4_t* sig) {
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)llg_sv4_width(*sig) - 1, 0, llg_sv4_width(*sig), 0, 0
    };
    llg_release_parts(&part, 1, 0, 0);
}

// ── redrives: a released variable takes its continuous drivers' value ─────

// Everything below runs in a release only and lies outside the unit's other
// code, so that no function of a run without a release moves.
static LLG_OUTLYING int32_t fixed_array_owns(const llg_fixed_array_t* array, const void* cell);
static LLG_OUTLYING void inertial_redrive(sv4_t* target);

// The model's table (llg_redrives_register). `order` holds row numbers once
// the first release of a run indexed them: the rows with cells by storage
// address, then the keyed-array rows. `busy` marks rows whose action is
// running: a release made from inside a driver's own evaluation finds that
// driver busy and leaves the value to it. All of it is the model's static
// storage, so there is nothing to free at the end of a run and the table
// stays for the releases of final procedures.
static const llg_redrive_table_t* redrives;
static uint32_t redrive_ranged;
static int32_t redrive_indexed;
// The release under way came through VPI (llg_release_parts_scheduled).
static int32_t redrive_scheduled;

LLG_OUTLYING void llg_redrives_register(const llg_redrive_table_t* table) {
    redrives = table;
    redrive_ranged = 0;
    redrive_indexed = 0;
}

// The class of `row`: the last one that starts at or before it.
static LLG_OUTLYING const llg_redrive_class_t* redrive_class(uint32_t row) {
    uint32_t low = 0, high = redrives->class_count;
    while (high - low > 1u) {
        uint32_t middle = low + (high - low) / 2u;
        if (redrives->classes[middle].first <= row) low = middle;
        else high = middle;
    }
    return &redrives->classes[low];
}

static LLG_OUTLYING int32_t redrive_compare(const void* left, const void* right) {
    uint32_t a = *(const uint32_t*)left, b = *(const uint32_t*)right;
    uintptr_t at = (uintptr_t)redrives->storage[a], bt = (uintptr_t)redrives->storage[b];
    if (at != bt) return at < bt ? -1 : 1;
    return a < b ? -1 : a > b;
}

static LLG_OUTLYING void redrive_index(void) {
    uint32_t keyed = redrives->count;
    redrive_ranged = 0;
    for (uint32_t index = 0; index < redrives->class_count; index++) {
        const llg_redrive_class_t* group = &redrives->classes[index];
        uint32_t end = index + 1u < redrives->class_count ? group[1].first : redrives->count;
        for (uint32_t row = group->first; row < end; row++) {
            if (group->cells) redrives->order[redrive_ranged++] = row;
            else redrives->order[--keyed] = row;
        }
    }
    llg_sort(redrives->order, redrive_ranged, sizeof(*redrives->order), redrive_compare);
    memset(redrives->busy, 0, redrives->count);
    redrive_indexed = 1;
}

// Wake the live process a PROCESS class names, like a change of a value it
// waits on: one already due to run, or running, will evaluate anyway.
// Releases are rare, so the registry is scanned; the wake is the VPI
// bridge's (llg_rt_thread_wake), so no scheduling path gains a caller.
static LLG_OUTLYING void redrive_wake(const llg_redrive_class_t* group, const void* record) {
    for (int32_t slot = 0; slot < g.n_procs; slot++) {
        llg_proc_t* proc = g.all_procs[slot];
        if (!proc || proc->completed || proc->killed || proc->chain.root != group->process)
            continue;
        if (group->instance_offset) {
            const void* held = NULL;
            memcpy(&held, (const uint8_t*)LLG_CO_ROOT(&proc->chain) + group->instance_offset,
                   sizeof(held));
            if (held != record) continue;
        }
        (void)llg_rt_thread_wake(proc->registry_slot, proc->assertion_owner);
        return;
    }
}

static LLG_OUTLYING void redrive_run(uint32_t row, void* cell) {
    if (redrives->busy[row]) return;
    const llg_redrive_class_t* group = redrive_class(row);
    const void* operand = redrives->operand[row];
    redrives->busy[row] = 1;
    switch (group->action) {
    case LLG_REDRIVE_COPY:
        llg_ba_from((sv4_t*)cell, (const sv4_t*)operand);
        break;
    case LLG_REDRIVE_LIMBS: {
        const sv4_t* target = (const sv4_t*)cell;
        const uint64_t* limbs = (const uint64_t*)operand;
        size_t count = ((size_t)llg_sv4_width(*target) + 63u) / 64u;
        sv4_t value = sv4_from_limbs(limbs, limbs + count, limbs + 2u * count,
                                     llg_sv4_width(*target), llg_sv4_signed(*target));
        llg_ba((sv4_t*)cell, value);
        sv4_destroy(&value);
        break;
    }
    case LLG_REDRIVE_COMMITTED:
        inertial_redrive((sv4_t*)cell);
        break;
    case LLG_REDRIVE_EVALUATE:
        if (!redrive_scheduled) group->evaluate((void*)(uintptr_t)operand);
        break;
    default:
        if (redrive_scheduled) redrive_wake(group, operand);
        break;
    }
    redrives->busy[row] = 0;
}

// The continuous drivers of the released variable at `cell` are
// re-established (IEEE 1800-2009 10.6.2; llg_redrive_class_t): in place with
// ordinary blocking writes, so bits another force still holds keep their
// forced value, or by waking the driver. A variable without a row is written
// by procedures only and keeps its value until the next of their
// assignments.
static LLG_OUTLYING void redrive(void* cell) {
    if (!redrives || !redrives->count || !cell) return;
    if (!redrive_indexed) redrive_index();
    uintptr_t key = (uintptr_t)cell;
    uint32_t low = 0, high = redrive_ranged;
    while (low < high) {
        uint32_t middle = low + (high - low) / 2u;
        if ((uintptr_t)redrives->storage[redrives->order[middle]] <= key) low = middle + 1u;
        else high = middle;
    }
    // Rows of one variable or array share their storage address and no two
    // variables overlap, so the rows of this cell end at the first miss.
    while (low) {
        uint32_t row = redrives->order[--low];
        const llg_redrive_class_t* group = redrive_class(row);
        if (key - (uintptr_t)redrives->storage[row] >= (uintptr_t)group->cells * group->stride)
            break;
        redrive_run(row, cell);
    }
    for (uint32_t keyed = redrive_ranged; keyed < redrives->count; keyed++) {
        uint32_t row = redrives->order[keyed];
        if (fixed_array_owns((const llg_fixed_array_t*)redrives->storage[row], cell))
            redrive_run(row, cell);
    }
}

LLG_OUTLYING void llg_release_parts_scheduled(const llg_force_part_t* parts, int32_t n_parts) {
    int32_t outer = redrive_scheduled;
    redrive_scheduled = 1;
    llg_release_parts(parts, n_parts, 0, 0);
    redrive_scheduled = outer;
}

LLG_OUTLYING void llg_release_real_scheduled(double* target) {
    int32_t outer = redrive_scheduled;
    redrive_scheduled = 1;
    llg_release_real(target);
    redrive_scheduled = outer;
}
