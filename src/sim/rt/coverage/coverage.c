// ── Functional coverage (IEEE 1800-2009 19) ─────────────────────────────────
//
// Generated code emits one static `llg_cov_group_t` per covergroup
// declaration site (its coverpoint metadata) and one `llg_cov_type_t` per
// declaration identity; type coverage merges every instance ever created for
// that identity (19.11.3). Instance records are runtime-owned and live until
// llg_rt_cleanup, so counters survive the collection of the covergroup
// object. A model without covergroups never reaches this file's state.

typedef struct {
    uint64_t* keys;   // bin index + 1; zero marks an empty slot
    uint64_t* counts;
    size_t capacity;  // a power of two, or zero before the first hit
    size_t used;
} llg_cov_sparse_t;

typedef struct {
    int64_t weight;
    int64_t goal;
    int64_t at_least;
    int64_t auto_bin_max;
    uint8_t has_at_least;
    uint8_t has_auto_bin_max;
    uint8_t stopped;
    uint32_t n_bins;  // resolved by llg_cov_ready
    uint64_t* dense;  // n_bins counters when n_bins <= LLG_COV_DENSE_BINS
    llg_cov_sparse_t sparse;
} llg_cov_point_state_t;

struct llg_cov_inst {
    llg_cov_group_t* group;
    struct llg_cov_inst* next;  // instance list of the declaration's type
    int64_t weight;
    int64_t goal;
    int64_t at_least;
    int64_t auto_bin_max;
    uint8_t stopped;
    uint8_t ready;
    uint8_t strobe_pending;  // a Postponed sample is queued for this slot
    llg_cov_point_state_t points[];
};

// Types that own at least one instance, for $get_coverage and cleanup.
static llg_cov_type_t* llg_cov_types_used;
// Groups whose registry holds sampled objects, for cleanup.
static llg_cov_group_t* llg_cov_groups_registered;

static void* llg_cov_grow(void* data, size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size) llg_fatal_allocation(what, count, size);
    size_t bytes = count * size;
    void* grown = realloc(data, bytes != 0 ? bytes : 1);
    if (!grown) llg_fatal_allocation(what, count, size);
    return grown;
}

// Signed coverpoints order bins by numeric value: flipping the sign bit maps
// the two's-complement range onto ascending unsigned keys.
static uint64_t llg_cov_key(const llg_cov_point_t* point, uint64_t raw) {
    uint64_t mask = point->width >= 64 ? UINT64_MAX : ((UINT64_C(1) << point->width) - 1u);
    uint64_t key = raw & mask;
    if (point->is_signed) key ^= UINT64_C(1) << (point->width - 1u);
    return key;
}

static uint32_t llg_cov_auto_bins(const llg_cov_point_t* point, int64_t auto_bin_max) {
    uint64_t limit = auto_bin_max < 1 ? 1u : (uint64_t)auto_bin_max;
    if (limit > UINT32_MAX) limit = UINT32_MAX;
    if (point->width < 32u && (UINT64_C(1) << point->width) <= limit)
        return (uint32_t)(UINT64_C(1) << point->width);
    return (uint32_t)limit;
}

// 19.5.2: 2^M values spread uniformly over N bins; the last bin also takes
// the remainder.
static uint32_t llg_cov_auto_bin(const llg_cov_point_t* point, uint32_t n_bins, uint64_t key) {
    uint64_t span;
    if (point->width >= 64u) {
        span = UINT64_MAX / n_bins;
        if (UINT64_MAX % n_bins == (uint64_t)n_bins - 1u) span++;
    } else {
        span = (UINT64_C(1) << point->width) / n_bins;
    }
    uint64_t bin = key / span;
    return bin >= n_bins ? n_bins - 1u : (uint32_t)bin;
}

static void llg_cov_sparse_add(llg_cov_sparse_t* table, uint64_t bin) {
    if ((table->used + 1u) * 2u > table->capacity) {
        size_t capacity = table->capacity ? table->capacity * 2u : 16u;
        uint64_t* keys = (uint64_t*)llg_checked_calloc(capacity, sizeof(uint64_t), "coverage bins");
        uint64_t* counts = (uint64_t*)llg_checked_calloc(capacity, sizeof(uint64_t), "coverage bins");
        for (size_t i = 0; i < table->capacity; ++i) {
            if (!table->keys[i]) continue;
            size_t slot = (size_t)(table->keys[i] * UINT64_C(0x9E3779B97F4A7C15)) & (capacity - 1u);
            while (keys[slot]) slot = (slot + 1u) & (capacity - 1u);
            keys[slot] = table->keys[i];
            counts[slot] = table->counts[i];
        }
        free(table->keys);
        free(table->counts);
        table->keys = keys;
        table->counts = counts;
        table->capacity = capacity;
    }
    uint64_t key = bin + 1u;
    size_t slot = (size_t)(key * UINT64_C(0x9E3779B97F4A7C15)) & (table->capacity - 1u);
    while (table->keys[slot] && table->keys[slot] != key)
        slot = (slot + 1u) & (table->capacity - 1u);
    if (!table->keys[slot]) {
        table->keys[slot] = key;
        table->used++;
    }
    if (table->counts[slot] != UINT64_MAX) table->counts[slot]++;
}

static void llg_cov_count(llg_cov_point_state_t* state, uint32_t bin) {
    if (state->dense) {
        if (state->dense[bin] != UINT64_MAX) state->dense[bin]++;
    } else {
        llg_cov_sparse_add(&state->sparse, bin);
    }
}

void* llg_cov_new(llg_cov_group_t* group) {
    size_t points = group->n_points;
    llg_cov_inst_t* inst = (llg_cov_inst_t*)llg_checked_calloc(
        1, sizeof(llg_cov_inst_t) + points * sizeof(llg_cov_point_state_t), "covergroup instance");
    inst->group = group;
    // Table 19-1 defaults.
    inst->weight = 1;
    inst->goal = 100;
    inst->at_least = 1;
    inst->auto_bin_max = 64;
    for (size_t i = 0; i < points; ++i) {
        inst->points[i].weight = 1;
        inst->points[i].goal = 100;
    }
    llg_cov_type_t* type = group->type;
    if (!type->used) {
        type->used = 1;
        type->next_used = llg_cov_types_used;
        llg_cov_types_used = type;
    }
    if (type->last) type->last->next = inst;
    else type->first = inst;
    type->last = inst;
    type->n_instances++;
    return inst;
}

static int llg_cov_option_value(sv4_t value, int64_t* out, const char* option, const char* site) {
    if (sv4_is_unknown(value)) {
        fprintf(stderr, "llg: %s: coverage option %s is X or Z; the default is kept\n", site, option);
        llg_rt_mark_failed();
        return 0;
    }
    uint64_t raw = sv4_to_u64(value);
    uint32_t width = llg_sv4_width(value);
    // Option values are int (19.10); narrower signed values sign-extend.
    if (width > 0u && width < 64u && llg_sv4_signed(value) && ((raw >> (width - 1u)) & 1u))
        raw |= ~((UINT64_C(1) << width) - 1u);
    int64_t number = (int64_t)raw;
    *out = number;
    return 1;
}

void llg_cov_option(void* record, int32_t point, uint32_t option, sv4_t value, const char* site) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    static const char* const names[] = {"weight", "goal", "at_least", "auto_bin_max"};
    if (!inst || option > LLG_COV_OPTION_AUTO_BIN_MAX) return;
    int64_t number;
    if (!llg_cov_option_value(value, &number, names[option], site)) return;
    // Table 19-1: weight is non-negative; a bin count needs at least one bin.
    if ((option == LLG_COV_OPTION_WEIGHT && number < 0) ||
        (option == LLG_COV_OPTION_AUTO_BIN_MAX && number < 1)) {
        fprintf(stderr, "llg: %s: coverage option %s value %lld is out of range; the default is kept\n",
                site, names[option], (long long)number);
        llg_rt_mark_failed();
        return;
    }
    if (point < 0) {
        switch (option) {
        case LLG_COV_OPTION_WEIGHT: inst->weight = number; break;
        case LLG_COV_OPTION_GOAL: inst->goal = number; break;
        case LLG_COV_OPTION_AT_LEAST: inst->at_least = number; break;
        default: inst->auto_bin_max = number; break;
        }
        return;
    }
    if ((uint32_t)point >= inst->group->n_points) return;
    llg_cov_point_state_t* state = &inst->points[point];
    switch (option) {
    case LLG_COV_OPTION_WEIGHT: state->weight = number; break;
    case LLG_COV_OPTION_GOAL: state->goal = number; break;
    case LLG_COV_OPTION_AT_LEAST:
        state->at_least = number;
        state->has_at_least = 1;
        break;
    default:
        state->auto_bin_max = number;
        state->has_auto_bin_max = 1;
        break;
    }
}

// Group-level at_least and auto_bin_max are coverpoint defaults (19.7); bin
// storage is sized once the options are final.
void llg_cov_ready(void* record) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || inst->ready) return;
    inst->ready = 1;
    for (uint32_t i = 0; i < inst->group->n_points; ++i) {
        const llg_cov_point_t* point = &inst->group->points[i];
        llg_cov_point_state_t* state = &inst->points[i];
        if (!state->has_at_least) state->at_least = inst->at_least;
        if (!state->has_auto_bin_max) state->auto_bin_max = inst->auto_bin_max;
        state->n_bins = point->kind == LLG_COV_POINT_AUTO
            ? llg_cov_auto_bins(point, state->auto_bin_max)
            : point->n_bins;
        if (state->n_bins <= LLG_COV_DENSE_BINS)
            state->dense = (uint64_t*)llg_checked_calloc(state->n_bins, sizeof(uint64_t),
                                                         "coverage bins");
    }
}

void llg_cov_hit(void* record, uint32_t point_index, const sv4_t* value) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || inst->stopped || point_index >= inst->group->n_points) return;
    llg_cov_point_state_t* state = &inst->points[point_index];
    if (state->stopped || !state->n_bins) return;
    // Bins only hold 2-state values (19.5.2, 19.5.6).
    if (sv4_is_unknown(*value)) return;
    const llg_cov_point_t* point = &inst->group->points[point_index];
    uint64_t key = llg_cov_key(point, sv4_to_u64(*value));
    switch (point->kind) {
    case LLG_COV_POINT_AUTO:
        llg_cov_count(state, llg_cov_auto_bin(point, state->n_bins, key));
        return;
    case LLG_COV_POINT_ENUM: {
        uint32_t lo = 0, hi = point->n_bins;
        while (lo < hi) {
            uint32_t mid = lo + (hi - lo) / 2u;
            if (point->keys[mid] < key) lo = mid + 1u;
            else hi = mid;
        }
        if (lo < point->n_bins && point->keys[lo] == key) llg_cov_count(state, lo);
        return;
    }
    default: {
        uint32_t lo = 0, hi = point->n_segments;
        while (lo < hi) {
            uint32_t mid = lo + (hi - lo) / 2u;
            if (point->segments[mid].hi < key) lo = mid + 1u;
            else hi = mid;
        }
        if (lo == point->n_segments || point->segments[lo].lo > key) return;
        const llg_cov_segment_t* segment = &point->segments[lo];
        // A value in several overlapping bins counts in each of them.
        for (uint32_t i = 0; i < segment->count; ++i)
            llg_cov_count(state, point->segment_bins[segment->first + i]);
        return;
    }
    }
}

void llg_cov_control(void* record, int32_t point, int start) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst) return;
    if (point < 0) inst->stopped = (uint8_t)!start;
    else if ((uint32_t)point < inst->group->n_points) inst->points[point].stopped = (uint8_t)!start;
}

void llg_cov_register(llg_cov_group_t* group, void* object) {
    if (!object) return;
    if (group->n_registered == group->registry_capacity) {
        size_t capacity = group->registry_capacity ? group->registry_capacity * 2u : 8u;
        group->registry = (void**)llg_cov_grow(group->registry, capacity, sizeof(void*),
                                               "covergroup registry");
        group->registry_capacity = capacity;
    }
    if (!group->registered) {
        group->registered = 1;
        group->next_registered = llg_cov_groups_registered;
        llg_cov_groups_registered = group;
    }
    // The sampling event refers to the instance for the rest of the run.
    llg_gc_pin(object);
    group->registry[group->n_registered++] = object;
}

size_t llg_cov_registered(const llg_cov_group_t* group) {
    return group->n_registered;
}

void* llg_cov_registered_at(const llg_cov_group_t* group, size_t index) {
    return index < group->n_registered ? group->registry[index] : NULL;
}

int llg_cov_sampling(const void* record) {
    const llg_cov_inst_t* inst = (const llg_cov_inst_t*)record;
    return inst && !inst->stopped;
}

// Strobe requests of the current time slot, flushed by one Postponed region
// callback in request order (19.7.1). An entry without an instance stands
// for a pass over its group's registry. Objects stay reachable through the
// registry pin or the waiter's capture frame.
typedef struct {
    llg_cov_group_t* group;
    llg_cov_inst_t* inst;
    void* object;
} llg_cov_strobe_request_t;
static llg_cov_strobe_request_t* llg_cov_strobes;
static size_t llg_cov_n_strobes;
static size_t llg_cov_strobe_capacity;
static uint8_t llg_cov_strobe_scheduled;

static void llg_cov_strobe_flush(void* data) {
    (void)data;
    llg_cov_strobe_scheduled = 0;
    // Requests made while flushing belong to no slot of this one; the
    // Postponed region cannot trigger a clocking event.
    for (size_t i = 0; i < llg_cov_n_strobes; ++i) {
        llg_cov_strobe_request_t request = llg_cov_strobes[i];
        if (request.inst) {
            request.inst->strobe_pending = 0;
            request.group->strobe_sample(request.object);
            continue;
        }
        request.group->strobe_pending = 0;
        for (size_t k = 0; k < request.group->n_registered; ++k)
            request.group->strobe_sample(request.group->registry[k]);
    }
    llg_cov_n_strobes = 0;
}

static void llg_cov_strobe_queue(llg_cov_group_t* group, llg_cov_inst_t* inst, void* object) {
    if (llg_cov_n_strobes == llg_cov_strobe_capacity) {
        size_t capacity = llg_cov_strobe_capacity ? llg_cov_strobe_capacity * 2u : 8u;
        llg_cov_strobes = (llg_cov_strobe_request_t*)llg_cov_grow(
            llg_cov_strobes, capacity, sizeof(*llg_cov_strobes), "covergroup strobe requests");
        llg_cov_strobe_capacity = capacity;
    }
    llg_cov_strobes[llg_cov_n_strobes++] = (llg_cov_strobe_request_t){group, inst, object};
    if (!llg_cov_strobe_scheduled) {
        // A refused schedule (the run is ending) leaves the requests for
        // the next request to retry.
        llg_cov_strobe_scheduled = (uint8_t)llg_schedule_region_callback(
            LLG_REGION_POSTPONED, llg_cov_strobe_flush, NULL);
    }
}

void llg_cov_strobe(llg_cov_group_t* group, void* record, void* object) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!group->strobe_sample || !inst || !object || inst->strobe_pending) return;
    inst->strobe_pending = 1;
    llg_cov_strobe_queue(group, inst, object);
}

void llg_cov_strobe_registered(llg_cov_group_t* group) {
    if (!group->strobe_sample || group->strobe_pending || !group->n_registered) return;
    group->strobe_pending = 1;
    llg_cov_strobe_queue(group, NULL, NULL);
}

// Covered and defined bins of one coverpoint (19.11.1). A defined bin with no
// value never counts (only explicit bins can lack values).
static void llg_cov_point_bins(const llg_cov_inst_t* inst, uint32_t index, uint64_t* covered,
                               uint64_t* defined) {
    const llg_cov_point_t* point = &inst->group->points[index];
    const llg_cov_point_state_t* state = &inst->points[index];
    uint64_t at_least = state->at_least < 0 ? 0u : (uint64_t)state->at_least;
    *defined = point->kind == LLG_COV_POINT_EXPLICIT ? point->n_valued : state->n_bins;
    *covered = 0;
    if (!inst->ready) return;
    if (state->dense) {
        for (uint32_t bin = 0; bin < state->n_bins; ++bin) {
            if (point->kind == LLG_COV_POINT_EXPLICIT && !point->valued[bin]) continue;
            if (state->dense[bin] >= at_least) (*covered)++;
        }
        return;
    }
    // Sparse storage lists hit bins only; unhit bins are covered only when
    // at_least is zero.
    if (at_least == 0) {
        *covered = *defined;
        return;
    }
    for (size_t slot = 0; slot < state->sparse.capacity; ++slot) {
        uint64_t key = state->sparse.keys[slot];
        if (!key) continue;
        if (point->kind == LLG_COV_POINT_EXPLICIT && !point->valued[key - 1u]) continue;
        if (state->sparse.counts[slot] >= at_least) (*covered)++;
    }
}

// Weighted coverpoint average of one instance (19.11); `point` < 0 selects
// the whole group. Returns 0 when the denominator is zero.
static int llg_cov_inst_score(const llg_cov_inst_t* inst, int32_t point, double* score) {
    if (point >= 0) {
        uint64_t covered, defined;
        llg_cov_point_bins(inst, (uint32_t)point, &covered, &defined);
        if (!defined) return 0;
        *score = 100.0 * (double)covered / (double)defined;
        return 1;
    }
    double numerator = 0.0, denominator = 0.0;
    for (uint32_t i = 0; i < inst->group->n_points; ++i) {
        uint64_t covered, defined;
        llg_cov_point_bins(inst, i, &covered, &defined);
        if (!defined || inst->points[i].weight <= 0) continue;
        double weight = (double)inst->points[i].weight;
        numerator += weight * 100.0 * (double)covered / (double)defined;
        denominator += weight;
    }
    if (denominator == 0.0) return 0;
    *score = numerator / denominator;
    return 1;
}

double llg_cov_inst_coverage(const void* record, int32_t point) {
    const llg_cov_inst_t* inst = (const llg_cov_inst_t*)record;
    if (!inst) return 0.0;
    double score;
    if (llg_cov_inst_score(inst, point, &score)) return score;
    int64_t weight = point < 0 ? inst->weight : inst->points[point].weight;
    return weight != 0 ? 0.0 : 100.0;
}

// Type coverage with merge_instances = 0 (19.11.3): instances weighted by
// their option.weight (or a coverpoint's option.weight for one coverpoint).
static int llg_cov_type_score(const llg_cov_type_t* type, int32_t point, double* score) {
    double numerator = 0.0, denominator = 0.0;
    for (const llg_cov_inst_t* inst = type->first; inst; inst = inst->next) {
        if (point >= 0 && (uint32_t)point >= inst->group->n_points) continue;
        double value;
        if (!llg_cov_inst_score(inst, point, &value)) continue;
        int64_t weight = point < 0 ? inst->weight : inst->points[point].weight;
        if (weight <= 0) continue;
        numerator += (double)weight * value;
        denominator += (double)weight;
    }
    if (denominator == 0.0) return 0;
    *score = numerator / denominator;
    return 1;
}

double llg_cov_type_coverage(const llg_cov_type_t* type, int32_t point, int64_t type_weight) {
    double score;
    if (llg_cov_type_score(type, point, &score)) return score;
    return type_weight != 0 ? 0.0 : 100.0;
}

// $get_coverage (19.9): covergroup types weighted by type_option.weight; a
// type with a zero denominator does not contribute, and no contribution at
// all yields 100.0 (19.11).
double llg_cov_total_coverage(void) {
    double numerator = 0.0, denominator = 0.0;
    for (const llg_cov_type_t* type = llg_cov_types_used; type; type = type->next_used) {
        double score;
        if (type->weight <= 0 || !llg_cov_type_score(type, -1, &score)) continue;
        numerator += (double)type->weight * score;
        denominator += (double)type->weight;
    }
    return denominator == 0.0 ? 100.0 : numerator / denominator;
}

void llg_cov_release_all(void) {
    free(llg_cov_strobes);
    llg_cov_strobes = NULL;
    llg_cov_n_strobes = llg_cov_strobe_capacity = 0;
    llg_cov_strobe_scheduled = 0;
    while (llg_cov_types_used) {
        llg_cov_type_t* type = llg_cov_types_used;
        llg_cov_types_used = type->next_used;
        llg_cov_inst_t* inst = type->first;
        while (inst) {
            llg_cov_inst_t* next = inst->next;
            for (uint32_t i = 0; i < inst->group->n_points; ++i) {
                free(inst->points[i].dense);
                free(inst->points[i].sparse.keys);
                free(inst->points[i].sparse.counts);
            }
            free(inst);
            inst = next;
        }
        type->first = type->last = NULL;
        type->n_instances = 0;
        type->used = 0;
        type->next_used = NULL;
    }
    while (llg_cov_groups_registered) {
        llg_cov_group_t* group = llg_cov_groups_registered;
        llg_cov_groups_registered = group->next_registered;
        free(group->registry);
        group->registry = NULL;
        group->n_registered = group->registry_capacity = 0;
        group->registered = 0;
        group->strobe_pending = 0;
        group->next_registered = NULL;
    }
}
