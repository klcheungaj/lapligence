
// ── Public scheduler API ──────────────────────────────────────────────────────

static void free_group_storage(llg_fork_group_t* grp) {
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            free(c);
            c = next_c;
        }
        activation_release(grp->owner_activation);
        grp->owner_activation = NULL;
        free(grp);
        grp = next_g;
    }
}

static void free_proc_storage(llg_proc_t* p) {
    cancel_proc_nbas(p);
    remove_waiters_entry(&p->wait);
    event_unlink(&p->wait);
    event_triggered_unlink(&p->wait);
    semaphore_waiter_unlink(&p->wait);
    // A kill has already returned the keys of a grant it interrupted.
    free(p->granted_request);
    p->granted_request = NULL;
    // Likewise a kill has handed back any mailbox delivery; one left here
    // belongs to a process torn down at model close.
    mailbox_message_free(p->mailbox_delivery);
    p->mailbox_delivery = NULL;
    if (p->wait.kind == W_MAILBOX_GET || p->wait.kind == W_MAILBOX_PUT)
        mailbox_unlink_wait(&p->wait);
    wait_payload_release(&p->wait);
    value_scopes_unwind(p);
    activation_unwind_proc(p);
    llg_frame_release(p->frame);
    p->frame = NULL;
    process_local_release_all(p);
    process_handle_shutdown(p);
    free_proc_record(p);
}

static void clocking_copy_observed(void* data);

static void free_region_callbacks(void) {
    while (g.callbacks) {
        llg_region_callback_t* next = g.callbacks->next;
        if (g.callbacks->callback == deferred_assertion_callback)
            free_deferred_assertion_report(
                (llg_deferred_assertion_report_t*)g.callbacks->data);
        else if (g.callbacks->callback == clocking_copy_observed)
            free(g.callbacks->data);
        free(g.callbacks);
        g.callbacks = next;
    }
}

static void free_sampled_values(void) {
    while (g.sampled_reals) {
        llg_sampled_real_t* next = g.sampled_reals->next;
        free(g.sampled_reals);
        g.sampled_reals = next;
    }
    while (g.sampled_values) {
        llg_sampled_value_t* next = g.sampled_values->next;
        sv4_destroy(&g.sampled_values->value);
        free(g.sampled_values);
        g.sampled_values = next;
    }
    while (g.sampled) {
        llg_sampled_value_t* next = g.sampled->next;
        while (g.sampled->history) {
            llg_sampled_history_t* history = g.sampled->history;
            g.sampled->history = history->next;
            sv4_destroy(&history->value);
            free(history);
        }
        sv4_destroy(&g.sampled->value);
        free(g.sampled);
        g.sampled = next;
    }
    for (size_t identity = 0; identity < g.sampled_domains_capacity; identity++) {
        llg_sampled_domain_t* domain = g.sampled_domains[identity];
        if (!domain) continue;
        for (size_t index = 0; index < domain->capacity; index++)
            sv4_destroy(&domain->samples[index]);
        free(domain->samples);
        free(domain->times);
        sv4_destroy(&domain->initial);
        sv4_destroy(&domain->current);
        free(domain);
    }
    free(g.sampled_domains);
    g.sampled_domains = NULL;
    g.sampled_domains_capacity = 0;
    for (size_t identity = 0; identity < g.sampled_clocks_capacity; identity++) {
        llg_sampled_clock_t* clock = g.sampled_clocks[identity];
        if (!clock) continue;
        free(clock->domains);
        free(clock);
    }
    free(g.sampled_clocks);
    g.sampled_clocks = NULL;
    g.sampled_clocks_capacity = 0;
    g.sampled_edge_clocks = NULL;
}

/* Assertion clock events are recycled: every sequence assertion edge queues
 * one event and one history entry, so a free list keeps the steady state
 * allocation-free. The pool is released by free_sequence_pools(). */
static llg_assertion_clock_event_t* assertion_clock_event_alloc(void) {
    llg_assertion_clock_event_t* event = g.assertion_clock_event_pool;
    if (!event)
        return llg_checked_calloc(1, sizeof(*event), "concurrent assertion clock event");
    g.assertion_clock_event_pool = event->next;
    memset(event, 0, sizeof(*event));
    return event;
}

static void assertion_clock_event_recycle(llg_assertion_clock_event_t* event) {
    if (!event) return;
    event->next = g.assertion_clock_event_pool;
    g.assertion_clock_event_pool = event;
}

static void free_assertion_clock_events(llg_concurrent_assertion_t* assertion) {
    while (assertion && assertion->clock_events) {
        llg_assertion_clock_event_t* next = assertion->clock_events->next;
        assertion_clock_event_recycle(assertion->clock_events);
        assertion->clock_events = next;
    }
    if (assertion) {
        assertion->clock_events_tail = NULL;
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            assertion_clock_event_recycle(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
}

static void sequence_attempt_discard(llg_sequence_attempt_t* attempt);
static void free_sequence_pools(void);

static void free_assertion_attempts(llg_concurrent_assertion_t* assertion) {
    free_assertion_clock_events(assertion);
    while (assertion->attempts) {
        llg_assertion_attempt_t* next = assertion->attempts->next;
        free(assertion->attempts);
        assertion->attempts = next;
    }
    assertion->attempts_tail = NULL;
    llg_sequence_attempt_t** lists[] = {
        &assertion->sequence_antecedents,
        &assertion->sequence_consequents,
    };
    llg_sequence_attempt_t** tails[] = {
        &assertion->sequence_antecedents_tail,
        &assertion->sequence_consequents_tail,
    };
    for (size_t list_index = 0; list_index < sizeof(lists) / sizeof(lists[0]);
         list_index++) {
        while (*lists[list_index]) {
            llg_sequence_attempt_t* attempt = *lists[list_index];
            *lists[list_index] = attempt->next;
            sequence_attempt_discard(attempt);
        }
        *tails[list_index] = NULL;
    }
}

static void free_assertions(void) {
    while (g.assertions) {
        llg_concurrent_assertion_t* next = g.assertions->next;
        free_assertion_attempts(g.assertions);
        free(g.assertions->antecedent_rank);
        free(g.assertions->consequent_rank);
        free(g.assertions);
        g.assertions = next;
    }
    g.assertion_tail = NULL;
}

static void free_clocking_edges(void) {
    while (g.clocking_edges) {
        llg_clocking_edge_t* next = g.clocking_edges->next;
        free(g.clocking_edges);
        g.clocking_edges = next;
    }
    free(g.clocking_index);
    g.clocking_index = NULL;
    g.clocking_capacity = 0;
    g.clocking_count = 0;
    g.clocking_used = 0;
}

static void free_clocking_drives(void) {
    while (g.clocking_drives) {
        llg_clocking_drive_t* next = g.clocking_drives->next;
        free_clocking_drive(g.clocking_drives);
        g.clocking_drives = next;
    }
    g.clocking_drives_tail = NULL;
}

static void free_q_queues(void) {
    while (g.q_queues) {
        llg_q_queue_t* queue = g.q_queues;
        g.q_queues = queue->next;
        while (queue->head) {
            llg_q_entry_t* entry = queue->head;
            queue->head = entry->next;
            free(entry);
        }
        free(queue);
    }
}

static void free_mailboxes(void) {
    while (g.mailboxes) {
        llg_mailbox_t* mailbox = g.mailboxes;
        g.mailboxes = mailbox->next;
        while (mailbox->head) {
            llg_mailbox_message_t* message = mailbox_message_pop(mailbox);
            mailbox_value_destroy(&message->value);
            free(message);
        }
        free(mailbox);
    }
}

void llg_rt_cleanup(void) {
    gc_release_registrations();
    value_scopes_unwind(NULL);
    while (root_reference_top) llg_ref_scope_end(root_reference_top);
    for (int i = 0; i < g.force_count; i++) force_free_entry(&g.force_table[i]);
    while (g.inertial_drivers) {
        llg_inertial_t* driver = g.inertial_drivers;
        g.inertial_drivers = driver->next_all;
        *driver->handle = NULL;
        sv4_destroy(&driver->current);
        sv4_destroy(&driver->value);
        sv4_destroy(&driver->mask);
        free(driver);
    }
    free_all_nbas();
    free_deferred_triggers();
    free_deferred_assertions();
    free_assertion_rules();
    // Groups own only child-list nodes; process objects are owned once by
    // all_procs and are released separately below.
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (p && p->fork_groups) {
            free_group_storage(p->fork_groups);
            p->fork_groups = NULL;
            p->fork_groups_tail = NULL;
            p->pending_fork_groups = NULL;
        }
    }
    free_group_storage(g.zombie_groups);
    g.zombie_groups = NULL;

    while (g.strobes) {
        llg_strobe_t* next = g.strobes->next;
        free(g.strobes->fmt);
        if (g.strobes->typed) {
            llg_fmt_args_destroy(g.strobes->typed_work, g.strobes->n);
            free(g.strobes->typed_work);
            free(g.strobes->scope);
        } else {
            sv4_destroy_array(g.strobes->work, (size_t)g.strobes->n);
            free(g.strobes->work);
        }
        free(g.strobes);
        g.strobes = next;
    }
    g.strobe_tail = NULL;
    free(g.mon.fmt);
    if (g.mon.last) sv4_destroy_array(g.mon.last, (size_t)g.mon.n);
    if (g.mon.work) sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
    free(g.mon.last);
    free(g.mon.work);
    free(g.mon.reads);
    llg_fmt_args_destroy(g.mon.typed_last, g.mon.n);
    llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
    free(g.mon.typed_last);
    free(g.mon.typed_work);
    free(g.mon.typed_reads);
    free(g.mon.scope);
    free_region_callbacks();
    free_sampled_values();
    free_assertions();
    free_sequence_pools();
    free_clocking_edges();
    free_clocking_drives();
    free_q_queues();
    llg_string_destroy(&g.time_format.suffix);

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    for (int level = 0; level < g.proc_free_levels; level++)
        free(g.proc_free_bits[level]);
    free(g.wait_sources);
    free(g.all_procs);
    g.all_procs = NULL;
    g.all_procs_capacity = 0;
    free(g.force_table);
    g.force_table = NULL;
    g.force_capacity = 0;
    free_mailboxes();
    // Containers owned by the model may still name these objects; they only
    // drop handles at model close and never dereference them. The handle
    // hooks stay installed so that model teardown after this cleanup still
    // releases counted handles held by containers.
    free_dynamic_events();
    reap_retired_procs();
    rng_scopes_free();
    llg_container_set_rng_source(NULL);
    while (g.programs) {
        llg_program_t* next = g.programs->next;
        free(g.programs);
        g.programs = next;
    }
    while (g.semaphores) {
        llg_semaphore_t* semaphore = g.semaphores;
        g.semaphores = semaphore->next_all;
        while (semaphore->wait_head) {
            llg_semaphore_wait_t* next = semaphore->wait_head->next;
            free(semaphore->wait_head);
            semaphore->wait_head = next;
        }
        semaphore->wait_tail = NULL;
        free(semaphore);
    }
    // External HDL references may keep terminal process identities alive, but
    // no handle may retain a pointer into the context being reset below.
    // A pinned handle's permanent reference ends here; counted holders that
    // outlive the context release (and then free) theirs later.
    llg_process_handle_t* handle = g.process_handles;
    while (handle) {
        llg_process_handle_t* next = handle->next;
        handle->linked = 0;
        handle->prev_link = NULL;
        handle->next = NULL;
        if (handle->pinned) {
            handle->pinned = 0;
            llg_process_release(handle);
        }
        handle = next;
    }
    g.process_handles = NULL;
    if (!llg_file_defer_cleanup) llg_file_cleanup();
    if (llg_event_generation == UINT64_MAX) {
        // A process cannot execute enough complete runtime lifetimes to wrap
        // this counter in practice. Keep the fallback deterministic if a
        // hostile embedding nevertheless reaches the boundary.
        llg_event_generation = 1;
    } else {
        llg_event_generation++;
    }
    memset(&g, 0, sizeof(g));
    while (llg_dependency_bindings) {
        llg_dependency_binding_t* next = llg_dependency_bindings->next;
        free(llg_dependency_bindings);
        llg_dependency_bindings = next;
    }
    free(llg_dependency_buckets);
    llg_dependency_buckets = NULL;
    llg_dependency_bucket_count = 0;
    llg_dependency_binding_count = 0;
    llg_rt_co_cache_release();
}

void llg_rt_init_with_args_and_precision(int argc, char** argv,
                                         uint64_t precision_fs) {
    llg_clear_final_timeformat();
    llg_rt_cleanup();
    llg_warn_host_stack_limit();
    llg_last_failure = 0;
    llg_last_config_error = 0;
    memset(llg_severity_counts, 0, sizeof(llg_severity_counts));
    memset(llg_assertion_failure_counts, 0, sizeof(llg_assertion_failure_counts));
    llg_assertion_cover_count = 0;
    llg_assertion_vacuous_total = 0;
    llg_assertion_event_order = 0;
    finals_release(); // a fresh run never inherits final registrations
    llg_n_finals = 0;
    if (precision_fs == 0) {
        fprintf(stderr, "llg: runtime precision must be non-zero\n");
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_timeformat_defaults(precision_fs);
    if (!configure_limits() || !configure_stop_policy() || !configure_output_files() ||
        !gc_runtime_init()) {
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_configured_zero_loop_limit = g.zero_loop_limit;
    llg_configured_process_step_limit = g.process_step_limit;
    llg_configured_stop_policy = g.stop_policy;
    g.initialized = 1;
    install_value_handle_hooks();
    g.current_region = LLG_REGION_PREPONED;
    llg_rng_state_seed(&g.rng_root, LLG_RNG_DEFAULT_SEED);
    llg_container_set_rng_source(llg_container_thread_rng);
    g.argc = argc > 0 ? argc : 0;
    g.argv = g.argc > 0 ? argv : NULL;
}

void llg_rt_init_with_args(int argc, char** argv) {
    llg_rt_init_with_args_and_precision(argc, argv, 1);
}

void llg_rt_init_with_precision(uint64_t precision_fs) {
    llg_rt_init_with_args_and_precision(0, NULL, precision_fs);
}

void llg_rt_init(void) {
    llg_rt_init_with_args_and_precision(0, NULL, 1);
}
