
static llg_proc_t* spawn_process(const llg_co_desc_t* desc,
                                 const char* name, llg_region_t region,
                                 llg_program_t* program, int is_initial,
                                 int runtime_owned, int enqueue) {
    if (g.config_error || !desc || !desc->fn || !region_valid(region)) return NULL;
    if (!callback_region_allowed(region, 0)) return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t)) {
        llg_rt_co_oom(desc->frame_size);
    }
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size, "process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    // Runtime-owned processes (covergroup waiters and the like) are
    // infrastructure: they never make a run deadlocked.
    p->deadlock_counted = !runtime_owned && (desc->host_flags & LLG_DESC_BLOCKABLE) != 0;
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
    if (runtime_owned) {
        // Derived, not drawn: creating one never shifts another stream.
        llg_rng_state_derive(&p->rng, &g.rng_root, ++g.owned_spawns);
    } else {
        rng_seed_static(p);
    }
    p->budget_time = g.now;
    p->region = region;
    register_proc(p);
    if (enqueue) enqueue_region(p, region);
    return p;
}

static llg_proc_t* spawn_in_region(const llg_co_desc_t* desc,
                                   const char* name, llg_region_t region,
                                   llg_program_t* program, int is_initial) {
    return spawn_process(desc, name, region, program, is_initial, 0, 1);
}

llg_proc_t* llg_spawn_in_region(const llg_co_desc_t* desc,
                                const char* name, llg_region_t region) {
    return spawn_in_region(desc, name, region, NULL, 0);
}

static void proc_complete(llg_proc_t* self);

llg_proc_t* llg_spawn_owned_with_frame(const llg_co_desc_t* desc,
                                       const char* name,
                                       llg_frame_t* frame) {
    // From a process turn, the new process runs its first turn now, nested
    // in the caller's, so that a wait it arms observes changes the caller
    // makes before it suspends (decision S44-D6). Elsewhere (model
    // initialization, region callbacks) it is queued in the Active region.
    int now = g.process_turn_active && g.current != NULL;
    llg_proc_t* p =
        spawn_process(desc, name, LLG_REGION_ACTIVE, NULL, 0, 1, !now);
    if (!p) return NULL;
    p->frame = frame;
    llg_frame_retain(frame);
    if (!now) return p;
    llg_proc_t* caller = g.current;
    const char* caller_name = g.last_process_name;
    g.last_process_name = p->name;
    g.current = p;
    llg_co_status_t status = llg_co_run(&p->chain);
    g.current = caller;
    g.last_process_name = caller_name;
    if (status == LLG_CO_DONE ||
        (status == LLG_CO_EXIT && p->chain.exiting == LLG_EXIT_COMPLETE)) {
        proc_complete(p);
        proc_retire_completed(p);
        return NULL;
    }
    if (status == LLG_CO_CALLED) {
        fprintf(stderr,
                "llg runtime fatal: coroutine anchor escaped llg_co_run\n");
        abort();
    }
    return p;
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

static llg_proc_t* initialize_instance(llg_proc_t* p,
    const llg_co_desc_t* desc, const void* record, size_t offset) {
    if (!p) return NULL;
    if (!record || offset < sizeof(llg_co_frame_t) ||
        desc->frame_size < sizeof(record) ||
        offset > desc->frame_size - sizeof(record)) {
        llg_rt_co_bad_state(LLG_CO_ROOT(&p->chain), "invalid root instance record offset");
    }
    memcpy((unsigned char*)LLG_CO_ROOT(&p->chain) + offset, &record, sizeof(record));
    return p;
}

llg_proc_t* llg_spawn_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, const void* record, size_t record_offset) {
    return initialize_instance(llg_spawn_in_region(desc, name, region),
                               desc, record, record_offset);
}

llg_proc_t* llg_spawn_program_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, uint64_t instance, int is_initial,
    const void* record, size_t record_offset) {
    return initialize_instance(llg_spawn_program_in_region(desc, name, region,
                                                          instance, is_initial),
                               desc, record, record_offset);
}

llg_frame_t* llg_proc_frame(llg_proc_t* self) {
    return self ? self->frame : NULL;
}

// ── Delayed Active-region calls ──────────────────────────────────────────────
//
// A carrier is an unregistered process record (registry_slot -1, no handle,
// no program, no RNG stream) whose root frame holds the call. It is linked
// into g.delayed_calls from scheduling until its callback starts, the only
// window in which llg_cancel_delayed_call or cleanup discard it. Once
// started it completes like any top-level process and proc_retire_completed
// retires it, so the run loop has no carrier-specific branch.

typedef struct {
    llg_co_frame_t co;
    uint64_t ticks;
    llg_region_callback_fn callback;
    void* data;
    llg_proc_t* prev; // g.delayed_calls links while pending
    llg_proc_t* next;
    int pending;
} llg_delayed_call_frame_t;
LLG_CO_ROOT_FRAME_OK(llg_delayed_call_frame_t);

static llg_co_status_t delayed_call_body(llg_co_frame_t* co, llg_co_chain_t* ch);
static const llg_co_desc_t delayed_call_desc = {
    delayed_call_body, "delayed call", sizeof(llg_delayed_call_frame_t), NULL, 0, 0};

static llg_delayed_call_frame_t* delayed_call_frame(llg_proc_t* carrier) {
    return (llg_delayed_call_frame_t*)LLG_CO_ROOT(&carrier->chain);
}

static int proc_is_delayed_call(const llg_proc_t* proc) {
    return proc->chain.root == &delayed_call_desc;
}

static void delayed_call_unlink(llg_proc_t* carrier) {
    llg_delayed_call_frame_t* F = delayed_call_frame(carrier);
    if (!F->pending) return;
    F->pending = 0;
    if (F->prev) delayed_call_frame(F->prev)->next = F->next;
    else g.delayed_calls = F->next;
    if (F->next) delayed_call_frame(F->next)->prev = F->prev;
    F->prev = F->next = NULL;
}

static llg_co_status_t delayed_call_body(llg_co_frame_t* co, llg_co_chain_t* ch) {
    llg_delayed_call_frame_t* F = (llg_delayed_call_frame_t*)co;
    LLG_CO_DISPATCH_BEGIN(co)
    LLG_CO_RESUME_CASE(1)
    LLG_CO_DISPATCH_END(co)
    if (F->ticks != 0) {
        LLG_CO_AWAIT(co, ch, 1, llg_arm_time(LLG_CO_OWNER(ch, llg_proc_t), F->ticks));
    }
    // Started: from here on the call can no longer be cancelled.
    delayed_call_unlink(LLG_CO_OWNER(ch, llg_proc_t));
    F->callback(F->data);
    return LLG_CO_DONE;
}

int llg_schedule_delayed_call(const char* name, llg_region_callback_fn callback,
                              void* data, uint64_t ticks,
                              llg_delayed_call_id_t* id) {
    if (!callback || g.config_error || ticks > UINT64_MAX - g.now) return 0;
    if (!region_can_mutate("delayed call scheduling")) return 0;
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + sizeof(llg_delayed_call_frame_t), "delayed call carrier");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, sizeof(llg_delayed_call_frame_t));
#endif
    p->name = name;
    llg_co_start(&p->chain, &delayed_call_desc, p);
    llg_delayed_call_frame_t* F = delayed_call_frame(p);
    F->ticks = ticks;
    F->callback = callback;
    F->data = data;
    F->prev = NULL;
    F->next = g.delayed_calls;
    F->pending = 1;
    if (g.delayed_calls) delayed_call_frame(g.delayed_calls)->prev = p;
    g.delayed_calls = p;
    p->registry_slot = -1;
    p->status = LLG_PROCESS_RUNNING;
    p->budget_time = g.now;
    p->region = LLG_REGION_ACTIVE;
    if (ticks == 0) {
        // A zero-delay call is an Active event of the current slot, after
        // the work already queued there.
        enqueue_region(p, LLG_REGION_ACTIVE);
    } else {
        // Run the first turn now, whatever the caller (a process turn, a
        // region callback or startup before llg_rt_run): it arms `#ticks`,
        // so the timer enters the timed queue in issue order.
        llg_proc_t* caller = g.current;
        g.current = p;
        llg_co_status_t status = llg_co_run(&p->chain);
        g.current = caller;
        if (status != LLG_CO_PENDING)
            llg_rt_co_bad_state(LLG_CO_ROOT(&p->chain), "delayed call did not suspend");
    }
    if (id) {
        id->generation = llg_event_generation;
        id->carrier = p;
    }
    return 1;
}

// Unlink a carrier that has not started from its timer or region queue and
// free it. It is never the current process.
static void delayed_call_discard(llg_proc_t* carrier) {
    delayed_call_unlink(carrier);
    llg_wait_t* w = &carrier->wait;
    if (w->kind != W_NONE) {
        remove_waiters_entry(w);
        remove_timed_entry(w);
        wait_payload_release(w);
        g.wait_count -= carrier->deadlock_counted;
    }
    if (carrier->queued) remove_region_entry(carrier);
    free_proc_record(carrier);
}

int llg_cancel_delayed_call(llg_delayed_call_id_t id) {
    if (id.generation != llg_event_generation || !id.carrier ||
        !delayed_call_frame(id.carrier)->pending)
        return 0;
    delayed_call_discard(id.carrier);
    return 1;
}

static void free_delayed_calls(void) {
    while (g.delayed_calls) delayed_call_discard(g.delayed_calls);
}

static void proc_complete(llg_proc_t* self) {
    if (!self || self->completed || self->killed) return;
    // Natural process termination is the other join_none eligibility
    // boundary.  Release children before unwinding the creator's activation
    // and frame; their copied captures remain retained by the child process.
    start_pending_fork_children(self);
    wait_retained_drop(self);
    self->completed = 1;
    // SV 9.7: "KILLED means the process was forcibly killed (via kill or
    // disable)"; a disabled whole-branch block leaves nothing to resume at.
    int terminal =
        self->disabled_whole ? LLG_PROCESS_KILLED : LLG_PROCESS_FINISHED;
    process_status_set(self, terminal);
    process_handle_terminal(self, terminal);
    thread_event(self, LLG_RT_THREAD_END);
    if (process_end_hook) process_end_hook(self, 0);
    value_scopes_unwind(self);
    activation_unwind_proc(self);
    llg_frame_release(self->frame);
    self->frame = NULL;
    process_local_release_all(self);
    if (self->grp) llg_fork_group_child_done(self->grp);
    release_program_process(self);
    service_program_completions();
}

llg_co_arm_t llg_arm_time(llg_proc_t* self, uint64_t ticks) {
    llg_runtime_service_enter(self, "delay");
    if (!self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    // A delay past the last 64-bit time is a fatal run-time error; the final
    // procedures still run (decision S01-D2).
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr,
                "llg: error: simulation time overflow at %llu while scheduling a delay of %llu tick(s)\n",
                (unsigned long long)g.now, (unsigned long long)ticks);
        llg_rt_mark_failed();
        llg_rt_fatal_typed(0, "simulation time overflow", NULL, 0, "", "delay");
        return LLG_CO_ARM_EXIT;
    }
    llg_wait_t* w = &self->wait;
    w->kind = W_TIME;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->payload.timer.time = g.now + ticks;
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
    if (n == 1 && wait_inline_fits(sigs[0])) {
        w->kind = W_EVENTS_INLINE;
        w->payload.single.specs[0] = (llg_event_spec_t){sigs[0], LLG_EV_ANY};
        wait_inline_copy(w, sigs[0]);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
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

// Dependency lists compare as bytes: the record has no padding, so equal
// bytes are equal pointers, offsets and widths.
_Static_assert(sizeof(llg_wait_dependency_t) ==
                   3 * sizeof(void*) + 2 * sizeof(uint32_t),
               "llg_wait_dependency_t must stay free of padding");

// The retained record to arm with `deps`: the process's previous one when it
// names the same list, otherwise a new one replacing it. The previous wait
// has been released, so any retained memberships are dormant.
static llg_wait_retained_t* wait_retained_for(llg_proc_t* self,
                                              const llg_wait_dependency_t* deps,
                                              int n) {
    llg_wait_retained_t* retained = self->retained_wait;
    if (retained && retained->n == n &&
        memcmp(retained->dependencies, deps, (size_t)n * sizeof(*deps)) == 0)
        return retained;
    wait_retained_drop(self);
    retained = (llg_wait_retained_t*)llg_checked_calloc(
        1, sizeof(*retained), "retained dependency wait");
    retained->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
    retained->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "packed-prefix wait snapshots");
    retained->n = n;
    self->retained_wait = retained;
    return retained;
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
    llg_wait_retained_t* retained = LLG_GATE_FAST_PATH && n > 0
        ? wait_retained_for(self, deps, n) : NULL;
    if (retained) {
        payload->dependencies = retained->dependencies;
        payload->last = retained->last;
    } else {
        payload->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
            (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
        payload->last = (sv4_t*)llg_checked_calloc(
            (size_t)n, sizeof(sv4_t), "packed-prefix wait snapshots");
    }
    for (int i = 0; i < n; i++) {
        if ((deps[i].sig == NULL) == (deps[i].real == NULL)) {
            fprintf(stderr, "llg: typed wait dependency must name one storage kind\n");
            abort();
        }
        payload->dependencies[i] = deps[i];
        if (deps[i].width) {
            sv4_t* value = deps[i].value ? deps[i].value : deps[i].sig;
            if (!value || deps[i].real || deps[i].lsb >= llg_sv4_width(*value) ||
                deps[i].width > llg_sv4_width(*value) - deps[i].lsb) {
                fprintf(stderr, "llg: invalid packed-prefix wait dependency\n"); abort();
            }
            sv4_replace(&payload->last[i], sv4_part_select(
                *value, (int64_t)deps[i].lsb + deps[i].width - 1,
                deps[i].lsb));
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_event_dependencies(llg_proc_t* self,
                                        const llg_wait_dependency_t* deps,
                                        int n) {
    if (self) self->next_wait_event = 1;
    llg_co_arm_t arm = llg_arm_any_dependencies(self, deps, n);
    if (self) self->next_wait_event = 0;
    return arm;
}

void llg_wait_refresh_on_resume(llg_proc_t* self) {
    if (self) self->next_wait_refresh = 1;
}

int llg_wait_refreshed(const llg_proc_t* self) {
    return self && self->wait_refreshed;
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
    if (n == 1 && wait_inline_fits(specs[0].sig)) {
        w->kind = W_EVENTS_INLINE;
        w->payload.single.specs[0] = specs[0];
        wait_inline_copy(w, specs[0].sig);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
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
    if (wait_inline_fits(&value)) {
        w->kind = W_LEVEL_INLINE;
        w->payload.single.specs[0] = (llg_event_spec_t){sig, LLG_EV_ANY};
        wait_inline_copy(w, &value);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
    llg_wait_level_payload_t* payload =
        &wait_rare_allocate(w, "level wait payload")->level;
    payload->sig = sig;
    sv4_copy(&payload->value, &value);
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────
