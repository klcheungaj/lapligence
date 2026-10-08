// ── Scheduler roots of the object collector (SIM-018) ─────────────────────────
//
// Everything the scheduler owns that can keep a class object reachable while
// no process turn is active. Generated models add their own static roots,
// payload tracers and coroutine frame maps through the public registration API.

static void gc_frame_visit(void* user, const llg_co_desc_t* desc,
                           const llg_co_frame_t* frame, unsigned depth) {
    (void)depth;
    gc_trace_frame((llg_gc_tracer_t*)user, desc, frame);
}

static void gc_value_visit(void* const* slot, const llg_value_desc_t* desc,
                           void* context) {
    gc_visit_value_slot(slot, desc, context);
}

static void gc_trace_processes(llg_gc_tracer_t* tracer) {
    for (int i = 0; i < g.all_procs_capacity; ++i) {
        llg_proc_t* proc = g.all_procs[i];
        if (!proc) continue;
        // Root, static embedded callees (site tables) and anchored or arena
        // callees of a suspended or queued coroutine chain.
        (void)llg_co_backtrace(&proc->chain, gc_frame_visit, tracer);
        // A wait subscribed to an object's field or handle marker keeps the
        // object until the wait is woken or cancelled.
        for (size_t s = 0; s < proc->wait.n_subscriptions; ++s) {
            const llg_wait_source_t* source = proc->wait.subscriptions[s].source;
            if (source) llg_gc_visit_interior(tracer, source->key);
        }
    }
}

static void gc_trace_value_scopes(llg_gc_tracer_t* tracer) {
    for (llg_value_scope_t* scope = all_value_scopes; scope; scope = scope->all_next) {
        if (!scope->object || !scope->destroy_object) continue;
        if (scope->destroy_object == llg_gc_handle_cell_drop)
            llg_gc_visit(tracer, *(void* const*)scope->object);
        else
            llg_gc_visit_payload(tracer, scope->destroy_object, scope->object);
    }
}

static void gc_trace_frames(llg_gc_tracer_t* tracer) {
    for (llg_frame_t* frame = all_frames; frame; frame = frame->all_next) {
        for (size_t i = 0; i < frame->nslots; ++i) {
            const llg_frame_slot_t* slot = &frame->slots[i];
            if (slot->alias_kind != LLG_FRAME_ALIAS_NONE) continue;
            if (slot->kind == LLG_FRAME_OPAQUE)
                llg_gc_visit(tracer, slot->value.opaque);
            else if (slot->kind == LLG_FRAME_OBJECT)
                llg_gc_visit_payload(tracer, slot->value.object.destroy,
                                     slot->value.object.data);
        }
    }
}

static void gc_trace_nba(llg_gc_tracer_t* tracer, const llg_nba_t* nba) {
    llg_gc_visit_interior(tracer, nba->target);
    llg_gc_visit_interior(tracer, nba->real_target);
    if (nba->is_string) llg_gc_visit_interior(tracer, nba->native.string.target);
    if (nba->is_chandle) {
        llg_gc_visit_interior(tracer, nba->native.chandle.target);
        llg_gc_visit(tracer, nba->native.chandle.value);
    }
    if (nba->is_values) {
        llg_gc_visit_interior(tracer, nba->native.values.target);
        const llg_dyn_value_array_t* payload = nba->native.values.payload;
        if (payload)
            for (size_t i = 0; i < payload->size; ++i)
                llg_gc_visit_value(tracer, &payload->data[i]);
    }
}

static void gc_trace_nba_queue(llg_gc_tracer_t* tracer, const llg_nba_queue_t* queue) {
    for (const llg_nba_t* nba = queue->head; nba; nba = nba->queue_next)
        gc_trace_nba(tracer, nba);
}

static void gc_trace_nbas(llg_gc_tracer_t* tracer) {
    for (int region = 0; region < LLG_REGION_COUNT; ++region)
        gc_trace_nba_queue(tracer, &g.nba_queues[region]);
    for (const llg_nba_bucket_t* bucket = g.delayed_nba_buckets; bucket;
         bucket = bucket->next)
        for (int region = 0; region < LLG_REGION_COUNT; ++region)
            gc_trace_nba_queue(tracer, &bucket->queues[region]);
}

static void gc_scheduler_roots(llg_gc_tracer_t* tracer, void* context) {
    (void)context;
    gc_trace_processes(tracer);
    gc_trace_value_scopes(tracer);
    gc_trace_frames(tracer);
    llg_native_roots_trace(gc_value_visit, tracer);
    llg_mailbox_trace(gc_value_visit, tracer);
    gc_trace_nbas(tracer);
}

// Called by the scheduler between process turns and region callbacks, the
// only points where no generated code or callback handoff is on the C stack.
static void gc_safe_point(void) {
    if (!llg_gc_pending || g.current || g.process_turn_active) return;
    llg_gc_collect();
}

static int gc_runtime_init(void) {
    if (!gc_configure()) return 0;
    // Class-property native records own an unregistered value released by
    // llg_native_value_destroy; registered native roots are traced above.
    if (!llg_gc_register_roots(gc_scheduler_roots, NULL) ||
        !llg_gc_register_payload_tracer(llg_native_value_destroy,
                                        gc_trace_native_value))
        llg_fatal_allocation("collector roots", 1, sizeof(llg_gc_root_entry_t));
    return 1;
}
