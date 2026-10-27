// ── Free checker variables (IEEE 1800-2009 17.7.2) ──────────────────────────
//
// Each checker instance with active free variables registers one set: a
// generated solve callback and the identities of the assume statements of
// its assume set. "When an implementation is about to begin the Observed
// region, it shall solve for all the active free variables": the scheduler
// calls `solve_free_variables` at the top of every Observed set, which pays
// one pointer test when no set is registered. A set with assumes solves when
// a leading clock event of one of them waits for this Observed pass (an
// assume set clock event); a set without assumes ("implicitly clocked")
// solves once per time step. The callback feeds the generated constraint
// problem; while it runs, `llg_checker_assume_flag` reports which assume
// statements see a clock event and which owe a pending `|=>` consequent,
// randomize steps draw from the set's own stream, and an unsuccessful solve
// returns 0 silently: "any values may be chosen for the active variables"
// and the assumptions report any resulting failure when they are checked.

typedef struct llg_free_set {
    struct llg_free_set* next;
    llg_free_solve_fn solve;
    uint64_t* identities;               // owned copy of the assume identities
    llg_concurrent_assertion_t** assumes; // resolved on the first solve
    unsigned char* flags;               // per assume: bit 0 event, bit 1 due
    uint32_t count;
    uint64_t step;                      // g.now + 1 of the last implicit solve
    llg_rng_state_t rng;
} llg_free_set_t;

static int free_assertion_identity_cmp(const void* left, const void* right) {
    const llg_concurrent_assertion_t* a = *(llg_concurrent_assertion_t* const*)left;
    const llg_concurrent_assertion_t* b = *(llg_concurrent_assertion_t* const*)right;
    return a->identity < b->identity ? -1 : a->identity > b->identity ? 1 : 0;
}

/* Resolve every set's assume identities with one sorted pass over the
 * assertions, so registration stays linear in the design. */
static int free_sets_resolve(void) {
    size_t total = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next)
        total++;
    llg_concurrent_assertion_t** sorted = NULL;
    if (total) {
        if (total > SIZE_MAX / sizeof(*sorted))
            llg_fatal_allocation("free variable assumes", total, sizeof(*sorted));
        sorted = (llg_concurrent_assertion_t**)llg_checked_calloc(
            total, sizeof(*sorted), "free variable assumes");
        size_t index = 0;
        for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
             assertion = assertion->next)
            sorted[index++] = assertion;
        qsort(sorted, total, sizeof(*sorted), free_assertion_identity_cmp);
    }
    int ok = 1;
    for (llg_free_set_t* set = g.free_sets; set && ok; set = set->next) {
        for (uint32_t k = 0; k < set->count; ++k) {
            size_t low = 0, high = total;
            llg_concurrent_assertion_t* found = NULL;
            while (low < high) {
                size_t mid = low + (high - low) / 2;
                if (sorted[mid]->identity < set->identities[k]) low = mid + 1;
                else high = mid;
            }
            if (low < total && sorted[low]->identity == set->identities[k] &&
                sorted[low]->kind == LLG_ASSERTION_ASSUME)
                found = sorted[low];
            if (!found) {
                fprintf(stderr, "llg: free variable set names unknown assume %llu\n",
                        (unsigned long long)set->identities[k]);
                llg_last_failure = 1;
                g.finish = 1;
                ok = 0;
                break;
            }
            set->assumes[k] = found;
        }
    }
    free(sorted);
    g.free_sets_resolved = 1;
    return ok;
}

int llg_checker_free_set(llg_free_solve_fn solve, const uint64_t* assumes,
                         uint32_t count, const char* label) {
    if (!g.initialized || g.running || g.config_error || !solve ||
        (count && !assumes)) {
        fprintf(stderr, "llg: invalid free variable set registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_free_set_t* set = (llg_free_set_t*)llg_checked_calloc(1, sizeof(*set),
                                                              "free variable set");
    set->solve = solve;
    set->count = count;
    if (count) {
        set->identities = (uint64_t*)llg_checked_calloc(count, sizeof(uint64_t),
                                                        "free variable set");
        memcpy(set->identities, assumes, count * sizeof(uint64_t));
        set->assumes = (llg_concurrent_assertion_t**)llg_checked_calloc(
            count, sizeof(*set->assumes), "free variable set");
        set->flags = (unsigned char*)llg_checked_calloc(count, 1, "free variable set");
    }
    // The set draws like a static process of its checker instance (18.14.1).
    llg_rng_state_t* parent = g.rng_instance_streams && label
        ? rng_instance_stream(label)
        : &g.rng_root;
    llg_rng_state_child(parent, &set->rng);
    // Registration order is the solving order.
    llg_free_set_t** tail = &g.free_sets;
    while (*tail) tail = &(*tail)->next;
    *tail = set;
    return 1;
}

int llg_checker_assume_flag(uint32_t assume, int due) {
    const llg_free_set_t* set = g.free_solving;
    if (!set || assume >= set->count) return 0;
    return (set->flags[assume] >> (due ? 1 : 0)) & 1;
}

/* The assume set clock events of `set` in this Observed pass: a leading
 * clock event waits and the assume is enabled and not disabled. `due` also
 * needs a pending attempt of a predicate `|=>` assume, whose consequent this
 * event evaluates. */
static int free_set_events(llg_free_set_t* set) {
    int any = 0;
    for (uint32_t k = 0; k < set->count; ++k) {
        llg_concurrent_assertion_t* assume = set->assumes[k];
        int event = assertion_root_event_pending(assume) && assertion_clock_live(assume) &&
                    !(assume->disable && sv4_to_bool(*assume->disable));
        int due = event && !assume->property && !assume->consequent_sequence &&
                  !assume->overlapped && assume->attempts != NULL;
        set->flags[k] = (unsigned char)(event | (due << 1));
        any |= event;
    }
    return any;
}

static void solve_free_variables(void) {
    if (!g.free_sets_resolved && !free_sets_resolve()) return;
    for (llg_free_set_t* set = g.free_sets; set && !g.finish; set = set->next) {
        if (set->count) {
            if (!free_set_events(set)) continue;
        } else {
            // "Implicitly clocked active variables may be updated with
            // unconstrained random values at every time step. Once updated,
            // the variables stay constant until the end of the time step."
            if (set->step == g.now + 1) continue;
            set->step = g.now + 1;
        }
        g.free_solving = set;
        g.free_rng = &set->rng;
        set->solve(NULL);
        g.free_solving = NULL;
        g.free_rng = NULL;
    }
}

static void free_sets_release(void) {
    while (g.free_sets) {
        llg_free_set_t* set = g.free_sets;
        g.free_sets = set->next;
        free(set->identities);
        free(set->assumes);
        free(set->flags);
        free(set);
    }
    g.free_sets_resolved = 0;
    g.free_solving = NULL;
    g.free_rng = NULL;
}
