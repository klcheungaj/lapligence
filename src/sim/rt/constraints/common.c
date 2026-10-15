/* llg_constraint.c - constraint solver (IEEE 1800-2009 clause 18).
 *
 * Assembled from src/sim/rt/constraints/{common,eval,aig,bdd,sample,cdcl,solve,expand}.c in
 * that order; the files share file-scope statics. See llg_constraint.h for
 * the interface contract and src/sim/rt/constraints/readme.md for the
 * engine portfolio.
 */
#include "llg_constraint.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CR_WORDS(width) ((width) == 0u ? 1u : (((width) + 63u) / 64u))

static void cr_fatal(const char* message) {
    fprintf(stderr, "llg: constraint solver: %s\n", message);
    abort();
}

static void* cr_alloc(size_t count, size_t size) {
    if (count == 0 || size == 0) return NULL;
    if (count > SIZE_MAX / size) cr_fatal("allocation size overflow");
    void* memory = calloc(count, size);
    if (!memory) cr_fatal("out of memory");
    return memory;
}

static void* cr_grow(void* memory, size_t* capacity, size_t needed,
                     size_t size) {
    if (needed <= *capacity) return memory;
    size_t next = *capacity ? *capacity : 16u;
    while (next < needed) {
        if (next > SIZE_MAX / 2u) cr_fatal("allocation size overflow");
        next *= 2u;
    }
    if (next > SIZE_MAX / size) cr_fatal("allocation size overflow");
    void* grown = realloc(memory, next * size);
    if (!grown) cr_fatal("out of memory");
    memset((char*)grown + *capacity * size, 0, (next - *capacity) * size);
    *capacity = next;
    return grown;
}

/* Growable uint32 / uint64 vectors. */
typedef struct {
    uint32_t* data;
    size_t len, cap;
} cr_u32vec_t;

typedef struct {
    uint64_t* data;
    size_t len, cap;
} cr_u64vec_t;

static void cr_u32_push(cr_u32vec_t* v, uint32_t x) {
    v->data = (uint32_t*)cr_grow(v->data, &v->cap, v->len + 1u, sizeof(uint32_t));
    v->data[v->len++] = x;
}

static void cr_u64_push(cr_u64vec_t* v, uint64_t x) {
    v->data = (uint64_t*)cr_grow(v->data, &v->cap, v->len + 1u, sizeof(uint64_t));
    v->data[v->len++] = x;
}

/* ---- tunables --------------------------------------------------------- */

static llg_cr_tunables_t cr_tunables_value;
static int cr_tunables_ready;

static uint64_t cr_env_u64(const char* name, uint64_t fallback) {
    const char* text = getenv(name);
    if (!text || !*text) return fallback;
    uint64_t value = 0;
    for (const char* p = text; *p; ++p) {
        if (*p < '0' || *p > '9') {
            fprintf(stderr,
                    "llg: constraint solver: ignoring %s=%s (not a decimal "
                    "count)\n",
                    name, text);
            return fallback;
        }
        uint64_t digit = (uint64_t)(*p - '0');
        if (value > (UINT64_MAX - digit) / 10u) return UINT64_MAX;
        value = value * 10u + digit;
    }
    return value;
}

const llg_cr_tunables_t* llg_cr_tunables(void) {
    if (!cr_tunables_ready) {
        /* Defaults: about 16 MiB of BDD nodes per component, an exact-engine
         * width bound that keeps BDD recursion shallow on coroutine-hosted
         * stacks, and search budgets measured on the SIM-029 probes. */
        cr_tunables_value.bdd_nodes = cr_env_u64("LLG_RAND_BDD_NODES", 1u << 20);
        cr_tunables_value.bdd_bits = cr_env_u64("LLG_RAND_BDD_BITS", 512u);
        cr_tunables_value.reject_tries = cr_env_u64("LLG_RAND_REJECT_TRIES", 4096u);
        cr_tunables_value.conflicts = cr_env_u64("LLG_RAND_CONFLICTS", 100000u);
        /* Expansion bounds (SIM-030): about 1 Mi random array elements and
         * 4 Mi expanded nodes (on the order of 100 MiB of solver tables). */
        cr_tunables_value.array_elements = cr_env_u64("LLG_RAND_ARRAY_ELEMENTS", 1u << 20);
        cr_tunables_value.expand_nodes = cr_env_u64("LLG_RAND_EXPAND_NODES", 1u << 22);
        cr_tunables_value.stats = cr_env_u64("LLG_RAND_STATS", 0u) != 0u;
        cr_tunables_ready = 1;
    }
    return &cr_tunables_value;
}

/* ---- session ---------------------------------------------------------- */

enum { CR_K = 0, CR_R = 1, CR_E = 2, CR_SKIP = 3 };

typedef struct cr_cache_entry cr_cache_entry_t;
typedef struct cr_tpl cr_tpl_t;

struct llg_cr_session {
    const llg_cr_problem_t* p;
    llg_cr_session_t* next_session;
    /* Template problems (units, SIM-030): instances and expanded sessions;
     * NULL for a problem without units. */
    cr_tpl_t* tpl;
    /* Node layout. */
    uint32_t* node_off;         /* word offset of each node value */
    uint64_t* val;              /* node values (K: folded; R: scratch) */
    uint8_t* status;            /* CR_K / CR_R / CR_E / CR_SKIP */
    uint8_t* ereason;           /* LLG_CR_REASON_* of an E node */
    uint32_t* eitem;
    uint32_t* stamp;            /* DFS marks */
    uint32_t stamp_now;
    uint32_t max_words;         /* widest node */
    /* Variables. */
    uint32_t* var_off;
    uint64_t* var_cur;          /* current (gathered) values */
    uint64_t* sol;              /* solution values */
    uint8_t* var_active;
    uint8_t* var_input;
    uint8_t* var_fixed;
    uint32_t total_var_words;
    /* Blocks, leaves and calls. */
    uint8_t* block_enabled;
    uint32_t* leaf_off;
    uint64_t* leaf_val;
    uint8_t* leaf_input;
    uint32_t* call_off;
    uint64_t* call_val;
    uint8_t* call_state;        /* 0 not evaluated, 1 ready, 2 done */
    uint8_t* call_input;
    const uint64_t** call_args; /* argument pointers for the thunk */
    int need_calls;             /* a classification found ready calls */
    uint32_t engines;           /* engines used since llg_cr_begin */
    /* Scratch for big-integer and evaluation helpers. */
    uint64_t* tmp_a;
    uint64_t* tmp_b;
    uint64_t* tmp_c;
    uint64_t* tmp_d;
    /* randc cycle state (LLG_CR_CYCLIC_WORDS per cyclic variable). */
    uint32_t* cyc_off;
    uint64_t* cyc;
    /* Structural hash of each node (randc signatures), 0 until computed. */
    uint64_t* node_hash;
    /* Exact-engine cache (most recent components). */
    cr_cache_entry_t* cache;
    uint32_t cache_count;
    uint32_t cache_next;
};

static llg_cr_session_t* cr_all_sessions;
