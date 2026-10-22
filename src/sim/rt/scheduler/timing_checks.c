// ── System timing checks (IEEE 1364-2001 15, IEEE 1800-2009 31) ─────────────
//
// One generated hidden process per instance calls llg_timing_check_step when
// a terminal changes. A step derives each terminal's transitions since the
// last step, records the timestamp events and evaluates the timecheck events
// of every check, then the process toggles the notifiers of the violated
// checks with ordinary variable writes. Times are design-precision ticks;
// a stamp of 0 means no event because events at time 0 never stamp.

static int timing_check_messages = 1;

void llg_timing_checks_configure(int messages) { timing_check_messages = messages != 0; }

// 0, 1 or 2: z counts as x (31.5).
static uint8_t timing_state(const sv4_t* value, uint64_t bit) {
    unsigned state = llg_sv4_state(*value, bit);
    return (uint8_t)(state > 1u ? 2u : state);
}

void llg_timing_checks_start(llg_timing_check_set_t* set) {
    if (!set || set->offsets) return;
    uint32_t terminals = set->n_terminals ? set->n_terminals : 1u;
    set->offsets = llg_checked_calloc(terminals, sizeof(*set->offsets),
                                      "timing check terminals");
    set->events = llg_checked_calloc(terminals, sizeof(*set->events),
                                     "timing check terminals");
    uint64_t total = 0;
    for (uint32_t index = 0; index < set->n_terminals; index++) {
        if (total > UINT32_MAX) {
            fprintf(stderr, "llg: fatal: timing check terminals are too wide\n");
            abort();
        }
        set->offsets[index] = (uint32_t)total;
        total += set->terminals[index].n_bits;
    }
    set->last = llg_checked_calloc(total ? (size_t)total : 1u, sizeof(*set->last),
                                   "timing check terminals");
    for (uint32_t index = 0; index < set->n_terminals; index++) {
        const llg_timing_terminal_t* terminal = &set->terminals[index];
        uint8_t* last = set->last + set->offsets[index];
        for (uint32_t bit = 0; bit < terminal->n_bits; bit++)
            last[bit] = timing_state(terminal->value, (uint64_t)terminal->lsb + bit);
    }
    if (set->n_conditions) {
        set->condition_values = llg_checked_calloc(
            set->n_conditions, sizeof(*set->condition_values), "timing check conditions");
        for (uint32_t index = 0; index < set->n_conditions; index++)
            set->condition_values[index] = (sv4_t)SV4_EMPTY;
    }
    set->notify = llg_checked_calloc(set->n_notifiers ? set->n_notifiers : 1u,
                                     sizeof(*set->notify), "timing check notifiers");
    for (uint32_t index = 0; index < set->n_checks; index++) {
        llg_timing_check_t* check = &set->checks[index];
        check->reference_time = 0;
        check->data_time = 0;
        check->trail_time = 0;
        check->open = 0;
    }
    set->next = g.timing_check_sets;
    g.timing_check_sets = set;
}

llg_timing_check_set_t* llg_timing_check_sets(void) { return g.timing_check_sets; }

int llg_timing_notify(const llg_timing_check_set_t* set, uint32_t notifier) {
    return set && set->notify && notifier < set->n_notifiers && set->notify[notifier];
}

// The transition bit of one bit's change, or 0 for no transition (x<->z,
// which timing_state has already folded, counts as none).
static uint8_t timing_transition(uint8_t from, uint8_t to) {
    static const uint8_t table[3][3] = {
        {0u, LLG_TIMING_EDGE_01, LLG_TIMING_EDGE_0X},
        {LLG_TIMING_EDGE_10, 0u, LLG_TIMING_EDGE_1X},
        {LLG_TIMING_EDGE_X0, LLG_TIMING_EDGE_X1, 0u},
    };
    return table[from][to];
}

typedef struct {
    llg_timing_check_set_t* set;
    int evaluated;
} timing_step_t;

// An event occurs when its terminal made one of its transitions on any bit
// (any transition without edge control) while its condition holds (31.7):
// the condition's least significant bit is 1, or x or z for `==` and `!=`.
static int timing_condition(timing_step_t* step, uint32_t condition,
                            uint8_t nondeterministic) {
    if (!condition) return 1;
    llg_timing_check_set_t* set = step->set;
    if (!step->evaluated) {
        set->conditions(set->condition_values, NULL);
        step->evaluated = 1;
    }
    const sv4_t* value = &set->condition_values[condition - 1u];
    if (llg_sv4_width(*value) == 0) return 0;
    unsigned state = llg_sv4_state(*value, 0);
    return state == 1u || (state > 1u && nondeterministic);
}

static int timing_edge(const timing_step_t* step, const llg_timing_event_t* event) {
    uint8_t events = step->set->events[event->terminal];
    return event->edges ? (events & event->edges) != 0 : events != 0;
}

// The transitions opposite to `edges`: 0 and 1 swapped (01 <-> 10,
// 0x <-> 1x, x0 <-> x1).
static uint8_t timing_opposite(uint8_t edges) {
    uint8_t opposite = 0;
    if (edges & LLG_TIMING_EDGE_01) opposite |= LLG_TIMING_EDGE_10;
    if (edges & LLG_TIMING_EDGE_10) opposite |= LLG_TIMING_EDGE_01;
    if (edges & LLG_TIMING_EDGE_0X) opposite |= LLG_TIMING_EDGE_1X;
    if (edges & LLG_TIMING_EDGE_1X) opposite |= LLG_TIMING_EDGE_0X;
    if (edges & LLG_TIMING_EDGE_X0) opposite |= LLG_TIMING_EDGE_X1;
    if (edges & LLG_TIMING_EDGE_X1) opposite |= LLG_TIMING_EDGE_X0;
    return opposite;
}

static int timing_event(timing_step_t* step, const llg_timing_event_t* event) {
    return timing_edge(step, event) &&
           timing_condition(step, event->condition, event->nondeterministic);
}

static void timing_unit(char* out, size_t size) {
    static const struct {
        uint64_t fs;
        const char* name;
    } units[] = {{UINT64_C(1000000000000000), "s"}, {UINT64_C(1000000000000), "ms"},
                 {UINT64_C(1000000000), "us"},      {UINT64_C(1000000), "ns"},
                 {UINT64_C(1000), "ps"},            {UINT64_C(1), "fs"}};
    uint64_t fs = g.design_precision_fs ? g.design_precision_fs : 1u;
    for (size_t index = 0; index < sizeof(units) / sizeof(units[0]); index++) {
        if (fs >= units[index].fs && fs % units[index].fs == 0) {
            snprintf(out, size, "%llu%s",
                     (unsigned long long)(fs / units[index].fs), units[index].name);
            return;
        }
    }
    snprintf(out, size, "%llufs", (unsigned long long)fs);
}

// One violation: a report on stderr (unless turned off) and the notifier.
static void timing_violation(llg_timing_check_set_t* set,
                             const llg_timing_check_t* check,
                             uint64_t reference, uint64_t data) {
    if (check->notifier && check->notifier <= set->n_notifiers)
        set->notify[check->notifier - 1u] = 1u;
    if (!timing_check_messages) return;
    char unit[32];
    timing_unit(unit, sizeof(unit));
    llg_report_t report;
    llg_report_init(&report);
    llg_report_printf(&report, "llg: warning: timing violation at %s in %s at time %llu (%s): ",
                      check->origin && check->origin[0] ? check->origin : "<unknown>",
                      set->scope ? set->scope : "<unknown>",
                      (unsigned long long)g.now, unit);
    for (const char* text = check->text ? check->text : ""; *text; text++) {
        if (*text == '\001' || *text == '\002')
            llg_report_printf(&report, "%llu",
                              (unsigned long long)(*text == '\001' ? reference : data));
        else
            llg_report_bytes(&report, text, 1);
    }
    llg_report_bytes(&report, "\n", 1);
    llg_report_emit(&report, stderr);
}

static int timing_within(uint64_t stamp, uint64_t now, uint64_t limit) {
    return stamp && now - stamp < limit;
}

static void timing_check_run(timing_step_t* step, llg_timing_check_t* check) {
    llg_timing_check_set_t* set = step->set;
    uint64_t now = g.now;
    switch (check->kind) {
        case LLG_TIMING_SETUP:
        case LLG_TIMING_REMOVAL: {
            // Timestamp: data event; timecheck: reference event. Violation
            // when (timecheck - limit) < timestamp < timecheck (31.3.1,
            // 31.3.4). A data event in the same step stamps first, so it
            // never violates.
            if (timing_event(step, &check->data)) check->data_time = now;
            if (timing_event(step, &check->reference) && check->data_time < now &&
                timing_within(check->data_time, now, check->limit))
                timing_violation(set, check, now, check->data_time);
            break;
        }
        case LLG_TIMING_HOLD:
        case LLG_TIMING_RECOVERY: {
            // Timestamp: reference event; timecheck: data event. Violation
            // when timestamp <= timecheck < timestamp + limit (31.3.2,
            // 31.3.5), so a data event earlier in the reference event's time
            // slot violates when the reference event arrives.
            int reference = timing_event(step, &check->reference);
            int data = timing_event(step, &check->data);
            if (reference) {
                check->reference_time = now;
                if (!data && check->data_time == now && check->limit)
                    timing_violation(set, check, now, now);
            }
            if (data) {
                check->data_time = now;
                if (timing_within(check->reference_time, now, check->limit))
                    timing_violation(set, check, check->reference_time, now);
            }
            break;
        }
        case LLG_TIMING_SKEW: {
            // Violation when timecheck - timestamp > limit (31.4.1); every
            // later data event beyond the limit violates again.
            if (timing_event(step, &check->reference)) check->reference_time = now;
            if (timing_event(step, &check->data) && check->reference_time &&
                now - check->reference_time > check->limit)
                timing_violation(set, check, check->reference_time, now);
            break;
        }
        case LLG_TIMING_PERIOD: {
            // Each edge checks the previous one, then restamps (31.4.5).
            if (timing_event(step, &check->reference)) {
                if (timing_within(check->reference_time, now, check->limit))
                    timing_violation(set, check, check->reference_time, now);
                check->reference_time = now;
            }
            break;
        }
        case LLG_TIMING_WIDTH: {
            // Violation when threshold < width < limit (31.4.4); the stamp
            // stays, so consecutive opposite edges are all checked.
            if (timing_event(step, &check->reference)) check->reference_time = now;
            if (timing_event(step, &check->data) && check->reference_time) {
                uint64_t width = now - check->reference_time;
                if (width > check->limit2 && width < check->limit)
                    timing_violation(set, check, check->reference_time, now);
            }
            break;
        }
        case LLG_TIMING_NOCHANGE: {
            // Window (leading edge - start offset, trailing edge + end
            // offset), exclusive (31.4.6). The leading and trailing edges
            // always open and close it; the reference and data conditions
            // are evaluated at the data event. Edges are handled before the
            // step's data event, so a data change in the same slot as an
            // edge is outside a window with zero offsets.
            llg_timing_event_t trailing = check->reference;
            trailing.edges = timing_opposite(check->reference.edges);
            if (timing_edge(step, &check->reference) && !check->open) {
                check->open = 1u;
                check->reference_time = now;
                check->trail_time = 0;
                if (timing_within(check->data_time, now, check->limit))
                    timing_violation(set, check, now, check->data_time);
            } else if (timing_edge(step, &trailing) && check->open) {
                check->open = 0;
                check->trail_time = now;
            }
            if (!timing_edge(step, &check->data) ||
                !timing_condition(step, check->reference.condition,
                                  check->reference.nondeterministic) ||
                !timing_condition(step, check->data.condition,
                                  check->data.nondeterministic))
                break;
            check->data_time = now;
            if (check->open) {
                if (now > check->reference_time || check->limit)
                    timing_violation(set, check, check->reference_time, now);
            } else if (check->trail_time && now - check->trail_time < check->limit2) {
                timing_violation(set, check, check->reference_time, now);
            }
            break;
        }
        default:
            break;
    }
}

void llg_timing_check_step(llg_timing_check_set_t* set) {
    if (!set || !set->offsets) return;
    if (set->n_notifiers) memset(set->notify, 0, set->n_notifiers);
    int changed = 0;
    for (uint32_t index = 0; index < set->n_terminals; index++) {
        const llg_timing_terminal_t* terminal = &set->terminals[index];
        uint8_t* last = set->last + set->offsets[index];
        uint8_t events = 0;
        for (uint32_t bit = 0; bit < terminal->n_bits; bit++) {
            uint8_t state = timing_state(terminal->value, (uint64_t)terminal->lsb + bit);
            events |= timing_transition(last[bit], state);
            last[bit] = state;
        }
        set->events[index] = events;
        changed |= events != 0;
    }
    // Events at time 0 never stamp or check (the initial values settle).
    if (!changed || g.now == 0) return;
    timing_step_t step = {set, 0};
    for (uint32_t index = 0; index < set->n_checks; index++)
        timing_check_run(&step, &set->checks[index]);
}

// Release every started set; the generated model owns the tables.
static void timing_checks_release(void) {
    while (g.timing_check_sets) {
        llg_timing_check_set_t* set = g.timing_check_sets;
        g.timing_check_sets = set->next;
        set->next = NULL;
        if (set->condition_values) {
            for (uint32_t index = 0; index < set->n_conditions; index++)
                sv4_destroy(&set->condition_values[index]);
        }
        free(set->condition_values);
        free(set->last);
        free(set->offsets);
        free(set->events);
        free(set->notify);
        set->condition_values = NULL;
        set->last = NULL;
        set->offsets = NULL;
        set->events = NULL;
        set->notify = NULL;
    }
    timing_check_messages = 1;
}
