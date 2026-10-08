
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

// Record `value` as the slot's newest sample. A skewed read selects the
// newest entry at or before `now - ticks`, so everything older than the newest
// entry at or before `now - history_ticks` can never be selected again.
static void sampled_history_record(llg_sampled_value_t* item, const sv4_t* value) {
    llg_sampled_history_t* last = item->history;
    if (last && last->time == g.now) {
        sv4_copy(&last->value, value);
        return;
    }
    llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
        1, sizeof(*history), "sampled history");
    history->time = g.now;
    history->value = sv4_clone(value);
    history->prev = NULL;
    history->next = last;
    if (last) last->prev = history;
    else item->history_tail = history;
    item->history = history;
    const uint64_t horizon =
        g.now < item->history_ticks ? 0 : g.now - item->history_ticks;
    while (item->history_tail->prev && item->history_tail->prev->time <= horizon) {
        llg_sampled_history_t* oldest = item->history_tail;
        item->history_tail = oldest->prev;
        item->history_tail->next = NULL;
        sv4_destroy(&oldest->value);
        free(oldest);
    }
}

static void sampled_record_write(sv4_t* signal) {
    llg_sampled_value_t* item = find_sampled_value(signal);
    if (item) sampled_history_record(item, signal);
}

static void sampled_register(sv4_t* signal, uint64_t history_ticks) {
    if (!signal) {
        fprintf(stderr, "llg: cannot register a null sampled signal\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) {
            if (item->history_ticks < history_ticks) item->history_ticks = history_ticks;
            return;
        }
    }
    // A value-only registration is promoted, so each signal is sampled once.
    llg_sampled_value_t* item = NULL;
    for (llg_sampled_value_t** link = &g.sampled_values; *link;
         link = &(*link)->next) {
        if ((*link)->signal == signal) {
            item = *link;
            *link = item->next;
            sv4_destroy(&item->value);
            break;
        }
    }
    if (!item)
        item = (llg_sampled_value_t*)llg_checked_malloc(1, sizeof(*item),
                                                        "sampled value");
    item->signal = signal;
    item->value = sv4_clone(signal);
    item->history = NULL;
    item->history_tail = NULL;
    item->history_ticks = history_ticks;
    sampled_history_record(item, signal);
    item->next = g.sampled;
    g.sampled = item;
}

void llg_sampled_register(sv4_t* signal) {
    sampled_register(signal, 0);
}

void llg_sampled_register_history(sv4_t* signal, uint64_t ticks) {
    sampled_register(signal, ticks);
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

static void report_sampled_failure(const char* message, uint64_t identity) {
    fprintf(stderr, "llg: %s %llu\n", message, (unsigned long long)identity);
    llg_last_failure = 1;
    g.finish = 1;
}

static llg_sampled_clock_t* find_sampled_clock(uint64_t identity) {
    return identity < g.sampled_clocks_capacity ? g.sampled_clocks[identity] : NULL;
}

static llg_sampled_domain_t* find_sampled_domain(uint64_t identity) {
    return identity < g.sampled_domains_capacity ? g.sampled_domains[identity]
                                                 : NULL;
}

// Capacity of a dense identity table that `identity` indexes; doubling keeps
// registration linear in the number of identities.
static size_t sampled_table_capacity(size_t capacity, uint64_t identity,
                                     const char* what) {
    if (identity >= SIZE_MAX / 2 / sizeof(void*)) llg_fatal_allocation(what, SIZE_MAX, 1);
    size_t grown = capacity ? capacity : 8;
    while (grown <= identity) grown *= 2;
    return grown;
}

static void sampled_clocks_reserve(uint64_t identity) {
    if (identity < g.sampled_clocks_capacity) return;
    size_t grown = sampled_table_capacity(g.sampled_clocks_capacity, identity,
                                          "sampled-value clocks");
    llg_sampled_clock_t** table = (llg_sampled_clock_t**)llg_checked_calloc(
        grown, sizeof(*table), "sampled-value clocks");
    for (size_t index = 0; index < g.sampled_clocks_capacity; index++)
        table[index] = g.sampled_clocks[index];
    free(g.sampled_clocks);
    g.sampled_clocks = table;
    g.sampled_clocks_capacity = grown;
}

static void sampled_domains_reserve(uint64_t identity) {
    if (identity < g.sampled_domains_capacity) return;
    size_t grown = sampled_table_capacity(g.sampled_domains_capacity, identity,
                                          "sampled-value domains");
    llg_sampled_domain_t** table = (llg_sampled_domain_t**)llg_checked_calloc(
        grown, sizeof(*table), "sampled-value domains");
    for (size_t index = 0; index < g.sampled_domains_capacity; index++)
        table[index] = g.sampled_domains[index];
    free(g.sampled_domains);
    g.sampled_domains = table;
    g.sampled_domains_capacity = grown;
}

static llg_sampled_clock_t* sampled_clock_new(uint64_t identity) {
    if (find_sampled_clock(identity)) {
        report_sampled_failure("duplicate sampled-value clock", identity);
        return NULL;
    }
    sampled_clocks_reserve(identity);
    llg_sampled_clock_t* clock = (llg_sampled_clock_t*)llg_checked_calloc(
        1, sizeof(*clock), "sampled-value clock");
    g.sampled_clocks[identity] = clock;
    return clock;
}

int llg_sampled_clock_register_edge(uint64_t identity, sv4_t* signal, int edge,
                                    llg_sampled_gate_fn gate, void* data) {
    if (!signal || (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE)) {
        report_sampled_failure("invalid sampled-value clock", identity);
        return 0;
    }
    llg_sampled_clock_t* clock = sampled_clock_new(identity);
    if (!clock) return 0;
    clock->signal = signal;
    clock->edge = edge;
    clock->gate = gate;
    clock->data = data;
    clock->next_edge = g.sampled_edge_clocks;
    g.sampled_edge_clocks = clock;
    return 1;
}

int llg_sampled_clock_register_event(uint64_t identity, llg_sampled_gate_fn gate,
                                     void* data) {
    llg_sampled_clock_t* clock = sampled_clock_new(identity);
    if (!clock) return 0;
    clock->gate = gate;
    clock->data = data;
    return 1;
}

int llg_sampled_domain_register(uint64_t identity, uint64_t clock_identity,
                                llg_sampled_domain_eval_fn value, void* data,
                                uint64_t history_ticks) {
    llg_sampled_clock_t* clock = find_sampled_clock(clock_identity);
    if (!clock || !value || history_ticks == 0) {
        report_sampled_failure("invalid sampled-value domain", identity);
        return 0;
    }
    if (find_sampled_domain(identity)) {
        report_sampled_failure("duplicate sampled-value domain", identity);
        return 0;
    }
    sampled_domains_reserve(identity);
    llg_sampled_domain_t* domain = (llg_sampled_domain_t*)llg_checked_calloc(
        1, sizeof(*domain), "sampled-value domain");
    domain->clock = clock;
    domain->value = value;
    domain->data = data;
    // Registration precedes process startup, so the Preponed snapshots still
    // hold every variable's declaration or default value: the initial value
    // that $past and the value change functions use before enough ticks.
    const sv4_t empty = SV4_EMPTY;
    domain->initial = empty;
    value(data, &domain->initial);
    domain->current = empty;
    // `$past(e, n)` may skip the current time step and then n - 1 earlier
    // ones; a ring larger than memory can hold is never reached in practice
    // because it grows only as ticks occur.
    domain->limit = history_ticks >= SIZE_MAX ? SIZE_MAX : (size_t)history_ticks + 1;
    if (clock->n_domains == clock->domains_capacity) {
        size_t grown = clock->domains_capacity ? clock->domains_capacity * 2 : 4;
        llg_sampled_domain_t** domains = (llg_sampled_domain_t**)llg_checked_malloc(
            grown, sizeof(*domains), "sampled-value clock domains");
        for (size_t index = 0; index < clock->n_domains; index++)
            domains[index] = clock->domains[index];
        free(clock->domains);
        clock->domains = domains;
        clock->domains_capacity = grown;
    }
    clock->domains[clock->n_domains++] = domain;
    g.sampled_domains[identity] = domain;
    return 1;
}

// Make room for one more time step, oldest first in the new storage. Growth
// doubles up to the domain's limit, so steady-state ticks reuse slots.
static void sampled_domain_grow(llg_sampled_domain_t* domain) {
    size_t grown = domain->capacity == 0              ? 4
                   : domain->capacity > domain->limit / 2 ? domain->limit
                                                          : domain->capacity * 2;
    if (grown > domain->limit) grown = domain->limit;
    sv4_t* samples = (sv4_t*)llg_checked_malloc(grown, sizeof(*samples),
                                                "sampled-value history");
    uint64_t* times = (uint64_t*)llg_checked_malloc(grown, sizeof(*times),
                                                    "sampled-value history");
    size_t oldest = domain->count
                        ? (domain->newest + domain->capacity + 1 - domain->count) %
                              domain->capacity
                        : 0;
    for (size_t index = 0; index < domain->count; index++) {
        size_t from = (oldest + index) % domain->capacity;
        samples[index] = domain->samples[from];
        times[index] = domain->times[from];
    }
    const sv4_t empty = SV4_EMPTY;
    for (size_t index = domain->count; index < grown; index++) samples[index] = empty;
    free(domain->samples);
    free(domain->times);
    domain->samples = samples;
    domain->times = times;
    domain->capacity = grown;
    domain->newest = domain->count ? domain->count - 1 : 0;
}

// Record the clock's tick in every domain on it. A tick repeated within a
// time step overwrites that step's sample, so each step counts once.
static void sampled_clock_record(llg_sampled_clock_t* clock) {
    for (size_t index = 0; index < clock->n_domains; index++) {
        llg_sampled_domain_t* domain = clock->domains[index];
        if (domain->count == 0 || domain->times[domain->newest] != g.now) {
            if (domain->count == domain->capacity && domain->capacity < domain->limit)
                sampled_domain_grow(domain);
            domain->newest = domain->count ? (domain->newest + 1) % domain->capacity : 0;
            if (domain->count < domain->capacity) domain->count++;
            domain->times[domain->newest] = g.now;
        }
        domain->value(domain->data, &domain->samples[domain->newest]);
    }
}

void llg_sampled_clock_tick(uint64_t identity) {
    llg_sampled_clock_t* clock = find_sampled_clock(identity);
    if (!clock || clock->signal) {
        report_sampled_failure("sampled-value clock is not an event clock", identity);
        return;
    }
    if (clock->gate && !clock->gate(clock->data)) return;
    sampled_clock_record(clock);
}

static void sampled_domain_clock_signal_changed(sv4_t* signal, sv4_t old,
                                                sv4_t value) {
    for (llg_sampled_clock_t* clock = g.sampled_edge_clocks; clock;
         clock = clock->next_edge) {
        if (clock->signal != signal || !ev_matches_changed(old, value, clock->edge))
            continue;
        if (clock->gate && !clock->gate(clock->data)) continue;
        sampled_clock_record(clock);
    }
}

// The k-th time step strictly before the current one in which the clock
// ticked (IEEE 1800-2009 16.9.3), or NULL when fewer ticks exist. A tick in
// the current step is not one of them, but the latest tick before it is, as
// when procedural code evaluates between clock edges.
static const sv4_t* sampled_domain_prior(const llg_sampled_domain_t* domain,
                                         uint64_t ticks) {
    size_t skip = domain->count && domain->times[domain->newest] == g.now ? 1 : 0;
    if (ticks == 0 || ticks > domain->count - skip) return NULL;
    size_t back = skip + (size_t)ticks - 1;
    return &domain->samples[(domain->newest + domain->capacity - back) % domain->capacity];
}

sv4_t llg_sampled_domain_past(uint64_t identity, uint64_t ticks) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_sampled_failure("sampled-value domain is not registered", identity);
        return sv4_x(1, 0);
    }
    if (ticks == 0) return sv4_clone(&domain->initial);
    const sv4_t* prior = sampled_domain_prior(domain, ticks);
    return sv4_clone(prior ? prior : &domain->initial);
}

void llg_sampled_domain_past_to(sv4_t* dst, uint64_t identity, uint64_t ticks) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_sampled_failure("sampled-value domain is not registered", identity);
        sv4_replace(dst, sv4_x(1, 0));
        return;
    }
    const sv4_t* prior = ticks ? sampled_domain_prior(domain, ticks) : NULL;
    sv4_copy(dst, prior ? prior : &domain->initial);
}

static int sampled_domain_lsb_one(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) != 0;
}

static int sampled_domain_lsb_zero(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) == 0;
}

// The Preponed value of the calling time step: the tick sample when the clock
// ticked in this step, otherwise one evaluation cached for the step.
static const sv4_t* sampled_domain_current(llg_sampled_domain_t* domain) {
    if (domain->count && domain->times[domain->newest] == g.now)
        return &domain->samples[domain->newest];
    if (!domain->current_valid || domain->current_time != g.now) {
        domain->value(domain->data, &domain->current);
        domain->current_time = g.now;
        domain->current_valid = 1;
    }
    return &domain->current;
}

int llg_sampled_domain_status(uint64_t identity, int kind) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_sampled_failure("sampled-value domain is not registered", identity);
        return 0;
    }
    // Value change functions compare the current step's Preponed value with
    // the most recent strictly prior tick, or with the initial value at or
    // before the first tick (16.9.3).
    sv4_t current = *sampled_domain_current(domain);
    const sv4_t* prior = sampled_domain_prior(domain, 1);
    sv4_t previous = prior ? *prior : domain->initial;
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
        sampled_history_record(item, &item->value);
    }
}

// Clockvar samples publish like signal writes, so `@(cb.x)` and edges of a
// clockvar wake on a changed sampled value (IEEE 1800-2009 14.15).
static void clocking_publish(sv4_t* sample, const sv4_t* value) {
    clocking_sample_publication = 1;
    sig_write(sample, *value);
    clocking_sample_publication = 0;
}

static void clocking_publish_real(double* sample, double value) {
    clocking_sample_publication = 1;
    real_write(sample, value);
    clocking_sample_publication = 0;
}

int llg_clocking_sample(const sv4_t* source, sv4_t* sample) {
    if (!sample) return 0;
    const sv4_t* value = llg_sampled_value(source);
    if (!value) return 0;
    clocking_publish(sample, value);
    return 1;
}

typedef struct {
    sv4_t* source;
    sv4_t* sample;
} llg_clocking_observed_t;

static void clocking_copy_observed(void* data) {
    llg_clocking_observed_t* copy = (llg_clocking_observed_t*)data;
    clocking_publish(copy->sample, copy->source);
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

// The newest retained value at or before `ticks` in the past.
static const sv4_t* clocking_history_value(const sv4_t* source, uint64_t ticks) {
    llg_sampled_value_t* item = find_sampled_value(source);
    if (!item) {
        report_unregistered_sampled_signal();
        return NULL;
    }
    uint64_t target = g.now < ticks ? 0 : g.now - ticks;
    llg_sampled_history_t* selected = NULL;
    for (llg_sampled_history_t* history = item->history; history;
         history = history->next) {
        if (history->time > target) continue;
        if (!selected || selected->time < history->time) selected = history;
    }
    return selected ? &selected->value : &item->value;
}

int llg_clocking_sample_history(sv4_t* source, sv4_t* sample, uint64_t ticks) {
    if (!source || !sample) return 0;
    const sv4_t* value = clocking_history_value(source, ticks);
    if (!value) return 0;
    clocking_publish(sample, value);
    return 1;
}

// Real clockvars are sampled from the 64-bit IEEE image their expression
// keeps in packed storage, so they share the packed Preponed, Observed and
// history entries; only the final store decodes the image.
static int clocking_store_real(const sv4_t* image, double* sample) {
    if (!image || llg_sv4_width(*image) != 64) {
        fprintf(stderr, "llg: real clocking sample needs a 64-bit image\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    clocking_publish_real(sample, sv4_bitstoreal(*image));
    return 1;
}

int llg_clocking_sample_real(const sv4_t* source, double* sample) {
    if (!source || !sample) return 0;
    return clocking_store_real(llg_sampled_value(source), sample);
}

typedef struct {
    sv4_t* source;
    double* sample;
} llg_clocking_observed_real_t;

static void clocking_copy_observed_real(void* data) {
    llg_clocking_observed_real_t* copy = (llg_clocking_observed_real_t*)data;
    (void)clocking_store_real(copy->source, copy->sample);
    free(copy);
}

int llg_clocking_sample_observed_real(sv4_t* source, double* sample) {
    if (!source || !sample) return 0;
    if (!find_sampled_value(source)) {
        report_unregistered_sampled_signal();
        return 0;
    }
    llg_clocking_observed_real_t* copy =
        (llg_clocking_observed_real_t*)llg_checked_malloc(
            1, sizeof(*copy), "clocking observed sample");
    copy->source = source;
    copy->sample = sample;
    if (!llg_schedule_region_callback(LLG_REGION_OBSERVED,
                                      clocking_copy_observed_real, copy)) {
        free(copy);
        return 0;
    }
    return 1;
}

int llg_clocking_sample_history_real(sv4_t* source, double* sample,
                                     uint64_t ticks) {
    if (!source || !sample) return 0;
    const sv4_t* value = clocking_history_value(source, ticks);
    return value ? clocking_store_real(value, sample) : 0;
}
