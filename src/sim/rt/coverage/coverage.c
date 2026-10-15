// ── Functional coverage (IEEE 1800-2009 19) ─────────────────────────────────
//
// Generated code emits one static `llg_cov_group_t` per covergroup
// declaration site (its coverpoint specifications) and one `llg_cov_type_t`
// per declaration identity; type coverage merges every instance ever created
// for that identity (19.11.3). Instance records are runtime-owned and live
// until llg_rt_cleanup, so counters survive the collection of the covergroup
// object. A model without covergroups never reaches this file's state.
//
// llg_cov_ready turns each coverpoint specification into a layout: the bins
// as counter indices and the key segments a sampled value is looked up in.
// A layout that depends on nothing per instance is built once per group and
// shared; one that reads argument slots, or follows an auto_bin_max other
// than the shared one's, belongs to its instance. Layouts live until
// llg_rt_cleanup.

typedef struct {
    uint64_t* keys;   // bin index + 1; zero marks an empty slot
    uint64_t* counts;
    size_t capacity;  // a power of two, or zero before the first hit
    size_t used;
} llg_cov_sparse_t;

// An inclusive key or bin-index interval.
typedef struct {
    uint64_t lo, hi;
} llg_cov_span_t;

// The bins one value of a segment falls into for one declared bin: the
// element `base` (div == 0), or element base + min(last, (pos + key - lo) /
// div) of an array, where `pos` is the position of the segment's first value
// in the declaration's value list (19.5).
typedef struct {
    uint64_t base;
    uint64_t pos;
    uint64_t div;
    uint64_t last;
    uint32_t decl;
} llg_cov_entry_t;

// A maximal key range hit by the same entries. Ignored and illegal values are
// removed from every bin (19.5.4, 19.5.5): a removed segment counts nothing,
// and its entries are the illegal_bins declarations that cover it.
typedef struct {
    uint64_t lo, hi;
    uint32_t first, count;
    uint8_t removed;
} llg_cov_seg_t;

struct llg_cov_layout {
    struct llg_cov_layout* next_all;
    uint64_t n_bins;        // counters: the automatic bins, then declared elements
    uint64_t n_auto;
    int64_t auto_bin_max;   // the option value the automatic bins follow
    int64_t default_bin;    // the `default` bin's counter, or -1
    int64_t default_decl;   // the `default` (or illegal default) declaration, or -1
    uint8_t default_illegal;
    uint32_t n_segments;
    llg_cov_seg_t* segments;
    llg_cov_entry_t* entries;
    // Coverable bins (19.11.1): automatic bins and `bins` elements that keep
    // a value after removal, as sorted disjoint spans with their ranks.
    uint32_t n_cover;
    llg_cov_span_t* cover;
    uint64_t* cover_rank;
    uint64_t n_coverable;
};

// One pending transition attempt: an automaton state of a set's step, the
// sample its latest start was taken at, and whether it was pending before
// the current sample.
typedef struct {
    uint32_t state;
    uint32_t step;
    uint64_t start;
    uint8_t old;
} llg_cov_attempt_t;

// The count of one sequence of an array transition bin, named by its keys.
typedef struct {
    uint64_t hash;
    uint64_t count;
    uint64_t stamp;  // the sample that last counted it
    size_t keys;     // offset into the history's key pool
    uint32_t bin;
    uint32_t length;  // zero marks an empty slot
} llg_cov_sequence_t;

// The transition history of one coverpoint of one instance (19.5.1): the
// pending attempts, the bins completed by the current sample and the counts.
typedef struct {
    uint64_t serial;         // samples taken; attempts start at serial >= 1
    uint64_t removal_start;  // latest start of a completed ignored/illegal sequence
    uint8_t fresh;           // the next sample begins the history
    uint32_t n_active, n_next, n_done;
    llg_cov_attempt_t* active;
    llg_cov_attempt_t* next;
    uint64_t* state_stamp;  // per state: the sample that last reached it
    uint32_t* state_slot;   // per state: its index in `next`
    uint64_t* set_stamp;    // per set: the sample that last completed it
    uint64_t* set_start;    // per set: the latest start completed at that sample
    uint32_t* done;         // sets completed by the current sample
    uint64_t* counts;       // per bin (scalar bins)
    uint64_t* bin_stamp;    // per bin: the sample that last counted it
    uint64_t* recent;       // the last `window` keys, by serial modulo window
    uint64_t* sequence;     // scratch: one array-bin sequence in order
    llg_cov_sequence_t* sequences;
    size_t sequence_capacity, n_sequences;
    uint64_t* pool;
    size_t pool_used, pool_capacity;
} llg_cov_history_t;

typedef struct {
    int64_t weight;
    int64_t goal;
    int64_t at_least;
    int64_t auto_bin_max;
    uint8_t has_at_least;
    uint8_t has_auto_bin_max;
    uint8_t stopped;
    const llg_cov_layout_t* layout;  // set by llg_cov_ready
    uint64_t* dense;  // n_bins counters when n_bins <= LLG_COV_DENSE_BINS
    llg_cov_sparse_t sparse;
    llg_cov_history_t* history;  // transition bins only
} llg_cov_point_state_t;

// One argument value a bins declaration reads (19.5), as given to `new`.
typedef struct {
    uint64_t raw;  // the low 64 bits, sign-extended for a signed value
    uint8_t signed_value;
    uint8_t unknown;  // any X or Z bit
} llg_cov_slot_value_t;

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
    llg_cov_slot_value_t* slots;
    llg_cov_point_state_t points[];
};

static llg_cov_history_t* llg_cov_history_new(const llg_cov_trans_t* trans);

// Types that own at least one instance, for $get_coverage and cleanup.
static llg_cov_type_t* llg_cov_types_used;
// Groups whose registry holds sampled objects, for cleanup.
static llg_cov_group_t* llg_cov_groups_registered;
// Every layout, and the groups that cache shared layouts, for cleanup.
static llg_cov_layout_t* llg_cov_layouts_all;
static llg_cov_group_t** llg_cov_cached_groups;
static size_t llg_cov_n_cached_groups;
static size_t llg_cov_cached_capacity;

static void* llg_cov_grow(void* data, size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size) llg_fatal_allocation(what, count, size);
    size_t bytes = count * size;
    void* grown = realloc(data, bytes != 0 ? bytes : 1);
    if (!grown) llg_fatal_allocation(what, count, size);
    return grown;
}

// Append `value` to the growable array `data` of `len` elements and `cap`
// capacity.
#define LLG_COV_PUSH(data, len, cap, value, what)                                 \
    do {                                                                          \
        if ((len) == (cap)) {                                                     \
            (cap) = (cap) ? (cap) * 2u : 8u;                                      \
            (data) = llg_cov_grow((data), (cap), sizeof(*(data)), (what));        \
        }                                                                         \
        (data)[(len)++] = (value);                                                \
    } while (0)

static uint64_t llg_cov_mask(const llg_cov_point_t* point) {
    return point->width >= 64 ? UINT64_MAX : ((UINT64_C(1) << point->width) - 1u);
}

// Signed coverpoints order bins by numeric value: flipping the sign bit maps
// the two's-complement range onto ascending unsigned keys.
static uint64_t llg_cov_key(const llg_cov_point_t* point, uint64_t raw) {
    uint64_t key = raw & llg_cov_mask(point);
    if (point->is_signed) key ^= UINT64_C(1) << (point->width - 1u);
    return key;
}

static uint64_t llg_cov_auto_bins(const llg_cov_point_t* point, int64_t auto_bin_max) {
    uint64_t limit = auto_bin_max < 1 ? 1u : (uint64_t)auto_bin_max;
    if (point->width < 64u && (UINT64_C(1) << point->width) <= limit)
        return UINT64_C(1) << point->width;
    return limit;
}

// 19.5.2: 2^M values spread uniformly over N bins; the last bin also takes
// the remainder.
static uint64_t llg_cov_auto_bin(const llg_cov_point_t* point, uint64_t n_bins, uint64_t key) {
    uint64_t span;
    if (point->width >= 64u) {
        span = UINT64_MAX / n_bins;
        if (UINT64_MAX % n_bins == n_bins - 1u) span++;
    } else {
        span = (UINT64_C(1) << point->width) / n_bins;
    }
    uint64_t bin = key / span;
    return bin >= n_bins ? n_bins - 1u : bin;
}

// ── Value resolution (19.5.6) ─────────────────────────────────────────────

// A value of a signed or unsigned operand of at most 64 bits.
typedef struct {
    uint8_t neg;
    uint64_t mag;
} llg_cov_num_t;

static int llg_cov_num_cmp(llg_cov_num_t a, llg_cov_num_t b) {
    if (a.neg != b.neg) return a.neg ? -1 : 1;
    if (a.mag == b.mag) return 0;
    return (a.mag < b.mag) == !a.neg ? -1 : 1;
}

static llg_cov_num_t llg_cov_type_min(const llg_cov_point_t* point) {
    llg_cov_num_t min = {0, 0};
    if (point->is_signed) {
        min.neg = 1;
        min.mag = UINT64_C(1) << (point->width - 1u);
    }
    return min;
}

static llg_cov_num_t llg_cov_type_max(const llg_cov_point_t* point) {
    llg_cov_num_t max = {0, point->is_signed ? (llg_cov_mask(point) >> 1) : llg_cov_mask(point)};
    return max;
}

// The key of a value inside the coverpoint type.
static uint64_t llg_cov_num_key(const llg_cov_point_t* point, llg_cov_num_t value) {
    uint64_t raw = value.neg ? ~value.mag + 1u : value.mag;
    return llg_cov_key(point, raw);
}

static llg_cov_num_t llg_cov_slot_num(const llg_cov_slot_value_t* slot) {
    llg_cov_num_t value = {0, slot->raw};
    if (slot->signed_value && (slot->raw >> 63)) {
        value.neg = 1;
        value.mag = ~slot->raw + 1u;
    }
    return value;
}

static void llg_cov_value_warning(const llg_cov_item_t* item, const llg_cov_bin_t* bin,
                                  const char* what) {
    fprintf(stderr, "llg: warning: %s: bins `%s` %s (IEEE 1800-2009 19.5.6)\n",
            item->site ? item->site : bin->site, bin->name, what);
}

// The key range of one item, resolved by 19.5.6 against the coverpoint
// type; zero when the item does not participate in the bin's values.
static int llg_cov_item_keys(const llg_cov_inst_t* inst, const llg_cov_point_t* point,
                             const llg_cov_bin_t* bin, const llg_cov_item_t* item,
                             llg_cov_span_t* keys) {
    if (item->lo_kind == LLG_COV_BOUND_KEY && item->hi_kind == LLG_COV_BOUND_KEY) {
        keys->lo = item->lo;
        keys->hi = item->hi;
        return 1;
    }
    llg_cov_num_t bounds[2];
    const uint64_t refs[2] = {item->lo, item->hi};
    const uint8_t kinds[2] = {item->lo_kind, item->hi_kind};
    for (int i = 0; i < 2; ++i) {
        if (kinds[i] == LLG_COV_BOUND_MIN) {
            bounds[i] = llg_cov_type_min(point);
        } else if (kinds[i] == LLG_COV_BOUND_MAX) {
            bounds[i] = llg_cov_type_max(point);
        } else {
            const llg_cov_slot_value_t* slot = &inst->slots[refs[i]];
            if (slot->unknown) {
                llg_cov_value_warning(item, bin, "value has X or Z bits and is ignored");
                return 0;
            }
            bounds[i] = llg_cov_slot_num(slot);
        }
    }
    llg_cov_num_t min = llg_cov_type_min(point), max = llg_cov_type_max(point);
    llg_cov_num_t lo = llg_cov_num_cmp(bounds[0], min) < 0 ? min : bounds[0];
    llg_cov_num_t hi = llg_cov_num_cmp(bounds[1], max) > 0 ? max : bounds[1];
    if (llg_cov_num_cmp(bounds[0], bounds[1]) > 0 || llg_cov_num_cmp(lo, hi) > 0) {
        llg_cov_value_warning(item, bin, "value is outside the coverpoint type and is ignored");
        return 0;
    }
    if (llg_cov_num_cmp(lo, bounds[0]) != 0 || llg_cov_num_cmp(hi, bounds[1]) != 0)
        llg_cov_value_warning(item, bin,
                              "range is limited to the values of the coverpoint type");
    keys->lo = llg_cov_num_key(point, lo);
    keys->hi = llg_cov_num_key(point, hi);
    return 1;
}

// ── Layouts ───────────────────────────────────────────────────────────────

static int llg_cov_span_order(const void* a, const void* b) {
    const llg_cov_span_t* x = (const llg_cov_span_t*)a;
    const llg_cov_span_t* y = (const llg_cov_span_t*)b;
    if (x->lo != y->lo) return x->lo < y->lo ? -1 : 1;
    return x->hi < y->hi ? -1 : x->hi > y->hi;
}

// Sort and merge overlapping or adjacent spans in place; returns the count.
static size_t llg_cov_spans_merge(llg_cov_span_t* spans, size_t n) {
    if (n == 0) return 0;
    qsort(spans, n, sizeof(*spans), llg_cov_span_order);
    size_t out = 0;
    for (size_t i = 1; i < n; ++i) {
        if (spans[out].hi == UINT64_MAX || spans[i].lo <= spans[out].hi + 1u) {
            if (spans[i].hi > spans[out].hi) spans[out].hi = spans[i].hi;
        } else {
            spans[++out] = spans[i];
        }
    }
    return out + 1u;
}

// The number of values of a span; zero stands for the full 64-bit range.
static uint64_t llg_cov_span_size(llg_cov_span_t span) {
    return span.hi - span.lo + 1u;
}

// One declared item's keys with the bins they map to, before the segments
// are cut.
typedef struct {
    uint64_t lo, hi;
    llg_cov_entry_t entry;
    uint8_t role;  // LLG_COV_BIN_BINS, _IGNORE or _ILLEGAL
} llg_cov_piece_t;

static int llg_cov_piece_order(const void* a, const void* b) {
    const llg_cov_piece_t* x = (const llg_cov_piece_t*)a;
    const llg_cov_piece_t* y = (const llg_cov_piece_t*)b;
    return x->lo < y->lo ? -1 : x->lo > y->lo;
}

static int llg_cov_u64_order(const void* a, const void* b) {
    uint64_t x = *(const uint64_t*)a, y = *(const uint64_t*)b;
    return x < y ? -1 : x > y;
}

// Bin counts beyond 2^64 - 1 cannot be indexed.
static uint64_t llg_cov_add_bins(uint64_t total, uint64_t more) {
    if (more == 0 || more > UINT64_MAX - total) llg_fatal_allocation("coverage bins", 2, SIZE_MAX);
    return total + more;
}

// The element range one entry covers over the keys lo..hi of a segment.
static llg_cov_span_t llg_cov_entry_bins(const llg_cov_entry_t* entry, uint64_t lo, uint64_t hi) {
    llg_cov_span_t bins = {entry->base, entry->base};
    if (entry->div) {
        uint64_t first = entry->pos / entry->div, last = (entry->pos + (hi - lo)) / entry->div;
        bins.lo = entry->base + (first < entry->last ? first : entry->last);
        bins.hi = entry->base + (last < entry->last ? last : entry->last);
    }
    return bins;
}

// The `[N]` count of a bins array; zero when it is not a positive value.
static uint64_t llg_cov_bin_count(const llg_cov_inst_t* inst, const llg_cov_bin_t* bin) {
    if (!bin->count_is_slot) return bin->count;
    const llg_cov_slot_value_t* slot = &inst->slots[bin->count];
    llg_cov_num_t value = llg_cov_slot_num(slot);
    if (slot->unknown || value.neg || value.mag == 0) {
        fprintf(stderr,
                "llg: runtime error: bins `%s` at %s: the number of bins is not a positive "
                "value, so the declaration has no bins\n",
                bin->name, bin->site);
        llg_rt_mark_failed();
        return 0;
    }
    return value.mag;
}

// Coverable spans of the automatic bins whose keys lo..hi are not removed.
static size_t llg_cov_auto_spans(const llg_cov_point_t* point, const llg_cov_layout_t* layout,
                                 uint64_t lo, uint64_t hi, llg_cov_span_t** spans, size_t n,
                                 size_t* cap) {
    if (point->kind == LLG_COV_POINT_AUTO) {
        llg_cov_span_t bins = {llg_cov_auto_bin(point, layout->n_auto, lo),
                               llg_cov_auto_bin(point, layout->n_auto, hi)};
        LLG_COV_PUSH(*spans, n, *cap, bins, "coverage bins");
        return n;
    }
    uint32_t first = 0, end = point->n_keys;
    while (first < end) {
        uint32_t mid = first + (end - first) / 2u;
        if (point->keys[mid] < lo) first = mid + 1u;
        else end = mid;
    }
    uint32_t last = first;
    while (last < point->n_keys && point->keys[last] <= hi) last++;
    if (last > first) {
        llg_cov_span_t bins = {first, last - 1u};
        LLG_COV_PUSH(*spans, n, *cap, bins, "coverage bins");
    }
    return n;
}

static llg_cov_layout_t* llg_cov_build_layout(const llg_cov_inst_t* inst, uint32_t index,
                                              int64_t auto_bin_max) {
    const llg_cov_point_t* point = &inst->group->points[index];
    llg_cov_layout_t* layout =
        (llg_cov_layout_t*)llg_checked_calloc(1, sizeof(*layout), "coverage layout");
    layout->default_bin = -1;
    layout->default_decl = -1;
    layout->auto_bin_max = auto_bin_max;
    if (point->kind == LLG_COV_POINT_AUTO) layout->n_auto = llg_cov_auto_bins(point, auto_bin_max);
    else if (point->kind == LLG_COV_POINT_ENUM) layout->n_auto = point->n_keys;
    uint64_t n_bins = layout->n_auto;

    llg_cov_piece_t* pieces = NULL;
    size_t n_pieces = 0, pieces_cap = 0;
    llg_cov_span_t* spans = NULL;
    size_t spans_cap = 0;
    for (uint32_t d = 0; d < point->n_bins; ++d) {
        const llg_cov_bin_t* bin = &point->bins[d];
        if (bin->kind == LLG_COV_BIN_DEFAULT || bin->kind == LLG_COV_BIN_ILLEGAL_DEFAULT) {
            layout->default_decl = d;
            layout->default_illegal = bin->kind == LLG_COV_BIN_ILLEGAL_DEFAULT;
            if (bin->kind == LLG_COV_BIN_DEFAULT) {
                layout->default_bin = (int64_t)n_bins;
                n_bins = llg_cov_add_bins(n_bins, 1);
            }
            continue;
        }
        size_t n_spans = 0;
        for (uint32_t i = 0; i < bin->n_items; ++i) {
            llg_cov_span_t keys;
            if (llg_cov_item_keys(inst, point, bin, &point->items[bin->first_item + i], &keys))
                LLG_COV_PUSH(spans, n_spans, spans_cap, keys, "coverage bin values");
        }
        llg_cov_entry_t entry = {n_bins, 0, 0, 0, d};
        if (bin->kind == LLG_COV_BIN_BINS && bin->shape == LLG_COV_SHAPE_COUNT) {
            // 19.5: the values, in order and with duplicates, are dealt to
            // the N bins n at a time; the last bin takes the remainder. With
            // fewer values than bins, each value gets its own bin and the
            // remaining bins stay empty (decision S45-D1).
            uint64_t count = llg_cov_bin_count(inst, bin);
            if (!count) continue;
            uint64_t values = 0;
            for (size_t i = 0; i < n_spans; ++i) {
                uint64_t size = llg_cov_span_size(spans[i]);
                if (size == 0 || size > UINT64_MAX - values)
                    llg_fatal_allocation("coverage bin values", 2, SIZE_MAX);
                values += size;
            }
            entry.div = values >= count ? values / count : 1u;
            entry.last = count - 1u;
            for (size_t i = 0; i < n_spans; ++i) {
                llg_cov_piece_t piece = {spans[i].lo, spans[i].hi, entry, LLG_COV_BIN_BINS};
                LLG_COV_PUSH(pieces, n_pieces, pieces_cap, piece, "coverage bin values");
                entry.pos += llg_cov_span_size(spans[i]);
            }
            n_bins = llg_cov_add_bins(n_bins, count);
            continue;
        }
        // A value listed twice is still one value of the bin.
        n_spans = llg_cov_spans_merge(spans, n_spans);
        if (bin->kind == LLG_COV_BIN_BINS && bin->shape == LLG_COV_SHAPE_PER_VALUE) {
            entry.div = 1;
            entry.last = UINT64_MAX;
            for (size_t i = 0; i < n_spans; ++i) {
                llg_cov_piece_t piece = {spans[i].lo, spans[i].hi, entry, LLG_COV_BIN_BINS};
                LLG_COV_PUSH(pieces, n_pieces, pieces_cap, piece, "coverage bin values");
                uint64_t size = llg_cov_span_size(spans[i]);
                if (size == 0 || size > UINT64_MAX - entry.pos)
                    llg_fatal_allocation("coverage bins", 2, SIZE_MAX);
                entry.pos += size;
            }
            if (entry.pos) n_bins = llg_cov_add_bins(n_bins, entry.pos);
            continue;
        }
        for (size_t i = 0; i < n_spans; ++i) {
            llg_cov_piece_t piece = {spans[i].lo, spans[i].hi, entry, bin->kind};
            LLG_COV_PUSH(pieces, n_pieces, pieces_cap, piece, "coverage bin values");
        }
        if (bin->kind == LLG_COV_BIN_BINS) n_bins = llg_cov_add_bins(n_bins, 1);
    }
    layout->n_bins = n_bins;

    // Cut the pieces into maximal segments with one set of entries.
    if (n_pieces) {
        qsort(pieces, n_pieces, sizeof(*pieces), llg_cov_piece_order);
        uint64_t* bounds = (uint64_t*)llg_checked_malloc(n_pieces * 2u, sizeof(uint64_t),
                                                         "coverage segments");
        size_t n_bounds = 0;
        for (size_t i = 0; i < n_pieces; ++i) {
            bounds[n_bounds++] = pieces[i].lo;
            if (pieces[i].hi != UINT64_MAX) bounds[n_bounds++] = pieces[i].hi + 1u;
        }
        qsort(bounds, n_bounds, sizeof(uint64_t), llg_cov_u64_order);
        size_t unique = 0;
        for (size_t i = 0; i < n_bounds; ++i)
            if (!unique || bounds[unique - 1u] != bounds[i]) bounds[unique++] = bounds[i];
        size_t* active = (size_t*)llg_checked_malloc(n_pieces, sizeof(size_t), "coverage segments");
        size_t n_active = 0, next = 0;
        size_t n_segments = 0, segments_cap = 0, n_entries = 0, entries_cap = 0;
        for (size_t k = 0; k < unique; ++k) {
            uint64_t lo = bounds[k];
            size_t kept = 0;
            for (size_t i = 0; i < n_active; ++i)
                if (pieces[active[i]].hi >= lo) active[kept++] = active[i];
            n_active = kept;
            while (next < n_pieces && pieces[next].lo == lo) active[n_active++] = next++;
            if (!n_active) continue;
            uint64_t hi = k + 1u < unique ? bounds[k + 1u] - 1u : UINT64_MAX;
            llg_cov_seg_t segment = {lo, hi, (uint32_t)n_entries, 0, 0};
            for (size_t i = 0; i < n_active; ++i)
                if (pieces[active[i]].role != LLG_COV_BIN_BINS) segment.removed = 1;
            for (size_t i = 0; i < n_active; ++i) {
                const llg_cov_piece_t* piece = &pieces[active[i]];
                if (segment.removed ? piece->role != LLG_COV_BIN_ILLEGAL
                                    : piece->role != LLG_COV_BIN_BINS)
                    continue;
                llg_cov_entry_t entry = piece->entry;
                entry.pos += lo - piece->lo;
                if (n_entries >= UINT32_MAX) llg_fatal_allocation("coverage segments", 2, SIZE_MAX);
                LLG_COV_PUSH(layout->entries, n_entries, entries_cap, entry, "coverage segments");
                segment.count++;
            }
            if (n_segments >= UINT32_MAX) llg_fatal_allocation("coverage segments", 2, SIZE_MAX);
            LLG_COV_PUSH(layout->segments, n_segments, segments_cap, segment, "coverage segments");
        }
        layout->n_segments = (uint32_t)n_segments;
        free(active);
        free(bounds);
    }

    // Coverable bins: declared elements with a value left after removal, and
    // automatic bins with a value outside every removed segment (19.11.1).
    size_t n_cover = 0;
    for (uint32_t s = 0; s < layout->n_segments; ++s) {
        const llg_cov_seg_t* segment = &layout->segments[s];
        if (segment->removed) continue;
        for (uint32_t e = 0; e < segment->count; ++e) {
            llg_cov_span_t bins =
                llg_cov_entry_bins(&layout->entries[segment->first + e], segment->lo, segment->hi);
            LLG_COV_PUSH(spans, n_cover, spans_cap, bins, "coverage bins");
        }
    }
    if (point->kind != LLG_COV_POINT_EXPLICIT) {
        uint64_t key = 0, max = llg_cov_mask(point);
        int exhausted = 0;
        for (uint32_t s = 0; s < layout->n_segments && !exhausted; ++s) {
            const llg_cov_seg_t* segment = &layout->segments[s];
            if (!segment->removed) continue;
            if (segment->lo > key)
                n_cover = llg_cov_auto_spans(point, layout, key, segment->lo - 1u, &spans, n_cover,
                                             &spans_cap);
            if (segment->hi >= max) exhausted = 1;
            else key = segment->hi + 1u;
        }
        if (!exhausted)
            n_cover = llg_cov_auto_spans(point, layout, key, max, &spans, n_cover, &spans_cap);
    }
    n_cover = llg_cov_spans_merge(spans, n_cover);
    if (n_cover > UINT32_MAX) llg_fatal_allocation("coverage bins", n_cover, sizeof(*spans));
    layout->n_cover = (uint32_t)n_cover;
    layout->cover = (llg_cov_span_t*)llg_checked_malloc(n_cover, sizeof(*spans), "coverage bins");
    layout->cover_rank =
        (uint64_t*)llg_checked_malloc(n_cover, sizeof(uint64_t), "coverage bins");
    for (size_t i = 0; i < n_cover; ++i) {
        layout->cover[i] = spans[i];
        layout->cover_rank[i] = layout->n_coverable;
        layout->n_coverable += spans[i].hi - spans[i].lo + 1u;
    }
    free(spans);
    free(pieces);
    layout->next_all = llg_cov_layouts_all;
    llg_cov_layouts_all = layout;
    return layout;
}

// The layout of one point of a new instance: the group's shared layout when
// the bins depend on nothing per instance.
static const llg_cov_layout_t* llg_cov_layout_for(llg_cov_inst_t* inst, uint32_t index) {
    llg_cov_group_t* group = inst->group;
    const llg_cov_point_t* point = &group->points[index];
    int64_t auto_bin_max = inst->points[index].auto_bin_max;
    if (point->dynamic) return llg_cov_build_layout(inst, index, auto_bin_max);
    if (!group->layouts) {
        group->layouts = (llg_cov_layout_t**)llg_checked_calloc(group->n_points, sizeof(void*),
                                                                "coverage layouts");
        LLG_COV_PUSH(llg_cov_cached_groups, llg_cov_n_cached_groups, llg_cov_cached_capacity,
                     group, "coverage layouts");
    }
    llg_cov_layout_t* shared = group->layouts[index];
    if (shared && (point->kind != LLG_COV_POINT_AUTO || shared->auto_bin_max == auto_bin_max))
        return shared;
    llg_cov_layout_t* layout = llg_cov_build_layout(inst, index, auto_bin_max);
    if (!shared) group->layouts[index] = layout;
    return layout;
}

// ── Counters ──────────────────────────────────────────────────────────────

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

static void llg_cov_count(llg_cov_point_state_t* state, uint64_t bin) {
    if (state->dense) {
        if (state->dense[bin] != UINT64_MAX) state->dense[bin]++;
    } else {
        llg_cov_sparse_add(&state->sparse, bin);
    }
}

// The rank of `bin` among the coverable bins, or -1 when it is not one.
static int64_t llg_cov_cover_rank(const llg_cov_layout_t* layout, uint64_t bin) {
    uint32_t lo = 0, hi = layout->n_cover;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        if (layout->cover[mid].hi < bin) lo = mid + 1u;
        else hi = mid;
    }
    if (lo == layout->n_cover || layout->cover[lo].lo > bin) return -1;
    return (int64_t)(layout->cover_rank[lo] + (bin - layout->cover[lo].lo));
}

// ── Instances ─────────────────────────────────────────────────────────────

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
    if (group->n_slots)
        inst->slots = (llg_cov_slot_value_t*)llg_checked_calloc(
            group->n_slots, sizeof(llg_cov_slot_value_t), "covergroup arguments");
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

void llg_cov_slot(void* record, uint32_t slot, sv4_t value) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || slot >= inst->group->n_slots) return;
    llg_cov_slot_value_t* target = &inst->slots[slot];
    uint32_t width = llg_sv4_width(value);
    uint64_t bits = sv4_to_u64(value);
    target->signed_value = (uint8_t)(llg_sv4_signed(value) != 0);
    target->unknown = (uint8_t)(sv4_is_unknown(value) != 0);
    // Generated code passes at most 64 bits; the value keeps its own
    // signedness for 19.5.6.
    if (width > 0u && width < 64u) {
        bits &= (UINT64_C(1) << width) - 1u;
        if (target->signed_value && ((bits >> (width - 1u)) & 1u))
            bits |= ~((UINT64_C(1) << width) - 1u);
    }
    target->raw = bits;
}

// Group-level at_least and auto_bin_max are coverpoint defaults (19.7); bin
// layouts and counters are built once the options and arguments are final.
void llg_cov_ready(void* record) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || inst->ready) return;
    inst->ready = 1;
    for (uint32_t i = 0; i < inst->group->n_points; ++i) {
        llg_cov_point_state_t* state = &inst->points[i];
        if (!state->has_at_least) state->at_least = inst->at_least;
        if (!state->has_auto_bin_max) state->auto_bin_max = inst->auto_bin_max;
        state->layout = llg_cov_layout_for(inst, i);
        if (state->layout->n_bins <= LLG_COV_DENSE_BINS)
            state->dense = (uint64_t*)llg_checked_calloc((size_t)state->layout->n_bins,
                                                         sizeof(uint64_t), "coverage bins");
        const llg_cov_trans_t* trans = inst->group->points[i].trans;
        if (trans) state->history = llg_cov_history_new(trans);
    }
}

static int llg_cov_guard(const llg_cov_point_t* point, uint32_t decl, const uint8_t* guards) {
    int32_t guard = point->bins[decl].guard;
    return guard < 0 || !guards || guards[guard];
}

// A key as the signed or unsigned value it stands for.
static void llg_cov_format_key(const llg_cov_point_t* point, uint64_t key, char* text,
                               size_t size) {
    uint64_t raw = key;
    if (point->is_signed) raw ^= UINT64_C(1) << (point->width - 1u);
    if (point->is_signed && ((raw >> (point->width - 1u)) & 1u)) {
        uint64_t magnitude = (~raw & llg_cov_mask(point)) + 1u;
        snprintf(text, size, "-%llu", (unsigned long long)magnitude);
    } else {
        snprintf(text, size, "%llu", (unsigned long long)raw);
    }
}

// 19.5.5: an illegal value is a run-time error even when another bin also
// holds it; the simulation continues and the value counts in no bin
// (decision S45-D3).
static void llg_cov_illegal(const llg_cov_inst_t* inst, const llg_cov_point_t* point,
                            uint32_t decl, uint64_t key) {
    const llg_cov_bin_t* bin = &point->bins[decl];
    char value[32];
    llg_cov_format_key(point, key, value, sizeof value);
    fprintf(stderr,
            "llg: runtime error: covergroup `%s` coverpoint `%s` sampled %s, a value of "
            "illegal_bins `%s` at %s (IEEE 1800-2009 19.5.5)\n",
            inst->group->name, point->name, value, bin->name, bin->site);
    llg_rt_mark_failed();
}

static const llg_cov_seg_t* llg_cov_find(const llg_cov_layout_t* layout, uint64_t key) {
    uint32_t lo = 0, hi = layout->n_segments;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        if (layout->segments[mid].hi < key) lo = mid + 1u;
        else hi = mid;
    }
    if (lo == layout->n_segments || layout->segments[lo].lo > key) return NULL;
    return &layout->segments[lo];
}

// ── Transition bins (19.5.1) ─────────────────────────────────────────────

static void llg_cov_history_reset(llg_cov_history_t* history) {
    if (!history) return;
    history->n_active = 0;
    history->fresh = 1;
}

static int llg_cov_accepts(const llg_cov_trans_t* trans, uint32_t step, uint32_t cls) {
    if (cls == UINT32_MAX) return 0;
    uint64_t word = trans->accept[(size_t)step * trans->words + cls / 64u];
    return (int)((word >> (cls % 64u)) & 1u);
}

// The class of a key: the last bound at or below it.
static uint32_t llg_cov_class(const llg_cov_trans_t* trans, uint64_t key) {
    uint32_t lo = 0, hi = trans->n_classes;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        if (trans->bounds[mid] <= key) lo = mid + 1u;
        else hi = mid;
    }
    return lo - 1u;
}

// Keep one attempt per state for the next sample: the latest start gives
// the shortest sequence, which every longer one contains (19.5.4 removal).
static void llg_cov_attempt_put(llg_cov_history_t* history, uint32_t step, uint32_t state,
                                uint64_t start, uint8_t old) {
    if (history->state_stamp[state] == history->serial) {
        llg_cov_attempt_t* attempt = &history->next[history->state_slot[state]];
        if (start > attempt->start) attempt->start = start;
        attempt->old |= old;
        return;
    }
    history->state_stamp[state] = history->serial;
    history->state_slot[state] = history->n_next;
    history->next[history->n_next++] = (llg_cov_attempt_t){state, step, start, old};
}

static void llg_cov_step_done(const llg_cov_trans_t* trans, llg_cov_history_t* history,
                              uint32_t step, uint64_t start, uint8_t old) {
    const llg_cov_step_t* spec = &trans->steps[step];
    if (!spec->last) {
        llg_cov_attempt_put(history, step + 1u, trans->steps[step + 1u].state, start, old);
        return;
    }
    uint32_t set = spec->set;
    if (history->set_stamp[set] == history->serial) {
        if (start > history->set_start[set]) history->set_start[set] = start;
        return;
    }
    history->set_stamp[set] = history->serial;
    history->set_start[set] = start;
    history->done[history->n_done++] = set;
}

// Advance one thread at `step` with repetition count `count` by a sample of
// class `cls` (UINT32_MAX for a value with X or Z bits, which no item holds).
// Mirrors `Tables::consume` in the transition lowering.
static void llg_cov_consume(const llg_cov_trans_t* trans, llg_cov_history_t* history,
                            uint32_t step, uint32_t count, uint32_t cls, uint64_t start,
                            uint8_t old) {
    const llg_cov_step_t* spec = &trans->steps[step];
    int accepted = llg_cov_accepts(trans, step, cls);
    switch (spec->repeat) {
    case LLG_COV_REPEAT_CONSECUTIVE:
        if (!accepted) return;
        if (count + 1u < spec->hi)
            llg_cov_attempt_put(history, step, spec->state + count + 1u, start, old);
        if (count + 1u >= spec->lo) llg_cov_step_done(trans, history, step, start, old);
        return;
    case LLG_COV_REPEAT_GOTO:
        // Samples between occurrences do not hold the item.
        if (!accepted) {
            llg_cov_attempt_put(history, step, spec->state + count, start, old);
            return;
        }
        if (count + 1u < spec->hi)
            llg_cov_attempt_put(history, step, spec->state + count + 1u, start, old);
        if (count + 1u >= spec->lo) llg_cov_step_done(trans, history, step, start, old);
        return;
    default: {
        if (count < spec->lo) {
            llg_cov_attempt_put(history, step, spec->state + count + (uint32_t)accepted, start,
                                old);
            return;
        }
        // Enough repetitions: the next item may occur at this sample, which
        // ends the gap (decision S45-D8).
        int begins = llg_cov_accepts(trans, step + 1u, cls);
        if (begins) llg_cov_consume(trans, history, step + 1u, 0u, cls, start, old);
        if (accepted) {
            if (count < spec->hi)
                llg_cov_attempt_put(history, step, spec->state + count + 1u, start, old);
        } else if (!begins) {
            llg_cov_attempt_put(history, step, spec->state + count, start, old);
        }
        return;
    }
    }
}

static llg_cov_history_t* llg_cov_history_new(const llg_cov_trans_t* trans) {
    llg_cov_history_t* history =
        (llg_cov_history_t*)llg_checked_calloc(1, sizeof(*history), "coverage transitions");
    size_t states = trans->n_states ? trans->n_states : 1u;
    size_t sets = trans->n_sets ? trans->n_sets : 1u;
    size_t bins = trans->n_bins ? trans->n_bins : 1u;
    history->active = (llg_cov_attempt_t*)llg_checked_calloc(states, sizeof(llg_cov_attempt_t),
                                                             "coverage transitions");
    history->next = (llg_cov_attempt_t*)llg_checked_calloc(states, sizeof(llg_cov_attempt_t),
                                                           "coverage transitions");
    history->state_stamp =
        (uint64_t*)llg_checked_calloc(states, sizeof(uint64_t), "coverage transitions");
    history->state_slot =
        (uint32_t*)llg_checked_calloc(states, sizeof(uint32_t), "coverage transitions");
    history->set_stamp = (uint64_t*)llg_checked_calloc(sets, sizeof(uint64_t), "coverage transitions");
    history->set_start = (uint64_t*)llg_checked_calloc(sets, sizeof(uint64_t), "coverage transitions");
    history->done = (uint32_t*)llg_checked_calloc(sets, sizeof(uint32_t), "coverage transitions");
    history->counts = (uint64_t*)llg_checked_calloc(bins, sizeof(uint64_t), "coverage transitions");
    history->bin_stamp = (uint64_t*)llg_checked_calloc(bins, sizeof(uint64_t), "coverage transitions");
    if (trans->window) {
        history->recent =
            (uint64_t*)llg_checked_calloc(trans->window, sizeof(uint64_t), "coverage transitions");
        history->sequence =
            (uint64_t*)llg_checked_calloc(trans->window, sizeof(uint64_t), "coverage transitions");
    }
    history->fresh = 1;
    return history;
}

static void llg_cov_history_free(llg_cov_history_t* history) {
    if (!history) return;
    free(history->active);
    free(history->next);
    free(history->state_stamp);
    free(history->state_slot);
    free(history->set_stamp);
    free(history->set_start);
    free(history->done);
    free(history->counts);
    free(history->bin_stamp);
    free(history->recent);
    free(history->sequence);
    free(history->sequences);
    free(history->pool);
    free(history);
}

static uint64_t llg_cov_sequence_hash(uint32_t bin, const uint64_t* keys, uint32_t length) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325) ^ bin;
    for (uint32_t i = 0; i < length; ++i) {
        hash ^= keys[i];
        hash *= UINT64_C(0x100000001b3);
        hash ^= hash >> 29;
    }
    return hash ^ length;
}

static llg_cov_sequence_t* llg_cov_sequence_slot(llg_cov_sequence_t* table, size_t capacity,
                                                 const uint64_t* pool, uint64_t hash, uint32_t bin,
                                                 const uint64_t* keys, uint32_t length) {
    size_t slot = (size_t)hash & (capacity - 1u);
    for (;;) {
        llg_cov_sequence_t* entry = &table[slot];
        if (!entry->length) return entry;
        if (entry->hash == hash && entry->bin == bin && entry->length == length &&
            memcmp(pool + entry->keys, keys, length * sizeof(uint64_t)) == 0)
            return entry;
        slot = (slot + 1u) & (capacity - 1u);
    }
}

// Count one sequence of an array bin once per sample; the table holds only
// sequences that occurred.
static void llg_cov_sequence_count(llg_cov_history_t* history, uint32_t bin,
                                   const uint64_t* keys, uint32_t length) {
    if ((history->n_sequences + 1u) * 2u > history->sequence_capacity) {
        size_t capacity = history->sequence_capacity ? history->sequence_capacity * 2u : 16u;
        llg_cov_sequence_t* table = (llg_cov_sequence_t*)llg_checked_calloc(
            capacity, sizeof(llg_cov_sequence_t), "coverage transition bins");
        for (size_t i = 0; i < history->sequence_capacity; ++i) {
            llg_cov_sequence_t* entry = &history->sequences[i];
            if (!entry->length) continue;
            *llg_cov_sequence_slot(table, capacity, history->pool, entry->hash, entry->bin,
                                   history->pool + entry->keys, entry->length) = *entry;
        }
        free(history->sequences);
        history->sequences = table;
        history->sequence_capacity = capacity;
    }
    uint64_t hash = llg_cov_sequence_hash(bin, keys, length);
    llg_cov_sequence_t* entry = llg_cov_sequence_slot(history->sequences, history->sequence_capacity,
                                                      history->pool, hash, bin, keys, length);
    if (!entry->length) {
        if (history->pool_used + length > history->pool_capacity) {
            size_t capacity = history->pool_capacity ? history->pool_capacity * 2u : 64u;
            while (capacity < history->pool_used + length) capacity *= 2u;
            history->pool = llg_cov_grow(history->pool, capacity, sizeof(uint64_t),
                                         "coverage transition bins");
            history->pool_capacity = capacity;
        }
        memcpy(history->pool + history->pool_used, keys, length * sizeof(uint64_t));
        *entry = (llg_cov_sequence_t){hash, 0u, 0u, history->pool_used, bin, length};
        history->pool_used += length;
        history->n_sequences++;
    }
    if (entry->stamp == history->serial) return;
    entry->stamp = history->serial;
    if (entry->count != UINT64_MAX) entry->count++;
}

static void llg_cov_illegal_transition(const llg_cov_inst_t* inst, const llg_cov_point_t* point,
                                       const llg_cov_tbin_t* bin, int known, uint64_t key) {
    char value[32];
    if (known) llg_cov_format_key(point, key, value, sizeof value);
    else snprintf(value, sizeof value, "a value with X or Z bits");
    const char* what = bin->kind == LLG_COV_BIN_ILLEGAL_DEFAULT
                           ? "ending a transition no other bin holds, which is in"
                           : "completing a transition of";
    fprintf(stderr,
            "llg: runtime error: covergroup `%s` coverpoint `%s` sampled %s, %s illegal_bins "
            "`%s` at %s (IEEE 1800-2009 19.5.5)\n",
            inst->group->name, point->name, value, what, bin->name, bin->site);
    llg_rt_mark_failed();
}

// One sample of a coverpoint with transition bins. Every pending attempt
// advances, attempts start at the sets whose first step holds the value,
// and each completed bin counts at most once per sample (19.5.1). A covered
// sequence that contains a completed ignored or illegal sequence counts in
// no bin (19.5.4, 19.5.5).
static void llg_cov_transitions(const llg_cov_inst_t* inst, llg_cov_history_t* history,
                                const llg_cov_point_t* point, int known, uint64_t key,
                                const uint8_t* guards) {
    const llg_cov_trans_t* trans = point->trans;
    history->serial++;
    uint64_t serial = history->serial;
    uint32_t cls = known ? llg_cov_class(trans, key) : UINT32_MAX;
    history->n_next = 0;
    history->n_done = 0;
    for (uint32_t i = 0; i < history->n_active; ++i) {
        llg_cov_attempt_t attempt = history->active[i];
        llg_cov_consume(trans, history, attempt.step, attempt.state - trans->steps[attempt.step].state,
                        cls, attempt.start, 1u);
    }
    // Leading gaps of goto and nonconsecutive first steps start at the
    // first occurrence: an attempt begins only where its first step holds
    // the value.
    if (cls != UINT32_MAX) {
        for (uint32_t i = trans->start_index[cls]; i < trans->start_index[cls + 1u]; ++i) {
            const llg_cov_tset_t* set = &trans->sets[trans->start_sets[i]];
            llg_cov_consume(trans, history, set->first_step, 0u, cls, serial, 0u);
        }
    }
    uint8_t pending = 0;
    for (uint32_t i = 0; i < history->n_next; ++i) pending |= history->next[i].old;
    llg_cov_attempt_t* swap = history->active;
    history->active = history->next;
    history->next = swap;
    history->n_active = history->n_next;
    if (trans->window) history->recent[serial % trans->window] = key;

    for (uint32_t i = 0; i < history->n_done; ++i) {
        uint32_t set = history->done[i];
        const llg_cov_tbin_t* bin = &trans->bins[trans->sets[set].bin];
        if (bin->kind != LLG_COV_BIN_IGNORE && bin->kind != LLG_COV_BIN_ILLEGAL) continue;
        if (history->set_start[set] > history->removal_start)
            history->removal_start = history->set_start[set];
        uint32_t index = trans->sets[set].bin;
        if (bin->kind == LLG_COV_BIN_ILLEGAL && history->bin_stamp[index] != serial &&
            (bin->guard < 0 || !guards || guards[bin->guard])) {
            history->bin_stamp[index] = serial;
            llg_cov_illegal_transition(inst, point, bin, known, key);
        }
    }
    for (uint32_t i = 0; i < history->n_done; ++i) {
        uint32_t set = history->done[i];
        uint32_t index = trans->sets[set].bin;
        const llg_cov_tbin_t* bin = &trans->bins[index];
        if (bin->kind != LLG_COV_BIN_BINS) continue;
        if (history->set_start[set] <= history->removal_start) continue;
        if (bin->guard >= 0 && guards && !guards[bin->guard]) continue;
        if (!bin->array) {
            if (history->bin_stamp[index] == serial) continue;
            history->bin_stamp[index] = serial;
            if (history->counts[index] != UINT64_MAX) history->counts[index]++;
            continue;
        }
        uint32_t length = trans->sets[set].n_steps;
        for (uint32_t k = 0; k < length; ++k)
            history->sequence[k] = history->recent[(serial - length + 1u + k) % trans->window];
        llg_cov_sequence_count(history, index, history->sequence, length);
    }
    // 19.5.1: the default sequence counts when no other transition bin
    // completes and no attempt pending before this sample still is
    // (decision S45-D10).
    if (trans->default_bin >= 0 && known && !history->fresh && !history->n_done && !pending) {
        const llg_cov_tbin_t* bin = &trans->bins[trans->default_bin];
        if (bin->guard < 0 || !guards || guards[bin->guard]) {
            if (bin->kind == LLG_COV_BIN_ILLEGAL_DEFAULT) {
                llg_cov_illegal_transition(inst, point, bin, known, key);
            } else if (history->counts[trans->default_bin] != UINT64_MAX) {
                history->counts[trans->default_bin]++;
            }
        }
    }
    history->fresh = 0;
}

void llg_cov_skip(void* record, uint32_t point_index) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || point_index >= inst->group->n_points) return;
    llg_cov_history_reset(inst->points[point_index].history);
}

void llg_cov_hit(void* record, uint32_t point_index, const sv4_t* value, const uint8_t* guards) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || inst->stopped || point_index >= inst->group->n_points) return;
    llg_cov_point_state_t* state = &inst->points[point_index];
    if (state->stopped || !state->layout) return;
    const llg_cov_point_t* point = &inst->group->points[point_index];
    int known = !sv4_is_unknown(*value);
    uint64_t key = known ? llg_cov_key(point, sv4_to_u64(*value)) : 0u;
    if (state->history) llg_cov_transitions(inst, state->history, point, known, key, guards);
    // Bins only hold 2-state values (19.5.2, 19.5.3, 19.5.6).
    if (!known) return;
    const llg_cov_layout_t* layout = state->layout;
    if (layout->n_segments) {
        const llg_cov_seg_t* segment = llg_cov_find(layout, key);
        if (segment && segment->removed) {
            for (uint32_t e = 0; e < segment->count; ++e) {
                const llg_cov_entry_t* entry = &layout->entries[segment->first + e];
                if (llg_cov_guard(point, entry->decl, guards)) {
                    llg_cov_illegal(inst, point, entry->decl, key);
                    break;
                }
            }
            return;
        }
        if (segment) {
            // A value in several overlapping bins counts in each of them.
            for (uint32_t e = 0; e < segment->count; ++e) {
                const llg_cov_entry_t* entry = &layout->entries[segment->first + e];
                if (!llg_cov_guard(point, entry->decl, guards)) continue;
                uint64_t bin = entry->base;
                if (entry->div) {
                    uint64_t element = (entry->pos + (key - segment->lo)) / entry->div;
                    bin += element < entry->last ? element : entry->last;
                }
                llg_cov_count(state, bin);
            }
            return;
        }
    }
    switch (point->kind) {
    case LLG_COV_POINT_AUTO:
        llg_cov_count(state, llg_cov_auto_bin(point, layout->n_auto, key));
        return;
    case LLG_COV_POINT_ENUM: {
        uint32_t lo = 0, hi = point->n_keys;
        while (lo < hi) {
            uint32_t mid = lo + (hi - lo) / 2u;
            if (point->keys[mid] < key) lo = mid + 1u;
            else hi = mid;
        }
        if (lo < point->n_keys && point->keys[lo] == key) llg_cov_count(state, lo);
        return;
    }
    default:
        // The default bin catches values no other bin holds (19.5).
        if (layout->default_decl < 0 ||
            !llg_cov_guard(point, (uint32_t)layout->default_decl, guards))
            return;
        if (layout->default_illegal)
            llg_cov_illegal(inst, point, (uint32_t)layout->default_decl, key);
        else llg_cov_count(state, (uint64_t)layout->default_bin);
        return;
    }
}

void llg_cov_control(void* record, int32_t point, int start) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst) return;
    // A stop ends the transition sequences in progress (decision S45-D9).
    if (point < 0) {
        inst->stopped = (uint8_t)!start;
        if (!start)
            for (uint32_t i = 0; i < inst->group->n_points; ++i)
                llg_cov_history_reset(inst->points[i].history);
    } else if ((uint32_t)point < inst->group->n_points) {
        inst->points[point].stopped = (uint8_t)!start;
        if (!start) llg_cov_history_reset(inst->points[point].history);
    }
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

// ── Coverage computation (19.11) ──────────────────────────────────────────

// Covered and defined bins of one coverpoint (19.11.1): only coverable bins
// count; empty, ignored, illegal and default bins do not.
static void llg_cov_value_bins(const llg_cov_inst_t* inst, uint32_t index, uint64_t* covered,
                               uint64_t* defined) {
    const llg_cov_point_state_t* state = &inst->points[index];
    uint64_t at_least = state->at_least < 0 ? 0u : (uint64_t)state->at_least;
    *covered = 0;
    *defined = 0;
    const llg_cov_layout_t* layout = state->layout;
    if (!layout) return;
    *defined = layout->n_coverable;
    if (at_least == 0) {
        *covered = *defined;
        return;
    }
    if (state->dense) {
        for (uint32_t c = 0; c < layout->n_cover; ++c)
            for (uint64_t bin = layout->cover[c].lo; bin <= layout->cover[c].hi; ++bin)
                if (state->dense[bin] >= at_least) (*covered)++;
        return;
    }
    // Sparse storage lists hit bins only.
    for (size_t slot = 0; slot < state->sparse.capacity; ++slot) {
        uint64_t key = state->sparse.keys[slot];
        if (!key || state->sparse.counts[slot] < at_least) continue;
        if (llg_cov_cover_rank(layout, key - 1u) >= 0) (*covered)++;
    }
}

static uint64_t llg_cov_saturating_add(uint64_t a, uint64_t b) {
    return b > UINT64_MAX - a ? UINT64_MAX : a + b;
}

// Covered and defined transition bins of one coverpoint: scalar bins with a
// surviving sequence, and the sequences of array bins.
static void llg_cov_transition_bins(const llg_cov_inst_t* inst, uint32_t index,
                                    uint64_t* covered, uint64_t* defined) {
    const llg_cov_point_state_t* state = &inst->points[index];
    const llg_cov_trans_t* trans = inst->group->points[index].trans;
    const llg_cov_history_t* history = state->history;
    *covered = 0;
    *defined = 0;
    if (!trans || !history) return;
    uint64_t at_least = state->at_least < 0 ? 0u : (uint64_t)state->at_least;
    for (uint32_t b = 0; b < trans->n_bins; ++b) {
        const llg_cov_tbin_t* bin = &trans->bins[b];
        if (bin->kind != LLG_COV_BIN_BINS) continue;
        *defined = llg_cov_saturating_add(*defined, bin->coverable);
        if (at_least == 0) *covered = llg_cov_saturating_add(*covered, bin->coverable);
        else if (!bin->array && bin->coverable && history->counts[b] >= at_least) (*covered)++;
    }
    if (at_least == 0) return;
    for (size_t i = 0; i < history->sequence_capacity; ++i)
        if (history->sequences[i].length && history->sequences[i].count >= at_least) (*covered)++;
}

static void llg_cov_point_bins(const llg_cov_inst_t* inst, uint32_t index, uint64_t* covered,
                               uint64_t* defined) {
    llg_cov_value_bins(inst, index, covered, defined);
    uint64_t more_covered, more_defined;
    llg_cov_transition_bins(inst, index, &more_covered, &more_defined);
    *covered = llg_cov_saturating_add(*covered, more_covered);
    *defined = llg_cov_saturating_add(*defined, more_defined);
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
                llg_cov_history_free(inst->points[i].history);
            }
            free(inst->slots);
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
    for (size_t i = 0; i < llg_cov_n_cached_groups; ++i) {
        free(llg_cov_cached_groups[i]->layouts);
        llg_cov_cached_groups[i]->layouts = NULL;
    }
    free(llg_cov_cached_groups);
    llg_cov_cached_groups = NULL;
    llg_cov_n_cached_groups = llg_cov_cached_capacity = 0;
    while (llg_cov_layouts_all) {
        llg_cov_layout_t* layout = llg_cov_layouts_all;
        llg_cov_layouts_all = layout->next_all;
        free(layout->segments);
        free(layout->entries);
        free(layout->cover);
        free(layout->cover_rank);
        free(layout);
    }
}
