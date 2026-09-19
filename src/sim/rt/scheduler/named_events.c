
// ── Named events ──────────────────────────────────────────────────────────────

// Grow one event waiter table to hold at least `needed` entries. The grown
// copy is completed before it replaces the old table, so an allocation failure
// aborts without leaving a partially rebound event. Only `used` entries are
// copied; the rest of a freshly allocated table is unspecified.
static void event_table_reserve(llg_proc_t*** table, size_t* capacity,
                                int needed, int used, const char* what) {
    if (needed < 0 || (size_t)needed <= *capacity) return;
    size_t next = *capacity ? *capacity : 8u;
    while (next < (size_t)needed) {
        if (next > SIZE_MAX / 2u) {
            fprintf(stderr, "llg runtime fatal: %s capacity overflow\n", what);
            abort();
        }
        next *= 2u;
    }
    llg_proc_t** grown = (llg_proc_t**)llg_checked_malloc(
        next, sizeof(*grown), what);
    if (*table && used > 0) memcpy(grown, *table, (size_t)used * sizeof(*grown));
    free(*table);
    *table = grown;
    *capacity = next;
}

// Release any grown waiter tables and clear the event state. Generated
// init/teardown calls this; safe on a zero-initialized object and idempotent.
void llg_event_object_reset(llg_event_object_t* ev) {
    if (!ev) return;
    free(ev->waiters);
    free(ev->triggered_waiters);
    ev->waiters = NULL;
    ev->n_waiters = 0;
    ev->waiters_capacity = 0;
    ev->triggered_waiters = NULL;
    ev->n_triggered_waiters = 0;
    ev->triggered_waiters_capacity = 0;
    ev->triggered_time = 0;
    ev->triggered_generation = 0;
    ev->triggered = 0;
}

// Register `p` on `ev`'s waiter table, growing it with checked allocation.
static void event_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_waiters == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: named-event waiter count overflow\n");
        abort();
    }
    event_table_reserve(&ev->waiters, &ev->waiters_capacity,
                        ev->n_waiters + 1, ev->n_waiters,
                        "named-event waiters");
    ev->waiters[ev->n_waiters++] = p;
}

// Register a process on the persistent same-time-slot state of `ev`. This
// list is deliberately separate from ordinary event waiters: `@(ev)` remains
// edge-triggered and never observes a trigger that happened before it parked.
static void event_triggered_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_triggered_waiters == INT_MAX) {
        fprintf(stderr,
                "llg runtime fatal: named-event triggered waiter count overflow\n");
        abort();
    }
    event_table_reserve(&ev->triggered_waiters, &ev->triggered_waiters_capacity,
                        ev->n_triggered_waiters + 1, ev->n_triggered_waiters,
                        "named-event triggered waiters");
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
