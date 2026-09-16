
static llg_proc_t* spawn_in_region(void (*fn)(llg_proc_t*), const char* name,
                                   llg_region_t region, llg_program_t* program,
                                   int is_initial) {
    if (g.config_error || !fn || !region_valid(region)) return NULL;
    if (!callback_region_allowed(region, 0)) return NULL;
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(llg_proc_t), "process");
    p->name = name;
    p->fn = fn;
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
    p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_proc_t* llg_spawn_in_region(void (*fn)(llg_proc_t*), const char* name,
                                llg_region_t region) {
    return spawn_in_region(fn, name, region, NULL, 0);
}

llg_proc_t* llg_spawn_program_in_region(void (*fn)(llg_proc_t*),
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
    return spawn_in_region(fn, name, region, program, is_initial);
}

llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name) {
    return llg_spawn_in_region(fn, name, LLG_REGION_ACTIVE);
}

llg_frame_t* llg_proc_frame(llg_proc_t* self) {
    return self ? self->frame : NULL;
}

_Noreturn void llg_proc_done(llg_proc_t* self) {
    if (!self || self != llg_current()) {
        fprintf(stderr, "llg: process completion outside the current coroutine\n");
        abort();
    }
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
    aco_exit(); // never returns
}

void llg_wait_time(uint64_t ticks) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_TIME;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr,
                "llg: fatal: simulation time overflow at %llu while scheduling a delay of %llu tick(s)\n",
                (unsigned long long)g.now, (unsigned long long)ticks);
        abort();
    }
    w->time = g.now + ticks;
    if (ticks == 0) {
        // `#0` yields into the INACTIVE region of the current time step
        // (LRM §4.4.2): it runs after the active region drains and before
        // the NBA region commits.
        w->resume_region = region_is_reactive(p->region)
                               ? LLG_REGION_RE_INACTIVE
                               : LLG_REGION_INACTIVE;
        insert_zero_wait(w, w->resume_region);
    } else {
        insert_timed(w);
    }
    register_wait();
    aco_yield();
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

void llg_wait_any(sv4_t** sigs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->resume_region = take_wait_resume_region(p);
    w->n = n;
    w->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "event wait specifications");
    w->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "event wait snapshots");
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = sigs[i];
        w->specs[i].kind = LLG_EV_ANY;
        w->last[i] = sv4_clone(sigs[i]);
    }
    register_wait();
    aco_yield();
}

void llg_wait_any_dependencies(const llg_wait_dependency_t* deps, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_DEPS;
    w->resume_region = take_wait_resume_region(p);
    w->n = n;
    w->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
    w->last = (sv4_t*)llg_checked_calloc((size_t)n, sizeof(sv4_t), "packed-prefix wait snapshots");
    for (int i = 0; i < n; i++) {
        if ((deps[i].sig == NULL) == (deps[i].real == NULL)) {
            fprintf(stderr, "llg: typed wait dependency must name one storage kind\n");
            abort();
        }
        w->dependencies[i] = deps[i];
        if (deps[i].width) {
            sv4_t* value = deps[i].value ? deps[i].value : deps[i].sig;
            if (!value || deps[i].real || deps[i].lsb >= value->width ||
                deps[i].width > value->width - deps[i].lsb) {
                fprintf(stderr, "llg: invalid packed-prefix wait dependency\n"); abort();
            }
            w->last[i] = sv4_part_select(*value, (int64_t)deps[i].lsb + deps[i].width - 1, deps[i].lsb);
        }
    }
    register_wait();
    aco_yield();
}

void llg_wait_any_events(llg_event_spec_t* specs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n = n;
    w->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "edge wait specifications");
    w->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "edge wait snapshots");
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = specs[i].sig;
        w->specs[i].kind = specs[i].kind;
        w->last[i] = sv4_clone(specs[i].sig);
    }
    register_wait();
    aco_yield();
}

void llg_wait_edge(sv4_t* sig, int posedge) {
    llg_event_spec_t spec;
    spec.sig = sig;
    spec.kind = posedge ? LLG_EV_POSEDGE : LLG_EV_NEGEDGE;
    llg_wait_any_events(&spec, 1);
}

void llg_wait_level(sv4_t* sig, sv4_t value) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_LEVEL;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->sig = sig;
    sv4_copy(&w->level_val, &value);
    register_wait();
    aco_yield();
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────
