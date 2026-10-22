
// Registered final-block processes (see llg_rt.h).  Kept OUTSIDE the runtime
// context: `llg_rt_cleanup` memsets the context, and registration happens
// around the `llg_rt_run()` call in generated `main()`.
typedef struct {
    void (*fn)(void);
    const char* name;
} llg_final_registration_t;
static llg_final_registration_t* llg_finals;
static int llg_n_finals;
static int llg_finals_capacity;
static int llg_in_finals;
// Scheduler time when `llg_rt_run` exited; `$time` inside finals reports it.
static uint64_t llg_final_time;

#define LLG_REGISTRY_INITIAL 16

// Double a registry capacity until it can hold `needed` entries. Growth is
// bounded by the `int` index type; overflow aborts explicitly. Every grown
// table is fully populated before it replaces the live one, so an allocation
// failure aborts without a partially rebound registry.
static int llg_registry_capacity(int current, int needed) {
    int capacity = current > 0 ? current : LLG_REGISTRY_INITIAL;
    while (capacity < needed) {
        if (capacity > INT_MAX / 2) {
            fprintf(stderr, "llg runtime fatal: registry capacity overflow\n");
            abort();
        }
        capacity *= 2;
    }
    return capacity;
}

// A set bit denotes a hole below n_procs; upper levels summarize nonempty
// words. Lowest-hole reuse preserves cancellation traversal order. At most
// six words are visited for any int-sized registry, independent of population.
static void proc_free_set(int slot, int available) {
    size_t index = (size_t)slot;
    for (int level = 0; level < g.proc_free_levels; level++) {
        size_t word = index / 64;
        uint64_t mask = UINT64_C(1) << (index % 64);
        uint64_t before = g.proc_free_bits[level][word];
        if (available) g.proc_free_bits[level][word] |= mask;
        else g.proc_free_bits[level][word] &= ~mask;
        uint64_t after = g.proc_free_bits[level][word];
        if ((before != 0) == (after != 0)) break;
        available = after != 0;
        index = word;
    }
}

static unsigned proc_first_bit(uint64_t bits) {
    unsigned index = 0;
    // Portable C11 bounded binary search; callers supply a nonzero word.
    if (!(bits & UINT64_C(0xffffffff))) { bits >>= 32; index += 32; }
    if (!(bits & UINT64_C(0xffff))) { bits >>= 16; index += 16; }
    if (!(bits & UINT64_C(0xff))) { bits >>= 8; index += 8; }
    if (!(bits & UINT64_C(0xf))) { bits >>= 4; index += 4; }
    if (!(bits & UINT64_C(0x3))) { bits >>= 2; index += 2; }
    if (!(bits & UINT64_C(0x1))) index++;
    return index;
}

static int proc_first_free(void) {
    if (!g.proc_free_levels ||
        !g.proc_free_bits[g.proc_free_levels - 1][0]) return g.n_procs;
    size_t index = 0;
    for (int level = g.proc_free_levels - 1; level >= 0; level--)
        index = index * 64 + proc_first_bit(g.proc_free_bits[level][index]);
    return (int)index;
}

static void all_procs_reserve(int needed) {
    if (needed <= g.all_procs_capacity) return;
    int capacity = llg_registry_capacity(g.all_procs_capacity, needed);
    llg_proc_t** grown = (llg_proc_t**)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "process registry");
    if (g.all_procs)
        memcpy(grown, g.all_procs, (size_t)g.n_procs * sizeof(*grown));
    uint64_t* bits[6] = {0};
    size_t count = (size_t)capacity;
    int levels = 0;
    do {
        count = (count + 63) / 64;
        bits[levels++] = (uint64_t*)llg_checked_calloc(
            count, sizeof(uint64_t), "process free slots");
    } while (count > 1);
    for (int level = 0; level < g.proc_free_levels; level++)
        free(g.proc_free_bits[level]);
    memcpy(g.proc_free_bits, bits, sizeof(bits));
    g.proc_free_levels = levels;
    free(g.all_procs);
    g.all_procs = grown;
    g.all_procs_capacity = capacity;
    for (int i = 0; i < g.n_procs; i++)
        if (!grown[i]) proc_free_set(i, 1);
}

static void finals_reserve(int needed) {
    if (needed <= llg_finals_capacity) return;
    int capacity = llg_registry_capacity(llg_finals_capacity, needed);
    llg_final_registration_t* grown = (llg_final_registration_t*)llg_checked_malloc(
        (size_t)capacity, sizeof(*grown), "final block registry");
    if (llg_finals)
        memcpy(grown, llg_finals, (size_t)llg_n_finals * sizeof(*grown));
    free(llg_finals);
    llg_finals = grown;
    llg_finals_capacity = capacity;
}

static void finals_release(void) {
    free(llg_finals);
    llg_finals = NULL;
    llg_finals_capacity = 0;
}

static void register_proc(llg_proc_t* p) {
    if (!p || g.next_process_identity == UINT64_MAX) {
        fprintf(stderr, "llg: process identity overflow\n");
        abort();
    }
    p->assertion_owner = ++g.next_process_identity;
    int slot = proc_first_free();
    if (slot == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: process registry size overflow\n");
        abort();
    }
    all_procs_reserve(slot + 1);
    p->registry_slot = slot;
    g.all_procs[slot] = p;
    if (slot == g.n_procs) g.n_procs++;
    else proc_free_set(slot, 0);
}

static void unregister_proc(llg_proc_t* p) {
    if (!p || p->registry_slot < 0 || p->registry_slot >= g.n_procs ||
        g.all_procs[p->registry_slot] != p) return;
    int slot = p->registry_slot;
    p->registry_slot = -1;
    g.all_procs[slot] = NULL;
    proc_free_set(slot, 1);
    // Each trailing slot is trimmed once per registration: amortized O(1).
    while (g.n_procs > 0 && !g.all_procs[g.n_procs - 1]) {
        proc_free_set(g.n_procs - 1, 0);
        g.n_procs--;
    }
}

llg_proc_t* llg_current(void) { return g.current; }

static llg_co_chain_t* llg_rt_current_chain(void) {
    return g.current ? &g.current->chain : NULL;
}

// Do not recurse into cancellation while a tree is being unlinked. The
// caller services completed programs after finishing the cancellation batch.
static void release_program_process(llg_proc_t* process) {
    if (!process || !process->program_live) return;
    process->program_live = 0;
    if (!process->program || process->program->live_initials == 0 ||
        g.program_processes == 0) {
        fprintf(stderr, "llg: program initial accounting underflow\n");
        abort();
    }
    process->program->live_initials--;
    g.program_processes--;
    g.program_completion_pending = 1;
}

static void service_program_completions(void) {
    if (!g.program_completion_pending || g.finish || g.config_error) return;
    g.program_completion_pending = 0;
    for (llg_program_t* program = g.programs; program; program = program->next) {
        if (!program->had_initial || program->live_initials || program->closed)
            continue;
        program->closed = 1;
        // Restart after cancellation: it changes the process and group lists.
        for (;;) {
            llg_proc_t* victim = NULL;
            for (int i = 0; i < g.n_procs; i++) {
                llg_proc_t* proc = g.all_procs[i];
                if (proc && proc->program == program && !proc->completed &&
                    !proc->killed) {
                    victim = proc;
                    break;
                }
            }
            if (!victim) break;
            llg_kill_proc_tree(victim);
        }
    }
    // This is an immediate finish boundary, not a request to drain Re-NBAs
    // or execute detached descendants before entering final procedures.
    if (g.program_processes == 0) g.finish = 1;
}

/* Calls made while a generated process is running use that process's stream.
 * The root stream is the safe fallback for initialization callbacks and
 * embedding code that invokes the service outside a coroutine. */
static llg_rng_state_t* llg_process_rng(void) {
    llg_proc_t* process = llg_current();
    if (process) return &process->rng;
    return g.rng_initializer ? g.rng_initializer : &g.rng_root;
}

/* Container methods (shuffle) draw from the calling thread's stream. */
static llg_rng_state_t* llg_container_thread_rng(void) {
    return llg_process_rng();
}

/* Seeds, $urandom seeds and $urandom_range bounds are `int`/`int unsigned`
 * formals: a 4-state actual converts to 2-state with X/Z bits read as 0
 * (IEEE 1800-2009 6.11.2), then truncates to 32 bits. */
static uint32_t llg_rng_argument(sv4_t value) {
    if (llg_sv4_width(value) == 0) return 0;
    if (!sv4_is_unknown(value)) return (uint32_t)sv4_to_u64(value);
    sv4_t known = sv4_to_two_state(value);
    uint32_t result = (uint32_t)sv4_to_u64(known);
    sv4_destroy(&known);
    return result;
}

/* IEEE 1800-2009 18.14.1: every module, interface and program instance has
 * an initialization RNG, and a static process is seeded with the next value
 * of the initialization RNG of the instance that declares it. The instance is
 * the process label without its final kind component (`tb.u.always` ->
 * `tb.u`); an indexed initializer label (`tb.class_initializer.3`) also drops
 * its index. "Each initialization RNG is seeded with the default seed", so
 * every instance stream starts from the same state: identical instances draw
 * identical sequences, and processes added to one instance never move
 * another instance's seeds. The name only keys the table. */
typedef struct llg_rng_scope_entry {
    const char* scope;
    size_t length;
    llg_rng_state_t rng;
} llg_rng_scope_t;

static size_t rng_scope_length(const char* name) {
    if (!name) return 0;
    size_t length = strlen(name);
    size_t digits = length;
    while (digits > 0 && name[digits - 1] >= '0' && name[digits - 1] <= '9')
        --digits;
    if (digits < length && digits > 0 && name[digits - 1] == '.')
        length = digits - 1;
    while (length > 0 && name[length - 1] != '.') --length;
    return length > 0 ? length - 1 : 0;
}

static uint64_t rng_scope_hash(const char* scope, size_t length) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325);
    for (size_t index = 0; index < length; ++index) {
        hash ^= (unsigned char)scope[index];
        hash *= UINT64_C(0x100000001b3);
    }
    return hash;
}

static void rng_scopes_grow(void) {
    size_t capacity = g.rng_scope_capacity ? g.rng_scope_capacity * 2 : 64;
    if (capacity > SIZE_MAX / sizeof(llg_rng_scope_t))
        llg_fatal_allocation("instance random streams", capacity,
                             sizeof(llg_rng_scope_t));
    llg_rng_scope_t* table = (llg_rng_scope_t*)llg_checked_calloc(
        capacity, sizeof(*table), "instance random streams");
    for (size_t index = 0; index < g.rng_scope_capacity; ++index) {
        llg_rng_scope_t* entry = &g.rng_scopes[index];
        if (!entry->scope) continue;
        size_t slot = (size_t)rng_scope_hash(entry->scope, entry->length) &
                      (capacity - 1);
        while (table[slot].scope) slot = (slot + 1) & (capacity - 1);
        table[slot] = *entry;
    }
    free(g.rng_scopes);
    g.rng_scopes = table;
    g.rng_scope_capacity = capacity;
}

static llg_rng_state_t* rng_instance_stream(const char* name) {
    static const char model_scope[] = "";
    size_t length = rng_scope_length(name);
    const char* scope = length ? name : model_scope;
    if ((g.rng_scope_count + 1) * 4 > g.rng_scope_capacity * 3)
        rng_scopes_grow();
    uint64_t hash = rng_scope_hash(scope, length);
    size_t mask = g.rng_scope_capacity - 1;
    size_t slot = (size_t)hash & mask;
    while (g.rng_scopes[slot].scope) {
        llg_rng_scope_t* entry = &g.rng_scopes[slot];
        if (entry->length == length && memcmp(entry->scope, scope, length) == 0)
            return &entry->rng;
        slot = (slot + 1) & mask;
    }
    llg_rng_scope_t* entry = &g.rng_scopes[slot];
    entry->scope = scope;
    entry->length = length;
    llg_rng_state_seed(&entry->rng, LLG_RNG_DEFAULT_SEED);
    ++g.rng_scope_count;
    return &entry->rng;
}

static void rng_scopes_free(void) {
    free(g.rng_scopes);
    g.rng_scopes = NULL;
    g.rng_scope_capacity = 0;
    g.rng_scope_count = 0;
}

/* Model-spawned processes are static processes of their instance. Forked
 * and detached children derive from their parent thread instead (forks.c).
 * A model that never observes a random stream keeps the cheaper root-order
 * seeding: no label hashing or instance table on its spawns. */
static void rng_seed_static(llg_proc_t* p) {
    llg_rng_state_t* parent = g.rng_instance_streams
        ? rng_instance_stream(p->name)
        : &g.rng_root;
    llg_rng_state_child(parent, &p->rng);
}

void llg_rt_use_instance_random_streams(void) {
    g.rng_instance_streams = 1;
}

/* A static handle initializer runs in the initialization frame, outside any
 * process; the objects it creates seed from the declaring instance's
 * initialization stream, like a static process of that instance. `label` is
 * a static string whose last component names nothing (`tb.u.initializer`). */
void llg_rng_initializer_scope_begin(const char* label) {
    g.rng_initializer = g.rng_instance_streams ? rng_instance_stream(label) : NULL;
}

void llg_rng_initializer_scope_end(void) {
    g.rng_initializer = NULL;
}

sv4_t llg_urandom(void) {
    return sv4_from_u64((uint64_t)llg_rng_state_next(llg_process_rng()), 32, 0);
}

sv4_t llg_urandom_seed(sv4_t seed) {
    llg_rng_state_seed(llg_process_rng(), llg_rng_argument(seed));
    return llg_urandom();
}

/* Legacy seed variables (S28-D2, S28-D3). The low min(width, 32) bits are
 * the seed, zero-extended whatever the variable's signedness; an X or Z among
 * them makes the whole seed 0. */
int32_t llg_random_seed_read(sv4_t seed) {
    uint32_t width = llg_sv4_width(seed);
    if (width == 0) return 0;
    uint64_t mask = width >= 32u ? UINT64_C(0xffffffff) : (UINT64_C(1) << width) - 1u;
    return (int32_t)(uint32_t)(llg_sv4_word(seed, 0, LLG_SV4_BITS) & mask);
}

/* The returned seed replaces only those low bits; wider variables keep their
 * upper bits. */
sv4_t llg_random_seed_merge(sv4_t old, int32_t seed) {
    sv4_t merged = sv4_clone(&old);
    uint64_t low = UINT64_C(0xffffffff);
    llg_sv4_set_word(&merged, 0,
                     (llg_sv4_word(old, 0, LLG_SV4_BITS) & ~low) | (uint64_t)(uint32_t)seed,
                     llg_sv4_word(old, 0, LLG_SV4_X) & ~low, llg_sv4_word(old, 0, LLG_SV4_Z) & ~low);
    return merged;
}

sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min) {
    uint32_t high = llg_rng_argument(max);
    uint32_t low = has_min ? llg_rng_argument(min) : 0;
    return sv4_from_u64(
        (uint64_t)llg_rng_state_uniform(llg_process_rng(), high, low), 32, 0);
}

/* randcase (IEEE 1800-2009 18.16). Weights are read as unsigned two-state
 * values (X/Z bits as 0); the sum is exact in `sum_width` bits, which lowering
 * sized so it cannot wrap (weight width plus ceil(log2(items))). One draw per
 * call from the calling process stream picks a number in [0, sum-1], and the
 * first item whose running total exceeds it wins, so a zero weight is never
 * chosen. Totals up to 32 bits draw exactly like $urandom_range(sum-1, 0),
 * wider ones combine more 32-bit draws with rejection to stay unbiased.
 * The weights are "non-negative integral values" (L31311): a signed weight
 * with its sign bit set ends the run at once, without final procedures
 * (S31-D4, S16-D3), and selects nothing. */
static const char* randcase_where(const char* location) {
    return location && location[0] != '\0' ? location : "<unknown>";
}

static int random_weight_negative(const sv4_t* weight) {
    if (!llg_sv4_signed(*weight) || llg_sv4_width(*weight) == 0) return 0;
    sv4_t sign = sv4_bit_select(*weight, (uint64_t)llg_sv4_width(*weight) - 1u);
    int negative = sv4_to_bool(sign);
    sv4_destroy(&sign);
    return negative;
}

/* The caller returns -1 (nothing selected) and its generated statement ends
 * the process at the exit check that follows the selection. */
static void random_weight_fatal(const char* message, const char* location) {
    llg_rt_mark_failed();
    llg_rt_skip_finals();
    llg_rt_fatal_typed(0, message, NULL, 0, "", randcase_where(location));
}

static int randcase_negative_weight(const sv4_t* const* weights, size_t count,
                                    const char* location) {
    for (size_t index = 0; index < count; ++index) {
        if (!random_weight_negative(weights[index])) continue;
        fprintf(stderr, "llg: error: randcase at %s: weight %zu is negative\n",
                randcase_where(location), index);
        random_weight_fatal("randcase weight is negative", location);
        return 1;
    }
    return 0;
}

static void randcase_warn_zero(const char* location) {
    fprintf(stderr,
            "llg: warning: randcase at %s: all weights are zero; no branch "
            "is taken\n",
            randcase_where(location));
}

static uint64_t randcase_weight_u64(const sv4_t* weight) {
    if (llg_sv4_width(*weight) == 0) return 0;
    if (!sv4_is_unknown(*weight)) return sv4_to_u64(*weight);
    sv4_t known = sv4_to_two_state(*weight);
    uint64_t result = sv4_to_u64(known);
    sv4_destroy(&known);
    return result;
}

static int32_t randcase_select_u64(llg_rng_state_t* rng,
                                   const sv4_t* const* weights, size_t count,
                                   const char* location) {
    uint64_t total = 0;
    for (size_t index = 0; index < count; ++index)
        total += randcase_weight_u64(weights[index]);
    if (total == 0) {
        randcase_warn_zero(location);
        return -1;
    }
    uint64_t pick;
    if (total - 1u <= UINT32_MAX) {
        pick = llg_rng_state_uniform(rng, (uint32_t)(total - 1u), 0);
    } else {
        uint64_t remainder = (UINT64_C(0) - total) % total;
        do {
            pick = (uint64_t)llg_rng_state_next(rng) << 32;
            pick |= llg_rng_state_next(rng);
        } while (pick < remainder);
        pick %= total;
    }
    uint64_t running = 0;
    for (size_t index = 0; index < count; ++index) {
        running += randcase_weight_u64(weights[index]);
        if (pick < running) return (int32_t)index;
    }
    return -1;
}

static sv4_t randcase_weight_wide(const sv4_t* weight, uint32_t sum_width) {
    sv4_t known = sv4_to_two_state(*weight);
    llg_sv4_set_signed(&known, 0);
    sv4_t wide = sv4_cast(known, sum_width, 0);
    sv4_destroy(&known);
    return wide;
}

static int randcase_less(sv4_t left, sv4_t right) {
    sv4_t less = sv4_lt(left, right);
    int result = sv4_to_bool(less);
    sv4_destroy(&less);
    return result;
}

static int32_t randcase_select_wide(llg_rng_state_t* rng,
                                    const sv4_t* const* weights, size_t count,
                                    uint32_t sum_width, const char* location) {
    sv4_t total = sv4_zero(sum_width, 0);
    for (size_t index = 0; index < count; ++index) {
        sv4_t wide = randcase_weight_wide(weights[index], sum_width);
        sv4_add_into(&total, total, wide);
        sv4_destroy(&wide);
    }
    if (!sv4_to_bool(total)) {
        randcase_warn_zero(location);
        sv4_destroy(&total);
        return -1;
    }
    /* Mask each candidate to the bits needed for `total`, so at least half of
     * the candidates are accepted. */
    sv4_t bits = sv4_clog2(total);
    uint32_t needed = (uint32_t)sv4_to_u64(bits);
    sv4_destroy(&bits);
    size_t limbs = ((size_t)needed + 63u) / 64u;
    uint64_t* words = (uint64_t*)llg_checked_calloc(limbs ? limbs : 1u,
                                                    sizeof(uint64_t),
                                                    "randcase draw");
    sv4_t pick = SV4_EMPTY;
    do {
        sv4_destroy(&pick);
        for (size_t limb = 0; limb < limbs; ++limb) {
            words[limb] = llg_rng_state_next(rng);
            if (needed > 64u * limb + 32u)
                words[limb] |= (uint64_t)llg_rng_state_next(rng) << 32;
        }
        pick = needed ? sv4_from_limbs(words, NULL, NULL, needed, 0)
                      : sv4_zero(1, 0);
    } while (!randcase_less(pick, total));
    free(words);
    sv4_t running = sv4_zero(sum_width, 0);
    int32_t chosen = -1;
    for (size_t index = 0; index < count && chosen < 0; ++index) {
        sv4_t wide = randcase_weight_wide(weights[index], sum_width);
        sv4_add_into(&running, running, wide);
        sv4_destroy(&wide);
        if (randcase_less(pick, running)) chosen = (int32_t)index;
    }
    sv4_destroy(&running);
    sv4_destroy(&pick);
    sv4_destroy(&total);
    return chosen;
}

int32_t llg_randcase_select(const sv4_t* const* weights, size_t count,
                            uint32_t sum_width, const char* location) {
    if (randcase_negative_weight(weights, count, location)) return -1;
    llg_rng_state_t* rng = llg_process_rng();
    return sum_width <= 64u
        ? randcase_select_u64(rng, weights, count, location)
        : randcase_select_wide(rng, weights, count, sum_width, location);
}

/* randsequence rule selection (IEEE 1800-2009 18.17.1): "A
 * weight_specification shall evaluate to an integral non-negative value", so
 * a negative weight ends the run at once, without final procedures, and
 * selects no rule (S16-D3). X/Z bits read as 0, as in randcase. Valid
 * weights draw exactly like randcase. */
int32_t llg_randsequence_select(const sv4_t* const* weights, size_t count,
                                uint32_t sum_width, const char* location) {
    for (size_t index = 0; index < count; ++index) {
        if (!random_weight_negative(weights[index])) continue;
        fprintf(stderr,
                "llg: error: randsequence production at %s: the weight of rule %zu "
                "is negative\n",
                randcase_where(location), index + 1u);
        random_weight_fatal("randsequence weight is negative", location);
        return -1;
    }
    int any = 0;
    for (size_t index = 0; index < count && !any; ++index) {
        if (llg_sv4_width(*weights[index]) == 0) continue;
        sv4_t known = sv4_to_two_state(*weights[index]);
        any = sv4_to_bool(known);
        sv4_destroy(&known);
    }
    if (!any) {
        fprintf(stderr,
                "llg: warning: randsequence production at %s: all rule weights "
                "are zero; nothing is generated\n",
                randcase_where(location));
        return -1;
    }
    return llg_randcase_select(weights, count, sum_width, location);
}

/* rand join (IEEE 1800-2009 18.17.5): the next sequence to advance among
 * those with a nonzero remaining length (S16-D6). With probability
 * p = |2*ratio - 1| the choice is restricted to the longest sequences
 * (ratio > 0.5) or the shortest (ratio < 0.5); the restriction is decided by
 * one 32-bit draw only when 0 < p < 1 (p is quantized to T = round(p * 2^32)
 * and the draw restricts when below T). One more draw picks uniformly among
 * the candidates when there are at least two. Returns -1 when every sequence
 * is exhausted. The caller validated `ratio`. */
int32_t llg_randsequence_join_pick(const sv4_t* const* remaining, size_t count,
                                   double ratio) {
    llg_rng_state_t* rng = llg_process_rng();
    uint64_t lengths_stack[16];
    uint64_t* lengths = count <= 16u
        ? lengths_stack
        : (uint64_t*)llg_checked_calloc(count, sizeof(uint64_t),
                                        "rand join lengths");
    size_t live = 0;
    uint64_t shortest = UINT64_MAX, longest = 0;
    for (size_t index = 0; index < count; ++index) {
        uint64_t length = randcase_weight_u64(remaining[index]);
        lengths[index] = length;
        if (length == 0) continue;
        live++;
        if (length < shortest) shortest = length;
        if (length > longest) longest = length;
    }
    int32_t chosen = -1;
    if (live != 0) {
        double bias = ratio * 2.0 - 1.0;
        double p = bias < 0.0 ? -bias : bias;
        double scaled = p * 4294967296.0 + 0.5;
        uint64_t threshold = scaled >= 4294967296.0 ? UINT64_C(4294967296)
                                                    : (uint64_t)scaled;
        int restrict_length = 0;
        if (threshold >= UINT64_C(4294967296))
            restrict_length = 1;
        else if (threshold != 0)
            restrict_length = (uint64_t)llg_rng_state_next(rng) < threshold;
        uint64_t target = bias > 0.0 ? longest : shortest;
        size_t candidates = 0;
        for (size_t index = 0; index < count; ++index)
            if (lengths[index] != 0 &&
                (!restrict_length || lengths[index] == target))
                candidates++;
        size_t pick = candidates > 1u
            ? (size_t)llg_rng_state_uniform(rng, (uint32_t)(candidates - 1u), 0)
            : 0u;
        for (size_t index = 0; index < count; ++index) {
            if (lengths[index] == 0 ||
                (restrict_length && lengths[index] != target))
                continue;
            if (pick-- == 0) {
                chosen = (int32_t)index;
                break;
            }
        }
    }
    if (lengths != lengths_stack) free(lengths);
    return chosen;
}

void llg_process_srandom(sv4_t seed) {
    llg_rng_state_seed(llg_process_rng(), llg_rng_argument(seed));
}

llg_string_t llg_process_get_randstate(void) {
    return llg_rng_state_get(llg_process_rng());
}

int llg_process_set_randstate(llg_string_t state) {
    int ok = llg_rng_state_set(llg_process_rng(), &state);
    if (!ok) {
        fprintf(stderr, "llg: random runtime: invalid randstate string\n");
        llg_last_failure = 1;
    }
    llg_string_destroy(&state);
    return ok;
}

/* Object streams (IEEE 1800-2009 18.14.1 object stability): creation seeds
 * the object with the next value of the creating thread, or of the root
 * initialization stream outside a process. A model that keeps no object
 * state still consumes that draw so the thread's sequence does not depend on
 * whether some object's stream is ever inspected. */
void llg_object_rng_create(llg_rng_state_t* object) {
    llg_rng_state_t* thread = llg_process_rng();
    if (object) llg_rng_state_child(thread, object);
    else llg_rng_state_skip_child(thread);
}

void llg_object_srandom(llg_rng_state_t* object, sv4_t seed) {
    if (object) llg_rng_state_seed(object, llg_rng_argument(seed));
}

llg_string_t llg_object_get_randstate(const llg_rng_state_t* object) {
    return object ? llg_rng_state_get(object) : llg_string_bytes("", 0);
}

void llg_object_set_randstate(llg_rng_state_t* object, llg_string_t state) {
    if (object && !llg_rng_state_set(object, &state)) {
        fprintf(stderr, "llg: random runtime: invalid randstate string\n");
        llg_last_failure = 1;
    }
    llg_string_destroy(&state);
}
