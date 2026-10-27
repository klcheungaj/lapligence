// ── System timing checks (IEEE 1364-2001 15, IEEE 1800-2009 31) ─────────────
//
// Every terminal of a started set is watched on its storage row: a change
// only marks the terminal and queues its set (timing_terminals_changed). The
// first queued set rings a doorbell variable that wakes the model's one
// generated timing process, which calls llg_timing_checks_run in the Active
// region of the same time step, after the statements that caused the change.
// A step derives each changed terminal's transitions, records the timestamp
// events and evaluates the timecheck events of the checks that read a
// changed terminal, schedules the delayed signals fed by it, then toggles
// the notifiers of the violated checks with ordinary variable writes.
// Delayed signals (31.9) are transport delay lines written from an
// Active-region callback at their due time; their storage is itself a
// watched terminal. Timer-based `$timeskew`/`$fullskew` windows expire from a
// Pre-Postponed callback, after every event of the expiry time step. Times
// are design-precision ticks; a stamp of 0 means no event because events at
// time 0 never stamp (A9-D3).

static int timing_check_messages = 1;
static llg_timing_violation_fn timing_violation_hook;

void llg_timing_violation_hook(llg_timing_violation_fn hook) { timing_violation_hook = hook; }

// A heap entry: due time, issue order for equal times, and the item.
typedef struct {
    uint64_t time;
    uint64_t sequence;
    void* item;
    uint32_t index;
} timing_heap_entry_t;

typedef struct {
    timing_heap_entry_t* entries;
    size_t n;
    size_t capacity;
    uint64_t sequence;
    // The time of the scheduled region callback; UINT64_MAX when none.
    uint64_t scheduled;
    llg_region_callback_id_t id;
} timing_heap_t;

#define TIMING_HEAP_EMPTY {NULL, 0, 0, 0, UINT64_MAX, {0, 0}}

static struct {
    sv4_t* doorbell;
    int rung;
    int running;
    llg_timing_check_set_t* dirty_head;
    llg_timing_check_set_t* dirty_tail;
    timing_heap_t delays;  // delayed signal transitions (line; Active)
    timing_heap_t timers;  // skew window expiries (set, check; Pre-Postponed)
} timing = {NULL, 0, 0, NULL, NULL, TIMING_HEAP_EMPTY, TIMING_HEAP_EMPTY};

void llg_timing_checks_configure(int messages) { timing_check_messages = messages != 0; }

void llg_timing_checks_doorbell(sv4_t* doorbell) { timing.doorbell = doorbell; }

// 0, 1 or 2: z counts as x (31.5).
static uint8_t timing_state(const sv4_t* value, uint64_t bit) {
    unsigned state = llg_sv4_state(*value, bit);
    return (uint8_t)(state > 1u ? 2u : state);
}

static void* timing_grow(void* items, size_t* capacity, size_t size, const char* what) {
    size_t grown = *capacity ? *capacity * 2u : 8u;
    if (grown > SIZE_MAX / size) llg_fatal_allocation(what, grown, size);
    void* resized = realloc(items, grown * size);
    if (!resized) llg_fatal_allocation(what, grown, size);
    *capacity = grown;
    return resized;
}

// ── Heaps ────────────────────────────────────────────────────────────────────

static int timing_heap_before(const timing_heap_entry_t* a, const timing_heap_entry_t* b) {
    return a->time < b->time || (a->time == b->time && a->sequence < b->sequence);
}

static void timing_heap_push(timing_heap_t* heap, uint64_t time, void* item, uint32_t index) {
    if (heap->n == heap->capacity)
        heap->entries = (timing_heap_entry_t*)timing_grow(
            heap->entries, &heap->capacity, sizeof(*heap->entries), "timing check timers");
    size_t at = heap->n++;
    timing_heap_entry_t entry = {time, heap->sequence++, item, index};
    while (at > 0) {
        size_t parent = (at - 1u) / 2u;
        if (!timing_heap_before(&entry, &heap->entries[parent])) break;
        heap->entries[at] = heap->entries[parent];
        at = parent;
    }
    heap->entries[at] = entry;
}

static timing_heap_entry_t timing_heap_pop(timing_heap_t* heap) {
    timing_heap_entry_t top = heap->entries[0];
    timing_heap_entry_t last = heap->entries[--heap->n];
    size_t at = 0;
    for (;;) {
        size_t child = at * 2u + 1u;
        if (child >= heap->n) break;
        if (child + 1u < heap->n &&
            timing_heap_before(&heap->entries[child + 1u], &heap->entries[child]))
            child++;
        if (!timing_heap_before(&heap->entries[child], &last)) break;
        heap->entries[at] = heap->entries[child];
        at = child;
    }
    if (heap->n) heap->entries[at] = last;
    return top;
}

// Keep one region callback at the earliest due time of `heap`.
static void timing_heap_schedule(timing_heap_t* heap, llg_region_t region,
                                 llg_region_callback_fn callback) {
    if (!heap->n) return;
    uint64_t due = heap->entries[0].time;
    if (heap->scheduled != UINT64_MAX) {
        if (heap->scheduled <= due) return;
        llg_cancel_region_callback(heap->id);
        heap->scheduled = UINT64_MAX;
    }
    uint64_t ticks = due > g.now ? due - g.now : 0u;
    if (llg_schedule_region_callback_id(region, callback, NULL, ticks, &heap->id))
        heap->scheduled = due;
}

// ── Watches and the dirty list ───────────────────────────────────────────────

static void timing_watch_add(llg_timing_check_set_t* set, uint32_t terminal) {
    llg_wait_source_t* source = wait_source_get(set->terminals[terminal].value);
    if (!source->timing_keep) {
        llg_wait_subscription_t* node = (llg_wait_subscription_t*)llg_checked_calloc(
            1, sizeof(*node), "timing check watch");
        node->source = source;
        wait_subscription_insert(node, &source->head);
        source->timing_keep = node;
    }
    llg_timing_watch_t* watch = (llg_timing_watch_t*)llg_checked_calloc(
        1, sizeof(*watch), "timing check watch");
    watch->set = set;
    watch->terminal = terminal;
    watch->next = source->timing;
    source->timing = watch;
}

static void timing_watch_release(const void* key) {
    llg_wait_source_t* source = wait_source_find(key);
    if (!source || !source->timing_keep) return;
    while (source->timing) {
        llg_timing_watch_t* next = source->timing->next;
        free(source->timing);
        source->timing = next;
    }
    llg_wait_subscription_t* node = source->timing_keep;
    source->timing_keep = NULL;
    // Unlinking the last subscriber frees the row itself.
    wait_subscription_unlink(node);
    free(node);
}

static void timing_ring(void) {
    if (timing.rung || timing.running || !timing.doorbell) return;
    timing.rung = 1;
    sv4_t value = sv4_from_u64(llg_sv4_state(*timing.doorbell, 0) == 1u ? 0u : 1u, 1, 0);
    sig_write(timing.doorbell, value);
    sv4_destroy(&value);
}

static void timing_mark(llg_timing_check_set_t* set, uint32_t terminal) {
    if (!set->dirty[terminal]) {
        set->dirty[terminal] = 1u;
        set->changed[set->n_changed++] = terminal;
    }
    if (set->queued) return;
    set->queued = 1u;
    set->dirty_next = NULL;
    if (timing.dirty_tail) timing.dirty_tail->dirty_next = set;
    else timing.dirty_head = set;
    timing.dirty_tail = set;
}

static void timing_terminals_changed(llg_timing_watch_t* watch) {
    for (; watch; watch = watch->next) timing_mark(watch->set, watch->terminal);
    timing_ring();
}

// ── Delayed signals (31.9) ───────────────────────────────────────────────────

// The delay of one bit's change: the rise delay for a change to 1 or from 0
// to x or z, the fall delay for a change to 0 or from 1 to x or z, the
// smaller one between x and z.
static uint64_t timing_line_delay(const llg_timing_line_t* line, uint8_t from, uint8_t to) {
    if (to == 1u || (to > 1u && from == 0u)) return line->rise;
    if (to == 0u || (to > 1u && from == 1u)) return line->fall;
    return line->rise < line->fall ? line->rise : line->fall;
}

// Write the line's delayed value: its target bits through its net driver
// slot (z elsewhere) or into its variable.
static void timing_line_write(llg_timing_line_t* line) {
    const llg_timing_terminal_t* target = &line->set->terminals[line->target];
    sv4_t value = SV4_EMPTY;
    if (line->net) value = sv4_fill(3, llg_sv4_width(line->net->resolved), 0);
    else sv4_copy(&value, target->value);
    for (uint32_t bit = 0; bit < target->n_bits; bit++)
        llg_sv4_set_state(&value, (uint64_t)target->lsb + bit, line->state[bit]);
    if (line->net) llg_net_write(line->net, line->slot, value);
    else llg_ba(target->value, value);
    sv4_destroy(&value);
}

static void timing_delays_due(void* data);

// Schedule bit `bit` of a delayed signal to become `state` at `time`; its
// pending transitions that are not earlier are cancelled. Returns 1 when the
// transition applies now.
static int timing_line_schedule(llg_timing_line_t* line, uint32_t bit, uint8_t state,
                                uint64_t time) {
    uint32_t kept = 0;
    for (uint32_t index = 0; index < line->n_pending; index++) {
        const llg_timing_pending_t* pending = &line->pending[index];
        if (pending->bit == bit && pending->time >= time) continue;
        line->pending[kept++] = *pending;
    }
    line->n_pending = kept;
    if (time <= g.now) {
        line->state[bit] = state;
        return 1;
    }
    if (line->n_pending == line->pending_capacity) {
        size_t capacity = line->pending_capacity;
        line->pending = (llg_timing_pending_t*)timing_grow(
            line->pending, &capacity, sizeof(*line->pending), "delayed signal events");
        line->pending_capacity = (uint32_t)capacity;
    }
    // Keep the pending transitions in time order, issue order for equals.
    uint32_t at = line->n_pending++;
    while (at > 0 && line->pending[at - 1u].time > time) {
        line->pending[at] = line->pending[at - 1u];
        at--;
    }
    line->pending[at] = (llg_timing_pending_t){time, bit, state};
    timing_heap_push(&timing.delays, time, line, 0);
    return 0;
}

// The source of a delayed signal changed: schedule each changed bit.
static void timing_line_source_changed(llg_timing_line_t* line) {
    const llg_timing_terminal_t* source = &line->set->terminals[line->source];
    const llg_timing_terminal_t* target = &line->set->terminals[line->target];
    uint32_t bits = source->n_bits < target->n_bits ? source->n_bits : target->n_bits;
    int now = 0;
    for (uint32_t bit = 0; bit < bits; bit++) {
        uint8_t state = (uint8_t)llg_sv4_state(*source->value, (uint64_t)source->lsb + bit);
        uint8_t from = line->last[bit];
        if (state == from) continue;
        line->last[bit] = state;
        now |= timing_line_schedule(line, bit, state,
                                    g.now + timing_line_delay(line, from, state));
    }
    if (now) timing_line_write(line);
    timing_heap_schedule(&timing.delays, LLG_REGION_ACTIVE, timing_delays_due);
}

static void timing_delays_due(void* data) {
    (void)data;
    timing.delays.scheduled = UINT64_MAX;
    while (timing.delays.n && timing.delays.entries[0].time <= g.now) {
        llg_timing_line_t* line = (llg_timing_line_t*)timing_heap_pop(&timing.delays).item;
        uint32_t due = 0;
        while (due < line->n_pending && line->pending[due].time <= g.now) {
            line->state[line->pending[due].bit] = line->pending[due].state;
            due++;
        }
        // A cancelled transition leaves a stale heap entry: nothing is due.
        if (!due) continue;
        memmove(line->pending, line->pending + due,
                (size_t)(line->n_pending - due) * sizeof(*line->pending));
        line->n_pending -= due;
        timing_line_write(line);
    }
    timing_heap_schedule(&timing.delays, LLG_REGION_ACTIVE, timing_delays_due);
}

// ── Start and release ────────────────────────────────────────────────────────

// A delayed signal starts x and reaches its source's initial value through
// its delay (31.9).
static void timing_line_start(llg_timing_check_set_t* set, llg_timing_line_t* line) {
    const llg_timing_terminal_t* target = &set->terminals[line->target];
    line->set = set;
    line->last = (uint8_t*)llg_checked_calloc(target->n_bits ? target->n_bits : 1u, 1,
                                              "delayed signal");
    line->state = (uint8_t*)llg_checked_calloc(target->n_bits ? target->n_bits : 1u, 1,
                                               "delayed signal");
    line->pending = NULL;
    line->n_pending = 0;
    line->pending_capacity = 0;
    for (uint32_t bit = 0; bit < target->n_bits; bit++) {
        line->state[bit] = 2u;
        line->last[bit] = 2u;
    }
    timing_line_write(line);
    timing_line_source_changed(line);
}

void llg_timing_checks_layout(llg_timing_check_set_t* set, const char* scope,
                              const llg_timing_template_t* checks_template,
                              const void* condition_context,
                              llg_timing_pair_t* pairs, uint32_t n_pairs,
                              llg_timing_pools_t* pools) {
    const llg_timing_template_t* t = checks_template;
    memset(set, 0, sizeof(*set));
    set->scope = scope;
    set->n_terminals = t->n_terminals;
    set->terminals = pools->terminals;
    for (uint32_t index = 0; index < t->n_terminals; index++) {
        pools->terminals[index] = t->terminals[index];
        pools->terminals[index].value = pools->values[index];
    }
    pools->terminals += t->n_terminals;
    pools->values += t->n_terminals;
    set->n_checks = t->n_checks;
    set->checks = t->n_checks ? pools->checks : NULL;
    if (t->n_checks) memcpy(pools->checks, t->checks, t->n_checks * sizeof(*pools->checks));
    pools->checks += t->n_checks;
    set->conditions = t->conditions;
    set->n_conditions = t->n_conditions;
    set->condition_context = condition_context;
    set->n_notifiers = t->n_notifiers;
    set->notifiers = t->n_notifiers ? pools->values : NULL;
    pools->values += t->n_notifiers;
    set->n_lines = t->n_lines;
    set->lines = t->n_lines ? pools->lines : NULL;
    for (uint32_t index = 0; index < t->n_lines; index++) {
        pools->lines[index] = t->lines[index];
        pools->lines[index].net = pools->line_nets[index];
    }
    pools->lines += t->n_lines;
    pools->line_nets += t->n_lines;
    set->pairs = pairs;
    set->n_pairs = n_pairs;
}

void llg_timing_checks_start(llg_timing_check_set_t* set) {
    if (!set || set->offsets) return;
    uint32_t terminals = set->n_terminals ? set->n_terminals : 1u;
    set->offsets = (uint32_t*)llg_checked_calloc(terminals, sizeof(*set->offsets),
                                                 "timing check terminals");
    set->events = (uint8_t*)llg_checked_calloc(terminals, sizeof(*set->events),
                                               "timing check terminals");
    set->dirty = (uint8_t*)llg_checked_calloc(terminals, sizeof(*set->dirty),
                                              "timing check terminals");
    // Every terminal at most once, plus the changes a step itself makes.
    set->changed = (uint32_t*)llg_checked_calloc((size_t)terminals * 2u,
                                                 sizeof(*set->changed),
                                                 "timing check terminals");
    set->n_changed = 0;
    set->queued = 0;
    uint64_t total = 0;
    for (uint32_t index = 0; index < set->n_terminals; index++) {
        if (total > UINT32_MAX) {
            fprintf(stderr, "llg: fatal: timing check terminals are too wide\n");
            abort();
        }
        set->offsets[index] = (uint32_t)total;
        total += set->terminals[index].n_bits;
    }
    set->last = (uint8_t*)llg_checked_calloc(total ? (size_t)total : 1u, sizeof(*set->last),
                                             "timing check terminals");
    for (uint32_t index = 0; index < set->n_terminals; index++) {
        const llg_timing_terminal_t* terminal = &set->terminals[index];
        uint8_t* last = set->last + set->offsets[index];
        for (uint32_t bit = 0; bit < terminal->n_bits; bit++)
            last[bit] = timing_state(terminal->value, (uint64_t)terminal->lsb + bit);
    }
    // The checks reading each terminal, in check order.
    set->reader_first = (uint32_t*)llg_checked_calloc(
        (size_t)terminals + 1u, sizeof(*set->reader_first), "timing check readers");
    for (uint32_t index = 0; index < set->n_checks; index++) {
        const llg_timing_check_t* check = &set->checks[index];
        if (check->reference.terminal < set->n_terminals)
            set->reader_first[check->reference.terminal + 1u]++;
        if (check->data.terminal < set->n_terminals &&
            check->data.terminal != check->reference.terminal)
            set->reader_first[check->data.terminal + 1u]++;
    }
    for (uint32_t index = 0; index < terminals; index++)
        set->reader_first[index + 1u] += set->reader_first[index];
    set->readers = (uint32_t*)llg_checked_calloc(
        set->reader_first[terminals] ? set->reader_first[terminals] : 1u,
        sizeof(*set->readers), "timing check readers");
    uint32_t* fill = (uint32_t*)llg_checked_calloc(terminals, sizeof(*fill),
                                                   "timing check readers");
    for (uint32_t index = 0; index < set->n_checks; index++) {
        const llg_timing_check_t* check = &set->checks[index];
        uint32_t reference = check->reference.terminal;
        uint32_t data = check->data.terminal;
        if (reference < set->n_terminals)
            set->readers[set->reader_first[reference] + fill[reference]++] = index;
        if (data < set->n_terminals && data != reference)
            set->readers[set->reader_first[data] + fill[data]++] = index;
    }
    free(fill);
    set->candidates = (uint64_t*)llg_checked_calloc(
        ((size_t)set->n_checks + 63u) / 64u + 1u, sizeof(*set->candidates),
        "timing check candidates");
    if (set->n_conditions) {
        set->condition_values = (sv4_t*)llg_checked_calloc(
            set->n_conditions, sizeof(*set->condition_values), "timing check conditions");
        for (uint32_t index = 0; index < set->n_conditions; index++)
            set->condition_values[index] = (sv4_t)SV4_EMPTY;
    }
    set->notify = (uint8_t*)llg_checked_calloc(set->n_notifiers ? set->n_notifiers : 1u,
                                               sizeof(*set->notify), "timing check notifiers");
    for (uint32_t index = 0; index < set->n_checks; index++) {
        llg_timing_check_t* check = &set->checks[index];
        check->reference_time = 0;
        check->data_time = 0;
        check->trail_time = 0;
        check->reference_delay = 0;
        check->data_delay = 0;
        check->deadline = 0;
        check->open = 0;
        check->state = 0;
        check->window_data = NULL;
        check->n_window_data = 0;
        check->window_data_capacity = 0;
    }
    set->next = g.timing_check_sets;
    g.timing_check_sets = set;
    for (uint32_t index = 0; index < set->n_terminals; index++) timing_watch_add(set, index);
    for (uint32_t index = 0; index < set->n_lines; index++)
        timing_line_start(set, &set->lines[index]);
}

llg_timing_check_set_t* llg_timing_check_sets(void) { return g.timing_check_sets; }

// ── Events and reports ───────────────────────────────────────────────────────

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

static int timing_condition_value(timing_step_t* step, uint32_t condition,
                                  int nondeterministic) {
    if (!condition) return 1;
    llg_timing_check_set_t* set = step->set;
    if (!step->evaluated) {
        set->conditions(set->condition_values, (void*)set->condition_context);
        step->evaluated = 1;
    }
    const sv4_t* value = &set->condition_values[condition - 1u];
    if (llg_sv4_width(*value) == 0) return 0;
    unsigned state = llg_sv4_state(*value, 0);
    return state == 1u || (state > 1u && nondeterministic);
}

// An event's conditions hold (31.7): each condition's least significant bit
// is 1, or x or z for `==` and `!=`.
static int timing_conditions(timing_step_t* step, const llg_timing_event_t* event) {
    return timing_condition_value(step, event->condition, event->nondeterministic & 1u) &&
           timing_condition_value(step, event->condition2, event->nondeterministic & 2u);
}

// The transitions of this step that match the event's edge control.
static uint8_t timing_matched(const timing_step_t* step, const llg_timing_event_t* event) {
    uint8_t events = step->set->events[event->terminal];
    return event->edges ? (uint8_t)(events & event->edges) : events;
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

// The delay of a delayed terminal for the transitions `matched` (its rise
// delay when any of them rises); 0 for an undelayed terminal.
static uint64_t timing_delay(const timing_step_t* step, uint32_t terminal, uint8_t matched) {
    uint32_t line = step->set->terminals[terminal].line;
    if (!line) return 0;
    const llg_timing_line_t* delayed = &step->set->lines[line - 1u];
    return matched & LLG_TIMING_EDGE_RISING ? delayed->rise : delayed->fall;
}

// A limit compared between a timestamp transition delayed by `stamp` and a
// timecheck transition delayed by `check` (31.9): it moves by the delay
// difference and stays at least 0.
static uint64_t timing_adjusted(int64_t limit, uint64_t check, uint64_t stamp) {
    int64_t adjusted = limit + (int64_t)check - (int64_t)stamp;
    return adjusted > 0 ? (uint64_t)adjusted : 0u;
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

static void timing_report(llg_timing_check_set_t* set, const llg_timing_check_t* check,
                          uint64_t reference, uint64_t data);

// One violation: a report on stderr (unless turned off), the notifier flag
// and the violation observer. The times are those of the undelayed events.
static void timing_violation(llg_timing_check_set_t* set, const llg_timing_check_t* check,
                             uint64_t reference, uint64_t data) {
    if (check->notifier && check->notifier <= set->n_notifiers)
        set->notify[check->notifier - 1u] = 1u;
    if (timing_check_messages) timing_report(set, check, reference, data);
    if (LLG_CO_UNLIKELY(timing_violation_hook != NULL)) timing_violation_hook(set, check);
}

static void timing_report(llg_timing_check_set_t* set, const llg_timing_check_t* check,
                          uint64_t reference, uint64_t data) {
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
        else if (*text == '\003' || *text == '\004')
            llg_report_printf(&report, "%lld",
                              (long long)(*text == '\003' ? check->limit : check->limit2));
        else
            llg_report_bytes(&report, text, 1);
    }
    llg_report_bytes(&report, "\n", 1);
    llg_report_emit(&report, stderr);
}

// Table 31-13 with x becoming 0 (V Table 61, A9-D5): x -> 0, 0 -> 1,
// 1 -> 0, z stays.
static void timing_toggle(sv4_t* notifier) {
    unsigned state = llg_sv4_state(*notifier, 0);
    if (state == 3u) return;
    sv4_t value = sv4_from_u64(state == 0u ? 1u : 0u, llg_sv4_width(*notifier),
                               llg_sv4_signed(*notifier));
    llg_ba(notifier, value);
    sv4_destroy(&value);
}

static void timing_toggle_flagged(llg_timing_check_set_t* set) {
    for (uint32_t notifier = 0; notifier < set->n_notifiers; notifier++) {
        if (!set->notify[notifier]) continue;
        set->notify[notifier] = 0;
        timing_toggle(set->notifiers[notifier]);
    }
}

// ── Timers of `$timeskew` and `$fullskew` ────────────────────────────────────

static void timing_timers_due(void* data);

static void timing_arm(llg_timing_check_set_t* set, uint32_t index, uint64_t deadline) {
    set->checks[index].deadline = deadline;
    timing_heap_push(&timing.timers, deadline, set, index);
    timing_heap_schedule(&timing.timers, LLG_REGION_PRE_POSTPONED, timing_timers_due);
}

// The limit of the open skew window: limit 1 after a reference timestamp
// (state 1), the `$fullskew` limit 2 after a data timestamp (state 2).
static uint64_t timing_skew_limit(const llg_timing_check_t* check) {
    int64_t limit = check->kind == LLG_TIMING_FULLSKEW && check->state == 2u ? check->limit2
                                                                              : check->limit;
    return limit > 0 ? (uint64_t)limit : 0u;
}

static uint64_t timing_skew_stamp(const llg_timing_check_t* check) {
    return check->state == 2u ? check->data_time : check->reference_time;
}

// A timer-based window expired without a timecheck event (31.4.2, 31.4.3):
// a violation, and the check turns dormant. It runs after every event of the
// expiry time step, so a timecheck or new timestamp event at that time wins.
static void timing_timers_due(void* data) {
    (void)data;
    timing.timers.scheduled = UINT64_MAX;
    while (timing.timers.n && timing.timers.entries[0].time <= g.now) {
        timing_heap_entry_t entry = timing_heap_pop(&timing.timers);
        llg_timing_check_set_t* set = (llg_timing_check_set_t*)entry.item;
        llg_timing_check_t* check = &set->checks[entry.index];
        // A rearmed or dormant check leaves a stale entry.
        if (!check->state || check->deadline != entry.time) continue;
        uint64_t stamp = timing_skew_stamp(check);
        uint64_t deadline = stamp + timing_skew_limit(check);
        if (deadline > g.now) {
            timing_arm(set, entry.index, deadline);
            continue;
        }
        if (check->state == 2u) timing_violation(set, check, g.now, stamp);
        else timing_violation(set, check, stamp, g.now);
        check->state = 0;
        timing_toggle_flagged(set);
    }
    timing_heap_schedule(&timing.timers, LLG_REGION_PRE_POSTPONED, timing_timers_due);
}

void llg_timing_check_limits_changed(llg_timing_check_set_t* set) {
    if (!set || !set->offsets) return;
    for (uint32_t index = 0; index < set->n_checks; index++) {
        llg_timing_check_t* check = &set->checks[index];
        if ((check->kind != LLG_TIMING_TIMESKEW && check->kind != LLG_TIMING_FULLSKEW) ||
            (check->flags & LLG_TIMING_EVENT_BASED) || !check->state)
            continue;
        uint64_t deadline = timing_skew_stamp(check) + timing_skew_limit(check);
        if (deadline < g.now) deadline = g.now;
        if (deadline != check->deadline) timing_arm(set, index, deadline);
    }
}

// ── Checks ───────────────────────────────────────────────────────────────────

// `$timeskew` (31.4.2): the reference event stamps and activates; with a
// false condition it makes the check dormant unless remain_active is set. A
// data event beyond the limit violates; an event-based check then turns
// dormant unless remain_active is set. A data event within the limit makes a
// timer-based check dormant; a timer-based check reports at the expiry.
static void timing_timeskew(timing_step_t* step, uint32_t index, llg_timing_check_t* check) {
    uint64_t now = g.now;
    int timer = !(check->flags & LLG_TIMING_EVENT_BASED);
    int remain = (check->flags & LLG_TIMING_REMAIN_ACTIVE) != 0;
    if (timing_matched(step, &check->reference)) {
        if (timing_conditions(step, &check->reference)) {
            check->reference_time = now;
            check->state = 1u;
            if (timer) timing_arm(step->set, index, now + timing_skew_limit(check));
        } else if (!remain) {
            check->state = 0;
        }
    }
    if (!check->state || !timing_matched(step, &check->data) ||
        !timing_conditions(step, &check->data))
        return;
    if (now - check->reference_time > timing_skew_limit(check)) {
        timing_violation(step->set, check, check->reference_time, now);
        if (timer || !remain) check->state = 0;
    } else if (timer) {
        check->state = 0;
    }
}

// `$fullskew` (31.4.3): either signal stamps (state 1: reference, 2: data)
// and the other one is the timecheck event, limit 1 after a reference stamp
// and limit 2 after a data stamp. A timecheck event within the limit ends
// the window (dormant); beyond it, a violation, after which an event-based
// check stamps the next window. A timestamp event replaces the window; with
// a false condition it is ignored under remain_active and otherwise makes an
// active check dormant.
static void timing_fullskew(timing_step_t* step, uint32_t index, llg_timing_check_t* check) {
    uint64_t now = g.now;
    int timer = !(check->flags & LLG_TIMING_EVENT_BASED);
    int remain = (check->flags & LLG_TIMING_REMAIN_ACTIVE) != 0;
    const llg_timing_event_t* events[2] = {&check->reference, &check->data};
    for (uint8_t side = 1u; side <= 2u; side++) {
        const llg_timing_event_t* event = events[side - 1u];
        if (!timing_matched(step, event)) continue;
        int enabled = timing_conditions(step, event);
        if (check->state && check->state != side) {
            // A timecheck event; one with a false condition is no event.
            if (!enabled) continue;
            uint64_t stamp = timing_skew_stamp(check);
            if (now - stamp <= timing_skew_limit(check)) {
                check->state = 0;
                continue;
            }
            if (check->state == 2u) timing_violation(step->set, check, now, stamp);
            else timing_violation(step->set, check, stamp, now);
            if (timer) {
                check->state = 0;
                continue;
            }
        } else if (!enabled) {
            if (!remain) check->state = 0;
            continue;
        }
        check->state = side;
        if (side == 2u) check->data_time = now;
        else check->reference_time = now;
        if (timer) timing_arm(step->set, index, now + timing_skew_limit(check));
    }
}

// `$nochange` (31.4.6) with start offset S and end offset E, in undelayed
// times: a data event in (leading edge - S, trailing edge + E) violates. The
// leading and trailing edges always open and close the window; the
// reference and data conditions are read at the data event. A data event is
// decided when it is seen (the last one before the leading edge within S at
// that edge) unless the window can end before the delayed trailing edge
// arrives: E below the trailing edge's delay minus the data event's delay,
// which without delayed signals is a negative E. Such events are kept and
// each one inside the window is reported at the trailing edge.
static int timing_nochange_deferred(int64_t end, int64_t trail_delay, uint64_t data_delay) {
    return end - trail_delay + (int64_t)data_delay < 0;
}

static void timing_nochange_keep(llg_timing_check_t* check, int64_t data) {
    if (check->n_window_data == check->window_data_capacity) {
        size_t capacity = check->window_data_capacity;
        check->window_data = (int64_t*)timing_grow(check->window_data, &capacity,
                                                   sizeof(*check->window_data),
                                                   "timing check window");
        check->window_data_capacity = (uint32_t)capacity;
    }
    check->window_data[check->n_window_data++] = data;
}

static void timing_nochange(timing_step_t* step, llg_timing_check_t* check) {
    uint64_t now = g.now;
    int64_t start = check->limit;
    int64_t end = check->limit2;
    llg_timing_event_t trailing = check->reference;
    trailing.edges = timing_opposite(check->reference.edges);
    uint8_t lead = timing_matched(step, &check->reference);
    uint8_t trail = timing_matched(step, &trailing);
    int64_t trail_delay = (int64_t)timing_delay(step, check->reference.terminal, trailing.edges);
    // Undelayed times, signed so that offsets can move them past each other.
    int64_t data = (int64_t)check->data_time - (int64_t)check->data_delay;
    if (lead && !check->open) {
        check->open = 1u;
        check->reference_time = now;
        check->reference_delay = timing_delay(step, check->reference.terminal, lead);
        check->trail_time = 0;
        check->n_window_data = 0;
        int64_t leading = (int64_t)now - (int64_t)check->reference_delay;
        if (check->data_time && data > leading - start) {
            if (timing_nochange_deferred(end, trail_delay, check->data_delay))
                timing_nochange_keep(check, data);
            else
                timing_violation(step->set, check, (uint64_t)leading, (uint64_t)data);
        }
    } else if (trail && check->open) {
        check->open = 0;
        check->trail_time = now - timing_delay(step, check->reference.terminal, trail);
        int64_t leading = (int64_t)check->reference_time - (int64_t)check->reference_delay;
        for (uint32_t index = 0; index < check->n_window_data; index++) {
            int64_t kept = check->window_data[index];
            if (kept > leading - start && kept < (int64_t)check->trail_time + end)
                timing_violation(step->set, check, (uint64_t)leading, (uint64_t)kept);
        }
        check->n_window_data = 0;
    }
    uint8_t matched = timing_matched(step, &check->data);
    if (!matched || !timing_conditions(step, &check->reference) ||
        !timing_conditions(step, &check->data))
        return;
    check->data_time = now;
    check->data_delay = timing_delay(step, check->data.terminal, matched);
    if (!check->reference_time) return;
    data = (int64_t)now - (int64_t)check->data_delay;
    int64_t leading = (int64_t)check->reference_time - (int64_t)check->reference_delay;
    if (timing_nochange_deferred(end, trail_delay, check->data_delay)) {
        if (check->open) timing_nochange_keep(check, data);
        return;
    }
    if (check->open) {
        if (data > leading - start)
            timing_violation(step->set, check, (uint64_t)leading, (uint64_t)data);
    } else if (check->trail_time && data - (int64_t)check->trail_time < end) {
        timing_violation(step->set, check, (uint64_t)leading, (uint64_t)data);
    }
}

static void timing_check_run(timing_step_t* step, uint32_t index, llg_timing_check_t* check) {
    llg_timing_check_set_t* set = step->set;
    uint64_t now = g.now;
    switch (check->kind) {
        case LLG_TIMING_SETUP:
        case LLG_TIMING_REMOVAL: {
            // Timestamp: data event; timecheck: reference event. Violation
            // when (timecheck - limit) < timestamp < timecheck (31.3.1,
            // 31.3.4). A data event in the same step stamps first, so it
            // never violates.
            uint8_t data = timing_matched(step, &check->data);
            if (data && timing_conditions(step, &check->data)) {
                check->data_time = now;
                check->data_delay = timing_delay(step, check->data.terminal, data);
            }
            uint8_t reference = timing_matched(step, &check->reference);
            if (!reference || !timing_conditions(step, &check->reference) ||
                !check->data_time || check->data_time >= now)
                break;
            uint64_t delay = timing_delay(step, check->reference.terminal, reference);
            if (now - check->data_time < timing_adjusted(check->limit, delay, check->data_delay))
                timing_violation(set, check, now - delay, check->data_time - check->data_delay);
            break;
        }
        case LLG_TIMING_HOLD:
        case LLG_TIMING_RECOVERY: {
            // Timestamp: reference event; timecheck: data event. Violation
            // when timestamp <= timecheck < timestamp + limit (31.3.2,
            // 31.3.5), so a data event earlier in the reference event's time
            // slot violates when the reference event arrives.
            uint8_t reference = timing_matched(step, &check->reference);
            if (reference && !timing_conditions(step, &check->reference)) reference = 0;
            uint8_t data = timing_matched(step, &check->data);
            if (data && !timing_conditions(step, &check->data)) data = 0;
            if (reference) {
                check->reference_time = now;
                check->reference_delay =
                    timing_delay(step, check->reference.terminal, reference);
                if (!data && check->data_time == now &&
                    timing_adjusted(check->limit, check->data_delay, check->reference_delay))
                    timing_violation(set, check, now - check->reference_delay,
                                     now - check->data_delay);
            }
            if (data) {
                check->data_time = now;
                check->data_delay = timing_delay(step, check->data.terminal, data);
                uint64_t limit =
                    timing_adjusted(check->limit, check->data_delay, check->reference_delay);
                if (check->reference_time && now - check->reference_time < limit)
                    timing_violation(set, check,
                                     check->reference_time - check->reference_delay,
                                     now - check->data_delay);
            }
            break;
        }
        case LLG_TIMING_SKEW: {
            // Violation when timecheck - timestamp > limit (31.4.1); every
            // later data event beyond the limit violates again.
            if (timing_matched(step, &check->reference) &&
                timing_conditions(step, &check->reference))
                check->reference_time = now;
            uint64_t limit = check->limit > 0 ? (uint64_t)check->limit : 0u;
            if (timing_matched(step, &check->data) && timing_conditions(step, &check->data) &&
                check->reference_time && now - check->reference_time > limit)
                timing_violation(set, check, check->reference_time, now);
            break;
        }
        case LLG_TIMING_PERIOD: {
            // Each edge checks the previous one, then restamps (31.4.5).
            uint8_t reference = timing_matched(step, &check->reference);
            if (!reference || !timing_conditions(step, &check->reference)) break;
            uint64_t delay = timing_delay(step, check->reference.terminal, reference);
            uint64_t limit = timing_adjusted(check->limit, delay, check->reference_delay);
            if (check->reference_time && now - check->reference_time < limit)
                timing_violation(set, check, check->reference_time - check->reference_delay,
                                 now - delay);
            check->reference_time = now;
            check->reference_delay = delay;
            break;
        }
        case LLG_TIMING_WIDTH: {
            // Violation when threshold < width < limit (31.4.4); the stamp
            // stays, so consecutive opposite edges are all checked.
            uint8_t reference = timing_matched(step, &check->reference);
            if (reference && timing_conditions(step, &check->reference)) {
                check->reference_time = now;
                check->reference_delay =
                    timing_delay(step, check->reference.terminal, reference);
            }
            uint8_t data = timing_matched(step, &check->data);
            if (!data || !timing_conditions(step, &check->data) || !check->reference_time)
                break;
            uint64_t delay = timing_delay(step, check->data.terminal, data);
            uint64_t width = now - check->reference_time;
            if (width > timing_adjusted(check->limit2, delay, check->reference_delay) &&
                width < timing_adjusted(check->limit, delay, check->reference_delay))
                timing_violation(set, check, check->reference_time - check->reference_delay,
                                 now - delay);
            break;
        }
        case LLG_TIMING_NOCHANGE:
            timing_nochange(step, check);
            break;
        case LLG_TIMING_TIMESKEW:
            timing_timeskew(step, index, check);
            break;
        case LLG_TIMING_FULLSKEW:
            timing_fullskew(step, index, check);
            break;
        default:
            break;
    }
}

static void timing_check_step(llg_timing_check_set_t* set) {
    uint32_t changed = set->n_changed;
    int any = 0;
    for (uint32_t position = 0; position < changed; position++) {
        uint32_t index = set->changed[position];
        const llg_timing_terminal_t* terminal = &set->terminals[index];
        uint8_t* last = set->last + set->offsets[index];
        uint8_t events = 0;
        for (uint32_t bit = 0; bit < terminal->n_bits; bit++) {
            uint8_t state = timing_state(terminal->value, (uint64_t)terminal->lsb + bit);
            events |= timing_transition(last[bit], state);
            last[bit] = state;
        }
        set->events[index] = events;
        if (!events) continue;
        any = 1;
        for (uint32_t reader = set->reader_first[index]; reader < set->reader_first[index + 1u];
             reader++) {
            uint32_t check = set->readers[reader];
            set->candidates[check / 64u] |= UINT64_C(1) << (check % 64u);
        }
    }
    // Events at time 0 never stamp or check (the initial values settle).
    size_t words = ((size_t)set->n_checks + 63u) / 64u;
    if (any && g.now != 0) {
        timing_step_t step = {set, 0};
        for (size_t word = 0; word < words; word++) {
            uint64_t bits = set->candidates[word];
            while (bits) {
                uint32_t offset = 0;
                while (!(bits & (UINT64_C(1) << offset))) offset++;
                bits &= bits - 1u;
                uint32_t index = (uint32_t)(word * 64u) + offset;
                if (!(set->checks[index].flags & LLG_TIMING_DISABLED))
                    timing_check_run(&step, index, &set->checks[index]);
            }
        }
    }
    if (any) memset(set->candidates, 0, words * sizeof(*set->candidates));
    for (uint32_t position = 0; position < changed; position++) {
        set->events[set->changed[position]] = 0;
        set->dirty[set->changed[position]] = 0;
    }
    // Changes the delayed signals and notifiers make from here on are
    // listed after this step's terminals and run in a later step.
    for (uint32_t position = 0; position < changed && set->n_lines; position++) {
        uint32_t index = set->changed[position];
        for (uint32_t line = 0; line < set->n_lines; line++) {
            if (set->lines[line].source == index)
                timing_line_source_changed(&set->lines[line]);
        }
    }
    timing_toggle_flagged(set);
    set->n_changed -= changed;
    memmove(set->changed, set->changed + changed, (size_t)set->n_changed * sizeof(*set->changed));
}

void llg_timing_checks_run(void) {
    timing.rung = 0;
    timing.running = 1;
    while (timing.dirty_head) {
        llg_timing_check_set_t* set = timing.dirty_head;
        timing.dirty_head = set->dirty_next;
        if (!timing.dirty_head) timing.dirty_tail = NULL;
        set->dirty_next = NULL;
        set->queued = 0;
        timing_check_step(set);
    }
    timing.running = 0;
}

// Release every started set; the generated model owns the tables.
static void timing_checks_release(void) {
    while (g.timing_check_sets) {
        llg_timing_check_set_t* set = g.timing_check_sets;
        g.timing_check_sets = set->next;
        set->next = NULL;
        for (uint32_t index = 0; index < set->n_terminals; index++)
            timing_watch_release(set->terminals[index].value);
        if (set->condition_values) {
            for (uint32_t index = 0; index < set->n_conditions; index++)
                sv4_destroy(&set->condition_values[index]);
        }
        for (uint32_t index = 0; index < set->n_checks; index++) {
            llg_timing_check_t* check = &set->checks[index];
            free(check->window_data);
            check->window_data = NULL;
            check->n_window_data = 0;
            check->window_data_capacity = 0;
        }
        for (uint32_t index = 0; index < set->n_lines; index++) {
            llg_timing_line_t* line = &set->lines[index];
            free(line->last);
            free(line->state);
            free(line->pending);
            line->last = NULL;
            line->state = NULL;
            line->pending = NULL;
            line->n_pending = 0;
            line->pending_capacity = 0;
        }
        free(set->condition_values);
        free(set->last);
        free(set->offsets);
        free(set->events);
        free(set->dirty);
        free(set->changed);
        free(set->readers);
        free(set->reader_first);
        free(set->candidates);
        free(set->notify);
        set->condition_values = NULL;
        set->last = NULL;
        set->offsets = NULL;
        set->events = NULL;
        set->dirty = NULL;
        set->changed = NULL;
        set->readers = NULL;
        set->reader_first = NULL;
        set->candidates = NULL;
        set->notify = NULL;
        set->n_changed = 0;
        set->queued = 0;
        set->dirty_next = NULL;
    }
    free(timing.delays.entries);
    free(timing.timers.entries);
    timing.delays = (timing_heap_t)TIMING_HEAP_EMPTY;
    timing.timers = (timing_heap_t)TIMING_HEAP_EMPTY;
    timing.doorbell = NULL;
    timing.rung = 0;
    timing.running = 0;
    timing.dirty_head = NULL;
    timing.dirty_tail = NULL;
    timing_check_messages = 1;
}
