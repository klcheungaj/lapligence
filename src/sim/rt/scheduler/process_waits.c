
static llg_proc_t* spawn_in_region(const llg_co_desc_t* desc,
                                   const char* name, llg_region_t region,
                                   llg_program_t* program, int is_initial) {
    if (g.config_error || !desc || !desc->fn || !region_valid(region)) return NULL;
    if (!callback_region_allowed(region, 0)) return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t)) {
        llg_rt_co_oom(desc->frame_size);
    }
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size, "process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), 0xA5, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->program = is_initial ? program : NULL;
    p->program_live = program && is_initial;
    if (p->program_live) {
        if (program->live_initials == SIZE_MAX || g.program_processes == SIZE_MAX) {
            fprintf(stderr, "llg: program initial accounting overflow\n");
            abort();
        }
        program->had_initial = 1;
        program->live_initials++;
        g.program_processes++;
    }
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_rng_state_child(&g.rng_root, &p->rng);
    p->budget_time = g.now;
    p->region = region;
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_proc_t* llg_spawn_in_region(const llg_co_desc_t* desc,
                                const char* name, llg_region_t region) {
    return spawn_in_region(desc, name, region, NULL, 0);
}

llg_proc_t* llg_spawn_program_in_region(const llg_co_desc_t* desc,
                                         const char* name,
                                         llg_region_t region,
                                         uint64_t instance, int is_initial) {
    if (region != LLG_REGION_REACTIVE) {
        fprintf(stderr,
                "llg: program process must be spawned in a reactive region\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    llg_program_t* program = g.programs;
    while (program && program->instance != instance) program = program->next;
    if (!program) {
        program = llg_checked_calloc(1, sizeof(*program), "program origin");
        program->instance = instance;
        program->next = g.programs;
        g.programs = program;
    }
    if (program->closed) {
        fprintf(stderr, "llg: cannot spawn into a completed program\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    return spawn_in_region(desc, name, region, program, is_initial);
}

llg_proc_t* llg_spawn(const llg_co_desc_t* desc, const char* name) {
    return llg_spawn_in_region(desc, name, LLG_REGION_ACTIVE);
}

llg_frame_t* llg_proc_frame(llg_proc_t* self) {
    return self ? self->frame : NULL;
}

static void proc_complete(llg_proc_t* self) {
    if (!self || self->completed || self->killed) return;
    // Natural process termination is the other join_none eligibility
    // boundary.  Release children before unwinding the creator's activation
    // and frame; their copied captures remain retained by the child process.
    start_pending_fork_children(self);
    self->completed = 1;
    process_status_set(self, LLG_PROCESS_FINISHED);
    process_handle_terminal(self, LLG_PROCESS_FINISHED);
    value_scopes_unwind(self);
    activation_unwind_proc(self);
    llg_frame_release(self->frame);
    self->frame = NULL;
    process_local_release_all(self);
    if (self->grp) llg_fork_group_child_done(self->grp);
    release_program_process(self);
    service_program_completions();
    semaphore_service_cancelled_waiters();
}

llg_co_arm_t llg_arm_time(llg_proc_t* self, uint64_t ticks) {
    llg_runtime_service_enter(self, "delay");
    if (!self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_TIME;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr,
                "llg: fatal: simulation time overflow at %llu while scheduling a delay of %llu tick(s)\n",
                (unsigned long long)g.now, (unsigned long long)ticks);
        abort();
    }
    w->payload.time = g.now + ticks;
    if (ticks == 0) {
        // `#0` yields into the INACTIVE region of the current time step
        // (LRM §4.4.2): it runs after the active region drains and before
        // the NBA region commits.
        w->resume_region = region_is_reactive(self->region)
                               ? LLG_REGION_RE_INACTIVE
                               : LLG_REGION_INACTIVE;
        insert_zero_wait(w, w->resume_region);
    } else {
        insert_timed(w);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

static llg_region_t take_wait_resume_region(llg_proc_t* p) {
    if (p->has_wait_resume_region) {
        p->has_wait_resume_region = 0;
        return p->wait_resume_region;
    }
    return region_is_reactive(p->region) ? LLG_REGION_REACTIVE : LLG_REGION_ACTIVE;
}

void llg_wait_resume_in_region(llg_region_t region) {
    llg_proc_t* p = llg_current();
    if (!p) {
        fprintf(stderr, "llg: wait region requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (!region_can_mutate("wait region scheduling")) return;
    p->wait_resume_region = region;
    p->has_wait_resume_region = 1;
}

llg_co_arm_t llg_arm_any(llg_proc_t* self, sv4_t** sigs, int n) {
    llg_runtime_service_enter(self, "signal wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENTS;
    w->resume_region = take_wait_resume_region(self);
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "event wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "event wait snapshots");
    for (int i = 0; i < n; i++) {
        payload->specs[i].sig = sigs[i];
        payload->specs[i].kind = LLG_EV_ANY;
        payload->last[i] = sv4_clone(sigs[i]);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_any_dependencies(llg_proc_t* self,
                                      const llg_wait_dependency_t* deps,
                                      int n) {
    llg_runtime_service_enter(self, "dependency wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_DEPS;
    w->resume_region = take_wait_resume_region(self);
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "packed-prefix wait snapshots");
    for (int i = 0; i < n; i++) {
        if ((deps[i].sig == NULL) == (deps[i].real == NULL)) {
            fprintf(stderr, "llg: typed wait dependency must name one storage kind\n");
            abort();
        }
        payload->dependencies[i] = deps[i];
        if (deps[i].width) {
            sv4_t* value = deps[i].value ? deps[i].value : deps[i].sig;
            if (!value || deps[i].real || deps[i].lsb >= value->width ||
                deps[i].width > value->width - deps[i].lsb) {
                fprintf(stderr, "llg: invalid packed-prefix wait dependency\n"); abort();
            }
            payload->last[i] = sv4_part_select(
                *value, (int64_t)deps[i].lsb + deps[i].width - 1,
                deps[i].lsb);
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_any_events(llg_proc_t* self,
                                const llg_event_spec_t* specs, int n) {
    llg_runtime_service_enter(self, "edge wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENTS;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "edge wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "edge wait snapshots");
    for (int i = 0; i < n; i++) {
        payload->specs[i].sig = specs[i].sig;
        payload->specs[i].kind = specs[i].kind;
        payload->last[i] = sv4_clone(specs[i].sig);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_edge(llg_proc_t* self, sv4_t* sig, int posedge) {
    llg_event_spec_t spec;
    spec.sig = sig;
    spec.kind = posedge ? LLG_EV_POSEDGE : LLG_EV_NEGEDGE;
    return llg_arm_any_events(self, &spec, 1);
}

llg_co_arm_t llg_arm_level(llg_proc_t* self, sv4_t* sig, sv4_t value) {
    llg_runtime_service_enter(self, "level wait");
    if (!self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_LEVEL;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_level_payload_t* payload =
        &wait_rare_allocate(w, "level wait payload")->level;
    payload->sig = sig;
    sv4_copy(&payload->value, &value);
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────
