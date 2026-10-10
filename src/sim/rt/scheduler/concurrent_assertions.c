struct llg_property_inst;
static void run_property_assertion(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event, int root_event);
static void property_assertions_async(void);

static void assertion_action(llg_concurrent_assertion_t* assertion,
                             const llg_co_desc_t* desc) {
    if (!desc || g.finish) return;
    const char* name = assertion->label && assertion->label[0]
                           ? assertion->label
                           : "concurrent assertion action";
    llg_proc_t* proc = llg_spawn_in_region(desc, name, LLG_REGION_REACTIVE);
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
            assertion_action(assertion, assertion->pass_desc);
    } else {
        if (assertion->kind == LLG_ASSERTION_COVER) {
            assertion_action(assertion, assertion->fail_desc);
        } else {
            assertion_record_failure(assertion->kind);
            if (assertion->fail_desc) {
                // Even an explicit null else is a generated action function.
                assertion_action(assertion, assertion->fail_desc);
            } else if (!(assertion->kind == LLG_ASSERTION_EXPECT &&
                         assertion->expect_has_fail)) {
                // An expect's else arm runs inline in its resumed caller.
                (void)llg_schedule_region_callback(
                    LLG_REGION_REACTIVE, assertion_default_failure_action,
                    assertion);
            }
        }
    }
    if (assertion->kind == LLG_ASSERTION_EXPECT && assertion->expect_active) {
        assertion->expect_active = 0;
        assertion->expect_outcome = success ? 1 : 2;
        wake_assertion_waiter(assertion->identity);
    }
}

/* Whether a leading-clock event starts a new evaluation attempt. An armed
 * expect starts "a single thread of evaluation" on its first clocking event
 * (16.18); every other assertion starts one per event. */
static int assertion_attempt_starts(llg_concurrent_assertion_t* assertion) {
    if (!assertion->enabled) return 0;
    if (assertion->single_attempt) {
        if (assertion->single_attempt_started) return 0;
        assertion->single_attempt_started = 1;
        return 1;
    }
    if (assertion->kind != LLG_ASSERTION_EXPECT) return 1;
    if (!assertion->expect_active || assertion->expect_started) return 0;
    assertion->expect_started = 1;
    return 1;
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
            // An implication attempt shares one result across its antecedent
            // and consequent threads; report it once.
            llg_assertion_eval_t* eval = attempt->eval;
            int report = !eval || !eval->decided;
            if (eval) eval->decided = 1;
            sequence_attempt_discard(attempt);
            if (report)
                assertion_result(assertion, success, assertion->abort_reject ? 0 : 1);
            if (g.finish) return;
        }
        *tails[list_index] = NULL;
    }
}

static void assertion_abort_condition_changed(void) {
    property_assertions_async();
    if (g.finish) return;
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
    llg_assertion_clock_event_t* event = assertion_clock_event_alloc();
    event->signal = signal;
    event->edge = edge;
    event->time = g.now;
    event->order = order;
    event->tick = assertion->clock_gate && signal == assertion->clock &&
                          edge == assertion->edge
                      ? ++assertion->gated_ticks
                      : assertion_clock_tick(signal, edge);
    if (assertion->clock_events_tail)
        assertion->clock_events_tail->next = event;
    else
        assertion->clock_events = event;
    assertion->clock_events_tail = event;
    if (assertion->clock_history && assertion->clock_history->time != g.now) {
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            assertion_clock_event_recycle(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
    llg_assertion_clock_event_t* saved = assertion_clock_event_alloc();
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
            ev_matches_changed(old, value, assertion->edge)) {
            // A gated-off edge is no clock tick at all, for new attempts and
            // pending threads alike: transitions on this edge are the
            // leading clocking event (lowering rejects other forms).
            if (assertion->clock_gate && !assertion->clock_gate(NULL)) continue;
            if (assertion->consequent_sequence || assertion->property)
                assertion_clock_event_append(assertion, signal, assertion->edge,
                                             order);
            else
                assertion->edge_pending = 1;
            g.assertion_edges_pending = 1;
            continue;
        }
        if (assertion->consequent_sequence &&
            ev_matches(old, value, LLG_EV_POSEDGE) &&
            assertion_sequence_uses_clock(assertion, signal, LLG_EV_POSEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_POSEDGE,
                                         order);
            g.assertion_edges_pending = 1;
        } else if (assertion->consequent_sequence &&
                   ev_matches(old, value, LLG_EV_NEGEDGE) &&
                   assertion_sequence_uses_clock(assertion, signal,
                                                 LLG_EV_NEGEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_NEGEDGE,
                                         order);
            g.assertion_edges_pending = 1;
        }
    }
}

static void sequence_attempt_unlink(llg_sequence_attempt_t** link,
                                    llg_sequence_attempt_t** tail,
                                    llg_sequence_attempt_t* attempt) {
    *link = attempt->next;
    if (*tail == attempt) *tail = NULL;
}

static void sequence_list_fix_tail(llg_sequence_attempt_t* head,
                                   llg_sequence_attempt_t** tail) {
    if (*tail) return;
    for (llg_sequence_attempt_t* item = head; item; item = item->next) *tail = item;
}

/* The antecedent of an implication attempt can match no more. With no match
 * the attempt succeeds vacuously; otherwise it succeeds now only if every
 * consequent it started has already succeeded (16.13.6). */
static void sequence_antecedent_finished(llg_concurrent_assertion_t* assertion,
                                         llg_sequence_attempt_t* attempt) {
    llg_assertion_eval_t* eval = attempt->eval;
    int vacuous = !attempt->matched && (!eval || !eval->decided);
    int pass = attempt->matched && eval && !eval->decided && eval->pending == 0;
    if (eval && (vacuous || pass)) eval->decided = 1;
    sequence_attempt_discard(attempt);
    if (vacuous) assertion_result(assertion, 1, 1);
    else if (pass) assertion_result(assertion, 1, 0);
}

static int run_sequence_concurrent_assertion(llg_concurrent_assertion_t* assertion,
                                             uint64_t cycle,
                                             sv4_t* event_clock,
                                             int event_edge, uint64_t event_time,
                                             uint64_t event_order,
                                             uint64_t event_tick, int root_event) {
    g.sequence_current = assertion;
    llg_sequence_attempt_t** antecedent_link = &assertion->sequence_antecedents;
    while (*antecedent_link) {
        llg_sequence_attempt_t* attempt = *antecedent_link;
        if (attempt->eval && attempt->eval->decided) {
            // The attempt already failed: its further matches change nothing.
            sequence_attempt_unlink(antecedent_link, &assertion->sequence_antecedents_tail,
                                    attempt);
            sequence_attempt_discard(attempt);
            continue;
        }
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted && !sequence_spawn_consequents(assertion, attempt, cycle)) return 0;
        if (!alive) {
            sequence_attempt_unlink(antecedent_link, &assertion->sequence_antecedents_tail,
                                    attempt);
            sequence_antecedent_finished(assertion, attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        } else {
            antecedent_link = &attempt->next;
        }
        if (g.finish) return 0;
    }
    sequence_list_fix_tail(assertion->sequence_antecedents,
                           &assertion->sequence_antecedents_tail);

    int starts = root_event && assertion_attempt_starts(assertion);
    if (starts && assertion->antecedent_sequence) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->antecedent_sequence, cycle, NULL, NULL);
        attempt->eval = assertion_eval_new();
        attempt->eval_owner = 1;
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
            sequence_antecedent_finished(assertion, attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        }
    } else if (starts) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->consequent_sequence, cycle, NULL, NULL);
        sequence_attempt_append(&assertion->sequence_consequents,
                                &assertion->sequence_consequents_tail, attempt);
    }

    llg_sequence_attempt_t** consequent_link = &assertion->sequence_consequents;
    while (*consequent_link) {
        llg_sequence_attempt_t* attempt = *consequent_link;
        llg_assertion_eval_t* eval = attempt->eval;
        if (eval && eval->decided) {
            sequence_attempt_unlink(consequent_link, &assertion->sequence_consequents_tail,
                                    attempt);
            sequence_attempt_discard(attempt);
            continue;
        }
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (assertion->cover_sequence) {
            // Sequence coverage reports every nonempty match of the attempt,
            // with multiplicity, as it completes and keeps the attempt until
            // no thread remains.
            for (const llg_sequence_endpoint_t* endpoint = attempt->endpoints;
                 accepted && endpoint && !g.finish; endpoint = endpoint->next) {
                if (endpoint->empty || !sequence_mult_enumerable(endpoint->mult)) continue;
                for (uint64_t match = 0; match < endpoint->mult && !g.finish; match++)
                    assertion_result(assertion, 1, 0);
            }
            if (alive) {
                consequent_link = &attempt->next;
            } else {
                sequence_attempt_unlink(consequent_link,
                                        &assertion->sequence_consequents_tail, attempt);
                sequence_attempt_discard(attempt);
            }
            if (g.finish) return 0;
            continue;
        }
        if (accepted || !alive) {
            sequence_attempt_unlink(consequent_link, &assertion->sequence_consequents_tail,
                                    attempt);
            // One result per evaluation attempt: a failing consequent decides
            // it at once; the last success decides it once the antecedent is
            // exhausted (16.13.6, 16.15.3 "maximum of one per attempt").
            int report = 1;
            if (eval) {
                report = !eval->decided &&
                         (!accepted || (eval->pending == 1 && !eval->antecedent_live));
                if (report) eval->decided = 1;
            }
            sequence_attempt_discard(attempt);
            if (report) assertion_result(assertion, accepted, 0);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
            if (g.finish) return 0;
        } else {
            consequent_link = &attempt->next;
        }
    }
    sequence_list_fix_tail(assertion->sequence_consequents,
                           &assertion->sequence_consequents_tail);
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
    if (assertion->property) {
        while (assertion->clock_events) {
            llg_assertion_clock_event_t* event = assertion->clock_events;
            assertion->clock_events = event->next;
            if (!assertion->clock_events) assertion->clock_events_tail = NULL;
            int root_event = event->signal == assertion->clock &&
                             event->edge == assertion->edge;
            run_property_assertion(assertion, event, root_event);
            assertion_clock_event_recycle(event);
            if (g.finish) return;
        }
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
                assertion_clock_event_recycle(event);
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
                assertion_clock_event_recycle(event);
                if (g.finish) return;
                continue;
            }
            (void)run_sequence_concurrent_assertion(
                assertion, cycle, event->signal, event->edge, event->time,
                event->order, event->tick, root_event);
            assertion_clock_event_recycle(event);
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

    if (!assertion_attempt_starts(assertion)) return;

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
    g.assertion_edges_pending = 0;
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
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    if (!g.initialized || g.running || g.config_error || !clock || !consequent ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition) ||
        (pass_desc && !pass_desc->fn) || (fail_desc && !fail_desc->fn)) {
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
    assertion->pass_desc = pass_desc;
    assertion->fail_desc = fail_desc;
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
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_desc,
        fail_desc, data, kind, overlapped, 0, 0,
        identity, label, location, scope);
}

static int valid_sequence_graph(const llg_sequence_graph_t* graph,
                                sv4_t* root_clock, int root_edge) {
    if (!graph || graph->states == 0 || graph->start >= graph->states ||
        graph->accept >= graph->states ||
        (graph->transition_count != 0 && !graph->transitions) ||
        graph->first_match_state_count != 0 ||
        (graph->local_count != 0 && !graph->locals) ||
        (graph->join_count != 0 && !graph->joins))
        return 0;
    for (uint32_t index = 0; index < graph->join_count; index++) {
        const llg_sequence_join_t* join = &graph->joins[index];
        if ((join->kind != LLG_SEQUENCE_JOIN_AND && join->kind != LLG_SEQUENCE_JOIN_INTERSECT) ||
            join->left_start >= graph->states || join->right_start >= graph->states)
            return 0;
    }
    for (uint32_t index = 0; index < graph->local_count; index++) {
        const llg_sequence_local_t* local = &graph->locals[index];
        if (local->width == 0 || local->width >= LLG_SUPPORTED_WIDTH_LIMIT ||
            (local->two_state != 0 && local->two_state != 1))
            return 0;
    }
    for (uint32_t index = 0; index < graph->first_match_state_count; index++)
        if (graph->first_match_states[index] >= graph->states) return 0;
    for (uint32_t index = 0; index < graph->transition_count; index++) {
        const llg_sequence_transition_t* transition = &graph->transitions[index];
        if (transition->from >= graph->states || transition->to >= graph->states ||
            transition->max_delay < transition->min_delay ||
            (transition->atom != LLG_SEQUENCE_EPSILON && !graph->atom) ||
            transition->enter_join > graph->join_count ||
            transition->exit_join > graph->join_count)
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
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    int cover_sequence = kind == LLG_ASSERTION_COVER_SEQUENCE;
    if (cover_sequence) kind = LLG_ASSERTION_COVER;
    if (!g.initialized || g.running || g.config_error || !clock ||
        !valid_sequence_graph(consequent, clock, edge) ||
        (antecedent && !valid_sequence_graph(antecedent, clock, edge)) ||
        (cover_sequence && antecedent) ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition) ||
        (pass_desc && !pass_desc->fn) || (fail_desc && !fail_desc->fn)) {
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
    assertion->pass_desc = pass_desc;
    assertion->fail_desc = fail_desc;
    assertion->abort_condition = abort_condition;
    assertion->data = data;
    assertion->kind = kind;
    assertion->cover_sequence = cover_sequence;
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
    assertion->antecedent_rank = antecedent ? sequence_graph_rank(antecedent) : NULL;
    assertion->consequent_rank = sequence_graph_rank(consequent);
    /* Registration precedes execution. Only sequence consumers need ticks
     * across slots, including explicit clocks in either transition graph. */
    clocking_edge_get(clock)->keep_ticks = 1;
    const llg_sequence_graph_t* graphs[] = {antecedent, consequent};
    for (size_t gi = 0; gi < sizeof(graphs) / sizeof(graphs[0]); ++gi) {
        if (!graphs[gi]) continue;
        for (uint32_t ti = 0; ti < graphs[gi]->transition_count; ++ti) {
            sv4_t* source = graphs[gi]->transitions[ti].clock;
            if (source) clocking_edge_get(source)->keep_ticks = 1;
        }
    }
    if (g.assertion_tail)
        g.assertion_tail->next = assertion;
    else
        g.assertions = assertion;
    g.assertion_tail = assertion;
    return 1;
}

int llg_assertion_single_attempt(uint64_t identity) {
    llg_concurrent_assertion_t* assertion = g.assertion_tail;
    if (!g.initialized || g.running || g.config_error || !assertion ||
        assertion->identity != identity || assertion->kind == LLG_ASSERTION_EXPECT) {
        fprintf(stderr, "llg: invalid single-attempt concurrent assertion\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    assertion->single_attempt = 1;
    return 1;
}

int llg_assertion_gate_clock(uint64_t identity, llg_sampled_gate_fn gate) {
    llg_concurrent_assertion_t* assertion = g.assertion_tail;
    if (!g.initialized || g.running || g.config_error || !gate || !assertion ||
        assertion->identity != identity || assertion->clock_gate) {
        fprintf(stderr, "llg: invalid concurrent assertion clock gate\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    assertion->clock_gate = gate;
    return 1;
}

int llg_assertion_register_sequence(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_sequence_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_desc,
        fail_desc, data, kind, overlapped, 0, 0,
        identity, label, location, scope);
}
