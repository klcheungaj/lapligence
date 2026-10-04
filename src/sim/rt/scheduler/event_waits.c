
// Return the outcome of one event observed by a wait_order waiter:
//  1 completes the sequence, 0 keeps waiting, and -1 takes the failure arm.
// Repeated occurrences of already-consumed events are ignored, while an
// event that is still ahead in the sequence is an ordering violation.
static int event_order_match(llg_wait_t* w, llg_event_object_t* ev) {
    llg_wait_order_payload_t* order =
        w->payload.rare ? &w->payload.rare->order : NULL;
    if (!order || !order->sequence || order->next < 0 ||
        order->next >= order->n_order)
        return -1;
    if (order->sequence[order->next] == ev) {
        order->next++;
        return order->next == order->n_order ? 1 : 0;
    }
    for (int i = 0; i < order->next; i++) {
        if (order->sequence[i] == ev) return 0;
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
    ev->trigger_count++;
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
    for (int i = 0; i < n_triggered; i++)
        triggered[i]->wait.payload.event.inline_slot = -1;
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
    for (int i = 0; i < n; i++) {
        int* slot = event_wait_slot(&wake[i]->wait, ev, i);
        if (!slot) abort();
        *slot = -1;
    }
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        llg_wait_t* w = &wake[i]->wait;
        if (w->kind == W_EVENT_ORDER) {
            int result = event_order_match(w, ev);
            if (result != 0) {
                *w->payload.rare->order.result = result;
                wake_proc(wake[i]);
            } else {
                event_list_add(ev, wake[i], event_wait_slot(w, ev, -1));
            }
            continue;
        }
        int matched = w->kind != W_EXPR;
        if (!matched) {
            llg_wait_expression_payload_t* expression = &w->payload.expression;
            for (int j = 0; j < expression->n; j++) {
                if (expression->expressions[j].event_object == ev &&
                    expression_qualifies(&expression->expressions[j]))
                    matched = 1;
            }
        }
        if (matched) wake_proc(wake[i]);
        else event_list_add(ev, wake[i], event_wait_slot(w, ev, -1));
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

uint64_t llg_event_trigger_count(const llg_event_t* ev) {
    return ev && ev->object ? ev->object->trigger_count : 0;
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

llg_co_arm_t llg_arm_event(llg_proc_t* self, llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    return llg_arm_events(self, list, 1);
}

llg_co_arm_t llg_arm_events(llg_proc_t* self,
                            const llg_event_t* const* evs, int n) {
    llg_runtime_service_enter(self, "event wait");
    if (n <= 0 || !self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->payload.event.n_evs = n;
    if (n == 1) {
        w->payload.event.evs = &w->payload.event.inline_ev;
        w->payload.event.event_slots = &w->payload.event.inline_slot;
        w->payload.event.inline_slot = -1;
    } else {
        w->payload.event.evs = event_wait_list_new(n, &w->payload.event.event_slots);
    }
    for (int i = 0; i < n; i++) {
        w->payload.event.evs[i] = evs[i] ? evs[i]->object : NULL;
        event_list_add(w->payload.event.evs[i], self, &w->payload.event.event_slots[i]);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_event_triggered(llg_proc_t* self,
                                     const llg_event_t* ev) {
    llg_runtime_service_enter(self, "event triggered wait");
    if (!self || !region_can_mutate("wait scheduling") ||
        llg_event_triggered(ev))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT_TRIGGERED;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->payload.event.triggered_ev = ev ? ev->object : NULL;
    w->payload.event.inline_slot = -1;
    event_triggered_list_add(w->payload.event.triggered_ev, self);
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_assertion(llg_proc_t* self, uint64_t identity) {
    llg_runtime_service_enter(self, "assertion wait");
    if (!self || !region_can_mutate("expect scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_ASSERTION;
    // An assertion result is observed before Reactive actions, so resume the
    // procedural expect continuation in Reactive after its action callback
    // has been queued.
    w->resume_region = LLG_REGION_REACTIVE;
    wait_rare_allocate(w, "assertion wait payload")->assertion.identity = identity;
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_order(llg_proc_t* self,
                           const llg_event_t* const* evs, int n,
                           int* result) {
    llg_runtime_service_enter(self, "wait_order");
    if (n <= 0 || !result || !self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT_ORDER;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_order_payload_t* order =
        &wait_rare_allocate(w, "wait_order payload")->order;
    order->n_order = n;
    order->next = 0;
    *result = 0;
    order->result = result;
    order->sequence = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order sequence");
    order->evs = event_wait_list_new(n, &order->event_slots);
    order->n_evs = 0;
    for (int i = 0; i < n; i++) {
        llg_event_object_t* object = evs[i] ? evs[i]->object : NULL;
        order->sequence[i] = object;
        if (!object) continue;
        int seen = 0;
        for (int j = 0; j < order->n_evs; j++) {
            if (order->evs[j] == object) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            order->evs[order->n_evs] = object;
            event_list_add(object, self, &order->event_slots[order->n_evs++]);
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_mixed(llg_proc_t* self,
                           const llg_wait_src_t* srcs, int n) {
    llg_runtime_service_enter(self, "mixed wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_mixed_payload_t* mixed =
        &wait_rare_allocate(w, "mixed wait payload")->mixed;
    mixed->n = nsig;
    mixed->specs = nsig ? (llg_event_spec_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(llg_event_spec_t), "mixed wait specifications") : NULL;
    mixed->last = nsig ? (sv4_t*)llg_checked_calloc(
        (size_t)nsig, sizeof(sv4_t), "mixed wait snapshots") : NULL;
    mixed->n_evs = nev;
    mixed->evs = nev ? event_wait_list_new(nev, &mixed->event_slots) : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            mixed->specs[si].sig = srcs[i].sig;
            mixed->specs[si].kind = srcs[i].kind;
            mixed->last[si] = sv4_clone(srcs[i].sig);
            si++;
        } else {
            mixed->evs[ei] = srcs[i].ev ? srcs[i].ev->object : NULL;
            event_list_add(mixed->evs[ei], self, &mixed->event_slots[ei]);
            ei++;
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_clocking_cycle(llg_proc_t* self,
                                    const llg_wait_src_t* srcs, int n,
                                    int accept_current) {
    llg_runtime_service_enter(self, "clocking cycle wait");
    if (!srcs || n <= 0 || !self ||
        !region_can_mutate("clocking cycle wait"))
        return LLG_CO_ARM_READY;
    if (accept_current && clocking_event_current(srcs, n))
        return LLG_CO_ARM_READY;
    return llg_arm_mixed(self, srcs, n);
}

llg_co_arm_t llg_arm_expressions(llg_proc_t* self,
                                 const llg_expr_event_spec_t* specs, int n) {
    if (n < 0) abort();
    llg_runtime_service_enter(self, "expression wait");
    if (!self || !region_can_mutate("wait scheduling")) {
        release_expression_contexts(specs, n);
        return LLG_CO_ARM_READY;
    }
    llg_wait_t* w = &self->wait;
    w->kind = W_EXPR;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_expression_payload_t* expression = &w->payload.expression;
    expression->n = n;
    expression->n_evs = 0;
    expression->expressions = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(llg_expr_event_spec_t), "expression event descriptors");
    expression->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "expression event snapshots");
    expression->real_last = (double*)llg_checked_malloc(
        (size_t)n, sizeof(double), "real expression event snapshots");
    expression->evs = event_wait_list_new(n, &expression->event_slots);
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        expression->expressions[i] = specs[i];
        expression->expressions[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        expression->expressions[i].reads = NULL;
        expression->expressions[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            expression->expressions[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "expression dependencies");
            memcpy(expression->expressions[i].reads, specs[i].reads,
                   (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            expression->expressions[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "typed expression dependencies");
            for (int j = 0; j < specs[i].n_dependencies; j++) {
                if ((specs[i].dependencies[j].sig == NULL) ==
                    (specs[i].dependencies[j].real == NULL))
                    abort();
            }
            memcpy(expression->expressions[i].dependencies, specs[i].dependencies,
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
            for (int j = 0; j < expression->n_evs; j++)
                if (expression->evs[j] == object) seen = 1;
            if (!seen) {
                expression->evs[expression->n_evs] = object;
                event_list_add(object, self,
                               &expression->event_slots[expression->n_evs++]);
            }
        } else if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&expression->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig)
                expression->real_last[i] = *specs[i].real_sig;
            else abort();
        } else if (specs[i].eval) {
            specs[i].eval(&expression->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            expression->last[i] = sv4_clone(specs[i].sig);
        } else {
            abort();
        }
        if (self->chain.exiting) break;
    }
    if (self->chain.exiting) {
        event_unlink(w);
        wait_payload_release(w);
        return LLG_CO_ARM_EXIT;
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

uint64_t llg_repeat_count(sv4_t value) {
    sv4_t count = sv4_repeat_count(value);
    for (int i = 1; i < llg_sv4_nlimbs(llg_sv4_width(count)); i++) {
        if (llg_sv4_word(count, i, LLG_SV4_BITS)) {
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
