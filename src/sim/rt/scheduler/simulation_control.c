
static void report_finish(int verbosity, const char* location) {
    if (verbosity >= 1) {
        fprintf(stderr, "llg: $finish at time %llu",
                (unsigned long long)g.now);
        if (location && location[0] != '\0') fprintf(stderr, " at %s", location);
        fputc('\n', stderr);
    }
    if (verbosity >= 2) {
        fprintf(stderr, "llg: simulation statistics: processes=%d\n", g.n_procs);
        if (llg_severity_counts[LLG_SEVERITY_INFO] != 0 ||
            llg_severity_counts[LLG_SEVERITY_WARNING] != 0 ||
            llg_severity_counts[LLG_SEVERITY_ERROR] != 0 ||
            llg_severity_counts[LLG_SEVERITY_FATAL] != 0) {
            fprintf(stderr,
                    "llg: severity counts: info=%llu warning=%llu error=%llu fatal=%llu\n",
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_INFO],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_WARNING],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_ERROR],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_FATAL]);
        }
        if (llg_assertion_failure_counts[LLG_ASSERTION_ASSERT] != 0 ||
            llg_assertion_failure_counts[LLG_ASSERTION_ASSUME] != 0 ||
            llg_assertion_failure_counts[LLG_ASSERTION_EXPECT] != 0 ||
            llg_assertion_cover_count != 0) {
            if (llg_assertion_failure_counts[LLG_ASSERTION_EXPECT] != 0) {
                fprintf(stderr,
                        "llg: assertion counts: assert_failed=%llu assume_failed=%llu expect_failed=%llu cover=%llu\n",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_EXPECT],
                        (unsigned long long)llg_assertion_cover_count);
            } else {
                fprintf(stderr,
                        "llg: assertion counts: assert_failed=%llu assume_failed=%llu cover=%llu\n",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_cover_count);
            }
        }
        if (llg_assertion_vacuous_total != 0)
            fprintf(stderr, "llg: assertion vacuous=%llu\n",
                    (unsigned long long)llg_assertion_vacuous_total);
    }
}

_Noreturn void llg_rt_finish_with_level(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $finish verbosity %d\n", verbosity);
        abort();
    }
    run_deferred_assertions_now();
    report_finish(verbosity, location);
    g.finish = 1;
    llg_proc_done(llg_current());
}

_Noreturn void llg_rt_finish(void) {
    llg_rt_finish_with_level(0, NULL);
}

void llg_rt_request_finish(void) {
    g.finish = 1;
}

void llg_rt_mark_failed(void) {
    llg_last_failure = 1;
}

static void report_stop(int verbosity, const char* location) {
    if (verbosity >= 1) {
        fprintf(stderr, "llg: $stop at time %llu",
                (unsigned long long)g.now);
        if (location && location[0] != '\0') fprintf(stderr, " at %s", location);
        fputc('\n', stderr);
    }
    if (verbosity >= 2) {
        fprintf(stderr, "llg: simulation statistics: processes=%d\n", g.n_procs);
    }
}

static int resume_stopped_process(void) {
    if (!g.suspended || !g.stop_proc) return 0;
    llg_proc_t* process = g.stop_proc;
    if (process->killed || process->completed) {
        fprintf(stderr, "llg runtime fatal: stopped process is no longer resumable\n");
        llg_last_failure = 1;
        g.finish = 1;
        g.suspended = 0;
        g.stop_proc = NULL;
        return 0;
    }
    g.stop_proc = NULL;
    g.suspended = 0;
    g.current_region = g.stop_region;
    enqueue_region(process, g.stop_region);
    return 1;
}

void llg_rt_stop_with_level(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $stop verbosity %d\n", verbosity);
        abort();
    }
    llg_proc_t* process = llg_current();
    if (!g.running || !process || g.suspended || g.stop_proc) {
        fprintf(stderr, "llg runtime fatal: $stop requires a running simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    report_stop(verbosity, location);
    g.stop_proc = process;
    g.stop_region = process->region;
    g.suspended = 1;
    // The process remains live and its stack/frame/queued NBA state remains
    // owned by the scheduler. Returning from this yield resumes immediately
    // at the statement following `$stop`.
    aco_yield();
}

void llg_rt_stop(void) {
    llg_rt_stop_with_level(0, NULL);
}

int llg_rt_set_stop_policy(int policy) {
    if (policy != LLG_STOP_POLICY_RESUME && policy != LLG_STOP_POLICY_EXIT) return 0;
    if (g.running) return 0;
    llg_stop_policy_override = 1;
    llg_configured_stop_policy = policy;
    g.stop_policy = policy;
    return 1;
}

int llg_rt_stop_policy(void) {
    return g.main_co ? g.stop_policy : llg_configured_stop_policy;
}

int llg_rt_is_suspended(void) { return g.suspended != 0; }

int llg_rt_resume(void) {
    if (g.running) return 0;
    return resume_stopped_process();
}

uint64_t llg_time(void) { return g.now; }

static void insert_region_callback(llg_region_callback_t* entry) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && ((*slot)->time < entry->time ||
                     ((*slot)->time == entry->time &&
                      ((*slot)->region < entry->region ||
                       ((*slot)->region == entry->region &&
                        (*slot)->sequence < entry->sequence))))) {
        slot = &(*slot)->next;
    }
    entry->next = *slot;
    *slot = entry;
}

int llg_schedule_region_callback_after(llg_region_t region,
                                       llg_region_callback_fn callback,
                                       void* data, uint64_t ticks) {
    if (!callback || !callback_region_allowed(region, ticks)) return 0;
    if (ticks > UINT64_MAX - g.now || g.callback_sequence == UINT64_MAX) {
        fprintf(stderr, "llg: fatal: region callback time or sequence overflow\n");
        abort();
    }
    llg_region_callback_t* entry = (llg_region_callback_t*)llg_checked_malloc(
        1, sizeof(*entry), "region callback");
    entry->region = region;
    entry->time = g.now + ticks;
    entry->sequence = g.callback_sequence++;
    entry->callback = callback;
    entry->data = data;
    entry->next = NULL;
    insert_region_callback(entry);
    return 1;
}

int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback_after(region, callback, data, 0);
}

int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback(region, callback, data);
}
