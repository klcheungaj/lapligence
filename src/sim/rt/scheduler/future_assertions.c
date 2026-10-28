/* Global clocking future sampled value functions (IEEE 1800-2009 16.9.4).
 *
 * `$future_gclk(v)` is the sampled value of `v` at the next global clock tick,
 * yet the attempt's interval is defined "as though the future sampled values
 * were known in advance", and its action block runs at the global clock tick
 * after its last assertion clock tick. An assertion that calls these
 * functions therefore evaluates each of its clock events one global clock
 * tick late, with the exact operands of the event's own time step:
 *
 * - Each Observed pass with clock events takes them into one queue entry,
 *   with the operands the generated `capture` callback stores (every
 *   subexpression that reads sampled values, evaluated as the predicates
 *   would evaluate it now), the attempts a leading-clock event begins, and
 *   the step's clock history for multiclock alignment.
 * - In the Observed region of the first global clock tick strictly after an
 *   entry's time step, the entry's events run through the ordinary engines.
 *   Predicates read captured operands through llg_assertion_captured; the
 *   future function arguments read the current sampled values, which are
 *   those of that global clock tick. Results and actions happen then.
 * - Disable iff and $assertkill act on an attempt only within its interval
 *   (16.9.4): while entries are queued they append a marker, so they discard
 *   exactly the attempts running at their own point of the queue, and an
 *   attempt whose last tick precedes them still reports. The action controls
 *   (20.12), which 16.9.4 names with them, apply as they stood at the events
 *   that decide a result, so an entry keeps them.
 * - An entry after the last global clock tick needs a value that does not
 *   exist (F.6.3) and is dropped at the end of the run; the attempts already
 *   evaluated settle by the ordinary end-of-simulation rule.
 *
 * Entries and their operand arrays are recycled per assertion and count
 * against LLG_SEQUENCE_THREAD_LIMIT, so a global clock that stops ticking
 * cannot grow the queue silently. Assertions without these functions never
 * reach this file beyond one branch per Observed pass. */

static void future_event_list_recycle(llg_assertion_clock_event_t* event) {
    while (event) {
        llg_assertion_clock_event_t* next = event->next;
        assertion_clock_event_recycle(event);
        event = next;
    }
}

static llg_assertion_clock_event_t* future_event_list_clone(
    const llg_assertion_clock_event_t* event) {
    llg_assertion_clock_event_t* head = NULL;
    llg_assertion_clock_event_t** link = &head;
    for (; event; event = event->next) {
        llg_assertion_clock_event_t* copy = assertion_clock_event_alloc();
        *copy = *event;
        copy->next = NULL;
        *link = copy;
        link = &copy->next;
    }
    return head;
}

/* A queue entry for `assertion`, counted against the thread budget. */
static llg_assertion_lag_t* future_lag_alloc(llg_concurrent_assertion_t* assertion) {
    llg_assertion_lag_t* entry = assertion->lag_free;
    if (entry) {
        assertion->lag_free = entry->next;
    } else {
        entry = (llg_assertion_lag_t*)llg_checked_calloc(1, sizeof(*entry),
                                                         "deferred assertion evaluation");
    }
    sv4_t* slots = entry->slots;
    memset(entry, 0, sizeof(*entry));
    entry->slots = slots;
    entry->time = g.now;
    g.sequence_current = assertion;
    sequence_thread_acquire();
    return entry;
}

static void future_lag_recycle(llg_concurrent_assertion_t* assertion, llg_assertion_lag_t* entry) {
    future_event_list_recycle(entry->events);
    future_event_list_recycle(entry->history);
    entry->events = NULL;
    entry->history = NULL;
    entry->next = assertion->lag_free;
    assertion->lag_free = entry;
    sequence_thread_release();
}

static void future_lag_append(llg_concurrent_assertion_t* assertion, llg_assertion_lag_t* entry) {
    if (assertion->lag_tail) assertion->lag_tail->next = entry;
    else assertion->lag_head = entry;
    assertion->lag_tail = entry;
}

/* Drop every queued entry (end of the run, cleanup). */
static void future_assertion_drop(llg_concurrent_assertion_t* assertion) {
    while (assertion->lag_head) {
        llg_assertion_lag_t* entry = assertion->lag_head;
        assertion->lag_head = entry->next;
        future_lag_recycle(assertion, entry);
    }
    assertion->lag_tail = NULL;
}

/* Release the queue and the recycled entries with their operand owners. */
static void future_assertion_free(llg_concurrent_assertion_t* assertion) {
    future_assertion_drop(assertion);
    while (assertion->lag_free) {
        llg_assertion_lag_t* entry = assertion->lag_free;
        assertion->lag_free = entry->next;
        if (entry->slots) {
            sv4_destroy_array(entry->slots, assertion->future_slots);
            free(entry->slots);
        }
        free(entry);
    }
}

/* Called by end_assertion_attempts: with entries queued, the discard is a
 * marker at the end of the queue (1); otherwise it happens now (0). The
 * clock events of the current step not yet taken are discarded either way,
 * as for an assertion evaluated without delay. */
static int future_assertion_defer_end(llg_concurrent_assertion_t* assertion, int killed) {
    if (!assertion->lag_head || assertion->lag_replaying) return 0;
    free_assertion_clock_events(assertion);
    // Consecutive markers discard the same attempts: the first one counts
    // them as killed or disabled.
    if (assertion->lag_tail->marker && !assertion->lag_tail->kill_one) return 1;
    llg_assertion_lag_t* entry = future_lag_alloc(assertion);
    entry->marker = 1;
    entry->killed = killed ? 1 : 0;
    future_lag_append(assertion, entry);
    return 1;
}

/* vpiAssertionKill of the attempts that began at `start` (39.5.2). With
 * entries queued the kill is a marker at the end of the queue, as for
 * $assertkill; otherwise it happens now. */
static void future_assertion_kill_at(llg_concurrent_assertion_t* assertion, uint64_t start) {
    if (!assertion->lag_head || assertion->lag_replaying) {
        assertion_kill_attempts_at(assertion, start);
        return;
    }
    llg_assertion_lag_t* entry = future_lag_alloc(assertion);
    entry->marker = 1;
    entry->killed = 1;
    entry->kill_one = 1;
    entry->kill_start = start;
    future_lag_append(assertion, entry);
}

/* An attempt that began now while disable iff holds: its disabled result
 * is queued, so its callbacks run at the next global clock tick with the
 * results before it (39.4.2.1, A17-D7). The starts of one time step share
 * an entry. */
static void future_assertion_disabled_start(llg_concurrent_assertion_t* assertion) {
    if (!assertion->acov) return;
    llg_assertion_lag_t* tail = assertion->lag_tail;
    if (tail && tail->disabled_starts && tail->time == g.now) {
        tail->disabled_starts++;
        return;
    }
    llg_assertion_lag_t* entry = future_lag_alloc(assertion);
    entry->disabled_starts = 1;
    future_lag_append(assertion, entry);
}

/* The attempts a queued leading-clock event begins, decided in its own time
 * step: assertion controls and procedural maturity are real-time state.
 * Assertion coverage counts them when they are evaluated
 * (assertion_root_starts), as 39.4.2.1 delivers their callbacks then. */
static uint64_t future_root_starts(llg_concurrent_assertion_t* assertion) {
    if (assertion->procedural) {
        uint64_t starts = assertion->procedural_matured;
        assertion->procedural_matured = 0;
        return starts;
    }
    if (!assertion->enabled) return 0;
    if (assertion->single_attempt) {
        if (assertion->single_attempt_started) return 0;
        assertion->single_attempt_started = 1;
    }
    return 1;
}

/* A procedural instance matured after this step's leading-clock event was
 * queued: it begins at that event (16.15.6), so the entry takes it. */
static void future_assertion_add_start(llg_concurrent_assertion_t* assertion) {
    llg_assertion_lag_t* found = NULL;
    for (llg_assertion_lag_t* entry = assertion->lag_head; entry; entry = entry->next)
        if (!entry->marker && entry->has_root && entry->time == g.now) found = entry;
    if (found) found->starts++;
    else assertion->procedural_matured++;
}

/* Evaluate one queued clock event through the assertion's engine. */
static void future_event_step(llg_concurrent_assertion_t* assertion,
                              const llg_assertion_clock_event_t* event, int root_event) {
    if (assertion->property) {
        run_property_assertion(assertion, event, root_event);
        return;
    }
    if (assertion->consequent_sequence) {
        uint64_t cycle = 0;
        if (!sequence_cycle_next(assertion, &cycle)) return;
        sequence_assertion_event(assertion, event, root_event, cycle);
        return;
    }
    if (root_event) simple_assertion_edge(assertion);
}

/* Evaluate the entries whose time step precedes this global clock tick. The
 * current step's clock events and history stay untouched for the capture
 * that follows. */
static void future_assertion_replay(llg_concurrent_assertion_t* assertion) {
    llg_assertion_clock_event_t* events = assertion->clock_events;
    llg_assertion_clock_event_t* events_tail = assertion->clock_events_tail;
    llg_assertion_clock_event_t* history = assertion->clock_history;
    llg_assertion_clock_event_t* history_tail = assertion->clock_history_tail;
    assertion->clock_events = assertion->clock_events_tail = NULL;
    while (assertion->lag_head && assertion->lag_head->time < g.now && !g.finish) {
        llg_assertion_lag_t* entry = assertion->lag_head;
        assertion->lag_head = entry->next;
        if (!assertion->lag_head) assertion->lag_tail = NULL;
        assertion->lag_replaying = 1;
        // Attempt events belong to the entry's time step (39.4.2.1).
        g.assertion_replay_time1 = entry->time + 1u;
        if (entry->disabled_starts) {
            for (uint64_t index = 0; index < entry->disabled_starts && !g.finish; index++)
                llg_acov_disabled_attempt(assertion->acov, entry->time);
        } else if (entry->marker) {
            assertion->clock_history = assertion->clock_history_tail = NULL;
            if (entry->kill_one) assertion_kill_attempts_at(assertion, entry->kill_start);
            else end_assertion_attempts(assertion, entry->killed);
        } else {
            assertion->clock_history = entry->history;
            assertion->clock_history_tail = NULL;
            for (llg_assertion_clock_event_t* item = entry->history; item; item = item->next)
                assertion->clock_history_tail = item;
            g.assertion_lag_slots = entry->slots;
            g.assertion_lag_slot_count = assertion->future_slots;
            assertion->lag_starts = entry->starts;
            uint8_t pass_off = assertion->pass_action_off;
            uint8_t vacuous_off = assertion->vacuous_action_off;
            uint8_t fail_off = assertion->fail_action_off;
            assertion->pass_action_off = entry->pass_action_off;
            assertion->vacuous_action_off = entry->vacuous_action_off;
            assertion->fail_action_off = entry->fail_action_off;
            for (const llg_assertion_clock_event_t* event = entry->events; event && !g.finish;
                 event = event->next) {
                int root_event = event->signal == assertion->clock && event->edge == assertion->edge;
                future_event_step(assertion, event, root_event);
            }
            assertion->pass_action_off = pass_off;
            assertion->vacuous_action_off = vacuous_off;
            assertion->fail_action_off = fail_off;
            g.assertion_lag_slots = NULL;
            g.assertion_lag_slot_count = 0;
            assertion->lag_starts = 0;
        }
        assertion->lag_replaying = 0;
        g.assertion_replay_time1 = 0;
        // The history list belongs to the entry; detach before recycling.
        assertion->clock_history = assertion->clock_history_tail = NULL;
        future_lag_recycle(assertion, entry);
    }
    assertion->clock_events = events;
    assertion->clock_events_tail = events_tail;
    assertion->clock_history = history;
    assertion->clock_history_tail = history_tail;
}

/* The Observed pass of an assertion with future global sampled functions:
 * evaluate what this global clock tick completes, then queue this pass's
 * clock events with their operands. */
static void future_assertion_observed(llg_concurrent_assertion_t* assertion) {
    if (assertion->lag_head && assertion->future_clock->tick_time1 == g.now + 1) {
        future_assertion_replay(assertion);
        if (g.finish) return;
    }
    if (!assertion_clock_live(assertion)) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
        return;
    }
    if (assertion->disable && sv4_to_bool(*assertion->disable)) {
        // As without delay: a leading-clock event recorded before disable
        // iff became true starts an attempt that is disabled at once
        // (40.5.3), and the running attempts end at this point.
        int root_event = 0;
        for (const llg_assertion_clock_event_t* event = assertion->clock_events;
             event && !root_event; event = event->next)
            root_event = event->signal == assertion->clock && event->edge == assertion->edge;
        end_assertion_attempts(assertion, 0);
        free_assertion_clock_events(assertion);
        if (root_event && assertion_would_start(assertion)) assertion_disabled_start(assertion);
        if (root_event) procedural_disabled_starts(assertion);
        return;
    }
    if (!assertion->clock_events) return;
    llg_assertion_lag_t* entry = future_lag_alloc(assertion);
    entry->events = assertion->clock_events;
    assertion->clock_events = assertion->clock_events_tail = NULL;
    for (const llg_assertion_clock_event_t* event = entry->events; event; event = event->next) {
        if (event->signal != assertion->clock || event->edge != assertion->edge) continue;
        procedural_note_tick(assertion, event, 0);
        entry->has_root = 1;
        entry->starts += future_root_starts(assertion);
    }
    entry->pass_action_off = assertion->pass_action_off;
    entry->vacuous_action_off = assertion->vacuous_action_off;
    entry->fail_action_off = assertion->fail_action_off;
    if (assertion->extra_clock_count)
        entry->history = future_event_list_clone(assertion->clock_history);
    if (assertion->future_slots) {
        if (!entry->slots) {
            entry->slots = (sv4_t*)llg_checked_malloc(assertion->future_slots, sizeof(sv4_t),
                                                      "deferred assertion operands");
            const sv4_t empty = SV4_EMPTY;
            for (uint32_t index = 0; index < assertion->future_slots; index++)
                entry->slots[index] = empty;
        }
        assertion->future_capture(assertion->data, entry->slots);
    }
    future_lag_append(assertion, entry);
}

int llg_assertion_future(uint64_t identity, uint64_t global_clock, uint32_t slots,
                         llg_assertion_capture_fn capture) {
    llg_concurrent_assertion_t* assertion = g.assertion_tail;
    llg_sampled_clock_t* clock = find_sampled_clock(global_clock);
    // Asynchronous aborts act between assertion clock ticks and expect
    // blocks its caller; the generated model rejects both before this.
    if (!g.initialized || g.running || g.config_error || !assertion || !clock ||
        assertion->identity != identity || assertion->future_clock ||
        assertion->kind == LLG_ASSERTION_EXPECT || assertion->endpoint_event ||
        (assertion->abort_condition && !assertion->abort_sync) || assertion->property_async ||
        (slots && !capture)) {
        fprintf(stderr, "llg: invalid future global sampled-value assertion\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    assertion->future_clock = clock;
    assertion->future_capture = capture;
    assertion->future_slots = slots;
    return 1;
}

static const sv4_t* future_captured_slot(uint32_t slot) {
    if (!g.assertion_lag_slots || slot >= g.assertion_lag_slot_count) {
        fprintf(stderr, "llg runtime fatal: captured assertion operand %u read outside its"
                        " deferred evaluation\n",
                (unsigned)slot);
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    return &g.assertion_lag_slots[slot];
}

sv4_t llg_assertion_captured(uint32_t slot) {
    const sv4_t* value = future_captured_slot(slot);
    return value ? sv4_clone(value) : sv4_x(1, 0);
}

void llg_assertion_captured_to(sv4_t* dst, uint32_t slot) {
    const sv4_t* value = future_captured_slot(slot);
    if (value) sv4_copy(dst, value);
    else sv4_replace(dst, sv4_x(1, 0));
}

int llg_assertion_future_status(int kind, uint32_t slot, const sv4_t* future) {
    const sv4_t* current = future_captured_slot(slot);
    if (!current || !future) return 0;
    // Annex F.3.4.4: $rising_gclk(e) is `b !== 1 && $future_gclk(b) === 1`
    // for the LSB b of e, $steady_gclk(e) is `e === $future_gclk(e)`.
    switch (kind) {
        case 0: return !sampled_domain_lsb_one(*current) && sampled_domain_lsb_one(*future);
        case 1: return !sampled_domain_lsb_zero(*current) && sampled_domain_lsb_zero(*future);
        case 2: return sv4_same(*current, *future);
        case 3: return !sv4_same(*current, *future);
        default:
            fprintf(stderr, "llg runtime fatal: invalid future sampled-value status %d\n", kind);
            llg_last_failure = 1;
            g.finish = 1;
            return 0;
    }
}
