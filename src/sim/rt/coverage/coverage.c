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
    // Per declaration: its first counter and its number of `bins` elements
    // (zero for other kinds), for cross selections of one bin.
    uint64_t* decl_base;
    uint64_t* decl_size;
    // `b[]` declarations: their values in element order, for bin names.
    uint32_t n_decls;
    llg_cov_span_t** decl_values;
    size_t* n_decl_values;
    uint8_t overlap_reported;  // option.detect_overlap warnings were issued
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
    uint8_t detect_overlap;
    uint8_t has_detect_overlap;
    uint8_t stopped;
    const llg_cov_layout_t* layout;  // set by llg_cov_ready
    uint64_t* dense;  // n_bins counters when n_bins <= LLG_COV_DENSE_BINS
    llg_cov_sparse_t sparse;
    llg_cov_history_t* history;  // transition bins only
    // Crossed coverpoints: the coverable ranks this sample counted.
    uint64_t hit_serial;
    uint32_t n_hits, hit_capacity;
    uint64_t* hits;
    struct llg_cov_xstate* cross;  // crosses only
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
    uint8_t detect_overlap;
    uint8_t get_inst_coverage;
    uint64_t serial;  // samples begun, for crosses
    llg_cov_slot_value_t* slots;
    llg_cov_point_state_t points[];  // points, then crosses
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

static uint64_t llg_cov_saturating_add(uint64_t a, uint64_t b) {
    return b > UINT64_MAX - a ? UINT64_MAX : a + b;
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
    size_t decls = point->n_bins ? point->n_bins : 1u;
    layout->decl_base = (uint64_t*)llg_checked_calloc(decls, sizeof(uint64_t), "coverage layout");
    layout->decl_size = (uint64_t*)llg_checked_calloc(decls, sizeof(uint64_t), "coverage layout");
    layout->n_decls = point->n_bins;
    layout->decl_values =
        (llg_cov_span_t**)llg_checked_calloc(decls, sizeof(void*), "coverage layout");
    layout->n_decl_values = (size_t*)llg_checked_calloc(decls, sizeof(size_t), "coverage layout");
    for (uint32_t d = 0; d < point->n_bins; ++d) {
        const llg_cov_bin_t* bin = &point->bins[d];
        layout->decl_base[d] = n_bins;
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
            layout->decl_size[d] = count;
            continue;
        }
        // A value listed twice is still one value of the bin.
        n_spans = llg_cov_spans_merge(spans, n_spans);
        if (bin->kind == LLG_COV_BIN_BINS && bin->shape == LLG_COV_SHAPE_PER_VALUE) {
            if (n_spans) {
                layout->decl_values[d] = (llg_cov_span_t*)llg_checked_malloc(
                    n_spans, sizeof(llg_cov_span_t), "coverage layout");
                memcpy(layout->decl_values[d], spans, n_spans * sizeof(llg_cov_span_t));
                layout->n_decl_values[d] = n_spans;
            }
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
            layout->decl_size[d] = entry.pos;
            continue;
        }
        for (size_t i = 0; i < n_spans; ++i) {
            llg_cov_piece_t piece = {spans[i].lo, spans[i].hi, entry, bin->kind};
            LLG_COV_PUSH(pieces, n_pieces, pieces_cap, piece, "coverage bin values");
        }
        if (bin->kind == LLG_COV_BIN_BINS) {
            n_bins = llg_cov_add_bins(n_bins, 1);
            layout->decl_size[d] = 1;
        }
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
        if (!group->xlayouts)
            LLG_COV_PUSH(llg_cov_cached_groups, llg_cov_n_cached_groups, llg_cov_cached_capacity,
                         group, "coverage layouts");
        group->layouts = (llg_cov_layout_t**)llg_checked_calloc(group->n_points, sizeof(void*),
                                                                "coverage layouts");
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

static int64_t llg_cov_cover_rank(const llg_cov_layout_t* layout, uint64_t bin);

// Count a value bin; a crossed coverpoint also notes the bin's coordinate
// for the crosses of this sample.
static void llg_cov_count_point(llg_cov_point_state_t* state, const llg_cov_point_t* point,
                                uint64_t bin) {
    llg_cov_count(state, bin);
    if (!point->crossed) return;
    int64_t rank = llg_cov_cover_rank(state->layout, bin);
    if (rank < 0) return;
    if (state->n_hits == state->hit_capacity) {
        state->hit_capacity = state->hit_capacity ? state->hit_capacity * 2u : 4u;
        state->hits = llg_cov_grow(state->hits, state->hit_capacity, sizeof(uint64_t),
                                   "coverage cross");
    }
    state->hits[state->n_hits++] = (uint64_t)rank;
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

// ── Crosses (19.6) ────────────────────────────────────────────────────────
//
// A cross layout splits each dimension's coordinates (the crossed
// coverpoint's coverable bins, ranked) into runs whose coordinates belong to
// the same boxes; a run's signature is that box set. A sampled product
// belongs to the boxes in the AND of its coordinates' signatures: none makes
// it an automatic cross bin; otherwise it counts in the user bins of those
// boxes, unless an ignore or illegal box holds it (19.6.2, 19.6.3).

struct llg_cov_xlayout {
    struct llg_cov_xlayout* next_all;
    uint32_t n_dims;
    uint32_t words;  // signature words
    uint64_t* sizes;  // coordinates per dimension
    uint64_t* strides;  // automatic bin index = sum of rank * stride
    uint32_t* n_runs;
    llg_cov_span_t** runs;
    uint64_t** sigs;  // per dimension: `words` per run
    uint64_t* bin_masks;  // per bin: `words` of its boxes
    uint64_t* removal;  // boxes of ignore and illegal bins
    const llg_cov_layout_t** from;  // the point layouts it was built from
    uint64_t product;  // the number of cross products
    uint64_t n_auto;  // Bc of 19.11.2
    uint64_t n_user;  // Bu of 19.11.2
    uint8_t* significant;  // per bin: a `bins` bin with a product left
};
typedef struct llg_cov_xlayout llg_cov_xlayout_t;

typedef struct llg_cov_xstate {
    const llg_cov_xlayout_t* layout;
    uint64_t* counts;  // per bin
    uint64_t* stamp;  // per bin: the sample that last counted it
} llg_cov_xstate_t;

static llg_cov_xlayout_t* llg_cov_xlayouts_all;

// Rank spans of the coverable bins in the counter range bins.lo..bins.hi.
static size_t llg_cov_rank_spans(const llg_cov_layout_t* layout, llg_cov_span_t bins,
                                 llg_cov_span_t** out, size_t n, size_t* cap) {
    uint32_t lo = 0, hi = layout->n_cover;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        if (layout->cover[mid].hi < bins.lo) lo = mid + 1u;
        else hi = mid;
    }
    for (uint32_t c = lo; c < layout->n_cover && layout->cover[c].lo <= bins.hi; ++c) {
        uint64_t first = layout->cover[c].lo > bins.lo ? layout->cover[c].lo : bins.lo;
        uint64_t last = layout->cover[c].hi < bins.hi ? layout->cover[c].hi : bins.hi;
        llg_cov_span_t ranks = {layout->cover_rank[c] + (first - layout->cover[c].lo),
                                layout->cover_rank[c] + (last - layout->cover[c].lo)};
        LLG_COV_PUSH(*out, n, *cap, ranks, "coverage cross");
    }
    return n;
}

// The coordinates of one select condition, as sorted disjoint rank spans.
static size_t llg_cov_select_spans(const llg_cov_inst_t* inst, const llg_cov_cross_t* cross,
                                   const llg_cov_xbin_t* xbin, const llg_cov_select_t* select,
                                   llg_cov_span_t** out) {
    uint32_t index = cross->dims[select->dim];
    const llg_cov_point_t* point = &inst->group->points[index];
    const llg_cov_layout_t* layout = inst->points[index].layout;
    size_t n = 0, cap = 0;
    uint64_t size = layout->n_coverable;
    if (!select->intersect) {
        if (select->decl < 0) {
            if (size) {
                llg_cov_span_t all = {0, size - 1u};
                LLG_COV_PUSH(*out, n, cap, all, "coverage cross");
            }
        } else if (layout->decl_size[select->decl]) {
            uint64_t base = layout->decl_base[select->decl];
            llg_cov_span_t bins = {base, base + layout->decl_size[select->decl] - 1u};
            n = llg_cov_rank_spans(layout, bins, out, n, &cap);
        }
    } else {
        // 19.6: the bins whose values intersect the listed values.
        llg_cov_bin_t named = {xbin->name, xbin->site, LLG_COV_BIN_BINS, 0, 0, -1, 0, 0, 0};
        llg_cov_span_t* keys = NULL;
        size_t n_keys = 0, keys_cap = 0;
        for (uint32_t i = 0; i < select->n_items; ++i) {
            llg_cov_span_t span;
            if (llg_cov_item_keys(inst, point, &named, &cross->items[select->first_item + i], &span))
                LLG_COV_PUSH(keys, n_keys, keys_cap, span, "coverage cross");
        }
        n_keys = llg_cov_spans_merge(keys, n_keys);
        llg_cov_span_t* bins = NULL;
        size_t n_bins = 0, bins_cap = 0;
        for (size_t k = 0; k < n_keys; ++k) {
            uint64_t lo = keys[k].lo, hi = keys[k].hi;
            uint32_t s = 0, end = layout->n_segments;
            while (s < end) {
                uint32_t mid = s + (end - s) / 2u;
                if (layout->segments[mid].hi < lo) s = mid + 1u;
                else end = mid;
            }
            uint64_t next = lo;
            int exhausted = 0;
            for (; s < layout->n_segments && layout->segments[s].lo <= hi; ++s) {
                const llg_cov_seg_t* segment = &layout->segments[s];
                uint64_t first = segment->lo > lo ? segment->lo : lo;
                uint64_t last = segment->hi < hi ? segment->hi : hi;
                if (point->kind != LLG_COV_POINT_EXPLICIT && select->decl < 0 && first > next)
                    n_bins = llg_cov_auto_spans(point, layout, next, first - 1u, &bins, n_bins,
                                                &bins_cap);
                if (last == UINT64_MAX) exhausted = 1;
                else next = last + 1u;
                if (segment->removed) continue;
                for (uint32_t e = 0; e < segment->count; ++e) {
                    llg_cov_entry_t entry = layout->entries[segment->first + e];
                    if (select->decl >= 0 && entry.decl != (uint32_t)select->decl) continue;
                    entry.pos += first - segment->lo;
                    llg_cov_span_t span = llg_cov_entry_bins(&entry, first, last);
                    LLG_COV_PUSH(bins, n_bins, bins_cap, span, "coverage cross");
                }
            }
            if (point->kind != LLG_COV_POINT_EXPLICIT && select->decl < 0 && !exhausted &&
                next <= hi)
                n_bins = llg_cov_auto_spans(point, layout, next, hi, &bins, n_bins, &bins_cap);
        }
        for (size_t b = 0; b < n_bins; ++b) n = llg_cov_rank_spans(layout, bins[b], out, n, &cap);
        free(bins);
        free(keys);
    }
    n = llg_cov_spans_merge(*out, n);
    if (!select->negate) return n;
    // `!` selects the coordinates of the dimension the condition leaves out.
    llg_cov_span_t* rest = NULL;
    size_t n_rest = 0, rest_cap = 0;
    uint64_t next = 0;
    for (size_t i = 0; i < n && next < size; ++i) {
        if ((*out)[i].lo > next) {
            llg_cov_span_t gap = {next, (*out)[i].lo - 1u};
            LLG_COV_PUSH(rest, n_rest, rest_cap, gap, "coverage cross");
        }
        next = (*out)[i].hi + 1u;
    }
    if (next < size) {
        llg_cov_span_t gap = {next, size - 1u};
        LLG_COV_PUSH(rest, n_rest, rest_cap, gap, "coverage cross");
    }
    free(*out);
    *out = rest;
    return n_rest;
}

// The intersection of two sorted disjoint span lists, into `out`.
static size_t llg_cov_spans_and(const llg_cov_span_t* a, size_t n_a, const llg_cov_span_t* b,
                                size_t n_b, llg_cov_span_t** out) {
    size_t n = 0, cap = 0, i = 0, j = 0;
    while (i < n_a && j < n_b) {
        uint64_t lo = a[i].lo > b[j].lo ? a[i].lo : b[j].lo;
        uint64_t hi = a[i].hi < b[j].hi ? a[i].hi : b[j].hi;
        if (lo <= hi) {
            llg_cov_span_t span = {lo, hi};
            LLG_COV_PUSH(*out, n, cap, span, "coverage cross");
        }
        if (a[i].hi < b[j].hi) i++;
        else j++;
    }
    return n;
}

static int llg_cov_spans_hold(const llg_cov_span_t* spans, size_t n, uint64_t value) {
    size_t lo = 0, hi = n;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        if (spans[mid].hi < value) lo = mid + 1u;
        else hi = mid;
    }
    return lo < n && spans[lo].lo <= value;
}

static void llg_cov_xlayout_error(const llg_cov_inst_t* inst, const llg_cov_cross_t* cross,
                                  const char* what) {
    fprintf(stderr,
            "llg: runtime error: covergroup `%s` cross `%s` %s; its bins are not counted (llg "
            "limitation)\n",
            inst->group->name, cross->name, what);
    llg_rt_mark_failed();
}

typedef struct {
    const uint64_t* sig;
    uint64_t size;
} llg_cov_xclass_t;

static uint32_t llg_cov_sort_words;
static int llg_cov_xclass_order(const void* a, const void* b) {
    const llg_cov_xclass_t* x = (const llg_cov_xclass_t*)a;
    const llg_cov_xclass_t* y = (const llg_cov_xclass_t*)b;
    return memcmp(x->sig, y->sig, llg_cov_sort_words * sizeof(uint64_t));
}

// Bb, Bc and Bu of 19.11.2: products are enumerated per class of runs with
// one signature in every dimension, so the work follows the select
// expressions, not the number of products.
static void llg_cov_xlayout_count(const llg_cov_inst_t* inst, const llg_cov_cross_t* cross,
                                  llg_cov_xlayout_t* xl) {
    uint32_t dims = xl->n_dims, words = xl->words;
    llg_cov_xclass_t** classes =
        (llg_cov_xclass_t**)llg_checked_calloc(dims, sizeof(void*), "coverage cross");
    size_t* n_classes = (size_t*)llg_checked_calloc(dims, sizeof(size_t), "coverage cross");
    uint64_t tuples = 1;
    int too_many = 0;
    for (uint32_t d = 0; d < dims; ++d) {
        size_t runs = xl->n_runs[d];
        llg_cov_xclass_t* list =
            (llg_cov_xclass_t*)llg_checked_calloc(runs ? runs : 1u, sizeof(*list), "coverage cross");
        for (size_t r = 0; r < runs; ++r)
            list[r] = (llg_cov_xclass_t){xl->sigs[d] + r * words,
                                         xl->runs[d][r].hi - xl->runs[d][r].lo + 1u};
        llg_cov_sort_words = words;
        qsort(list, runs, sizeof(*list), llg_cov_xclass_order);
        size_t unique = 0;
        for (size_t r = 0; r < runs; ++r) {
            if (unique && memcmp(list[unique - 1u].sig, list[r].sig, words * sizeof(uint64_t)) == 0)
                list[unique - 1u].size += list[r].size;
            else list[unique++] = list[r];
        }
        classes[d] = list;
        n_classes[d] = unique;
        if (unique && tuples > LLG_COV_CROSS_CLASSES / unique) too_many = 1;
        else tuples *= unique ? unique : 1u;
    }
    if (too_many) {
        llg_cov_xlayout_error(inst, cross, "has select expressions with too many value classes");
        xl->n_auto = 0;
    } else if (xl->product) {
        uint64_t* acc = (uint64_t*)llg_checked_calloc(words, sizeof(uint64_t), "coverage cross");
        size_t* pick = (size_t*)llg_checked_calloc(dims, sizeof(size_t), "coverage cross");
        uint64_t in_boxes = 0;
        for (;;) {
            uint64_t size = 1;
            for (uint32_t w = 0; w < words; ++w) acc[w] = ~UINT64_C(0);
            for (uint32_t d = 0; d < dims; ++d) {
                const llg_cov_xclass_t* cls = &classes[d][pick[d]];
                size *= cls->size;
                for (uint32_t w = 0; w < words; ++w) acc[w] &= cls->sig[w];
            }
            int any = 0, removed = 0;
            for (uint32_t w = 0; w < words; ++w) {
                any |= acc[w] != 0;
                removed |= (acc[w] & xl->removal[w]) != 0;
            }
            if (any) {
                in_boxes += size;
                for (uint32_t b = 0; b < cross->n_bins && !removed; ++b) {
                    if (cross->bins[b].kind != LLG_COV_BIN_BINS || xl->significant[b]) continue;
                    for (uint32_t w = 0; w < words; ++w)
                        if (acc[w] & xl->bin_masks[(size_t)b * words + w]) xl->significant[b] = 1;
                }
            }
            uint32_t d = 0;
            while (d < dims && ++pick[d] == n_classes[d]) pick[d++] = 0;
            if (d == dims) break;
        }
        xl->n_auto = xl->product - in_boxes;
        free(pick);
        free(acc);
    }
    for (uint32_t b = 0; b < cross->n_bins; ++b) xl->n_user += xl->significant[b];
    for (uint32_t d = 0; d < dims; ++d) free(classes[d]);
    free(classes);
    free(n_classes);
}

static llg_cov_xlayout_t* llg_cov_build_xlayout(const llg_cov_inst_t* inst, uint32_t index) {
    const llg_cov_cross_t* cross = &inst->group->crosses[index];
    uint32_t dims = cross->n_dims, boxes = cross->n_boxes;
    uint32_t words = boxes ? (boxes + 63u) / 64u : 1u;
    llg_cov_xlayout_t* xl = (llg_cov_xlayout_t*)llg_checked_calloc(1, sizeof(*xl), "coverage cross");
    xl->n_dims = dims;
    xl->words = words;
    xl->sizes = (uint64_t*)llg_checked_calloc(dims, sizeof(uint64_t), "coverage cross");
    xl->strides = (uint64_t*)llg_checked_calloc(dims, sizeof(uint64_t), "coverage cross");
    xl->n_runs = (uint32_t*)llg_checked_calloc(dims, sizeof(uint32_t), "coverage cross");
    xl->runs = (llg_cov_span_t**)llg_checked_calloc(dims, sizeof(void*), "coverage cross");
    xl->sigs = (uint64_t**)llg_checked_calloc(dims, sizeof(void*), "coverage cross");
    xl->from = (const llg_cov_layout_t**)llg_checked_calloc(dims, sizeof(void*), "coverage cross");
    size_t bins = cross->n_bins ? cross->n_bins : 1u;
    xl->bin_masks = (uint64_t*)llg_checked_calloc(bins * words, sizeof(uint64_t), "coverage cross");
    xl->removal = (uint64_t*)llg_checked_calloc(words, sizeof(uint64_t), "coverage cross");
    xl->significant = (uint8_t*)llg_checked_calloc(bins, 1, "coverage cross");
    xl->product = 1;
    int overflow = 0;
    for (uint32_t d = 0; d < dims; ++d) {
        const llg_cov_layout_t* layout = inst->points[cross->dims[d]].layout;
        xl->from[d] = layout;
        xl->sizes[d] = layout->n_coverable;
        xl->strides[d] = xl->product;
        if (xl->sizes[d] && xl->product > UINT64_MAX / xl->sizes[d]) overflow = 1;
        else xl->product *= xl->sizes[d];
    }
    uint32_t* box_bin = (uint32_t*)llg_checked_calloc(boxes ? boxes : 1u, sizeof(uint32_t),
                                                      "coverage cross");
    for (uint32_t b = 0; b < cross->n_bins; ++b) {
        const llg_cov_xbin_t* xbin = &cross->bins[b];
        for (uint32_t k = 0; k < xbin->n_boxes; ++k) {
            uint32_t box = xbin->first_box + k;
            box_bin[box] = b;
            xl->bin_masks[(size_t)b * words + box / 64u] |= UINT64_C(1) << (box % 64u);
            if (xbin->kind != LLG_COV_BIN_BINS) xl->removal[box / 64u] |= UINT64_C(1) << (box % 64u);
        }
    }
    // Each condition once, then each box's coordinates per dimension.
    llg_cov_span_t** select_spans = (llg_cov_span_t**)llg_checked_calloc(
        cross->n_selects ? cross->n_selects : 1u, sizeof(void*), "coverage cross");
    size_t* select_n = (size_t*)llg_checked_calloc(cross->n_selects ? cross->n_selects : 1u,
                                                   sizeof(size_t), "coverage cross");
    for (uint32_t s = 0; s < cross->n_selects; ++s) {
        // The bin that names a condition, for 19.5.6 warnings.
        const llg_cov_xbin_t* owner = &cross->bins[0];
        for (uint32_t box = 0; box < boxes; ++box)
            for (uint32_t k = 0; k < cross->boxes[box].n_selects; ++k)
                if (cross->box_selects[cross->boxes[box].first_select + k] == s)
                    owner = &cross->bins[box_bin[box]];
        select_n[s] = llg_cov_select_spans(inst, cross, owner, &cross->selects[s], &select_spans[s]);
    }
    for (uint32_t d = 0; d < dims; ++d) {
        uint64_t size = xl->sizes[d];
        llg_cov_span_t** box_spans =
            (llg_cov_span_t**)llg_checked_calloc(boxes ? boxes : 1u, sizeof(void*), "coverage cross");
        size_t* box_n = (size_t*)llg_checked_calloc(boxes ? boxes : 1u, sizeof(size_t), "coverage cross");
        size_t n_bounds = 0, bounds_cap = 0;
        uint64_t* bounds = NULL;
        for (uint32_t box = 0; box < boxes; ++box) {
            llg_cov_span_t* spans = NULL;
            size_t n = 0, cap = 0;
            if (size) {
                llg_cov_span_t all = {0, size - 1u};
                LLG_COV_PUSH(spans, n, cap, all, "coverage cross");
            }
            for (uint32_t k = 0; k < cross->boxes[box].n_selects; ++k) {
                uint32_t s = cross->box_selects[cross->boxes[box].first_select + k];
                if (cross->selects[s].dim != d) continue;
                llg_cov_span_t* both = NULL;
                n = llg_cov_spans_and(spans, n, select_spans[s], select_n[s], &both);
                free(spans);
                spans = both;
            }
            box_spans[box] = spans;
            box_n[box] = n;
            for (size_t i = 0; i < n; ++i) {
                LLG_COV_PUSH(bounds, n_bounds, bounds_cap, spans[i].lo, "coverage cross");
                if (spans[i].hi + 1u < size)
                    LLG_COV_PUSH(bounds, n_bounds, bounds_cap, spans[i].hi + 1u, "coverage cross");
            }
        }
        if (size) {
            LLG_COV_PUSH(bounds, n_bounds, bounds_cap, UINT64_C(0), "coverage cross");
            qsort(bounds, n_bounds, sizeof(uint64_t), llg_cov_u64_order);
            size_t unique = 0;
            for (size_t i = 0; i < n_bounds; ++i)
                if (!unique || bounds[unique - 1u] != bounds[i]) bounds[unique++] = bounds[i];
            if (unique > UINT32_MAX) llg_fatal_allocation("coverage cross", unique, sizeof(uint64_t));
            xl->n_runs[d] = (uint32_t)unique;
            xl->runs[d] = (llg_cov_span_t*)llg_checked_malloc(unique, sizeof(llg_cov_span_t),
                                                              "coverage cross");
            xl->sigs[d] = (uint64_t*)llg_checked_calloc(unique * words, sizeof(uint64_t),
                                                        "coverage cross");
            for (size_t r = 0; r < unique; ++r) {
                llg_cov_span_t run = {bounds[r], r + 1u < unique ? bounds[r + 1u] - 1u : size - 1u};
                xl->runs[d][r] = run;
                for (uint32_t box = 0; box < boxes; ++box)
                    if (llg_cov_spans_hold(box_spans[box], box_n[box], run.lo))
                        xl->sigs[d][r * words + box / 64u] |= UINT64_C(1) << (box % 64u);
            }
        }
        for (uint32_t box = 0; box < boxes; ++box) free(box_spans[box]);
        free(box_spans);
        free(box_n);
        free(bounds);
    }
    for (uint32_t s = 0; s < cross->n_selects; ++s) free(select_spans[s]);
    free(select_spans);
    free(select_n);
    free(box_bin);
    if (overflow) {
        llg_cov_xlayout_error(inst, cross, "has more than 2^64 cross products");
        xl->product = 0;
    } else {
        llg_cov_xlayout_count(inst, cross, xl);
    }
    xl->next_all = llg_cov_xlayouts_all;
    llg_cov_xlayouts_all = xl;
    return xl;
}

static void llg_cov_xlayout_free(llg_cov_xlayout_t* xl) {
    for (uint32_t d = 0; d < xl->n_dims; ++d) {
        free(xl->runs[d]);
        free(xl->sigs[d]);
    }
    free(xl->sizes);
    free(xl->strides);
    free(xl->n_runs);
    free(xl->runs);
    free(xl->sigs);
    free(xl->from);
    free(xl->bin_masks);
    free(xl->removal);
    free(xl->significant);
    free(xl);
}

static void llg_cov_cache_group(llg_cov_group_t* group) {
    if (group->layouts || group->xlayouts) return;
    LLG_COV_PUSH(llg_cov_cached_groups, llg_cov_n_cached_groups, llg_cov_cached_capacity, group,
                 "coverage layouts");
}

// A cross's layout for a ready instance: shared while every crossed
// coverpoint uses the layout the shared one was built from.
static const llg_cov_xlayout_t* llg_cov_xlayout_for(llg_cov_inst_t* inst, uint32_t index) {
    llg_cov_group_t* group = inst->group;
    const llg_cov_cross_t* cross = &group->crosses[index];
    if (cross->dynamic) return llg_cov_build_xlayout(inst, index);
    if (!group->xlayouts) {
        llg_cov_cache_group(group);
        group->xlayouts = (llg_cov_xlayout_t**)llg_checked_calloc(group->n_crosses, sizeof(void*),
                                                                  "coverage layouts");
    }
    llg_cov_xlayout_t* shared = group->xlayouts[index];
    if (shared) {
        int same = 1;
        for (uint32_t d = 0; d < cross->n_dims; ++d)
            same &= shared->from[d] == inst->points[cross->dims[d]].layout;
        if (same) return shared;
        return llg_cov_build_xlayout(inst, index);
    }
    shared = llg_cov_build_xlayout(inst, index);
    group->xlayouts[index] = shared;
    return shared;
}

static void llg_cov_cross_ready(llg_cov_inst_t* inst, uint32_t index) {
    llg_cov_point_state_t* state = &inst->points[inst->group->n_points + index];
    if (!state->has_at_least) state->at_least = inst->at_least;
    llg_cov_xstate_t* cross = (llg_cov_xstate_t*)llg_checked_calloc(1, sizeof(*cross), "coverage cross");
    cross->layout = llg_cov_xlayout_for(inst, index);
    size_t bins = inst->group->crosses[index].n_bins ? inst->group->crosses[index].n_bins : 1u;
    cross->counts = (uint64_t*)llg_checked_calloc(bins, sizeof(uint64_t), "coverage cross");
    cross->stamp = (uint64_t*)llg_checked_calloc(bins, sizeof(uint64_t), "coverage cross");
    state->cross = cross;
    if (cross->layout->product && cross->layout->product <= LLG_COV_DENSE_BINS)
        state->dense = (uint64_t*)llg_checked_calloc((size_t)cross->layout->product,
                                                     sizeof(uint64_t), "coverage cross");
}

void llg_cov_sample_begin(void* record) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (inst) inst->serial++;
}

static const uint64_t* llg_cov_run_sig(const llg_cov_xlayout_t* xl, uint32_t d, uint64_t rank) {
    uint32_t lo = 0, hi = xl->n_runs[d];
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        if (xl->runs[d][mid].hi < rank) lo = mid + 1u;
        else hi = mid;
    }
    return xl->sigs[d] + (size_t)lo * xl->words;
}

void llg_cov_cross_hit(void* record, uint32_t index, const uint8_t* guards) {
    llg_cov_inst_t* inst = (llg_cov_inst_t*)record;
    if (!inst || inst->stopped || index >= inst->group->n_crosses) return;
    const llg_cov_cross_t* cross = &inst->group->crosses[index];
    llg_cov_point_state_t* state = &inst->points[inst->group->n_points + index];
    if (state->stopped || !state->cross) return;
    const llg_cov_xlayout_t* xl = state->cross->layout;
    if (!xl->product) return;
    // A crossed coverpoint that counted nothing in this sample (disabled by
    // its iff, stopped, or an X/Z value) leaves the cross unsampled
    // (decision S45-D14).
    for (uint32_t d = 0; d < cross->n_dims; ++d) {
        const llg_cov_point_state_t* point = &inst->points[cross->dims[d]];
        if (point->hit_serial != inst->serial || !point->n_hits) return;
    }
    uint32_t words = xl->words;
    uint64_t acc_small[4];
    uint64_t* acc = words <= 4u ? acc_small
                                : (uint64_t*)llg_checked_calloc(words, sizeof(uint64_t),
                                                                "coverage cross");
    uint32_t pick[64];
    uint32_t* picks = cross->n_dims <= 64u
                          ? pick
                          : (uint32_t*)llg_checked_calloc(cross->n_dims, sizeof(uint32_t),
                                                          "coverage cross");
    for (uint32_t d = 0; d < cross->n_dims; ++d) picks[d] = 0;
    // A value in overlapping bins hits several coordinates: every product
    // of them counts.
    for (;;) {
        uint64_t auto_index = 0;
        for (uint32_t w = 0; w < words; ++w) acc[w] = ~UINT64_C(0);
        for (uint32_t d = 0; d < cross->n_dims; ++d) {
            uint64_t rank = inst->points[cross->dims[d]].hits[picks[d]];
            auto_index += rank * xl->strides[d];
            const uint64_t* sig = llg_cov_run_sig(xl, d, rank);
            for (uint32_t w = 0; w < words; ++w) acc[w] &= sig[w];
        }
        int any = 0, removed = 0;
        for (uint32_t w = 0; w < words; ++w) {
            any |= acc[w] != 0;
            removed |= (acc[w] & xl->removal[w]) != 0;
        }
        if (!any) {
            llg_cov_count(state, auto_index);
        } else {
            for (uint32_t b = 0; b < cross->n_bins; ++b) {
                const llg_cov_xbin_t* bin = &cross->bins[b];
                int member = 0;
                for (uint32_t w = 0; w < words; ++w)
                    member |= (acc[w] & xl->bin_masks[(size_t)b * words + w]) != 0;
                if (!member || (bin->guard >= 0 && guards && !guards[bin->guard])) continue;
                // 19.6.2, 19.6.3: ignored and illegal products count in no
                // bin; an illegal product is an error even when ignored.
                if (removed && bin->kind == LLG_COV_BIN_BINS) continue;
                if (bin->kind == LLG_COV_BIN_IGNORE) continue;
                if (state->cross->stamp[b] == inst->serial) continue;
                state->cross->stamp[b] = inst->serial;
                if (bin->kind == LLG_COV_BIN_ILLEGAL) {
                    fprintf(stderr,
                            "llg: runtime error: covergroup `%s` cross `%s` sampled a product of "
                            "illegal_bins `%s` at %s (IEEE 1800-2009 19.6.3)\n",
                            inst->group->name, cross->name, bin->name, bin->site);
                    llg_rt_mark_failed();
                    continue;
                }
                if (state->cross->counts[b] != UINT64_MAX) state->cross->counts[b]++;
            }
        }
        uint32_t d = 0;
        while (d < cross->n_dims && ++picks[d] == inst->points[cross->dims[d]].n_hits)
            picks[d++] = 0;
        if (d == cross->n_dims) break;
    }
    if (acc != acc_small) free(acc);
    if (picks != pick) free(picks);
}

// Covered and defined bins of one cross (19.11.2): automatic cross bins and
// significant user bins.
static void llg_cov_cross_bins(const llg_cov_inst_t* inst, uint32_t index, uint64_t* covered,
                               uint64_t* defined) {
    const llg_cov_point_state_t* state = &inst->points[inst->group->n_points + index];
    const llg_cov_cross_t* cross = &inst->group->crosses[index];
    *covered = 0;
    *defined = 0;
    if (!state->cross) return;
    const llg_cov_xlayout_t* xl = state->cross->layout;
    *defined = llg_cov_saturating_add(xl->n_auto, xl->n_user);
    uint64_t at_least = state->at_least < 0 ? 0u : (uint64_t)state->at_least;
    if (at_least == 0) {
        *covered = *defined;
        return;
    }
    for (uint32_t b = 0; b < cross->n_bins; ++b)
        if (xl->significant[b] && state->cross->counts[b] >= at_least) (*covered)++;
    if (state->dense) {
        for (uint64_t i = 0; i < xl->product; ++i)
            if (state->dense[i] >= at_least) (*covered)++;
        return;
    }
    for (size_t slot = 0; slot < state->sparse.capacity; ++slot)
        if (state->sparse.keys[slot] && state->sparse.counts[slot] >= at_least) (*covered)++;
}

// ── Instances ─────────────────────────────────────────────────────────────

void* llg_cov_new(llg_cov_group_t* group) {
    size_t points = (size_t)group->n_points + group->n_crosses;
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
    static const char* const names[] = {"weight",         "goal",
                                        "at_least",       "auto_bin_max",
                                        "detect_overlap", "get_inst_coverage"};
    if (!inst || option > LLG_COV_OPTION_GET_INST_COVERAGE) return;
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
        case LLG_COV_OPTION_DETECT_OVERLAP: inst->detect_overlap = number != 0; break;
        case LLG_COV_OPTION_GET_INST_COVERAGE: inst->get_inst_coverage = number != 0; break;
        default: inst->auto_bin_max = number; break;
        }
        return;
    }
    if ((uint32_t)point >= inst->group->n_points + inst->group->n_crosses) return;
    llg_cov_point_state_t* state = &inst->points[point];
    switch (option) {
    case LLG_COV_OPTION_WEIGHT: state->weight = number; break;
    case LLG_COV_OPTION_GOAL: state->goal = number; break;
    case LLG_COV_OPTION_AT_LEAST:
        state->at_least = number;
        state->has_at_least = 1;
        break;
    case LLG_COV_OPTION_DETECT_OVERLAP:
        state->detect_overlap = number != 0;
        state->has_detect_overlap = 1;
        break;
    case LLG_COV_OPTION_GET_INST_COVERAGE: break;
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

static int llg_cov_pair_order(const void* a, const void* b) {
    const uint32_t* x = (const uint32_t*)a;
    const uint32_t* y = (const uint32_t*)b;
    if (x[0] != y[0]) return x[0] < y[0] ? -1 : 1;
    return x[1] < y[1] ? -1 : x[1] > y[1];
}

static void llg_cov_overlap_warning(const llg_cov_inst_t* inst, const llg_cov_point_t* point,
                                    const char* a, const char* b) {
    fprintf(stderr,
            "llg: warning: covergroup `%s` coverpoint `%s`: bins `%s` and `%s` overlap "
            "(option.detect_overlap, IEEE 1800-2009 19.7)\n",
            inst->group->name, point->name, a, b);
}

// option.detect_overlap (19.7): `bins` declarations whose values (after
// ignore and illegal removal) or transition sequences meet, once per layout
// (decision S45-D17).
static void llg_cov_report_overlaps(const llg_cov_inst_t* inst, uint32_t index) {
    const llg_cov_point_t* point = &inst->group->points[index];
    llg_cov_layout_t* layout = (llg_cov_layout_t*)inst->points[index].layout;
    if (!layout || layout->overlap_reported) return;
    layout->overlap_reported = 1;
    uint32_t(*pairs)[2] = NULL;
    size_t n_pairs = 0, cap = 0;
    for (uint32_t s = 0; s < layout->n_segments; ++s) {
        const llg_cov_seg_t* segment = &layout->segments[s];
        if (segment->removed) continue;
        for (uint32_t e = 0; e < segment->count; ++e)
            for (uint32_t f = e + 1u; f < segment->count; ++f) {
                uint32_t a = layout->entries[segment->first + e].decl;
                uint32_t b = layout->entries[segment->first + f].decl;
                if (a == b) continue;
                if (n_pairs == cap) {
                    cap = cap ? cap * 2u : 8u;
                    pairs = llg_cov_grow(pairs, cap, sizeof(*pairs), "coverage overlaps");
                }
                pairs[n_pairs][0] = a < b ? a : b;
                pairs[n_pairs][1] = a < b ? b : a;
                n_pairs++;
            }
    }
    qsort(pairs, n_pairs, sizeof(*pairs), llg_cov_pair_order);
    for (size_t i = 0; i < n_pairs; ++i) {
        if (i && pairs[i][0] == pairs[i - 1u][0] && pairs[i][1] == pairs[i - 1u][1]) continue;
        llg_cov_overlap_warning(inst, point, point->bins[pairs[i][0]].name,
                                point->bins[pairs[i][1]].name);
    }
    free(pairs);
    if (point->trans)
        for (uint32_t i = 0; i < point->trans->n_overlaps; ++i)
            llg_cov_overlap_warning(inst, point,
                                    point->trans->bins[point->trans->overlaps[2u * i]].name,
                                    point->trans->bins[point->trans->overlaps[2u * i + 1u]].name);
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
        if (!state->has_detect_overlap) state->detect_overlap = inst->detect_overlap;
        if (state->detect_overlap) llg_cov_report_overlaps(inst, i);
    }
    for (uint32_t k = 0; k < inst->group->n_crosses; ++k) llg_cov_cross_ready(inst, k);
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

// The entry of one array-bin sequence, inserted with count 0 when new; the
// table holds only sequences that occurred.
static llg_cov_sequence_t* llg_cov_sequence_entry(llg_cov_history_t* history, uint32_t bin,
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
    return entry;
}

// Count one sequence of an array bin once per sample.
static void llg_cov_sequence_count(llg_cov_history_t* history, uint32_t bin,
                                   const uint64_t* keys, uint32_t length) {
    llg_cov_sequence_t* entry = llg_cov_sequence_entry(history, bin, keys, length);
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
    if (point->crossed) {
        state->hit_serial = inst->serial;
        state->n_hits = 0;
    }
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
                llg_cov_count_point(state, point, bin);
            }
            return;
        }
    }
    switch (point->kind) {
    case LLG_COV_POINT_AUTO:
        llg_cov_count_point(state, point, llg_cov_auto_bin(point, layout->n_auto, key));
        return;
    case LLG_COV_POINT_ENUM: {
        uint32_t lo = 0, hi = point->n_keys;
        while (lo < hi) {
            uint32_t mid = lo + (hi - lo) / 2u;
            if (point->keys[mid] < key) lo = mid + 1u;
            else hi = mid;
        }
        if (lo < point->n_keys && point->keys[lo] == key) llg_cov_count_point(state, point, lo);
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
    } else if ((uint32_t)point < inst->group->n_points + inst->group->n_crosses) {
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
    if (index >= inst->group->n_points) {
        llg_cov_cross_bins(inst, index - inst->group->n_points, covered, defined);
        return;
    }
    llg_cov_value_bins(inst, index, covered, defined);
    uint64_t more_covered, more_defined;
    llg_cov_transition_bins(inst, index, &more_covered, &more_defined);
    *covered = llg_cov_saturating_add(*covered, more_covered);
    *defined = llg_cov_saturating_add(*defined, more_defined);
}

// ── Merged type coverage (19.11.3) ────────────────────────────────────────
//
// With type_option.merge_instances, type coverage counts the union of the
// bins of every instance, bins being equal when their names are (19.11.3):
// a scalar bin by its declaration, `b[]` elements by value, `b[N]` elements
// by index, automatic bins by their key range, transition bins by
// declaration and sequence, cross bins by name. A merged bin's count is the
// sum over the instances that have it; it is covered when the sum reaches
// the largest at_least among them (decision S45-D18).

typedef struct {
    uint32_t tag;  // 1 automatic range, 2 enum, 3 declaration element, 4 `b[]` value
    uint64_t a, b;
} llg_cov_name_t;

typedef struct {
    llg_cov_name_t name;
    uint64_t sum;
    uint64_t at_least;
    uint8_t used;
} llg_cov_merged_t;

typedef struct {
    llg_cov_merged_t* slots;
    size_t capacity, used;
} llg_cov_merge_table_t;

static uint64_t llg_cov_name_hash(llg_cov_name_t name) {
    uint64_t hash = (name.a * UINT64_C(0x9E3779B97F4A7C15)) ^ (name.b + name.tag);
    return hash ^ (hash >> 31) ^ (name.b * UINT64_C(0xC2B2AE3D27D4EB4F));
}

static void llg_cov_merge_add(llg_cov_merge_table_t* table, llg_cov_name_t name, uint64_t count,
                              uint64_t at_least) {
    if ((table->used + 1u) * 2u > table->capacity) {
        size_t capacity = table->capacity ? table->capacity * 2u : 64u;
        llg_cov_merged_t* slots = (llg_cov_merged_t*)llg_checked_calloc(
            capacity, sizeof(llg_cov_merged_t), "merged coverage");
        for (size_t i = 0; i < table->capacity; ++i) {
            if (!table->slots[i].used) continue;
            size_t slot = (size_t)llg_cov_name_hash(table->slots[i].name) & (capacity - 1u);
            while (slots[slot].used) slot = (slot + 1u) & (capacity - 1u);
            slots[slot] = table->slots[i];
        }
        free(table->slots);
        table->slots = slots;
        table->capacity = capacity;
    }
    size_t slot = (size_t)llg_cov_name_hash(name) & (table->capacity - 1u);
    while (table->slots[slot].used &&
           !(table->slots[slot].name.tag == name.tag && table->slots[slot].name.a == name.a &&
             table->slots[slot].name.b == name.b))
        slot = (slot + 1u) & (table->capacity - 1u);
    llg_cov_merged_t* entry = &table->slots[slot];
    if (!entry->used) {
        entry->used = 1;
        entry->name = name;
        table->used++;
    }
    entry->sum = llg_cov_saturating_add(entry->sum, count);
    if (at_least > entry->at_least) entry->at_least = at_least;
}

static void llg_cov_merge_result(const llg_cov_merge_table_t* table, uint64_t* covered,
                                 uint64_t* defined) {
    for (size_t i = 0; i < table->capacity; ++i) {
        const llg_cov_merged_t* entry = &table->slots[i];
        if (!entry->used) continue;
        (*defined)++;
        if (entry->sum >= entry->at_least) (*covered)++;
    }
}

static uint64_t llg_cov_sparse_get(const llg_cov_sparse_t* table, uint64_t bin) {
    if (!table->capacity) return 0;
    uint64_t key = bin + 1u;
    size_t slot = (size_t)(key * UINT64_C(0x9E3779B97F4A7C15)) & (table->capacity - 1u);
    while (table->keys[slot]) {
        if (table->keys[slot] == key) return table->counts[slot];
        slot = (slot + 1u) & (table->capacity - 1u);
    }
    return 0;
}

static uint64_t llg_cov_bin_count_of(const llg_cov_point_state_t* state, uint64_t bin) {
    return state->dense ? state->dense[bin] : llg_cov_sparse_get(&state->sparse, bin);
}

// The name of a coverable counter of a value layout (19.11.3).
static llg_cov_name_t llg_cov_bin_name(const llg_cov_point_t* point,
                                       const llg_cov_layout_t* layout, uint64_t bin) {
    if (bin < layout->n_auto) {
        if (point->kind == LLG_COV_POINT_ENUM) return (llg_cov_name_t){2u, point->keys[bin], 0};
        uint64_t n = layout->n_auto, span;
        if (point->width >= 64u) {
            span = UINT64_MAX / n;
            if (UINT64_MAX % n == n - 1u) span++;
        } else {
            span = (UINT64_C(1) << point->width) / n;
        }
        uint64_t lo = bin * span;
        uint64_t hi = bin + 1u == n ? llg_cov_mask(point) : lo + span - 1u;
        return (llg_cov_name_t){1u, lo, hi};
    }
    uint32_t d = 0;
    while (d + 1u < layout->n_decls &&
           !(layout->decl_size[d] && bin >= layout->decl_base[d] &&
             bin - layout->decl_base[d] < layout->decl_size[d]))
        d++;
    uint64_t element = bin - layout->decl_base[d];
    if (point->bins[d].shape != LLG_COV_SHAPE_PER_VALUE) return (llg_cov_name_t){3u, d, element};
    for (size_t i = 0; i < layout->n_decl_values[d]; ++i) {
        llg_cov_span_t span = layout->decl_values[d][i];
        uint64_t size = span.hi - span.lo + 1u;
        if (element < size) return (llg_cov_name_t){4u, d, span.lo + element};
        element -= size;
    }
    return (llg_cov_name_t){3u, d, element};
}

static uint64_t llg_cov_at_least(const llg_cov_point_state_t* state) {
    return state->at_least < 0 ? 0u : (uint64_t)state->at_least;
}

// Merged value bins of coverpoint `index` over the type's instances.
static void llg_cov_merged_values(const llg_cov_type_t* type, uint32_t index, uint64_t* covered,
                                  uint64_t* defined) {
    const llg_cov_inst_t* first = type->first;
    const llg_cov_layout_t* shared = first->points[index].layout;
    uint64_t at_least = 0;
    int same = 1;
    for (const llg_cov_inst_t* inst = first; inst; inst = inst->next) {
        same &= inst->points[index].layout == shared;
        uint64_t value = llg_cov_at_least(&inst->points[index]);
        if (value > at_least) at_least = value;
    }
    if (same && shared) {
        // One layout: sum the counters bin by bin.
        *defined = shared->n_coverable;
        if (!at_least) {
            *covered = *defined;
            return;
        }
        llg_cov_merge_table_t sums = {0};
        for (const llg_cov_inst_t* inst = first; inst; inst = inst->next) {
            const llg_cov_point_state_t* state = &inst->points[index];
            if (state->dense) {
                for (uint64_t bin = 0; bin < shared->n_bins; ++bin)
                    if (state->dense[bin])
                        llg_cov_merge_add(&sums, (llg_cov_name_t){0u, bin, 0}, state->dense[bin], 0);
            } else {
                for (size_t slot = 0; slot < state->sparse.capacity; ++slot)
                    if (state->sparse.keys[slot])
                        llg_cov_merge_add(&sums,
                                          (llg_cov_name_t){0u, state->sparse.keys[slot] - 1u, 0},
                                          state->sparse.counts[slot], 0);
            }
        }
        for (size_t i = 0; i < sums.capacity; ++i)
            if (sums.slots[i].used && sums.slots[i].sum >= at_least &&
                llg_cov_cover_rank(shared, sums.slots[i].name.a) >= 0)
                (*covered)++;
        free(sums.slots);
        return;
    }
    // Layouts differ (arguments or auto_bin_max): merge by bin name.
    const llg_cov_point_t* point = &first->group->points[index];
    llg_cov_merge_table_t merged = {0};
    for (const llg_cov_inst_t* inst = first; inst; inst = inst->next) {
        const llg_cov_point_state_t* state = &inst->points[index];
        const llg_cov_layout_t* layout = state->layout;
        if (!layout) continue;
        uint64_t mine = llg_cov_at_least(state);
        for (uint32_t c = 0; c < layout->n_cover; ++c)
            for (uint64_t bin = layout->cover[c].lo;; ++bin) {
                llg_cov_merge_add(&merged, llg_cov_bin_name(point, layout, bin),
                                  llg_cov_bin_count_of(state, bin), mine);
                if (bin == layout->cover[c].hi) break;
            }
    }
    llg_cov_merge_result(&merged, covered, defined);
    free(merged.slots);
}

// Merged transition bins of coverpoint `index`: the tables are the same in
// every instance of the declaration.
static void llg_cov_merged_transitions(const llg_cov_type_t* type, uint32_t index,
                                       uint64_t* covered, uint64_t* defined) {
    const llg_cov_trans_t* trans = type->first->group->points[index].trans;
    if (!trans) return;
    uint64_t at_least = 0;
    for (const llg_cov_inst_t* inst = type->first; inst; inst = inst->next) {
        uint64_t value = llg_cov_at_least(&inst->points[index]);
        if (value > at_least) at_least = value;
    }
    for (uint32_t b = 0; b < trans->n_bins; ++b) {
        const llg_cov_tbin_t* bin = &trans->bins[b];
        if (bin->kind != LLG_COV_BIN_BINS) continue;
        *defined = llg_cov_saturating_add(*defined, bin->coverable);
        if (!at_least) {
            *covered = llg_cov_saturating_add(*covered, bin->coverable);
            continue;
        }
        if (bin->array || !bin->coverable) continue;
        uint64_t sum = 0;
        for (const llg_cov_inst_t* inst = type->first; inst; inst = inst->next)
            if (inst->points[index].history)
                sum = llg_cov_saturating_add(sum, inst->points[index].history->counts[b]);
        if (sum >= at_least) (*covered)++;
    }
    if (!at_least) return;
    // Array-bin sequences are named by their bin and keys.
    llg_cov_history_t merged = {0};
    for (const llg_cov_inst_t* inst = type->first; inst; inst = inst->next) {
        const llg_cov_history_t* history = inst->points[index].history;
        if (!history) continue;
        for (size_t i = 0; i < history->sequence_capacity; ++i) {
            const llg_cov_sequence_t* entry = &history->sequences[i];
            if (!entry->length) continue;
            llg_cov_sequence_t* sum = llg_cov_sequence_entry(&merged, entry->bin,
                                                             history->pool + entry->keys,
                                                             entry->length);
            sum->count = llg_cov_saturating_add(sum->count, entry->count);
        }
    }
    for (size_t i = 0; i < merged.sequence_capacity; ++i)
        if (merged.sequences[i].length && merged.sequences[i].count >= at_least) (*covered)++;
    free(merged.sequences);
    free(merged.pool);
}

static int llg_cov_xlayout_same(const llg_cov_xlayout_t* a, const llg_cov_xlayout_t* b) {
    if (a == b) return 1;
    if (a->n_dims != b->n_dims || a->words != b->words || a->product != b->product ||
        a->n_auto != b->n_auto || a->n_user != b->n_user)
        return 0;
    for (uint32_t d = 0; d < a->n_dims; ++d) {
        if (a->sizes[d] != b->sizes[d] || a->n_runs[d] != b->n_runs[d]) return 0;
        if (memcmp(a->runs[d], b->runs[d], a->n_runs[d] * sizeof(llg_cov_span_t)) ||
            memcmp(a->sigs[d], b->sigs[d], (size_t)a->n_runs[d] * a->words * sizeof(uint64_t)))
            return 0;
    }
    return 1;
}

// Merged bins of cross `index`; instances must have the same cross bins.
static void llg_cov_merged_cross(llg_cov_type_t* type, uint32_t index, uint64_t* covered,
                                 uint64_t* defined) {
    const llg_cov_inst_t* first = type->first;
    uint32_t item = first->group->n_points + index;
    const llg_cov_xstate_t* base = first->points[item].cross;
    if (!base) return;
    const llg_cov_xlayout_t* xl = base->layout;
    uint64_t at_least = 0;
    for (const llg_cov_inst_t* inst = first; inst; inst = inst->next) {
        const llg_cov_xstate_t* cross = inst->points[item].cross;
        if (!cross || !llg_cov_xlayout_same(cross->layout, xl)) {
            if (!type->merge_error) {
                type->merge_error = 1;
                fprintf(stderr,
                        "llg: runtime error: covergroup `%s` cross `%s`: merged type coverage "
                        "needs every instance to have the same cross bins (llg limitation)\n",
                        first->group->name, first->group->crosses[index].name);
                llg_rt_mark_failed();
            }
            return;
        }
        uint64_t value = llg_cov_at_least(&inst->points[item]);
        if (value > at_least) at_least = value;
    }
    *defined = llg_cov_saturating_add(xl->n_auto, xl->n_user);
    if (!at_least) {
        *covered = *defined;
        return;
    }
    const llg_cov_cross_t* cross = &first->group->crosses[index];
    for (uint32_t b = 0; b < cross->n_bins; ++b) {
        if (!xl->significant[b]) continue;
        uint64_t sum = 0;
        for (const llg_cov_inst_t* inst = first; inst; inst = inst->next)
            sum = llg_cov_saturating_add(sum, inst->points[item].cross->counts[b]);
        if (sum >= at_least) (*covered)++;
    }
    llg_cov_merge_table_t sums = {0};
    for (const llg_cov_inst_t* inst = first; inst; inst = inst->next) {
        const llg_cov_point_state_t* state = &inst->points[item];
        if (state->dense) {
            for (uint64_t i = 0; i < xl->product; ++i)
                if (state->dense[i])
                    llg_cov_merge_add(&sums, (llg_cov_name_t){0u, i, 0}, state->dense[i], 0);
        } else {
            for (size_t slot = 0; slot < state->sparse.capacity; ++slot)
                if (state->sparse.keys[slot])
                    llg_cov_merge_add(&sums, (llg_cov_name_t){0u, state->sparse.keys[slot] - 1u, 0},
                                      state->sparse.counts[slot], 0);
        }
    }
    for (size_t i = 0; i < sums.capacity; ++i)
        if (sums.slots[i].used && sums.slots[i].sum >= at_least) (*covered)++;
    free(sums.slots);
}

static void llg_cov_merged_item(llg_cov_type_t* type, uint32_t index, uint64_t* covered,
                                uint64_t* defined) {
    *covered = 0;
    *defined = 0;
    if (!type->first) return;
    uint32_t points = type->first->group->n_points;
    if (index >= points) {
        llg_cov_merged_cross(type, index - points, covered, defined);
        return;
    }
    llg_cov_merged_values(type, index, covered, defined);
    llg_cov_merged_transitions(type, index, covered, defined);
}

static int64_t llg_cov_item_type_weight(const llg_cov_group_t* group, uint32_t index) {
    return index < group->n_points ? group->points[index].type_weight
                                   : group->crosses[index - group->n_points].type_weight;
}

// Merged type coverage of one item, or of the group weighted by the items'
// type_option.weight (19.7.1, 19.11.3). Returns 0 for a zero denominator.
static int llg_cov_merged_score(llg_cov_type_t* type, int32_t point, double* score) {
    if (!type->first) return 0;
    const llg_cov_group_t* group = type->first->group;
    uint32_t items = group->n_points + group->n_crosses;
    double numerator = 0.0, denominator = 0.0;
    for (uint32_t i = 0; i < items; ++i) {
        if (point >= 0 && (uint32_t)point != i) continue;
        uint64_t covered, defined;
        llg_cov_merged_item(type, i, &covered, &defined);
        if (!defined) continue;
        double value = 100.0 * (double)covered / (double)defined;
        if (point >= 0) {
            *score = value;
            return 1;
        }
        int64_t weight = llg_cov_item_type_weight(group, i);
        if (weight <= 0) continue;
        numerator += (double)weight * value;
        denominator += (double)weight;
    }
    if (denominator == 0.0) return 0;
    *score = numerator / denominator;
    return 1;
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
    // Coverpoints and crosses, weighted by option.weight (19.11).
    double numerator = 0.0, denominator = 0.0;
    for (uint32_t i = 0; i < inst->group->n_points + inst->group->n_crosses; ++i) {
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
    // Table 19-1 get_inst_coverage: with merged type coverage, an instance
    // reports the type's coverage unless the option is set.
    if (inst->group->type->merge && !inst->get_inst_coverage)
        return llg_cov_type_coverage(inst->group->type, point,
                                     point < 0 ? inst->group->type->weight
                                               : llg_cov_item_type_weight(inst->group,
                                                                          (uint32_t)point));
    double score;
    if (llg_cov_inst_score(inst, point, &score)) return score;
    int64_t weight = point < 0 ? inst->weight : inst->points[point].weight;
    return weight != 0 ? 0.0 : 100.0;
}

// Type coverage with merge_instances = 0 (19.11.3): instances weighted by
// their option.weight (or a coverpoint's option.weight for one coverpoint).
static int llg_cov_type_score(const llg_cov_type_t* type, int32_t point, double* score) {
    if (type->merge) return llg_cov_merged_score((llg_cov_type_t*)type, point, score);
    double numerator = 0.0, denominator = 0.0;
    for (const llg_cov_inst_t* inst = type->first; inst; inst = inst->next) {
        if (point >= 0 && (uint32_t)point >= inst->group->n_points + inst->group->n_crosses)
            continue;
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
                free(inst->points[i].hits);
            }
            for (uint32_t k = 0; k < inst->group->n_crosses; ++k) {
                llg_cov_point_state_t* state = &inst->points[inst->group->n_points + k];
                free(state->dense);
                free(state->sparse.keys);
                free(state->sparse.counts);
                if (state->cross) {
                    free(state->cross->counts);
                    free(state->cross->stamp);
                    free(state->cross);
                }
            }
            free(inst->slots);
            free(inst);
            inst = next;
        }
        type->first = type->last = NULL;
        type->n_instances = 0;
        type->used = 0;
        type->merge_error = 0;
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
        free(llg_cov_cached_groups[i]->xlayouts);
        llg_cov_cached_groups[i]->xlayouts = NULL;
    }
    while (llg_cov_xlayouts_all) {
        llg_cov_xlayout_t* xl = llg_cov_xlayouts_all;
        llg_cov_xlayouts_all = xl->next_all;
        llg_cov_xlayout_free(xl);
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
        for (uint32_t d = 0; d < layout->n_decls; ++d) free(layout->decl_values[d]);
        free(layout->decl_values);
        free(layout->n_decl_values);
        free(layout->decl_base);
        free(layout->decl_size);
        free(layout);
    }
}
