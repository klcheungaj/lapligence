
// ── fork/join (coroutine children) ────────────────────────────────────────────

static void llg_kill_proc_tree(llg_proc_t* p); // mutual recursion below

static void process_handle_terminal(llg_proc_t* proc, int status) {
    llg_process_handle_t* handle = proc ? proc->handle : NULL;
    if (!handle) return;
    proc->handle = NULL;
    proc->status = status;
    handle->proc = NULL;
    handle->status = status;
    // Process-await registrations use the same address index as signal waits.
    // Head insertion retains the previous global-list wake order.
    llg_wait_source_t* source = wait_source_find(handle);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
        llg_wait_t* wait;
        while ((wait = wait_source_next(&cursor)) != NULL)
            wake_proc(wait->proc);
        wait_subscription_unlink(&cursor);
    }
    // Drop the process-owned reference after all awaiters have been woken;
    // each awaiter holds its own reference until wake/cancellation.
    llg_process_release(handle);
}

static void process_handle_shutdown(llg_proc_t* proc) {
    llg_process_handle_t* handle = proc ? proc->handle : NULL;
    if (!handle) return;
    proc->handle = NULL;
    proc->status = handle->status == LLG_PROCESS_FINISHED
                       ? LLG_PROCESS_FINISHED
                       : LLG_PROCESS_KILLED;
    handle->proc = NULL;
    if (handle->status != LLG_PROCESS_FINISHED)
        handle->status = LLG_PROCESS_KILLED;
    llg_process_release(handle);
}

static void release_killed_proc_resources(llg_proc_t* proc) {
    value_scopes_unwind(proc);
    activation_unwind_proc(proc);
    llg_frame_release(proc->frame);
    proc->frame = NULL;
    process_local_release_all(proc);
}

// Unlink a suspended or queued proc from every scheduler queue, free its
// pending NBA list, and retire its storage. A named disable may cancel the
// executing coroutine; its destruction is deferred until the scheduler resumes.
static void llg_kill_proc(llg_proc_t* p, int notify_parent) {
    if (!p || p->killed) return;
    p->killed = 1;
    release_program_process(p);
    p->suspended = 0;
    p->wake_pending = 0;
    cancel_proc_nbas(p);

    llg_wait_t* w = &p->wait;
    if (w->kind != W_NONE) {
        if (w->kind == W_ASSERTION) {
            for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
                 assertion = assertion->next) {
                if (assertion->identity == w->payload.rare->assertion.identity &&
                    assertion->kind == LLG_ASSERTION_EXPECT)
                    assertion->expect_active = 0;
            }
        }
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
    }
    remove_region_entry(p);

    llg_fork_group_t* parent_group = p->grp;
    if (parent_group) {
        for (llg_fork_child_t* child = parent_group->children; child;
             child = child->next) {
            if (child->proc == p) {
                child->proc = NULL;
                break;
            }
        }
        p->grp = NULL;
    }
    // A process may kill itself indirectly by killing an ancestor or its
    // program origin. Keep its live owners intact until generated code has
    // returned through the active C call chain; the scheduler reaps them at
    // the first safe boundary below.
    if (p != llg_current()) release_killed_proc_resources(p);
    process_handle_terminal(p, LLG_PROCESS_KILLED);
    if (notify_parent && parent_group && !parent_group->terminal)
        llg_fork_group_child_done(parent_group);
    unregister_proc(p);
    p->next_retired = g.retired_procs;
    g.retired_procs = p;
}

// The active coroutine's frames must survive until llg_co_run returns to the
// scheduler. Other cancelled processes can be reclaimed after cancellation
// traversal, including within a long-running caller.
static void reap_retired_procs(void) {
    llg_proc_t* current = llg_current();
    llg_proc_t** slot = &g.retired_procs;
    while (*slot) {
        llg_proc_t* proc = *slot;
        if (proc == current) {
            slot = &proc->next_retired;
            continue;
        }
        *slot = proc->next_retired;
        release_killed_proc_resources(proc);
        free_proc_record(proc);
    }
}

// Kill every group spawned by `p`: each child (and its descendants) is freed
// recursively, the group structs go onto the zombie list for teardown.  `p`
// itself is untouched — used by disable_fork, which kills only descendants.
static void llg_kill_proc_groups(llg_proc_t* p) {
    llg_fork_group_t* grp = p->fork_groups;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            if (c->proc) {
                llg_kill_proc_tree_internal(c->proc, 0);
                c->proc = NULL; // freed inline; teardown skips it
            }
            c = next_c;
        }
        grp->remaining = 0;
        grp->terminal = 1;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        grp = next_g;
    }
    p->fork_groups = NULL;
}

// Kill `p` and all of its descendants.
static void llg_kill_proc_tree(llg_proc_t* p) {
    llg_kill_proc_tree_internal(p, 1);
}

static void llg_kill_proc_tree_internal(llg_proc_t* p, int notify_parent) {
    llg_kill_proc_groups(p);
    llg_kill_proc(p, notify_parent);
}

llg_process_handle_t* llg_process_self(llg_proc_t* self) {
    llg_runtime_service_enter(self, "process::self");
    return self ? self->handle : NULL;
}

int llg_process_status(const llg_process_handle_t* handle) {
    return handle ? handle->status : LLG_PROCESS_KILLED;
}

void llg_process_retain(llg_process_handle_t* handle) {
    if (!handle) return;
    if (handle->refs == SIZE_MAX) {
        fprintf(stderr, "llg: process handle reference count overflow\n");
        abort();
    }
    handle->refs++;
}

void llg_process_release(llg_process_handle_t* handle) {
    if (!handle) return;
    if (handle->refs == 0) {
        fprintf(stderr, "llg: process handle reference count underflow\n");
        abort();
    }
    handle->refs--;
    if (handle->refs != 0) return;
    process_handle_unlink(handle);
    free(handle);
}

static llg_process_local_ref_t* process_local_find(llg_proc_t* proc,
                                                    llg_process_handle_t** slot) {
    for (llg_process_local_ref_t* local = proc ? proc->process_locals : NULL;
         local; local = local->next) {
        if (local->slot == slot) return local;
    }
    return NULL;
}

void llg_process_local_register(llg_process_handle_t** slot) {
    llg_proc_t* proc = llg_current();
    if (!proc || !slot || !region_can_mutate("process local registration")) return;
    llg_process_local_ref_t* local = process_local_find(proc, slot);
    if (local) {
        // A repeated declaration is a fresh automatic lifetime (for example,
        // an always-loop iteration). Drop the previous reference and clear
        // the caller's slot before a declaration initializer assigns again.
        llg_process_release(local->value);
        local->value = NULL;
        *slot = NULL;
        return;
    }
    local = (llg_process_local_ref_t*)llg_checked_calloc(
        1, sizeof(*local), "process local reference");
    local->slot = slot;
    local->next = proc->process_locals;
    proc->process_locals = local;
}

static void process_local_release_all(llg_proc_t* proc) {
    while (proc && proc->process_locals) {
        llg_process_local_ref_t* local = proc->process_locals;
        proc->process_locals = local->next;
        llg_process_release(local->value);
        free(local);
    }
}

void llg_process_assign(llg_process_handle_t** target,
                        llg_process_handle_t* source) {
    if (!target || !region_can_mutate("process handle write")) return;
    if (source) llg_process_retain(source);
    if (*target) llg_process_release(*target);
    *target = source;
    llg_proc_t* proc = llg_current();
    llg_process_local_ref_t* local = process_local_find(proc, target);
    if (local) local->value = source;
}

void llg_process_kill(llg_proc_t* self, llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::kill");
    if (!handle || !handle->proc || !region_can_mutate("process control")) return;
    llg_proc_t* target = handle->proc;
    llg_kill_proc_tree(target);
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    if (self && self->killed)
        self->chain.exiting = LLG_EXIT_ABANDON;
    else if (g.finish && self)
        self->chain.exiting = LLG_EXIT_COMPLETE;
}

llg_co_arm_t llg_arm_process_suspend(llg_proc_t* self,
                                     llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::suspend");
    if (!handle || !handle->proc || !region_can_mutate("process suspension"))
        return LLG_CO_ARM_READY;
    llg_proc_t* target = handle->proc;
    if (target->suspended || target->killed || target->completed)
        return LLG_CO_ARM_READY;
    target->suspended = 1;
    target->wake_pending = 0;
    remove_region_entry(target);
    process_status_set(target, LLG_PROCESS_SUSPENDED);
    if (target == self) {
        // Suspending is a blocking control for join_none eligibility, but the
        // wait itself is represented by the stable handle state rather than a
        // second scheduler waiter.
        start_pending_fork_children(target);
        return LLG_CO_ARM_SUSPEND;
    }
    return LLG_CO_ARM_READY;
}

void llg_process_resume(llg_proc_t* self, llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::resume");
    if (!handle || !handle->proc || !region_can_mutate("process resumption")) return;
    llg_proc_t* target = handle->proc;
    if (!target->suspended || target->killed || target->completed) return;
    target->suspended = 0;
    if (target->wait.kind != W_NONE) {
        // The outstanding condition remains registered and must be satisfied
        // before this process becomes runnable again.
        process_status_set(target, LLG_PROCESS_WAITING);
        return;
    }
    if (target->wake_pending) target->wake_pending = 0;
    process_status_set(target, LLG_PROCESS_RUNNING);
    enqueue_region(target, target->region);
}

llg_co_arm_t llg_arm_process_await(llg_proc_t* self,
                                   llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::await");
    if (!self || !region_can_mutate("process await scheduling"))
        return LLG_CO_ARM_READY;
    if (!handle || !handle->proc || handle->proc == self)
        return LLG_CO_ARM_READY;
    llg_wait_t* wait = &self->wait;
    wait->kind = W_PROCESS;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    wait_rare_allocate(wait, "process await payload")->process.target = handle;
    llg_process_retain(handle);
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_semaphore_t* llg_semaphore_new(sv4_t key_count) {
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys)) return NULL;
    llg_semaphore_t* semaphore = (llg_semaphore_t*)llg_checked_calloc(
        1, sizeof(*semaphore), "semaphore");
    semaphore->available = keys;
    semaphore->next_all = g.semaphores;
    g.semaphores = semaphore;
    return semaphore;
}

void llg_semaphore_put(llg_semaphore_t* semaphore, sv4_t key_count) {
    if (!region_can_mutate("semaphore put") ||
        !semaphore_valid(semaphore, "put"))
        return;
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys) || keys == 0) return;
    if (keys > UINT64_MAX - semaphore->available) {
        fprintf(stderr, "llg: semaphore key count overflow in put\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    semaphore->available += keys;
    semaphore_wake_available(semaphore);
}

llg_co_arm_t llg_arm_semaphore_get(llg_proc_t* self,
                                   llg_semaphore_t* semaphore,
                                   sv4_t key_count) {
    llg_runtime_service_enter(self, "semaphore::get");
    if (!region_can_mutate("semaphore get") ||
        !semaphore_valid(semaphore, "get"))
        return LLG_CO_ARM_READY;
    if (!self) {
        fprintf(stderr, "llg: semaphore get requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return LLG_CO_ARM_READY;
    }
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys) || keys == 0)
        return LLG_CO_ARM_READY;
    if (!semaphore->wait_head && semaphore->available >= keys) {
        semaphore->available -= keys;
        return LLG_CO_ARM_READY;
    }

    llg_wait_t* wait = &self->wait;
    llg_semaphore_wait_t* node = (llg_semaphore_wait_t*)llg_checked_calloc(
        1, sizeof(*node), "semaphore waiter");
    node->owner = semaphore;
    node->proc = self;
    node->keys = keys;
    if (semaphore->wait_tail) {
        semaphore->wait_tail->next = node;
    } else {
        semaphore->wait_head = node;
    }
    semaphore->wait_tail = node;
    wait->kind = W_SEMAPHORE;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_semaphore_payload_t* payload =
        &wait_rare_allocate(wait, "semaphore wait payload")->semaphore;
    payload->semaphore = semaphore;
    payload->waiter = node;
    payload->keys = keys;
    process_status_set(self, LLG_PROCESS_WAITING);
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

int llg_semaphore_try_get(llg_semaphore_t* semaphore, sv4_t key_count) {
    if (!region_can_mutate("semaphore try_get") ||
        !semaphore_valid(semaphore, "try_get"))
        return 0;
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys)) return 0;
    if (keys == 0) return 1;
    // Preserve the specified FIFO ordering: an immediate attempt never skips
    // an already queued request, even when enough keys are currently visible.
    if (semaphore->wait_head || semaphore->available < keys) return 0;
    semaphore->available -= keys;
    return 1;
}
