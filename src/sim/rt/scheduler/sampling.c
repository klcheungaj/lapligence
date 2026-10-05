
static llg_sampled_value_t* find_sampled_value(const sv4_t* signal) {
    if (!signal) return NULL;
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) return item;
    }
    return NULL;
}

static void report_unregistered_sampled_signal(void) {
    fprintf(stderr, "llg: sampled value requested for an unregistered signal\n");
    llg_last_failure = 1;
    g.finish = 1;
}

static void sampled_record_write(sv4_t* signal) {
    llg_sampled_value_t* item = find_sampled_value(signal);
    if (!item) return;
    llg_sampled_history_t* last = item->history;
    if (last && last->time == g.now) {
        sv4_copy(&last->value, signal);
        return;
    }
    llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
        1, sizeof(*history), "sampled history");
    history->time = g.now;
    history->value = sv4_clone(signal);
    history->next = item->history;
    item->history = history;
}

void llg_sampled_register(sv4_t* signal) {
    if (!signal) {
        fprintf(stderr, "llg: cannot register a null sampled signal\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) return;
    }
    llg_sampled_value_t* item = (llg_sampled_value_t*)llg_checked_malloc(
        1, sizeof(*item), "sampled value");
    item->signal = signal;
    item->value = sv4_clone(signal);
    item->history = NULL;
    llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
        1, sizeof(*history), "sampled history");
    history->time = g.now;
    history->value = sv4_clone(signal);
    history->next = NULL;
    item->history = history;
    item->next = g.sampled;
    g.sampled = item;
}

void llg_sampled_register_value(sv4_t* signal) {
    if (!signal) {
        fprintf(stderr, "llg: cannot register a null sampled signal\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (find_sampled_value(signal)) return;
    for (llg_sampled_value_t* item = g.sampled_values; item; item = item->next) {
        if (item->signal == signal) return;
    }
    llg_sampled_value_t* item = (llg_sampled_value_t*)llg_checked_malloc(
        1, sizeof(*item), "sampled value");
    item->signal = signal;
    item->value = sv4_clone(signal);
    item->history = NULL;
    item->next = g.sampled_values;
    g.sampled_values = item;
}

const sv4_t* llg_sampled_value(const sv4_t* signal) {
    llg_sampled_value_t* item = find_sampled_value(signal);
    if (item) return &item->value;
    for (item = g.sampled_values; item; item = item->next) {
        if (item->signal == signal) return &item->value;
    }
    report_unregistered_sampled_signal();
    return NULL;
}

int llg_sampled_copy(const sv4_t* signal, sv4_t* out) {
    if (!out) return 0;
    const sv4_t* value = llg_sampled_value(signal);
    if (!value) return 0;
    sv4_copy(out, value);
    return 1;
}

void llg_sampled_register_real(double* signal) {
    if (!signal) {
        fprintf(stderr, "llg: cannot register a null sampled signal\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    for (llg_sampled_real_t* item = g.sampled_reals; item; item = item->next) {
        if (item->signal == signal) return;
    }
    llg_sampled_real_t* item = (llg_sampled_real_t*)llg_checked_malloc(
        1, sizeof(*item), "sampled real value");
    item->signal = signal;
    item->value = *signal;
    item->next = g.sampled_reals;
    g.sampled_reals = item;
}

double llg_sampled_real(const double* signal) {
    for (llg_sampled_real_t* item = g.sampled_reals; item; item = item->next) {
        if (item->signal == signal) return item->value;
    }
    report_unregistered_sampled_signal();
    return 0.0;
}

static int sampled_real_image_equal(sv4_t left, sv4_t right) {
    if (llg_sv4_width(left) != 64 || llg_sv4_width(right) != 64) return 0;
    return sv4_bitstoreal(left) == sv4_bitstoreal(right);
}

static llg_sampled_domain_t* find_sampled_domain(uint64_t identity) {
    for (llg_sampled_domain_t* domain = g.sampled_domains; domain;
         domain = domain->next) {
        if (domain->identity == identity) return domain;
    }
    return NULL;
}

static void report_missing_sampled_domain(uint64_t identity) {
    fprintf(stderr, "llg: sampled-value domain %llu is not registered\n",
            (unsigned long long)identity);
    llg_last_failure = 1;
    g.finish = 1;
}

int llg_sampled_domain_register(uint64_t identity, sv4_t* clock, int edge,
                                llg_sampled_domain_eval_fn value,
                                llg_sampled_domain_eval_fn gate, void* data) {
    if (!clock || !value || (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE)) {
        fprintf(stderr, "llg: invalid sampled-value domain registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    if (find_sampled_domain(identity)) {
        fprintf(stderr, "llg: duplicate sampled-value domain %llu\n",
                (unsigned long long)identity);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_sampled_domain_t* domain = (llg_sampled_domain_t*)llg_checked_malloc(
        1, sizeof(*domain), "sampled-value domain");
    domain->identity = identity;
    domain->clock = clock;
    domain->edge = edge;
    domain->value = value;
    domain->gate = gate;
    domain->data = data;
    domain->initial = value(data);
    domain->history = NULL;
    domain->next = g.sampled_domains;
    g.sampled_domains = domain;
    return 1;
}

static void sampled_domain_clock_signal_changed(sv4_t* signal, sv4_t old,
                                                sv4_t value) {
    if (!signal) return;
    for (llg_sampled_domain_t* domain = g.sampled_domains; domain;
         domain = domain->next) {
        if (domain->clock != signal ||
            !ev_matches_changed(old, value, domain->edge))
            continue;
        if (domain->gate) {
            sv4_t gate = domain->gate(domain->data);
            int enabled = sv4_to_bool(gate);
            sv4_destroy(&gate);
            if (!enabled) continue;
        }
        llg_sampled_domain_history_t* history =
            (llg_sampled_domain_history_t*)llg_checked_malloc(
                1, sizeof(*history), "sampled-value domain history");
        history->time = g.now;
        history->sequence = g.sampled_domain_sequence++;
        history->value = domain->value(domain->data);
        history->next = domain->history;
        domain->history = history;
    }
}

sv4_t llg_sampled_domain_past(uint64_t identity, uint64_t ticks) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_missing_sampled_domain(identity);
        return sv4_x(1, 0);
    }
    if (ticks == 0) return sv4_clone(&domain->initial);
    // $past counts time steps strictly before the evaluating one in which
    // the clocking event occurred (IEEE 1800-2009 §16.9.3). A tick in the
    // current time step is not one of them, but the latest tick before it is,
    // as when procedural code evaluates $past between clock edges. Repeated
    // edges in one time step count once.
    llg_sampled_domain_history_t* history = domain->history;
    while (history && history->time == g.now) history = history->next;
    for (uint64_t index = 1; history && index < ticks; index++) {
        uint64_t time = history->time;
        while (history && history->time == time) history = history->next;
    }
    return sv4_clone(history ? &history->value : &domain->initial);
}

static int sampled_domain_lsb_one(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) != 0;
}

static int sampled_domain_lsb_zero(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) == 0;
}

int llg_sampled_domain_status(uint64_t identity, int kind) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_missing_sampled_domain(identity);
        return 0;
    }
    sv4_t current = domain->history ? domain->history->value : domain->initial;
    sv4_t previous = domain->history && domain->history->next
                         ? domain->history->next->value
                         : domain->initial;
    switch (kind) {
        case 0: return sampled_domain_lsb_one(current) && !sampled_domain_lsb_one(previous);
        case 1: return sampled_domain_lsb_zero(current) && !sampled_domain_lsb_zero(previous);
        case 2: return sv4_same(current, previous);
        case 3: return !sv4_same(current, previous);
        case 4: return sampled_real_image_equal(current, previous);
        case 5: return !sampled_real_image_equal(current, previous);
        default:
            fprintf(stderr, "llg: invalid sampled-value status kind %d\n", kind);
            llg_last_failure = 1;
            g.finish = 1;
            return 0;
    }
}

static void sample_preponed_values(void) {
    g.assertion_edges_pending = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
    }
    // The scheduler revisits PREPONED for zero-delay deltas in the same time
    // slot. #1step samples are fixed at the slot boundary and must not observe
    // values written by later active/NBA iterations.
    if (g.sampled_time_valid && g.sampled_time == g.now) return;
    g.sampled_time = g.now;
    g.sampled_time_valid = 1;
    for (llg_sampled_real_t* item = g.sampled_reals; item; item = item->next)
        item->value = *item->signal;
    for (llg_sampled_value_t* item = g.sampled_values; item; item = item->next)
        sv4_copy(&item->value, item->signal);
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        sv4_copy(&item->value, item->signal);
        llg_sampled_history_t* last = item->history;
        if (last && last->time == g.now) {
            sv4_copy(&last->value, &item->value);
            continue;
        }
        llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
            1, sizeof(*history), "sampled history");
        history->time = g.now;
        history->value = sv4_clone(&item->value);
        history->next = item->history;
        item->history = history;
    }
}

typedef struct {
    sv4_t* source;
    sv4_t* sample;
} llg_clocking_observed_t;

static void clocking_copy_observed(void* data) {
    llg_clocking_observed_t* copy = (llg_clocking_observed_t*)data;
    sv4_copy(copy->sample, copy->source);
    free(copy);
}

int llg_clocking_sample_observed(sv4_t* source, sv4_t* sample) {
    if (!source || !sample) return 0;
    if (!find_sampled_value(source)) {
        report_unregistered_sampled_signal();
        return 0;
    }
    llg_clocking_observed_t* copy = (llg_clocking_observed_t*)llg_checked_malloc(
        1, sizeof(*copy), "clocking observed sample");
    copy->source = source;
    copy->sample = sample;
    if (!llg_schedule_region_callback(LLG_REGION_OBSERVED,
                                      clocking_copy_observed, copy)) {
        free(copy);
        return 0;
    }
    return 1;
}

int llg_clocking_sample_history(sv4_t* source, sv4_t* sample, uint64_t ticks) {
    if (!source || !sample) return 0;
    llg_sampled_value_t* item = find_sampled_value(source);
    if (!item) {
        report_unregistered_sampled_signal();
        return 0;
    }
    uint64_t target = g.now < ticks ? 0 : g.now - ticks;
    llg_sampled_history_t* selected = NULL;
    for (llg_sampled_history_t* history = item->history; history;
         history = history->next) {
        if (history->time > target) continue;
        if (!selected || selected->time < history->time) selected = history;
    }
    sv4_copy(sample, selected ? &selected->value : &item->value);
    return 1;
}
