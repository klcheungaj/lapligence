/* llg_constraint.h - constraint solver for randomize() (IEEE 1800-2009 18).
 *
 * The solver is independent of the scheduler and of the packed-value backend:
 * values cross this interface as exact 2-state little-endian 64-bit words
 * (bit i of a value is bit i%64 of word i/64; bits above the width are zero).
 * Generated models describe each randomize problem with static const tables
 * and drive one persistent session per call site; the scheduler glue
 * (llg_rt.h) converts packed values and reports outcomes.
 *
 * Determinism: the result is a pure function of the tables, the inputs, the
 * budgets and the random stream. No floating point, pointer ordering or host
 * word size affects a decision, so every platform draws the same values from
 * the same seed.
 */
#ifndef LLG_CONSTRAINT_H
#define LLG_CONSTRAINT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LLG_CR_NONE UINT32_MAX

/* Expression node operations. Nodes are stored in post-order: every operand
 * index is smaller than the node's own index. `bit_width`/`sign_flag` give the
 * node's result type after IEEE 1800-2009 11.6/11.8 sizing; lowering inserts
 * explicit RESIZE nodes, and operands of arithmetic and bitwise nodes have
 * the node's width. */
enum {
    LLG_CR_CONST = 1,   /* a = first word in problem consts */
    LLG_CR_VAR,         /* a = random variable */
    LLG_CR_STATE,       /* a = state leaf */
    LLG_CR_CALL,        /* a = function call site */
    LLG_CR_RESIZE,      /* a; extends by a's signedness, truncates */
    LLG_CR_NOT,         /* bitwise ~a */
    LLG_CR_NEG,         /* -a */
    LLG_CR_REDAND,      /* reductions of a, 1-bit results */
    LLG_CR_REDOR,
    LLG_CR_REDXOR,
    LLG_CR_REDNAND,
    LLG_CR_REDNOR,
    LLG_CR_REDXNOR,
    LLG_CR_LNOT,        /* !a */
    LLG_CR_ADD,         /* a, b */
    LLG_CR_SUB,
    LLG_CR_MUL,
    LLG_CR_DIV,         /* signed iff node is signed; divisor 0 is an error */
    LLG_CR_MOD,
    LLG_CR_AND,
    LLG_CR_OR,
    LLG_CR_XOR,
    LLG_CR_XNOR,
    LLG_CR_SHL,         /* a << b, b unsigned */
    LLG_CR_SHR,         /* a >> b, logical */
    LLG_CR_ASHR,        /* a >>> b, arithmetic iff node is signed */
    LLG_CR_EQ,          /* comparisons: signed iff both operands are */
    LLG_CR_NE,
    LLG_CR_LT,
    LLG_CR_LE,
    LLG_CR_GT,
    LLG_CR_GE,
    LLG_CR_LAND,        /* a && b, Figure 18-3 conjunction */
    LLG_CR_LOR,         /* a || b, Figure 18-3 disjunction */
    LLG_CR_IMPL,        /* a -> b, as !a || b */
    LLG_CR_EQUIV,       /* a <-> b */
    LLG_CR_COND,        /* a ? b : c */
    LLG_CR_CONCAT,      /* args[b .. b+c), most significant first */
    LLG_CR_REPL,        /* a repeated b times */
    LLG_CR_SLICE,       /* a[b +: width] with constant b */
    LLG_CR_DSLICE,      /* a[idx +: width], idx = node b (state only);
                           a bit outside a is an evaluation error */
    LLG_CR_ALL,         /* conjunction of args[b .. b+c), Figure 18-3 */
    LLG_CR_POW,         /* a ** b (11.4.3, Table 11-4); b self-determined;
                           0 ** negative is an evaluation error */
    LLG_CR_DIST,        /* a = dists index; 1 when the expression matches an
                           item of nonzero weight (18.5.4) */
    LLG_CR_ELEM,        /* tables[a] entry selected by node b (0-based);
                           an index outside the table or a null entry is an
                           evaluation error. Expanded problems only. */
    /* Template operations (problems with units, SIM-030). The solver
     * expands them per object instance and loop iteration before solving.
     * In a template, VAR names the variable of the instance whose
     * constraint is expanded (or a global variable) and STATE that
     * instance's leaf. A "ref" operand is a REF node or LLG_CR_NONE (the
     * instance itself). */
    LLG_CR_REF,         /* object: handle slot a of the object ref b names;
                           c = element index node of a handle array slot
                           (constants and loop indices only), else
                           LLG_CR_NONE. No value of its own. */
    LLG_CR_HVAR,        /* variable b of the object ref a names */
    LLG_CR_AELEM,       /* element at 0-based position (node b) of array a
                           of the object ref c names */
    LLG_CR_ASIZE,       /* 32-bit signed size of array a (LLG_CR_SLOT_ARRAY
                           | slot: of a handle array) of the object ref c
                           names */
    LLG_CR_ITER,        /* 32-bit signed index of loop a's current element
                           (the declared index for fixed arrays) */
    LLG_CR_FOREACH,     /* args[b .. b+4) = {array, body, ref, loop}: the
                           conjunction of body over the elements (array as
                           for ASIZE) */
    LLG_CR_REDUCE,      /* args[b .. b+5) = {array, body, ref, loop, op}:
                           body over the elements, each converted to the
                           node's width, folded with op (ADD, MUL, AND, OR,
                           XOR, LAND, LOR); an empty array gives the
                           identity of ADD, MUL, LAND and LOR, and an
                           evaluation error for AND, OR and XOR (S30-D6) */
    LLG_CR_ISNULL,      /* 1-bit: the object ref a names is null */
    LLG_CR_OP_LIMIT
};

typedef struct {
    uint16_t op;
    uint8_t sign_flag;
    uint8_t reserved;
    uint32_t bit_width;
    uint32_t a, b, c;
} llg_cr_node_t;

typedef struct {
    const char* name;
    uint32_t bit_width;
    uint8_t sign_flag;
    /* Domain constraint applied only while the variable is active (an enum
     * variable takes its named values, decision S29-D2), or LLG_CR_NONE. */
    uint32_t domain_node;
    /* randc (18.4.2): solved first, cycling through its values; its state
     * is set with llg_cr_set_cyclic. Width at most LLG_CR_CYCLIC_MAX_WIDTH. */
    uint8_t cyclic;
} llg_cr_var_t;

/* Widest randc variable: its cycle state holds one bit per value. */
#define LLG_CR_CYCLIC_MAX_WIDTH 16u

/* Words of a randc variable's cycle state: one "returned in this cycle" bit
 * per value, then a 64-bit signature of the constraints of the cycle. */
#define LLG_CR_CYCLIC_WORDS(width) \
    ((width) >= 6u ? (1u << ((width) - 6u)) + 1u : 2u)

/* One `dist` constraint (18.5.4). `expr` is the distributed expression,
 * `guard` the conjunction of the enclosing implication and if conditions
 * (LLG_CR_NONE when unconditional): the weight applies to solutions where
 * the guard holds (decision S30-D1). */
typedef struct {
    uint32_t expr;
    uint32_t guard;
    uint32_t item_first;  /* in problem dist_items */
    uint32_t item_count;
} llg_cr_dist_t;

/* One dist item. `match` is the 1-bit node "expr is in the item"; `lo` and
 * `hi` are the range bounds in the comparison type (equal for a single
 * value), used to count the values a `:/` weight is shared by. `weight` is
 * a node (LLG_CR_NONE for the default weight 1). */
typedef struct {
    uint32_t match;
    uint32_t lo;
    uint32_t hi;
    uint32_t weight;
    uint8_t per_range;    /* `:/` */
    uint8_t lo_signed;    /* the bounds compare as signed values */
} llg_cr_dist_item_t;

/* `solve before before after` from constraint block `block` (18.5.9). */
typedef struct {
    uint32_t block;
    uint32_t before;
    uint32_t after;
} llg_cr_order_t;

typedef struct {
    const char* name;
    uint32_t root;      /* 1-bit node that must hold while enabled */
} llg_cr_block_t;

/* Expanded problems: a table of entries for LLG_CR_ELEM (node indices, or
 * LLG_CR_NONE for a null object's entry). */
typedef struct {
    uint32_t first;     /* in problem table_items */
    uint32_t count;
} llg_cr_table_t;

/* Array kinds. A fixed array's position i has the declared index
 * left + i (left <= right) or left - i (left > right). */
enum { LLG_CR_ARRAY_FIXED = 0, LLG_CR_ARRAY_DYNAMIC = 1, LLG_CR_ARRAY_QUEUE = 2 };

/* Bit marking a handle-array slot in LLG_CR_ASIZE / LLG_CR_FOREACH. */
#define LLG_CR_SLOT_ARRAY 0x80000000u

/* A random (or state) unpacked array of packed elements of one unit. */
typedef struct {
    const char* name;
    uint32_t elem_width;
    uint8_t elem_signed;
    uint8_t kind;        /* LLG_CR_ARRAY_* */
    int32_t left;        /* fixed arrays: declared bounds */
    int32_t right;
} llg_cr_array_t;

/* An object handle (or handle array) property reached by constraints. */
typedef struct {
    const char* name;
    uint8_t kind;        /* LLG_CR_ARRAY_*; scalar handles use FIXED, 0, 0 */
    uint8_t is_array;
    int32_t left;        /* fixed handle arrays: declared bounds */
    int32_t right;
} llg_cr_slot_t;

/* One class layout of a problem with objects or arrays (SIM-030).
 * Variables, arrays and slots are problem-wide (one per property), so a
 * base-class property has the same index in every derived class's unit;
 * unit_items[item_first ..] lists the unit's variables, then its arrays,
 * then its slots. Variables listed by no unit are global (static
 * properties, std::randomize arguments). Blocks and leaves are ranges.
 * Unit 0 is the randomized object (or the scope of std::randomize) and is
 * expanded once; only its constraints may call functions, and not with
 * loop indices in the arguments. */
typedef struct {
    const char* name;
    uint32_t item_first;
    uint32_t var_count, array_count, slot_count;
    uint32_t block_first, block_count;
    uint32_t leaf_first, leaf_count;
} llg_cr_unit_t;

typedef struct {
    const char* name;   /* source text for diagnostics */
    uint32_t bit_width;
    uint8_t sign_flag;
} llg_cr_leaf_t;

typedef struct {
    const char* name;
    uint32_t bit_width;
    uint8_t sign_flag;
    uint32_t arg_first;  /* node indices in problem args */
    uint32_t arg_count;
} llg_cr_call_t;

typedef struct {
    const char* name;
    const llg_cr_node_t* nodes;
    uint32_t node_count;
    const uint32_t* args;
    uint32_t arg_count;
    const uint64_t* consts;
    uint32_t const_count;
    const llg_cr_var_t* vars;
    uint32_t var_count;
    const llg_cr_block_t* blocks;
    uint32_t block_count;
    const llg_cr_leaf_t* leaves;
    uint32_t leaf_count;
    const llg_cr_call_t* calls;
    uint32_t call_count;
    const llg_cr_dist_t* dists;
    uint32_t dist_count;
    const llg_cr_dist_item_t* dist_items;
    uint32_t dist_item_count;
    const llg_cr_order_t* orders;
    uint32_t order_count;
    const llg_cr_table_t* tables;     /* expanded problems */
    uint32_t table_count;
    const uint32_t* table_items;
    uint32_t table_item_count;
    const llg_cr_unit_t* units;       /* template problems (SIM-030) */
    uint32_t unit_count;
    const uint32_t* unit_items;
    uint32_t unit_item_count;
    const llg_cr_array_t* arrays;
    uint32_t array_count;
    const llg_cr_slot_t* slots;
    uint32_t slot_count;
} llg_cr_problem_t;

typedef enum {
    LLG_CR_OK = 0,       /* solution available through llg_cr_value */
    LLG_CR_UNSAT = 1,    /* proven: no assignment satisfies the constraints */
    LLG_CR_RESOURCE = 2, /* a named budget ran out; nothing was proven */
    LLG_CR_ERROR = 3,    /* evaluation error (18.5.12 E, X/Z state, order) */
    LLG_CR_NEED_CALLS = 4 /* llg_cr_step only: evaluate the ready calls */
} llg_cr_status_t;

/* Leaf / inactive-variable / call-result input states. */
enum {
    LLG_CR_INPUT_KNOWN = 0,
    LLG_CR_INPUT_NULL = 1,   /* evaluating it dereferenced a null handle */
    LLG_CR_INPUT_XZ = 2,     /* it holds an X or Z bit (S29-D1) */
    LLG_CR_INPUT_EMPTY = 3   /* an and/or/xor reduction of an empty array
                                (S30-D6; set by the expansion only) */
};

/* Reasons for LLG_CR_ERROR, with the offending item in report fields. */
enum {
    LLG_CR_REASON_NONE = 0,
    LLG_CR_REASON_NULL,      /* leaf: null handle dereference */
    LLG_CR_REASON_XZ_LEAF,   /* leaf holds X/Z */
    LLG_CR_REASON_XZ_VAR,    /* var: inactive random variable holds X/Z */
    LLG_CR_REASON_XZ_CALL,   /* call: result holds X/Z */
    LLG_CR_REASON_DIVZERO,   /* node: division or modulo by zero */
    LLG_CR_REASON_INDEX,     /* node: select index out of range */
    LLG_CR_REASON_ORDER,     /* var: circular function-argument order */
    LLG_CR_REASON_WEIGHT,    /* node: negative or unevaluated dist weight
                                or bound (S30-D8) */
    LLG_CR_REASON_SOLVE_BEFORE, /* var: circular solve...before order */
    LLG_CR_REASON_EMPTY_REDUCTION /* leaf: and/or/xor of an empty array
                                     (S30-D6) */
};

/* Engines, as bits of llg_cr_report_t.engines. */
enum {
    LLG_CR_ENGINE_DIRECT = 1u << 0, /* unconstrained draws */
    LLG_CR_ENGINE_BDD = 1u << 1,    /* exact counting and uniform sampling */
    LLG_CR_ENGINE_REJECT = 1u << 2, /* uniform rejection sampling */
    LLG_CR_ENGINE_CDCL = 1u << 3,   /* search fallback, not uniform */
    LLG_CR_ENGINE_CACHE = 1u << 4   /* a cached exact model was reused */
};

typedef struct {
    uint32_t engines;
    int reason;
    uint32_t item;           /* leaf, var, call or node named by `reason` */
    const char* budget;      /* tunable that ran out, for LLG_CR_RESOURCE */
} llg_cr_report_t;

/* Generated function-call thunk (18.5.11). `args[i]` holds the words of
 * argument i; the callee writes the result words and returns an
 * LLG_CR_INPUT_* state. */
typedef int (*llg_cr_call_fn)(void* context, uint32_t call,
                              const uint64_t* const* args, uint64_t* result);

typedef struct llg_cr_session llg_cr_session_t;

/* Random source: `next` returns the stream's next 32-bit draw. The glue
 * passes the object's or the calling thread's llg_rng stream, so the solver
 * itself has no runtime dependency. */
typedef struct {
    uint32_t (*next)(void* stream);
    void* stream;
} llg_cr_rng_t;

/* Return the site's session, creating it on first use. Sessions live until
 * llg_cr_session_release_all (model shutdown). Allocation failure is fatal,
 * like every other runtime allocation. */
llg_cr_session_t* llg_cr_session_get(const llg_cr_problem_t* problem,
                                     llg_cr_session_t** slot);
void llg_cr_session_release_all(void);

/* Inputs for one solve; every variable, block and leaf must be set. */
void llg_cr_set_var(llg_cr_session_t* s, uint32_t var, const uint64_t* words,
                    int active, int input_state);
void llg_cr_set_block(llg_cr_session_t* s, uint32_t block, int enabled);
void llg_cr_set_leaf(llg_cr_session_t* s, uint32_t leaf, const uint64_t* words,
                     int input_state);
/* Cycle state of a randc variable (LLG_CR_CYCLIC_WORDS(width) words; NULL
 * starts a new cycle). After LLG_CR_OK, llg_cr_cyclic returns the updated
 * state to store back. */
void llg_cr_set_cyclic(llg_cr_session_t* s, uint32_t var, const uint64_t* words);
const uint64_t* llg_cr_cyclic(const llg_cr_session_t* s, uint32_t var);

/* Step-wise solving for generated code: llg_cr_begin, then llg_cr_step
 * until it returns something other than LLG_CR_NEED_CALLS. On NEED_CALLS,
 * evaluate every call for which llg_cr_call_ready holds (its arguments are
 * known; read them with llg_cr_call_arg, in the argument node's width) and
 * store each result with llg_cr_set_call_result (18.5.11). */
void llg_cr_begin(llg_cr_session_t* s);
llg_cr_status_t llg_cr_step(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                            llg_cr_report_t* report);
int llg_cr_call_ready(const llg_cr_session_t* s, uint32_t call);
const uint64_t* llg_cr_call_arg(const llg_cr_session_t* s, uint32_t call, uint32_t arg);
void llg_cr_set_call_result(llg_cr_session_t* s, uint32_t call, const uint64_t* words,
                            int input_state);

/* Whole solve with a thunk for the calls (runtime probes). */
llg_cr_status_t llg_cr_solve(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                             llg_cr_call_fn calls, void* context,
                             llg_cr_report_t* report);

/* Solution words of an active variable after LLG_CR_OK. */
const uint64_t* llg_cr_value(const llg_cr_session_t* s, uint32_t var);
uint32_t llg_cr_var_words(const llg_cr_session_t* s, uint32_t var);

/* ---- objects and arrays (problems with units, SIM-030) -------------------
 *
 * llg_cr_begin registers instance 0: unit 0, random, its pre_randomize
 * already called by the caller. llg_cr_inst_root gives it its handle (NULL
 * for std::randomize). The caller then feeds every instance llg_cr_next_inst
 * returns: its variables, blocks, leaves, cyclic states and arrays, and its
 * handle properties through llg_cr_link, which registers a new object
 * (identified by its handle) or returns the existing instance; an object
 * first reached through a non-random link and later through a random one is
 * returned again to be fed as random. llg_cr_inst_pre is 1 the first time an
 * instance is fed as random: the caller then calls its pre_randomize.
 * llg_cr_set_var and the other instance-0 forms set instance 0's variables
 * and the global ones. After LLG_CR_OK, commit with llg_cr_ivalue,
 * llg_cr_icyclic, llg_cr_array_size and llg_cr_elem_value. */
void llg_cr_inst_root(llg_cr_session_t* s, void* handle);
uint32_t llg_cr_next_inst(llg_cr_session_t* s);
/* The k-th random instance llg_cr_next_inst returned (depth-first preorder
 * over the links, each object once: the pre_randomize order, S30-D7), or
 * LLG_CR_NONE past the last; post_randomize runs in the same order. */
uint32_t llg_cr_visit(const llg_cr_session_t* s, uint32_t k);
int llg_cr_inst_pre(llg_cr_session_t* s, uint32_t inst);
uint32_t llg_cr_inst_count(const llg_cr_session_t* s);
void* llg_cr_inst_handle(const llg_cr_session_t* s, uint32_t inst);
uint32_t llg_cr_inst_unit(const llg_cr_session_t* s, uint32_t inst);
int llg_cr_inst_random(const llg_cr_session_t* s, uint32_t inst);
/* Link element `index` (0 for a scalar slot) of handle slot `slot` of
 * instance `inst` to `child` (NULL: a null handle) of class unit `unit`;
 * `random` says whether the child's variables and constraints take part
 * (the slot is `rand` and enabled). Returns the child's instance, or
 * LLG_CR_NONE for NULL. A handle array's element count is set first. */
uint32_t llg_cr_link(llg_cr_session_t* s, uint32_t inst, uint32_t slot, uint32_t index,
                     void* child, uint32_t unit, int random);
void llg_cr_set_slot_size(llg_cr_session_t* s, uint32_t inst, uint32_t slot, uint32_t count);
void llg_cr_iset_var(llg_cr_session_t* s, uint32_t inst, uint32_t var, const uint64_t* words,
                     int active, int input_state);
void llg_cr_iset_block(llg_cr_session_t* s, uint32_t inst, uint32_t block, int enabled);
void llg_cr_iset_leaf(llg_cr_session_t* s, uint32_t inst, uint32_t leaf,
                      const uint64_t* words, int input_state);
void llg_cr_iset_cyclic(llg_cr_session_t* s, uint32_t inst, uint32_t var,
                        const uint64_t* words);
/* Array `array` of instance `inst`: its current size and whether its
 * elements (and, for dynamic arrays and queues, its size) are random; then
 * each current element. */
void llg_cr_set_array(llg_cr_session_t* s, uint32_t inst, uint32_t array, uint32_t size,
                      int active);
void llg_cr_set_elem(llg_cr_session_t* s, uint32_t inst, uint32_t array, uint32_t index,
                     const uint64_t* words, int input_state);
const uint64_t* llg_cr_ivalue(const llg_cr_session_t* s, uint32_t inst, uint32_t var);
/* Whether variable `var` (array `array`) of `inst` took part as random. */
int llg_cr_iactive(const llg_cr_session_t* s, uint32_t inst, uint32_t var);
int llg_cr_array_active(const llg_cr_session_t* s, uint32_t inst, uint32_t array);
const uint64_t* llg_cr_icyclic(const llg_cr_session_t* s, uint32_t inst, uint32_t var);
uint32_t llg_cr_array_size(const llg_cr_session_t* s, uint32_t inst, uint32_t array);
const uint64_t* llg_cr_elem_value(const llg_cr_session_t* s, uint32_t inst, uint32_t array,
                                  uint32_t index);

/* Budgets (named tunables, environment overrides read once). */
typedef struct {
    uint64_t bdd_nodes;      /* LLG_RAND_BDD_NODES */
    uint64_t bdd_bits;       /* LLG_RAND_BDD_BITS */
    uint64_t reject_tries;   /* LLG_RAND_REJECT_TRIES */
    uint64_t conflicts;      /* LLG_RAND_CONFLICTS */
    uint64_t array_elements; /* LLG_RAND_ARRAY_ELEMENTS: elements per solve */
    uint64_t expand_nodes;   /* LLG_RAND_EXPAND_NODES: expanded nodes */
    int stats;               /* LLG_RAND_STATS */
    int overflow_memo;       /* LLG_RAND_OVERFLOW_MEMO: 0 rebuilds every
                              * overflowing exact model (never changes values) */
    int search_cache;        /* LLG_RAND_SEARCH_CACHE: 0 re-encodes every
                              * search (never changes values) */
} llg_cr_tunables_t;

const llg_cr_tunables_t* llg_cr_tunables(void);

#ifdef __cplusplus
}
#endif

#endif
