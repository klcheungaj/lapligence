
static void enqueue_region(llg_proc_t* p, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for process\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (p->queued) return;
    p->region = region;
    p->next_region = NULL;
    p->queued = 1;
    llg_proc_queue_t* queue = &g.process_queues[region];
    if (queue->tail) {
        queue->tail->next_region = p;
        queue->tail = p;
    } else {
        queue->head = queue->tail = p;
    }
}

// Source rows are stable across rehash. Both hash-chain and subscriber-list
// links have back-links, so unlink never searches either list. Empty rows are
// reclaimed immediately; address churn cannot accumulate historical sources.
#define LLG_WAIT_SOURCES_INITIAL 16u

static size_t wait_source_hash(const void* key) {
    uint64_t hash = (uint64_t)(uintptr_t)key;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void wait_sources_reserve(void) {
    size_t capacity = g.wait_sources_capacity;
    if (capacity && g.n_wait_sources < capacity - capacity / 4) return;
    if (capacity > SIZE_MAX / 2) {
        fputs("llg runtime fatal: wait source index capacity overflow\n", stderr);
        abort();
    }
    capacity = capacity ? capacity * 2 : LLG_WAIT_SOURCES_INITIAL;
    llg_wait_source_t** table = (llg_wait_source_t**)llg_checked_calloc(
        capacity, sizeof(*table), "wait source index");
    for (size_t i = 0; i < g.wait_sources_capacity; i++) {
        llg_wait_source_t* source = g.wait_sources[i];
        while (source) {
            llg_wait_source_t* next = source->next;
            size_t slot = wait_source_hash(source->key) & (capacity - 1);
            source->next = table[slot];
            source->prev_link = &table[slot];
            if (source->next) source->next->prev_link = &source->next;
            table[slot] = source;
            source = next;
        }
    }
    free(g.wait_sources);
    g.wait_sources = table;
    g.wait_sources_capacity = capacity;
}

static llg_wait_source_t* wait_source_find(const void* key) {
    if (!key || !g.n_wait_sources) return NULL;
    size_t slot = wait_source_hash(key) & (g.wait_sources_capacity - 1);
    for (llg_wait_source_t* source = g.wait_sources[slot]; source;
         source = source->next)
        if (source->key == key) return source;
    return NULL;
}

static llg_wait_source_t* wait_source_get(const void* key) {
    llg_wait_source_t* source = wait_source_find(key);
    if (source) return source;
    wait_sources_reserve();
    size_t slot = wait_source_hash(key) & (g.wait_sources_capacity - 1);
    source = (llg_wait_source_t*)llg_checked_calloc(
        1, sizeof(*source), "wait source");
    source->key = key;
    source->next = g.wait_sources[slot];
    source->prev_link = &g.wait_sources[slot];
    if (source->next) source->next->prev_link = &source->next;
    g.wait_sources[slot] = source;
    g.n_wait_sources++;
    return source;
}

static void wait_subscription_insert(llg_wait_subscription_t* node,
                                      llg_wait_subscription_t** slot) {
    node->next = *slot;
    node->prev_link = slot;
    if (node->next) node->next->prev_link = &node->next;
    *slot = node;
}

static void wait_subscription_unlink(llg_wait_subscription_t* node) {
    *node->prev_link = node->next;
    if (node->next) node->next->prev_link = node->prev_link;
    node->prev_link = NULL;
    llg_wait_source_t* source = node->source;
    if (!source->head) {
        *source->prev_link = source->next;
        if (source->next) source->next->prev_link = source->prev_link;
        g.n_wait_sources--;
        free(source);
    }
}

// A stack-owned cursor is a subscriber with no waiter. Moving it past the
// current subscriber before callbacks makes arbitrary removal (including the
// next subscriber) safe, without snapshots or a scan of active iterators.
// Nested publications skip other cursors; rows stay alive until cursors leave.
static llg_wait_t* wait_source_next(llg_wait_subscription_t* cursor) {
    llg_wait_subscription_t* node = cursor->next;
    while (node && !node->wait) node = node->next;
    if (!node) return NULL;
    *cursor->prev_link = cursor->next;
    if (cursor->next) cursor->next->prev_link = cursor->prev_link;
    wait_subscription_insert(cursor, &node->next);
    return node->wait;
}

// Value-change watches. The hook is process-global so that it survives the
// `g` reset of llg_rt_cleanup; watches themselves end with the runtime
// lifetime (free_value_watches).
static llg_value_watch_fn value_watch_hook;

void llg_value_watch_hook(llg_value_watch_fn hook) { value_watch_hook = hook; }

int llg_value_watch_add(const void* key) {
    if (!key) return 0;
    llg_wait_source_t* source = wait_source_get(key);
    if (!source->watch) {
        llg_wait_subscription_t* node = (llg_wait_subscription_t*)llg_checked_calloc(
            1, sizeof(*node), "value watch");
        node->source = source;
        wait_subscription_insert(node, &source->head);
        source->watch = node;
    }
    if (source->watchers == UINT32_MAX) {
        fputs("llg runtime fatal: value watch count overflow\n", stderr);
        abort();
    }
    source->watchers++;
    return 1;
}

static void value_watch_drop(llg_wait_source_t* source) {
    llg_wait_subscription_t* node = source->watch;
    source->watch = NULL;
    source->watchers = 0;
    // Unlinking the last subscriber frees the row itself.
    wait_subscription_unlink(node);
    free(node);
}

void llg_value_watch_remove(const void* key) {
    // A watch from an earlier runtime lifetime is already gone.
    llg_wait_source_t* source = wait_source_find(key);
    if (!source || !source->watchers) return;
    if (--source->watchers == 0) value_watch_drop(source);
}

static void free_value_watches(void) {
    for (size_t slot = 0; slot < g.wait_sources_capacity; slot++) {
        llg_wait_source_t* source = g.wait_sources[slot];
        while (source) {
            llg_wait_source_t* next = source->next;
            if (source->watch) value_watch_drop(source);
            source = next;
        }
    }
}

typedef struct {
    const void** keys;
    size_t count;
    size_t capacity;
} llg_wait_keys_t;

static void wait_key_add(llg_wait_keys_t* keys, const void* key) {
    if (!key) return;
    if (keys->count == keys->capacity) {
        if (keys->capacity > SIZE_MAX / 2) {
            fputs("llg runtime fatal: wait dependency count overflow\n", stderr);
            abort();
        }
        size_t capacity = keys->capacity ? keys->capacity * 2 : 8;
        const void** grown = (const void**)llg_checked_malloc(
            capacity, sizeof(*grown), "wait dependency keys");
        if (keys->count) memcpy(grown, keys->keys, keys->count * sizeof(*grown));
        free(keys->keys);
        keys->keys = grown;
        keys->capacity = capacity;
    }
    keys->keys[keys->count++] = key;
}

static void wait_dependency_key_add(llg_wait_keys_t* keys,
                                    const llg_wait_dependency_t* dependency) {
    wait_key_add(keys, dependency->sig ? (const void*)dependency->sig
                                       : (const void*)dependency->real);
}

static int wait_key_compare(const void* left, const void* right) {
    uintptr_t a = (uintptr_t)*(const void* const*)left;
    uintptr_t b = (uintptr_t)*(const void* const*)right;
    return (a > b) - (a < b);
}

static int wait_inline_fits(const sv4_t* value) {
    return llg_sv4_width(*value) <= 64u * LLG_WAIT_INLINE_LIMBS;
}

static void wait_inline_copy(llg_wait_t* wait, const sv4_t* value) {
    llg_wait_inline_payload_t* single = &wait->payload.single;
    single->width = llg_sv4_width(*value);
    single->is_signed = (int8_t)(llg_sv4_signed(*value) != 0);
    llg_sv4_export_vpi_words(*value, 0, single->words, LLG_WAIT_INLINE_LIMBS);
}

static int wait_inline_same(llg_wait_t* wait, sv4_t value) {
    return llg_sv4_same_vpi_words(value, wait->payload.single.words,
                                 LLG_WAIT_INLINE_LIMBS);
}

// The retained record whose dependency list this wait borrows, if any.
static llg_wait_retained_t* wait_retained_of(const llg_wait_t* wait) {
    if (wait->kind != W_DEPS || !wait->proc) return NULL;
    llg_wait_retained_t* retained = wait->proc->retained_wait;
    return retained && retained->dependencies == wait->payload.expression.dependencies
               ? retained
               : NULL;
}

// Re-arm retained memberships in their original registration order, each at
// the head of its source list as a fresh registration would place it.
static void wait_retained_relink(llg_wait_t* wait, llg_wait_retained_t* retained) {
    for (size_t i = 0; i < retained->n_subscriptions; i++) {
        llg_wait_subscription_t* node = &retained->subscriptions[i];
        if (node->source->head != node) {
            *node->prev_link = node->next;
            if (node->next) node->next->prev_link = node->prev_link;
            wait_subscription_insert(node, &node->source->head);
        }
        node->wait = wait;
    }
    wait->subscriptions = retained->subscriptions;
    wait->n_subscriptions = retained->n_subscriptions;
}

// Release a process's retained dependency wait. Its memberships are dormant:
// callers drop it only while the process is not waiting on it.
static void wait_retained_drop(llg_proc_t* proc) {
    llg_wait_retained_t* retained = proc ? proc->retained_wait : NULL;
    if (!retained) return;
    proc->retained_wait = NULL;
    for (size_t i = 0; i < retained->n_subscriptions; i++)
        wait_subscription_unlink(&retained->subscriptions[i]);
    free(retained->subscriptions);
    sv4_destroy_array(retained->last, (size_t)retained->n);
    free(retained->last);
    free(retained->dependencies);
    free(retained);
}

static void wait_subscriptions_register(llg_wait_t* wait) {
    llg_wait_retained_t* retained = wait_retained_of(wait);
    if (retained && retained->subscriptions) {
        wait_retained_relink(wait, retained);
        return;
    }
    if (wait->kind == W_EVENTS_INLINE || wait->kind == W_LEVEL_INLINE) {
        const void* key = wait->payload.single.specs[0].sig;
        if (!key) return;
        llg_wait_subscription_t* node = &wait->payload.single.subscription;
        wait->subscriptions = node;
        wait->n_subscriptions = 1;
        node->source = wait_source_get(key);
        node->wait = wait;
        wait_subscription_insert(node, &node->source->head);
        return;
    }
    llg_wait_keys_t keys = {0};
    if (wait->kind == W_EVENTS) {
        for (int i = 0; i < wait->payload.expression.n; i++)
            wait_key_add(&keys, wait->payload.expression.specs[i].sig);
    } else if (wait->kind == W_DEPS) {
        for (int i = 0; i < wait->payload.expression.n; i++)
            wait_dependency_key_add(&keys, &wait->payload.expression.dependencies[i]);
    } else if (wait->kind == W_MIXED) {
        for (int i = 0; i < wait->payload.rare->mixed.n; i++)
            wait_key_add(&keys, wait->payload.rare->mixed.specs[i].sig);
    } else if (wait->kind == W_PROCESS) {
        wait_key_add(&keys, wait->payload.rare->process.target);
    } else if (wait->kind == W_LEVEL) {
        wait_key_add(&keys, wait->payload.rare->level.sig);
    } else if (wait->kind == W_EXPR) {
        for (int i = 0; i < wait->payload.expression.n; i++) {
            const llg_expr_event_spec_t* spec = &wait->payload.expression.expressions[i];
            if (spec->event) continue;
            wait_key_add(&keys, spec->sig);
            wait_key_add(&keys, spec->real_sig);
            if (spec->n_dependencies > 0) {
                for (int j = 0; j < spec->n_dependencies; j++)
                    wait_dependency_key_add(&keys, &spec->dependencies[j]);
            } else {
                for (int j = 0; j < spec->n_reads; j++)
                    wait_key_add(&keys, spec->reads[j]);
            }
        }
    }
    if (!keys.count) return;
    qsort(keys.keys, keys.count, sizeof(*keys.keys), wait_key_compare);
    size_t unique = 0;
    for (size_t i = 0; i < keys.count; i++)
        if (!unique || keys.keys[i] != keys.keys[unique - 1])
            keys.keys[unique++] = keys.keys[i];
    wait->subscriptions = (llg_wait_subscription_t*)llg_checked_calloc(
        unique, sizeof(*wait->subscriptions), "wait subscriptions");
    wait->n_subscriptions = unique;
    for (size_t i = 0; i < unique; i++) {
        llg_wait_subscription_t* node = &wait->subscriptions[i];
        node->source = wait_source_get(keys.keys[i]);
        node->wait = wait;
        // Head insertion matches the global list's reverse registration order.
        wait_subscription_insert(node, &node->source->head);
    }
    free(keys.keys);
    if (retained) {
        retained->subscriptions = wait->subscriptions;
        retained->n_subscriptions = wait->n_subscriptions;
    }
}

static void wait_inline_promote(llg_wait_t* wait, const sv4_t* value) {
    llg_event_spec_t spec = wait->payload.single.specs[0];
    llg_wait_subscription_t* old = &wait->payload.single.subscription;
    llg_wait_subscription_t* node = (llg_wait_subscription_t*)llg_checked_malloc(
        1, sizeof(*node), "wait subscriptions");
    *node = *old;
    *node->prev_link = node;
    if (node->next) node->next->prev_link = &node->next;
    wait->subscriptions = node;
    memset(&wait->payload, 0, sizeof(wait->payload));
    wait->kind = W_EVENTS;
    llg_wait_expression_payload_t* payload = &wait->payload.expression;
    payload->n = 1;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        1, sizeof(*payload->specs), "edge wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        1, sizeof(*payload->last), "edge wait snapshots");
    payload->specs[0] = spec;
    payload->last[0] = sv4_clone(value);
}

static void remove_waiters_entry(llg_wait_t* w) {
    if (w->prev_link) {
        *w->prev_link = w->next;
        if (w->next) w->next->prev_link = w->prev_link;
        w->prev_link = NULL;
    }
    llg_wait_retained_t* retained = wait_retained_of(w);
    if (retained && w->subscriptions == retained->subscriptions) {
        // Dormant until the next re-arm or wait_retained_drop.
        for (size_t i = 0; i < w->n_subscriptions; i++)
            w->subscriptions[i].wait = NULL;
        w->subscriptions = NULL;
        w->n_subscriptions = 0;
        return;
    }
    for (size_t i = 0; i < w->n_subscriptions; i++)
        wait_subscription_unlink(&w->subscriptions[i]);
    if (w->subscriptions != &w->payload.single.subscription)
        free(w->subscriptions);
    w->subscriptions = NULL;
    w->n_subscriptions = 0;
}

static void remove_timed_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->payload.timer.next;
            return;
        }
        pp = &(*pp)->payload.timer.next;
    }
}

static llg_wait_rare_t* wait_rare_allocate(llg_wait_t* wait,
                                           const char* description) {
    llg_wait_rare_t* payload = (llg_wait_rare_t*)llg_checked_calloc(
        1, sizeof(*payload), description);
    wait->payload.rare = payload;
    return payload;
}

static void insert_zero_wait(llg_wait_t* w, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for zero-delay wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    w->resume_region = region;
    w->payload.timer.region_next = NULL;
    llg_wait_queue_t* queue = &g.zero_waits[region];
    if (queue->tail) {
        queue->tail->payload.timer.region_next = w;
    } else {
        queue->head = w;
    }
    queue->tail = w;
}

static void remove_zero_wait_entry(llg_wait_t* w) {
    for (int i = 0; i < LLG_REGION_COUNT; i++) {
        llg_wait_queue_t* queue = &g.zero_waits[i];
        llg_wait_t** pp = &queue->head;
        while (*pp) {
            if (*pp == w) {
                *pp = w->payload.timer.region_next;
                if (queue->tail == w) {
                    queue->tail = NULL;
                    for (llg_wait_t* q = queue->head; q; q = q->payload.timer.region_next)
                        queue->tail = q;
                }
                w->payload.timer.region_next = NULL;
                return;
            }
            pp = &(*pp)->payload.timer.region_next;
        }
    }
}

static llg_proc_t* dequeue_region(llg_region_t region) {
    llg_proc_queue_t* queue = &g.process_queues[region];
    llg_proc_t* p = queue->head;
    if (!p) return NULL;
    queue->head = p->next_region;
    if (!queue->head) queue->tail = NULL;
    p->next_region = NULL;
    p->queued = 0;
    return p;
}

static void remove_region_entry(llg_proc_t* p) {
    for (int i = 0; i < LLG_REGION_COUNT; i++) {
        llg_proc_queue_t* queue = &g.process_queues[i];
        llg_proc_t** pp = &queue->head;
        while (*pp) {
            if (*pp == p) {
                *pp = p->next_region;
                if (queue->tail == p) {
                    queue->tail = NULL;
                    for (llg_proc_t* q = queue->head; q; q = q->next_region)
                        queue->tail = q;
                }
                p->next_region = NULL;
                p->queued = 0;
                return;
            }
            pp = &(*pp)->next_region;
        }
    }
}

static void remove_inactive_entry(llg_wait_t* w) {
    remove_zero_wait_entry(w);
}

static int semaphore_key_count(sv4_t value, uint64_t* result) {
    int64_t signed_value = 0;
    if (!result || !sv4_to_index_i64(value, &signed_value) || signed_value < 0) {
        fprintf(stderr,
                "llg: semaphore key count must be a known nonnegative integral value\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    *result = (uint64_t)signed_value;
    return 1;
}

static int semaphore_valid(llg_semaphore_t* semaphore, const char* action) {
    if (semaphore) return 1;
    fprintf(stderr, "llg: semaphore %s requires a live semaphore handle\n", action);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

// Remove a blocked get without changing the semaphore's available keys.  This
// is used by process cancellation and teardown before the process storage is
// reclaimed, so a killed waiter can never consume a later put. Removing a
// waiter never lets another one proceed: every request still queued after a
// service needs more keys than are available.
static void semaphore_waiter_unlink(llg_wait_t* wait) {
    if (!wait || wait->kind != W_SEMAPHORE || !wait->payload.rare ||
        !wait->payload.rare->semaphore.waiter)
        return;
    llg_wait_semaphore_payload_t* payload = &wait->payload.rare->semaphore;
    llg_semaphore_wait_t* node = payload->waiter;
    llg_semaphore_t* semaphore = payload->semaphore ? payload->semaphore : node->owner;
    if (semaphore) {
        llg_semaphore_wait_t* previous = NULL;
        llg_semaphore_wait_t** slot = &semaphore->wait_head;
        while (*slot && *slot != node) {
            previous = *slot;
            slot = &(*slot)->next;
        }
        if (*slot == node) {
            *slot = node->next;
            if (semaphore->wait_tail == node) semaphore->wait_tail = previous;
        }
    }
    free(node);
    payload->semaphore = NULL;
    payload->waiter = NULL;
    payload->keys = 0;
}

/* Grant keys to every queued request that fits, in arrival order (SV 15.3.3:
 * the queue is FIFO and preserves arrival order). A request that needs more
 * keys than are available does not block a later, smaller one (decision
 * AB-N4). A suspended waiter stays a candidate (AB-N1): it takes its keys
 * while suspended and continues when resumed. Keys are taken at the grant,
 * so a waiter killed before it runs does not return them. */
static void semaphore_wake_available(llg_semaphore_t* semaphore) {
    if (!semaphore) return;
    llg_semaphore_wait_t* previous = NULL;
    llg_semaphore_wait_t* node = semaphore->wait_head;
    while (node && semaphore->available > 0) {
        llg_semaphore_wait_t* next = node->next;
        llg_proc_t* proc = node->proc;
        llg_wait_t* wait = proc ? &proc->wait : NULL;
        int stale = !proc || !wait || wait->kind != W_SEMAPHORE ||
                    !wait->payload.rare || wait->payload.rare->semaphore.waiter != node;
        if (!stale && node->keys > semaphore->available) {
            previous = node;
            node = next;
            continue;
        }
        if (previous)
            previous->next = next;
        else
            semaphore->wait_head = next;
        if (semaphore->wait_tail == node) semaphore->wait_tail = previous;
        if (!stale) {
            semaphore->available -= node->keys;
            wait->payload.rare->semaphore.semaphore = NULL;
            wait->payload.rare->semaphore.waiter = NULL;
            wait->payload.rare->semaphore.keys = 0;
        }
        free(node);
        if (!stale) wake_proc(proc);
        node = next;
    }
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on; defined below with the other named-event helpers.
static void event_unlink(llg_wait_t* w);

static void insert_timed(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp && (*pp)->payload.timer.time <= w->payload.timer.time)
        pp = &(*pp)->payload.timer.next;
    w->payload.timer.next = *pp;
    *pp = w;
}

static void free_expression_wait(llg_wait_t* w) {
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    if (!payload->expressions) return;
    for (int i = 0; i < payload->n; i++) {
        llg_frame_release((llg_frame_t*)payload->expressions[i].eval_context);
        llg_frame_release((llg_frame_t*)payload->expressions[i].condition_context);
        free(payload->expressions[i].reads);
        free(payload->expressions[i].dependencies);
    }
    free(payload->expressions);
    payload->expressions = NULL;
}

static void wait_payload_release(llg_wait_t* wait) {
    if (!wait) return;
    llg_process_handle_t* process_target = NULL;
    switch (wait->kind) {
        case W_EVENTS_INLINE:
        case W_LEVEL_INLINE:
            break;
        case W_EVENTS:
            free(wait->payload.expression.specs);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            break;
        case W_DEPS:
            if (wait_retained_of(wait)) break;
            free(wait->payload.expression.dependencies);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            break;
        case W_EXPR:
            free_expression_wait(wait);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            free(wait->payload.expression.real_last);
            free(wait->payload.expression.evs);
            break;
        case W_EVENT:
            if (wait->payload.event.evs != &wait->payload.event.inline_ev)
                free(wait->payload.event.evs);
            break;
        case W_MIXED:
            if (wait->payload.rare) {
                llg_wait_mixed_payload_t* mixed = &wait->payload.rare->mixed;
                free(mixed->specs);
                sv4_destroy_array(mixed->last,
                                  mixed->last ? (size_t)mixed->n : 0);
                free(mixed->last);
                free(mixed->evs);
            }
            break;
        case W_EVENT_ORDER:
            if (wait->payload.rare) {
                free(wait->payload.rare->order.evs);
                free(wait->payload.rare->order.sequence);
            }
            break;
        case W_LEVEL:
            if (wait->payload.rare)
                sv4_destroy(&wait->payload.rare->level.value);
            break;
        case W_PROCESS:
            if (wait->payload.rare)
                process_target = wait->payload.rare->process.target;
            break;
        case W_MAILBOX_PUT:
            if (wait->payload.rare)
                mailbox_value_destroy(&wait->payload.rare->mailbox_put.value);
            break;
        default:
            break;
    }
    if (wait->kind != W_TIME && wait->kind != W_EVENTS &&
        wait->kind != W_DEPS && wait->kind != W_EXPR &&
        wait->kind != W_EVENT && wait->kind != W_EVENT_TRIGGERED &&
        wait->kind != W_EVENTS_INLINE && wait->kind != W_LEVEL_INLINE &&
        wait->kind != W_NONE)
        free(wait->payload.rare);
    memset(&wait->payload, 0, sizeof(wait->payload));
    wait->kind = W_NONE;
    if (process_target) llg_process_release(process_target);
}

static void release_expression_contexts(const llg_expr_event_spec_t* specs, int n) {
    if (!specs) return;
    for (int i = 0; i < n; i++) {
        llg_frame_release((llg_frame_t*)specs[i].eval_context);
        llg_frame_release((llg_frame_t*)specs[i].condition_context);
    }
}

static void free_deferred_trigger(llg_deferred_trigger_t* trigger) {
    if (!trigger) return;
    for (int i = 0; i < trigger->n; i++) {
        llg_frame_release((llg_frame_t*)trigger->specs[i].eval_context);
        llg_frame_release((llg_frame_t*)trigger->specs[i].condition_context);
        free(trigger->specs[i].reads);
        free(trigger->specs[i].dependencies);
    }
    free(trigger->specs);
    sv4_destroy_array(trigger->last, trigger->last ? (size_t)trigger->n : 0);
    free(trigger->last);
    free(trigger->real_last);
    if (trigger->action_frame) llg_frame_release(trigger->action_frame);
    free(trigger);
}

static void free_deferred_triggers(void) {
    while (g.deferred_triggers) {
        llg_deferred_trigger_t* next = g.deferred_triggers->next;
        free_deferred_trigger(g.deferred_triggers);
        g.deferred_triggers = next;
    }
    g.deferred_trigger_tail = NULL;
}
