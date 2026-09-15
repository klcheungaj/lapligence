
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

static void remove_waiters_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.waiters;
    while (*pp) {
        if (*pp == w) {
            *pp = w->next;
            return;
        }
        pp = &(*pp)->next;
    }
}

static void remove_timed_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->time_next;
            return;
        }
        pp = &(*pp)->time_next;
    }
}

static void insert_zero_wait(llg_wait_t* w, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for zero-delay wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    w->resume_region = region;
    w->region_next = NULL;
    llg_wait_queue_t* queue = &g.zero_waits[region];
    if (queue->tail) {
        queue->tail->region_next = w;
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
                *pp = w->region_next;
                if (queue->tail == w) {
                    queue->tail = NULL;
                    for (llg_wait_t* q = queue->head; q; q = q->region_next)
                        queue->tail = q;
                }
                w->region_next = NULL;
                return;
            }
            pp = &(*pp)->region_next;
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
// reclaimed, so a killed waiter can never consume a later put.
static void semaphore_waiter_unlink(llg_wait_t* wait) {
    if (!wait || !wait->semaphore_waiter) return;
    llg_semaphore_wait_t* node = wait->semaphore_waiter;
    llg_semaphore_t* semaphore = wait->semaphore ? wait->semaphore : node->owner;
    if (semaphore) {
        llg_semaphore_wait_t** slot = &semaphore->wait_head;
        while (*slot && *slot != node) slot = &(*slot)->next;
        if (*slot == node) {
            *slot = node->next;
            semaphore->cancelled_waiter = 1;
            if (semaphore->wait_tail == node) {
                semaphore->wait_tail = NULL;
                for (llg_semaphore_wait_t* item = semaphore->wait_head; item;
                     item = item->next)
                    semaphore->wait_tail = item;
            }
        }
    }
    free(node);
    wait->semaphore = NULL;
    wait->semaphore_waiter = NULL;
    wait->semaphore_keys = 0;
}

// Service only the head request.  A later smaller request cannot bypass a
// larger request at the front of the specified semaphore FIFO.
static void semaphore_wake_available(llg_semaphore_t* semaphore) {
    if (!semaphore) return;
    while (semaphore->wait_head) {
        llg_semaphore_wait_t* node = semaphore->wait_head;
        if (node->keys > semaphore->available) break;
        semaphore->wait_head = node->next;
        if (!semaphore->wait_head) semaphore->wait_tail = NULL;
        llg_proc_t* proc = node->proc;
        llg_wait_t* wait = proc ? &proc->wait : NULL;
        if (!proc || !wait || wait->kind != W_SEMAPHORE ||
            wait->semaphore_waiter != node) {
            free(node);
            continue;
        }
        semaphore->available -= node->keys;
        free(node);
        wait->semaphore = NULL;
        wait->semaphore_waiter = NULL;
        wait->semaphore_keys = 0;
        wake_proc(proc);
    }
}

// Finish the entire cancellation batch before granting keys. Servicing from
// unlink would let a sibling that is about to be killed consume a grant.
// Teardown only unlinks requests; it must never schedule new work.
static void semaphore_service_cancelled_waiters(void) {
    if (g.finish) return;
    for (llg_semaphore_t* semaphore = g.semaphores; semaphore;
         semaphore = semaphore->next_all) {
        if (!semaphore->cancelled_waiter) continue;
        semaphore->cancelled_waiter = 0;
        semaphore_wake_available(semaphore);
    }
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on; defined below with the other named-event helpers.
static void event_unlink(llg_wait_t* w);

static void insert_timed(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp && (*pp)->time <= w->time) pp = &(*pp)->time_next;
    w->time_next = *pp;
    *pp = w;
}

static void free_expression_wait(llg_wait_t* w) {
    if (!w->expressions) return;
    for (int i = 0; i < w->n; i++) {
        llg_frame_release((llg_frame_t*)w->expressions[i].eval_context);
        llg_frame_release((llg_frame_t*)w->expressions[i].condition_context);
        free(w->expressions[i].reads);
        free(w->expressions[i].dependencies);
    }
    free(w->expressions);
    w->expressions = NULL;
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
