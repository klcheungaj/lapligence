
static void assertion_action(llg_concurrent_assertion_t* assertion,
                             llg_concurrent_assertion_action_fn action) {
    if (!action || g.finish) return;
    const char* name = assertion->label && assertion->label[0]
                           ? assertion->label
                           : "concurrent assertion action";
    llg_proc_t* proc = llg_spawn_in_region(action, name, LLG_REGION_REACTIVE);
    if (proc) {
        proc->is_assertion_action = 1;
        proc->action_assertion = assertion->identity;
    }
}

static void assertion_vacuous(void) {
    if (llg_assertion_vacuous_total == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: assertion vacuity counter overflow\n");
        abort();
    }
    llg_assertion_vacuous_total++;
}

static void assertion_default_failure_action(void* data) {
    // Assertion records outlive callbacks; cleanup discards callbacks first.
    llg_concurrent_assertion_t* assertion = data;
    assertion_report_failure(assertion->kind, assertion->label,
                              assertion->location);
}

static void assertion_result(llg_concurrent_assertion_t* assertion, int success,
                             int vacuous) {
    if (vacuous) assertion_vacuous();
    if (success) {
        // Cover counts and pass actions represent a non-vacuous match. An
        // implication with a false antecedent is accounted separately but is
        // not a coverage hit. Assert/assume pass actions still run for their
        // vacuous success, as required by assertion action semantics.
        if (assertion->kind == LLG_ASSERTION_COVER && !vacuous)
            llg_assertion_cover(assertion->identity, assertion->label,
                                assertion->location);
        if (assertion->kind != LLG_ASSERTION_COVER || !vacuous)
            assertion_action(assertion, assertion->pass_action);
    } else {
        if (assertion->kind == LLG_ASSERTION_COVER) {
            assertion_action(assertion, assertion->fail_action);
        } else {
            assertion_record_failure(assertion->kind);
            if (assertion->fail_action) {
                // Even an explicit null else is a generated action function.
                assertion_action(assertion, assertion->fail_action);
            } else {
                (void)llg_schedule_region_callback(
                    LLG_REGION_REACTIVE, assertion_default_failure_action,
                    assertion);
            }
        }
    }
    if (assertion->kind == LLG_ASSERTION_EXPECT && assertion->expect_active) {
        assertion->expect_active = 0;
        wake_assertion_waiter(assertion->identity);
    }
}

static void assertion_disable_signal_changed(sv4_t* signal) {
    if (!signal) return;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (assertion->disable == signal && sv4_to_bool(*signal))
            free_assertion_attempts(assertion);
    }
}

static void assertion_abort_attempts(llg_concurrent_assertion_t* assertion) {
    if (!assertion || !assertion->abort_condition) return;
    free_assertion_clock_events(assertion);
    const int success = assertion->abort_reject ? 0 : 1;
    while (assertion->attempts) {
        llg_assertion_attempt_t* attempt = assertion->attempts;
        assertion->attempts = attempt->next;
        if (!assertion->attempts) assertion->attempts_tail = NULL;
        assertion_result(assertion, success, assertion->abort_reject ? 0 : 1);
        free(attempt);
        if (g.finish) return;
    }
    llg_sequence_attempt_t** lists[] = {
        &assertion->sequence_antecedents,
        &assertion->sequence_consequents,
    };
    llg_sequence_attempt_t** tails[] = {
        &assertion->sequence_antecedents_tail,
        &assertion->sequence_consequents_tail,
    };
    for (size_t list_index = 0; list_index < sizeof(lists) / sizeof(lists[0]);
         list_index++) {
        while (*lists[list_index]) {
            llg_sequence_attempt_t* attempt = *lists[list_index];
            *lists[list_index] = attempt->next;
            assertion_result(assertion, success, assertion->abort_reject ? 0 : 1);
            sequence_attempt_discard(attempt);
            if (g.finish) return;
        }
        *tails[list_index] = NULL;
    }
}

static void assertion_abort_condition_changed(void) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if ((assertion->enabled || assertion_has_attempts(assertion)) &&
            (assertion->kind != LLG_ASSERTION_EXPECT || assertion->expect_active) &&
            assertion->abort_condition && !assertion->abort_sync &&
            assertion->abort_condition(assertion->data)) {
            assertion_abort_attempts(assertion);
            if (g.finish) return;
        }
    }
}

static int sequence_graph_uses_clock(const llg_sequence_graph_t* graph,
                                     sv4_t* signal, int edge) {
    if (!graph || !signal) return 0;
    for (uint32_t index = 0; index < graph->transition_count; index++) {
        const llg_sequence_transition_t* transition = &graph->transitions[index];
        if (transition->clock == signal && transition->edge == edge) return 1;
    }
    return 0;
}

static int assertion_sequence_uses_clock(llg_concurrent_assertion_t* assertion,
                                         sv4_t* signal, int edge) {
    return assertion &&
           (sequence_graph_uses_clock(assertion->antecedent_sequence, signal,
                                      edge) ||
            sequence_graph_uses_clock(assertion->consequent_sequence, signal,
                                      edge));
}

static void assertion_clock_event_append(llg_concurrent_assertion_t* assertion,
                                         sv4_t* signal, int edge,
                                         uint64_t order) {
    llg_assertion_clock_event_t* event = (llg_assertion_clock_event_t*)llg_checked_calloc(
        1, sizeof(*event), "concurrent assertion clock event");
    event->signal = signal;
    event->edge = edge;
    event->time = g.now;
    event->order = order;
    event->tick = assertion_clock_tick(signal, edge);
    if (assertion->clock_events_tail)
        assertion->clock_events_tail->next = event;
    else
        assertion->clock_events = event;
    assertion->clock_events_tail = event;
    if (assertion->clock_history && assertion->clock_history->time != g.now) {
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            free(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
    llg_assertion_clock_event_t* saved = llg_checked_calloc(1, sizeof(*saved), "sequence clock history");
    *saved = *event;
    saved->next = NULL;
    if (assertion->clock_history_tail) assertion->clock_history_tail->next = saved;
    else assertion->clock_history = saved;
    assertion->clock_history_tail = saved;
}

static void assertion_clock_signal_changed(sv4_t* signal, sv4_t old,
                                           sv4_t value) {
    if (!signal) return;
    if (llg_assertion_event_order == UINT64_MAX) {
        fprintf(stderr, "llg: concurrent assertion event-order overflow\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    uint64_t order = llg_assertion_event_order++;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if ((!assertion->enabled && !assertion_has_attempts(assertion)) ||
            (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active))
            continue;
        if (assertion->disable && sv4_to_bool(*assertion->disable)) continue;
        if (assertion->clock == signal &&
            ev_matches(old, value, assertion->edge)) {
            if (assertion->consequent_sequence)
                assertion_clock_event_append(assertion, signal, assertion->edge,
                                             order);
            else
                assertion->edge_pending = 1;
            continue;
        }
        if (assertion->consequent_sequence &&
            ev_matches(old, value, LLG_EV_POSEDGE) &&
            assertion_sequence_uses_clock(assertion, signal, LLG_EV_POSEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_POSEDGE,
                                         order);
        } else if (assertion->consequent_sequence &&
                   ev_matches(old, value, LLG_EV_NEGEDGE) &&
                   assertion_sequence_uses_clock(assertion, signal,
                                                 LLG_EV_NEGEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_NEGEDGE,
                                         order);
        }
    }
}

static int run_sequence_concurrent_assertion(llg_concurrent_assertion_t* assertion,
                                             uint64_t cycle,
                                             sv4_t* event_clock,
                                             int event_edge, uint64_t event_time,
                                             uint64_t event_order,
                                             uint64_t event_tick, int root_event) {
    llg_sequence_attempt_t** antecedent_link = &assertion->sequence_antecedents;
    while (*antecedent_link) {
        llg_sequence_attempt_t* attempt = *antecedent_link;
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted && !sequence_spawn_consequents(assertion, attempt, cycle)) return 0;
        if (!alive) {
            *antecedent_link = attempt->next;
            if (assertion->sequence_antecedents_tail == attempt)
                assertion->sequence_antecedents_tail = NULL;
            if (!attempt->matched) assertion_result(assertion, 1, 1);
            sequence_attempt_discard(attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        } else {
            antecedent_link = &attempt->next;
        }
        if (g.finish) return 0;
    }
    if (assertion->sequence_antecedents_tail == NULL) {
        for (llg_sequence_attempt_t* item = assertion->sequence_antecedents;
             item; item = item->next)
            assertion->sequence_antecedents_tail = item;
    }

    if (root_event && assertion->enabled && assertion->antecedent_sequence) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->antecedent_sequence, cycle, NULL, NULL);
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted && !sequence_spawn_consequents(assertion, attempt, cycle)) {
            sequence_attempt_discard(attempt);
            return 0;
        }
        if (alive) {
            sequence_attempt_append(&assertion->sequence_antecedents,
                                    &assertion->sequence_antecedents_tail, attempt);
        } else {
            if (!attempt->matched) assertion_result(assertion, 1, 1);
            sequence_attempt_discard(attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        }
    } else if (root_event && assertion->enabled) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->consequent_sequence, cycle, NULL, NULL);
        sequence_attempt_append(&assertion->sequence_consequents,
                                &assertion->sequence_consequents_tail, attempt);
    }

    llg_sequence_attempt_t** consequent_link = &assertion->sequence_consequents;
    while (*consequent_link) {
        llg_sequence_attempt_t* attempt = *consequent_link;
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted || !alive) {
            *consequent_link = attempt->next;
            if (assertion->sequence_consequents_tail == attempt)
                assertion->sequence_consequents_tail = NULL;
            sequence_attempt_discard(attempt);
            assertion_result(assertion, accepted, 0);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
            if (g.finish) return 0;
        } else {
            consequent_link = &attempt->next;
        }
    }
    if (assertion->sequence_consequents_tail == NULL) {
        for (llg_sequence_attempt_t* item = assertion->sequence_consequents;
             item; item = item->next)
            assertion->sequence_consequents_tail = item;
    }
    return 1;
}

static void run_concurrent_assertion(llg_concurrent_assertion_t* assertion) {
    // A clock transition is observed after Active/NBA writes, while every
    // predicate reads the immutable Preponed snapshot from this time slot.
    if ((!assertion->enabled && !assertion_has_attempts(assertion)) ||
        (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
        return;
    }
    if (assertion->disable && sv4_to_bool(*assertion->disable)) {
        assertion->edge_pending = 0;
        free_assertion_attempts(assertion);
        return;
    }
    if (assertion->consequent_sequence) {
        while (assertion->clock_events) {
            llg_assertion_clock_event_t* event = assertion->clock_events;
            assertion->clock_events = event->next;
            if (!assertion->clock_events) assertion->clock_events_tail = NULL;
            int root_event = event->signal == assertion->clock &&
                             event->edge == assertion->edge;
            uint64_t cycle = 0;
            if (!sequence_cycle_next(assertion, &cycle)) {
                free(event);
                return;
            }
            if (root_event && assertion->abort_condition &&
                assertion->abort_condition(assertion->data)) {
                int had_pending = assertion->attempts != NULL ||
                                  assertion->sequence_antecedents != NULL ||
                                  assertion->sequence_consequents != NULL;
                assertion_abort_attempts(assertion);
                // A synchronous accept/reject control also controls the new
                // attempt begun at this sampled leading-clock edge.
                if (!had_pending && assertion->enabled && !g.finish)
                    assertion_result(assertion, assertion->abort_reject ? 0 : 1,
                                     assertion->abort_reject ? 0 : 1);
                free(event);
                if (g.finish) return;
                continue;
            }
            (void)run_sequence_concurrent_assertion(
                assertion, cycle, event->signal, event->edge, event->time,
                event->order, event->tick, root_event);
            free(event);
            if (g.finish) return;
        }
        return;
    }

    int edge = assertion->edge_pending;
    assertion->edge_pending = 0;
    if (!edge) return;

    if (assertion->abort_condition && assertion->abort_condition(assertion->data)) {
        int had_pending = assertion->attempts != NULL ||
                          assertion->sequence_antecedents != NULL ||
                          assertion->sequence_consequents != NULL;
        assertion_abort_attempts(assertion);
        // A synchronous accept/reject control also controls the new attempt
        // begun at this sampled edge. Emit one result even when no older
        // attempt was pending, matching the per-clock evaluation contract.
        if (!had_pending && assertion->enabled && !g.finish)
            assertion_result(assertion, assertion->abort_reject ? 0 : 1,
                             assertion->abort_reject ? 0 : 1);
        return;
    }

    while (assertion->attempts) {
        llg_assertion_attempt_t* attempt = assertion->attempts;
        assertion->attempts = attempt->next;
        if (!assertion->attempts) assertion->attempts_tail = NULL;
        int success = assertion->consequent(assertion->data) != 0;
        assertion_result(assertion, success, 0);
        free(attempt);
        if (g.finish) return;
    }

    if (!assertion->enabled ||
        (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)) return;

    int antecedent = assertion->antecedent == NULL ||
                     assertion->antecedent(assertion->data) != 0;
    if (!antecedent) {
        assertion_result(assertion, 1, 1);
    } else if (assertion->overlapped) {
        assertion_result(assertion, assertion->consequent(assertion->data) != 0,
                         0);
    } else {
        assertion_attempt_enqueue(assertion);
    }
}

static int run_concurrent_assertions(void) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        run_concurrent_assertion(assertion);
        if (g.finish) return 0;
    }
    return 1;
}

static void flush_assertion_attempts(void) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next)
        free_assertion_attempts(assertion);
}

int llg_assertion_register_control(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    llg_concurrent_assertion_action_fn pass_action,
    llg_concurrent_assertion_action_fn fail_action, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    if (!g.main_co || g.running || g.config_error || !clock || !consequent ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition)) {
        fprintf(stderr, "llg: invalid concurrent assertion registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t* assertion =
        (llg_concurrent_assertion_t*)llg_checked_calloc(
            1, sizeof(*assertion), "concurrent assertion");
    assertion->clock = clock;
    assertion->edge = edge;
    assertion->disable = disable;
    assertion->antecedent = antecedent;
    assertion->consequent = consequent;
    assertion->abort_condition = abort_condition;
    assertion->pass_action = pass_action;
    assertion->fail_action = fail_action;
    assertion->data = data;
    assertion->kind = kind;
    assertion->overlapped = overlapped;
    assertion->abort_reject = abort_reject;
    assertion->abort_sync = abort_sync;
    assertion->identity = identity;
    assertion->label = label;
    assertion->location = location;
    assertion->scope = scope;
    assertion->enabled = 1;
    assertion->expect_active = 0;
    if (g.assertion_tail) {
        g.assertion_tail->next = assertion;
    } else {
        g.assertions = assertion;
    }
    g.assertion_tail = assertion;
    return 1;
}

int llg_assertion_register(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    llg_concurrent_assertion_action_fn pass_action,
    llg_concurrent_assertion_action_fn fail_action, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_action,
        fail_action, data, kind, overlapped, 0, 0, identity, label, location,
        scope);
}

void llg_deferred_assertion_scoped(int kind, int passed, uint64_t identity,
                            const char* label, const char* location, const char* scope,
                            llg_deferred_assertion_fn action,
                            llg_frame_t* frame) {
    if (kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_COVER ||
        (passed != 0 && passed != 1)) {
        fprintf(stderr, "llg runtime fatal: invalid deferred assertion result\n");
        llg_frame_release(frame);
        abort();
    }
    if (!llg_deferred_assertion_enabled(kind, label, scope)) {
        llg_frame_release(frame);
        return;
    }
    if (!action && frame) {
        // A frame is meaningful only for a selected action. This also keeps
        // malformed embedding calls from leaking an owned capture.
        llg_frame_release(frame);
        frame = NULL;
    }
    llg_proc_t* current = llg_current();
    uint64_t owner = g.in_deferred_action || !current
                         ? 0
                         : current->assertion_owner;
    for (llg_deferred_assertion_report_t* report = g.deferred_assertions;
         report; report = report->next) {
        if (report->owner == owner && report->time == g.now &&
            report->identity == identity) {
            llg_frame_release(report->frame);
            report->kind = kind;
            report->passed = passed;
            report->label = label;
            report->location = location;
            report->scope = scope;
            report->action = action;
            report->frame = frame;
            return;
        }
    }
    llg_deferred_assertion_report_t* report =
        (llg_deferred_assertion_report_t*)llg_checked_calloc(
            1, sizeof(*report), "deferred assertion report");
    report->owner = owner;
    report->time = g.now;
    report->kind = kind;
    report->passed = passed;
    report->identity = identity;
    report->label = label;
    report->location = location;
    report->scope = scope;
    report->action = action;
    report->frame = frame;
    if (g.deferred_assertion_tail) {
        g.deferred_assertion_tail->next = report;
    } else {
        g.deferred_assertions = report;
    }
    g.deferred_assertion_tail = report;
}

// Compatibility entrypoint for embedding callers without hierarchy metadata.
void llg_deferred_assertion(int kind, int passed, uint64_t identity,
                            const char* label, const char* location,
                            llg_deferred_assertion_fn action, llg_frame_t* frame) {
    llg_deferred_assertion_scoped(kind, passed, identity, label, location, "", action, frame);
}

static int valid_sequence_graph(const llg_sequence_graph_t* graph,
                                sv4_t* root_clock, int root_edge) {
    if (!graph || graph->states == 0 || graph->start >= graph->states ||
        graph->accept >= graph->states ||
        (graph->transition_count != 0 && !graph->transitions) ||
        graph->first_match_state_count != 0 ||
        (graph->local_count != 0 && !graph->locals))
        return 0;
    for (uint32_t index = 0; index < graph->local_count; index++) {
        const llg_sequence_local_t* local = &graph->locals[index];
        if (local->width == 0 || local->width > LLG_MAX_WIDTH ||
            (local->two_state != 0 && local->two_state != 1))
            return 0;
    }
    for (uint32_t index = 0; index < graph->first_match_state_count; index++)
        if (graph->first_match_states[index] >= graph->states) return 0;
    for (uint32_t index = 0; index < graph->transition_count; index++) {
        const llg_sequence_transition_t* transition = &graph->transitions[index];
        if (transition->from >= graph->states || transition->to >= graph->states ||
            transition->max_delay < transition->min_delay ||
            (transition->atom != LLG_SEQUENCE_EPSILON && !graph->atom))
            return 0;
        if ((transition->clock && transition->edge != LLG_EV_POSEDGE &&
             transition->edge != LLG_EV_NEGEDGE) ||
            (!transition->clock && transition->edge != 0))
            return 0;
        sv4_t* destination = transition->clock ? transition->clock : root_clock;
        int destination_edge = transition->clock ? transition->edge : root_edge;
        int exact_boundary = (transition->min_delay == 0 && transition->max_delay == 0) ||
                             (transition->min_delay == 1 && transition->max_delay == 1);
        if (!exact_boundary) {
            sv4_t* initial = graph->leading_clock ? graph->leading_clock : root_clock;
            int initial_edge = graph->leading_clock ? graph->leading_edge : root_edge;
            if (transition->from == graph->start &&
                (destination != initial || destination_edge != initial_edge)) return 0;
            for (uint32_t j = 0; j < graph->transition_count; j++) {
                const llg_sequence_transition_t* previous = &graph->transitions[j];
                if (previous->to != transition->from) continue;
                sv4_t* source = previous->clock ? previous->clock : root_clock;
                int source_edge = previous->clock ? previous->edge : root_edge;
                if (source != destination || source_edge != destination_edge) return 0;
            }
        }
        if (transition->match_count > graph->match_item_count ||
            transition->match_start > graph->match_item_count -
                transition->match_count ||
            (transition->match_count != 0 && !graph->match))
            return 0;
    }
    return 1;
}

int llg_assertion_register_sequence_control(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    llg_concurrent_assertion_action_fn pass_action,
    llg_concurrent_assertion_action_fn fail_action, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    if (!g.main_co || g.running || g.config_error || !clock ||
        !valid_sequence_graph(consequent, clock, edge) ||
        (antecedent && !valid_sequence_graph(antecedent, clock, edge)) ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition)) {
        fprintf(stderr, "llg: invalid concurrent sequence assertion registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t* assertion =
        (llg_concurrent_assertion_t*)llg_checked_calloc(
            1, sizeof(*assertion), "concurrent sequence assertion");
    assertion->clock = clock;
    assertion->edge = edge;
    assertion->disable = disable;
    assertion->pass_action = pass_action;
    assertion->fail_action = fail_action;
    assertion->abort_condition = abort_condition;
    assertion->data = data;
    assertion->kind = kind;
    assertion->overlapped = overlapped;
    assertion->abort_reject = abort_reject;
    assertion->abort_sync = abort_sync;
    assertion->identity = identity;
    assertion->label = label;
    assertion->location = location;
    assertion->scope = scope;
    assertion->enabled = 1;
    assertion->expect_active = 0;
    assertion->antecedent_sequence = antecedent;
    assertion->consequent_sequence = consequent;
    if (g.assertion_tail)
        g.assertion_tail->next = assertion;
    else
        g.assertions = assertion;
    g.assertion_tail = assertion;
    return 1;
}

int llg_assertion_register_sequence(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    llg_concurrent_assertion_action_fn pass_action,
    llg_concurrent_assertion_action_fn fail_action, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_sequence_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_action,
        fail_action, data, kind, overlapped, 0, 0, identity, label, location,
        scope);
}
