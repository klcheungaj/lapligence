// ── FSM coverage (IEEE 1800-2009 40.4, 40.5.3) ──────────────────────────────
//
// The generated model owns one llg_fcov_fsm_t per FSM that the 40.4 pragmas
// declare (a state vector and the parameters naming its states, in one
// instance), present only when the model keeps its coverage tables. FSM
// coverage is "the number of states in an FSM that this simulation reached"
// (40.2.2): each FSM counts, per state, how often its state expression took
// that state's value.
//
// The monitor rides on the value-change watches of the scheduler
// (llg_value_watch_add): each part's storage is watched, and a changing
// publication of a watched address calls llg_fcov_changed after every other
// runtime consumer saw the new value. Only the FSMs' own signals pay, one
// binary search per changing publication; a model without FSMs never
// watches anything here. A stopped FSM drops its watches.
//
// Decision A17-D12: coverage starts with the state the expression holds
// when collection starts (after the storage initializers, or at START), and
// every later change into a state counts one visit, intermediate values of
// one time step included. A value with unknown bits, or that no state names,
// leaves the FSM in no state. A change from one state directly into another
// counts that transition (40.2.1 b) 3) ii)); entering a state from no state,
// or at START, counts none. RESET zeroes the visits and transitions and
// keeps the current state uncounted.

static llg_fcov_fsm_t* llg_fcov_fsms;
static size_t llg_fcov_n_fsms;

// One observed transition of an FSM: state indexes and its count.
typedef struct llg_fcov_transition {
    uint32_t from;
    uint32_t to;
    uint64_t count;
} llg_fcov_transition_t;

// The row of transition (from, to) in `fsm`, inserted with count 0 when
// `insert` is set; NULL when absent and not inserted. Rows stay sorted, so
// a lookup is a binary search; an FSM has few distinct transitions, and a
// new one is rare after the first cycles.
static llg_fcov_transition_t* llg_fcov_transition(llg_fcov_fsm_t* fsm, uint32_t from,
                                                  uint32_t to, int insert) {
    uint64_t key = ((uint64_t)from << 32) | to;
    uint32_t lo = 0, hi = fsm->n_transitions;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2u;
        const llg_fcov_transition_t* row = &fsm->transitions[mid];
        if ((((uint64_t)row->from << 32) | row->to) < key) lo = mid + 1u;
        else hi = mid;
    }
    if (lo < fsm->n_transitions && fsm->transitions[lo].from == from &&
        fsm->transitions[lo].to == to)
        return &fsm->transitions[lo];
    if (!insert) return NULL;
    if (fsm->n_transitions == fsm->transition_capacity) {
        if (fsm->transition_capacity > UINT32_MAX / 2u)
            llg_fatal_allocation("FSM coverage", fsm->transition_capacity,
                                 2u * sizeof(*fsm->transitions));
        uint32_t capacity = fsm->transition_capacity ? fsm->transition_capacity * 2u : 8u;
        llg_fcov_transition_t* grown = (llg_fcov_transition_t*)llg_checked_malloc(
            capacity, sizeof(*grown), "FSM coverage");
        if (fsm->n_transitions)
            memcpy(grown, fsm->transitions, (size_t)fsm->n_transitions * sizeof(*grown));
        free(fsm->transitions);
        fsm->transitions = grown;
        fsm->transition_capacity = capacity;
    }
    memmove(&fsm->transitions[lo + 1u], &fsm->transitions[lo],
            (size_t)(fsm->n_transitions - lo) * sizeof(*fsm->transitions));
    fsm->n_transitions++;
    fsm->transitions[lo] = (llg_fcov_transition_t){from, to, 0};
    return &fsm->transitions[lo];
}

static void llg_fcov_free_transitions(llg_fcov_fsm_t* fsm) {
    free(fsm->transitions);
    fsm->transitions = NULL;
    fsm->n_transitions = 0;
    fsm->transition_capacity = 0;
}

// One watched storage address and an FSM that reads it, sorted by address.
typedef struct {
    const void* key;
    uint32_t fsm;
} llg_fcov_watch_t;

static llg_fcov_watch_t* llg_fcov_watches;
static size_t llg_fcov_n_watches;
static size_t llg_fcov_watch_capacity;

// Bits [lsb, lsb + size) of one plane of `value`, size 1 to 64.
static uint64_t llg_fcov_bits(sv4_t value, uint32_t lsb, uint32_t size, unsigned plane) {
    size_t word = lsb / 64u;
    unsigned offset = lsb % 64u;
    uint64_t bits = llg_sv4_word(value, word, plane) >> offset;
    if (offset && offset + size > 64u) bits |= llg_sv4_word(value, word + 1u, plane) << (64u - offset);
    return size < 64u ? bits & ((UINT64_C(1) << size) - 1u) : bits;
}

// The state expression of `fsm`, or 0 with `*known` cleared when a bit is X
// or Z. Reads the parts' storage words in place: no value is built.
static uint64_t llg_fcov_read(const llg_fcov_fsm_t* fsm, int* known) {
    uint64_t value = 0;
    *known = 1;
    for (uint32_t i = 0; i < fsm->n_parts; ++i) {
        const llg_fcov_part_t* part = &fsm->parts[i];
        sv4_t storage = *part->storage;
        if (llg_fcov_bits(storage, part->lsb, part->size, LLG_SV4_X) |
            llg_fcov_bits(storage, part->lsb, part->size, LLG_SV4_Z)) {
            *known = 0;
            return 0;
        }
        uint64_t word = llg_fcov_bits(storage, part->lsb, part->size, LLG_SV4_BITS);
        value = part->size >= 64u ? word : (value << part->size) | word;
    }
    return value;
}

static int32_t llg_fcov_state_of(const llg_fcov_fsm_t* fsm, uint64_t value) {
    for (uint32_t i = 0; i < fsm->n_states; ++i)
        if (fsm->states[i].value == value) return (int32_t)i;
    return -1;
}

// Take the state expression's value now and remember where the FSM is.
// Being in a state counts one visit, except that with `only_changes` an
// unchanged value counts nothing.
static void llg_fcov_sample(llg_fcov_fsm_t* fsm, int only_changes) {
    int known;
    uint64_t value = llg_fcov_read(fsm, &known);
    if (only_changes && known == (int)fsm->known && (!known || value == fsm->value)) return;
    int32_t previous = only_changes ? fsm->current : -1;
    fsm->known = (uint8_t)known;
    fsm->value = value;
    fsm->current = known ? llg_fcov_state_of(fsm, value) : -1;
    if (fsm->current >= 0) {
        llg_acov_add(&fsm->visits[fsm->current], 1u);
        if (previous >= 0 && previous != fsm->current)
            llg_acov_add(
                &llg_fcov_transition(fsm, (uint32_t)previous, (uint32_t)fsm->current, 1)->count,
                1u);
    }
}

static int llg_fcov_watch_compare(const void* left, const void* right) {
    const llg_fcov_watch_t* a = (const llg_fcov_watch_t*)left;
    const llg_fcov_watch_t* b = (const llg_fcov_watch_t*)right;
    uintptr_t ka = (uintptr_t)a->key, kb = (uintptr_t)b->key;
    if (ka != kb) return ka < kb ? -1 : 1;
    return a->fsm < b->fsm ? -1 : a->fsm > b->fsm ? 1 : 0;
}

// Watch (`on`) or stop watching the parts of FSM `index`. Two parts of one
// signal share one watch.
static void llg_fcov_watch(size_t index, int on) {
    llg_fcov_fsm_t* fsm = &llg_fcov_fsms[index];
    for (uint32_t i = 0; i < fsm->n_parts; ++i) {
        const void* key = fsm->parts[i].storage;
        int repeated = 0;
        for (uint32_t j = 0; j < i; ++j) repeated |= fsm->parts[j].storage == fsm->parts[i].storage;
        if (repeated) continue;
        if (on) (void)llg_value_watch_add(key);
        else llg_value_watch_remove(key);
    }
}

// FSM `index` (re)starts collecting: watch its storage and take its state.
static void llg_fcov_start(size_t index) {
    llg_fcov_fsm_t* fsm = &llg_fcov_fsms[index];
    fsm->stopped = 0;
    llg_fcov_watch(index, 1);
    fsm->known = 0;
    llg_fcov_sample(fsm, 0);
}

static void llg_fcov_stop(size_t index) {
    llg_fcov_fsm_t* fsm = &llg_fcov_fsms[index];
    if (fsm->stopped) return;
    fsm->stopped = 1;
    llg_fcov_watch(index, 0);
}

void llg_fcov_model(llg_fcov_fsm_t* fsms, size_t n_fsms) {
    llg_fcov_fsms = fsms;
    llg_fcov_n_fsms = fsms ? n_fsms : 0u;
    llg_fcov_n_watches = 0;
    size_t parts = 0;
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) parts += llg_fcov_fsms[i].n_parts;
    if (parts > llg_fcov_watch_capacity) {
        free(llg_fcov_watches);
        llg_fcov_watches =
            (llg_fcov_watch_t*)llg_checked_malloc(parts, sizeof(*llg_fcov_watches), "FSM coverage");
        llg_fcov_watch_capacity = parts;
    }
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) {
        llg_fcov_fsm_t* fsm = &llg_fcov_fsms[i];
        for (uint32_t s = 0; s < fsm->n_states; ++s) fsm->visits[s] = 0;
        llg_fcov_free_transitions(fsm);
        for (uint32_t p = 0; p < fsm->n_parts; ++p)
            llg_fcov_watches[llg_fcov_n_watches++] =
                (llg_fcov_watch_t){fsm->parts[p].storage, (uint32_t)i};
    }
    if (llg_fcov_n_watches > 1u)
        qsort(llg_fcov_watches, llg_fcov_n_watches, sizeof(*llg_fcov_watches),
              llg_fcov_watch_compare);
    value_watch_fsm = llg_fcov_n_fsms != 0;
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) llg_fcov_start(i);
}

// A watched address changed: resample the running FSMs that read it.
static void llg_fcov_changed(const void* key) {
    size_t lo = 0, hi = llg_fcov_n_watches;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        if ((uintptr_t)llg_fcov_watches[mid].key < (uintptr_t)key) lo = mid + 1u;
        else hi = mid;
    }
    uint32_t last = UINT32_MAX;
    for (size_t i = lo; i < llg_fcov_n_watches && llg_fcov_watches[i].key == key; ++i) {
        uint32_t index = llg_fcov_watches[i].fsm;
        if (index == last) continue;  // two parts of one signal
        last = index;
        llg_fcov_fsm_t* fsm = &llg_fcov_fsms[index];
        if (!fsm->stopped) llg_fcov_sample(fsm, 1);
    }
}

// The scheduler releases the watches themselves (free_value_watches). The
// table and its visits stay bound for VPI applications in
// cbEndOfSimulation; the end-of-run database save ran before this.
static void llg_fcov_release(void) {
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) llg_fcov_free_transitions(&llg_fcov_fsms[i]);
    value_watch_fsm = 0;
    free(llg_fcov_watches);
    llg_fcov_watches = NULL;
    llg_fcov_n_watches = 0;
    llg_fcov_watch_capacity = 0;
}

static void llg_fcov_reset(llg_fcov_fsm_t* fsm) {
    for (uint32_t s = 0; s < fsm->n_states; ++s) fsm->visits[s] = 0;
    fsm->n_transitions = 0;
}

static uint32_t llg_fcov_reached(const llg_fcov_fsm_t* fsm) {
    uint32_t reached = 0;
    for (uint32_t s = 0; s < fsm->n_states; ++s) reached += fsm->visits[s] != 0;
    return reached;
}

// Visit the FSMs in scope (llg_acov_in_scope); `operation` is START, STOP or
// RESET, or UINT32_MAX to only tally them.
static llg_fcov_tally_t llg_fcov_scope(int hier, int32_t scope, const char* definition,
                                       uint32_t operation) {
    llg_fcov_tally_t tally = {0, 0, 0};
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) {
        llg_fcov_fsm_t* fsm = &llg_fcov_fsms[i];
        if (!llg_acov_in_scope(fsm->instance, hier, scope, definition)) continue;
        if (operation == LLG_SV_COV_START && fsm->stopped) llg_fcov_start(i);
        else if (operation == LLG_SV_COV_STOP) llg_fcov_stop(i);
        else if (operation == LLG_SV_COV_RESET) llg_fcov_reset(fsm);
        tally.fsms++;
        tally.states += fsm->n_states;
        tally.reached += llg_fcov_reached(fsm);
    }
    return tally;
}

// ── Database section ────────────────────────────────────────────────────────

static int llg_fcov_available(void) { return llg_fcov_n_fsms != 0; }

// Identity of the FSM table: per FSM its name, instance, width and states.
static uint64_t llg_fcov_schema_hash(void) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_u64(&hash, (uint64_t)llg_fcov_n_fsms);
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) {
        const llg_fcov_fsm_t* fsm = &llg_fcov_fsms[i];
        llg_cov_fnv_text(&hash, fsm->full_name);
        llg_cov_fnv_u64(&hash, fsm->size);
        llg_cov_fnv_u64(&hash, fsm->n_states);
        for (uint32_t s = 0; s < fsm->n_states; ++s) {
            llg_cov_fnv_text(&hash, fsm->states[s].name);
            llg_cov_fnv_u64(&hash, fsm->states[s].value);
        }
    }
    return hash;
}

// u64 schema hash, u32 FSMs, then per FSM: u32 states, one u64 visit count
// per state, u32 transitions and per transition (sorted by from, then to)
// u32 from, u32 to and its u64 count.
static void llg_fcov_put_section(llg_cov_buf_t* buf) {
    llg_cov_put_u64(buf, llg_fcov_schema_hash());
    llg_cov_put_u32(buf, (uint32_t)llg_fcov_n_fsms);
    for (size_t i = 0; i < llg_fcov_n_fsms; ++i) {
        const llg_fcov_fsm_t* fsm = &llg_fcov_fsms[i];
        llg_cov_put_u32(buf, fsm->n_states);
        for (uint32_t s = 0; s < fsm->n_states; ++s) llg_cov_put_u64(buf, fsm->visits[s]);
        llg_cov_put_u32(buf, fsm->n_transitions);
        for (uint32_t t = 0; t < fsm->n_transitions; ++t) {
            llg_cov_put_u32(buf, fsm->transitions[t].from);
            llg_cov_put_u32(buf, fsm->transitions[t].to);
            llg_cov_put_u64(buf, fsm->transitions[t].count);
        }
    }
}

// Validate the FSM section of `db` completely, then add its visits.
// Returns NULL on success, else the reason; nothing changes on failure.
static const char* llg_fcov_merge_section(const llg_cov_db_file_t* db) {
    llg_cov_reader_t reader = {db->data, db->fsm_end, db->fsm_at, NULL};
    uint64_t hash;
    uint32_t n;
    if (!llg_cov_get_u64(&reader, &hash) || !llg_cov_get_u32(&reader, &n))
        return "the FSM section is malformed";
    if (hash != llg_fcov_schema_hash() || (size_t)n != llg_fcov_n_fsms)
        return "it was saved by a model with different FSMs";
    size_t start = reader.pos;
    for (size_t pass = 0; pass < 2u; ++pass) {
        reader.pos = start;
        for (size_t i = 0; i < llg_fcov_n_fsms; ++i) {
            llg_fcov_fsm_t* fsm = &llg_fcov_fsms[i];
            uint32_t states;
            if (!llg_cov_get_u32(&reader, &states) || states != fsm->n_states)
                return "the FSM section does not match its FSM table";
            for (uint32_t s = 0; s < states; ++s) {
                uint64_t visits;
                if (!llg_cov_get_u64(&reader, &visits))
                    return "the FSM section does not match its FSM table";
                if (pass) llg_acov_add(&fsm->visits[s], visits);
            }
            uint32_t transitions;
            if (!llg_cov_get_u32(&reader, &transitions))
                return "the FSM section does not match its FSM table";
            uint64_t last = 0;
            for (uint32_t t = 0; t < transitions; ++t) {
                uint32_t from, to;
                uint64_t count;
                if (!llg_cov_get_u32(&reader, &from) || !llg_cov_get_u32(&reader, &to) ||
                    !llg_cov_get_u64(&reader, &count) || from >= states || to >= states ||
                    from == to)
                    return "the FSM section does not match its FSM table";
                uint64_t key = ((uint64_t)from << 32) | to;
                if (t && key <= last) return "the FSM section's transitions are not sorted";
                last = key;
                if (pass) llg_acov_add(&llg_fcov_transition(fsm, from, to, 1)->count, count);
            }
        }
        if (reader.pos != reader.len) return "the FSM section does not match its FSM table";
    }
    return NULL;
}

// ── VPI bridge (40.5.3) ─────────────────────────────────────────────────────

size_t llg_fcov_count(void) { return llg_fcov_n_fsms; }

const llg_fcov_fsm_t* llg_fcov_at(size_t index) {
    return index < llg_fcov_n_fsms ? &llg_fcov_fsms[index] : NULL;
}

size_t llg_fcov_covered_in(int32_t instance) {
    return llg_fcov_scope(0, instance, NULL, UINT32_MAX).reached;
}
