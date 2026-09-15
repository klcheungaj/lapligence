
static void free_deferred_assertion_report(
    llg_deferred_assertion_report_t* report) {
    if (!report) return;
    llg_frame_release(report->frame);
    free(report);
}

static void free_assertion_rules(void) {
    while (llg_assertion_rules) {
        llg_assertion_rule_t* next = llg_assertion_rules->next;
        free(llg_assertion_rules->scope);
        free(llg_assertion_rules);
        llg_assertion_rules = next;
    }
}

static void free_deferred_assertions(void) {
    while (g.deferred_assertions) {
        llg_deferred_assertion_report_t* next = g.deferred_assertions->next;
        free_deferred_assertion_report(g.deferred_assertions);
        g.deferred_assertions = next;
    }
    g.deferred_assertion_tail = NULL;
}

static void deferred_assertion_callback(void* data) {
    llg_deferred_assertion_report_t* report =
        (llg_deferred_assertion_report_t*)data;
    if (!report) return;
    llg_frame_t* frame = report->frame;
    report->frame = NULL;
    if (report->passed) {
        if (report->kind == LLG_ASSERTION_COVER)
            llg_assertion_cover(report->identity, report->label, report->location);
        if (report->action) {
            int saved = g.in_deferred_action;
            g.in_deferred_action = 1;
            report->action(frame);
            g.in_deferred_action = saved;
        }
    } else if (report->kind != LLG_ASSERTION_COVER) {
        if (report->action) {
            int saved = g.in_deferred_action;
            g.in_deferred_action = 1;
            report->action(frame);
            g.in_deferred_action = saved;
        } else {
            llg_assertion_failure(report->kind, report->identity,
                                  report->label, report->location);
        }
    }
    llg_frame_release(frame);
    free(report);
}

// Transfer queued reports to the Reactive region while the scheduler is at
// the Observed-to-Reactive handoff. Region callback ordering preserves source
// issue order after same-assertion coalescing.
static void flush_deferred_assertions(void) {
    while (g.deferred_assertions) {
        llg_deferred_assertion_report_t* report = g.deferred_assertions;
        g.deferred_assertions = report->next;
        report->next = NULL;
        if (!llg_schedule_region_callback(LLG_REGION_REACTIVE,
                                          deferred_assertion_callback, report)) {
            free_deferred_assertion_report(report);
            break;
        }
    }
    if (!g.deferred_assertions) g.deferred_assertion_tail = NULL;
}

// A finish request from a later read-only callback can stop the normal
// scheduler before the Reactive queue gets a turn. Deferred assertion
// callbacks are still mature reports and must run before teardown; unrelated
// callbacks remain subject to the ordinary finish discard rule.
static llg_region_callback_t* take_deferred_assertion_callback_now(void) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && (*slot)->time <= g.now) {
        llg_region_callback_t* entry = *slot;
        if (entry->time == g.now && entry->callback == deferred_assertion_callback) {
            *slot = entry->next;
            entry->next = NULL;
            return entry;
        }
        slot = &entry->next;
    }
    return NULL;
}

static int deferred_assertion_callback_pending_now(void) {
    for (llg_region_callback_t* entry = g.callbacks;
         entry && entry->time <= g.now; entry = entry->next) {
        if (entry->time == g.now && entry->callback == deferred_assertion_callback)
            return 1;
    }
    return 0;
}

// Finish/deadlock teardown can occur before the normal Observed handoff (for
// example, a process executes `$finish` immediately after `assert #0`). Run
// those reports in the Reactive context before releasing the scheduler.
static void run_deferred_assertions_now(void) {
    if (!g.deferred_assertions && !deferred_assertion_callback_pending_now()) return;
    llg_region_t saved_region = g.current_region;
    int saved_action = g.in_deferred_action;
    g.current_region = LLG_REGION_REACTIVE;
    for (;;) {
        while (g.deferred_assertions) {
            llg_deferred_assertion_report_t* report = g.deferred_assertions;
            g.deferred_assertions = report->next;
            report->next = NULL;
            deferred_assertion_callback(report);
        }
        g.deferred_assertion_tail = NULL;
        llg_region_callback_t* callback = take_deferred_assertion_callback_now();
        if (!callback) break;
        llg_region_callback_fn fn = callback->callback;
        void* data = callback->data;
        free(callback);
        fn(data);
    }
    g.in_deferred_action = saved_action;
    g.current_region = saved_region;
}
