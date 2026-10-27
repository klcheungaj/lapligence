struct llg_property_inst;
static void run_property_assertion(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event, int root_event);
static void property_attempt_begin(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event);
static void property_assertions_async(void);
static void future_assertion_observed(llg_concurrent_assertion_t* assertion);
static int future_assertion_defer_end(llg_concurrent_assertion_t* assertion, int killed);
static void future_assertion_add_start(llg_concurrent_assertion_t* assertion);
static void future_assertion_drop(llg_concurrent_assertion_t* assertion);

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

/* Report one attempt result. The result counters always change; action
 * controls (SV 20.12: "shall not affect the statistics counters") only
 * decide whether the pass, fail or default fail action executes. */
/* Whether a `.matched` slot holds a match visible at destination tick
 * `tick`. */
static int matched_slot_visible(const llg_matched_slot_t* slot, uint64_t tick) {
    return slot->has_match &&
           (slot->visible_tick == tick || (slot->also_next && slot->visible_tick + 1u == tick));
}

/* A match of a sequence endpoint, in the Observed region: `.triggered` and
 * `@(seq)` see the event; each `.matched` slot becomes visible at the next
 * tick of its destination clock, so a destination tick already in this time
 * step does not see it (S39-D5). An earlier match that this step's
 * destination tick shows stays visible for the rest of the step. */
static void sequence_endpoint_matched(llg_concurrent_assertion_t* endpoint) {
    // Observed is not a mutating region for process code; the runtime
    // itself publishes the endpoint, as for clocking-block events.
    event_trigger_object_unchecked(endpoint->endpoint_event->object);
    for (uint32_t index = 0; index < endpoint->endpoint_slot_count; index++) {
        llg_matched_slot_t* slot = &endpoint->endpoint_slots[index];
        uint64_t ticks = assertion_clock_tick(slot->clock, slot->edge);
        if (matched_slot_visible(slot, ticks)) {
            slot->visible_tick = ticks;
            slot->also_next = 1;
        } else {
            slot->visible_tick = ticks + 1u;
            slot->also_next = 0;
        }
        slot->has_match = 1;
    }
}

int llg_sequence_triggered(uint32_t endpoint) {
    if (endpoint >= g.sequence_endpoint_count || !g.sequence_endpoints[endpoint]) {
        fprintf(stderr, "llg runtime fatal: invalid sequence endpoint %u\n", (unsigned)endpoint);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    return llg_event_triggered(g.sequence_endpoints[endpoint]->endpoint_event);
}

int llg_sequence_matched(uint32_t endpoint, uint32_t slot) {
    if (endpoint >= g.sequence_endpoint_count || !g.sequence_endpoints[endpoint] ||
        slot >= g.sequence_endpoints[endpoint]->endpoint_slot_count) {
        fprintf(stderr, "llg runtime fatal: invalid sequence endpoint %u slot %u\n",
                (unsigned)endpoint, (unsigned)slot);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    const llg_matched_slot_t* state = &g.sequence_endpoints[endpoint]->endpoint_slots[slot];
    return matched_slot_visible(state, assertion_clock_tick(state->clock, state->edge));
}

int llg_assertion_endpoint(uint64_t identity, uint32_t index, llg_event_t* event,
                           uint32_t slot_count, sv4_t* const* clocks, const int* edges) {
    llg_concurrent_assertion_t* assertion = g.assertion_tail;
    if (!g.initialized || g.running || g.config_error || !assertion || !event ||
        assertion->identity != identity || assertion->endpoint_event ||
        (slot_count && (!clocks || !edges)) ||
        (assertion->kind != LLG_ASSERTION_COVER) || index != g.sequence_endpoint_count) {
        fprintf(stderr, "llg: invalid sequence endpoint registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t** endpoints = (llg_concurrent_assertion_t**)llg_checked_malloc(
        (size_t)index + 1u, sizeof(*endpoints), "sequence endpoints");
    if (index) memcpy(endpoints, g.sequence_endpoints, (size_t)index * sizeof(*endpoints));
    free(g.sequence_endpoints);
    endpoints[index] = assertion;
    g.sequence_endpoints = endpoints;
    g.sequence_endpoint_count = index + 1u;
    assertion->endpoint_event = event;
    // An end point is no assertion statement: it has no coverage item.
    assertion->acov = NULL;
    if (slot_count) {
        assertion->endpoint_slots = (llg_matched_slot_t*)llg_checked_calloc(
            slot_count, sizeof(*assertion->endpoint_slots), "sequence matched slots");
        assertion->endpoint_slot_count = slot_count;
        for (uint32_t slot = 0; slot < slot_count; slot++) {
            if (!clocks[slot] || (edges[slot] != LLG_EV_POSEDGE && edges[slot] != LLG_EV_NEGEDGE)) {
                fprintf(stderr, "llg: invalid sequence matched clock\n");
                llg_last_failure = 1;
                g.finish = 1;
                return 0;
            }
            assertion->endpoint_slots[slot].clock = clocks[slot];
            assertion->endpoint_slots[slot].edge = edges[slot];
            // Destination ticks are counted across time slots.
            clocking_edge_get(clocks[slot])->keep_ticks = 1;
        }
    }
    return 1;
}

static void assertion_result(llg_concurrent_assertion_t* assertion, int success,
                             int vacuous) {
    int suppressed;
    if (assertion->endpoint_event) {
        if (success && !vacuous) sequence_endpoint_matched(assertion);
        return;
    }
    if (vacuous) assertion_vacuous();
    llg_acov_result(assertion->acov, success, vacuous);
    if (success) {
        // Cover counts and pass actions represent a hit: every success
        // except a cover-vacuous one, whose implication had no antecedent
        // match or whose else-less if had a false condition (decision
        // S38-D10). Assert/assume pass actions still run for their vacuous
        // success, as required by assertion action semantics.
        if (assertion->kind == LLG_ASSERTION_COVER && !vacuous)
            llg_assertion_cover(assertion->identity, assertion->label,
                                assertion->location);
        suppressed = vacuous ? assertion->vacuous_action_off : assertion->pass_action_off;
        if ((assertion->kind != LLG_ASSERTION_COVER || !vacuous) && !suppressed)
            assertion_action(assertion, assertion->pass_desc);
    } else {
        suppressed = assertion->fail_action_off;
        if (assertion->kind == LLG_ASSERTION_COVER) {
            if (!suppressed) assertion_action(assertion, assertion->fail_desc);
        } else {
            assertion_record_failure(assertion->kind);
            if (suppressed) {
                // $assertfailoff also stops the default fail action.
            } else if (assertion->fail_desc) {
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
        // 3: decided, but the caller's inline action arm is suppressed.
        assertion->expect_outcome = suppressed ? 3 : success ? 1 : 2;
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
        llg_acov_attempt(assertion->acov);
        return 1;
    }
    if (assertion->kind != LLG_ASSERTION_EXPECT) {
        llg_acov_attempt(assertion->acov);
        return 1;
    }
    if (!assertion->expect_active || assertion->expect_started) return 0;
    assertion->expect_started = 1;
    return 1;
}

/* The number of evaluation attempts a leading-clock event begins: at most
 * one for a static assertion, and one per matured instance for a
 * procedural one (16.15.6). A matured instance was confirmed for execution,
 * so it begins even if $assertoff came after it was queued (20.11 "may
 * still mature"). */
static uint64_t assertion_root_starts(llg_concurrent_assertion_t* assertion) {
    if (assertion->lag_replaying) {
        // The starts were decided when the event was queued (16.9.4); they
        // count as attempts when the event is evaluated.
        uint64_t starts = assertion->lag_starts;
        assertion->lag_starts = 0;
        for (uint64_t index = 0; index < starts; index++) llg_acov_attempt(assertion->acov);
        return starts;
    }
    if (!assertion->procedural) return (uint64_t)assertion_attempt_starts(assertion);
    uint64_t starts = assertion->procedural_matured;
    assertion->procedural_matured = 0;
    for (uint64_t index = 0; index < starts; index++) llg_acov_attempt(assertion->acov);
    return starts;
}

/* Whether a clock event of `assertion` would begin a new attempt, for the
 * disabled-attempt count of an event that arrives while disable iff holds
 * (40.5.3 "an attempt starts when the disable iff is true"). A procedural
 * assertion counts its matured instances instead
 * (procedural_disabled_starts). */
static int assertion_would_start(const llg_concurrent_assertion_t* assertion) {
    return assertion->acov && assertion->enabled && !assertion->single_attempt &&
           !assertion->procedural && assertion->kind != LLG_ASSERTION_EXPECT;
}

/* A leading clock event of a procedural assertion arrived while disable iff
 * holds: every matured instance begins an attempt that is disabled at once
 * (40.5.3), and so does one that matures later in this time step. */
static void procedural_disabled_starts(llg_concurrent_assertion_t* assertion) {
    if (!assertion->procedural) return;
    for (uint64_t index = 0; index < assertion->procedural_matured; index++)
        llg_acov_disabled_attempt(assertion->acov);
    assertion->procedural_matured = 0;
    assertion->procedural_disabled_tick = g.now + 1;
}

/* Whether the assertion takes part in clock processing: it is enabled, has
 * attempts running, or has matured procedural instances to begin. */
static int assertion_clock_live(const llg_concurrent_assertion_t* assertion) {
    return assertion->enabled || assertion_has_attempts(assertion) ||
           assertion->procedural_matured;
}

/* Discard every running attempt of `assertion` because its disable iff
 * condition holds (`killed` 0) or a control task killed it, counting them
 * for assertion coverage. */
static void end_assertion_attempts(llg_concurrent_assertion_t* assertion, int killed) {
    // An assertion with future global sampled functions discards the
    // attempts running at this point of its deferred queue (16.9.4).
    if (assertion->future_clock && future_assertion_defer_end(assertion, killed)) return;
    if (assertion->acov)
        llg_acov_end_attempts(assertion->acov, assertion_attempts_in_progress(assertion), killed);
    free_assertion_attempts(assertion);
}

static void assertion_disable_signal_changed(sv4_t* signal) {
    if (!signal) return;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (assertion->disable == signal && sv4_to_bool(*signal))
            end_assertion_attempts(assertion, 0);
    }
}

static void assertion_abort_attempts(llg_concurrent_assertion_t* assertion) {
    if (!assertion || !assertion->abort_condition) return;
    free_assertion_clock_events(assertion);
    const int success = assertion->abort_reject ? 0 : 1;
    // An accepted attempt succeeds vacuously (16.15.8 ab); a cover still
    // counts it as a hit (decision S38-D10).
    const int vacuous = success && assertion->kind != LLG_ASSERTION_COVER;
    while (assertion->attempts) {
        llg_assertion_attempt_t* attempt = assertion->attempts;
        assertion->attempts = attempt->next;
        if (!assertion->attempts) assertion->attempts_tail = NULL;
        assertion_result(assertion, success, vacuous);
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
                assertion_result(assertion, success, vacuous);
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

/* Record one more clock edge whose events the assertion consumes besides
 * its leading clock, and number that edge's ticks across time slots. */
static void assertion_add_extra_clock(llg_concurrent_assertion_t* assertion,
                                      sv4_t* signal, int edge) {
    if (!signal || (signal == assertion->clock && edge == assertion->edge)) return;
    for (uint32_t index = 0; index < assertion->extra_clock_count; index++)
        if (assertion->extra_clocks[index].signal == signal &&
            assertion->extra_clocks[index].edge == edge)
            return;
    if ((assertion->extra_clock_count & (assertion->extra_clock_count - 1u)) == 0) {
        size_t capacity = assertion->extra_clock_count ? (size_t)assertion->extra_clock_count * 2u : 1u;
        llg_property_clock_t* clocks = (llg_property_clock_t*)llg_checked_malloc(
            capacity, sizeof(*clocks), "assertion clocks");
        if (assertion->extra_clock_count)
            memcpy(clocks, assertion->extra_clocks,
                   (size_t)assertion->extra_clock_count * sizeof(*clocks));
        free(assertion->extra_clocks);
        assertion->extra_clocks = clocks;
    }
    assertion->extra_clocks[assertion->extra_clock_count++] =
        (llg_property_clock_t){signal, edge};
    clocking_edge_get(signal)->keep_ticks = 1;
}

static void assertion_add_graph_clocks(llg_concurrent_assertion_t* assertion,
                                       const llg_sequence_graph_t* graph) {
    if (!graph) return;
    for (uint32_t index = 0; index < graph->transition_count; index++)
        assertion_add_extra_clock(assertion, graph->transitions[index].clock,
                                  graph->transitions[index].edge);
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
        if (!assertion_clock_live(assertion) ||
            (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active))
            continue;
        if (assertion->disable && sv4_to_bool(*assertion->disable)) {
            if (assertion->clock == signal &&
                ev_matches_changed(old, value, assertion->edge) &&
                (!assertion->clock_gate || assertion->clock_gate(NULL))) {
                if (assertion_would_start(assertion))
                    llg_acov_disabled_attempt(assertion->acov);
                procedural_disabled_starts(assertion);
            }
            continue;
        }
        if (assertion->clock == signal &&
            ev_matches_changed(old, value, assertion->edge)) {
            // A gated-off edge is no clock tick at all, for new attempts and
            // pending threads alike: transitions on this edge are the
            // leading clocking event (lowering rejects other forms).
            if (assertion->clock_gate && !assertion->clock_gate(NULL)) continue;
            if (assertion->consequent_sequence || assertion->property ||
                assertion->future_clock)
                assertion_clock_event_append(assertion, signal, assertion->edge,
                                             order);
            else
                assertion->edge_pending = 1;
            g.assertion_edges_pending = 1;
            continue;
        }
        for (uint32_t index = 0; index < assertion->extra_clock_count; index++) {
            const llg_property_clock_t* extra = &assertion->extra_clocks[index];
            if (extra->signal != signal || !ev_matches(old, value, extra->edge)) continue;
            assertion_clock_event_append(assertion, signal, extra->edge, order);
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

    for (uint64_t starts = root_event ? assertion_root_starts(assertion) : 0; starts;
         starts--) {
        if (!assertion->antecedent_sequence) {
            llg_sequence_attempt_t* attempt = sequence_attempt_new(
                assertion->consequent_sequence, cycle, NULL, NULL);
            sequence_attempt_append(&assertion->sequence_consequents,
                                    &assertion->sequence_consequents_tail, attempt);
            continue;
        }
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
        if (g.finish) return 0;
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

/* Begin the new attempts of a leading-clock event whose accept_on or
 * reject_on condition holds; the control decides each at once. A static
 * assertion reports one such attempt only when no older attempt was
 * pending, matching its per-clock evaluation contract. */
static void abort_new_attempts(llg_concurrent_assertion_t* assertion, int had_pending) {
    uint64_t starts;
    if (assertion->procedural) {
        starts = assertion_root_starts(assertion);
    } else if (assertion->lag_replaying) {
        starts = !had_pending && assertion->lag_starts;
        assertion->lag_starts = 0;
        if (starts) llg_acov_attempt(assertion->acov);
    } else {
        starts = !had_pending && assertion->enabled;
        if (starts) llg_acov_attempt(assertion->acov);
    }
    for (; starts && !g.finish; starts--)
        assertion_result(assertion, assertion->abort_reject ? 0 : 1,
                         assertion->abort_reject ? 0 : 1);
}

/* Begin one attempt of a predicate assertion (no sequence or property
 * program) at the current leading-clock event. */
static void simple_attempt_begin(llg_concurrent_assertion_t* assertion) {
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

/* Remember the leading-clock event a procedural assertion processes in this
 * time step, so an instance maturing after it begins at it. */
static void procedural_note_tick(llg_concurrent_assertion_t* assertion,
                                 const llg_assertion_clock_event_t* event,
                                 uint64_t cycle) {
    if (!assertion->procedural) return;
    assertion->procedural_tick = g.now + 1;
    if (event) {
        assertion->procedural_event = *event;
        assertion->procedural_event.next = NULL;
    }
    assertion->procedural_cycle = cycle;
}

/* One clock event of a sequence assertion after its cycle was taken: a
 * synchronous accept/reject control at a leading-clock event, otherwise a
 * step of every thread. */
static void sequence_assertion_event(llg_concurrent_assertion_t* assertion,
                                     const llg_assertion_clock_event_t* event,
                                     int root_event, uint64_t cycle) {
    if (root_event && assertion->abort_condition &&
        assertion->abort_condition(assertion->data)) {
        int had_pending = assertion->attempts != NULL ||
                          assertion->sequence_antecedents != NULL ||
                          assertion->sequence_consequents != NULL;
        assertion_abort_attempts(assertion);
        // A synchronous accept/reject control also controls the new
        // attempt begun at this sampled leading-clock edge.
        if (!g.finish) abort_new_attempts(assertion, had_pending);
        return;
    }
    (void)run_sequence_concurrent_assertion(assertion, cycle, event->signal, event->edge,
                                            event->time, event->order, event->tick,
                                            root_event);
}

/* One leading-clock edge of a predicate assertion. */
static void simple_assertion_edge(llg_concurrent_assertion_t* assertion) {
    if (assertion->abort_condition && assertion->abort_condition(assertion->data)) {
        int had_pending = assertion->attempts != NULL ||
                          assertion->sequence_antecedents != NULL ||
                          assertion->sequence_consequents != NULL;
        assertion_abort_attempts(assertion);
        // A synchronous accept/reject control also controls the new attempt
        // begun at this sampled edge. Emit one result even when no older
        // attempt was pending, matching the per-clock evaluation contract.
        if (!g.finish) abort_new_attempts(assertion, had_pending);
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

    for (uint64_t starts = assertion_root_starts(assertion); starts && !g.finish; starts--)
        simple_attempt_begin(assertion);
}

static void run_concurrent_assertion(llg_concurrent_assertion_t* assertion) {
    if (assertion->future_clock) {
        future_assertion_observed(assertion);
        return;
    }
    // A clock transition is observed after Active/NBA writes, while every
    // predicate reads the immutable Preponed snapshot from this time slot.
    if (!assertion_clock_live(assertion) ||
        (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
        return;
    }
    if (assertion->disable && sv4_to_bool(*assertion->disable)) {
        // A leading-clock event recorded before disable iff became true
        // starts an attempt that is disabled at once (40.5.3).
        int root_event = assertion->edge_pending;
        for (const llg_assertion_clock_event_t* event = assertion->clock_events;
             event && !root_event; event = event->next)
            root_event = event->signal == assertion->clock && event->edge == assertion->edge;
        assertion->edge_pending = 0;
        end_assertion_attempts(assertion, 0);
        if (root_event && assertion_would_start(assertion))
            llg_acov_disabled_attempt(assertion->acov);
        if (root_event) procedural_disabled_starts(assertion);
        return;
    }
    if (assertion->property) {
        while (assertion->clock_events) {
            llg_assertion_clock_event_t* event = assertion->clock_events;
            assertion->clock_events = event->next;
            if (!assertion->clock_events) assertion->clock_events_tail = NULL;
            int root_event = event->signal == assertion->clock &&
                             event->edge == assertion->edge;
            if (root_event) procedural_note_tick(assertion, event, 0);
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
            if (root_event) procedural_note_tick(assertion, event, cycle);
            sequence_assertion_event(assertion, event, root_event, cycle);
            assertion_clock_event_recycle(event);
            if (g.finish) return;
        }
        return;
    }

    int edge = assertion->edge_pending;
    assertion->edge_pending = 0;
    if (!edge) return;
    procedural_note_tick(assertion, NULL, 0);
    simple_assertion_edge(assertion);
}

/* Whether a leading-clock event of `assertion` waits for this Observed
 * pass (the event list or the predicate engine's pending edge). */
static int assertion_root_event_pending(const llg_concurrent_assertion_t* assertion) {
    if (assertion->edge_pending) return 1;
    for (const llg_assertion_clock_event_t* event = assertion->clock_events; event;
         event = event->next)
        if (event->signal == assertion->clock && event->edge == assertion->edge) return 1;
    return 0;
}

/* Begin one attempt of a procedural assertion at the leading-clock event it
 * already processed in this time step (16.15.6: an instance maturing in a
 * time step of its leading clocking event begins in this Observed region).
 * Older attempts were advanced by that event already, so only the new
 * attempt takes it. */
static void procedural_begin_at_tick(llg_concurrent_assertion_t* assertion) {
    if (assertion->abort_condition && assertion->abort_condition(assertion->data)) {
        assertion->procedural_matured = 1;
        abort_new_attempts(assertion, 0);
        return;
    }
    if (assertion->property) {
        llg_acov_attempt(assertion->acov);
        property_attempt_begin(assertion, &assertion->procedural_event);
        return;
    }
    if (assertion->consequent_sequence) {
        llg_sequence_attempt_t* antecedents = assertion->sequence_antecedents;
        llg_sequence_attempt_t* consequents = assertion->sequence_consequents;
        llg_sequence_attempt_t* antecedents_tail = NULL;
        llg_sequence_attempt_t* consequents_tail = NULL;
        sequence_list_fix_tail(antecedents, &antecedents_tail);
        sequence_list_fix_tail(consequents, &consequents_tail);
        assertion->sequence_antecedents = assertion->sequence_antecedents_tail = NULL;
        assertion->sequence_consequents = assertion->sequence_consequents_tail = NULL;
        assertion->procedural_matured = 1;
        const llg_assertion_clock_event_t* event = &assertion->procedural_event;
        (void)run_sequence_concurrent_assertion(
            assertion, assertion->procedural_cycle, event->signal, event->edge,
            event->time, event->order, event->tick, 1);
        // Older threads keep their place ahead of the new attempt's.
        if (antecedents) {
            sequence_list_fix_tail(assertion->sequence_antecedents,
                                   &assertion->sequence_antecedents_tail);
            antecedents_tail->next = assertion->sequence_antecedents;
            if (!assertion->sequence_antecedents)
                assertion->sequence_antecedents_tail = antecedents_tail;
            assertion->sequence_antecedents = antecedents;
        }
        if (consequents) {
            sequence_list_fix_tail(assertion->sequence_consequents,
                                   &assertion->sequence_consequents_tail);
            consequents_tail->next = assertion->sequence_consequents;
            if (!assertion->sequence_consequents)
                assertion->sequence_consequents_tail = consequents_tail;
            assertion->sequence_consequents = consequents;
        }
        return;
    }
    llg_acov_attempt(assertion->acov);
    simple_attempt_begin(assertion);
}

/* One pending instance of procedural assertion `assertion` matures in the
 * Observed region (16.15.6). In a time step of its leading clocking event
 * it begins an attempt now; otherwise it waits on the matured queue for the
 * next leading clocking event. */
static void procedural_instance_mature(llg_concurrent_assertion_t* assertion) {
    if (assertion_root_event_pending(assertion)) {
        assertion->procedural_matured++;
    } else if (assertion->procedural_disabled_tick == g.now + 1) {
        llg_acov_disabled_attempt(assertion->acov);
    } else if (assertion->procedural_tick == g.now + 1) {
        // A deferred assertion has not evaluated that event yet.
        if (assertion->future_clock) future_assertion_add_start(assertion);
        else procedural_begin_at_tick(assertion);
    } else {
        assertion->procedural_matured++;
    }
}

/* Observed region: every pending procedural instance that no flush point
 * cleared matures, before the clock events of this pass are evaluated. */
static void procedural_assertions_mature(void) {
    for (uint32_t i = 0; g.procedural_pending_live && i < g.deferred_pending.count &&
                         !g.finish;
         i++) {
        llg_deferred_assertion_report_t* report = &g.deferred_pending.items[i];
        if (!report->live || report->kind != LLG_DEFERRED_PROCEDURAL) continue;
        llg_concurrent_assertion_t* assertion = report->procedural;
        deferred_report_drop(report);
        procedural_instance_mature(assertion);
    }
}

static int run_concurrent_assertions(void) {
    procedural_assertions_mature();
    if (g.finish) return 0;
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
         assertion = assertion->next) {
        // Queued events after the last global clock tick are never
        // evaluated (16.9.4, F.6.3).
        if (assertion->future_clock) future_assertion_drop(assertion);
        free_assertion_attempts(assertion);
    }
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
    assertion->acov =
        assertion->kind == LLG_ASSERTION_EXPECT ? NULL : llg_acov_find(identity);
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
    assertion->acov =
        assertion->kind == LLG_ASSERTION_EXPECT ? NULL : llg_acov_find(identity);
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
    assertion_add_graph_clocks(assertion, antecedent);
    assertion_add_graph_clocks(assertion, consequent);
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

int llg_assertion_procedural(uint64_t identity, uint32_t index) {
    llg_concurrent_assertion_t* assertion = g.assertion_tail;
    if (!g.initialized || g.running || g.config_error || !assertion ||
        assertion->identity != identity || assertion->kind == LLG_ASSERTION_EXPECT ||
        assertion->endpoint_event || assertion->single_attempt || assertion->procedural ||
        index != g.procedural_assertion_count || index == UINT32_MAX) {
        fprintf(stderr, "llg: invalid procedural concurrent assertion\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    // Registration indices are dense, so a power-of-two capacity is the
    // count rounded up.
    if ((index & (index - 1u)) == 0) {
        size_t capacity = index ? (size_t)index * 2u : 1u;
        llg_concurrent_assertion_t** table = (llg_concurrent_assertion_t**)llg_checked_malloc(
            capacity, sizeof(*table), "procedural assertions");
        if (index) memcpy(table, g.procedural_assertions, (size_t)index * sizeof(*table));
        free(g.procedural_assertions);
        g.procedural_assertions = table;
    }
    g.procedural_assertions[index] = assertion;
    g.procedural_assertion_count = index + 1u;
    assertion->procedural = 1;
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

void llg_assertion_clock_pulse(sv4_t* signal) {
    if (!signal) return;
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (edge && edge->time == g.now && (edge->occurred & CLOCKING_POSEDGE)) return;
    sv4_t one = sv4_from_u64(1, 1, 0);
    sv4_t zero = sv4_from_u64(0, 1, 0);
    sig_write(signal, one);
    sig_write(signal, zero);
    sv4_destroy(&one);
    sv4_destroy(&zero);
}
