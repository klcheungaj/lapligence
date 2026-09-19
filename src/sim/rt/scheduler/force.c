
static sv4_t llg_net_compute(const llg_net_t* net);
static void force_recompute_target(sv4_t* target, llg_net_t* net);
static void llg_net_alias_refresh_all(llg_net_t* net);
static void inertial_unlink_pending(llg_inertial_t* driver);

// Grow one live-binding table to hold at least `needed` entries. The grown copy
// is completed before it replaces the old table, so an allocation failure
// aborts without a partially rebound registry. Only `used` entries are copied.
static void force_table_reserve(int needed) {
    if (needed <= g.force_capacity) return;
    int capacity = llg_registry_capacity(g.force_capacity, needed);
    llg_force_entry_t* grown = (llg_force_entry_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "force table");
    if (g.force_table)
        memcpy(grown, g.force_table, (size_t)g.force_count * sizeof(*grown));
    free(g.force_table);
    g.force_table = grown;
    g.force_capacity = capacity;
}

static void pca_table_reserve(int needed) {
    if (needed <= g.pca_capacity) return;
    int capacity = llg_registry_capacity(g.pca_capacity, needed);
    llg_pca_binding_t* grown = (llg_pca_binding_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "PCA table");
    if (g.pca_table)
        memcpy(grown, g.pca_table, (size_t)g.pca_count * sizeof(*grown));
    free(g.pca_table);
    g.pca_table = grown;
    g.pca_capacity = capacity;
}

static void pca_real_table_reserve(int needed) {
    if (needed <= g.pca_real_capacity) return;
    int capacity = llg_registry_capacity(g.pca_real_capacity, needed);
    llg_pca_real_binding_t* grown = (llg_pca_real_binding_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "real PCA table");
    if (g.pca_real_table)
        memcpy(grown, g.pca_real_table,
               (size_t)g.pca_real_count * sizeof(*grown));
    free(g.pca_real_table);
    g.pca_real_table = grown;
    g.pca_real_capacity = capacity;
}

// Is `sig` currently covered by a packed force part? Procedural writes are
// dropped while a signal is forced; net driver slots remain writable so their
// current resolved value can be exposed on release.
static int llg_is_forced(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int j = 0; j < entry->n_parts; j++)
            if (entry->parts[j].target == sig && sv4_to_bool(entry->masks[j])) return 1;
    }
    return 0;
}

static int llg_is_real_forced(double* target) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) return 1;
    }
    return 0;
}

static llg_pca_binding_t* pca_binding(sv4_t* target) {
    for (int i = 0; i < g.pca_count; i++) {
        if (g.pca_table[i].target == target) return &g.pca_table[i];
    }
    return NULL;
}

static int pca_active(sv4_t* target) {
    llg_pca_binding_t* binding = pca_binding(target);
    return binding && binding->active;
}

static llg_pca_real_binding_t* pca_real_binding(double* target) {
    for (int i = 0; i < g.pca_real_count; i++) {
        if (g.pca_real_table[i].target == target) return &g.pca_real_table[i];
    }
    return NULL;
}

static int pca_real_active(double* target) {
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    return binding && binding->active;
}

static void pca_set_enable(sv4_t* enable, int active) {
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    sv4_replace(value, sv4_from_u64(active ? 1 : 0, enable->width, enable->is_signed));
    sig_write(enable, *value);
    llg_value_scope_end(scope);
}

void llg_pca_assign(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding) {
        if (g.pca_count == INT_MAX) {
            fprintf(stderr, "llg runtime fatal: PCA binding count overflow\n");
            abort();
        }
        pca_table_reserve(g.pca_count + 1);
        binding = &g.pca_table[g.pca_count++];
        memset(binding, 0, sizeof(*binding));
        binding->target = target;
    }
    if (binding->active &&
        (binding->enable != enable || binding->site != site)) {
        pca_set_enable(binding->enable, 0);
    }
    binding->enable = enable;
    binding->site = site;
    sv4_replace(&binding->value, sv4_resize(value, target->width, target->is_signed));
    binding->active = 1;
    pca_set_enable(enable, 1);
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_drive(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active || binding->enable != enable || binding->site != site)
        return;
    sv4_replace(&binding->value, sv4_resize(value, target->width, target->is_signed));
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_deassign(sv4_t* target) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active) return;
    binding->active = 0;
    sv4_destroy(&binding->value);
    pca_set_enable(binding->enable, 0);
}

void llg_pca_assign_d(double* target, sv4_t* enable, uint64_t site, double value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding) {
        if (g.pca_real_count == INT_MAX) {
            fprintf(stderr, "llg runtime fatal: real PCA binding count overflow\n");
            abort();
        }
        pca_real_table_reserve(g.pca_real_count + 1);
        binding = &g.pca_real_table[g.pca_real_count++];
        memset(binding, 0, sizeof(*binding));
        binding->target = target;
    }
    if (binding->active &&
        (binding->enable != enable || binding->site != site)) {
        pca_set_enable(binding->enable, 0);
    }
    binding->enable = enable;
    binding->site = site;
    binding->value = value;
    binding->active = 1;
    pca_set_enable(enable, 1);
    if (!llg_is_real_forced(target)) real_write(target, value);
}

void llg_pca_drive_d(double* target, sv4_t* enable, uint64_t site, double value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding || !binding->active || binding->enable != enable || binding->site != site)
        return;
    binding->value = value;
    if (!llg_is_real_forced(target)) real_write(target, value);
}

void llg_pca_deassign_d(double* target) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding || !binding->active) return;
    binding->active = 0;
    pca_set_enable(binding->enable, 0);
}

static void force_free_entry(llg_force_entry_t* entry) {
    sv4_destroy(&entry->value);
    sv4_destroy_array(entry->masks, entry->masks ? (size_t)entry->n_parts : 0);
    free(entry->parts);
    free(entry->masks);
    free(entry->reads);
    memset(entry, 0, sizeof(*entry));
}

static sv4_t force_part_mask(const llg_force_part_t* part) {
    if (!part->target || part->target->width >= LLG_SUPPORTED_WIDTH_LIMIT ||
        part->width >= LLG_SUPPORTED_WIDTH_LIMIT) {
        fprintf(stderr, "llg: invalid force target or width\n");
        abort();
    }
    sv4_t mask = sv4_from_u64(0, part->target->width, 0);
    sv4_t ones = sv4_from_u64(0, part->width, 0);
    for (uint32_t bit = 0; bit < part->width; bit++)
        ones.bits[bit / 64u] |= UINT64_C(1) << (bit % 64u);
    sv4_part_select_set(&mask, part->left, part->right, ones);
    sv4_destroy(&ones);
    return mask;
}

// Force replacement and release affect target bits, not the shape of the
// original LHS or its RHS offsets. Overwritten forces never become active again.
static void force_remove_coverage(const llg_force_part_t* parts, int n_parts) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        int remains = 0;
        for (int j = 0; j < entry->n_parts; j++) {
            sv4_t* mask = &entry->masks[j];
            for (int k = 0; k < n_parts; k++) {
                if (entry->parts[j].target != parts[k].target) continue;
                sv4_t removed = force_part_mask(&parts[k]);
                for (uint32_t limb = 0; limb < ((mask->width + 63u) / 64u); limb++)
                    mask->bits[limb] &= ~removed.bits[limb];
                sv4_destroy(&removed);
            }
            remains |= sv4_to_bool(*mask);
        }
        if (!remains) force_free_entry(entry);
    }
}

static int force_find_free_slot(void) {
    for (int i = 0; i < g.force_count; i++)
        if (!g.force_table[i].active) return i;
    if (g.force_count == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: force entry count overflow\n");
        abort();
    }
    force_table_reserve(g.force_count + 1);
    return g.force_count++;
}

static llg_force_entry_t* force_prepare_packed(
    const llg_force_part_t* parts, int n_parts, uint32_t stream_slice,
    int stream_right_to_left, llg_force_eval_fn eval,
    const llg_force_read_t* reads, int n_reads) {
    if (!parts || n_parts <= 0 || n_reads < 0) {
        fprintf(stderr, "llg: invalid packed force descriptor\n");
        abort();
    }
    force_remove_coverage(parts, n_parts);
    int slot = force_find_free_slot();
    llg_force_entry_t* entry = &g.force_table[slot];
    memset(entry, 0, sizeof(*entry));
    entry->parts = (llg_force_part_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->parts), "force parts");
    memcpy(entry->parts, parts, (size_t)n_parts * sizeof(*parts));
    entry->masks = (sv4_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->masks), "force coverage masks");
    for (int i = 0; i < n_parts; i++) entry->masks[i] = force_part_mask(&parts[i]);
    if (n_reads > 0) {
        if (!reads) {
            fprintf(stderr, "llg: force dependency count has no descriptor array\n");
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
    return entry;
}

static int force_read_matches(const llg_force_entry_t* entry, sv4_t* sig,
                              double* real, int is_real) {
    for (int i = 0; i < entry->n_reads; i++) {
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
    uint32_t count = (target->width + 63u) / 64u;
    for (uint32_t i = 0; i < count; ++i) {
        uint64_t bits = mask->bits[i];
        target->bits[i] = (target->bits[i] & ~bits) | (updated.bits[i] & bits);
        target->x[i] = (target->x[i] & ~bits) | (updated.x[i] & bits);
        target->z[i] = (target->z[i] & ~bits) | (updated.z[i] & bits);
    }
    sv4_destroy(&updated);
    sv4_destroy(&selected);
}

static llg_net_t* force_net_for_target(sv4_t* target, llg_net_t* fallback) {
    if (fallback) return fallback;
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int j = 0; j < entry->n_parts; j++) {
            if (entry->parts[j].target == target && entry->parts[j].net)
                return entry->parts[j].net;
        }
    }
    return NULL;
}

static void force_recompute_target(sv4_t* target, llg_net_t* net) {
    net = force_net_for_target(target, net);
    if (net && net->propagation) inertial_unlink_pending(net->propagation);
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (net) {
        sv4_replace(value, llg_net_compute(net));
    } else {
        llg_pca_binding_t* pca = pca_binding(target);
        sv4_copy(value, pca && pca->active ? &pca->value : target);
    }
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        sv4_t streamed = entry->stream_slice
            ? sv4_unstream(entry->value, entry->stream_slice, entry->stream_right_to_left)
            : sv4_clone(&entry->value);
        for (int j = 0; j < entry->n_parts; j++) {
            llg_force_part_t* part = &entry->parts[j];
            if (part->target == target)
                force_apply_part(value, part, &entry->masks[j], &streamed);
        }
        sv4_destroy(&streamed);
    }
    sig_write(target, *value);
    if (net) llg_net_alias_refresh_all(net);
    llg_value_scope_end(scope);
}

static void force_entry_targets(const llg_force_entry_t* entry) {
    for (int i = 0; i < entry->n_parts; i++) {
        sv4_t* target = entry->parts[i].target;
        int seen = 0;
        for (int j = 0; j < i; j++)
            if (entry->parts[j].target == target) seen = 1;
        if (!seen) force_recompute_target(target, entry->parts[i].net);
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

static void force_dependency_changed(sv4_t* sig, double* real, int is_real) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && force_read_matches(entry, sig, real, is_real))
            force_evaluate_entry(entry);
    }
}

void llg_force_expr_parts(const llg_force_part_t* parts, int n_parts,
                          uint32_t stream_slice, int stream_right_to_left,
                          llg_force_eval_fn eval,
                          const llg_force_read_t* reads, int n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    llg_force_entry_t* entry = force_prepare_packed(
        parts, n_parts, stream_slice, stream_right_to_left, eval, reads, n_reads);
    force_evaluate_entry(entry);
}

void llg_force_real(double* target, llg_force_real_eval_fn eval,
                    const llg_force_read_t* reads, int n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    if (!target || !eval || n_reads < 0 || (n_reads > 0 && !reads)) {
        fprintf(stderr, "llg: invalid real force descriptor\n");
        abort();
    }
    llg_force_entry_t* entry = NULL;
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].active && g.force_table[i].is_real &&
            g.force_table[i].real_target == target) {
            entry = &g.force_table[i];
            free(entry->reads);
            entry->reads = NULL;
            break;
        }
    }
    if (!entry) {
        int slot = force_find_free_slot();
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
    force_evaluate_entry(entry);
}

void llg_release_parts(const llg_force_part_t* parts, int n_parts,
                       uint32_t stream_slice, int stream_right_to_left) {
    if (!region_can_mutate("force scheduling")) return;
    if (!parts || n_parts <= 0) return;
    (void)stream_slice;
    (void)stream_right_to_left;
    // Preserve net metadata before removing the final force on a target.
    llg_net_t** nets = llg_checked_malloc(
        (size_t)n_parts, sizeof(*nets), "released force nets");
    for (int i = 0; i < n_parts; i++)
        nets[i] = force_net_for_target(parts[i].target, parts[i].net);
    force_remove_coverage(parts, n_parts);
    for (int i = 0; i < n_parts; i++) {
        sv4_t* target = parts[i].target;
        int seen = 0;
        for (int j = 0; j < i; j++)
            if (parts[j].target == target) seen = 1;
        if (seen) continue;
        force_recompute_target(target, nets[i]);
        llg_pca_binding_t* pca = pca_binding(target);
        if (pca && pca->active) {
            pca_set_enable(pca->enable, 0);
            pca_set_enable(pca->enable, 1);
        }
    }
    free(nets);
}

void llg_release_real(double* target) {
    if (!region_can_mutate("force scheduling")) return;
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) {
            // A procedural real variable retains the forced value; no saved
            // value exists to restore.
            force_free_entry(entry);
            llg_pca_real_binding_t* pca = pca_real_binding(target);
            if (pca && pca->active) {
                pca_set_enable(pca->enable, 0);
                pca_set_enable(pca->enable, 1);
            }
            return;
        }
    }
}

void llg_force(sv4_t* sig, sv4_t value) {
    if (!region_can_mutate("force scheduling")) return;
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)sig->width - 1, 0, sig->width, 0, 0
    };
    llg_force_entry_t* entry = force_prepare_packed(&part, 1, 0, 0, NULL, NULL, 0);
    sv4_copy(&entry->value, &value);
    force_entry_targets(entry);
}

void llg_release(sv4_t* sig) {
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)sig->width - 1, 0, sig->width, 0, 0
    };
    llg_release_parts(&part, 1, 0, 0);
}
