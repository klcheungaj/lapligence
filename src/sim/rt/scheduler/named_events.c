
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

// Slot arrays share the event-list allocation; a single ordinary event uses
// the existing inline payload. Positions survive waiter-table reallocations.
static llg_event_object_t** event_wait_list_new(int count, int** slots) {
    if (!count) {
        *slots = NULL;
        return NULL;
    }
    llg_event_object_t** events = (llg_event_object_t**)llg_checked_malloc(
        (size_t)count, sizeof(*events) + sizeof(**slots), "indexed event wait list");
    *slots = (int*)(events + count);
    for (int i = 0; i < count; i++) (*slots)[i] = -1;
    return events;
}

static int* event_wait_slot(llg_wait_t* w, llg_event_object_t* ev, int index) {
    llg_event_object_t** events = NULL;
    int* slots = NULL;
    int count = 0;
    switch (w->kind) {
        case W_EVENT:
            events = w->payload.event.evs;
            slots = w->payload.event.event_slots;
            count = w->payload.event.n_evs;
            break;
        case W_EXPR:
            events = w->payload.expression.evs;
            slots = w->payload.expression.event_slots;
            count = w->payload.expression.n_evs;
            break;
        case W_MIXED:
            events = w->payload.rare->mixed.evs;
            slots = w->payload.rare->mixed.event_slots;
            count = w->payload.rare->mixed.n_evs;
            break;
        case W_EVENT_ORDER:
            events = w->payload.rare->order.evs;
            slots = w->payload.rare->order.event_slots;
            count = w->payload.rare->order.n_evs;
            break;
        default:
            return NULL;
    }
    for (int i = 0; i < count; i++)
        if (events[i] == ev && slots[i] == index) return &slots[i];
    return NULL;
}

// Register `p` on `ev`'s waiter table, growing it with checked allocation.
static void event_list_add(llg_event_object_t* ev, llg_proc_t* p, int* slot) {
    if (!ev) return;
    if (ev->n_waiters == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: named-event waiter count overflow\n");
        abort();
    }
    event_table_reserve(&ev->waiters, &ev->waiters_capacity,
                        ev->n_waiters + 1, ev->n_waiters,
                        "named-event waiters");
    *slot = ev->n_waiters;
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
    p->wait.payload.event.inline_slot = ev->n_triggered_waiters;
    ev->triggered_waiters[ev->n_triggered_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    llg_event_object_t** events = NULL;
    int* slots = NULL;
    int count = 0;
    if (w->kind == W_EVENT) {
        events = w->payload.event.evs;
        slots = w->payload.event.event_slots;
        count = w->payload.event.n_evs;
    } else if (w->kind == W_EXPR) {
        events = w->payload.expression.evs;
        slots = w->payload.expression.event_slots;
        count = w->payload.expression.n_evs;
    } else if (w->kind == W_MIXED && w->payload.rare) {
        events = w->payload.rare->mixed.evs;
        slots = w->payload.rare->mixed.event_slots;
        count = w->payload.rare->mixed.n_evs;
    } else if (w->kind == W_EVENT_ORDER && w->payload.rare) {
        events = w->payload.rare->order.evs;
        slots = w->payload.rare->order.event_slots;
        count = w->payload.rare->order.n_evs;
    }
    for (int i = 0; i < count; i++) {
        llg_event_object_t* ev = events[i];
        if (!ev) continue;
        int index = slots[i];
        if (index < 0) continue; // already detached into a trigger snapshot
        int last = --ev->n_waiters;
        if (index != last) {
            llg_proc_t* moved = ev->waiters[last];
            int* moved_slot = event_wait_slot(&moved->wait, ev, last);
            if (!moved_slot) abort();
            *moved_slot = index;
            ev->waiters[index] = moved;
        }
        slots[i] = -1;
    }
}

static void event_triggered_unlink(llg_wait_t* w) {
    if (!w || w->kind != W_EVENT_TRIGGERED) return;
    llg_event_object_t* ev = w->payload.event.triggered_ev;
    if (!ev) return;
    int index = w->payload.event.inline_slot;
    if (index < 0) return;
    int last = --ev->n_triggered_waiters;
    if (index != last) {
        llg_proc_t* moved = ev->triggered_waiters[last];
        ev->triggered_waiters[index] = moved;
        moved->wait.payload.event.inline_slot = index;
    }
    w->payload.event.inline_slot = -1;
}
