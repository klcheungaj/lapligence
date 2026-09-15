
// ── Named events ──────────────────────────────────────────────────────────────

// Register `p` on `ev`'s waiter table (fixed capacity, like the other
// runtime resource limits).
static void event_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->waiters[ev->n_waiters++] = p;
}

// Register a process on the persistent same-time-slot state of `ev`. This
// list is deliberately separate from ordinary event waiters: `@(ev)` remains
// edge-triggered and never observes a trigger that happened before it parked.
static void event_triggered_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_triggered_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many triggered waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->triggered_waiters[ev->n_triggered_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    for (int i = 0; i < w->n_evs; i++) {
        llg_event_object_t* ev = w->evs[i];
        if (!ev) continue;
        for (int k = 0; k < ev->n_waiters; k++) {
            if (ev->waiters[k] == w->proc) {
                ev->waiters[k] = ev->waiters[ev->n_waiters - 1];
                ev->n_waiters--;
                break;
            }
        }
    }
}

static void event_triggered_unlink(llg_wait_t* w) {
    llg_event_object_t* ev = w->triggered_ev;
    if (!ev) return;
    for (int i = 0; i < ev->n_triggered_waiters; i++) {
        if (ev->triggered_waiters[i] == w->proc) {
            ev->triggered_waiters[i] =
                ev->triggered_waiters[ev->n_triggered_waiters - 1];
            ev->n_triggered_waiters--;
            break;
        }
    }
}
