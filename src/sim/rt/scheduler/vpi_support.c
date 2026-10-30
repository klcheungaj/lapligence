// ── VPI views of the scheduler (IEEE 1800-2009 37.76) ─────────────────────
// Read-only queries the VPI bridge (llg_vpi_runtime.c) makes on demand; no
// scheduling path calls them, so a model whose application never asks pays
// nothing.

static int pending_times_push(uint64_t** times, size_t* count, size_t* capacity,
                              uint64_t time) {
    if (*count && (*times)[*count - 1] == time) return 1;
    if (*count == *capacity) {
        size_t grown = *capacity ? *capacity * 2u : 16u;
        uint64_t* next = (uint64_t*)realloc(*times, grown * sizeof(*next));
        if (!next) return 0;
        *times = next;
        *capacity = grown;
    }
    (*times)[(*count)++] = time;
    return 1;
}

// The four time-ordered queues are merged in one pass: the smallest head
// among them is the next distinct time.
uint64_t* llg_rt_pending_times(size_t* count, int* current) {
    *count = 0;
    *current = 0;
    llg_wait_t* wait = g.timed_head;
    llg_nba_bucket_t* bucket = g.delayed_nba_buckets;
    llg_inertial_t* inertial = g.inertial_pending;
    llg_region_callback_t* callback = g.callbacks;
    for (llg_region_t region = LLG_REGION_PREPONED; region < LLG_REGION_PRE_POSTPONED_PLI;
         region++) {
        if (region_pending(region)) *current = 1;
    }
    if (g.zero_waits[LLG_REGION_INACTIVE].head || g.zero_waits[LLG_REGION_RE_INACTIVE].head ||
        nba_due(LLG_REGION_NBA) || nba_due(LLG_REGION_RE_NBA))
        *current = 1;
    uint64_t* times = NULL;
    size_t capacity = 0;
    for (;;) {
        uint64_t next = UINT64_MAX;
        int any = 0;
        if (wait) {
            next = wait->payload.timer.time;
            any = 1;
        }
        if (bucket && (!any || bucket->time < next)) {
            next = bucket->time;
            any = 1;
        }
        if (inertial && (!any || inertial->time < next)) {
            next = inertial->time;
            any = 1;
        }
        if (callback && (!any || callback->time < next)) {
            next = callback->time;
            any = 1;
        }
        if (!any) break;
        // Entries of the current slot count only when they precede the
        // read-only synchronization point (37.76 detail 3).
        int precedes = 0;
        while (wait && wait->payload.timer.time == next) {
            precedes = 1;
            wait = wait->payload.timer.next;
        }
        while (bucket && bucket->time == next) {
            precedes = 1;
            bucket = bucket->next;
        }
        while (inertial && inertial->time == next) {
            precedes = 1;
            inertial = inertial->next_pending;
        }
        while (callback && callback->time == next) {
            if (callback->region < LLG_REGION_PRE_POSTPONED_PLI) precedes = 1;
            callback = callback->next;
        }
        if (next <= g.now) {
            if (precedes) *current = 1;
            continue;
        }
        if (!pending_times_push(&times, count, &capacity, next)) {
            free(times);
            *count = 0;
            return NULL;
        }
    }
    return times;
}

// ── VPI threads (37.40) ─────────────────────────────────────────────────────

static llg_proc_t* thread_lookup(int slot, uint64_t identity) {
    if (slot < 0 || slot >= g.n_procs) return NULL;
    llg_proc_t* proc = g.all_procs[slot];
    if (!proc || proc->assertion_owner != identity || proc->completed || proc->killed)
        return NULL;
    return proc;
}

static int thread_procedural(const llg_proc_t* proc) {
    return proc->chain.root && (proc->chain.root->host_flags & LLG_DESC_PROCEDURAL) != 0;
}

int llg_rt_current_thread(uint64_t* identity) {
    llg_proc_t* proc = g.current;
    if (!proc || proc->registry_slot < 0) {
        *identity = 0;
        return -1;
    }
    *identity = proc->assertion_owner;
    return proc->registry_slot;
}

int llg_rt_thread_live(int slot, uint64_t identity) {
    llg_proc_t* proc = thread_lookup(slot, identity);
    if (!proc) return 0;
    return thread_procedural(proc) ? 2 : 1;
}

// A fork branch's parent is the process that ran the fork; a detached
// process (a join_none branch started from a function) keeps no group.
static llg_proc_t* thread_parent(const llg_proc_t* proc) {
    llg_proc_t* parent = proc->grp ? proc->grp->parent : NULL;
    if (!parent || parent->registry_slot < 0 || parent->completed || parent->killed)
        return NULL;
    return parent;
}

int llg_rt_thread_parent(int slot, uint64_t identity, int* parent_slot,
                         uint64_t* parent_identity) {
    llg_proc_t* proc = thread_lookup(slot, identity);
    llg_proc_t* parent = proc ? thread_parent(proc) : NULL;
    if (!parent || !thread_procedural(parent)) return 0;
    *parent_slot = parent->registry_slot;
    *parent_identity = parent->assertion_owner;
    return 1;
}

static int thread_identity_order(const void* left, const void* right) {
    const llg_proc_t* a = *(llg_proc_t* const*)left;
    const llg_proc_t* b = *(llg_proc_t* const*)right;
    return a->assertion_owner < b->assertion_owner ? -1
           : a->assertion_owner > b->assertion_owner ? 1
                                                      : 0;
}

// Whether `proc` is suspended on named event object `event` (alone, in an
// event list or in an expression wait).
static int thread_waits_on(const llg_proc_t* proc, const void* event) {
    const llg_wait_t* w = &proc->wait;
    llg_event_object_t* const* events = NULL;
    int count = 0;
    if (w->kind == W_EVENT) {
        events = w->payload.event.evs;
        count = w->payload.event.n_evs;
    } else if (w->kind == W_EXPR) {
        events = w->payload.expression.evs;
        count = w->payload.expression.n_evs;
    } else if (w->kind == W_MIXED && w->payload.rare) {
        events = w->payload.rare->mixed.evs;
        count = w->payload.rare->mixed.n_evs;
    }
    for (int i = 0; i < count; ++i)
        if ((const void*)events[i] == event) return 1;
    return 0;
}

typedef struct {
    const llg_proc_t* parent; // children of this thread, or NULL
    const void* event;        // threads waiting on this event object, or NULL
} thread_filter_t;

static int thread_selected(const llg_proc_t* proc, const thread_filter_t* filter) {
    if (!proc || proc->completed || proc->killed || !thread_procedural(proc)) return 0;
    if (filter->parent && thread_parent(proc) != filter->parent) return 0;
    if (filter->event && !thread_waits_on(proc, filter->event)) return 0;
    return 1;
}

static size_t thread_list(const thread_filter_t* filter, int** slots, uint64_t** identities) {
    *slots = NULL;
    *identities = NULL;
    size_t count = 0;
    for (int i = 0; i < g.n_procs; ++i)
        if (thread_selected(g.all_procs[i], filter)) ++count;
    if (!count) return 0;
    llg_proc_t** procs = (llg_proc_t**)malloc(count * sizeof(*procs));
    int* slot_out = (int*)malloc(count * sizeof(*slot_out));
    uint64_t* identity_out = (uint64_t*)malloc(count * sizeof(*identity_out));
    if (!procs || !slot_out || !identity_out) {
        free(procs);
        free(slot_out);
        free(identity_out);
        return 0;
    }
    size_t used = 0;
    for (int i = 0; i < g.n_procs && used < count; ++i)
        if (thread_selected(g.all_procs[i], filter)) procs[used++] = g.all_procs[i];
    qsort(procs, used, sizeof(*procs), thread_identity_order);
    for (size_t i = 0; i < used; ++i) {
        slot_out[i] = procs[i]->registry_slot;
        identity_out[i] = procs[i]->assertion_owner;
    }
    free(procs);
    *slots = slot_out;
    *identities = identity_out;
    return used;
}

size_t llg_rt_threads(int parent_slot, uint64_t parent_identity, int** slots,
                      uint64_t** identities) {
    thread_filter_t filter = {NULL, NULL};
    if (parent_slot >= 0) {
        filter.parent = thread_lookup(parent_slot, parent_identity);
        if (!filter.parent) {
            *slots = NULL;
            *identities = NULL;
            return 0;
        }
    }
    return thread_list(&filter, slots, identities);
}

size_t llg_rt_event_waiters(const llg_event_t* event, int** slots, uint64_t** identities) {
    thread_filter_t filter = {NULL, event ? (const void*)event->object : NULL};
    if (!filter.event) {
        *slots = NULL;
        *identities = NULL;
        return 0;
    }
    return thread_list(&filter, slots, identities);
}

void llg_rt_thread_watch_enter(int slot, uint64_t identity, int on) {
    llg_proc_t* proc = thread_lookup(slot, identity);
    if (proc) proc->vpi_enter = on ? 1u : 0u;
}
