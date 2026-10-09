
// ── Signal writes and waiter scanning ─────────────────────────────────────────




static void force_dependency_changed(sv4_t* sig, double* real, int is_real);
static void sig_write(sv4_t* target, sv4_t value);
static void fixed_array_changed(sv4_t* target);

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
    return binding->target ? (const void*)binding->target
                           : (const void*)binding->real_target;
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

static void dependency_bind(sv4_t* target, double* real_target, sv4_t* dependency,
                            const char* what) {
    const void* key = target ? (const void*)target : (const void*)real_target;
    for (llg_dependency_binding_t* binding = dependency_bucket_head(key);
         binding; binding = binding->bucket_next) {
        if (binding->target == target && binding->real_target == real_target &&
            binding->dependency == dependency) return;
    }
    dependency_index_reserve();
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), what);
    binding->target = target;
    binding->real_target = real_target;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
    size_t bucket = dependency_bucket(key);
    binding->bucket_next = llg_dependency_buckets[bucket];
    llg_dependency_buckets[bucket] = binding;
    ++llg_dependency_binding_count;
}

void llg_dependency_bind(sv4_t* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid dependency binding\n");
        abort();
    }
    dependency_bind(target, NULL, dependency, "dependency binding");
}

void llg_dependency_bind_real(double* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid real dependency binding\n");
        abort();
    }
    dependency_bind(NULL, target, dependency, "real dependency binding");
}

sv4_t* llg_dependency_or_never(sv4_t* dependency) {
    static sv4_t never = SV4_EMPTY;
    return dependency ? dependency : &never;
}

void llg_dependency_changed(sv4_t* dependency) {
    if (!dependency) return;
    uint64_t bit = llg_sv4_word(*dependency, 0, LLG_SV4_BITS) & 1u;
    /* Native strings and containers publish through this marker. A subscriber
     * may terminate the writer without returning through this function. */
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    value[0] = sv4_from_u64(bit ^ 1u, 1, 0);
    sig_write(dependency, value[0]);
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
    if (!g.clocking_count || !signal) return;
    size_t slot = clocking_edge_slot(signal);
    llg_clocking_edge_t* edge = g.clocking_index[slot];
    if (!edge) return;
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

static void clocking_record_edge(sv4_t* signal, sv4_t old, sv4_t value) {
    if (!signal) return;
    llg_clocking_edge_t* edge = clocking_edge_get(signal);
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
            llg_clocking_edge_t* edge = find_clocking_edge(srcs[i].sig);
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
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_signal(drive, signal, old, value)) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        free_clocking_drive(drive);
    }
    g.clocking_drives_tail = g.clocking_drives;
    while (g.clocking_drives_tail && g.clocking_drives_tail->next)
        g.clocking_drives_tail = g.clocking_drives_tail->next;
}

static void clocking_drive_event_match(llg_event_object_t* event) {
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_event(drive, event)) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        free_clocking_drive(drive);
    }
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
    int matched = ev_matches(payload->last[index], *value, spec->kind);
    sv4_move(&payload->last[index], value);
    llg_value_scope_end(scope);
    return matched && expression_qualifies(spec);
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

static void sig_publish_changed(sv4_t* target, sv4_t old, sv4_t value,
                                sv4_t published) {
#ifndef LLG_WAVEFORM
    (void)published;
#endif
    clocking_record_edge(target, old, value);
    clocking_drive_signal_match(target, old, value);
    sampled_record_write(target);
    sampled_domain_clock_signal_changed(target, old, value);
    // `disable iff` is an asynchronous, unsampled control. Abort pending
    // attempts at the write boundary, before any waiter or later region can
    // observe the changed value.
    assertion_disable_signal_changed(target);
    // Ordinary accept_on/reject_on controls are also asynchronous. Their
    // predicate is evaluated only after the write is visible, while the
    // synchronous variants are checked at the sampled assertion edge below.
    assertion_abort_condition_changed();
    // Queue the clock event after asynchronous controls have seen the new
    // value. This keeps a clock that also changes an accept/reject condition
    // from being discarded before its sampled control can resolve it.
    assertion_clock_signal_changed(target, old, value);
    if (g.mon.active) {
        for (int i = 0; i < g.mon.n_reads; i++) {
            if (g.mon.reads[i] == target) {
                g.mon.dirty = 1;
                break;
            }
        }
        for (int i = 0; i < g.mon.n_typed_reads; i++) {
            if (g.mon.typed_reads[i].kind == LLG_FMT_PACKED &&
                g.mon.typed_reads[i].ptr == target) {
                g.mon.dirty = 1;
                break;
            }
        }
    }
#ifdef LLG_WAVEFORM
    llg_wave_changed_sv4(target, &published, g.now);
#endif
    llg_wait_source_t* source = wait_source_find(target);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
    }
    llg_wait_t* w;
    while (source && (w = wait_source_next(&cursor)) != NULL) {
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
            for (int i = 0; i < payload->n; i++) {
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
        }
        if (wake && !wait_held_by_suspension(w)) wake_proc(w->proc);
    }
    if (source) wait_subscription_unlink(&cursor);
    deferred_trigger_source_change(target, NULL);
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->target == target) llg_dependency_changed(binding->dependency);
    }
    fixed_array_changed(target);
    force_dependency_changed(target, NULL, 0);
}

// Set while a clocking block publishes its own sampled clockvars. #0 inputs
// update in the Observed region (IEEE 1800-2009 14.13), whose user code is
// otherwise read-only, and `@(cb.x)` waiters must still see the change.
static int clocking_sample_publication;

static void sig_write(sv4_t* target, sv4_t value) {
    if (region_is_read_only_now(g.current_region) && !clocking_sample_publication) {
        if (region_private_store("signal write")) sv4_copy(target, &value);
        return;
    }
    if (llg_sv4_width(*target) == llg_sv4_width(value) && sv4_same(*target, value)) return;
    // Callbacks can finish/disable the writer without returning through here.
    // Heap-backed registered owners survive both suspension and stack discard.
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = llg_value_scope_begin(2);
    sv4_t* owned = llg_value_scope_values(snapshots);
    sv4_copy(&owned[0], &value);
    sv4_copy(&owned[1], target);
    value = owned[0]; /* Borrows the registered snapshot until scope end. */
    sv4_t old = owned[1];
    sv4_copy(target, &value);
    sig_publish_changed(target, old, value, value);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
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
    llg_value_scope_t* snapshots = llg_value_scope_begin(
#ifdef LLG_WAVEFORM
        3
#else
        2
#endif
    );
    sv4_t* edges = llg_value_scope_values(snapshots);
    // Every directional packed edge rule consumes only the LSB. Publication
    // has already established that some bit changed, so these stable one-bit
    // snapshots also preserve any-change behavior through nested callbacks.
    sv4_replace(&edges[0], sv4_bit_select(*target, 0));
    if (first_changed) sig_range_copy(target, first_offset, &first);
    if (second_changed) sig_range_copy(target, second_offset, &second);
    sv4_replace(&edges[1], sv4_bit_select(*target, 0));
#ifdef LLG_WAVEFORM
    sv4_copy(&edges[2], target);
    sv4_t published = edges[2];
#else
    sv4_t published = SV4_EMPTY;
#endif
    sig_publish_changed(target, edges[0], edges[1], published);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
    return 1;
}

static int sig_write_range(sv4_t* target, uint32_t offset, sv4_t value) {
    return sig_write_ranges(target, offset, value, 0, (sv4_t)SV4_EMPTY, 0);
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
    *target = value;
    if (g.mon.active) {
        for (int i = 0; i < g.mon.n_typed_reads; i++) {
            if (g.mon.typed_reads[i].kind == LLG_FMT_REAL &&
                g.mon.typed_reads[i].ptr == target) {
                g.mon.dirty = 1;
                break;
            }
        }
    }
#ifdef LLG_WAVEFORM
    llg_wave_changed_real(target, value, g.now);
#endif
    llg_wait_source_t* source = wait_source_find(target);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
    }
    llg_wait_t* w;
    while (source && (w = wait_source_next(&cursor)) != NULL) {
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
        }
        if (wake && !wait_held_by_suspension(w)) wake_proc(w->proc);
    }
    if (source) wait_subscription_unlink(&cursor);
    deferred_trigger_source_change(NULL, target);
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->real_target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(NULL, target, 1);
    if (target_pin) llg_value_scope_end(target_pin);
}

// ── Procedural force / release ───────────────────────────────────────────────
