
static int llg_fmt_arg_same(const llg_fmt_arg_t* a, const llg_fmt_arg_t* b) {
    if (a->kind != b->kind) return 0;
    if (a->kind == LLG_FMT_PACKED || a->kind == LLG_FMT_STRENGTH)
        return sv4_same(a->value.packed, b->value.packed);
    if (a->kind == LLG_FMT_REAL) return real_same(a->value.real, b->value.real);
    return a->value.string.len == b->value.string.len &&
           (!a->value.string.len ||
            memcmp(a->value.string.data, b->value.string.data,
                   a->value.string.len) == 0);
}

static void monitor_destroy(llg_monitor_state_t* m) {
    free(m->fmt);
    if (m->last) sv4_destroy_array(m->last, (size_t)m->n);
    if (m->work) sv4_destroy_array(m->work, (size_t)m->n);
    free(m->last);
    free(m->work);
    free(m->reads);
    llg_fmt_args_destroy(m->typed_last, m->n);
    llg_fmt_args_destroy(m->typed_work, m->n);
    free(m->typed_last);
    free(m->typed_work);
    free(m->typed_reads);
    free(m->scope);
    free(m);
}

static void monitor_unlink(llg_monitor_state_t* target) {
    llg_monitor_state_t* previous = NULL;
    for (llg_monitor_state_t* m = g.monitors; m; previous = m, m = m->next) {
        if (m != target) continue;
        if (previous) previous->next = m->next;
        else g.monitors = m->next;
        if (g.monitors_tail == m) g.monitors_tail = previous;
        return;
    }
}

// Remove every entry whose channels are all closed.
static void monitors_sweep(void) {
    llg_monitor_state_t* previous = NULL;
    llg_monitor_state_t* m = g.monitors;
    while (m) {
        llg_monitor_state_t* next = m->next;
        if (m->dead) {
            if (previous) previous->next = next;
            else g.monitors = next;
            if (g.monitors_tail == m) g.monitors_tail = previous;
            monitor_destroy(m);
        } else {
            previous = m;
        }
        m = next;
    }
}

static void llg_monitors_free_all(void) {
    while (g.monitors) {
        llg_monitor_state_t* next = g.monitors->next;
        monitor_destroy(g.monitors);
        g.monitors = next;
    }
    g.monitors_tail = NULL;
    g.monitor_off = 0;
    g.monitors_sweeping = 0;
}

// SV 21.3.1: "Active $fmonitor and/or $fstrobe operations on a file
// descriptor or multichannel descriptor are implicitly cancelled by an
// $fclose operation." Entries stay in the list until it is safe to free them.
static void llg_monitors_cancel_slot(unsigned slot) {
    int any_dead = 0;
    for (llg_monitor_state_t* m = g.monitors; m; m = m->next) {
        if (m->primary || !m->descriptor) continue;
        m->descriptor = llg_file_without_slot(m->descriptor, slot);
        if (!m->descriptor) {
            m->dead = 1;
            any_dead = 1;
        }
    }
    if (any_dead && !g.monitors_sweeping) monitors_sweep();
}

// Called for every published write while a monitor is registered. Entries
// already dirty are skipped, so a hot signal costs one pass per slot.
static void llg_monitor_target_changed(const void* target, int kind) {
    for (llg_monitor_state_t* m = g.monitors; m; m = m->next) {
        if (m->dirty) continue;
        if (kind == LLG_FMT_PACKED) {
            for (int i = 0; i < m->n_reads; i++) {
                if (m->reads[i] == target) {
                    m->dirty = 1;
                    break;
                }
            }
        }
        if (m->dirty) continue;
        for (int i = 0; i < m->n_typed_reads; i++) {
            if (m->typed_reads[i].kind == kind &&
                m->typed_reads[i].ptr == target) {
                m->dirty = 1;
                break;
            }
        }
    }
}

// Append a fresh entry. `$monitor` (primary) first discards its predecessor:
// "Only one $monitor display list can be active at any one time" (SV 21.2.3).
static llg_monitor_state_t* monitor_add(int primary, const char* fmt, int n) {
    if (primary) {
        for (llg_monitor_state_t* m = g.monitors; m; m = m->next) {
            if (!m->primary) continue;
            monitor_unlink(m);
            monitor_destroy(m);
            break;
        }
    }
    llg_monitor_state_t* m = (llg_monitor_state_t*)llg_checked_calloc(
        1, sizeof(llg_monitor_state_t), "monitor");
    m->primary = primary;
    m->dirty = 1;
    m->force_report = 1;
    m->n = n;
    m->region = LLG_REGION_POSTPONED;
    m->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(m->fmt, fmt);
    if (g.monitors_tail) g.monitors_tail->next = m;
    else g.monitors = m;
    g.monitors_tail = m;
    return m;
}

void llg_monitor_with_reads(const char* fmt, int n, llg_mon_eval_fn eval,
                            sv4_t* const* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    llg_monitor_state_t* m = monitor_add(1, fmt, n);
    m->eval = eval;
    m->n_reads = n_reads;
    int alloc = n > 0 ? n : 1;
    m->last = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor previous values");
    m->work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor working values");
    if (n_reads > 0) {
        m->reads = (sv4_t**)llg_checked_malloc(
            (size_t)n_reads, sizeof(sv4_t*), "monitor trigger set");
        memcpy(m->reads, reads, (size_t)n_reads * sizeof(sv4_t*));
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

static void monitor_register_typed(int primary, uint32_t descriptor,
                                   const char* fmt, int n,
                                   llg_display_eval_fn eval, const char* scope,
                                   const llg_display_read_t* reads,
                                   int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    // An invalid or closed descriptor registers nothing; the failure is
    // reported through $ferror like any other write to it.
    if (!primary && !llg_file_mask_valid(descriptor)) return;
    llg_monitor_state_t* m = monitor_add(primary, fmt, n);
    m->typed = 1;
    m->descriptor = descriptor;
    m->typed_eval = eval;
    m->scope = (char*)llg_checked_malloc(strlen(scope ? scope : "") + 1, 1,
                                         "monitor scope");
    strcpy(m->scope, scope ? scope : "");
    int alloc = n > 0 ? n : 1;
    m->typed_last = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor previous values");
    m->typed_work = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor working values");
    if (n_reads > 0) {
        m->typed_reads = (llg_display_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(llg_display_read_t), "monitor trigger set");
        for (int i = 0; i < n_reads; i++) {
            if (reads[i].kind == LLG_MONITOR_READ_POLL) m->poll = 1;
            else m->typed_reads[m->n_typed_reads++] = reads[i];
        }
    }
}

void llg_monitor_with_typed_reads(const char* fmt, int n,
                                  llg_display_eval_fn eval, const char* scope,
                                  const llg_display_read_t* reads, int n_reads) {
    monitor_register_typed(1, 1u, fmt, n, eval, scope, reads, n_reads);
}

void llg_file_monitor_with_typed_reads(
    uint32_t descriptor, const char* fmt, int n, llg_display_eval_fn eval,
    const char* scope, const llg_display_read_t* reads, int n_reads) {
    monitor_register_typed(0, descriptor, fmt, n, eval, scope, reads, n_reads);
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

// Re-print one dirty monitor at the settled observation point. The
// registration and enable paths set force_report so equal values still print.
static void check_monitor(llg_monitor_state_t* m) {
    if (!m->dirty && !m->force_report && !m->poll) return;
    if (m->primary && g.monitor_off) return;
    if (m->typed) {
        llg_fmt_args_destroy(m->typed_work, m->n);
        m->typed_eval(m->typed_work, NULL);
        int changed = m->force_report;
        if (!changed) {
            for (int i = 0; i < m->n; i++) {
                if (!llg_fmt_arg_same(&m->typed_work[i], &m->typed_last[i])) {
                    changed = 1;
                    break;
                }
            }
        }
        m->dirty = 0;
        m->force_report = 0;
        if (changed) {
            llg_fmt_args_destroy(m->typed_last, m->n);
            for (int i = 0; i < m->n; i++)
                m->typed_last[i] = llg_fmt_arg_clone(&m->typed_work[i]);
            llg_print_typed_to(m->descriptor, m->fmt, m->typed_work, m->n,
                               m->scope, 1);
        }
        llg_fmt_args_destroy(m->typed_work, m->n);
        return;
    }
    sv4_destroy_array(m->work, (size_t)m->n);
    m->eval(m->work, NULL);
    int changed = m->force_report;
    if (!changed) {
        for (int i = 0; i < m->n; i++) {
            if (!sv4_same(m->work[i], m->last[i])) {
                changed = 1;
                break;
            }
        }
    }
    m->dirty = 0;
    m->force_report = 0;
    if (!changed) {
        sv4_destroy_array(m->work, (size_t)m->n);
        return;
    }
    for (int i = 0; i < m->n; i++) sv4_copy(&m->last[i], &m->work[i]);
    llg_print_array(m->fmt, m->work, m->n);
    sv4_destroy_array(m->work, (size_t)m->n);
}

// Registration order is the report order within one time slot.
static void check_monitors(void) {
    if (!g.monitors) return;
    g.monitors_sweeping = 1;
    for (llg_monitor_state_t* m = g.monitors; m && !g.finish; m = m->next) {
        if (!m->dead) check_monitor(m);
    }
    g.monitors_sweeping = 0;
    monitors_sweep();
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

// `$monitoron`/`$monitoroff` set one flag that only `$monitor` observes; "there
// is no counterpart to $monitoron and $monitoroff tasks" for $fmonitor (SV
// 21.3.2). The flag survives a later `$monitor` call.
void llg_monitor_set(int on) {
    if (!region_can_mutate("monitor scheduling")) return;
    g.monitor_off = !on;
    if (!on) return;
    for (llg_monitor_state_t* m = g.monitors; m; m = m->next) {
        if (!m->primary) continue;
        m->dirty = 1;
        m->force_report = 1;
        break;
    }
}
