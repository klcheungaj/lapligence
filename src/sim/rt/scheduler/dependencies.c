
// ── Signal writes and waiter scanning ─────────────────────────────────────────




static void force_dependency_changed(sv4_t* sig, double* real, int is_real);
static void sig_write(sv4_t* target, sv4_t value);
static void sig_write_now(sv4_t* target, sv4_t value);
static int marker_flip(sv4_t* marker, int rowless);
// Passed as the row of a publication whose caller has just found that its
// storage has no row: the publication skips its own lookup. Never read.
static llg_wait_source_t wait_source_absent;
struct llg_fixed_block;
static struct llg_fixed_block* fixed_array_block(const sv4_t* target);
static sv4_t* fixed_block_contents(const struct llg_fixed_block* block);
static void fixed_block_sampled_changed(struct llg_fixed_block* block,
                                        sv4_t* target, sv4_t old);
static int fixed_array_contents_waited(const sv4_t* target);
static sv4_t* fixed_array_contents_marker(const sv4_t* target);
static void change_forget(const void* key);
static void change_forget_history(llg_clocking_edge_t* edge);
static llg_change_t* change_find_unrowed(const void* target);
// Set while a check settles deferred changes (change_flush).
static int change_settling;

enum { LLG_DEPENDENCY_INITIAL_BUCKETS = 64u };

static size_t dependency_bucket(const void* target) {
    uint64_t hash = (uint64_t)(uintptr_t)target;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash & (llg_dependency_bucket_count - 1);
}

static const void* dependency_key(const llg_dependency_binding_t* binding) {
    return binding->key;
}

static llg_dependency_binding_t* dependency_bucket_head(const void* target) {
    return llg_dependency_bucket_count
        ? llg_dependency_buckets[dependency_bucket(target)] : NULL;
}

/* Keep the load factor at most one; growth rehashes from the owning list. */
static void dependency_index_reserve(void) {
    if (llg_dependency_binding_count < llg_dependency_bucket_count) return;
    size_t count = llg_dependency_bucket_count
        ? llg_dependency_bucket_count : LLG_DEPENDENCY_INITIAL_BUCKETS;
    while (count <= llg_dependency_binding_count) {
        if (count > SIZE_MAX / 2)
            llg_fatal_allocation("dependency binding index", count, 2);
        count *= 2;
    }
    llg_dependency_binding_t** buckets = (llg_dependency_binding_t**)llg_checked_calloc(
        count, sizeof(*buckets), "dependency binding index");
    free(llg_dependency_buckets);
    llg_dependency_buckets = buckets;
    llg_dependency_bucket_count = count;
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        size_t bucket = dependency_bucket(dependency_key(binding));
        binding->bucket_next = buckets[bucket];
        buckets[bucket] = binding;
    }
}

// Advances with every new binding, so a change record can cache whether its
// storage has bound markers (change_bound).
static uint64_t dependency_bind_epoch;

static void dependency_bind(const void* key, uint8_t kind, sv4_t* dependency,
                            const char* what) {
    for (llg_dependency_binding_t* binding = dependency_bucket_head(key);
         binding; binding = binding->bucket_next) {
        if (binding->key == key && binding->kind == kind &&
            binding->dependency == dependency) return;
    }
    dependency_index_reserve();
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), what);
    binding->key = key;
    binding->kind = kind;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
    size_t bucket = dependency_bucket(key);
    binding->bucket_next = llg_dependency_buckets[bucket];
    llg_dependency_buckets[bucket] = binding;
    ++llg_dependency_binding_count;
    ++dependency_bind_epoch;
}

void llg_dependency_bind(sv4_t* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid dependency binding\n");
        abort();
    }
    dependency_bind(target, LLG_DEPENDENCY_BINDING_PACKED, dependency, "dependency binding");
}

void llg_dependency_bind_real(double* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid real dependency binding\n");
        abort();
    }
    dependency_bind(target, LLG_DEPENDENCY_BINDING_REAL, dependency, "real dependency binding");
}

/* Destroy hook of a change marker owned by a frame object slot. */
void llg_dependency_marker_destroy(void* object) {
    sv4_t* marker = (sv4_t*)object;
    if (!marker) return;
    llg_clocking_forget_signal(marker);
    sv4_destroy(marker);
}

sv4_t* llg_dependency_or_never(sv4_t* dependency) {
    static sv4_t never = SV4_EMPTY;
    return dependency ? dependency : &never;
}

void llg_dependency_changed(sv4_t* dependency) {
    if (!dependency || marker_flip(dependency, 0)) return;
    uint64_t bit = llg_sv4_word(*dependency, 0, LLG_SV4_BITS) & 1u;
    /* A reference marker keeps its width: the value already stored through
     * the reference reaches its element before readers wake. */
    int reference = llg_sv4_width(*dependency) == LLG_NATIVE_REF_MARKER_WIDTH;
    if (LLG_CO_UNLIKELY(reference)) llg_native_ref_stored(dependency);
    /* Native strings and containers publish through this marker. A subscriber
     * may terminate the writer without returning through this function. */
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    value[0] = reference ? sv4_from_u64(2u | (bit ^ 1u),
                                        LLG_NATIVE_REF_MARKER_WIDTH, 0)
                         : sv4_from_u64(bit ^ 1u, 1, 0);
    sig_write_now(dependency, value[0]);
    llg_value_scope_end(scope);
}

void llg_dependency_notify(sv4_t* contents, sv4_t* shape, int change) {
    if (change & 1)
        llg_dependency_changed(contents);
    if (change & 2)
        llg_dependency_changed(shape);
}

static int ev_matches(sv4_t old, sv4_t new, int kind) {
    if (kind == LLG_EV_ANY) return !sv4_same(old, new);
    // Edge controls use only the LSB. Read without allocating one-bit values.
    unsigned a = llg_sv4_state(old, 0);
    unsigned b = llg_sv4_state(new, 0);
    if (a == 3) a = 2;
    if (b == 3) b = 2;
    return kind == LLG_EV_POSEDGE ? (a == 0 && b != 0) || (a == 2 && b == 1)
                                  : (a == 1 && b != 1) || (a == 2 && b == 0);
}

static int ev_inline_matches(llg_wait_t* wait, sv4_t value, int kind) {
    if (kind == LLG_EV_ANY) return !wait_inline_same(wait, value);
    const llg_wait_inline_payload_t* single = &wait->payload.single;
    unsigned a = !single->width || (single->words[0].bval & 1u)
        ? 2u : (unsigned)(single->words[0].aval & 1u);
    unsigned b = llg_sv4_state(value, 0);
    if (b == 3) b = 2;
    return kind == LLG_EV_POSEDGE ? (a == 0 && b != 0) || (a == 2 && b == 1)
                                  : (a == 1 && b != 1) || (a == 2 && b == 0);
}

// Signal publication calls this only after establishing that some bit changed.
// Directional controls still inspect the packed value's LSB.
static int ev_matches_changed(sv4_t old, sv4_t new, int kind) {
    return kind == LLG_EV_ANY || ev_matches(old, new, kind);
}

/* Publication precedes callbacks and wait registration. Clocking drives and
 * ##0 may first query a descriptor AFTER its edge, so even automatic locals
 * must be recorded without a current subscriber. Ordinary waits use snapshots,
 * sampled inputs use sampling.c, and named-event .triggered uses event objects;
 * none of those reads this table. Entries with no sequence consumer die at time
 * advance. Registered sequence clocks keep cumulative directional ticks because
 * same-clock delays subtract ticks across slots; their occurrence flags expire.
 * Scoped descriptors are forgotten at final owner release (not lexical exit:
 * an NBA may still retain them). External owners must forget before freeing or
 * reusing a descriptor. Equality is exact pointer identity, never payload or
 * ordering of unrelated pointers. The list owns entries; the index borrows them.
 * Occurrence readers are clocking drives, ##0 cycle waits and assertion clock
 * pulses; a model without them calls llg_rt_skip_clocking_history and then only
 * the registered sequence clocks are recorded, for their tick counts.
 */
enum {
    CLOCKING_ANY = 1u,
    CLOCKING_POSEDGE = 2u,
    CLOCKING_NEGEDGE = 4u,
    CLOCKING_INDEX_INITIAL_CAPACITY = 16u
};

/* A private object supplies a portable tombstone without a fabricated pointer. */
static llg_clocking_edge_t clocking_deleted_entry;

static size_t clocking_edge_hash(const void* signal) {
    uint64_t hash = (uint64_t)(uintptr_t)signal;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void clocking_index_rebuild(size_t capacity) {
    llg_clocking_edge_t** index = (llg_clocking_edge_t**)llg_checked_calloc(
        capacity, sizeof(*index), "clocking history index");
    for (llg_clocking_edge_t* edge = g.clocking_edges; edge; edge = edge->next) {
        size_t slot = clocking_edge_hash(edge->signal) & (capacity - 1);
        while (index[slot]) slot = (slot + 1) & (capacity - 1);
        index[slot] = edge;
    }
    free(g.clocking_index);
    g.clocking_index = index;
    g.clocking_capacity = capacity;
    g.clocking_used = g.clocking_count;
}

static size_t clocking_edge_slot(sv4_t* signal) {
    size_t slot = clocking_edge_hash(signal) & (g.clocking_capacity - 1);
    while (g.clocking_index[slot]) {
        llg_clocking_edge_t* edge = g.clocking_index[slot];
        if (edge != &clocking_deleted_entry && edge->signal == signal) break;
        slot = (slot + 1) & (g.clocking_capacity - 1);
    }
    return slot;
}

static llg_clocking_edge_t* find_clocking_edge(sv4_t* signal) {
    if (!g.clocking_count || !signal) return NULL;
    return g.clocking_index[clocking_edge_slot(signal)];
}

static llg_clocking_edge_t* clocking_edge_get(sv4_t* signal) {
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (edge) return edge;
    size_t capacity = g.clocking_capacity ? g.clocking_capacity
                                         : CLOCKING_INDEX_INITIAL_CAPACITY;
    if (g.clocking_count >= capacity - capacity / 4) {
        if (capacity > SIZE_MAX / 2)
            llg_fatal_allocation("clocking history index", capacity, 2);
        capacity *= 2;
    }
    /* Rehash tombstones before occupied slots can make a probe unbounded. */
    if (capacity != g.clocking_capacity ||
        g.clocking_used >= capacity - capacity / 4)
        clocking_index_rebuild(capacity);
    size_t slot = clocking_edge_hash(signal) & (capacity - 1);
    while (g.clocking_index[slot] &&
           g.clocking_index[slot] != &clocking_deleted_entry)
        slot = (slot + 1) & (capacity - 1);
    if (!g.clocking_index[slot]) ++g.clocking_used;
    edge = (llg_clocking_edge_t*)llg_checked_calloc(
        1, sizeof(*edge), "clocking event history");
    edge->signal = signal;
    edge->next = g.clocking_edges;
    if (edge->next) edge->next->prev = edge;
    g.clocking_edges = edge;
    g.clocking_index[slot] = edge;
    ++g.clocking_count;
    return edge;
}

void llg_clocking_forget_signal(sv4_t* signal) {
    if (LLG_CO_UNLIKELY((g.change_rows | g.change_unrowed) != 0)) change_forget(signal);
    if (!g.clocking_count || !signal) return;
    size_t slot = clocking_edge_slot(signal);
    llg_clocking_edge_t* edge = g.clocking_index[slot];
    if (!edge) return;
    if (LLG_CO_UNLIKELY(edge->change != NULL)) change_forget_history(edge);
    if (edge->prev) edge->prev->next = edge->next;
    else g.clocking_edges = edge->next;
    if (edge->next) edge->next->prev = edge->prev;
    g.clocking_index[slot] = &clocking_deleted_entry;
    --g.clocking_count;
    free(edge);
}

static void clocking_advance_time(void) {
    llg_clocking_edge_t* edge = g.clocking_edges;
    while (edge) {
        llg_clocking_edge_t* next = edge->next;
        if (edge->keep_ticks) edge->occurred = 0;
        else llg_clocking_forget_signal(edge->signal);
        edge = next;
    }
    if (!g.clocking_count) {
        free(g.clocking_index);
        g.clocking_index = NULL;
        g.clocking_capacity = 0;
        g.clocking_used = 0;
    } else {
        size_t capacity = CLOCKING_INDEX_INITIAL_CAPACITY;
        while (g.clocking_count > capacity - capacity / 4) capacity *= 2;
        /* Bound retained index storage to surviving sequence-clock identities. */
        if (capacity != g.clocking_capacity || g.clocking_used != g.clocking_count)
            clocking_index_rebuild(capacity);
    }
}

void llg_rt_skip_clocking_history(void) {
    g.clocking_history_skipped = 1;
}

/* Without history consumers this is one empty-table test per publication
 * instead of an entry allocation, index growth and a time-advance free. */
static void clocking_edge_note(llg_clocking_edge_t* edge, sv4_t old, sv4_t value) {
    if (edge->time != g.now) edge->occurred = 0;
    edge->time = g.now;
    edge->occurred |= CLOCKING_ANY;
    if (ev_matches(old, value, LLG_EV_POSEDGE)) {
        edge->occurred |= CLOCKING_POSEDGE;
        if (edge->keep_ticks && edge->posedge_count != UINT64_MAX)
            edge->posedge_count++;
    }
    if (ev_matches(old, value, LLG_EV_NEGEDGE)) {
        edge->occurred |= CLOCKING_NEGEDGE;
        if (edge->keep_ticks && edge->negedge_count != UINT64_MAX)
            edge->negedge_count++;
    }
}

static void clocking_record_edge(sv4_t* signal, sv4_t old, sv4_t value) {
    if (!signal) return;
    llg_clocking_edge_t* edge = g.clocking_history_skipped
        ? find_clocking_edge(signal)
        : clocking_edge_get(signal);
    if (edge) clocking_edge_note(edge, old, value);
}

static uint64_t assertion_clock_tick(sv4_t* signal, int edge_kind) {
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (!edge) return 0;
    return edge_kind == LLG_EV_POSEDGE ? edge->posedge_count
                                       : edge->negedge_count;
}

static int clocking_event_current(const llg_wait_src_t* srcs, int n) {
    if (!srcs || n <= 0) return 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            // A change this turn has not published yet (T1-D11) is the
            // clocking event of this time step: its check follows in the
            // same slot, so a drive or cycle wait issued now treats it as
            // occurred instead of slipping to the next one.
            llg_clocking_edge_t* edge = find_clocking_edge(srcs[i].sig);
            if (LLG_CO_UNLIKELY(g.change_count)) {
                llg_wait_source_t* source = wait_source_find(srcs[i].sig);
                llg_change_t* change = source && source->change ? source->change
                                     : edge && edge->change ? edge->change
                                     : change_find_unrowed(srcs[i].sig);
                if (change && (change->kind == LLG_CHANGE_PACKED ||
                               change->kind == LLG_CHANGE_ROWLESS) &&
                    ev_matches(change->before, *srcs[i].sig, srcs[i].kind))
                    return 1;
            }
            if (!edge) continue;
            unsigned occurred = srcs[i].kind == LLG_EV_POSEDGE
                                    ? CLOCKING_POSEDGE
                                    : srcs[i].kind == LLG_EV_NEGEDGE
                                          ? CLOCKING_NEGEDGE : CLOCKING_ANY;
            if (edge->time == g.now && (edge->occurred & occurred)) return 1;
        } else if (srcs[i].ev && llg_event_triggered(srcs[i].ev)) {
            return 1;
        }
    }
    return 0;
}

static void clocking_drive_sources_acquire(const llg_clocking_drive_t* drive) {
    for (int i = 0; i < drive->n_specs; i++) {
        sv4_t* sig = drive->specs[i].sig;
        if (!sig) {
            g.clocking_drive_event_refs++;
            continue;
        }
        size_t at = 0;
        while (at < g.clocking_drive_source_count &&
               g.clocking_drive_sources[at].sig != sig)
            at++;
        if (at == g.clocking_drive_source_count) {
            if (at == g.clocking_drive_source_capacity) {
                size_t capacity = at ? at * 2 : 4;
                llg_clocking_drive_source_t* grown =
                    (llg_clocking_drive_source_t*)llg_checked_malloc(
                        capacity, sizeof(*grown), "clocking drive sources");
                if (at) memcpy(grown, g.clocking_drive_sources, at * sizeof(*grown));
                free(g.clocking_drive_sources);
                g.clocking_drive_sources = grown;
                g.clocking_drive_source_capacity = capacity;
            }
            g.clocking_drive_sources[at].sig = sig;
            g.clocking_drive_sources[at].refs = 0;
            g.clocking_drive_source_count++;
        }
        g.clocking_drive_sources[at].refs++;
    }
}

static void clocking_drive_sources_release(const llg_clocking_drive_t* drive) {
    for (int i = 0; i < drive->n_specs; i++) {
        sv4_t* sig = drive->specs[i].sig;
        if (!sig) {
            if (g.clocking_drive_event_refs) g.clocking_drive_event_refs--;
            continue;
        }
        for (size_t at = 0; at < g.clocking_drive_source_count; at++) {
            if (g.clocking_drive_sources[at].sig != sig) continue;
            if (--g.clocking_drive_sources[at].refs == 0)
                g.clocking_drive_sources[at] =
                    g.clocking_drive_sources[--g.clocking_drive_source_count];
            break;
        }
    }
}

static int clocking_drive_source_pending(const sv4_t* sig) {
    for (size_t at = 0; at < g.clocking_drive_source_count; at++)
        if (g.clocking_drive_sources[at].sig == sig) return 1;
    return 0;
}

static void free_clocking_drive(llg_clocking_drive_t* drive) {
    if (!drive) return;
    sv4_destroy(&drive->value);
    sv4_destroy(&drive->mask);
    value_scope_release(drive->target_scope);
    free(drive->specs);
    free(drive);
}

static void clocking_drive_enqueue(const llg_clocking_drive_t* drive) {
    llg_nba_t* n = new_clocking_nba(drive->ticks);
    if (!n) return;
    n->target = drive->target;
    n->target_scope = value_scope_retain_target(drive->target);
    n->net_target = drive->net_target;
    n->net_slot = drive->net_slot;
    sv4_copy(&n->value, &drive->value);
    sv4_copy(&n->mask, &drive->mask);
    n->has_mask = drive->has_mask;
    n->range_offset = drive->range_offset;
    n->range_width = drive->range_width;
    n->has_range = drive->has_range;
    n->is_real = drive->is_real;
    n->real_target = drive->real_target;
    n->real_value = drive->real_value;
    enqueue_nba(n);
}

static int clocking_drive_source_matches_signal(
    const llg_clocking_drive_t* drive, sv4_t* signal, sv4_t old, sv4_t value) {
    for (int i = 0; i < drive->n_specs; i++) {
        const llg_wait_src_t* source = &drive->specs[i];
        if (source->sig == signal &&
            ev_matches_changed(old, value, source->kind)) return 1;
    }
    return 0;
}

static int clocking_drive_source_matches_event(
    const llg_clocking_drive_t* drive, llg_event_object_t* event) {
    for (int i = 0; i < drive->n_specs; i++) {
        const llg_wait_src_t* source = &drive->specs[i];
        if (source->ev && source->ev->object == event) return 1;
    }
    return 0;
}

static void clocking_drive_signal_match(sv4_t* signal, sv4_t old, sv4_t value) {
    if (!g.clocking_drives || !clocking_drive_source_pending(signal)) return;
    int removed = 0;
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_signal(drive, signal, old, value)) {
            slot = &drive->next;
            continue;
        }
        if (--drive->events) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        clocking_drive_sources_release(drive);
        free_clocking_drive(drive);
        removed = 1;
    }
    if (!removed) return;
    g.clocking_drives_tail = g.clocking_drives;
    while (g.clocking_drives_tail && g.clocking_drives_tail->next)
        g.clocking_drives_tail = g.clocking_drives_tail->next;
}

static void clocking_drive_event_match(llg_event_object_t* event) {
    if (!g.clocking_drive_event_refs) return;
    int removed = 0;
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_event(drive, event)) {
            slot = &drive->next;
            continue;
        }
        if (--drive->events) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        clocking_drive_sources_release(drive);
        free_clocking_drive(drive);
        removed = 1;
    }
    if (!removed) return;
    g.clocking_drives_tail = g.clocking_drives;
    while (g.clocking_drives_tail && g.clocking_drives_tail->next)
        g.clocking_drives_tail = g.clocking_drives_tail->next;
}

static int real_same(double old, double new) {
    uint64_t old_bits;
    uint64_t new_bits;
    memcpy(&old_bits, &old, sizeof(old_bits));
    memcpy(&new_bits, &new, sizeof(new_bits));
    return old_bits == new_bits;
}

static int real_ev_matches(double old, double new, int kind) {
    // Edge descriptors are rejected by lowering for real values. Keep the
    // runtime defensive: real event controls are any-change only.
    return kind == LLG_EV_ANY && !real_same(old, new);
}

static int dependency_matches(const llg_wait_dependency_t* dependency,
                              sv4_t* sig, double* real) {
    return (dependency->sig && dependency->sig == sig) ||
           (dependency->real && dependency->real == real);
}

static int expression_dependency_changed(const llg_expr_event_spec_t* spec,
                                          sv4_t* sig, double* real) {
    if ((spec->sig && spec->sig == sig) ||
        (spec->real_sig && spec->real_sig == real))
        return 1;
    if (spec->n_dependencies > 0) {
        for (int i = 0; i < spec->n_dependencies; i++) {
            if (dependency_matches(&spec->dependencies[i], sig, real)) return 1;
        }
    } else {
        for (int i = 0; i < spec->n_reads; i++) {
            if (spec->reads[i] == sig) return 1;
        }
    }
    return 0;
}

static int expression_gate(const llg_wait_t* wait, int index) {
    const llg_wait_expression_payload_t* payload = &wait->payload.expression;
    if (change_settling && payload->gate_open && payload->expressions[index].condition &&
        (wait->proc->wait_gate & LLG_WAIT_GATE_ARMED))
        return payload->gate_open[index];
    return expression_qualifies(&payload->expressions[index]);
}

static int expression_update(llg_wait_t* wait, int index, sv4_t* sig,
                             double* real) {
    llg_wait_expression_payload_t* payload = &wait->payload.expression;
    llg_expr_event_spec_t* spec = &payload->expressions[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) {
            spec->real_eval(&value, spec->eval_context);
            if (llg_rt_exiting()) return 0;
        }
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(payload->real_last[index], value, spec->kind);
        payload->real_last[index] = value;
        return matched && expression_gate(wait, index);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) {
        spec->eval(value, spec->eval_context);
        if (llg_rt_exiting()) {
            llg_value_scope_end(scope);
            return 0;
        }
    }
    else sv4_copy(value, spec->sig);
    int matched = ev_matches(payload->last[index], *value, spec->kind);
    sv4_move(&payload->last[index], value);
    llg_value_scope_end(scope);
    return matched && expression_gate(wait, index);
}

// A gate-at-write wait latches its qualifier at each write after which the
// expression has its event against the value last seen (AA-D6); the check
// then uses the latch of the last such write.
static void expression_gate_latch(llg_wait_t* wait, int index, sv4_t* sig,
                                  double* real) {
    llg_wait_expression_payload_t* payload = &wait->payload.expression;
    llg_expr_event_spec_t* spec = &payload->expressions[index];
    if (!payload->gate_open || !spec->condition || spec->event ||
        !expression_dependency_changed(spec, sig, real))
        return;
    int matched;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) {
            spec->real_eval(&value, spec->eval_context);
            if (llg_rt_exiting()) return;
        } else if (spec->real_sig) value = *spec->real_sig;
        else return;
        matched = real_ev_matches(payload->real_last[index], value, spec->kind);
    } else {
        if (!spec->eval && !spec->sig) return;
        llg_value_scope_t* scope = llg_value_scope_begin(1);
        sv4_t* value = llg_value_scope_values(scope);
        if (spec->eval) {
            spec->eval(value, spec->eval_context);
            if (llg_rt_exiting()) {
                llg_value_scope_end(scope);
                return;
            }
        } else sv4_copy(value, spec->sig);
        matched = ev_matches(payload->last[index], *value, spec->kind);
        llg_value_scope_end(scope);
    }
    if (matched) payload->gate_open[index] = (uint8_t)(expression_qualifies(spec) != 0);
}

static int deferred_expression_update(llg_deferred_trigger_t* trigger,
                                      int index, sv4_t* sig, double* real) {
    llg_expr_event_spec_t* spec = &trigger->specs[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) {
            spec->real_eval(&value, spec->eval_context);
            if (llg_rt_exiting()) return 0;
        }
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(trigger->real_last[index], value, spec->kind);
        trigger->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) {
        spec->eval(value, spec->eval_context);
        if (llg_rt_exiting()) {
            llg_value_scope_end(scope);
            return 0;
        }
    }
    else sv4_copy(value, spec->sig);
    int matched = ev_matches(trigger->last[index], *value, spec->kind);
    sv4_move(&trigger->last[index], value);
    llg_value_scope_end(scope);
    return matched && expression_qualifies(spec);
}

static void invoke_deferred_action(llg_event_assignment_fn action,
                                   llg_frame_t* frame) {
    if (!action) {
        if (frame) llg_frame_release(frame);
        return;
    }
    int was_in_deferred_action = g.in_deferred_action;
    g.in_deferred_action = 1;
    action(frame);
    g.in_deferred_action = was_in_deferred_action;
    if (frame) llg_frame_release(frame);
}

static void deferred_trigger_fire(llg_deferred_trigger_t* trigger) {
    if (trigger->action) {
        llg_frame_t* frame = trigger->action_frame;
        trigger->action_frame = NULL;
        invoke_deferred_action(trigger->action, frame);
        return;
    }
    llg_nba_t* n = new_nba(0);
    if (!n) return;
    // The retained request is independent of both the issuer and the process
    // that happened to produce the matching source change.
    n->owner = NULL;
    n->event_target = trigger->target;
    n->is_event = 1;
    enqueue_nba(n);
}

static void deferred_trigger_source_change(sv4_t* sig, double* real) {
    llg_deferred_trigger_t** slot = &g.deferred_triggers;
    while (*slot) {
        llg_deferred_trigger_t* trigger = *slot;
        int matched = 0;
        for (int i = 0; i < trigger->n; i++) {
            if (deferred_expression_update(trigger, i, sig, real)) {
                matched = 1;
                break;
            }
        }
        if (!matched) {
            slot = &trigger->next;
            continue;
        }
        if (trigger->remaining > 1) {
            trigger->remaining--;
            slot = &trigger->next;
            continue;
        }
        *slot = trigger->next;
        if (g.deferred_trigger_tail == trigger) {
            g.deferred_trigger_tail = NULL;
            for (llg_deferred_trigger_t* tail = g.deferred_triggers; tail;
                 tail = tail->next)
                g.deferred_trigger_tail = tail;
        }
        deferred_trigger_fire(trigger);
        free_deferred_trigger(trigger);
    }
}

static void deferred_trigger_event(llg_event_object_t* ev) {
    if (!ev) return;
    llg_deferred_trigger_t** slot = &g.deferred_triggers;
    while (*slot) {
        llg_deferred_trigger_t* trigger = *slot;
        int matched = 0;
        for (int i = 0; i < trigger->n; i++) {
            llg_expr_event_spec_t* spec = &trigger->specs[i];
            if (spec->event_object == ev && expression_qualifies(spec)) {
                matched = 1;
                break;
            }
        }
        if (!matched) {
            slot = &trigger->next;
            continue;
        }
        if (trigger->remaining > 1) {
            trigger->remaining--;
            slot = &trigger->next;
            continue;
        }
        *slot = trigger->next;
        if (g.deferred_trigger_tail == trigger) {
            g.deferred_trigger_tail = NULL;
            for (llg_deferred_trigger_t* tail = g.deferred_triggers; tail;
                 tail = tail->next)
                g.deferred_trigger_tail = tail;
        }
        deferred_trigger_fire(trigger);
        free_deferred_trigger(trigger);
    }
}

// A publication of `target` seen by an ordered dependency wait, armed or
// between wakes: like W_DEPS, a whole dependency changes on every
// publication and a selected one only when its bits changed. Each changed
// dependency is logged with its offset; returns whether any changed.
static int wait_change_log_packed(llg_wait_change_log_t* log, sv4_t* target) {
    llg_wait_retained_t* retained = log->sink.proc->retained_wait;
    int changed = 0;
    for (int i = 0; i < retained->n; i++) {
        const llg_wait_dependency_t* dependency = &retained->dependencies[i];
        if (dependency->sig != target) continue;
        if (!dependency->width) {
            wait_change_note(log, target, LLG_WAIT_CHANGE_WHOLE);
            changed = 1;
            continue;
        }
        sv4_t selected = sv4_part_select(
            dependency->value ? *dependency->value : *target,
            (int64_t)dependency->lsb + dependency->width - 1, dependency->lsb);
        if (!sv4_same(retained->last[i], selected)) {
            wait_change_note(log, target, dependency->lsb);
            changed = 1;
        }
        sv4_move(&retained->last[i], &selected);
    }
    return changed;
}

// ── Checks (T1-D11) ──────────────────────────────────────────────────────────
//
// A process turn or an NBA region stores each write at once and runs the
// per-write consumers, but what depends on the change itself waits for a
// check: the first write of a row opens a record (llg_change_t) holding the
// value before it, and the check compares that value with the current one.
// Equal values publish nothing, so only the first and the last value count.
// What runs at the check is `sig_settle`/`real_settle`: clock history, clocking
// drives, sampled and assertion clocks, waiters, deferred triggers, and the
// change markers of cells and bound storage. Waiter snapshots already hold the
// value at the last check, so edges compare first to last with no extra logic.
//
// Writes outside both (startup, region callbacks, net settling after a turn) and
// every other change marker publish at once, as do strings and containers.
//
// A write that nothing depends on opens no record: a signal nobody waits on
// pays the row lookup it always paid.

// Set during a process turn and during an NBA region's commit.
static int change_defer_active;

static int wait_source_armed(const llg_wait_source_t* source) {
    if (!source) return 0;
    for (const llg_wait_subscription_t* node = source->head; node; node = node->next)
        if (node->wait) return 1;
    return 0;
}

static int change_marker_is_reference(const sv4_t* marker) {
    return llg_sv4_width(*marker) == LLG_NATIVE_REF_MARKER_WIDTH;
}

// A reference marker carries the stored value on to its element, so it runs at
// the write. Other bound markers count when something waits on them now: a
// waiter that arms later in this turn has no change to see.
static int dependency_bindings_waited(const void* target, uint8_t kind) {
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key != target || binding->kind != kind ||
            change_marker_is_reference(binding->dependency))
            continue;
        if (wait_source_armed(wait_source_find(binding->dependency))) return 1;
    }
    return 0;
}

static llg_change_t* change_take(llg_change_kind_t kind) {
    llg_change_t* change = g.change_free;
    if (change) g.change_free = (llg_change_t*)change->cursor.next;
    else change = llg_checked_calloc(1, sizeof(*change), "value change");
    if (g.change_count == g.change_capacity) {
        size_t capacity = g.change_capacity ? g.change_capacity * 2 : 16;
        llg_change_t** grown = llg_checked_malloc(capacity, sizeof(*grown), "value changes");
        if (g.change_count) memcpy(grown, g.changes, g.change_count * sizeof(*grown));
        free(g.changes);
        g.changes = grown;
        g.change_capacity = capacity;
    }
    change->kind = kind;
    change->fire = 0;
    change->edge = NULL;
    change->owner = NULL;
    change->bind_epoch = UINT64_MAX;
    change->contents_known = 0;
    change->fires_exact = 0;
    change->serial = ++g.change_serial;
    change->index = g.change_count;
    g.changes[g.change_count++] = change;
    return change;
}

static void change_attach(llg_change_t* change, llg_wait_source_t* source) {
    ++g.change_rows;
    change->key = source->key;
    change->cursor.source = source;
    change->cursor.wait = NULL;
    wait_subscription_insert(&change->cursor, &source->head);
    source->change = change;
}

static llg_change_t* change_open(llg_wait_source_t* source, llg_change_kind_t kind) {
    llg_change_t* change = change_take(kind);
    change_attach(change, source);
    return change;
}

// Unlinking the cursor may free the row. `before` keeps its limbs for the next
// record of the same width: most writes need no allocation.
static void change_close(llg_change_t* change) {
    if (change->kind == LLG_CHANGE_ROWLESS) {
        if (change->edge) change->edge->change = NULL;
        else --g.change_unrowed;
    } else {
        --g.change_rows;
        change->cursor.source->change = NULL;
        wait_subscription_unlink(&change->cursor);
    }
    // A cleared serial invalidates the references marker records and
    // storage records keep to this one (`owner`, `fires`).
    change->serial = 0;
    change->cursor.next = (llg_wait_subscription_t*)g.change_free;
    g.change_free = change;
}

// The rowless record that opened `marker`, while it is open.
static llg_change_t* change_marker_owner(const llg_change_t* marker) {
    if (!marker || marker->kind != LLG_CHANGE_MARKER || !marker->owner) return NULL;
    return marker->owner->serial == marker->owner_serial ? marker->owner : NULL;
}

// The rowless record of `target` that no clocking history entry holds: found
// through the marker records it opened (its bound markers or its array's
// contents marker). NULL when none is open.
static llg_change_t* change_find_unrowed(const void* target) {
    if (LLG_CO_LIKELY(!g.change_unrowed)) return NULL;
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key != target || binding->kind != LLG_DEPENDENCY_BINDING_PACKED)
            continue;
        llg_wait_source_t* source = wait_source_find(binding->dependency);
        llg_change_t* owner = change_marker_owner(source ? source->change : NULL);
        if (owner && owner->key == target) return owner;
    }
    sv4_t* contents = fixed_array_contents_marker((const sv4_t*)target);
    llg_wait_source_t* source = contents ? wait_source_find(contents) : NULL;
    llg_change_t* owner = change_marker_owner(source ? source->change : NULL);
    return owner && owner->key == target ? owner : NULL;
}

// A copy of a value of at most 64 bits is a plain store on the compact
// backends; a wider one reuses the limbs of the pooled record when the width
// matches, so most writes allocate nothing.
static void change_save_before(llg_change_t* change, const sv4_t* before) {
    uint32_t width = llg_sv4_width(*before);
    if (width > 64 && llg_sv4_width(change->before) == width)
        llg_sv4_range_copy(&change->before, 0, *before);
    else
        sv4_copy(&change->before, before);
}

// Whether packed markers are bound to the record's storage: looked up once
// per record unless a binding was added since.
static int change_bound(llg_change_t* change, const void* key) {
    if (change->bind_epoch != dependency_bind_epoch) {
        change->bind_epoch = dependency_bind_epoch;
        change->bound = 0;
        for (llg_dependency_binding_t* binding = dependency_bucket_head(key);
             binding; binding = binding->bucket_next) {
            if (binding->key == key && binding->kind == LLG_DEPENDENCY_BINDING_PACKED) {
                change->bound = 1;
                break;
            }
        }
    }
    return change->bound;
}

// The record of a packed storage something waits on now. A rowless record
// opened by an earlier write of this turn moves to the row, keeping its
// before value, so that the storage keeps one record.
// `source` is the storage's row, or NULL when it has none.
static llg_wait_source_t* change_open_packed(llg_wait_source_t* source,
                                             sv4_t* target, const sv4_t* before) {
    if (!source) source = wait_source_insert(target);
    // Only a turn with a rowless record open can hold one for this storage.
    llg_change_t* moved = NULL;
    if (g.change_count > g.change_rows) {
        llg_clocking_edge_t* edge = find_clocking_edge(target);
        if (edge && edge->change) {
            moved = edge->change;
            edge->change = NULL;
            moved->edge = NULL;
        } else if ((moved = change_find_unrowed(target)) != NULL) {
            --g.change_unrowed;
        }
    }
    if (moved) {
        moved->kind = LLG_CHANGE_PACKED;
        change_attach(moved, source);
    } else {
        change_save_before(change_open(source, LLG_CHANGE_PACKED), before);
    }
    return source;
}

// Called before a deferred packed write stores, or with the old value after
// it. Returns the row, whose `change` tells whether a check will settle it.
// `eager` also opens the record for waited bound markers and array contents
// markers; otherwise sig_publish opens it while it routes them (one pass over
// the bindings instead of two). A storage that only clocking history observes
// gets a history record, kept on its history entry: no row is created and
// freed per write.
static llg_wait_source_t* change_begin(sv4_t* target, const sv4_t* before, int eager) {
    llg_wait_source_t* source = wait_source_find(target);
    if (source && source->change) return source;
    if (wait_source_armed(source) || g.deferred_triggers ||
        (eager && (dependency_bindings_waited(target, LLG_DEPENDENCY_BINDING_PACKED) ||
                   fixed_array_contents_waited(target))))
        return change_open_packed(source, target, before);
    llg_clocking_edge_t* edge = g.clocking_history_skipped ? find_clocking_edge(target)
                                                           : clocking_edge_get(target);
    if (edge && !edge->change) {
        llg_change_t* change = change_take(LLG_CHANGE_ROWLESS);
        change->key = target;
        change->edge = edge;
        edge->change = change;
        change_save_before(change, before);
    }
    return source;
}

// A marker with a waiter now toggles at the check: its record's cursor leaves
// out waiters armed after this write, as a toggle at the write would. NULL
// for a marker nothing waits on, which toggles at once; and for a reference
// marker, which carries the stored value on and so runs at the write. A
// marker bound to further markers counts only its own waiters: those further
// markers toggle at its check when it has a record (LLG_BOUND_SETTLE), and
// at each write otherwise, which keeps this test to one row lookup.
// `rowless`, when given, is set when the marker has no row, so that the
// caller toggling it now skips a second lookup (marker_toggle).
// `opened`, when given, is set when this call opened the record.
static llg_change_t* change_marker_defer(sv4_t* marker, int* rowless, int* opened) {
    llg_wait_source_t* source = wait_source_find(marker);
    if (rowless) *rowless = source == NULL;
    if (opened) *opened = 0;
    if (source && source->change)
        return source->change->kind == LLG_CHANGE_MARKER ? source->change : NULL;
    if (!(g.deferred_triggers || wait_source_armed(source)) ||
        change_marker_is_reference(marker))
        return NULL;
    if (!source) source = wait_source_get(marker);
    llg_change_t* change = change_open(source, LLG_CHANGE_MARKER);
    change->slot = NULL;
    change->fire = 0;
    if (opened) *opened = 1;
    return change;
}

// Toggles a marker now; `rowless` as for marker_flip.
static void marker_toggle(sv4_t* marker, int rowless) {
    if (!(rowless && marker_flip(marker, 1))) llg_dependency_changed(marker);
}

// A deferred write of storage bound to markers: those with a record wait for
// the storage's own check, the others toggle now.
static void defer_bound_markers(const void* target, uint8_t kind) {
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key != target || binding->kind != kind) continue;
        int rowless;
        if (!change_marker_defer(binding->dependency, &rowless, NULL))
            marker_toggle(binding->dependency, rowless);
    }
}

void llg_handle_changed(sv4_t* dependency, void* const* slot, void* previous) {
    if (!dependency) return;
    if (change_defer_active && slot) {
        llg_wait_source_t* source = wait_source_find(dependency);
        llg_change_t* change = source ? source->change : NULL;
        if (change && change->kind == LLG_CHANGE_MARKER) {
            // One record per marker: another slot sharing it (two handle
            // properties of one object) or a bound cell toggles it.
            if (change->slot != slot) change->fire = 1;
            return;
        }
        if (!change && (change = change_marker_defer(dependency, NULL, NULL)) != NULL) {
            change->slot = slot;
            change->before_handle = previous;
            return;
        }
    }
    llg_dependency_changed(dependency);
}

// The check of changed storage: its deferred markers will toggle.
// A marker whose record is open toggles at its check.
static void fire_marker(sv4_t* marker) {
    llg_wait_source_t* source = wait_source_find(marker);
    if (source && source->change && source->change->kind == LLG_CHANGE_MARKER)
        source->change->fire = 1;
}

static void fire_bound_markers(const void* target, uint8_t kind, int toggle_rest) {
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key != target || binding->kind != kind) continue;
        llg_wait_source_t* source = wait_source_find(binding->dependency);
        if (source && source->change && source->change->kind == LLG_CHANGE_MARKER)
            source->change->fire = 1;
        else if (toggle_rest)
            marker_toggle(binding->dependency, source == NULL);
    }
}

static llg_wait_source_t* change_begin_real(double* target, double before) {
    llg_wait_source_t* source = wait_source_find(target);
    if (source && source->change) return source;
    if (wait_source_armed(source) || g.deferred_triggers ||
        dependency_bindings_waited(target, LLG_DEPENDENCY_BINDING_REAL)) {
        if (!source) source = wait_source_get(target);
        change_open(source, LLG_CHANGE_REAL)->before_real = before;
    }
    return source;
}

static void change_forget_history(llg_clocking_edge_t* edge) {
    g.changes[edge->change->index] = NULL;
    change_close(edge->change);
}

// Storage that ends before its check has nothing left to publish. Value
// scopes forget every value they release, mostly temporaries, while a turn
// has records open: a few records are scanned instead of a row lookup.
static void change_forget(const void* key) {
    llg_change_t* change = NULL;
    if (g.change_count <= 8) {
        for (size_t i = 0; i < g.change_count; i++) {
            llg_change_t* open = g.changes[i];
            if (open && open->key == key) {
                change = open;
                break;
            }
        }
    } else {
        llg_wait_source_t* source = wait_source_find(key);
        if (source) change = source->change;
        if (!change) change = change_find_unrowed(key);
    }
    if (!change) return;
    g.changes[change->index] = NULL;
    change_close(change);
}

static void changes_release(void) {
    for (size_t i = 0; i < g.change_count; i++)
        if (g.changes[i]) change_close(g.changes[i]);
    g.change_count = 0;
    free(g.changes);
    g.changes = NULL;
    g.change_capacity = 0;
    while (g.change_free) {
        llg_change_t* next = (llg_change_t*)g.change_free->cursor.next;
        sv4_destroy(&g.change_free->before);
        free(g.change_free);
        g.change_free = next;
    }
}

// Ordered waits see each write in the order of the writes (S3-D5), bit by bit
// on one vector, so they log and wake here and find nothing new at the check.
// Gate-at-write waits latch their qualifiers here and wake at the check.
// Nothing here unlinks a membership: a woken ordered wait keeps its retained
// memberships in place (they point at its log), and latching only evaluates
// effect-free callbacks. So the walk needs no cursor.
static void wait_write_walk(llg_wait_source_t* source, sv4_t* sig, double* real) {
    for (llg_wait_subscription_t *node = source->head, *next; node; node = next) {
        next = node->next;
        llg_wait_t* w = node->wait;
        if (!w) continue;
        if (w->kind == W_EXPR && (w->proc->wait_gate & LLG_WAIT_GATE_ARMED)) {
            for (int i = 0; i < w->payload.expression.n; i++)
                expression_gate_latch(w, i, sig, real);
            continue;
        }
        llg_wait_change_log_t* log = NULL;
        if (w->kind == W_CHANGE_LOG) log = (llg_wait_change_log_t*)w;
        else if (w->kind == W_DEPS && w->proc->wait_ordered)
            log = w->proc->retained_wait->changes;
        if (!log) continue;
        int changed = 1;
        if (sig) changed = wait_change_log_packed(log, sig);
        else wait_change_note(log, real, LLG_WAIT_CHANGE_WHOLE);
        if (changed && w->kind == W_DEPS && !wait_held_by_suspension(w)) wake_proc(w->proc);
    }
}

LLG_CO_INLINE void publish_waiters_packed(llg_wait_subscription_t* cursor, sv4_t* target) {
    llg_wait_t* w;
    while ((w = wait_source_next(cursor)) != NULL) {
        int wake = 0;
        if (w->kind == W_EVENTS_INLINE) {
            llg_wait_inline_payload_t* single = &w->payload.single;
            if (single->specs[0].sig == target) {
                wake = ev_inline_matches(w, *target, single->specs[0].kind);
                if (wake && wait_held_by_suspension(w)) wake = 0;
                if (!wake) {
                    if (wait_inline_fits(target)) wait_inline_copy(w, target);
                    else wait_inline_promote(w, target);
                }
            }
        } else if (w->kind == W_LEVEL_INLINE) {
            if (w->payload.single.specs[0].sig == target &&
                wait_inline_same(w, *target)) wake = 1;
        } else if (w->kind == W_EVENTS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            for (int i = 0; i < payload->n; i++) {
                if (payload->specs[i].sig == target) {
                    if (ev_matches(payload->last[i], *target,
                                   payload->specs[i].kind))
                        wake = 1;
                    sv4_copy(&payload->last[i], target);
                }
            }
        } else if (w->kind == W_MIXED && w->payload.rare) {
            llg_wait_mixed_payload_t* payload = &w->payload.rare->mixed;
            for (int i = 0; i < payload->n; i++) {
                if (payload->specs[i].sig == target) {
                    if (ev_matches(payload->last[i], *target,
                                   payload->specs[i].kind))
                        wake = 1;
                    sv4_copy(&payload->last[i], target);
                }
            }
        } else if (w->kind == W_DEPS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            // An ordered wait shares its lists with its retained record.
            if (LLG_CO_UNLIKELY(w->proc->wait_ordered))
                wake = wait_change_log_packed(w->proc->retained_wait->changes, target);
            else for (int i = 0; i < payload->n; i++) {
                const llg_wait_dependency_t* dependency = &payload->dependencies[i];
                if (dependency->sig == target) {
                    if (dependency->width) {
                        sv4_t selected = sv4_part_select(dependency->value ? *dependency->value : *target,
                            (int64_t)dependency->lsb + dependency->width - 1, dependency->lsb);
                        if (!sv4_same(payload->last[i], selected)) wake = 1;
                        sv4_move(&payload->last[i], &selected);
                    } else wake = 1;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->payload.expression.n; i++) {
                if (expression_update(w, i, target, NULL)) wake = 1;
            }
        } else if (w->kind == W_LEVEL) {
            llg_wait_level_payload_t* level = &w->payload.rare->level;
            if (level->sig == target && sv4_same(*target, level->value)) wake = 1;
        } else if (w->kind == W_CHANGE_LOG) {
            (void)wait_change_log_packed((llg_wait_change_log_t*)w, target);
        }
        if (wake && !wait_held_by_suspension(w)) wake_proc(w->proc);
    }
}

static void publish_bound_markers(const void* target, uint8_t kind) {
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key == target && binding->kind == kind)
            llg_dependency_changed(binding->dependency);
    }
}

// Clocking drives, sampled-value clocks and assertion clocks. Each is called
// only when it has entries: `sv4_t` arguments passed by value cost a copy
// per call on the hottest path.
LLG_CO_INLINE void clock_consumers_changed(sv4_t* target, sv4_t old, sv4_t value,
                                          int deferred) {
    if (g.clocking_drives && clocking_drive_source_pending(target))
        clocking_drive_signal_match(target, old, value);
    if (g.sampled_edge_clocks) sampled_domain_clock_signal_changed(target, old, value, deferred);
    // Queue the clock event after asynchronous controls have seen the new
    // value. This keeps a clock that also changes an accept/reject condition
    // from being discarded before its sampled control can resolve it.
    if (g.assertions) assertion_clock_signal_changed(target, old, value);
}

// How a publication reaches the markers bound to its storage.
enum {
    LLG_BOUND_PUBLISH,   // toggle each now (a write outside a turn)
    LLG_BOUND_FIRE,      // a storage's check: markers without a record
                         // toggled at the write; those with one toggle at
                         // their own check
    LLG_BOUND_SETTLE,    // a marker's check: as FIRE, but the markers
                         // without a record toggle now
    LLG_BOUND_NONE,      // a check of storage no marker is bound to
};

// Everything that depends on the change itself. `cursor`, when given, is an
// open record's and excludes waiters armed since the write. `old` and `value`
// need only their LSB unless the record compared them.
// `contents` is the contents marker of the array holding the storage, or NULL.
LLG_CO_INLINE void sig_settle(sv4_t* target, llg_wait_source_t* source,
                              llg_wait_subscription_t* cursor, sv4_t old, sv4_t value,
                              int bound, sv4_t* contents) {
    clocking_record_edge(target, old, value);
    clock_consumers_changed(target, old, value, cursor != NULL);
    if (cursor) {
        publish_waiters_packed(cursor, target);
    } else if (source) {
        llg_wait_subscription_t own = {0};
        own.source = source;
        wait_subscription_insert(&own, &source->head);
        publish_waiters_packed(&own, target);
        wait_subscription_unlink(&own);
    }
    deferred_trigger_source_change(target, NULL);
    if (bound == LLG_BOUND_PUBLISH) {
        publish_bound_markers(target, LLG_DEPENDENCY_BINDING_PACKED);
        if (contents) llg_dependency_changed(contents);
    } else {
        if (bound != LLG_BOUND_NONE)
            fire_bound_markers(target, LLG_DEPENDENCY_BINDING_PACKED,
                               bound == LLG_BOUND_SETTLE);
        if (contents) fire_marker(contents);
    }
}

// The check of a history record: nothing waited on the storage or its bound
// markers at the write, and its writes already ran every per-write consumer.
static void history_settle(llg_clocking_edge_t* edge, sv4_t* target, sv4_t old,
                           sv4_t value) {
    clocking_edge_note(edge, old, value);
    clock_consumers_changed(target, old, value, 1);
}

// Routes the markers bound to a deferred write's storage, and the contents
// marker of its array, when its row holds no record: a marker something waits
// on gets a record and so does the storage, with `before`; the others toggle
// at once. Returns whether the storage now has a record.
// `row` is the storage's row, or NULL when it has none. A storage without a
// row has no waiter to walk at its check, so its record is rowless: it is
// found again through its clocking history entry, when it has one, or
// through a marker record this write opened (`owner`). A storage whose
// waited markers all have records opened by other storage takes a row.
typedef struct {
    llg_change_t* fires[2];
    llg_change_t* opened[2];
    llg_change_t* found; // This storage's rowless record, from an earlier write.
    int fired;
    int n_opened;
} llg_route_t;

static void change_route_note(llg_route_t* route, const sv4_t* target,
                              llg_change_t* marker, int opened) {
    if (opened) {
        if (route->n_opened < 2) route->opened[route->n_opened++] = marker;
    } else if (!route->found) {
        llg_change_t* owner = change_marker_owner(marker);
        if (owner && owner->key == target) route->found = owner;
    }
    if (route->fired < 2) route->fires[route->fired] = marker;
    ++route->fired;
}

static int change_route_markers(sv4_t* target, const sv4_t* before, sv4_t* contents,
                                llg_wait_source_t* row) {
    llg_route_t route = {{NULL, NULL}, {NULL, NULL}, NULL, 0, 0};
    int bound = 0;
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->key != target || binding->kind != LLG_DEPENDENCY_BINDING_PACKED)
            continue;
        int rowless, opened;
        llg_change_t* marker = change_marker_defer(binding->dependency, &rowless, &opened);
        if (marker) {
            bound = 1;
            change_route_note(&route, target, marker, opened);
        } else {
            marker_toggle(binding->dependency, rowless);
        }
    }
    if (contents) {
        int rowless, opened;
        llg_change_t* marker = change_marker_defer(contents, &rowless, &opened);
        if (marker) change_route_note(&route, target, marker, opened);
        else marker_toggle(contents, rowless);
    }
    if (!route.fired) return 0;
    llg_change_t* storage = route.found;
    llg_clocking_edge_t* edge = !storage && g.change_count > g.change_rows
                                    ? find_clocking_edge(target) : NULL;
    if (edge && edge->change && edge->change->contents_known) storage = edge->change;
    if (storage) {
        // An earlier write of this turn routed the storage. A marker record
        // opened since is not among its direct fires: the check looks the
        // markers up instead.
        if (route.n_opened) storage->fires_exact = 0;
        return 1;
    }
    if (edge && edge->change) {
        // The history record change_begin opened.
        storage = edge->change;
    } else if (!row && route.n_opened) {
        storage = change_take(LLG_CHANGE_ROWLESS);
        storage->key = target;
        ++g.change_unrowed;
        change_save_before(storage, before);
    } else {
        storage = change_open_packed(row, target, before)->change;
    }
    for (int i = 0; i < route.n_opened; i++) {
        route.opened[i]->owner = storage;
        route.opened[i]->owner_serial = storage->serial;
    }
    if (bound) {
        storage->bound = 1;
        storage->bind_epoch = dependency_bind_epoch;
    }
    storage->contents = contents;
    storage->contents_known = 1;
    if (route.fired <= 2) {
        // The check fires these records directly instead of looking the
        // markers up again.
        storage->fires_exact = 1;
        for (int i = 0; i < 2; i++) {
            storage->fires[i] = route.fires[i];
            storage->fires_serial[i] = route.fires[i] ? route.fires[i]->serial : 0;
        }
    }
    return 1;
}

// `source` is the row after change_begin for a deferred write, else unused.
// `before`, when given, is the complete value before the write: with it a
// record can still open here for waited markers (change_route_markers).
// `cursor`, for a marker's check (marker_settle), is its record's: the
// publication then runs at once from that cursor.
static void sig_publish(sv4_t* target, sv4_t old, sv4_t value,
                        llg_wait_source_t* source, int deferred,
                        const sv4_t* before, llg_wait_subscription_t* cursor) {
    sampled_record_write(target);
    if (deferred && g.sampled_edge_clocks) sampled_clock_gates_latch(target, old, value);
    // `disable iff` is an asynchronous, unsampled control. Abort pending
    // attempts at the write boundary, before any waiter or later region can
    // observe the changed value.
    assertion_disable_signal_changed(target);
    // Ordinary accept_on/reject_on controls are also asynchronous. Their
    // predicate is evaluated only after the write is visible, while the
    // synchronous variants are checked at the sampled assertion edge below.
    assertion_abort_condition_changed();
    if (g.monitors) llg_monitor_target_changed(target, LLG_FMT_PACKED);
#ifdef LLG_WAVEFORM
    llg_wave_changed_sv4(target);
#endif
    struct llg_fixed_block* block = fixed_array_block(target);
    sv4_t* contents = fixed_block_contents(block);
    // Consumers above can drop a row no record keeps alive.
    if (cursor) source = cursor->source;
    else if (source == &wait_source_absent) source = NULL;
    else if (!deferred || (source && !source->change)) source = wait_source_find(target);
    int watched = source && source->watchers;
    llg_timing_watch_t* timing = source ? source->timing : NULL;
    llg_switch_watch_t* switches = source ? source->switches : NULL;
    if (cursor) {
        sig_settle(target, source, cursor, old, value,
                   change_bound(source->change, target) ? LLG_BOUND_SETTLE : LLG_BOUND_NONE,
                   contents);
    } else if (!deferred) {
        sig_settle(target, source, NULL, old, value, LLG_BOUND_PUBLISH, contents);
    } else {
        int noted = source && source->change;
        if (LLG_CO_UNLIKELY(source && source->write_walkers)) wait_write_walk(source, target, NULL);
        if (noted) {
            source->change->contents = contents;
            source->change->contents_known = 1;
            if (change_bound(source->change, target))
                defer_bound_markers(target, LLG_DEPENDENCY_BINDING_PACKED);
            int rowless;
            if (contents && !change_marker_defer(contents, &rowless, NULL))
                marker_toggle(contents, rowless);
        } else if (before) {
            (void)change_route_markers(target, before, contents, source);
        } else {
            publish_bound_markers(target, LLG_DEPENDENCY_BINDING_PACKED);
            if (contents) llg_dependency_changed(contents);
        }
    }
    fixed_block_sampled_changed(block, target, old);
    force_dependency_changed(target, NULL, 0);
    // Timing check terminals only mark their sets; the checks run later in
    // the time step (llg_timing_checks_run).
    if (LLG_CO_UNLIKELY(timing != NULL)) timing_terminals_changed(timing);
    // Direct switch controls set their switches' states; the networks are
    // solved like any other switch change (scheduler/switches.c).
    if (LLG_CO_UNLIKELY(switches != NULL)) switch_controls_changed(switches);
    // Watches run last, after every runtime consumer saw the new value.
    if (LLG_CO_UNLIKELY(watched)) value_watch_notify(target);
}

// `old` must be complete: the first write's record keeps it.
static void sig_publish_changed(sv4_t* target, sv4_t old, sv4_t value) {
    int deferred = change_defer_active;
    llg_wait_source_t* source = deferred ? change_begin(target, &old, 0) : NULL;
    sig_publish(target, old, value, source, deferred, &old, NULL);
}

// Set while a clocking block publishes its own sampled clockvars. #0 inputs
// update in the Observed region (IEEE 1800-2009 14.13), whose user code is
// otherwise read-only, and `@(cb.x)` waiters must still see the change.
static int clocking_sample_publication;

static void sig_write_as(sv4_t* target, sv4_t value, int deferred) {
    if (region_is_read_only_now(g.current_region) && !clocking_sample_publication) {
        if (region_private_store("signal write")) sv4_copy(target, &value);
        return;
    }
    if (llg_sv4_width(*target) == llg_sv4_width(value) && sv4_same(*target, value)) return;
#if LLG_SV4_USE_GMP
    // On the compact backend a value of at most 64 bits lives in its
    // descriptor: a copy owns no allocation, so a callback that abandons the
    // writer leaves nothing behind and no registered snapshot is needed.
    if (llg_sv4_width(*target) <= 64 && llg_sv4_width(value) <= 64) {
        llg_value_scope_t* target_pin = value_target_pin(target);
        sv4_t old = SV4_EMPTY;
        sv4_copy(&old, target);
        sv4_copy(target, &value);
        llg_wait_source_t* source = deferred ? change_begin(target, &old, 0) : NULL;
        sig_publish(target, old, value, source, deferred, &old, NULL);
        if (target_pin) llg_value_scope_end(target_pin);
        return;
    }
#endif
    // Callbacks can finish/disable the writer without returning through here.
    // Heap-backed registered owners survive both suspension and stack discard.
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = value_scope_begin_private(2);
    sv4_t* owned = llg_value_scope_values(snapshots);
    sv4_copy(&owned[0], &value);
    sv4_copy(&owned[1], target);
    value = owned[0]; /* Borrows the registered snapshot until scope end. */
    sv4_t old = owned[1];
    sv4_copy(target, &value);
    llg_wait_source_t* source = deferred ? change_begin(target, &old, 0) : NULL;
    sig_publish(target, old, value, source, deferred, &old, NULL);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
}

static void sig_write(sv4_t* target, sv4_t value) {
    sig_write_as(target, value, change_defer_active);
}

static void sig_write_now(sv4_t* target, sv4_t value) {
    sig_write_as(target, value, 0);
}

// The two values of a one-bit marker: permanent and never changed, so a
// callback that abandons the writer leaves nothing to release.
static sv4_t marker_values[2];

// A marker flips on every change of what it stands for, and sig_write's owned
// snapshots would cost more than the rest of the publication. A marker holding
// a known bit flips in place; anything else takes the general write.
// `rowless` tells that the caller has just found no row for the marker:
// nothing the publication runs before its walk arms a wait, so it skips the
// lookup.
static int marker_flip(sv4_t* marker, int rowless) {
    if (llg_sv4_width(*marker) != 1 || region_is_read_only_now(g.current_region))
        return 0;
    unsigned old = llg_sv4_state(*marker, 0);
    if (old > 1u) return 0;
    if (!llg_sv4_width(marker_values[0])) {
        marker_values[0] = sv4_from_u64(0u, 1, 0);
        marker_values[1] = sv4_from_u64(1u, 1, 0);
    }
    llg_value_scope_t* target_pin = value_target_pin(marker);
    llg_sv4_set_state(marker, 0, old ^ 1u);
    sig_publish(marker, marker_values[old], marker_values[old ^ 1u],
                rowless ? &wait_source_absent : NULL, 0, NULL, NULL);
    if (target_pin) llg_value_scope_end(target_pin);
    return 1;
}

static int sig_range_same(const sv4_t* target, uint32_t offset,
                          const sv4_t* value) {
    return llg_sv4_range_same(*target, offset, *value);
}

static void sig_range_copy(sv4_t* target, uint32_t offset,
                           const sv4_t* value) {
    llg_sv4_range_copy(target, offset, *value);
}

// Publish a changed slice without cloning the full packed signal. Waiters
// still subscribe to the canonical full-net address: full-value and selected
// dependency checks below read the already-updated target exactly as before.
static int sig_write_ranges(sv4_t* target,
                            uint32_t first_offset, sv4_t first,
                            uint32_t second_offset, sv4_t second,
                            int has_second) {
    int private_store = 0;
    if (region_is_read_only_now(g.current_region)) {
        if (!region_private_store("signal write")) return 0;
        private_store = 1;
    }
    if (!llg_sv4_width(first) || first_offset > llg_sv4_width(*target) ||
        llg_sv4_width(first) > llg_sv4_width(*target) - first_offset ||
        (has_second &&
         (!llg_sv4_width(second) || second_offset > llg_sv4_width(*target) ||
          llg_sv4_width(second) > llg_sv4_width(*target) - second_offset))) {
        fputs("llg: fatal: invalid signal write range\n", stderr);
        abort();
    }
    int first_changed = !sig_range_same(target, first_offset, &first);
    int second_changed = has_second &&
        !sig_range_same(target, second_offset, &second);
    if (!first_changed && !second_changed) return 0;
    if (private_store) {
        if (first_changed) sig_range_copy(target, first_offset, &first);
        if (second_changed) sig_range_copy(target, second_offset, &second);
        return 0;
    }
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = value_scope_begin_private(2);
    sv4_t* edges = llg_value_scope_values(snapshots);
    // Every directional packed edge rule consumes only the LSB. Publication
    // has already established that some bit changed, so these stable one-bit
    // snapshots also preserve any-change behavior through nested callbacks.
    // A check compares whole values, so a record is opened before the store.
    int deferred = change_defer_active;
    llg_wait_source_t* source = deferred ? change_begin(target, target, 1) : NULL;
    sv4_replace(&edges[0], sv4_bit_select(*target, 0));
    if (first_changed) sig_range_copy(target, first_offset, &first);
    if (second_changed) sig_range_copy(target, second_offset, &second);
    sv4_replace(&edges[1], sv4_bit_select(*target, 0));
    sig_publish(target, edges[0], edges[1], source, deferred, NULL, NULL);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
    return 1;
}

static int sig_write_range(sv4_t* target, uint32_t offset, sv4_t value) {
    return sig_write_ranges(target, offset, value, 0, (sv4_t)SV4_EMPTY, 0);
}

static void publish_waiters_real(llg_wait_subscription_t* cursor, double* target) {
    llg_wait_t* w;
    while ((w = wait_source_next(cursor)) != NULL) {
        int wake = 0;
        if (w->kind == W_DEPS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            for (int i = 0; i < payload->n; i++) {
                if (payload->dependencies[i].real == target) {
                    wake = 1;
                    break;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->payload.expression.n; i++) {
                if (expression_update(w, i, NULL, target)) wake = 1;
            }
        } else if (w->kind == W_CHANGE_LOG) {
            wait_change_note((llg_wait_change_log_t*)w, target,
                             LLG_WAIT_CHANGE_WHOLE);
        }
        if (wake && !wait_held_by_suspension(w)) {
            if (LLG_CO_UNLIKELY(w->proc->wait_ordered))
                wait_change_note(w->proc->retained_wait->changes, target,
                                 LLG_WAIT_CHANGE_WHOLE);
            wake_proc(w->proc);
        }
    }
}

static void real_settle(double* target, llg_wait_source_t* source,
                        llg_wait_subscription_t* cursor) {
    if (cursor) {
        publish_waiters_real(cursor, target);
    } else if (source) {
        llg_wait_subscription_t own = {0};
        own.source = source;
        wait_subscription_insert(&own, &source->head);
        publish_waiters_real(&own, target);
        wait_subscription_unlink(&own);
    }
    deferred_trigger_source_change(NULL, target);
    if (cursor) fire_bound_markers(target, LLG_DEPENDENCY_BINDING_REAL, 0);
    else publish_bound_markers(target, LLG_DEPENDENCY_BINDING_REAL);
}

// Real equality is bitwise: repeated NaNs with the same payload are
// suppressed, while changes in NaN payload and signed zero are observable.
static void real_write(double* target, double value) {
    if (region_is_read_only_now(g.current_region) && !clocking_sample_publication) {
        if (region_private_store("real write")) *target = value;
        return;
    }
    double old = *target;
    if (real_same(old, value)) return;
    /* Real locals have stable native owner slots, just like packed descriptors.
     * Keep the slot alive if a callback cancels its receiving process. */
    llg_value_scope_t* target_pin = value_target_pin(target);
    int deferred = change_defer_active;
    llg_wait_source_t* source = deferred ? change_begin_real(target, old) : NULL;
    *target = value;
    if (g.monitors) llg_monitor_target_changed(target, LLG_FMT_REAL);
#ifdef LLG_WAVEFORM
    llg_wave_changed_real(target);
#endif
    if (!deferred || (source && !source->change)) source = wait_source_find(target);
    int watched = source && source->watchers;
    if (!deferred) {
        real_settle(target, source, NULL);
    } else {
        int noted = source && source->change;
        if (LLG_CO_UNLIKELY(source && source->write_walkers)) wait_write_walk(source, NULL, target);
        if (noted) defer_bound_markers(target, LLG_DEPENDENCY_BINDING_REAL);
        else publish_bound_markers(target, LLG_DEPENDENCY_BINDING_REAL);
    }
    if (LLG_CO_UNLIKELY(llg_native_real_refs_live)) llg_native_ref_real_stored(target);
    force_dependency_changed(NULL, target, 1);
    if (LLG_CO_UNLIKELY(watched)) value_watch_notify(target);
    if (target_pin) llg_value_scope_end(target_pin);
}

// A deferred marker toggles once, publishing from its record's cursor.
static void marker_settle(sv4_t* marker, llg_wait_subscription_t* cursor) {
    unsigned old = llg_sv4_state(*marker, 0);
    if (llg_sv4_width(*marker) != 1 || old > 1u) {
        // A marker not yet holding a known bit takes the general write.
        llg_dependency_changed(marker);
        return;
    }
    if (!llg_sv4_width(marker_values[0])) {
        marker_values[0] = sv4_from_u64(0u, 1, 0);
        marker_values[1] = sv4_from_u64(1u, 1, 0);
    }
    llg_sv4_set_state(marker, 0, old ^ 1u);
    sig_publish(marker, marker_values[old], marker_values[old ^ 1u], NULL, 0, NULL, cursor);
}

// The check of a rowless record that routed markers: as sig_settle, with no
// waiter to walk.
static void rowless_settle(llg_change_t* change, sv4_t* target) {
    sv4_t old = change->before;
    if (change->edge) clocking_edge_note(change->edge, old, *target);
    else clocking_record_edge(target, old, *target);
    clock_consumers_changed(target, old, *target, 1);
    deferred_trigger_source_change(target, NULL);
    if (change->fires_exact) {
        for (int i = 0; i < 2; i++) {
            llg_change_t* marker = change->fires[i];
            if (marker && marker->serial == change->fires_serial[i]) marker->fire = 1;
        }
    } else {
        if (change_bound(change, target))
            fire_bound_markers(target, LLG_DEPENDENCY_BINDING_PACKED, 0);
        if (change->contents) fire_marker(change->contents);
    }
}

static void change_settle(llg_change_t* change) {
    llg_wait_subscription_t* cursor = &change->cursor;
    if (change->kind == LLG_CHANGE_ROWLESS) {
        sv4_t* target = (sv4_t*)change->key;
        if (llg_sv4_width(change->before) == llg_sv4_width(*target) &&
            sv4_same(change->before, *target))
            return;
        // A history record (no markers routed) always has its entry.
        if (!change->contents_known) history_settle(change->edge, target, change->before, *target);
        else rowless_settle(change, target);
        return;
    }
    const void* key = change->key;
    if (change->kind == LLG_CHANGE_PACKED) {
        sv4_t* target = (sv4_t*)key;
        if (llg_sv4_width(change->before) != llg_sv4_width(*target) ||
            !sv4_same(change->before, *target)) {
            if (change->fires_exact) {
                for (int i = 0; i < 2; i++) {
                    llg_change_t* marker = change->fires[i];
                    if (marker && marker->serial == change->fires_serial[i]) marker->fire = 1;
                }
                sig_settle(target, cursor->source, cursor, change->before, *target,
                           LLG_BOUND_NONE, NULL);
            } else {
                sig_settle(target, cursor->source, cursor, change->before, *target,
                           change_bound(change, target) ? LLG_BOUND_FIRE : LLG_BOUND_NONE,
                           change->contents_known ? change->contents
                                                  : fixed_array_contents_marker(target));
            }
        }
    } else if (change->kind == LLG_CHANGE_REAL) {
        double* target = (double*)key;
        if (!real_same(change->before_real, *target))
            real_settle(target, cursor->source, cursor);
    } else if (change->fire || (change->slot && *change->slot != change->before_handle)) {
        marker_settle((sv4_t*)key, cursor);
    }
}

// Publishes in first-write order, with deferral off: what a walk evaluates or
// triggers publishes at once and so cannot add a record.
// Storage records settle first and marker records after them, so that every
// storage a marker stands for has fired it, whatever the opening order (an
// array's contents marker is shared by its cells). A marker firing a further
// marker whose record already settled toggles it at once (LLG_BOUND_SETTLE).
static void change_flush(void) {
    int deferred = change_defer_active;
    int settling = change_settling;
    change_defer_active = 0;
    change_settling = 1;
    int markers_left = 0;
    for (size_t i = 0; i < g.change_count; i++) {
        llg_change_t* change = g.changes[i];
        if (!change) continue;
        if (change->kind == LLG_CHANGE_MARKER) {
            markers_left = 1;
            continue;
        }
        g.changes[i] = NULL;
        change_settle(change);
        change_close(change);
    }
    for (size_t i = 0; markers_left && i < g.change_count; i++) {
        llg_change_t* change = g.changes[i];
        if (!change) continue;
        g.changes[i] = NULL;
        change_settle(change);
        change_close(change);
    }
    g.change_count = 0;
    change_settling = settling;
    change_defer_active = deferred;
}

// ── Procedural force / release ───────────────────────────────────────────────
