// ── Statement and toggle coverage (IEEE 1800-2009 40.2.2, 40.3.2) ──────────
//
// Statement coverage (decision S46-D13): the generated model counts each
// statement entity in its own `uint64_t` (one increment as the statement
// starts). The runtime never writes those counters: an entity's coverage
// count is its counter minus `base`, read at `mark` while stopped, so
// START, STOP, RESET and merges are offsets kept here, and a stopped
// entity costs its statement the same increment. Counts are modulo 2^64.
//
// Toggle coverage (decision S46-D14): "for each bit of every signal ...
// whether that bit has both a 0 value and a 1 value" (40.2.2). Each bit
// records whether it was seen at 0 and at 1 while collecting, and counts
// its changes from a known 0 to a known 1 and back (X and Z are neither
// value: 0, X, 1 counts one 0->1 change). The value a started signal holds
// counts as seen, later a bit's value counts when it changes; changes are
// watched like FSM state signals
// (llg_value_watch_add), so only started signals pay, one binary search per
// changing publication of their storage. START samples the current value
// without counting a change; RESET clears the sightings and counts and
// leaves the current value unseen until it changes (as A17-D12 does for
// FSM states).

// ── Statement coverage ─────────────────────────────────────────────────────

typedef struct {
    uint64_t base;   // counter value that coverage count 0 corresponds to
    uint64_t mark;   // counter value when the entity stopped
    uint8_t stopped;
} llg_scov_state_t;

static const llg_scov_item_t* llg_scov_items;
static size_t llg_scov_n;
static llg_scov_state_t* llg_scov_states;
// (code row, entity) pairs sorted by code row, built on the first VPI query.
static uint64_t* llg_scov_by_code;
static size_t llg_scov_n_by_code;
static int llg_scov_indexed;

void llg_scov_model(const llg_scov_item_t* items, size_t n_items, int started) {
    free(llg_scov_states);
    free(llg_scov_by_code);
    llg_scov_by_code = NULL;
    llg_scov_n_by_code = 0;
    llg_scov_indexed = 0;
    llg_scov_items = items;
    llg_scov_n = items ? n_items : 0u;
    llg_scov_states = llg_scov_n ? (llg_scov_state_t*)llg_checked_calloc(
                                       llg_scov_n, sizeof(*llg_scov_states), "statement coverage")
                                 : NULL;
    // A model started again keeps its counters: coverage restarts at 0.
    for (size_t i = 0; i < llg_scov_n; ++i) {
        uint64_t now = *llg_scov_items[i].count;
        llg_scov_states[i] = (llg_scov_state_t){now, now, (uint8_t)!started};
    }
}

static uint64_t llg_scov_value(size_t entity) {
    const llg_scov_state_t* state = &llg_scov_states[entity];
    uint64_t raw = state->stopped ? state->mark : *llg_scov_items[entity].count;
    return raw - state->base;
}

uint64_t llg_scov_hits(size_t entity) { return entity < llg_scov_n ? llg_scov_value(entity) : 0u; }

static void llg_scov_control(size_t entity, uint32_t operation) {
    llg_scov_state_t* state = &llg_scov_states[entity];
    uint64_t now = *llg_scov_items[entity].count;
    if (operation == LLG_SV_COV_START && state->stopped) {
        state->base += now - state->mark;
        state->stopped = 0;
    } else if (operation == LLG_SV_COV_STOP && !state->stopped) {
        state->mark = now;
        state->stopped = 1;
    } else if (operation == LLG_SV_COV_RESET) {
        state->base = state->stopped ? state->mark : now;
    }
}

// Visit the statement entities in scope (llg_acov_in_scope); `operation` is
// START, STOP or RESET, or UINT32_MAX to only tally them.
static llg_ccov_tally_t llg_scov_scope(int hier, int32_t scope, const char* definition,
                                       uint32_t operation) {
    llg_ccov_tally_t tally = {0, 0};
    for (size_t i = 0; i < llg_scov_n; ++i) {
        if (!llg_acov_in_scope(llg_scov_items[i].instance, hier, scope, definition)) continue;
        llg_scov_control(i, operation);
        tally.items++;
        tally.covered += llg_scov_value(i) != 0;
    }
    return tally;
}

size_t llg_scov_covered_in(int32_t instance) {
    return llg_scov_scope(0, instance, NULL, UINT32_MAX).covered;
}

static int llg_scov_code_compare(const void* left, const void* right) {
    uint64_t a = *(const uint64_t*)left, b = *(const uint64_t*)right;
    return a < b ? -1 : a > b ? 1 : 0;
}

int64_t llg_scov_at_code(uint32_t code_row) {
    if (!llg_scov_indexed) {
        llg_scov_indexed = 1;
        for (size_t i = 0; i < llg_scov_n; ++i) llg_scov_n_by_code += llg_scov_items[i].code != 0;
        if (llg_scov_n_by_code) {
            llg_scov_by_code = (uint64_t*)llg_checked_malloc(
                llg_scov_n_by_code, sizeof(*llg_scov_by_code), "statement coverage");
            size_t at = 0;
            for (size_t i = 0; i < llg_scov_n; ++i)
                if (llg_scov_items[i].code)
                    llg_scov_by_code[at++] = ((uint64_t)(llg_scov_items[i].code - 1u) << 32) | i;
            qsort(llg_scov_by_code, llg_scov_n_by_code, sizeof(*llg_scov_by_code),
                  llg_scov_code_compare);
        }
    }
    size_t lo = 0, hi = llg_scov_n_by_code;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        uint32_t row = (uint32_t)(llg_scov_by_code[mid] >> 32);
        if (row == code_row) return (int64_t)(llg_scov_by_code[mid] & UINT32_MAX);
        if (row < code_row) lo = mid + 1u;
        else hi = mid;
    }
    return -1;
}

// ── Statement coverage database section ─────────────────────────────────────

static int llg_scov_available(void) { return llg_scov_n != 0; }

static const char* llg_ccov_instance_path(int32_t instance) {
    return instance >= 0 && (size_t)instance < llg_acov_n_instances
               ? llg_acov_instances[instance].path
               : "$root";
}

// The file name without its directories: the same design compiled from
// another directory keeps its identity.
static const char* llg_ccov_base_name(const char* file) {
    const char* base = file ? file : "";
    for (const char* at = base; *at; ++at)
        if (*at == '/' || *at == '\\') base = at + 1;
    return base;
}

// Identity of the statement table: per entity its instance and position.
static uint64_t llg_scov_schema_hash(void) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_u64(&hash, (uint64_t)llg_scov_n);
    for (size_t i = 0; i < llg_scov_n; ++i) {
        const llg_scov_item_t* item = &llg_scov_items[i];
        llg_cov_fnv_text(&hash, llg_ccov_instance_path(item->instance));
        llg_cov_fnv_text(&hash, llg_ccov_base_name(item->file));
        llg_cov_fnv_u64(&hash, item->line);
        llg_cov_fnv_u64(&hash, item->column);
    }
    return hash;
}

// u64 schema hash, u32 entities, then one u64 count per entity.
static void llg_scov_put_section(llg_cov_buf_t* buf) {
    llg_cov_put_u64(buf, llg_scov_schema_hash());
    llg_cov_put_u32(buf, (uint32_t)llg_scov_n);
    for (size_t i = 0; i < llg_scov_n; ++i) llg_cov_put_u64(buf, llg_scov_value(i));
}

// Validate the statement section of `db`, then add its counts. Returns NULL
// on success, else the reason; nothing changes on failure.
static const char* llg_scov_merge_section(const llg_cov_db_file_t* db) {
    llg_cov_reader_t reader = {db->data, db->statement_end, db->statement_at, NULL};
    uint64_t hash;
    uint32_t n;
    if (!llg_cov_get_u64(&reader, &hash) || !llg_cov_get_u32(&reader, &n))
        return "the statement section is malformed";
    if (hash != llg_scov_schema_hash() || (size_t)n != llg_scov_n)
        return "it was saved by a model with different statements";
    if (reader.len - reader.pos != (size_t)n * 8u)
        return "the statement section does not match its statement table";
    for (size_t i = 0; i < llg_scov_n; ++i) {
        uint64_t value = 0;
        (void)llg_cov_get_u64(&reader, &value);
        llg_scov_states[i].base -= value;
    }
    return NULL;
}

// ── Toggle coverage ─────────────────────────────────────────────────────────

// Per signal: `words` 64-bit words each of the last known value, the bits
// that have one, the bits seen at 0 and at 1, then `width` 0->1 counts and
// `width` 1->0 counts, in one allocation.
typedef struct {
    uint64_t* data;
    uint8_t stopped;
} llg_tcov_state_t;

static const llg_tcov_signal_t* llg_tcov_signals;
static size_t llg_tcov_n;
// The runtime ended: its watches are gone and none may be added.
static int llg_tcov_ended;
static llg_tcov_state_t* llg_tcov_states;
static uint64_t* llg_tcov_arena;

// One watched storage address and a signal that reads it, sorted by address.
typedef struct {
    const void* key;
    uint32_t signal;
} llg_tcov_watch_t;

static llg_tcov_watch_t* llg_tcov_watches;
static size_t llg_tcov_n_watches;
// (catalog row, signal) pairs sorted by row, built on the first VPI query.
static uint64_t* llg_tcov_by_object;
static size_t llg_tcov_n_by_object;
static int llg_tcov_indexed;

static size_t llg_tcov_words(const llg_tcov_signal_t* signal) {
    return ((size_t)signal->width + 63u) / 64u;
}

#define LLG_TCOV_LAST(data, words) (data)
#define LLG_TCOV_VALID(data, words) ((data) + (words))
#define LLG_TCOV_SEEN0(data, words) ((data) + 2u * (words))
#define LLG_TCOV_SEEN1(data, words) ((data) + 3u * (words))
#define LLG_TCOV_RISES(data, words) ((data) + 4u * (words))

static unsigned llg_tcov_lowest_bit(uint64_t value) {
#if defined(__GNUC__) || defined(__clang__)
    return (unsigned)__builtin_ctzll(value);
#else
    unsigned bit = 0;
    while (!(value & 1u)) {
        value >>= 1;
        ++bit;
    }
    return bit;
#endif
}

// Take the storage's value. Without `count_changes` (a start) every known
// bit counts as seen at its value; otherwise only a bit whose known value
// changed (or that has none yet) does, and a change between 0 and 1 counts.
static void llg_tcov_sample(size_t index, int count_changes) {
    const llg_tcov_signal_t* signal = &llg_tcov_signals[index];
    uint64_t* data = llg_tcov_states[index].data;
    size_t words = llg_tcov_words(signal);
    uint64_t* last = LLG_TCOV_LAST(data, words);
    uint64_t* valid = LLG_TCOV_VALID(data, words);
    uint64_t* seen0 = LLG_TCOV_SEEN0(data, words);
    uint64_t* seen1 = LLG_TCOV_SEEN1(data, words);
    uint64_t* rises = LLG_TCOV_RISES(data, words);
    uint64_t* falls = rises + signal->width;
    sv4_t value = *signal->storage;
    for (size_t w = 0; w < words; ++w) {
        unsigned tail = (unsigned)(signal->width - 64u * w);
        uint64_t mask = tail < 64u ? (UINT64_C(1) << tail) - 1u : UINT64_MAX;
        uint64_t unknown =
            llg_sv4_word(value, w, LLG_SV4_X) | llg_sv4_word(value, w, LLG_SV4_Z);
        uint64_t known = ~unknown & mask;
        uint64_t bits = llg_sv4_word(value, w, LLG_SV4_BITS);
        uint64_t one = bits & known, zero = ~bits & known;
        uint64_t observed = count_changes ? known & ~(valid[w] & ~(last[w] ^ bits)) : known;
        seen0[w] |= zero & observed;
        seen1[w] |= one & observed;
        if (count_changes) {
            uint64_t rise = valid[w] & ~last[w] & one;
            uint64_t fall = valid[w] & last[w] & zero;
            for (; rise; rise &= rise - 1u) {
                uint64_t* counter = &rises[64u * w + llg_tcov_lowest_bit(rise)];
                if (*counter != UINT64_MAX) ++*counter;
            }
            for (; fall; fall &= fall - 1u) {
                uint64_t* counter = &falls[64u * w + llg_tcov_lowest_bit(fall)];
                if (*counter != UINT64_MAX) ++*counter;
            }
        }
        last[w] = (last[w] & ~known) | one;
        valid[w] |= known;
    }
}

static void llg_tcov_start(size_t index) {
    llg_tcov_state_t* state = &llg_tcov_states[index];
    if (!state->stopped || llg_tcov_ended) return;
    state->stopped = 0;
    (void)llg_value_watch_add(llg_tcov_signals[index].storage);
    // Changes while stopped are not counted: the value now is the new base.
    size_t words = llg_tcov_words(&llg_tcov_signals[index]);
    memset(LLG_TCOV_VALID(state->data, words), 0, words * sizeof(uint64_t));
    llg_tcov_sample(index, 0);
}

static void llg_tcov_stop(size_t index) {
    llg_tcov_state_t* state = &llg_tcov_states[index];
    if (state->stopped) return;
    state->stopped = 1;
    if (!llg_tcov_ended) llg_value_watch_remove(llg_tcov_signals[index].storage);
}

static void llg_tcov_reset(size_t index) {
    const llg_tcov_signal_t* signal = &llg_tcov_signals[index];
    size_t words = llg_tcov_words(signal);
    uint64_t* data = llg_tcov_states[index].data;
    memset(LLG_TCOV_SEEN0(data, words), 0,
           (2u * words + 2u * (size_t)signal->width) * sizeof(uint64_t));
}

static int llg_tcov_watch_compare(const void* left, const void* right) {
    const llg_tcov_watch_t* a = (const llg_tcov_watch_t*)left;
    const llg_tcov_watch_t* b = (const llg_tcov_watch_t*)right;
    uintptr_t ka = (uintptr_t)a->key, kb = (uintptr_t)b->key;
    if (ka != kb) return ka < kb ? -1 : 1;
    return a->signal < b->signal ? -1 : a->signal > b->signal ? 1 : 0;
}

static void llg_tcov_release(void);

void llg_tcov_model(const llg_tcov_signal_t* signals, size_t n_signals, int started) {
    llg_tcov_release();
    llg_tcov_ended = 0;
    llg_tcov_signals = signals;
    llg_tcov_n = signals ? n_signals : 0u;
    if (!llg_tcov_n) return;
    size_t total = 0;
    for (size_t i = 0; i < llg_tcov_n; ++i) {
        size_t cells = 4u * llg_tcov_words(&signals[i]) + 2u * (size_t)signals[i].width;
        if (cells > SIZE_MAX - total) llg_fatal_allocation("toggle coverage", total, cells);
        total += cells;
    }
    llg_tcov_arena = (uint64_t*)llg_checked_calloc(total, sizeof(uint64_t), "toggle coverage");
    llg_tcov_states =
        (llg_tcov_state_t*)llg_checked_calloc(llg_tcov_n, sizeof(*llg_tcov_states), "toggle coverage");
    llg_tcov_watches =
        (llg_tcov_watch_t*)llg_checked_malloc(llg_tcov_n, sizeof(*llg_tcov_watches), "toggle coverage");
    size_t at = 0;
    for (size_t i = 0; i < llg_tcov_n; ++i) {
        llg_tcov_states[i] = (llg_tcov_state_t){llg_tcov_arena + at, 1u};
        at += 4u * llg_tcov_words(&signals[i]) + 2u * (size_t)signals[i].width;
        llg_tcov_watches[i] = (llg_tcov_watch_t){signals[i].storage, (uint32_t)i};
    }
    llg_tcov_n_watches = llg_tcov_n;
    qsort(llg_tcov_watches, llg_tcov_n_watches, sizeof(*llg_tcov_watches), llg_tcov_watch_compare);
    value_watch_toggle = 1;
    if (started)
        for (size_t i = 0; i < llg_tcov_n; ++i) llg_tcov_start(i);
}

// A watched address changed: sample the started signals stored there.
static void llg_tcov_changed(const void* key) {
    size_t lo = 0, hi = llg_tcov_n_watches;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        if ((uintptr_t)llg_tcov_watches[mid].key < (uintptr_t)key) lo = mid + 1u;
        else hi = mid;
    }
    for (size_t i = lo; i < llg_tcov_n_watches && llg_tcov_watches[i].key == key; ++i) {
        uint32_t index = llg_tcov_watches[i].signal;
        if (!llg_tcov_states[index].stopped) llg_tcov_sample(index, 1);
    }
}

// The scheduler releases the watches themselves (free_value_watches). The
// signal state stays until the next model binds, so VPI applications read
// the final coverage in cbEndOfSimulation; the end-of-run database save ran
// before the runtime cleanup.
static void llg_tcov_release(void) {
    value_watch_toggle = 0;
    free(llg_tcov_arena);
    free(llg_tcov_states);
    free(llg_tcov_watches);
    free(llg_tcov_by_object);
    llg_tcov_arena = NULL;
    llg_tcov_states = NULL;
    llg_tcov_watches = NULL;
    llg_tcov_by_object = NULL;
    llg_tcov_n_watches = 0;
    llg_tcov_n_by_object = 0;
    llg_tcov_indexed = 0;
    llg_tcov_n = 0;
}

// At runtime cleanup: collection ends (the scheduler frees the watches);
// the coverage itself stays readable.
static void llg_tcov_finish(void) {
    for (size_t i = 0; i < llg_tcov_n; ++i) llg_tcov_states[i].stopped = 1;
    llg_tcov_ended = 1;
    value_watch_toggle = 0;
}

// Covered bits and 0/1 changes of bits [lsb, lsb + n) of `signal`.
void llg_tcov_bits(size_t signal, uint32_t lsb, uint32_t n, uint64_t* covered, uint64_t* toggles) {
    *covered = *toggles = 0;
    if (signal >= llg_tcov_n) return;
    const llg_tcov_signal_t* row = &llg_tcov_signals[signal];
    size_t words = llg_tcov_words(row);
    const uint64_t* data = llg_tcov_states[signal].data;
    const uint64_t* seen0 = LLG_TCOV_SEEN0(data, words);
    const uint64_t* seen1 = LLG_TCOV_SEEN1(data, words);
    const uint64_t* rises = LLG_TCOV_RISES(data, words);
    const uint64_t* falls = rises + row->width;
    for (uint32_t bit = lsb; bit < row->width && bit - lsb < n; ++bit) {
        uint64_t mask = UINT64_C(1) << (bit % 64u);
        *covered += (seen0[bit / 64u] & seen1[bit / 64u] & mask) != 0;
        uint64_t changes = rises[bit] > UINT64_MAX - falls[bit] ? UINT64_MAX : rises[bit] + falls[bit];
        *toggles = changes > UINT64_MAX - *toggles ? UINT64_MAX : *toggles + changes;
    }
}

uint32_t llg_tcov_width(size_t signal) { return signal < llg_tcov_n ? llg_tcov_signals[signal].width : 0u; }

// Visit the toggle signals in scope; the tally counts their bits.
static llg_ccov_tally_t llg_tcov_scope(int hier, int32_t scope, const char* definition,
                                       uint32_t operation) {
    llg_ccov_tally_t tally = {0, 0};
    for (size_t i = 0; i < llg_tcov_n; ++i) {
        const llg_tcov_signal_t* signal = &llg_tcov_signals[i];
        if (!llg_acov_in_scope(signal->instance, hier, scope, definition)) continue;
        if (operation == LLG_SV_COV_START) llg_tcov_start(i);
        else if (operation == LLG_SV_COV_STOP) llg_tcov_stop(i);
        else if (operation == LLG_SV_COV_RESET) llg_tcov_reset(i);
        uint64_t covered, toggles;
        llg_tcov_bits(i, 0, signal->width, &covered, &toggles);
        tally.items += signal->width;
        tally.covered += (size_t)covered;
    }
    return tally;
}

size_t llg_tcov_covered_in(int32_t instance) {
    return llg_tcov_scope(0, instance, NULL, UINT32_MAX).covered;
}

int64_t llg_tcov_at_object(int32_t row) {
    if (row < 0) return -1;
    if (!llg_tcov_indexed) {
        llg_tcov_indexed = 1;
        for (size_t i = 0; i < llg_tcov_n; ++i) llg_tcov_n_by_object += llg_tcov_signals[i].object >= 0;
        if (llg_tcov_n_by_object) {
            llg_tcov_by_object = (uint64_t*)llg_checked_malloc(
                llg_tcov_n_by_object, sizeof(*llg_tcov_by_object), "toggle coverage");
            size_t at = 0;
            for (size_t i = 0; i < llg_tcov_n; ++i)
                if (llg_tcov_signals[i].object >= 0)
                    llg_tcov_by_object[at++] = ((uint64_t)llg_tcov_signals[i].object << 32) | i;
            qsort(llg_tcov_by_object, llg_tcov_n_by_object, sizeof(*llg_tcov_by_object),
                  llg_scov_code_compare);
        }
    }
    size_t lo = 0, hi = llg_tcov_n_by_object;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        uint32_t object = (uint32_t)(llg_tcov_by_object[mid] >> 32);
        if (object == (uint32_t)row) return (int64_t)(llg_tcov_by_object[mid] & UINT32_MAX);
        if (object < (uint32_t)row) lo = mid + 1u;
        else hi = mid;
    }
    return -1;
}

// ── Toggle coverage database section ────────────────────────────────────────

static int llg_tcov_available(void) { return llg_tcov_n != 0; }

// Identity of the toggle table: per signal its name and width.
static uint64_t llg_tcov_schema_hash(void) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_u64(&hash, (uint64_t)llg_tcov_n);
    for (size_t i = 0; i < llg_tcov_n; ++i) {
        llg_cov_fnv_text(&hash, llg_tcov_signals[i].full_name);
        llg_cov_fnv_u64(&hash, llg_tcov_signals[i].width);
    }
    return hash;
}

// u64 schema hash, u32 signals, then per signal its u32 width and per bit
// (least significant first) one byte of sightings (1: seen at 0, 2: seen at
// 1), its u64 0->1 count and its u64 1->0 count.
static void llg_tcov_put_section(llg_cov_buf_t* buf) {
    llg_cov_put_u64(buf, llg_tcov_schema_hash());
    llg_cov_put_u32(buf, (uint32_t)llg_tcov_n);
    for (size_t i = 0; i < llg_tcov_n; ++i) {
        const llg_tcov_signal_t* signal = &llg_tcov_signals[i];
        size_t words = llg_tcov_words(signal);
        const uint64_t* data = llg_tcov_states[i].data;
        const uint64_t* rises = LLG_TCOV_RISES(data, words);
        llg_cov_put_u32(buf, signal->width);
        for (uint32_t bit = 0; bit < signal->width; ++bit) {
            uint64_t mask = UINT64_C(1) << (bit % 64u);
            unsigned char seen =
                (unsigned char)(((LLG_TCOV_SEEN0(data, words)[bit / 64u] & mask) != 0) |
                                ((LLG_TCOV_SEEN1(data, words)[bit / 64u] & mask) != 0) << 1);
            llg_cov_put(buf, &seen, 1);
            llg_cov_put_u64(buf, rises[bit]);
            llg_cov_put_u64(buf, rises[signal->width + bit]);
        }
    }
}

// Validate the toggle section of `db` completely, then add it. Returns NULL
// on success, else the reason; nothing changes on failure.
static const char* llg_tcov_merge_section(const llg_cov_db_file_t* db) {
    llg_cov_reader_t reader = {db->data, db->toggle_end, db->toggle_at, NULL};
    uint64_t hash;
    uint32_t n;
    if (!llg_cov_get_u64(&reader, &hash) || !llg_cov_get_u32(&reader, &n))
        return "the toggle section is malformed";
    if (hash != llg_tcov_schema_hash() || (size_t)n != llg_tcov_n)
        return "it was saved by a model with different signals";
    size_t start = reader.pos;
    for (int pass = 0; pass < 2; ++pass) {
        reader.pos = start;
        for (size_t i = 0; i < llg_tcov_n; ++i) {
            const llg_tcov_signal_t* signal = &llg_tcov_signals[i];
            size_t words = llg_tcov_words(signal);
            uint64_t* data = llg_tcov_states[i].data;
            uint64_t* rises = LLG_TCOV_RISES(data, words);
            uint32_t width = 0;
            if (!llg_cov_get_u32(&reader, &width) || width != signal->width)
                return "the toggle section does not match its signal table";
            for (uint32_t bit = 0; bit < width; ++bit) {
                unsigned char seen = 0;
                uint64_t up, down;
                if (!llg_cov_get(&reader, &seen, 1) || seen > 3u || !llg_cov_get_u64(&reader, &up) ||
                    !llg_cov_get_u64(&reader, &down))
                    return "the toggle section does not match its signal table";
                if (!pass) continue;
                uint64_t mask = UINT64_C(1) << (bit % 64u);
                if (seen & 1u) LLG_TCOV_SEEN0(data, words)[bit / 64u] |= mask;
                if (seen & 2u) LLG_TCOV_SEEN1(data, words)[bit / 64u] |= mask;
                llg_acov_add(&rises[bit], up);
                llg_acov_add(&rises[width + bit], down);
            }
        }
        if (reader.pos != reader.len) return "the toggle section does not match its signal table";
    }
    return NULL;
}
