
// Deferred immediate assertion reports (SV 16.4). Every execution of a
// deferred assertion that has an observable report appends one record to the
// issuing process's report queue. A flush point of that process (SV 16.4.2)
// clears its records; the Observed region matures every remaining record
// (SV 16.4.1), and matured records execute in the Reactive region in issue
// order. Records live in two retained arrays (pending and matured), so a
// steady stream of reports performs no per-report allocation.

static void free_assertion_rules(void) {
    while (llg_assertion_rules) {
        llg_assertion_rule_t* next = llg_assertion_rules->next;
        free(llg_assertion_rules->scope);
        free(llg_assertion_rules);
        llg_assertion_rules = next;
    }
}

static void deferred_queue_release(llg_deferred_report_queue_t* queue,
                                   uint32_t from) {
    for (uint32_t i = from; i < queue->count; i++) {
        llg_frame_release(queue->items[i].frame);
        queue->items[i].frame = NULL;
    }
    queue->count = 0;
}

static void free_deferred_assertions(void) {
    deferred_queue_release(&g.deferred_pending, 0);
    deferred_queue_release(&g.deferred_matured, g.deferred_matured_next);
    free(g.deferred_pending.items);
    free(g.deferred_matured.items);
    g.deferred_pending = (llg_deferred_report_queue_t){0};
    g.deferred_matured = (llg_deferred_report_queue_t){0};
    g.deferred_pending_live = 0;
    g.deferred_matured_next = 0;
    g.deferred_matured_scheduled = 0;
}

static llg_deferred_assertion_report_t* deferred_queue_push(
    llg_deferred_report_queue_t* queue) {
    if (queue->count == queue->capacity) {
        if (queue->capacity >= UINT32_MAX / 2u) {
            fprintf(stderr, "llg: fatal: deferred assertion report queue overflow\n");
            abort();
        }
        uint32_t capacity = queue->capacity ? queue->capacity * 2u : 16u;
        llg_deferred_assertion_report_t* items =
            (llg_deferred_assertion_report_t*)llg_checked_malloc(
                capacity, sizeof(*items), "deferred assertion reports");
        if (queue->count)
            memcpy(items, queue->items, (size_t)queue->count * sizeof(*items));
        free(queue->items);
        queue->items = items;
        queue->capacity = capacity;
    }
    llg_deferred_assertion_report_t* report = &queue->items[queue->count++];
    memset(report, 0, sizeof(*report));
    return report;
}

// The index + 1 of `proc`'s newest pending report, or 0. A stale index from
// an earlier pass either lies beyond the current array or names a record of
// another owner: a process that issued a report in this pass updated the
// index to that report.
static uint32_t deferred_pending_head(const llg_proc_t* proc) {
    uint32_t head = proc->deferred_last;
    if (!head || head > g.deferred_pending.count) return 0;
    return g.deferred_pending.items[head - 1u].owner == proc->assertion_owner
               ? head
               : 0;
}

static void deferred_report_drop(llg_deferred_assertion_report_t* report) {
    if (!report->live) return;
    report->live = 0;
    llg_frame_release(report->frame);
    report->frame = NULL;
    g.deferred_pending_live--;
}

// Clear the pending reports of one process (SV 16.4.2 flush point).
static void deferred_flush_process(llg_proc_t* proc) {
    uint32_t index = deferred_pending_head(proc);
    proc->deferred_last = 0;
    proc->deferred_flush = 0;
    while (index) {
        llg_deferred_assertion_report_t* report =
            &g.deferred_pending.items[index - 1u];
        index = report->owner_prev;
        deferred_report_drop(report);
    }
}

void llg_deferred_assertion_scoped(int kind, int passed, uint64_t identity,
                                   const char* label, const char* location,
                                   const char* scope,
                                   llg_deferred_assertion_fn action,
                                   llg_frame_t* frame) {
    if (kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_COVER ||
        (passed != 0 && passed != 1)) {
        fprintf(stderr, "llg runtime fatal: invalid deferred assertion result\n");
        llg_frame_release(frame);
        abort();
    }
    if (!action && frame) {
        // A frame is meaningful only for a selected action. This also keeps
        // malformed embedding calls from leaking an owned capture.
        llg_frame_release(frame);
        frame = NULL;
    }
    // A passing assert/assume without a pass action, and a failing cover,
    // report nothing. Flush points act on the process queue, never on a
    // later report, so omitting these records changes no outcome.
    if ((kind == LLG_ASSERTION_COVER && !passed) ||
        (kind != LLG_ASSERTION_COVER && passed && !action) ||
        !llg_deferred_assertion_enabled(kind, label, scope)) {
        llg_frame_release(frame);
        return;
    }
    llg_proc_t* current = g.in_deferred_action ? NULL : llg_current();
    // Resolve the previous head before appending: a stale index may equal
    // the new record's position.
    uint32_t previous = current ? deferred_pending_head(current) : 0;
    llg_deferred_assertion_report_t* report =
        deferred_queue_push(&g.deferred_pending);
    report->owner = current ? current->assertion_owner : 0;
    report->identity = identity;
    report->label = label;
    report->location = location;
    report->scope = scope;
    report->action = action;
    report->frame = frame;
    report->kind = (uint8_t)kind;
    report->passed = (uint8_t)passed;
    report->live = 1;
    g.deferred_pending_live++;
    if (current) {
        report->owner_prev = previous;
        current->deferred_last = g.deferred_pending.count;
        const llg_activation_t* root = current->activation_top;
        while (root && root->parent) root = root->parent;
        if (root) {
            report->scope_declaration = root->declaration;
            report->scope_instance = root->instance;
        }
    }
}

// Compatibility entrypoint for embedding callers without hierarchy metadata.
void llg_deferred_assertion(int kind, int passed, uint64_t identity,
                            const char* label, const char* location,
                            llg_deferred_assertion_fn action, llg_frame_t* frame) {
    llg_deferred_assertion_scoped(kind, passed, identity, label, location, "", action, frame);
}

// `disable` of one deferred assertion cancels its pending reports in every
// process (SV 16.4.4). Matured reports are no longer pending.
void llg_deferred_assertion_cancel(uint64_t identity) {
    if (!g.deferred_pending_live) return;
    for (uint32_t i = 0; i < g.deferred_pending.count; i++) {
        llg_deferred_assertion_report_t* report = &g.deferred_pending.items[i];
        if (report->identity == identity) deferred_report_drop(report);
    }
}

// `disable` of the outermost scope of a procedure flushes that procedure's
// report queue (SV 16.4.4), whether or not the process is inside the block
// at the time (`always @(a) begin : b ... end` waits outside `b`). Reports
// remember the outermost activation they were issued under.
void llg_deferred_assertion_flush_scope(uint32_t declaration, uint32_t instance) {
    if (!g.deferred_pending_live) return;
    for (uint32_t i = 0; i < g.deferred_pending.count; i++) {
        llg_deferred_assertion_report_t* report = &g.deferred_pending.items[i];
        if (report->scope_declaration == declaration &&
            report->scope_instance == instance && report->owner)
            deferred_report_drop(report);
    }
}

static void deferred_report_execute(llg_deferred_assertion_report_t* report) {
    llg_frame_t* frame = report->frame;
    report->frame = NULL;
    int saved = g.in_deferred_action;
    if (report->passed && report->kind == LLG_ASSERTION_COVER)
        llg_assertion_cover(report->identity, report->label, report->location);
    if (report->action) {
        g.in_deferred_action = 1;
        report->action(frame);
        g.in_deferred_action = saved;
    } else if (!report->passed) {
        llg_assertion_failure(report->kind, report->identity, report->label,
                              report->location);
    }
    llg_frame_release(frame);
}

static void deferred_matured_callback(void* data);

static int deferred_schedule_matured(void) {
    if (g.deferred_matured_scheduled ||
        g.deferred_matured_next >= g.deferred_matured.count)
        return 1;
    if (!llg_schedule_region_callback(LLG_REGION_REACTIVE,
                                      deferred_matured_callback, NULL))
        return 0;
    g.deferred_matured_scheduled = 1;
    return 1;
}

// Run matured reports in issue order. The shared cursor makes a nested drain
// (an action calling $finish) continue with the next report rather than
// repeating one. A $stop from an action suspends the scheduler; the
// remaining reports run after it resumes.
static void deferred_run_matured(int stop_on_suspend) {
    while (g.deferred_matured_next < g.deferred_matured.count) {
        if (stop_on_suspend && g.suspended) return;
        llg_deferred_assertion_report_t* slot =
            &g.deferred_matured.items[g.deferred_matured_next++];
        // Copy: an action can append matured reports and move the array.
        llg_deferred_assertion_report_t report = *slot;
        slot->frame = NULL;
        deferred_report_execute(&report);
    }
    g.deferred_matured.count = 0;
    g.deferred_matured_next = 0;
}

static void deferred_matured_callback(void* data) {
    (void)data;
    g.deferred_matured_scheduled = 0;
    deferred_run_matured(1);
    if (g.deferred_matured_next < g.deferred_matured.count &&
        !deferred_schedule_matured()) {
        deferred_queue_release(&g.deferred_matured, g.deferred_matured_next);
        g.deferred_matured_next = 0;
    }
}

// Move every report that has not been flushed to the matured array; it can
// no longer be flushed (SV 16.4.1).
static void deferred_mature_pending(void) {
    if (!g.deferred_pending.count) return;
    if (g.deferred_matured_next == g.deferred_matured.count) {
        g.deferred_matured.count = 0;
        g.deferred_matured_next = 0;
    }
    for (uint32_t i = 0; i < g.deferred_pending.count; i++) {
        llg_deferred_assertion_report_t* report = &g.deferred_pending.items[i];
        if (!report->live) continue;
        llg_deferred_assertion_report_t* matured =
            deferred_queue_push(&g.deferred_matured);
        *matured = *report;
        matured->owner_prev = 0;
        report->frame = NULL;
    }
    g.deferred_pending.count = 0;
    g.deferred_pending_live = 0;
}

// Observed region: pending reports mature and their actions are scheduled
// in the Reactive region.
static void flush_deferred_assertions(void) {
    deferred_mature_pending();
    if (!deferred_schedule_matured()) {
        deferred_queue_release(&g.deferred_matured, g.deferred_matured_next);
        g.deferred_matured_next = 0;
    }
}

// A process resumed from an event control or wait statement reached a flush
// point (SV 16.4.2). Called by the scheduler before the process runs.
static void deferred_resume_flush(llg_proc_t* proc) {
    proc->deferred_flush = 0;
    if (deferred_pending_head(proc)) deferred_flush_process(proc);
}

static int wait_is_deferred_flush_point(llg_wait_kind_t kind) {
    switch (kind) {
    case W_EVENTS:
    case W_EVENTS_INLINE:
    case W_EVENT:
    case W_EVENT_TRIGGERED:
    case W_EVENT_ORDER:
    case W_MIXED:
    case W_DEPS:
    case W_EXPR:
    case W_LEVEL:
    case W_LEVEL_INLINE:
    case W_FORK_ALL:
        return 1;
    default:
        return 0;
    }
}

// Finish/deadlock teardown and the end of a final procedure execute the
// reports that are still queued, in issue order: matured ones first, then
// the pending ones (llg policy, recorded in docs/lrm_decisions.md).
static void run_deferred_assertions_now(void) {
    if (g.deferred_matured_next >= g.deferred_matured.count &&
        !g.deferred_pending_live) {
        g.deferred_pending.count = 0;
        return;
    }
    llg_region_t saved_region = g.current_region;
    int saved_action = g.in_deferred_action;
    g.current_region = LLG_REGION_REACTIVE;
    for (;;) {
        deferred_run_matured(0);
        if (!g.deferred_pending_live) {
            g.deferred_pending.count = 0;
            break;
        }
        deferred_mature_pending();
    }
    g.in_deferred_action = saved_action;
    g.current_region = saved_region;
}
