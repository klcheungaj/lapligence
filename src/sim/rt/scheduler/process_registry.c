
// Registered final-block processes (see llg_rt.h).  Kept OUTSIDE the runtime
// context: `llg_rt_cleanup` memsets the context, and registration happens
// around the `llg_rt_run()` call in generated `main()`.
typedef struct {
    void (*fn)(llg_proc_t*);
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

static void all_procs_reserve(int needed) {
    if (needed <= g.all_procs_capacity) return;
    int capacity = llg_registry_capacity(g.all_procs_capacity, needed);
    llg_proc_t** grown = (llg_proc_t**)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "process registry");
    if (g.all_procs)
        memcpy(grown, g.all_procs, (size_t)g.n_procs * sizeof(*grown));
    free(g.all_procs);
    g.all_procs = grown;
    g.all_procs_capacity = capacity;
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
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == NULL) {
            g.all_procs[i] = p;
            return;
        }
    }
    if (g.n_procs == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: process registry size overflow\n");
        abort();
    }
    all_procs_reserve(g.n_procs + 1);
    g.all_procs[g.n_procs++] = p;
}

static void unregister_proc(llg_proc_t* p) {
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == p) {
            g.all_procs[i] = NULL;
            while (g.n_procs > 0 && g.all_procs[g.n_procs - 1] == NULL)
                g.n_procs--;
            return;
        }
    }
}

static llg_proc_t* llg_current(void) {
    return aco_gtls_co && aco_gtls_co != g.main_co
               ? (llg_proc_t*)aco_get_arg() : NULL;
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

static int llg_rng_argument(sv4_t value, uint32_t* result) {
    if (sv4_is_unknown(value) || value.width == 0) return 0;
    *result = (uint32_t)sv4_to_u64(value);
    return 1;
}

sv4_t llg_urandom(void) {
    return sv4_from_u64((uint64_t)llg_rng_state_next(llg_process_rng()), 32, 0);
}

sv4_t llg_urandom_seed(sv4_t seed) {
    uint32_t value = 0;
    if (!llg_rng_argument(seed, &value)) return sv4_x(32, 0);
    llg_rng_state_seed(llg_process_rng(), value);
    return llg_urandom();
}

sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min) {
    uint32_t high;
    uint32_t low = 0;
    if (!llg_rng_argument(max, &high) ||
        (has_min && !llg_rng_argument(min, &low)))
        return sv4_x(32, 0);
    if (!has_min) low = 0;
    return sv4_from_u64(
        (uint64_t)llg_rng_state_uniform(llg_process_rng(), high, low), 32, 0);
}

void llg_process_srandom(sv4_t seed) {
    uint32_t value = 0;
    if (!llg_rng_argument(seed, &value)) {
        fprintf(stderr, "llg: random runtime: srandom seed is unknown or real\n");
        llg_last_failure = 1;
        return;
    }
    llg_rng_state_seed(llg_process_rng(), value);
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
