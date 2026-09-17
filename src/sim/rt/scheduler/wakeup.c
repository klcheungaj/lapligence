
static int expression_qualifies(const llg_expr_event_spec_t* spec) {
    if (!spec->condition) return 1;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* result = llg_value_scope_values(scope);
    spec->condition(result, spec->condition_context);
    int qualifies = sv4_to_bool(*result);
    llg_value_scope_end(scope);
    return qualifies;
}

// Wake a suspended process: clear its wait node and schedule it.
static void wake_proc(llg_proc_t* p) {
    llg_wait_t* w = &p->wait;
    if (w->kind == W_NONE) return;
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
    free_expression_wait(w);
    free(w->specs);
    free(w->dependencies);
    sv4_destroy_array(w->last, w->last ? (size_t)w->n : 0);
        sv4_destroy(&w->level_val);
        free(w->last);
    free(w->real_last);
    free(w->evs);
    free(w->order_sequence);
    mailbox_value_destroy(&w->mailbox_value);
    llg_process_handle_t* process_target = w->process_target;
    w->specs = NULL;
    w->dependencies = NULL;
    w->last = NULL;
    w->real_last = NULL;
    w->evs = NULL;
    w->order_sequence = NULL;
    w->n = 0;
    w->n_evs = 0;
    w->triggered_ev = NULL;
    w->process_target = NULL;
    w->mailbox = NULL;
    w->mailbox_next = NULL;
    w->mailbox_peek = 0;
    memset(&w->mailbox_target, 0, sizeof(w->mailbox_target));
    w->n_order = 0;
    w->order_next = 0;
    w->assertion_identity = 0;
    w->semaphore_keys = 0;
    w->kind = W_NONE;
    g.wait_count--;
    if (process_target) llg_process_release(process_target);
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

static void wake_assertion_waiter(uint64_t identity) {
    llg_wait_t* wait = g.waiters;
    while (wait) {
        llg_wait_t* next = wait->next;
        if (wait->kind == W_ASSERTION && wait->assertion_identity == identity)
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
    w->next = g.waiters;
    g.waiters = w;
    g.wait_count++;
    if (!p->suspended) process_status_set(p, LLG_PROCESS_WAITING);
}
