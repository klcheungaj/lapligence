
// ── Signal writes and waiter scanning ─────────────────────────────────────────




static void force_dependency_changed(sv4_t* sig, double* real, int is_real);
static void sig_write(sv4_t* target, sv4_t value);
static int pca_real_active(double* target);

void llg_dependency_bind(sv4_t* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid dependency binding\n");
        abort();
    }
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->target == target && binding->real_target == NULL && binding->dependency == dependency) return;
    }
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), "dependency binding");
    binding->target = target;
    binding->real_target = NULL;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
}

void llg_dependency_bind_real(double* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid real dependency binding\n");
        abort();
    }
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->real_target == target && binding->dependency == dependency) return;
    }
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), "real dependency binding");
    binding->target = NULL;
    binding->real_target = target;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
}

void llg_dependency_changed(sv4_t* dependency) {
    if (!dependency) return;
    uint64_t bit = dependency->width ? dependency->bits[0] & 1u : 0;
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
    int a = !old.width || ((old.x[0] | old.z[0]) & 1u)
        ? 2 : (int)(old.bits[0] & 1u);
    int b = !new.width || ((new.x[0] | new.z[0]) & 1u)
        ? 2 : (int)(new.bits[0] & 1u);
    return kind == LLG_EV_POSEDGE ? (a == 0 && b != 0) || (a == 2 && b == 1)
                                  : (a == 1 && b != 1) || (a == 2 && b == 0);
}

static llg_clocking_edge_t* find_clocking_edge(sv4_t* signal) {
    for (llg_clocking_edge_t* edge = g.clocking_edges; edge; edge = edge->next) {
        if (edge->signal == signal) return edge;
    }
    return NULL;
}

static void clocking_record_edge(sv4_t* signal, sv4_t old, sv4_t value) {
    if (!signal || sv4_same(old, value)) return;
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (!edge) {
        edge = (llg_clocking_edge_t*)llg_checked_malloc(
            1, sizeof(*edge), "clocking event history");
        edge->signal = signal;
        edge->any_time = UINT64_MAX;
        edge->posedge_time = UINT64_MAX;
        edge->negedge_time = UINT64_MAX;
        edge->posedge_count = 0;
        edge->negedge_count = 0;
        edge->next = g.clocking_edges;
        g.clocking_edges = edge;
    }
    edge->any_time = g.now;
    if (ev_matches(old, value, LLG_EV_POSEDGE)) {
        edge->posedge_time = g.now;
        if (edge->posedge_count != UINT64_MAX) edge->posedge_count++;
    }
    if (ev_matches(old, value, LLG_EV_NEGEDGE)) {
        edge->negedge_time = g.now;
        if (edge->negedge_count != UINT64_MAX) edge->negedge_count++;
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
            uint64_t time = srcs[i].kind == LLG_EV_POSEDGE
                                ? edge->posedge_time
                                : srcs[i].kind == LLG_EV_NEGEDGE
                                      ? edge->negedge_time
                                      : edge->any_time;
            if (time == g.now) return 1;
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
    n->is_real = drive->is_real;
    n->real_target = drive->real_target;
    n->real_value = drive->real_value;
    enqueue_nba(n);
}

static int clocking_drive_source_matches_signal(
    const llg_clocking_drive_t* drive, sv4_t* signal, sv4_t old, sv4_t value) {
    for (int i = 0; i < drive->n_specs; i++) {
        const llg_wait_src_t* source = &drive->specs[i];
        if (source->sig == signal && ev_matches(old, value, source->kind)) return 1;
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
    llg_expr_event_spec_t* spec = &wait->expressions[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) spec->real_eval(&value, spec->eval_context);
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(wait->real_last[index], value, spec->kind);
        wait->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) spec->eval(value, spec->eval_context);
    else sv4_copy(value, spec->sig);
    int matched = ev_matches(wait->last[index], *value, spec->kind);
    sv4_move(&wait->last[index], value);
    llg_value_scope_end(scope);
    return matched && expression_qualifies(spec);
}

static int deferred_expression_update(llg_deferred_trigger_t* trigger,
                                      int index, sv4_t* sig, double* real) {
    llg_expr_event_spec_t* spec = &trigger->specs[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) spec->real_eval(&value, spec->eval_context);
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(trigger->real_last[index], value, spec->kind);
        trigger->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) spec->eval(value, spec->eval_context);
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

static void sig_write(sv4_t* target, sv4_t value) {
    if (!region_can_mutate("signal write")) return;
    if (target->width == value.width && sv4_same(*target, value)) return;
    // Callbacks can finish/disable the writer without returning through here.
    // Heap-backed registered owners survive both suspension and stack discard.
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = llg_value_scope_begin(2);
    sv4_t* owned = llg_value_scope_values(snapshots);
    sv4_copy(&owned[0], &value);
    sv4_copy(&owned[1], target);
    value = owned[0]; /* Borrows the registered snapshot until scope end. */
    sv4_t old = owned[1];
    clocking_record_edge(target, old, value);
    clocking_drive_signal_match(target, old, value);
    sv4_copy(target, &value);
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
    llg_wave_changed_sv4(target, &value, g.now);
#endif
    llg_wait_t* w = g.waiters;
    while (w) {
        llg_wait_t* next = w->next;
        int wake = 0;
        if (w->kind == W_EVENTS || w->kind == W_MIXED) {
            for (int i = 0; i < w->n; i++) {
                if (w->specs[i].sig == target) {
                    if (ev_matches(w->last[i], *target, w->specs[i].kind)) wake = 1;
                    sv4_copy(&w->last[i], target);
                }
            }
        } else if (w->kind == W_DEPS) {
            for (int i = 0; i < w->n; i++) {
                const llg_wait_dependency_t* dependency = &w->dependencies[i];
                if (dependency->sig == target) {
                    if (dependency->width) {
                        sv4_t value = sv4_part_select(dependency->value ? *dependency->value : *target,
                            (int64_t)dependency->lsb + dependency->width - 1, dependency->lsb);
                        if (!sv4_same(w->last[i], value)) wake = 1;
                        sv4_move(&w->last[i], &value);
                    } else wake = 1;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->n; i++) {
                if (expression_update(w, i, target, NULL)) wake = 1;
            }
        } else if (w->kind == W_LEVEL) {
            if (w->sig == target && sv4_same(*target, w->level_val)) wake = 1;
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
    deferred_trigger_source_change(target, NULL);
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(target, NULL, 0);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
}

// Real equality is bitwise: repeated NaNs with the same payload are
// suppressed, while changes in NaN payload and signed zero are observable.
static void real_write(double* target, double value) {
    if (!region_can_mutate("real write")) return;
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
    llg_wait_t* w = g.waiters;
    while (w) {
        llg_wait_t* next = w->next;
        int wake = 0;
        if (w->kind == W_DEPS) {
            for (int i = 0; i < w->n; i++) {
                if (w->dependencies[i].real == target) {
                    wake = 1;
                    break;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->n; i++) {
                if (expression_update(w, i, NULL, target)) wake = 1;
            }
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
    deferred_trigger_source_change(NULL, target);
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->real_target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(NULL, target, 1);
    if (target_pin) llg_value_scope_end(target_pin);
}

// ── Procedural force / release ───────────────────────────────────────────────
