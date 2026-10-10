
static int expression_qualifies(const llg_expr_event_spec_t* spec) {
    if (!spec->condition) return 1;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* result = llg_value_scope_values(scope);
    spec->condition(result, spec->condition_context);
    if (llg_rt_exiting()) {
        llg_value_scope_end(scope);
        return 0;
    }
    int qualifies = sv4_to_bool(*result);
    llg_value_scope_end(scope);
    return qualifies;
}

// Wake a suspended process: clear its wait node and schedule it.
static void wake_proc(llg_proc_t* p) {
    llg_wait_t* w = &p->wait;
    if (w->kind == W_NONE) return;
    if (p->deferred_last && wait_is_deferred_flush_point(w->kind))
        p->deferred_flush = 1;
    remove_waiters_entry(w);
    if (w->kind == W_TIME) {
        remove_timed_entry(w);
        remove_inactive_entry(w);
    }
    if (w->kind == W_EVENT || w->kind == W_EVENT_ORDER ||
        w->kind == W_MIXED || w->kind == W_EXPR) {
        event_unlink(w);
    }
    if (w->kind == W_EVENT_TRIGGERED) event_triggered_unlink(w);
    if (w->kind == W_SEMAPHORE) semaphore_waiter_unlink(w);
    if (w->kind == W_MAILBOX_GET || w->kind == W_MAILBOX_PUT)
        mailbox_unlink_wait(w);
    wait_payload_release(w);
    g.wait_count--;
    if (p->suspended) {
        // A suspended waiter keeps its condition registered until it fires;
        // once it fires, retain only a pending wake so resume cannot enqueue
        // the same continuation twice.
        p->wake_pending = 1;
        process_status_set(p, LLG_PROCESS_SUSPENDED);
    } else {
        process_status_set(p, LLG_PROCESS_RUNNING);
        enqueue_region(p, w->resume_region);
    }
}

static int wait_is_event_control(llg_wait_kind_t kind) {
    return kind == W_EVENTS || kind == W_EVENTS_INLINE || kind == W_EVENT ||
           kind == W_MIXED || kind == W_EXPR;
}

/* Whether an occurrence that would wake `w` is withheld because its process
 * is suspended on an event expression (SV 9.7 resensitization). The wait
 * stays registered; snapshots the caller keeps current make resume see only
 * later changes. */
static int wait_held_by_suspension(llg_wait_t* w) {
    llg_proc_t* p = w->proc;
    if (!p || !p->suspended || p->wait_control == LLG_WAIT_CONDITION) return 0;
    p->wait_missed = 1;
    return 1;
}

static void wake_assertion_waiter(uint64_t identity) {
    llg_wait_t* wait = g.waiters;
    while (wait) {
        llg_wait_t* next = wait->next;
        if (wait->kind == W_ASSERTION && wait->payload.rare &&
            wait->payload.rare->assertion.identity == identity)
            wake_proc(wait->proc);
        wait = next;
    }
}

static void register_wait(void) {
    llg_proc_t* p = llg_current();
    if (!p) {
        fprintf(stderr, "llg: wait requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    // A join_none group is created immediately but its children are not
    // eligible until the spawning process reaches its first blocking control.
    // Registering any real wait is that suspension boundary.  Starting all
    // pending groups here also covers nested join_none groups and wait fork.
    start_pending_fork_children(p);
    llg_wait_t* w = &p->wait;
    w->proc = p;
    p->wait_control = LLG_WAIT_CONDITION;
    if (wait_is_event_control(w->kind) || (w->kind == W_DEPS && p->next_wait_event))
        p->wait_control = p->next_wait_refresh ? LLG_WAIT_EVENT_REFRESH : LLG_WAIT_EVENT;
    p->wait_missed = 0;
    p->next_wait_event = 0;
    p->next_wait_refresh = 0;
    p->wait_refreshed = 0;
    w->next = g.waiters;
    w->prev_link = &g.waiters;
    if (w->next) w->next->prev_link = &w->next;
    g.waiters = w;
    wait_subscriptions_register(w);
    g.wait_count++;
    if (!p->suspended) process_status_set(p, LLG_PROCESS_WAITING);
}
