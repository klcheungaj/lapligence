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
