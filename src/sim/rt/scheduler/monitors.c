
static int llg_fmt_arg_same(const llg_fmt_arg_t* a, const llg_fmt_arg_t* b) {
    if (a->kind != b->kind) return 0;
    if (a->kind == LLG_FMT_PACKED) return sv4_same(a->value.packed, b->value.packed);
    if (a->kind == LLG_FMT_REAL) return real_same(a->value.real, b->value.real);
    return a->value.string.len == b->value.string.len &&
           (!a->value.string.len ||
            memcmp(a->value.string.data, b->value.string.data,
                   a->value.string.len) == 0);
}

static void reset_monitor_state(void) {
    if (g.mon.active) {
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
    }
    memset(&g.mon, 0, sizeof(g.mon));
}

void llg_monitor_with_reads(const char* fmt, int n, llg_mon_eval_fn eval,
                            sv4_t* const* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    reset_monitor_state();
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.dirty = 1;
    g.mon.force_report = 1;
    g.mon.n = n;
    g.mon.eval = eval;
    g.mon.n_reads = n_reads;
    g.mon.region = LLG_REGION_POSTPONED;
    g.mon.fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(g.mon.fmt, fmt);
    int alloc = n > 0 ? n : 1;
    g.mon.last = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor previous values");
    g.mon.work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor working values");
    if (n_reads > 0) {
        g.mon.reads = (sv4_t**)llg_checked_malloc(
            (size_t)n_reads, sizeof(sv4_t*), "monitor trigger set");
        memcpy(g.mon.reads, reads, (size_t)n_reads * sizeof(sv4_t*));
    }
}

void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval) {
    llg_monitor_with_reads(fmt, n, eval, NULL, 0);
}

void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval) {
    if (!region_can_mutate("strobe scheduling")) return;
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = eval;
    e->typed = 0;
    e->typed_eval = NULL;
    e->typed_work = NULL;
    e->scope = NULL;
    e->region = LLG_REGION_POSTPONED;
    int alloc = n > 0 ? n : 1;
    e->work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "strobe working values");
    e->next = NULL;
    if (g.strobe_tail) {
        g.strobe_tail->next = e;
    } else {
        g.strobes = e;
    }
    g.strobe_tail = e;
}

void llg_monitor_with_typed_reads(const char* fmt, int n,
                                  llg_display_eval_fn eval, const char* scope,
                                  const llg_display_read_t* reads, int n_reads) {
    llg_file_monitor_with_typed_reads(1u, fmt, n, eval, scope, reads, n_reads);
}

void llg_file_monitor_with_typed_reads(
    uint32_t descriptor, const char* fmt, int n, llg_display_eval_fn eval,
    const char* scope, const llg_display_read_t* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    reset_monitor_state();
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.dirty = 1;
    g.mon.force_report = 1;
    g.mon.n = n;
    g.mon.typed = 1;
    g.mon.descriptor = descriptor;
    g.mon.typed_eval = eval;
    g.mon.n_typed_reads = n_reads;
    g.mon.region = LLG_REGION_POSTPONED;
    g.mon.fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(g.mon.fmt, fmt);
    g.mon.scope = (char*)llg_checked_malloc(strlen(scope ? scope : "") + 1, 1,
                                            "monitor scope");
    strcpy(g.mon.scope, scope ? scope : "");
    int alloc = n > 0 ? n : 1;
    g.mon.typed_last = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor previous values");
    g.mon.typed_work = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor working values");
    if (n_reads > 0) {
        g.mon.typed_reads = (llg_display_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(llg_display_read_t), "monitor trigger set");
        memcpy(g.mon.typed_reads, reads,
               (size_t)n_reads * sizeof(llg_display_read_t));
    }
}

void llg_strobe_typed(const char* fmt, int n, llg_display_eval_fn eval,
                      const char* scope) {
    llg_file_strobe_typed(1u, fmt, n, eval, scope);
}

void llg_file_strobe_typed(uint32_t descriptor, const char* fmt, int n,
                           llg_display_eval_fn eval, const char* scope) {
    if (!region_can_mutate("strobe scheduling")) return;
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "typed strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = NULL;
    e->typed = 1;
    e->descriptor = descriptor;
    e->typed_eval = eval;
    e->scope = (char*)llg_checked_malloc(strlen(scope ? scope : "") + 1, 1,
                                         "strobe scope");
    strcpy(e->scope, scope ? scope : "");
    int alloc = n > 0 ? n : 1;
    e->work = NULL;
    e->typed_work = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "strobe working values");
    e->region = LLG_REGION_POSTPONED;
    e->next = NULL;
    if (g.strobe_tail) {
        g.strobe_tail->next = e;
    } else {
        g.strobes = e;
    }
    g.strobe_tail = e;
}

// Re-print a dirty monitor line at the settled observation point. The
// registration and enable paths set force_report so equal values still print.
static void check_monitor(void) {
    if (!g.mon.active || !g.mon.enabled) return;
    if (!g.mon.dirty && !g.mon.force_report) return;
    if (g.mon.typed) {
        if (!g.mon.descriptor) return;
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        g.mon.typed_eval(g.mon.typed_work, NULL);
        int changed = g.mon.force_report;
        if (!changed) {
            for (int i = 0; i < g.mon.n; i++) {
                if (!llg_fmt_arg_same(&g.mon.typed_work[i], &g.mon.typed_last[i])) {
                    changed = 1;
                    break;
                }
            }
        }
        g.mon.dirty = 0;
        g.mon.force_report = 0;
        if (changed) {
            llg_fmt_args_destroy(g.mon.typed_last, g.mon.n);
            for (int i = 0; i < g.mon.n; i++)
                g.mon.typed_last[i] = llg_fmt_arg_clone(&g.mon.typed_work[i]);
            llg_print_typed_to(g.mon.descriptor, g.mon.fmt, g.mon.typed_work,
                               g.mon.n, g.mon.scope, 1);
        }
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        return;
    }
    sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
    g.mon.eval(g.mon.work, NULL);
    int changed = g.mon.force_report;
    if (!changed) {
        for (int i = 0; i < g.mon.n; i++) {
            if (!sv4_same(g.mon.work[i], g.mon.last[i])) {
                changed = 1;
                break;
            }
        }
    }
    g.mon.dirty = 0;
    g.mon.force_report = 0;
    if (!changed) {
        sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
        return;
    }
    for (int i = 0; i < g.mon.n; i++) sv4_copy(&g.mon.last[i], &g.mon.work[i]);
    llg_print_array(g.mon.fmt, g.mon.work, g.mon.n);
    sv4_destroy_array(g.mon.work, (size_t)g.mon.n);

}

// Print queued $strobe lines after the current time step has settled.
static void flush_strobes(void) {
    while (g.strobes && !g.finish) {
        llg_strobe_t* e = g.strobes;
        g.strobes = e->next;
        if (!g.strobes) g.strobe_tail = NULL;
        if (e->typed) {
            if (e->descriptor) {
                e->typed_eval(e->typed_work, NULL);
                llg_print_typed_to(e->descriptor, e->fmt, e->typed_work, e->n,
                               e->scope, 1);
            }
            llg_fmt_args_destroy(e->typed_work, e->n);
            free(e->typed_work);
            free(e->scope);
        } else {
            e->eval(e->work, NULL);
            llg_print_array(e->fmt, e->work, e->n);
            sv4_destroy_array(e->work, (size_t)e->n);
            free(e->work);
        }
        free(e->fmt);
        free(e);
    }
}

void llg_monitor_set(int on) {
    if (!region_can_mutate("monitor scheduling")) return;
    if (!g.mon.active) return;
    if (on) {
        g.mon.enabled = 1;
        g.mon.dirty = 1;
        g.mon.force_report = 1;
    } else {
        g.mon.enabled = 0;
    }
}
