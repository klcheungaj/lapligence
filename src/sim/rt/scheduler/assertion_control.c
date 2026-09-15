
static const char* llg_severity_name(int severity) {
    switch (severity) {
        case LLG_SEVERITY_INFO: return "info";
        case LLG_SEVERITY_WARNING: return "warning";
        case LLG_SEVERITY_ERROR: return "error";
        case LLG_SEVERITY_FATAL: return "fatal";
        default: return "invalid";
    }
}

static void llg_report_severity_typed(int severity, const char* fmt,
                                      llg_fmt_arg_t* args, int n,
                                      const char* scope, const char* location) {
    if (severity < LLG_SEVERITY_INFO || severity > LLG_SEVERITY_FATAL) {
        fprintf(stderr, "llg runtime fatal: invalid severity level %d\n", severity);
        abort();
    }
    if (n < 0) {
        fprintf(stderr, "llg runtime fatal: negative severity argument count\n");
        abort();
    }
    if (llg_severity_counts[severity] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: severity counter overflow\n");
        abort();
    }
    size_t len = 0;
    char* out = llg_typed_line_alloc(fmt, args, n, scope, &len);
    llg_severity_counts[severity]++;
    fprintf(stderr, "llg: severity %s: %s: ",
            llg_severity_name(severity),
            location && location[0] ? location : "<unknown>");
    fwrite(out, 1, len, stderr);
    fputc('\n', stderr);
    fflush(stderr);
    free(out);
}

void llg_rt_severity_typed(int severity, const char* fmt, llg_fmt_arg_t* args,
                           int n, const char* scope, const char* location) {
    llg_report_severity_typed(severity, fmt, args, n, scope, location);
    llg_fmt_args_destroy(args, n);
}

_Noreturn void llg_rt_fatal_typed(int finish_number, const char* fmt,
                                  llg_fmt_arg_t* args, int n,
                                  const char* scope, const char* location) {
    if (finish_number < 0 || finish_number > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $fatal finish number %d\n",
                finish_number);
        llg_fmt_args_destroy(args, n);
        abort();
    }
    llg_report_severity_typed(LLG_SEVERITY_FATAL, fmt, args, n, scope, location);
    llg_fmt_args_destroy(args, n);
    llg_rt_finish_with_level(finish_number, location);
}

uint64_t llg_rt_severity_count(int severity) {
    if (severity < LLG_SEVERITY_INFO || severity > LLG_SEVERITY_FATAL) return 0;
    return llg_severity_counts[severity];
}

static const char* llg_assertion_name(int kind) {
    switch (kind) {
        case LLG_ASSERTION_ASSERT: return "assert";
        case LLG_ASSERTION_ASSUME: return "assume";
        case LLG_ASSERTION_EXPECT: return "expect";
        default: return "invalid";
    }
}

static void assertion_record_failure(int kind) {
    if (kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        llg_assertion_failure_counts[kind] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: invalid assertion kind or counter overflow\n");
        abort();
    }
    llg_assertion_failure_counts[kind]++;
}

static void assertion_report_failure(int kind, const char* label,
                                      const char* location) {
    if (llg_severity_counts[LLG_SEVERITY_ERROR] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: severity counter overflow\n");
        abort();
    }
    llg_severity_counts[LLG_SEVERITY_ERROR]++;
    fprintf(stderr, "llg: assertion %s failed: %s",
            llg_assertion_name(kind),
            location && location[0] ? location : "<unknown>");
    if (label && label[0]) fprintf(stderr, " (%s)", label);
    fputc('\n', stderr);
    fflush(stderr);
}

void llg_assertion_failure(int kind, uint64_t identity, const char* label,
                           const char* location) {
    (void)identity;
    assertion_record_failure(kind);
    assertion_report_failure(kind, label, location);
}

void llg_assertion_cover(uint64_t identity, const char* label, const char* location) {
    (void)identity;
    (void)label;
    (void)location;
    if (llg_assertion_cover_count == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: assertion coverage counter overflow\n");
        abort();
    }
    llg_assertion_cover_count++;
}

uint64_t llg_assertion_count(int kind) {
    if (kind == LLG_ASSERTION_COVER) return llg_assertion_cover_count;
    if (kind >= LLG_ASSERTION_ASSERT && kind <= LLG_ASSERTION_EXPECT)
        return llg_assertion_failure_counts[kind];
    return 0;
}

uint64_t llg_assertion_vacuous_count(void) {
    return llg_assertion_vacuous_total;
}

static llg_concurrent_assertion_t* find_assertion(uint64_t identity) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (assertion->identity == identity) return assertion;
    }
    return NULL;
}

static int path_matches_selector(const char* path, const char* selector) {
    if (!path || !selector || !path[0] || !selector[0]) return 0;
    if (strcmp(path, selector) == 0) return 1;
    size_t path_len = strlen(path);
    size_t selector_len = strlen(selector);
    if (path_len > selector_len &&
        path[path_len - selector_len - 1] == '.' &&
        strcmp(path + path_len - selector_len, selector) == 0)
        return 1;
    return selector_len < path_len &&
           strncmp(path, selector, selector_len) == 0 &&
           path[selector_len] == '.';
}

static int assertion_matches_scope(const llg_concurrent_assertion_t* assertion,
                                   const char* selector) {
    if (!selector || !selector[0]) return 0;
    if (path_matches_selector(assertion->scope, selector)) return 1;
    const char* scope = assertion->scope ? assertion->scope : "";
    const char* label = assertion->label ? assertion->label : "";
    if (!label[0]) return 0;
    size_t scope_len = strlen(scope);
    size_t label_len = strlen(label);
    size_t full_len = scope_len + (scope_len != 0) + label_len;
    char* full = (char*)llg_checked_malloc(full_len + 1, 1,
                                           "assertion hierarchy selector");
    if (scope_len != 0) {
        memcpy(full, scope, scope_len);
        full[scope_len] = '.';
    }
    memcpy(full + scope_len + (scope_len != 0), label, label_len);
    full[full_len] = '\0';
    int matched = path_matches_selector(full, selector);
    free(full);
    return matched;
}

static int assertion_matches_type(const llg_concurrent_assertion_t* assertion,
                                  uint64_t assertion_type,
                                  uint64_t directive_type) {
    // Table 20-6: concurrent assertions use bit 1 and expect uses bit 16.
    // Immediate/unique report classes are intentionally ignored because this
    // runtime registry owns only sampled concurrent instances.
    uint64_t type_bit = assertion->kind == LLG_ASSERTION_EXPECT ? 16u : 1u;
    if ((assertion_type & type_bit) == 0) return 0;
    uint64_t directive_bit = assertion->kind == LLG_ASSERTION_COVER
                                 ? 2u
                                 : assertion->kind == LLG_ASSERTION_ASSUME ? 4u : 1u;
    return (directive_type & directive_bit) != 0;
}

static int assertion_control_failure(const char* reason) {
    fprintf(stderr, "llg: assertion control error: %s\n", reason);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

static int assertion_control_arg(const sv4_t* value, uint64_t* result) {
    if (!value || value->width == 0 || value->width > 64 ||
        sv4_is_unknown(*value))
        return 0;
    *result = sv4_to_u64(*value);
    return 1;
}

static int assertion_has_attempts(const llg_concurrent_assertion_t* assertion) {
    return assertion->attempts || assertion->sequence_antecedents ||
           assertion->sequence_consequents;
}

static int deferred_selected(int kind, const char* label, const char* scope,
                              uint64_t types, uint64_t directives,
                              const char* const* scopes, int count) {
    uint64_t directive = kind == LLG_ASSERTION_COVER ? 2u :
                         kind == LLG_ASSERTION_ASSUME ? 4u : 1u;
    if (!(types & 4u) || !(directives & directive)) return 0; // #0 deferred
    if (!count) return 1;
    llg_concurrent_assertion_t view = {0};
    view.scope = scope;
    view.label = label;
    for (int i = 0; i < count; ++i)
        if (assertion_matches_scope(&view, scopes[i])) return 1;
    return 0;
}

int llg_deferred_assertion_enabled(int kind, const char* label, const char* scope) {
    for (llg_assertion_rule_t* rule = llg_assertion_rules; rule; rule = rule->next) {
        const char* selectors[] = {rule->scope};
        if (deferred_selected(kind, label, scope, rule->assertion_type,
                              rule->directive_type, selectors, rule->scope ? 1 : 0))
            return rule->enabled;
    }
    return 1;
}

static void assertion_remember_control(int enabled, uint64_t types,
                                       uint64_t directives, const char* scope) {
    // Replace equal selector/mask rules; repeated control in a loop must not
    // retain an unbounded history of identical commands.
    llg_assertion_rule_t** link = &llg_assertion_rules;
    llg_assertion_rule_t* rule = NULL;
    while (*link) {
        if ((*link)->assertion_type == types && (*link)->directive_type == directives &&
            ((!scope && !(*link)->scope) ||
             (scope && (*link)->scope && strcmp(scope, (*link)->scope) == 0))) {
            rule = *link;
            *link = rule->next;
            break;
        }
        link = &(*link)->next;
    }
    if (!rule) {
        rule = llg_checked_calloc(1, sizeof(*rule), "assertion control rule");
        if (scope) {
            size_t length = strlen(scope);
            rule->scope = llg_checked_malloc(length + 1u, 1, "assertion control scope");
            memcpy(rule->scope, scope, length + 1u);
        }
        rule->assertion_type = types;
        rule->directive_type = directives;
    }
    rule->enabled = enabled;
    rule->next = llg_assertion_rules;
    llg_assertion_rules = rule;
}

static void assertion_default_failure_action(void* data);

static void assertion_kill_actions(llg_concurrent_assertion_t* assertion) {
    // Cancellation changes the process table; restart each scan.
    for (;;) {
        llg_proc_t* victim = NULL;
        for (int i = 0; i < g.n_procs; ++i) {
            llg_proc_t* p = g.all_procs[i];
            if (p && p->is_assertion_action &&
                p->action_assertion == assertion->identity && !p->killed && !p->completed) {
                victim = p;
                break;
            }
        }
        if (!victim) break;
        llg_kill_proc_tree(victim);
    }
    llg_region_callback_t** link = &g.callbacks;
    while (*link) {
        llg_region_callback_t* entry = *link;
        if (entry->callback == assertion_default_failure_action && entry->data == assertion) {
            *link = entry->next;
            free(entry);
        } else link = &entry->next;
    }
}

static void assertion_kill_deferred(uint64_t types, uint64_t directives,
                                    const char* const* scopes, int count) {
    llg_deferred_assertion_report_t** link = &g.deferred_assertions;
    g.deferred_assertion_tail = NULL;
    while (*link) {
        llg_deferred_assertion_report_t* report = *link;
        if (deferred_selected(report->kind, report->label, report->scope,
                              types, directives, scopes, count)) {
            *link = report->next;
            free_deferred_assertion_report(report);
        } else {
            g.deferred_assertion_tail = report;
            link = &report->next;
        }
    }
    llg_region_callback_t** callback = &g.callbacks;
    while (*callback) {
        llg_region_callback_t* entry = *callback;
        llg_deferred_assertion_report_t* report = entry->data;
        if (entry->callback == deferred_assertion_callback && report &&
            deferred_selected(report->kind, report->label, report->scope,
                              types, directives, scopes, count)) {
            *callback = entry->next;
            free_deferred_assertion_report(report);
            free(entry);
        } else callback = &entry->next;
    }
}

int llg_assertion_control(int kind, const sv4_t* args, int n_args,
                          const char* const* scopes, int n_scopes) {
    if (!region_can_mutate("assertion control")) return 0;
    if (kind < LLG_ASSERTION_CONTROL_ON || kind > LLG_ASSERTION_CONTROL_FULL ||
        n_args < 0 || n_args > 4 || n_scopes < 0 ||
        (n_args != 0 && !args) || (n_scopes != 0 && !scopes))
        return assertion_control_failure("invalid control argument shape");

    for (int i = 0; i < n_scopes; ++i)
        if (!scopes[i] || !scopes[i][0])
            return assertion_control_failure("empty assertion scope selector");

    uint64_t values[4] = {0, 0, 0, 0};
    for (int index = 0; index < n_args; index++) {
        if (!assertion_control_arg(&args[index], &values[index]))
            return assertion_control_failure("control arguments must be known 64-bit integers");
    }

    int operation = kind;
    uint64_t assertion_type = UINT64_C(255);
    uint64_t directive_type = UINT64_C(7);
    if (kind == LLG_ASSERTION_CONTROL_FULL) {
        if (n_args < 1) return assertion_control_failure("$assertcontrol requires control_type");
        uint64_t control_type = values[0];
        if (control_type < 3 || control_type > 5)
            return assertion_control_failure("bounded $assertcontrol supports only ON, OFF, and KILL");
        operation = control_type - 3;
        if (n_args > 1) assertion_type = values[1];
        if (n_args > 2) directive_type = values[2];
        // The optional fourth argument is `levels`. This bounded registry
        // implements the standard level-0 (all descendants) selector; other
        // hierarchy-depth policies remain fail-closed until represented in
        // the owned assertion catalog.
        if (n_args > 3 && values[3] != 0)
            return assertion_control_failure("bounded assertion control supports only level 0");
        if (assertion_type > 255 || directive_type > 7)
            return assertion_control_failure("unsupported assertion or directive type");
    } else if (n_args > 1) {
        return assertion_control_failure("assertion control task takes one level argument");
    } else if (n_args == 1 && values[0] != 0) {
        return assertion_control_failure("bounded assertion control supports only level 0");
    }

    if (n_scopes == 0)
        assertion_remember_control(operation == LLG_ASSERTION_CONTROL_ON,
                                   assertion_type, directive_type, NULL);
    for (int i = 0; i < n_scopes; ++i)
        assertion_remember_control(operation == LLG_ASSERTION_CONTROL_ON,
                                   assertion_type, directive_type, scopes[i]);
    if (operation == LLG_ASSERTION_CONTROL_KILL)
        assertion_kill_deferred(assertion_type, directive_type, scopes, n_scopes);

    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (kind == LLG_ASSERTION_CONTROL_FULL &&
            !assertion_matches_type(assertion, assertion_type, directive_type))
            continue;
        int selected = n_scopes == 0;
        for (int index = 0; !selected && index < n_scopes; index++)
            selected = assertion_matches_scope(assertion, scopes[index]);
        if (!selected) continue;
        switch (operation) {
            case LLG_ASSERTION_CONTROL_ON:
                assertion->enabled = 1;
                break;
            case LLG_ASSERTION_CONTROL_OFF:
                assertion->enabled = 0;
                break;
            case LLG_ASSERTION_CONTROL_KILL:
                assertion->enabled = 0;
                assertion_kill_actions(assertion);
                free_assertion_attempts(assertion);
                assertion->edge_pending = 0;
                if (assertion->kind == LLG_ASSERTION_EXPECT &&
                    assertion->expect_active) {
                    assertion->expect_active = 0;
                    wake_assertion_waiter(assertion->identity);
                }
                break;
            default:
                return assertion_control_failure("invalid assertion control operation");
        }
    }
    if (operation == LLG_ASSERTION_CONTROL_KILL) {
        llg_proc_t* current = llg_current();
        service_program_completions();
        semaphore_service_cancelled_waiters();
        reap_retired_procs();
        if (current && current->killed) { aco_exit(); abort(); }
        if (current && g.finish) llg_proc_done(current);
    }
    return 1;
}

int llg_assertion_expect_start(uint64_t identity) {
    if (!region_can_mutate("expect scheduling")) return 0;
    llg_concurrent_assertion_t* assertion = find_assertion(identity);
    llg_proc_t* current = llg_current();
    if (!assertion || assertion->kind != LLG_ASSERTION_EXPECT || !current ||
        assertion->expect_active)
        return assertion_control_failure("expect has no unique inactive assertion instance");
    free_assertion_attempts(assertion);
    assertion->expect_active = 1;
    assertion->edge_pending = 0;
    assertion->sequence_cycle = 0;
    return 1;
}

static void assertion_attempt_enqueue(llg_concurrent_assertion_t* assertion) {
    llg_assertion_attempt_t* attempt = (llg_assertion_attempt_t*)llg_checked_calloc(
        1, sizeof(*attempt), "concurrent assertion attempt");
    attempt->due = 1;
    if (assertion->attempts_tail) {
        assertion->attempts_tail->next = attempt;
    } else {
        assertion->attempts = attempt;
    }
    assertion->attempts_tail = attempt;
}
