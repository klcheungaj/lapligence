
// Return the outcome of one event observed by a wait_order waiter:
//  1 completes the sequence, 0 keeps waiting, and -1 takes the failure arm.
// Repeated occurrences of already-consumed events are ignored, while an
// event that is still ahead in the sequence is an ordering violation.
static int event_order_match(llg_wait_t* w, llg_event_object_t* ev) {
    if (!w->order_sequence || w->order_next < 0 ||
        w->order_next >= w->n_order)
        return -1;
    if (w->order_sequence[w->order_next] == ev) {
        w->order_next++;
        return w->order_next == w->n_order ? 1 : 0;
    }
    for (int i = 0; i < w->order_next; i++) {
        if (w->order_sequence[i] == ev) return 0;
    }
    return -1;
}

static void event_trigger_object_unchecked(llg_event_object_t* ev) {
    if (!ev) return;

    // The state is tied to both the current simulation time and this runtime
    // generation. Comparing the generation avoids stale `.triggered` state
    // when a generated model is initialized again after cleanup; comparing
    // the time preserves all zero-delay deltas in the current slot.
    ev->triggered = 1;
    ev->triggered_time = g.now;
    ev->triggered_generation = llg_event_generation;
    clocking_drive_event_match(ev);

    // Snapshot and detach everyone first: wake_proc unlinks the waiter from
    // every event list it registered on, which must not fight the iteration
    // over this event's own table. Wake order is the snapshot order, i.e. the
    // current table order: deterministic, and equal to registration order
    // unless earlier partial unlinks (swap-with-last) reordered it. The
    // snapshots are heap scratch because the waiter tables grow without a
    // fixed ceiling.
    int n_triggered = ev->n_triggered_waiters;
    llg_proc_t** triggered = n_triggered
        ? (llg_proc_t**)llg_checked_malloc(
              (size_t)n_triggered, sizeof(*triggered),
              "triggered event wake snapshot")
        : NULL;
    if (triggered)
        memcpy(triggered, ev->triggered_waiters,
               (size_t)n_triggered * sizeof(*triggered));
    ev->n_triggered_waiters = 0;
    for (int i = 0; i < n_triggered; i++) wake_proc(triggered[i]);
    free(triggered);

    int n = ev->n_waiters;
    llg_proc_t** wake = n
        ? (llg_proc_t**)llg_checked_malloc(
              (size_t)n, sizeof(*wake), "event wake snapshot")
        : NULL;
    if (wake)
        memcpy(wake, ev->waiters, (size_t)n * sizeof(*wake));
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        llg_wait_t* w = &wake[i]->wait;
        if (w->kind == W_EVENT_ORDER) {
            int result = event_order_match(w, ev);
            if (result != 0) {
                w->order_result_value = result;
                wake_proc(wake[i]);
            } else {
                event_list_add(ev, wake[i]);
            }
            continue;
        }
        int matched = w->kind != W_EXPR;
        if (!matched) {
            for (int j = 0; j < w->n; j++) {
                if (w->expressions[j].event_object == ev &&
                    expression_qualifies(&w->expressions[j]))
                    matched = 1;
            }
        }
        if (matched) wake_proc(wake[i]);
        else event_list_add(ev, wake[i]);
    }
    free(wake);
    deferred_trigger_event(ev);
}

static void event_trigger_object(llg_event_object_t* ev) {
    if (!region_can_mutate("event scheduling")) return;
    event_trigger_object_unchecked(ev);
}

static void clocking_event_callback(void* data) {
    llg_event_t* event = data;
    event_trigger_object_unchecked(event ? event->object : NULL);
}

int llg_clocking_event_observed(llg_event_t* event) {
    if (!event) return 0;
    // Event handles are runtime-owned until cleanup. Earlier Observed sample
    // callbacks are FIFO, so this callback sees the complete block publication.
    return llg_schedule_region_callback(LLG_REGION_OBSERVED, clocking_event_callback, event);
}

void llg_event_trigger(llg_event_t* ev) {
    event_trigger_object(ev ? ev->object : NULL);
}

int llg_event_triggered(const llg_event_t* ev) {
    if (!ev || !ev->object || !ev->object->triggered ||
        ev->object->triggered_generation != llg_event_generation)
        return 0;
    if (ev->object->triggered_time != g.now) {
        ev->object->triggered = 0;
        return 0;
    }
    return 1;
}

void llg_event_assign(llg_event_t* target, const llg_event_t* source) {
    if (!target || !region_can_mutate("event handle write")) return;
    target->object = source ? source->object : NULL;
}

void llg_event_assign_null(llg_event_t* target) {
    llg_event_assign(target, NULL);
}

llg_event_t* llg_event_array_select(llg_event_t* const* elements,
                                    uint64_t total,
                                    const int32_t* left,
                                    const int32_t* right,
                                    const sv4_t* indices,
                                    int n) {
    if (!elements || !left || !right || !indices || n <= 0 || !total)
        return NULL;
    uint64_t linear = 0;
    for (int i = 0; i < n; i++) {
        int64_t value;
        if (!sv4_to_index_i64(indices[i], &value)) return NULL;
        int64_t lo = left[i] < right[i] ? left[i] : right[i];
        int64_t hi = left[i] > right[i] ? left[i] : right[i];
        if (value < lo || value > hi) return NULL;
        uint64_t offset = left[i] >= right[i]
                              ? (uint64_t)((int64_t)left[i] - value)
                              : (uint64_t)(value - (int64_t)left[i]);
        uint64_t extent = (uint64_t)(llabs((int64_t)left[i] - right[i])) + 1;
        if (extent && linear > (UINT64_MAX - offset) / extent) return NULL;
        linear = linear * extent + offset;
    }
    return linear < total ? elements[linear] : NULL;
}

void llg_wait_event(llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    llg_wait_events(list, 1);
}

void llg_wait_events(const llg_event_t* const* evs, int n) {
    if (n <= 0) return;
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n_evs = n;
    w->evs = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "named-event wait list");
    for (int i = 0; i < n; i++) {
        w->evs[i] = evs[i] ? evs[i]->object : NULL;
        event_list_add(w->evs[i], p);
    }
    register_wait();
    aco_yield();
}

void llg_wait_event_triggered(const llg_event_t* ev) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    if (llg_event_triggered(ev)) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT_TRIGGERED;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->triggered_ev = ev ? ev->object : NULL;
    event_triggered_list_add(w->triggered_ev, p);
    register_wait();
    aco_yield();
}

void llg_wait_assertion(uint64_t identity) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("expect scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_ASSERTION;
    // An assertion result is observed before Reactive actions, so resume the
    // procedural expect continuation in Reactive after its action callback
    // has been queued.
    w->resume_region = LLG_REGION_REACTIVE;
    w->assertion_identity = identity;
    register_wait();
    aco_yield();
}

void llg_wait_order(const llg_event_t* const* evs, int n, int* result) {
    if (n <= 0 || !result) return;
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT_ORDER;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n_order = n;
    w->order_next = 0;
    w->order_result_value = 0;
    *result = 0;
    w->order_sequence = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order sequence");
    w->evs = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order event list");
    w->n_evs = 0;
    for (int i = 0; i < n; i++) {
        llg_event_object_t* object = evs[i] ? evs[i]->object : NULL;
        w->order_sequence[i] = object;
        if (!object) continue;
        int seen = 0;
        for (int j = 0; j < w->n_evs; j++) {
            if (w->evs[j] == object) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            w->evs[w->n_evs++] = object;
            event_list_add(object, p);
        }
    }
    register_wait();
    aco_yield();
    *result = w->order_result_value;
    w->order_result_value = 0;
}

void llg_wait_mixed(llg_wait_src_t* srcs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n = nsig;
    w->specs = nsig ? (llg_event_spec_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(llg_event_spec_t), "mixed wait specifications") : NULL;
    w->last = nsig ? (sv4_t*)llg_checked_calloc(
        (size_t)nsig, sizeof(sv4_t), "mixed wait snapshots") : NULL;
    w->n_evs = nev;
    w->evs = nev ? (llg_event_object_t**)llg_checked_malloc(
        (size_t)nev, sizeof(llg_event_object_t*), "mixed named-event wait list") : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            w->specs[si].sig = srcs[i].sig;
            w->specs[si].kind = srcs[i].kind;
            w->last[si] = sv4_clone(srcs[i].sig);
            si++;
        } else {
            w->evs[ei] = srcs[i].ev ? srcs[i].ev->object : NULL;
            event_list_add(w->evs[ei], p);
            ei++;
        }
    }
    register_wait();
    aco_yield();
}

void llg_wait_clocking_cycles(llg_wait_src_t* srcs, int n, sv4_t count) {
    if (!srcs || n <= 0 || !llg_current() || !region_can_mutate("clocking cycle wait")) return;
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_repeat_count(count));
    sv4_replace(&values[1], sv4_from_u64(1, values[0].width, 0));
    if (!sv4_to_bool(values[0])) {
        if (!clocking_event_current(srcs, n)) llg_wait_mixed(srcs, n);
    } else {
        while (sv4_to_bool(values[0])) {
            llg_wait_mixed(srcs, n);
            sv4_replace(&values[0], sv4_sub(values[0], values[1]));
        }
    }
    llg_value_scope_end(scope);
}

void llg_wait_expressions(const llg_expr_event_spec_t* specs, int n) {
    if (n < 0) abort();
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) {
        release_expression_contexts(specs, n);
        return;
    }
    llg_wait_t* w = &p->wait;
    w->kind = W_EXPR;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n = n;
    w->n_evs = 0;
    w->expressions = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(llg_expr_event_spec_t), "expression event descriptors");
    w->last = (sv4_t*)llg_checked_calloc((size_t)n, sizeof(sv4_t), "expression event snapshots");
    w->real_last = (double*)llg_checked_malloc(
        (size_t)n, sizeof(double), "real expression event snapshots");
    w->evs = (llg_event_object_t**)llg_checked_malloc((size_t)n, sizeof(llg_event_object_t*), "expression named events");
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        w->expressions[i] = specs[i];
        w->expressions[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        w->expressions[i].reads = NULL;
        w->expressions[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            w->expressions[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "expression dependencies");
            memcpy(w->expressions[i].reads, specs[i].reads, (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            w->expressions[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "typed expression dependencies");
            for (int j = 0; j < specs[i].n_dependencies; j++) {
                if ((specs[i].dependencies[j].sig == NULL) ==
                    (specs[i].dependencies[j].real == NULL))
                    abort();
            }
            memcpy(w->expressions[i].dependencies, specs[i].dependencies,
                   (size_t)specs[i].n_dependencies * sizeof(llg_wait_dependency_t));
        }
    }
    // Adopt every descriptor/context before invoking user code. An evaluator
    // can finish the process on its first call; teardown must also discover
    // contexts belonging to later entries which have not been evaluated yet.
    for (int i = 0; i < n; i++) {
        if (specs[i].event) {
            llg_event_object_t* object = specs[i].event->object;
            int seen = 0;
            for (int j = 0; j < w->n_evs; j++) if (w->evs[j] == object) seen = 1;
            if (!seen) {
                w->evs[w->n_evs++] = object;
                event_list_add(object, p);
            }
        } else if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&w->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig) w->real_last[i] = *specs[i].real_sig;
            else abort();
        } else if (specs[i].eval) {
            specs[i].eval(&w->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            w->last[i] = sv4_clone(specs[i].sig);
        } else {
            abort();
        }
    }
    register_wait();
    aco_yield();
}

uint64_t llg_repeat_count(sv4_t value) {
    sv4_t count = sv4_repeat_count(value);
    for (int i = 1; i < llg_sv4_nlimbs(count.width); i++) {
        if (count.bits[i]) {
            fprintf(stderr,
                    "llg runtime fatal: nonblocking repeat count exceeds 64 bits\n");
            abort();
        }
    }
    uint64_t result = sv4_to_u64(count);
    sv4_destroy(&count);
    return result;
}

static void register_deferred_trigger(const llg_expr_event_spec_t* specs,
                                      int n, uint64_t repeat,
                                      llg_event_object_t* target,
                                      llg_event_assignment_fn action,
                                      llg_frame_t* action_frame) {
    if (n < 0) abort();
    if ((!target && !action) ||
        !region_can_mutate("nonblocking event registration")) {
        release_expression_contexts(specs, n);
        if (action_frame) llg_frame_release(action_frame);
        return;
    }
    if (!repeat || n == 0) {
        release_expression_contexts(specs, n);
        if (action) {
            invoke_deferred_action(action, action_frame);
        } else if (target) {
            llg_nba_t* nba = new_nba(0);
            if (nba) {
                nba->event_target = target;
                nba->is_event = 1;
                enqueue_nba(nba);
            }
            if (action_frame) llg_frame_release(action_frame);
        } else if (action_frame) {
            llg_frame_release(action_frame);
        }
        return;
    }
    if (!specs) abort();
    llg_deferred_trigger_t* trigger = (llg_deferred_trigger_t*)llg_checked_calloc(
        1, sizeof(*trigger), "deferred nonblocking event trigger");
    trigger->target = target;
    trigger->action = action;
    trigger->action_frame = action_frame;
    trigger->n = n;
    trigger->remaining = repeat;
    trigger->specs = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->specs), "deferred event descriptors");
    trigger->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->last), "deferred event snapshots");
    trigger->real_last = (double*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->real_last), "deferred real event snapshots");
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        trigger->specs[i] = specs[i];
        trigger->specs[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        trigger->specs[i].reads = NULL;
        trigger->specs[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            trigger->specs[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "deferred event dependencies");
            memcpy(trigger->specs[i].reads, specs[i].reads,
                   (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            trigger->specs[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "deferred typed event dependencies");
            memcpy(trigger->specs[i].dependencies, specs[i].dependencies,
                   (size_t)specs[i].n_dependencies * sizeof(llg_wait_dependency_t));
        }
        if (specs[i].event) continue;
        if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&trigger->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig)
                trigger->real_last[i] = *specs[i].real_sig;
            else
                abort();
        } else if (specs[i].eval) {
            specs[i].eval(&trigger->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            trigger->last[i] = sv4_clone(specs[i].sig);
        } else {
            abort();
        }
    }
    if (g.deferred_trigger_tail)
        g.deferred_trigger_tail->next = trigger;
    else
        g.deferred_triggers = trigger;
    g.deferred_trigger_tail = trigger;
}

void llg_nba_event_when(const llg_expr_event_spec_t* specs, int n,
                        llg_event_t* target, uint64_t repeat) {
    register_deferred_trigger(specs, n, repeat,
                              target ? target->object : NULL, NULL, NULL);
}

void llg_nba_event_assign_when(const llg_expr_event_spec_t* specs, int n,
                               uint64_t repeat,
                               llg_event_assignment_fn action,
                               llg_frame_t* frame) {
    if (!action) {
        release_expression_contexts(specs, n);
        if (frame) llg_frame_release(frame);
        return;
    }
    register_deferred_trigger(specs, n, repeat, NULL, action, frame);
}
