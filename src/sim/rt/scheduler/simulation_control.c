// ── Message frames ───────────────────────────────────────────────────────────
//
// A status line, severity task or assertion failure names its site with one
// compile-time string: the design scope, the unit separator, then the
// physical `file:line:col` (`finish_location` in the lowering). A site
// without the separator is a bare location when it ends in `:line:col`
// (optionally followed by a `line suffix) after a name with a `.` and
// without " at " (a file name), else a scope or a runtime label such as
// "method call at tb:15:9".

#define LLG_RT_SITE_SEPARATOR '\037'

static int llg_rt_site_is_location(const char* text, size_t len) {
    const char* suffix = NULL;
    size_t i;
    for (i = 0; i + 8 <= len; i++)
        if (memcmp(text + i, " (`line ", 8) == 0) {
            suffix = text + i;
            break;
        }
    size_t end = suffix ? (size_t)(suffix - text) : len;
    int fields = 0;
    while (fields < 2) {
        size_t digits = 0;
        while (end > 0 && text[end - 1] >= '0' && text[end - 1] <= '9') {
            end--;
            digits++;
        }
        if (digits == 0 || end == 0 || text[end - 1] != ':') return 0;
        end--;
        fields++;
    }
    int dotted = 0;
    for (i = 0; i < end; i++) {
        if (text[i] == '.') dotted = 1;
        if (i + 4 <= end && memcmp(text + i, " at ", 4) == 0) return 0;
    }
    return end > 0 && dotted;
}

// The simulation time `g.now` in the largest unit that shows it exactly,
// with a space before the unit ("5 ns").
static void llg_rt_time_text(char* out, size_t cap) {
    static const char* const units[] = {"fs", "ps", "ns", "us", "ms", "s"};
    uint64_t precision = g.design_precision_fs ? g.design_precision_fs : 1u;
    uint64_t scale = 1;
    int unit = 0;
    while (unit < 5 && precision % (scale * 1000u) == 0) {
        scale *= 1000u;
        unit++;
    }
    uint64_t multiplier = precision / scale;
    if (multiplier != 0 && g.now > UINT64_MAX / multiplier) {
        snprintf(out, cap, "%llu x %llu fs", (unsigned long long)g.now,
                 (unsigned long long)precision);
        return;
    }
    uint64_t value = g.now * multiplier;
    while (value != 0 && unit < 5 && value % 1000u == 0) {
        value /= 1000u;
        unit++;
    }
    snprintf(out, cap, "%llu %s", (unsigned long long)value, units[unit]);
}

// Append message `id` framed by `site` and the current time. `scope` names
// the context when the site carries none.
static void llg_rt_framed(llg_report_t* out, llg_msg_id_t id, int severity,
                          const char* site, const char* scope, const char* text,
                          size_t text_len) {
    char scope_inline[256];
    char* site_scope = NULL;
    const char* location = NULL;
    char time_text[64];
    if (site && site[0]) {
        const char* separator = strchr(site, LLG_RT_SITE_SEPARATOR);
        if (separator) {
            size_t len = (size_t)(separator - site);
            site_scope = len < sizeof scope_inline ? scope_inline : (char*)malloc(len + 1u);
            if (site_scope) {
                memcpy(site_scope, site, len);
                site_scope[len] = '\0';
            }
            location = separator + 1;
        } else if (llg_rt_site_is_location(site, strlen(site))) {
            location = site;
        } else if (!scope || !scope[0]) {
            scope = site;
        }
    }
    llg_rt_time_text(time_text, sizeof time_text);
    llg_msg_append(out, id, severity, location, site_scope ? site_scope : scope, time_text,
                   text, text_len);
    if (site_scope && site_scope != scope_inline) free(site_scope);
}

static void llg_rt_framed_line(llg_msg_id_t id, const char* site, const char* text) {
    llg_report_t report;
    llg_report_init(&report);
    llg_rt_framed(&report, id, llg_msg_default_severity(id), site, NULL, text, strlen(text));
    llg_report_emit(&report, stderr);
}

LLG_MSG_EXPORT_COUNTS(scheduler)

// Units that define LLG_MSG_COUNT_HOOK (VPI, DPI-C contexts) count here.
void llg_rt_count_message(int severity) {
    if (severity >= 0 && severity < 4 && llg_msg_counts[severity] != UINT64_MAX)
        llg_msg_counts[severity]++;
}

// The end-of-run summary over every runtime unit that reports (SIM-000),
// printed when the run reported a warning or worse.
static void llg_rt_report_summary(void) {
    static int printed;
    uint64_t counts[4];
    int severity;
    if (printed) return;
    for (severity = 0; severity < 4; severity++) {
        counts[severity] = llg_msg_count_scheduler(severity) + llg_msg_count_container(severity);
#ifdef LLG_WAVEFORM
        counts[severity] += llg_msg_count_waveform(severity);
#endif
    }
    if (!counts[LLG_SEVERITY_WARNING] && !counts[LLG_SEVERITY_ERROR] &&
        !counts[LLG_SEVERITY_FATAL])
        return;
    printed = 1;
    llg_report_t text;
    llg_report_t report;
    llg_report_init(&text);
    llg_report_init(&report);
    llg_msg_summary_text(&text, counts);
    llg_msg_append(&report, LLG_MSG_SUMMARY, LLG_SEVERITY_INFO, NULL, NULL, NULL, text.data,
                   text.len);
    llg_report_free(&text);
    llg_report_emit(&report, stderr);
}


static void report_finish(int verbosity, const char* location) {
    if (verbosity >= 1) llg_rt_framed_line(LLG_MSG_FINISH, location, "$finish called");
    if (verbosity >= 2) {
        llg_msg(LLG_MSG_STATISTICS, NULL, "simulation statistics: processes=%d", g.n_procs);
        if (llg_severity_counts[LLG_SEVERITY_INFO] != 0 ||
            llg_severity_counts[LLG_SEVERITY_WARNING] != 0 ||
            llg_severity_counts[LLG_SEVERITY_ERROR] != 0 ||
            llg_severity_counts[LLG_SEVERITY_FATAL] != 0) {
            llg_msg(LLG_MSG_STATISTICS, NULL,
                    "severity counts: info=%llu warning=%llu error=%llu fatal=%llu",
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
                llg_msg(LLG_MSG_STATISTICS, NULL,
                        "assertion counts: assert_failed=%llu assume_failed=%llu expect_failed=%llu cover=%llu",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_EXPECT],
                        (unsigned long long)llg_assertion_cover_count);
            } else {
                llg_msg(LLG_MSG_STATISTICS, NULL,
                        "assertion counts: assert_failed=%llu assume_failed=%llu cover=%llu",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_cover_count);
            }
        }
        if (llg_assertion_vacuous_total != 0)
            llg_msg(LLG_MSG_STATISTICS, NULL, "assertion vacuous=%llu",
                    (unsigned long long)llg_assertion_vacuous_total);
    }
}

void llg_rt_finish_with_level(int verbosity, const char* location) {
    llg_proc_t* current = llg_current();
    llg_runtime_service_enter(current, "$finish");
    if (verbosity < 0 || verbosity > 2) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid $finish verbosity %d", verbosity);
        abort();
    }
    deferred_assertions_finish();
    report_finish(verbosity, location);
    g.finish = 1;
    if (current) current->chain.exiting = LLG_EXIT_COMPLETE;
}

void llg_rt_finish(void) {
    llg_rt_finish_with_level(0, NULL);
}

void llg_rt_request_finish(void) {
    g.finish = 1;
    if (g.current) g.current->chain.exiting = LLG_EXIT_COMPLETE;
}

int llg_rt_exiting(void) {
    return g.current && g.current->chain.exiting != LLG_EXIT_NONE;
}

const char* llg_static_init_variable = NULL;
static const char* llg_static_init_location = NULL;

void llg_rt_static_init_enter(const char* variable, const char* location) {
    llg_static_init_variable = variable;
    llg_static_init_location = location;
}

void llg_rt_static_init_leave(void) {
    llg_static_init_variable = NULL;
    llg_static_init_location = NULL;
}

void llg_rt_static_effect(const char* what, const char* location,
                          const char* subroutine) {
    llg_rt_mark_failed();
    llg_msg(LLG_MSG_RUNTIME_ERROR, NULL, "%s: %s in function '%s' schedules work after the call returns, during the static initializer of '%s' at %s; a function may do this only when its calling thread was created by an initial or always procedure or a fork of one (SV 13.4.4)", location, what, subroutine, llg_static_init_variable, llg_static_init_location);
    fflush(stderr);
    llg_rt_skip_finals();
    llg_rt_fatal_typed(0, "deferred statement during static initialization",
                       NULL, 0, "", location);
}

void llg_rt_mark_failed(void) {
    llg_last_failure = 1;
}

void llg_rt_skip_finals(void) {
    llg_skip_final_procedures = 1;
}

static void report_stop(int verbosity, const char* location) {
    if (verbosity >= 1) llg_rt_framed_line(LLG_MSG_STOP, location, "$stop called");
    if (verbosity >= 2)
        llg_msg(LLG_MSG_STATISTICS, NULL, "simulation statistics: processes=%d", g.n_procs);
}

static int resume_stopped_process(void) {
    if (!g.suspended) return 0;
    if (!g.stop_proc) {
        g.suspended = 0;
        return 1;
    }
    llg_proc_t* process = g.stop_proc;
    if (process->killed || process->completed) {
        llg_msg(LLG_MSG_RUNTIME_ERROR, NULL, "stopped process is no longer resumable");
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

llg_co_arm_t llg_arm_stop(llg_proc_t* self, int verbosity,
                          const char* location) {
    llg_runtime_service_enter(self, "$stop");
    if (verbosity < 0 || verbosity > 2) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid $stop verbosity %d", verbosity);
        abort();
    }
    if (!g.running || !g.process_turn_active || !self || self != g.current ||
        g.suspended || g.stop_proc) {
        llg_msg(LLG_MSG_RUNTIME_ERROR, NULL, "$stop requires a running simulation process");
        llg_last_failure = 1;
        g.finish = 1;
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    report_stop(verbosity, location);
    g.stop_proc = self;
    g.stop_region = self->region;
    g.suspended = 1;
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

void llg_rt_request_stop(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid $stop verbosity %d", verbosity);
        abort();
    }
    report_stop(verbosity, location);
    if (!g.running || !g.process_turn_active || !g.current) {
        llg_msg(LLG_MSG_RUNTIME_WARNING, NULL, "deferred $stop outside a process turn ignored");
        return;
    }
    g.deferred_stop = 1;
}

void llg_rt_action_stop(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "invalid $stop verbosity %d", verbosity);
        abort();
    }
    report_stop(verbosity, location);
    if (!g.running || g.finish || g.suspended || g.stop_proc) return;
    g.stop_proc = NULL;
    g.stop_region = g.current_region;
    g.suspended = 1;
}

int llg_rt_control_stop(int verbosity) {
    if (!g.running || g.finish || g.suspended || g.stop_proc || g.deferred_stop) return 0;
    if (g.process_turn_active && g.current) {
        llg_rt_request_stop(verbosity, NULL);
        return 1;
    }
    llg_rt_action_stop(verbosity, NULL);
    return 1;
}

int llg_rt_control_stop_before_run(int verbosity) {
    if (!g.initialized || g.running || g.finish || g.suspended || g.stop_proc ||
        g.deferred_stop)
        return 0;
    report_stop(verbosity, NULL);
    // The noninteractive RESUME policy resumes immediately; EXIT returns
    // control to the embedding before time 0, as a stop in a callback does.
    if (g.stop_policy == LLG_STOP_POLICY_EXIT) {
        g.stop_proc = NULL;
        g.stop_region = LLG_REGION_PREPONED;
        g.suspended = 1;
    }
    return 1;
}

int llg_rt_control_finish(int verbosity) {
    if (g.finish) return 0;
    report_finish(verbosity, NULL);
    llg_rt_request_finish();
    return 1;
}

uint64_t llg_rt_end_time(void) { return g.initialized ? g.now : llg_final_time; }

int llg_rt_set_stop_policy(int policy) {
    if (policy != LLG_STOP_POLICY_RESUME && policy != LLG_STOP_POLICY_EXIT) return 0;
    if (g.running) return 0;
    llg_stop_policy_override = 1;
    llg_configured_stop_policy = policy;
    g.stop_policy = policy;
    return 1;
}

int llg_rt_stop_policy(void) {
    return g.initialized ? g.stop_policy : llg_configured_stop_policy;
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
    return llg_schedule_region_callback_id(region, callback, data, ticks, NULL);
}

int llg_schedule_region_callback_id(llg_region_t region,
                                    llg_region_callback_fn callback,
                                    void* data, uint64_t ticks,
                                    llg_region_callback_id_t* id) {
    if (!callback || !callback_region_allowed(region, ticks)) return 0;
    if (ticks > UINT64_MAX - g.now || g.callback_sequence == UINT64_MAX) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "region callback time or sequence overflow");
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
    if (id) {
        id->generation = llg_event_generation;
        id->sequence = entry->sequence;
    }
    insert_region_callback(entry);
    return 1;
}

int llg_cancel_region_callback(llg_region_callback_id_t id) {
    if (id.generation != llg_event_generation) return 0;
    for (llg_region_callback_t** slot = &g.callbacks; *slot;
         slot = &(*slot)->next) {
        if ((*slot)->sequence != id.sequence) continue;
        llg_region_callback_t* entry = *slot;
        *slot = entry->next;
        free(entry);
        return 1;
    }
    return 0;
}

int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback_after(region, callback, data, 0);
}

int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback(region, callback, data);
}
