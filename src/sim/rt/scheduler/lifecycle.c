
// ── Public scheduler API ──────────────────────────────────────────────────────

static void llg_last_word(void) {
    fprintf(stderr, "llg: fatal: coroutine returned without aco_exit "
                    "(codegen bug)\n");
    abort();
}

static void llg_proc_entry(void) {
    llg_proc_t* self = (llg_proc_t*)aco_get_arg();
    self->fn(self);
    llg_last_word(); // never reached when the body called llg_proc_done
}

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
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* next = n->next;
        if (n->is_string) llg_string_destroy(&n->string_value);
        free(n);
        n = next;
    }
    event_unlink(&p->wait);
    event_triggered_unlink(&p->wait);
    semaphore_waiter_unlink(&p->wait);
    if (p->wait.kind == W_MAILBOX_GET || p->wait.kind == W_MAILBOX_PUT)
        mailbox_unlink_wait(&p->wait);
    mailbox_value_destroy(&p->wait.mailbox_value);
    free_expression_wait(&p->wait);
    free(p->wait.specs);
    free(p->wait.dependencies);
    free(p->wait.last);
    free(p->wait.real_last);
    free(p->wait.evs);
    free(p->wait.order_sequence);
    llg_process_release(p->wait.process_target);
    p->wait.process_target = NULL;
    activation_unwind_proc(p);
    llg_frame_release(p->frame);
    p->frame = NULL;
    process_local_release_all(p);
    process_handle_shutdown(p);
    if (p->co) aco_destroy(p->co);
    free(p);
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
    while (g.sampled) {
        llg_sampled_value_t* next = g.sampled->next;
        while (g.sampled->history) {
            llg_sampled_history_t* history = g.sampled->history;
            g.sampled->history = history->next;
            free(history);
        }
        free(g.sampled);
        g.sampled = next;
    }
    while (g.sampled_domains) {
        llg_sampled_domain_t* next = g.sampled_domains->next;
        while (g.sampled_domains->history) {
            llg_sampled_domain_history_t* history = g.sampled_domains->history;
            g.sampled_domains->history = history->next;
            free(history);
        }
        free(g.sampled_domains);
        g.sampled_domains = next;
    }
}

static void free_assertion_clock_events(llg_concurrent_assertion_t* assertion) {
    while (assertion && assertion->clock_events) {
        llg_assertion_clock_event_t* next = assertion->clock_events->next;
        free(assertion->clock_events);
        assertion->clock_events = next;
    }
    if (assertion) {
        assertion->clock_events_tail = NULL;
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            free(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
}

static void sequence_attempt_discard(llg_sequence_attempt_t* attempt);

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
    while (root_reference_top) llg_ref_scope_end(root_reference_top);
    for (int i = 0; i < g.force_count; i++) force_free_entry(&g.force_table[i]);
    while (g.inertial_drivers) {
        llg_inertial_t* driver = g.inertial_drivers;
        g.inertial_drivers = driver->next_all;
        *driver->handle = NULL;
        free(driver);
    }
    while (g.delayed_nbas) {
        llg_nba_t* next = g.delayed_nbas->next;
        if (g.delayed_nbas->is_string)
            llg_string_destroy(&g.delayed_nbas->string_value);
        free(g.delayed_nbas);
        g.delayed_nbas = next;
    }
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
            free(g.strobes->work);
        }
        free(g.strobes);
        g.strobes = next;
    }
    g.strobe_tail = NULL;
    free(g.mon.fmt);
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
    free_clocking_edges();
    free_clocking_drives();
    free_q_queues();
    llg_string_destroy(&g.time_format.suffix);

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    free_mailboxes();
    reap_retired_procs();
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
    llg_process_handle_t* handle = g.process_handles;
    while (handle) {
        llg_process_handle_t* next = handle->next;
        handle->linked = 0;
        handle->next = NULL;
        handle = next;
    }
    g.process_handles = NULL;
    if (g.share_stack) aco_share_stack_destroy(g.share_stack);
    if (g.main_co) aco_destroy(g.main_co);
    aco_gtls_co = NULL;
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
}

void llg_rt_init_with_args_precision_and_stack(int argc, char** argv,
                                               uint64_t precision_fs,
                                               size_t stack_values) {
    llg_clear_final_timeformat();
    llg_rt_cleanup();
    llg_last_failure = 0;
    llg_last_config_error = 0;
    memset(llg_severity_counts, 0, sizeof(llg_severity_counts));
    memset(llg_assertion_failure_counts, 0, sizeof(llg_assertion_failure_counts));
    llg_assertion_cover_count = 0;
    llg_assertion_vacuous_total = 0;
    llg_assertion_event_order = 0;
    llg_n_finals = 0; // a fresh run never inherits final registrations
    if (precision_fs == 0) {
        fprintf(stderr, "llg: runtime precision must be non-zero\n");
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    if (stack_values == 0) {
        fprintf(stderr, "llg: runtime coroutine stack value count must be non-zero\n");
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_stack_values = stack_values;
    llg_timeformat_defaults(precision_fs);
    if (!configure_limits() || !configure_stop_policy()) {
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_configured_zero_loop_limit = g.zero_loop_limit;
    llg_configured_process_step_limit = g.process_step_limit;
    llg_configured_stop_policy = g.stop_policy;
    g.current_region = LLG_REGION_PREPONED;
    llg_rng_state_seed(&g.rng_root, LLG_RNG_DEFAULT_SEED);
    g.argc = argc > 0 ? argc : 0;
    g.argv = g.argc > 0 ? argv : NULL;
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
}

void llg_rt_init_with_args_and_precision(int argc, char** argv,
                                         uint64_t precision_fs) {
    llg_rt_init_with_args_precision_and_stack(
        argc, argv, precision_fs, LLG_DEFAULT_STACK_VALUES);
}

void llg_rt_init_with_stack(size_t stack_values) {
    llg_rt_init_with_args_precision_and_stack(0, NULL, 1, stack_values);
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
