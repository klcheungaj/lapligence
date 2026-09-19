
static void report_zero_delay_loop(void) {
    const char* where = g.last_process_name ? g.last_process_name : "<scheduler>";
    fprintf(stderr,
            "llg: zero-delay loop detected at time %llu in process `%s` "
            "(scheduler pass limit %llu)\n",
            (unsigned long long)g.now, where,
            (unsigned long long)g.zero_loop_limit);
    llg_last_failure = 1;
    g.finish = 1;
}

static int callback_pending(llg_region_t region) {
    for (llg_region_callback_t* entry = g.callbacks; entry; entry = entry->next) {
        if (entry->time > g.now) break;
        if (entry->time == g.now && entry->region == region) return 1;
    }
    return 0;
}

static llg_region_callback_t* take_region_callback(llg_region_t region) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && (*slot)->time <= g.now) {
        llg_region_callback_t* entry = *slot;
        if (entry->time == g.now && entry->region == region) {
            *slot = entry->next;
            entry->next = NULL;
            return entry;
        }
        slot = &entry->next;
    }
    return NULL;
}

static int region_pending(llg_region_t region) {
    return g.process_queues[region].head != NULL || callback_pending(region);
}

static int run_region_queue(llg_region_t region) {
    g.current_region = region;
    while (!g.finish) {
        llg_region_callback_t* callback = take_region_callback(region);
        llg_proc_t* process = callback ? NULL : dequeue_region(region);
        if (!callback && !process) break;
        if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
            free(callback);
            report_zero_delay_loop();
            return 0;
        }
        if (callback) {
            llg_region_callback_fn fn = callback->callback;
            void* data = callback->data;
            free(callback);
            fn(data);
        } else {
            g.last_process_name = process->name;
            process->region = region;
            aco_resume(process->co);
            reap_retired_procs();
        }
        if (g.suspended) {
            if (g.stop_policy == LLG_STOP_POLICY_RESUME) {
                // The llg CLI is noninteractive. Its default policy resumes
                // the exact coroutine continuation in the same time slot,
                // while retaining all other queued work and state.
                if (!resume_stopped_process()) break;
            } else {
                break;
            }
        }
    }
    return !g.finish && !g.suspended;
}

static void wake_zero_waits(llg_region_t region) {
    llg_wait_queue_t* queue = &g.zero_waits[region];
    llg_wait_t* wait = queue->head;
    queue->head = NULL;
    queue->tail = NULL;
    while (wait) {
        llg_wait_t* next = wait->region_next;
        wait->region_next = NULL;
        wake_proc(wait->proc);
        wait = next;
    }
}

static int design_pending(void) {
    for (llg_region_t region = LLG_REGION_ACTIVE;
         region <= LLG_REGION_POST_NBA_PLI; region++) {
        if (region_pending(region)) return 1;
    }
    int pending = g.zero_waits[LLG_REGION_INACTIVE].head != NULL ||
                  inertial_ready(LLG_REGION_ACTIVE) || nba_due(LLG_REGION_NBA);
    return pending;
}

static int reactive_pending(void) {
    for (llg_region_t region = LLG_REGION_REACTIVE;
         region <= LLG_REGION_POST_RE_NBA_PLI; region++) {
        if (region_pending(region)) return 1;
    }
    return g.zero_waits[LLG_REGION_RE_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_REACTIVE) || nba_due(LLG_REGION_RE_NBA);
}

static int drain_design_set(void) {
    for (;;) {
        while (!g.finish &&
               (region_pending(LLG_REGION_ACTIVE) ||
                inertial_ready(LLG_REGION_ACTIVE))) {
            if (inertial_ready(LLG_REGION_ACTIVE)) {
                g.current_region = LLG_REGION_ACTIVE;
                if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                    report_zero_delay_loop();
                    return 0;
                }
                commit_inertial(LLG_REGION_ACTIVE);
            } else if (!run_region_queue(LLG_REGION_ACTIVE)) {
                return 0;
            }
        }
        if (g.finish) return 0;
        if (g.zero_waits[LLG_REGION_INACTIVE].head ||
            region_pending(LLG_REGION_INACTIVE)) {
            if (g.zero_waits[LLG_REGION_INACTIVE].head)
                wake_zero_waits(LLG_REGION_INACTIVE);
            if (!run_region_queue(LLG_REGION_INACTIVE)) return 0;
            continue;
        }
        break;
    }
    return 1;
}

// Earlier-phase work enabled by a callback precedes the next phase. NBA
// batches themselves retain issue order before another Active iteration.
static int design_pending_before(llg_region_t stop) {
    for (llg_region_t region = LLG_REGION_ACTIVE; region < stop; region++)
        if (region_pending(region)) return 1;
    return g.zero_waits[LLG_REGION_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_ACTIVE) ||
           (stop > LLG_REGION_NBA && nba_due(LLG_REGION_NBA));
}

static int run_design_set(void) {
    for (;;) {
        if (!drain_design_set()) return 0;
        if (!run_region_queue(LLG_REGION_PRE_NBA_PLI)) return 0;
        if (design_pending_before(LLG_REGION_PRE_NBA_PLI)) continue;
        if (!run_region_queue(LLG_REGION_PRE_NBA)) return 0;
        if (design_pending_before(LLG_REGION_PRE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_NBA)) return 0;
        if (nba_due(LLG_REGION_NBA)) {
            g.current_region = LLG_REGION_NBA;
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                return 0;
            }
            commit_nbas(LLG_REGION_NBA);
        }
        if (g.finish) return 0;
        if (design_pending_before(LLG_REGION_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_NBA)) return 0;
        if (design_pending_before(LLG_REGION_POST_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_NBA_PLI)) return 0;
        process_zombie_groups();
        if (!design_pending()) return 1;
    }
}

static int run_observed_set(void) {
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED_PLI)) return 0;
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED)) return 0;
    if (!run_region_queue(LLG_REGION_OBSERVED)) return 0;
    if (!run_concurrent_assertions()) return 0;
    flush_deferred_assertions();
    if (g.finish) return 0;
    if (!run_region_queue(LLG_REGION_POST_OBSERVED)) return 0;
    return run_region_queue(LLG_REGION_POST_OBSERVED_PLI);
}

static int drain_reactive_set(void) {
    for (;;) {
        while (!g.finish &&
               (region_pending(LLG_REGION_REACTIVE) ||
                inertial_ready(LLG_REGION_REACTIVE))) {
            if (inertial_ready(LLG_REGION_REACTIVE)) {
                g.current_region = LLG_REGION_REACTIVE;
                if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                    report_zero_delay_loop();
                    return 0;
                }
                commit_inertial(LLG_REGION_REACTIVE);
            } else if (!run_region_queue(LLG_REGION_REACTIVE)) {
                return 0;
            }
        }
        if (g.finish) return 0;
        // A deferred assertion can itself be evaluated by a Reactive
        // callback. Keep that newly coalesced report in the same Reactive
        // fixed-point pass, after the current queue has drained.
        if (g.deferred_assertions) {
            flush_deferred_assertions();
            if (g.finish) return 0;
            continue;
        }
        if (g.zero_waits[LLG_REGION_RE_INACTIVE].head ||
            region_pending(LLG_REGION_RE_INACTIVE)) {
            if (g.zero_waits[LLG_REGION_RE_INACTIVE].head)
                wake_zero_waits(LLG_REGION_RE_INACTIVE);
            if (!run_region_queue(LLG_REGION_RE_INACTIVE)) return 0;
            continue;
        }
        break;
    }
    return 1;
}

static int reactive_pending_before(llg_region_t stop) {
    for (llg_region_t region = LLG_REGION_REACTIVE; region < stop; region++)
        if (region_pending(region)) return 1;
    return g.zero_waits[LLG_REGION_RE_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_REACTIVE) ||
           (stop > LLG_REGION_RE_NBA && nba_due(LLG_REGION_RE_NBA));
}

static int run_reactive_set(void) {
    // Exhaust the reactive set before returning to newly enabled design work
    // (IEEE 1800-2009 4.5).
    for (;;) {
        if (!drain_reactive_set()) return 0;
        if (!run_region_queue(LLG_REGION_PRE_RE_NBA_PLI)) return 0;
        if (reactive_pending_before(LLG_REGION_PRE_RE_NBA_PLI)) continue;
        if (!run_region_queue(LLG_REGION_PRE_RE_NBA)) return 0;
        if (reactive_pending_before(LLG_REGION_PRE_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_RE_NBA)) return 0;
        if (nba_due(LLG_REGION_RE_NBA)) {
            g.current_region = LLG_REGION_RE_NBA;
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                return 0;
            }
            commit_nbas(LLG_REGION_RE_NBA);
        }
        if (g.finish) return 0;
        if (reactive_pending_before(LLG_REGION_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_RE_NBA)) return 0;
        if (reactive_pending_before(LLG_REGION_POST_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_RE_NBA_PLI)) return 0;
        process_zombie_groups();
        if (!reactive_pending()) return 1;
    }
}

static int run_pre_postponed_set(void) {
    if (!run_region_queue(LLG_REGION_PRE_POSTPONED_PLI)) return 0;
    return run_region_queue(LLG_REGION_PRE_POSTPONED);
}

static int run_postponed_set(void) {
    if (!run_region_queue(LLG_REGION_POSTPONED)) return 0;
    g.current_region = LLG_REGION_POSTPONED;
    flush_strobes();
    if (g.finish) return 0;
    check_monitor();
    if (g.finish) return 0;
    return run_region_queue(LLG_REGION_POSTPONED_PLI);
}

// ── final blocks (see llg_rt.h) ─────────────────────────────────────────────

void llg_spawn_final(void (*fn)(llg_proc_t*), const char* name) {
    if (llg_n_finals == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: final block registry size overflow\n");
        abort();
    }
    finals_reserve(llg_n_finals + 1);
    llg_finals[llg_n_finals].fn = fn;
    llg_finals[llg_n_finals].name = name;
    llg_n_finals++;
}

void llg_rt_run_finals(void) {
    if (llg_n_finals == 0) {
        llg_clear_final_timeformat();
        return;
    }
    // `$stop` is a resumable scheduler suspension, not a simulation exit.
    // Do not run final procedures while an embedding has intentionally
    // returned control to its caller under the EXIT policy.
    if (g.suspended) return;
    if (llg_last_config_error) {
        llg_clear_final_timeformat();
        finals_release();
        llg_n_finals = 0;
        return;
    }
    // Explicit reset, decoupled from the cleanup-memset invariant: a stale
    // $finish flag left by the scheduler exit must never read as
    // "$finish inside a final" after the first final completes.
    g.finish = 0;
    g.running = 0;
    g.current_region = LLG_REGION_POSTPONED;
    // Rebuild a minimal coroutine context: the scheduler-exit teardown in
    // llg_rt_run released the previous one.
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
    llg_restore_final_timeformat();
    g.now = llg_final_time;
    g.zero_loop_limit = llg_configured_zero_loop_limit;
    g.process_step_limit = llg_configured_process_step_limit;
    g.stop_policy = llg_configured_stop_policy;
    llg_in_finals = 1;
    for (int i = 0; i < llg_n_finals; i++) {
        llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
            1, sizeof(llg_proc_t), "final process");
        p->name = llg_finals[i].name;
        p->fn = llg_finals[i].fn;
        p->handle = process_handle_new(p);
        p->status = LLG_PROCESS_RUNNING;
        p->budget_time = g.now;
        p->region = LLG_REGION_POSTPONED;
        p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
        register_proc(p);
        enqueue_region(p, LLG_REGION_POSTPONED);
        run_region_queue(LLG_REGION_POSTPONED);
        if (p->wait.kind != W_NONE) {
            fprintf(stderr,
                    "llg: fatal: final block `%s` suspended on a wait "
                    "(timing controls are rejected by codegen)\n",
                    p->name ? p->name : "final");
            abort();
        }
        // Finals permit function statements only. Codegen rejects NBAs,
        // deferred output tasks, waits, and forks, so no scheduler region is
        // run between these sequential zero-time calls.
        if (g.finish) break;
    }
    llg_in_finals = 0;
    llg_rt_cleanup();
    finals_release();
    llg_n_finals = 0;
}

void llg_rt_run(void) {
    if (g.config_error) {
        llg_rt_cleanup();
        return;
    }
    // A caller must explicitly acknowledge an EXIT-policy stop before the
    // scheduler can advance. This keeps future queues and the suspended
    // coroutine untouched when an embedding probes the runtime again.
    if (g.suspended) return;
    g.running = 1;
    g.current_region = LLG_REGION_PREPONED;
    for (;;) {
        if (g.finish) break;
        sample_preponed_values();
        if (!run_region_queue(LLG_REGION_PREPONED)) break;
        if (!run_region_queue(LLG_REGION_PREPONED_PLI)) break;
        if (!run_region_queue(LLG_REGION_PRE_ACTIVE_PLI)) break;
        for (;;) {
            if (!run_design_set()) break;
            if (!run_observed_set()) break;
            if (!run_reactive_set()) break;
            if (design_pending() || reactive_pending()) continue;
            if (!run_pre_postponed_set()) break;
            if (design_pending() || reactive_pending()) continue;
            break;
        }
        if (g.finish) break;
        if (!run_postponed_set()) break;
        if (g.finish) break;
        int have_future_event = g.timed_head || g.delayed_nbas ||
                                g.inertial_pending || g.callbacks;
        uint64_t t = g.timed_head ? g.timed_head->time : UINT64_MAX;
        if (g.delayed_nbas && g.delayed_nbas->time < t) t = g.delayed_nbas->time;
        if (g.inertial_pending && g.inertial_pending->time < t) t = g.inertial_pending->time;
        if (g.callbacks && g.callbacks->time < t) t = g.callbacks->time;
        if (!have_future_event) {
            if (g.wait_count == 0) {
                fprintf(stderr, "llg: simulation ended without $finish "
                                "(no processes remain) at time %llu\n",
                        (unsigned long long)g.now);
            } else {
                fprintf(stderr, "llg: simulation deadlock at time %llu "
                                "(waiters never woken, no future events)\n",
                        (unsigned long long)g.now);
            }
            break;
        }
        if (t <= g.now) {
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                break;
            }
        } else {
            g.now = t;
            g.region_passes = 0;
        }
        llg_wait_t* wait = g.timed_head;
        while (wait && wait->time == g.now) {
            llg_wait_t* next = wait->time_next;
            wake_proc(wait->proc);
            wait = next;
        }
        g.current_region = LLG_REGION_PREPONED;
    }
    if (g.suspended) {
        // EXIT-policy suspension is deliberately resumable. Keep all
        // scheduler queues, coroutine stacks, activations and output state in
        // place; an embedding can call llg_rt_resume() and llg_rt_run().
        g.running = 0;
        return;
    }
    run_deferred_assertions_now();
    flush_assertion_attempts();
    // Finals ($time inside them) report when the scheduler loop ended.
    llg_final_time = g.now;
    // No pending update or deferred evaluator executes between $finish and
    // final procedures, regardless of whether its issuing process completed.
    g.running = 0;
    llg_file_defer_cleanup = llg_n_finals != 0;
    if (llg_n_finals > 0) llg_save_final_timeformat();
    llg_rt_cleanup();
    llg_file_defer_cleanup = 0;
}
