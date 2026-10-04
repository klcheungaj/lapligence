
// Detach a live group without changing the order of surviving siblings.
static void fork_group_unlink(llg_fork_group_t* grp) {
    llg_proc_t* parent = grp->parent;
    if (grp->prev_g) grp->prev_g->next_g = grp->next_g;
    else parent->fork_groups = grp->next_g;
    if (grp->next_g) grp->next_g->prev_g = grp->prev_g;
    else parent->fork_groups_tail = grp->prev_g;
    if (parent->pending_fork_groups == grp)
        parent->pending_fork_groups = grp->next_g;
    grp->prev_g = NULL;
}

// One child of `grp` finished. Decrement the live count,
// wake a join/wait_fork waiter whose condition is now met, and move the group
// to the zombie list once the last child is done.
static void llg_fork_group_child_done(llg_fork_group_t* grp) {
    if (!grp || grp->terminal || grp->remaining <= 0) return;
    llg_proc_t* parent = grp->parent;
    grp->remaining--;
    int wake = 0;
    if (grp->join_kind == LLG_JOIN) {
        if (grp->remaining == 0) wake = 1;
    } else if (grp->join_kind == LLG_JOIN_ANY) {
        if (!grp->resumed) {
            grp->resumed = 1;
            wake = 1;
        }
    }
    if (wake && parent->wait.kind == W_FORK && parent->wait.payload.rare &&
        parent->wait.payload.rare->fork.group == grp) {
        wake_proc(parent);
    }
    if (grp->remaining == 0) {
        grp->terminal = 1;
        // Unlink from the parent's live-group list; the group and its child
        // list are freed by process_zombie_groups at the next safe point.
        // join_any / join_none groups stay live until the last child finishes
        // so wait_fork still works.
        fork_group_unlink(grp);
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        // wait fork observes only the groups spawned by its own process.
        if (parent->wait.kind == W_FORK_ALL && !parent->fork_groups)
            wake_proc(parent);
    }
}

static llg_fork_group_t* llg_fork_group_new_impl(int join_kind,
                                                  int has_target,
                                                  uint32_t declaration,
                                                  uint32_t instance) {
    llg_proc_t* parent = llg_current();
    if (!parent || !region_can_mutate("fork scheduling")) return NULL;
    llg_fork_group_t* grp = (llg_fork_group_t*)llg_checked_calloc(
        1, sizeof(llg_fork_group_t), "fork group");
    grp->join_kind = join_kind;
    grp->started = join_kind != LLG_JOIN_NONE;
    grp->parent = parent;
    grp->has_target = has_target;
    grp->target_declaration = declaration;
    grp->target_instance = instance;
    grp->child_region = region_is_reactive(g.current_region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    grp->owner_activation = parent->activation_top;
    if (grp->owner_activation) activation_retain(grp->owner_activation);
    // Preserve source creation order when one suspension releases multiple
    // join_none groups.  Child order within each group is already the branch
    // list order, so this gives the scheduler one deterministic sequence.
    grp->prev_g = parent->fork_groups_tail;
    if (grp->prev_g) grp->prev_g->next_g = grp;
    else parent->fork_groups = grp;
    parent->fork_groups_tail = grp;
    if (!parent->pending_fork_groups) parent->pending_fork_groups = grp;
    return grp;
}

static void start_pending_fork_children(llg_proc_t* parent) {
    if (!parent) return;
    llg_fork_group_t* pending = parent->pending_fork_groups;
    parent->pending_fork_groups = NULL;
    for (llg_fork_group_t* grp = pending; grp; grp = grp->next_g) {
        if (grp->join_kind != LLG_JOIN_NONE || grp->started) continue;
        grp->started = 1;
        for (llg_fork_child_t* child = grp->children; child; child = child->next) {
            llg_proc_t* child_proc = child->proc;
            if (!child_proc || child_proc->killed || child_proc->completed) continue;
            enqueue_region(child_proc, grp->child_region);
        }
    }
}

llg_fork_group_t* llg_fork_group_new(int join_kind) {
    return llg_fork_group_new_impl(join_kind, 0, 0, 0);
}

llg_fork_group_t* llg_fork_group_new_target(int join_kind,
                                             uint32_t declaration,
                                             uint32_t instance) {
    return llg_fork_group_new_impl(join_kind, 1, declaration, instance);
}

static llg_proc_t* llg_fork_impl(const llg_co_desc_t* desc,
                                 const char* name, llg_fork_group_t* grp,
                                 llg_frame_t* frame) {
    if (!desc || !desc->fn || !grp || !region_can_mutate("fork scheduling"))
        return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t))
        llg_rt_co_oom(desc->frame_size);
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size,
        "forked process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->grp = grp;
    p->frame = frame;
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_frame_retain(frame);
    llg_rng_state_child(&grp->parent->rng, &p->rng);
    p->program = grp->parent->program;
    p->action_assertion = grp->parent->action_assertion;
    p->is_assertion_action = grp->parent->is_assertion_action;
    p->program_live = 0;
    p->budget_time = g.now;
    grp->remaining++;
    llg_fork_child_t* c = (llg_fork_child_t*)llg_checked_malloc(
        1, sizeof(llg_fork_child_t), "fork child");
    c->proc = p;
    c->next = NULL;
    if (grp->children_tail) grp->children_tail->next = c;
    else grp->children = c;
    grp->children_tail = c;
    p->fork_child = c;
    register_proc(p);
    if (grp->join_kind != LLG_JOIN_NONE) enqueue_region(p, grp->child_region);
    return p;
}

llg_proc_t* llg_fork(const llg_co_desc_t* desc, const char* name,
                     llg_fork_group_t* grp) {
    return llg_fork_impl(desc, name, grp, NULL);
}

llg_proc_t* llg_fork_with_frame(const llg_co_desc_t* desc,
                                const char* name, llg_fork_group_t* grp,
                                llg_frame_t* frame) {
    return llg_fork_impl(desc, name, grp, frame);
}

llg_proc_t* llg_spawn_detached_with_frame(const llg_co_desc_t* desc,
                                          const char* name,
                                          llg_frame_t* frame) {
    llg_proc_t* parent = llg_current();
    if (!desc || !desc->fn || !parent || !region_can_mutate("detached process scheduling"))
        return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t))
        llg_rt_co_oom(desc->frame_size);
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size,
        "detached process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->frame = frame;
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_frame_retain(frame);
    llg_rng_state_child(&parent->rng, &p->rng);
    // Like a fork descendant, the process keeps its origin for program
    // cancellation but never extends that program's lifetime.
    p->program = parent->program;
    p->action_assertion = parent->action_assertion;
    p->is_assertion_action = parent->is_assertion_action;
    p->program_live = 0;
    p->budget_time = g.now;
    llg_region_t region = region_is_reactive(g.current_region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    p->region = region;
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_co_arm_t llg_arm_join(llg_proc_t* self, llg_fork_group_t* grp) {
    llg_runtime_service_enter(self, "join");
    if (!grp || !region_can_mutate("fork wait scheduling"))
        return LLG_CO_ARM_READY;
    if (grp->remaining == 0) {
        // Empty fork groups never receive a child-done callback, so finalize
        // them here before join or wait_fork can observe a permanently live
        // group. The parent is the currently running process.
        fork_group_unlink(grp);
        grp->terminal = 1;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        return LLG_CO_ARM_READY;
    }
    if (grp->join_kind == LLG_JOIN_NONE) return LLG_CO_ARM_READY;
    if (grp->join_kind == LLG_JOIN_ANY && grp->resumed)
        return LLG_CO_ARM_READY;
    if (!self) return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_FORK;
    wait_rare_allocate(w, "fork wait payload")->fork.group = grp;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_wait_fork(llg_proc_t* self) {
    llg_runtime_service_enter(self, "wait fork");
    if (!self || self->fork_groups == NULL ||
        !region_can_mutate("fork wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_FORK_ALL;
    wait_rare_allocate(w, "wait-fork payload")->fork_all.parent = self;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

void llg_disable_fork(llg_proc_t* self) {
    llg_runtime_service_enter(self, "disable fork");
    if (!region_can_mutate("fork scheduling")) return;
    if (!self) return;
    llg_kill_proc_groups(self);
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
}

void llg_program_exit(llg_proc_t* self) {
    llg_runtime_service_enter(self, "program exit");
    if (!self || !self->program) return;
    if (!region_can_mutate("program exit")) return;
    llg_program_t* origin = self->program;
    // Mark closed before cancellation; recursive unlinking only adjusts counts.
    origin->closed = 1;
    for (;;) {
        llg_proc_t* victim = NULL;
        for (int i = 0; i < g.n_procs; i++) {
            llg_proc_t* proc = g.all_procs[i];
            if (proc && proc->program == origin && !proc->killed &&
                !proc->completed) {
                victim = proc;
                break;
            }
        }
        if (!victim) break;
        llg_kill_proc_tree(victim);
    }
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    // Cancellation already detached all process-owned resources.
    self->chain.exiting = LLG_EXIT_ABANDON;
}

static int activation_has_disabled_ancestor(llg_activation_t* activation) {
    for (; activation; activation = activation->parent) {
        if (activation->disabled) return 1;
    }
    return 0;
}

static void llg_kill_named_group(llg_fork_group_t* grp) {
    if (!grp || !grp->parent) return;
    llg_proc_t* parent = grp->parent;
    llg_fork_child_t* child = grp->children;
    while (child) {
        if (child->proc) {
            // The group is being cancelled as a unit. Suppress per-child
            // completion accounting until the group is detached below.
            llg_kill_proc_tree_internal(child->proc, 0);
            child->proc = NULL;
        }
        child = child->next;
    }
    grp->remaining = 0;
    grp->terminal = 1;
    fork_group_unlink(grp);
    grp->next_g = g.zombie_groups;
    g.zombie_groups = grp;

    if (parent->wait.kind == W_FORK && parent->wait.payload.rare &&
        parent->wait.payload.rare->fork.group == grp) {
        wake_proc(parent);
    }
    if (parent->wait.kind == W_FORK_ALL && parent->fork_groups == NULL) {
        wake_proc(parent);
    }
}

void llg_disable_target(llg_proc_t* self, uint32_t declaration,
                        uint32_t instance) {
    llg_runtime_service_enter(self, "disable");
    if (!region_can_mutate("named activation scheduling")) return;
    int matched = 0;
    for (llg_activation_t* activation = g.activations; activation;
         activation = activation->all_next) {
        if (activation->declaration == declaration &&
            activation->instance == instance) {
            activation->disabled = 1;
            matched = 1;
        }
    }

    // Restart after each removal: cancelling a tree can remove other groups
    // and processes from the registry, including the caller's ancestors.
    for (;;) {
        llg_fork_group_t* target = NULL;
        for (int i = 0; i < g.n_procs && !target; i++) {
            llg_proc_t* proc = g.all_procs[i];
            if (!proc) continue;
            for (llg_fork_group_t* group = proc->fork_groups; group;
                 group = group->next_g) {
                if ((group->has_target &&
                     group->target_declaration == declaration &&
                     group->target_instance == instance) ||
                    (group->owner_activation &&
                     activation_has_disabled_ancestor(group->owner_activation))) {
                    target = group;
                    break;
                }
            }
        }
        if (!target) break;
        matched = 1;
        llg_kill_named_group(target);
    }

    // A blocked activation must be detached from every wait list before its
    // coroutine is scheduled again. Immediate NBAs remain attached to the
    // process and are therefore committed normally after cancellation.
    if (matched) {
        for (llg_activation_t* activation = g.activations; activation;
             activation = activation->all_next) {
            if (activation->disabled && activation->proc &&
                activation->proc->wait.kind != W_NONE) {
                wake_proc(activation->proc);
            }
        }
    }
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    if (self && self->killed) self->chain.exiting = LLG_EXIT_ABANDON;
}

// Free completed/killed fork groups: their child procs that were not already
// freed by disable_fork, the child list nodes and the group struct.  Called
// from llg_rt_run after commit_nbas with no coroutine running, so done
// children's NBAs have been committed and their all_procs slots can be NULLed
// safely (the next commit_nbas then skips them).
static void process_zombie_groups(void) {
    llg_fork_group_t* grp = g.zombie_groups;
    g.zombie_groups = NULL;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        int deferred = 0;
        for (llg_fork_child_t* c = grp->children; c; c = c->next) {
            if (!c->proc) continue;
            // A completed child can still own live join_none descendants.
            // Keep its process object until those groups unlink themselves;
            // their completion path dereferences the parent pointer.
            if (c->proc->fork_groups || c->proc->nba_head) {
                deferred = 1;
                continue;
            }
            unregister_proc(c->proc);
            free_proc_record(c->proc);
            c->proc = NULL;
        }
        if (deferred) {
            grp->next_g = g.zombie_groups;
            g.zombie_groups = grp;
        } else {
            llg_fork_child_t* c = grp->children;
            while (c) {
                llg_fork_child_t* next_c = c->next;
                free(c);
                c = next_c;
            }
            activation_release(grp->owner_activation);
            grp->owner_activation = NULL;
            free(grp);
        }
        grp = next_g;
    }
}
