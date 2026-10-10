
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
    return process ? &process->rng : &g.rng_root;
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
 * its index. Each instance stream derives from the root and the instance
 * name, so processes added to one instance never move another instance's
 * seeds and identical instances still draw different sequences. */
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
    llg_rng_state_derive(&entry->rng, &g.rng_root, hash);
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

sv4_t llg_urandom(void) {
    return sv4_from_u64((uint64_t)llg_rng_state_next(llg_process_rng()), 32, 0);
}

sv4_t llg_urandom_seed(sv4_t seed) {
    llg_rng_state_seed(llg_process_rng(), llg_rng_argument(seed));
    return llg_urandom();
}

sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min) {
    uint32_t high = llg_rng_argument(max);
    uint32_t low = has_min ? llg_rng_argument(min) : 0;
    return sv4_from_u64(
        (uint64_t)llg_rng_state_uniform(llg_process_rng(), high, low), 32, 0);
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
