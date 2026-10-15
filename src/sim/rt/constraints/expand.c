/* ---- objects and arrays: template expansion (SIM-030) ---------------------
 *
 * A problem with units is a template. Its constraints name variables,
 * arrays and handle properties per class (unit), and the caller registers
 * the objects of one randomize call as instances (llg_cr_link). Each solve
 * expands the template over the instances' object graph and array sizes
 * into an ordinary problem: FOREACH and REDUCE become conjunctions and
 * operator chains, handle and array accesses become the selected objects'
 * variables, a random array index becomes an LLG_CR_ELEM table. That
 * problem is then solved by the engines above.
 *
 * Sizes first (S30-D5): when a random dynamic array or queue exists, a first
 * expansion replaces each such size by a 32-bit variable ordered before
 * every other variable and drops the conjuncts that need the elements of an
 * array whose size is not known yet; the second expansion fixes the sizes
 * it chose and solves everything. A size that no kept conjunct of an
 * enabled block constrains keeps its current value.
 *
 * Expanded problems are cached by structure (instances, units, links,
 * sizes, which arrays are random), so repeated calls on the same object
 * graph reuse their sessions and exact-engine caches. Budgets:
 * LLG_RAND_ARRAY_ELEMENTS bounds the random array elements of one solve,
 * LLG_RAND_EXPAND_NODES the nodes of one expansion; exceeding either is
 * LLG_CR_RESOURCE.
 */

#define CR_TPL_FLATS 4u
#define CR_X_NULL 0xFFFFFFFEu /* a ref that names no object */
#define CR_X_DROP 0xFFFFFFFDu /* needs elements of an array of unknown size */

enum { CR_FV_UNIT = 0, CR_FV_GLOBAL = 1, CR_FV_ELEM = 2, CR_FV_SIZE = 3 };
enum { CR_PH_IDLE = 0, CR_PH_SIZES = 1, CR_PH_ELEMS = 2, CR_PH_DONE = 3 };

#define CR_VEC_PUSH(vec, len, cap) \
    ((vec) = cr_grow((vec), &(cap), (len) + 1u, sizeof(*(vec))), &(vec)[(len)++])

typedef struct {
    uint32_t size;      /* current size */
    uint32_t chosen;    /* size of the solution */
    uint8_t active;
    uint64_t* words;
    size_t words_cap;
    uint8_t* input;
    size_t input_cap;
} cr_iarr_t;

typedef struct {
    uint32_t count;
    uint32_t* target;
    size_t cap;
} cr_islot_t;

typedef struct {
    void* handle;
    uint32_t unit;      /* unit of the buffers, LLG_CR_NONE before the first */
    uint8_t random, pre, queued;
    uint64_t* var_cur;
    uint8_t* var_active;
    uint8_t* var_input;
    uint64_t* cyc;
    uint8_t* block_en;
    uint64_t* leaf_val;
    uint8_t* leaf_input;
    cr_iarr_t* arrays;
    cr_islot_t* slots;
} cr_inst_t;

/* One expanded problem with its session and the maps back to instances. */
typedef struct {
    llg_cr_problem_t problem;
    llg_cr_session_t* session;
    cr_u32vec_t key;
    uint64_t used;
    int phase;
    llg_cr_node_t* nodes;
    size_t nodes_len, nodes_cap;
    cr_u64vec_t consts;
    cr_u32vec_t args;
    llg_cr_var_t* vars;
    size_t vars_len, vars_cap;
    llg_cr_block_t* blocks;
    size_t blocks_len, blocks_cap;
    llg_cr_leaf_t* leaves;
    size_t leaves_len, leaves_cap;
    llg_cr_call_t* calls;
    llg_cr_dist_t* dists;
    size_t dists_len, dists_cap;
    llg_cr_dist_item_t* items;
    size_t items_len, items_cap;
    llg_cr_order_t* orders;
    size_t orders_len, orders_cap;
    llg_cr_table_t* tables;
    size_t tables_len, tables_cap;
    cr_u32vec_t table_items;
    /* Origins: flat variable -> (kind, instance, template index, element),
     * flat leaf/block -> (instance or LLG_CR_NONE, template index). */
    cr_u32vec_t var_kind, var_inst, var_idx, var_elem;
    cr_u32vec_t leaf_inst, leaf_idx;
    cr_u32vec_t block_inst, block_idx;
    cr_u32vec_t size_refs; /* (size variable, flat block) pairs */
    /* Maps: per instance position -> flat index (LLG_CR_NONE if absent). */
    uint32_t ninst;
    uint32_t* var_base;
    uint32_t* var_map;
    uint32_t* global_map;
    uint32_t* leaf_base;
    uint32_t* leaf_map;
    uint32_t* block_base;
    uint32_t* block_map;
    uint32_t* arr_base;
    uint32_t* arr_size;
    uint32_t* arr_svar;
    uint32_t* arr_table;
    uint32_t* arr_elem;
    cr_u32vec_t elem_map;
} cr_flat_t;

struct cr_tpl {
    const llg_cr_problem_t* p;
    uint32_t vc, ac, sc, uc;
    uint32_t* var_pos;      /* [unit * vc + var] -> position, or NONE */
    uint32_t* arr_pos;
    uint32_t* slot_pos;
    uint32_t* var_off;      /* per unit (var_count + 1) word offsets */
    uint32_t* var_off_base;
    uint32_t* cyc_off;      /* per unit (var_count + 1) cycle-state offsets */
    uint32_t* leaf_off;     /* per leaf: word offset within its unit */
    uint32_t* leaf_words;   /* per unit */
    uint32_t* leaf_unit;
    uint32_t* block_unit;
    uint8_t* global;        /* per var */
    uint32_t* g_off;
    uint32_t* g_cyc_off;
    uint64_t* g_cur;
    uint8_t* g_active;
    uint8_t* g_input;
    uint64_t* g_cyc;
    uint64_t* deps;         /* per template node: loop indices it reads */
    uint32_t loops;
    cr_inst_t* inst;
    uint32_t count, cap;
    cr_u32vec_t queue;
    size_t qhead;
    void** hkey;
    uint32_t* hval;
    size_t hcap, hcount;
    int phase;
    cr_flat_t* flat;
    cr_flat_t* cache[CR_TPL_FLATS];
    uint64_t clock;
};

/* ---- template layout -------------------------------------------------- */

static const uint32_t* cr_unit_vars(const llg_cr_problem_t* p, uint32_t u) {
    return p->unit_items + p->units[u].item_first;
}

static const uint32_t* cr_unit_arrays(const llg_cr_problem_t* p, uint32_t u) {
    return p->unit_items + p->units[u].item_first + p->units[u].var_count;
}

static const uint32_t* cr_unit_slots(const llg_cr_problem_t* p, uint32_t u) {
    return p->unit_items + p->units[u].item_first + p->units[u].var_count +
           p->units[u].array_count;
}

/* Node operands of a template node; FOREACH and REDUCE bodies are not
 * operands (they are expanded once per element). */
static void cr_tpl_operands(const llg_cr_problem_t* p, uint32_t n, cr_u32vec_t* out) {
    const llg_cr_node_t* nd = &p->nodes[n];
    switch (nd->op) {
    case LLG_CR_REF:
        if (nd->b != LLG_CR_NONE) cr_u32_push(out, nd->b);
        if (nd->c != LLG_CR_NONE) cr_u32_push(out, nd->c);
        return;
    case LLG_CR_HVAR:
    case LLG_CR_ISNULL:
        if (nd->a != LLG_CR_NONE) cr_u32_push(out, nd->a);
        return;
    case LLG_CR_AELEM:
        cr_u32_push(out, nd->b);
        if (nd->c != LLG_CR_NONE) cr_u32_push(out, nd->c);
        return;
    case LLG_CR_ASIZE:
        if (nd->c != LLG_CR_NONE) cr_u32_push(out, nd->c);
        return;
    case LLG_CR_ITER:
        return;
    case LLG_CR_FOREACH:
    case LLG_CR_REDUCE:
        if (p->args[nd->b + 2u] != LLG_CR_NONE) cr_u32_push(out, p->args[nd->b + 2u]);
        return;
    default:
        cr_push_operands(p, n, out);
        return;
    }
}

static int cr_is_ref(const llg_cr_problem_t* p, uint32_t x, uint32_t n) {
    return x == LLG_CR_NONE || (x < n && p->nodes[x].op == LLG_CR_REF);
}

static int cr_tpl_array_ok(const llg_cr_problem_t* p, uint32_t a) {
    if (a & LLG_CR_SLOT_ARRAY) {
        uint32_t slot = a & ~LLG_CR_SLOT_ARRAY;
        return slot < p->slot_count && p->slots[slot].is_array;
    }
    return a < p->array_count;
}

static void cr_tpl_validate(const llg_cr_problem_t* p) {
    if (p->table_count || p->unit_count == 0u) cr_fatal("invalid template problem");
    for (uint32_t u = 0; u < p->unit_count; ++u) {
        const llg_cr_unit_t* unit = &p->units[u];
        uint64_t items = (uint64_t)unit->var_count + unit->array_count + unit->slot_count;
        if ((uint64_t)unit->item_first + items > p->unit_item_count ||
            (uint64_t)unit->block_first + unit->block_count > p->block_count ||
            (uint64_t)unit->leaf_first + unit->leaf_count > p->leaf_count)
            cr_fatal("unit out of range");
        for (uint32_t i = 0; i < unit->var_count; ++i)
            if (cr_unit_vars(p, u)[i] >= p->var_count) cr_fatal("unit variable out of range");
        for (uint32_t i = 0; i < unit->array_count; ++i)
            if (cr_unit_arrays(p, u)[i] >= p->array_count) cr_fatal("unit array out of range");
        for (uint32_t i = 0; i < unit->slot_count; ++i)
            if (cr_unit_slots(p, u)[i] >= p->slot_count) cr_fatal("unit slot out of range");
    }
    for (uint32_t a = 0; a < p->array_count; ++a)
        if (p->arrays[a].elem_width == 0u || p->arrays[a].kind > LLG_CR_ARRAY_QUEUE)
            cr_fatal("invalid array");
    for (uint32_t n = 0; n < p->node_count; ++n) {
        const llg_cr_node_t* nd = &p->nodes[n];
        switch (nd->op) {
        case LLG_CR_ELEM:
            cr_fatal("element table in a template");
            break;
        case LLG_CR_REF:
            if (nd->a >= p->slot_count || !cr_is_ref(p, nd->b, n) ||
                (nd->c != LLG_CR_NONE && nd->c >= n))
                cr_fatal("invalid object reference");
            break;
        case LLG_CR_HVAR:
            if (!cr_is_ref(p, nd->a, n) || nd->b >= p->var_count) cr_fatal("invalid object variable");
            break;
        case LLG_CR_ISNULL:
            if (!cr_is_ref(p, nd->a, n)) cr_fatal("invalid null test");
            break;
        case LLG_CR_AELEM:
            if (nd->a >= p->array_count || nd->b >= n || !cr_is_ref(p, nd->c, n))
                cr_fatal("invalid array element");
            break;
        case LLG_CR_ASIZE:
            if (!cr_tpl_array_ok(p, nd->a) || !cr_is_ref(p, nd->c, n)) cr_fatal("invalid array size");
            break;
        case LLG_CR_ITER:
            break;
        case LLG_CR_FOREACH:
        case LLG_CR_REDUCE: {
            uint32_t count = nd->op == LLG_CR_FOREACH ? 4u : 5u;
            if ((uint64_t)nd->b + count > p->arg_count) cr_fatal("loop operands out of range");
            const uint32_t* x = p->args + nd->b;
            if (!cr_tpl_array_ok(p, x[0]) || x[1] >= n || !cr_is_ref(p, x[2], n))
                cr_fatal("invalid loop");
            if (nd->op == LLG_CR_REDUCE && x[4] != LLG_CR_ADD && x[4] != LLG_CR_MUL &&
                x[4] != LLG_CR_AND && x[4] != LLG_CR_OR && x[4] != LLG_CR_XOR &&
                x[4] != LLG_CR_LAND && x[4] != LLG_CR_LOR)
                cr_fatal("invalid reduction");
            break;
        }
        default: {
            /* Ordinary nodes: their operands must not be refs. */
            cr_u32vec_t ops = {0, 0, 0};
            if (nd->op == 0 || nd->op >= LLG_CR_OP_LIMIT) cr_fatal("invalid node operation");
            if (nd->op == LLG_CR_CONCAT || nd->op == LLG_CR_ALL) {
                if ((uint64_t)nd->b + nd->c > p->arg_count) cr_fatal("operand list out of range");
            } else if (nd->op == LLG_CR_DIST) {
                if (nd->a >= p->dist_count) cr_fatal("dist out of range");
                const llg_cr_dist_t* d = &p->dists[nd->a];
                if ((uint64_t)d->item_first + d->item_count > p->dist_item_count)
                    cr_fatal("dist items out of range");
            } else if (nd->op == LLG_CR_CALL) {
                if (nd->a >= p->call_count) cr_fatal("call out of range");
                const llg_cr_call_t* c = &p->calls[nd->a];
                if ((uint64_t)c->arg_first + c->arg_count > p->arg_count)
                    cr_fatal("call arguments out of range");
            } else if (nd->op == LLG_CR_CONST) {
                if ((uint64_t)nd->a + CR_WORDS(nd->bit_width) > p->const_count)
                    cr_fatal("constant out of range");
            } else if (nd->op == LLG_CR_VAR) {
                if (nd->a >= p->var_count) cr_fatal("variable out of range");
            } else if (nd->op == LLG_CR_STATE) {
                if (nd->a >= p->leaf_count) cr_fatal("state leaf out of range");
            }
            cr_push_operands(p, n, &ops);
            for (size_t i = 0; i < ops.len; ++i)
                if (ops.data[i] >= n || p->nodes[ops.data[i]].op == LLG_CR_REF)
                    cr_fatal("invalid operand");
            free(ops.data);
            break;
        }
        }
        if (nd->bit_width == 0u) cr_fatal("zero-width node");
    }
    for (uint32_t b = 0; b < p->block_count; ++b)
        if (p->blocks[b].root >= p->node_count) cr_fatal("block root out of range");
    for (uint32_t o = 0; o < p->order_count; ++o)
        if (p->orders[o].block >= p->block_count || p->orders[o].before >= p->var_count ||
            p->orders[o].after >= p->var_count)
            cr_fatal("solve-before order out of range");
    for (uint32_t v = 0; v < p->var_count; ++v) {
        if (p->vars[v].bit_width == 0u) cr_fatal("zero-width variable");
        if (p->vars[v].cyclic && p->vars[v].bit_width > LLG_CR_CYCLIC_MAX_WIDTH)
            cr_fatal("randc variable too wide");
        if (p->vars[v].domain_node != LLG_CR_NONE && p->vars[v].domain_node >= p->node_count)
            cr_fatal("domain node out of range");
    }
}

static cr_tpl_t* cr_tpl_new(const llg_cr_problem_t* p) {
    cr_tpl_validate(p);
    cr_tpl_t* t = (cr_tpl_t*)cr_alloc(1u, sizeof(*t));
    t->p = p;
    t->vc = p->var_count;
    t->ac = p->array_count;
    t->sc = p->slot_count;
    t->uc = p->unit_count;
    size_t uc = t->uc;
    t->var_pos = (uint32_t*)cr_alloc(uc * (t->vc ? t->vc : 1u), sizeof(uint32_t));
    t->arr_pos = (uint32_t*)cr_alloc(uc * (t->ac ? t->ac : 1u), sizeof(uint32_t));
    t->slot_pos = (uint32_t*)cr_alloc(uc * (t->sc ? t->sc : 1u), sizeof(uint32_t));
    memset(t->var_pos, 0xFF, uc * (t->vc ? t->vc : 1u) * sizeof(uint32_t));
    memset(t->arr_pos, 0xFF, uc * (t->ac ? t->ac : 1u) * sizeof(uint32_t));
    memset(t->slot_pos, 0xFF, uc * (t->sc ? t->sc : 1u) * sizeof(uint32_t));
    t->global = (uint8_t*)cr_alloc(t->vc ? t->vc : 1u, 1u);
    for (uint32_t v = 0; v < t->vc; ++v) t->global[v] = 1;
    t->var_off_base = (uint32_t*)cr_alloc(uc + 1u, sizeof(uint32_t));
    size_t total = 0;
    for (uint32_t u = 0; u < t->uc; ++u) {
        t->var_off_base[u] = (uint32_t)total;
        total += p->units[u].var_count + 1u;
    }
    t->var_off = (uint32_t*)cr_alloc(total ? total : 1u, sizeof(uint32_t));
    t->cyc_off = (uint32_t*)cr_alloc(total ? total : 1u, sizeof(uint32_t));
    for (uint32_t u = 0; u < t->uc; ++u) {
        const llg_cr_unit_t* unit = &p->units[u];
        uint32_t* off = t->var_off + t->var_off_base[u];
        uint32_t* cyc = t->cyc_off + t->var_off_base[u];
        uint32_t words = 0, cwords = 0;
        for (uint32_t i = 0; i < unit->var_count; ++i) {
            uint32_t v = cr_unit_vars(p, u)[i];
            if (t->var_pos[(size_t)u * t->vc + v] != LLG_CR_NONE) cr_fatal("variable listed twice");
            t->var_pos[(size_t)u * t->vc + v] = i;
            t->global[v] = 0;
            off[i] = words;
            cyc[i] = cwords;
            words += CR_WORDS(p->vars[v].bit_width);
            if (p->vars[v].cyclic) cwords += LLG_CR_CYCLIC_WORDS(p->vars[v].bit_width);
        }
        off[unit->var_count] = words;
        cyc[unit->var_count] = cwords;
        for (uint32_t i = 0; i < unit->array_count; ++i)
            t->arr_pos[(size_t)u * t->ac + cr_unit_arrays(p, u)[i]] = i;
        for (uint32_t i = 0; i < unit->slot_count; ++i)
            t->slot_pos[(size_t)u * t->sc + cr_unit_slots(p, u)[i]] = i;
    }
    t->g_off = (uint32_t*)cr_alloc(t->vc ? t->vc : 1u, sizeof(uint32_t));
    t->g_cyc_off = (uint32_t*)cr_alloc(t->vc ? t->vc : 1u, sizeof(uint32_t));
    uint32_t gw = 0, gc = 0;
    for (uint32_t v = 0; v < t->vc; ++v) {
        t->g_off[v] = gw;
        t->g_cyc_off[v] = gc;
        if (!t->global[v]) continue;
        gw += CR_WORDS(p->vars[v].bit_width);
        if (p->vars[v].cyclic) gc += LLG_CR_CYCLIC_WORDS(p->vars[v].bit_width);
    }
    t->g_cur = (uint64_t*)cr_alloc(gw ? gw : 1u, sizeof(uint64_t));
    t->g_cyc = (uint64_t*)cr_alloc(gc ? gc : 1u, sizeof(uint64_t));
    t->g_active = (uint8_t*)cr_alloc(t->vc ? t->vc : 1u, 1u);
    t->g_input = (uint8_t*)cr_alloc(t->vc ? t->vc : 1u, 1u);
    t->leaf_off = (uint32_t*)cr_alloc(p->leaf_count ? p->leaf_count : 1u, sizeof(uint32_t));
    t->leaf_unit = (uint32_t*)cr_alloc(p->leaf_count ? p->leaf_count : 1u, sizeof(uint32_t));
    t->block_unit = (uint32_t*)cr_alloc(p->block_count ? p->block_count : 1u, sizeof(uint32_t));
    t->leaf_words = (uint32_t*)cr_alloc(uc, sizeof(uint32_t));
    memset(t->leaf_unit, 0xFF, (p->leaf_count ? p->leaf_count : 1u) * sizeof(uint32_t));
    memset(t->block_unit, 0xFF, (p->block_count ? p->block_count : 1u) * sizeof(uint32_t));
    for (uint32_t u = 0; u < t->uc; ++u) {
        const llg_cr_unit_t* unit = &p->units[u];
        uint32_t words = 0;
        for (uint32_t l = unit->leaf_first; l < unit->leaf_first + unit->leaf_count; ++l) {
            if (t->leaf_unit[l] != LLG_CR_NONE) cr_fatal("leaf in two units");
            t->leaf_unit[l] = u;
            t->leaf_off[l] = words;
            words += CR_WORDS(p->leaves[l].bit_width);
        }
        t->leaf_words[u] = words;
        for (uint32_t b = unit->block_first; b < unit->block_first + unit->block_count; ++b) {
            if (t->block_unit[b] != LLG_CR_NONE) cr_fatal("block in two units");
            t->block_unit[b] = u;
        }
    }
    for (uint32_t l = 0; l < p->leaf_count; ++l)
        if (t->leaf_unit[l] == LLG_CR_NONE) cr_fatal("leaf without a unit");
    for (uint32_t b = 0; b < p->block_count; ++b)
        if (t->block_unit[b] == LLG_CR_NONE) cr_fatal("block without a unit");
    /* Loop dependencies (bit 63 stands for loops 63 and above). */
    t->deps = (uint64_t*)cr_alloc(p->node_count ? p->node_count : 1u, sizeof(uint64_t));
    cr_u32vec_t ops = {0, 0, 0};
    for (uint32_t n = 0; n < p->node_count; ++n) {
        const llg_cr_node_t* nd = &p->nodes[n];
        uint64_t d = 0;
        ops.len = 0;
        cr_tpl_operands(p, n, &ops);
        for (size_t i = 0; i < ops.len; ++i) d |= t->deps[ops.data[i]];
        if (nd->op == LLG_CR_ITER) {
            d |= UINT64_C(1) << (nd->a < 63u ? nd->a : 63u);
            if (nd->a + 1u > t->loops) t->loops = nd->a + 1u;
        } else if (nd->op == LLG_CR_FOREACH || nd->op == LLG_CR_REDUCE) {
            uint32_t loop = p->args[nd->b + 3u];
            uint64_t body = t->deps[p->args[nd->b + 1u]];
            if (loop < 63u) body &= ~(UINT64_C(1) << loop);
            d |= body;
            if (loop + 1u > t->loops) t->loops = loop + 1u;
        }
        t->deps[n] = d;
    }
    free(ops.data);
    t->phase = CR_PH_IDLE;
    return t;
}

/* ---- instances -------------------------------------------------------- */

static void cr_inst_release(cr_inst_t* in, const cr_tpl_t* t) {
    if (in->unit != LLG_CR_NONE) {
        const llg_cr_unit_t* unit = &t->p->units[in->unit];
        for (uint32_t i = 0; i < unit->array_count; ++i) {
            free(in->arrays[i].words);
            free(in->arrays[i].input);
        }
        for (uint32_t i = 0; i < unit->slot_count; ++i) free(in->slots[i].target);
    }
    free(in->var_cur); free(in->var_active); free(in->var_input); free(in->cyc);
    free(in->block_en); free(in->leaf_val); free(in->leaf_input);
    free(in->arrays); free(in->slots);
    memset(in, 0, sizeof(*in));
    in->unit = LLG_CR_NONE;
}

static void cr_inst_prepare(cr_tpl_t* t, cr_inst_t* in, uint32_t unit) {
    if (in->unit != unit) {
        cr_inst_release(in, t);
        const llg_cr_unit_t* u = &t->p->units[unit];
        const uint32_t* off = t->var_off + t->var_off_base[unit];
        const uint32_t* cyc = t->cyc_off + t->var_off_base[unit];
        in->unit = unit;
        in->var_cur = (uint64_t*)cr_alloc(off[u->var_count] ? off[u->var_count] : 1u, sizeof(uint64_t));
        in->var_active = (uint8_t*)cr_alloc(u->var_count ? u->var_count : 1u, 1u);
        in->var_input = (uint8_t*)cr_alloc(u->var_count ? u->var_count : 1u, 1u);
        in->cyc = (uint64_t*)cr_alloc(cyc[u->var_count] ? cyc[u->var_count] : 1u, sizeof(uint64_t));
        in->block_en = (uint8_t*)cr_alloc(u->block_count ? u->block_count : 1u, 1u);
        in->leaf_val = (uint64_t*)cr_alloc(t->leaf_words[unit] ? t->leaf_words[unit] : 1u, sizeof(uint64_t));
        in->leaf_input = (uint8_t*)cr_alloc(u->leaf_count ? u->leaf_count : 1u, 1u);
        in->arrays = (cr_iarr_t*)cr_alloc(u->array_count ? u->array_count : 1u, sizeof(cr_iarr_t));
        in->slots = (cr_islot_t*)cr_alloc(u->slot_count ? u->slot_count : 1u, sizeof(cr_islot_t));
    }
    const llg_cr_unit_t* u = &t->p->units[unit];
    for (uint32_t i = 0; i < u->array_count; ++i) {
        in->arrays[i].size = in->arrays[i].chosen = 0;
        in->arrays[i].active = 0;
    }
    for (uint32_t i = 0; i < u->slot_count; ++i) {
        cr_islot_t* sl = &in->slots[i];
        const llg_cr_slot_t* slot = &t->p->slots[cr_unit_slots(t->p, unit)[i]];
        sl->count = slot->is_array ? 0u : 1u;
        sl->target = (uint32_t*)cr_grow(sl->target, &sl->cap, 1u, sizeof(uint32_t));
        sl->target[0] = LLG_CR_NONE;
    }
}

static size_t cr_ptr_hash(const void* handle, size_t cap) {
    uint64_t x = (uint64_t)(uintptr_t)handle;
    return (size_t)(cr_mix(x) & (uint64_t)(cap - 1u));
}

static void cr_hmap_insert(cr_tpl_t* t, void* handle, uint32_t inst) {
    if ((t->hcount + 1u) * 2u > t->hcap) {
        size_t cap = t->hcap ? t->hcap * 2u : 64u;
        void** keys = (void**)cr_alloc(cap, sizeof(void*));
        uint32_t* vals = (uint32_t*)cr_alloc(cap, sizeof(uint32_t));
        for (size_t i = 0; i < t->hcap; ++i) {
            if (!t->hkey[i]) continue;
            size_t h = cr_ptr_hash(t->hkey[i], cap);
            while (keys[h]) h = (h + 1u) & (cap - 1u);
            keys[h] = t->hkey[i];
            vals[h] = t->hval[i];
        }
        free(t->hkey);
        free(t->hval);
        t->hkey = keys;
        t->hval = vals;
        t->hcap = cap;
    }
    size_t h = cr_ptr_hash(handle, t->hcap);
    while (t->hkey[h]) h = (h + 1u) & (t->hcap - 1u);
    t->hkey[h] = handle;
    t->hval[h] = inst;
    ++t->hcount;
}

static uint32_t cr_hmap_find(const cr_tpl_t* t, const void* handle) {
    if (!t->hcap) return LLG_CR_NONE;
    size_t h = cr_ptr_hash(handle, t->hcap);
    while (t->hkey[h]) {
        if (t->hkey[h] == handle) return t->hval[h];
        h = (h + 1u) & (t->hcap - 1u);
    }
    return LLG_CR_NONE;
}

static uint32_t cr_inst_add(cr_tpl_t* t, void* handle, uint32_t unit, int random) {
    if (unit >= t->uc) cr_fatal("unit out of range");
    if (t->count == t->cap) {
        uint32_t cap = t->cap ? t->cap * 2u : 8u;
        cr_inst_t* grown = (cr_inst_t*)realloc(t->inst, (size_t)cap * sizeof(cr_inst_t));
        if (!grown) cr_fatal("out of memory");
        for (uint32_t i = t->cap; i < cap; ++i) {
            memset(&grown[i], 0, sizeof(cr_inst_t));
            grown[i].unit = LLG_CR_NONE;
        }
        t->inst = grown;
        t->cap = cap;
    }
    uint32_t id = t->count++;
    cr_inst_t* in = &t->inst[id];
    cr_inst_prepare(t, in, unit);
    in->handle = handle;
    in->random = (uint8_t)(random != 0);
    in->pre = 0;
    in->queued = 1;
    cr_u32_push(&t->queue, id);
    if (handle) cr_hmap_insert(t, handle, id);
    return id;
}

static cr_inst_t* cr_inst_get(const llg_cr_session_t* s, uint32_t inst) {
    if (!s->tpl || inst >= s->tpl->count) cr_fatal("instance out of range");
    return &s->tpl->inst[inst];
}

static void cr_tpl_begin(llg_cr_session_t* s) {
    cr_tpl_t* t = s->tpl;
    t->count = 0;
    t->queue.len = 0;
    t->qhead = 0;
    if (t->hcap) {
        memset(t->hkey, 0, t->hcap * sizeof(void*));
        t->hcount = 0;
    }
    t->phase = CR_PH_IDLE;
    t->flat = NULL;
    uint32_t root = cr_inst_add(t, NULL, 0u, 1);
    t->inst[root].pre = 1;
}

void llg_cr_inst_root(llg_cr_session_t* s, void* handle) {
    cr_inst_t* in = cr_inst_get(s, 0u);
    if (in->handle || !handle) return;
    in->handle = handle;
    cr_hmap_insert(s->tpl, handle, 0u);
}

uint32_t llg_cr_next_inst(llg_cr_session_t* s) {
    cr_tpl_t* t = s->tpl;
    if (!t) cr_fatal("not a problem with units");
    if (t->qhead >= t->queue.len) return LLG_CR_NONE;
    uint32_t id = t->queue.data[t->qhead++];
    t->inst[id].queued = 0;
    return id;
}

int llg_cr_inst_pre(llg_cr_session_t* s, uint32_t inst) {
    cr_inst_t* in = cr_inst_get(s, inst);
    if (!in->random || in->pre) return 0;
    in->pre = 1;
    return 1;
}

uint32_t llg_cr_inst_count(const llg_cr_session_t* s) {
    return s->tpl ? s->tpl->count : 0u;
}

void* llg_cr_inst_handle(const llg_cr_session_t* s, uint32_t inst) {
    return cr_inst_get(s, inst)->handle;
}

uint32_t llg_cr_inst_unit(const llg_cr_session_t* s, uint32_t inst) {
    return cr_inst_get(s, inst)->unit;
}

int llg_cr_inst_random(const llg_cr_session_t* s, uint32_t inst) {
    return cr_inst_get(s, inst)->random;
}

static cr_islot_t* cr_inst_slot(const llg_cr_session_t* s, uint32_t inst, uint32_t slot) {
    cr_inst_t* in = cr_inst_get(s, inst);
    const cr_tpl_t* t = s->tpl;
    if (slot >= t->sc) cr_fatal("slot out of range");
    uint32_t pos = t->slot_pos[(size_t)in->unit * t->sc + slot];
    if (pos == LLG_CR_NONE) cr_fatal("slot not in the instance's unit");
    return &in->slots[pos];
}

void llg_cr_set_slot_size(llg_cr_session_t* s, uint32_t inst, uint32_t slot, uint32_t count) {
    if (!s->tpl->p->slots[slot].is_array) cr_fatal("not a handle array");
    cr_islot_t* sl = cr_inst_slot(s, inst, slot);
    sl->target = (uint32_t*)cr_grow(sl->target, &sl->cap, (size_t)count + 1u, sizeof(uint32_t));
    for (uint32_t i = 0; i < count; ++i) sl->target[i] = LLG_CR_NONE;
    sl->count = count;
}

uint32_t llg_cr_link(llg_cr_session_t* s, uint32_t inst, uint32_t slot, uint32_t index,
                     void* child, uint32_t unit, int random) {
    cr_tpl_t* t = s->tpl;
    cr_islot_t* sl = cr_inst_slot(s, inst, slot);
    if (index >= sl->count) cr_fatal("handle array index out of range");
    if (!child) {
        sl->target[index] = LLG_CR_NONE;
        return LLG_CR_NONE;
    }
    /* Only a random object's random handles recurse (18.4, 18.5.10). */
    random = random && t->inst[inst].random;
    uint32_t id = cr_hmap_find(t, child);
    if (id == LLG_CR_NONE) {
        id = cr_inst_add(t, child, unit, random);
    } else {
        if (t->inst[id].unit != unit) cr_fatal("object linked with two units");
        if (random && !t->inst[id].random) {
            /* Reached again through a random handle: feed it again. */
            t->inst[id].random = 1;
            if (!t->inst[id].queued) {
                t->inst[id].queued = 1;
                cr_u32_push(&t->queue, id);
            }
        }
    }
    /* `sl` may have moved if the instance table grew. */
    sl = cr_inst_slot(s, inst, slot);
    sl->target[index] = id;
    return id;
}

static uint32_t cr_inst_var(const cr_tpl_t* t, const cr_inst_t* in, uint32_t var) {
    if (var >= t->vc) cr_fatal("variable out of range");
    uint32_t pos = t->var_pos[(size_t)in->unit * t->vc + var];
    if (pos == LLG_CR_NONE) cr_fatal("variable not in the instance's unit");
    return pos;
}

static void cr_store_words(uint64_t* d, const uint64_t* words, uint32_t width) {
    uint32_t n = CR_WORDS(width);
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
    bv_mask(d, width);
}

void llg_cr_iset_var(llg_cr_session_t* s, uint32_t inst, uint32_t var, const uint64_t* words,
                     int active, int input_state) {
    cr_tpl_t* t = s->tpl;
    if (!t) cr_fatal("not a problem with units");
    if (var >= t->vc) cr_fatal("variable out of range");
    uint32_t w = t->p->vars[var].bit_width;
    if (inst == LLG_CR_NONE || t->global[var]) {
        if (!t->global[var]) cr_fatal("not a global variable");
        cr_store_words(t->g_cur + t->g_off[var], words, w);
        t->g_active[var] = (uint8_t)(active != 0);
        t->g_input[var] = (uint8_t)input_state;
        return;
    }
    cr_inst_t* in = cr_inst_get(s, inst);
    uint32_t pos = cr_inst_var(t, in, var);
    cr_store_words(in->var_cur + t->var_off[t->var_off_base[in->unit] + pos], words, w);
    in->var_active[pos] = (uint8_t)(active != 0);
    in->var_input[pos] = (uint8_t)input_state;
}

void llg_cr_iset_block(llg_cr_session_t* s, uint32_t inst, uint32_t block, int enabled) {
    cr_inst_t* in = cr_inst_get(s, inst);
    const llg_cr_unit_t* u = &s->tpl->p->units[in->unit];
    if (block < u->block_first || block >= u->block_first + u->block_count)
        cr_fatal("block not in the instance's unit");
    in->block_en[block - u->block_first] = (uint8_t)(enabled != 0);
}

void llg_cr_iset_leaf(llg_cr_session_t* s, uint32_t inst, uint32_t leaf,
                      const uint64_t* words, int input_state) {
    cr_inst_t* in = cr_inst_get(s, inst);
    const cr_tpl_t* t = s->tpl;
    const llg_cr_unit_t* u = &t->p->units[in->unit];
    if (leaf < u->leaf_first || leaf >= u->leaf_first + u->leaf_count)
        cr_fatal("leaf not in the instance's unit");
    cr_store_words(in->leaf_val + t->leaf_off[leaf], words, t->p->leaves[leaf].bit_width);
    in->leaf_input[leaf - u->leaf_first] = (uint8_t)input_state;
}

static uint64_t* cr_cyc_slot(const llg_cr_session_t* s, uint32_t inst, uint32_t var) {
    cr_tpl_t* t = s->tpl;
    if (var >= t->vc || !t->p->vars[var].cyclic) cr_fatal("not a randc variable");
    if (inst == LLG_CR_NONE || t->global[var]) {
        if (!t->global[var]) cr_fatal("not a global variable");
        return t->g_cyc + t->g_cyc_off[var];
    }
    cr_inst_t* in = cr_inst_get(s, inst);
    uint32_t pos = cr_inst_var(t, in, var);
    return in->cyc + t->cyc_off[t->var_off_base[in->unit] + pos];
}

void llg_cr_iset_cyclic(llg_cr_session_t* s, uint32_t inst, uint32_t var,
                        const uint64_t* words) {
    uint64_t* d = cr_cyc_slot(s, inst, var);
    uint32_t n = LLG_CR_CYCLIC_WORDS(s->tpl->p->vars[var].bit_width);
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
}

const uint64_t* llg_cr_icyclic(const llg_cr_session_t* s, uint32_t inst, uint32_t var) {
    return cr_cyc_slot(s, inst, var);
}

static cr_iarr_t* cr_inst_array(const llg_cr_session_t* s, uint32_t inst, uint32_t array) {
    cr_inst_t* in = cr_inst_get(s, inst);
    const cr_tpl_t* t = s->tpl;
    if (array >= t->ac) cr_fatal("array out of range");
    uint32_t pos = t->arr_pos[(size_t)in->unit * t->ac + array];
    if (pos == LLG_CR_NONE) cr_fatal("array not in the instance's unit");
    return &in->arrays[pos];
}

void llg_cr_set_array(llg_cr_session_t* s, uint32_t inst, uint32_t array, uint32_t size,
                      int active) {
    cr_iarr_t* arr = cr_inst_array(s, inst, array);
    const llg_cr_array_t* a = &s->tpl->p->arrays[array];
    size_t ew = CR_WORDS(a->elem_width);
    if (a->kind == LLG_CR_ARRAY_FIXED) {
        int64_t span = (int64_t)a->left - (int64_t)a->right;
        uint64_t count = (uint64_t)(span < 0 ? -span : span) + 1u;
        if (count != size) cr_fatal("fixed array size mismatch");
    }
    arr->words = (uint64_t*)cr_grow(arr->words, &arr->words_cap, (size_t)size * ew + 1u,
                                    sizeof(uint64_t));
    arr->input = (uint8_t*)cr_grow(arr->input, &arr->input_cap, (size_t)size + 1u, 1u);
    arr->size = arr->chosen = size;
    arr->active = (uint8_t)(active != 0);
}

void llg_cr_set_elem(llg_cr_session_t* s, uint32_t inst, uint32_t array, uint32_t index,
                     const uint64_t* words, int input_state) {
    cr_iarr_t* arr = cr_inst_array(s, inst, array);
    const llg_cr_array_t* a = &s->tpl->p->arrays[array];
    if (index >= arr->size) cr_fatal("array element out of range");
    size_t ew = CR_WORDS(a->elem_width);
    cr_store_words(arr->words + (size_t)index * ew, words, a->elem_width);
    arr->input[index] = (uint8_t)input_state;
}

/* ---- expanded problems ------------------------------------------------ */

static void cr_flat_free(cr_flat_t* f) {
    if (!f) return;
    if (f->session) cr_session_free(f->session);
    free(f->key.data);
    free(f->nodes); free(f->consts.data); free(f->args.data); free(f->vars);
    free(f->blocks); free(f->leaves); free(f->calls); free(f->dists); free(f->items);
    free(f->orders); free(f->tables); free(f->table_items.data);
    free(f->var_kind.data); free(f->var_inst.data); free(f->var_idx.data); free(f->var_elem.data);
    free(f->leaf_inst.data); free(f->leaf_idx.data);
    free(f->block_inst.data); free(f->block_idx.data); free(f->size_refs.data);
    free(f->var_base); free(f->var_map); free(f->global_map);
    free(f->leaf_base); free(f->leaf_map); free(f->block_base); free(f->block_map);
    free(f->arr_base); free(f->arr_size); free(f->arr_svar); free(f->arr_table);
    free(f->arr_elem); free(f->elem_map.data);
    free(f);
}

static int cr_random_size(const cr_tpl_t* t, const cr_inst_t* in, uint32_t pos) {
    uint32_t array = cr_unit_arrays(t->p, in->unit)[pos];
    return in->random && in->arrays[pos].active &&
           t->p->arrays[array].kind != LLG_CR_ARRAY_FIXED;
}

/* Structure of the expansion for `phase`: what makes two calls expand to
 * the same problem. */
static void cr_tpl_key(const cr_tpl_t* t, int phase, cr_u32vec_t* key) {
    key->len = 0;
    cr_u32_push(key, (uint32_t)phase);
    cr_u32_push(key, t->count);
    for (uint32_t i = 0; i < t->count; ++i) {
        const cr_inst_t* in = &t->inst[i];
        const llg_cr_unit_t* u = &t->p->units[in->unit];
        cr_u32_push(key, in->unit);
        cr_u32_push(key, in->random);
        for (uint32_t k = 0; k < u->slot_count; ++k) {
            cr_u32_push(key, in->slots[k].count);
            for (uint32_t j = 0; j < in->slots[k].count; ++j) cr_u32_push(key, in->slots[k].target[j]);
        }
        for (uint32_t k = 0; k < u->array_count; ++k) {
            const cr_iarr_t* arr = &in->arrays[k];
            cr_u32_push(key, arr->active);
            if (phase == CR_PH_SIZES)
                cr_u32_push(key, cr_random_size(t, in, k) ? LLG_CR_NONE : arr->size);
            else
                cr_u32_push(key, arr->chosen);
        }
    }
}

/* ---- expansion -------------------------------------------------------- */

typedef struct {
    cr_tpl_t* t;
    const llg_cr_problem_t* p;
    cr_flat_t* f;
    const llg_cr_tunables_t* tun;
    int phase;
    uint32_t inst;           /* instance whose constraints are expanded */
    uint32_t frame, frames;  /* loop iteration frame (0: outside loops) */
    int64_t* loop_val;
    uint8_t* loop_on;
    /* memo: (template node, instance, frame) -> result */
    uint32_t* mkey;          /* 3 words per entry */
    uint32_t* mval;
    size_t mcap, mcount;
    /* hash-consing of flat nodes */
    uint32_t* hc;
    size_t hccap, hccount;
    cr_u64vec_t nhash;
    /* scratch */
    uint32_t* stamp;
    uint32_t stamp_now;
    cr_u32vec_t work;        /* stack of node lists being evaluated */
    cr_u32vec_t dfs;
    cr_u32vec_t items;       /* stack of loop results */
    cr_u32vec_t ops;
    uint8_t* called;
    int failed;
    /* folding */
    llg_cr_session_t fs;
    llg_cr_problem_t fp;
    llg_cr_node_t fnodes[4];
    uint32_t foff[5];
    uint64_t* fval;
    size_t fval_cap;
    uint64_t* ftmp;
    size_t ftmp_cap;
} cr_ex_t;

static uint64_t cr_fb_hash(const cr_flat_t* f, const llg_cr_node_t* nd) {
    uint64_t h = cr_mix(((uint64_t)nd->op << 40) ^ ((uint64_t)nd->sign_flag << 32) ^ nd->bit_width);
    if (nd->op == LLG_CR_CONST) {
        for (uint32_t i = 0; i < CR_WORDS(nd->bit_width); ++i) h = cr_mix(h ^ f->consts.data[nd->a + i]);
    } else if (nd->op == LLG_CR_CONCAT || nd->op == LLG_CR_ALL) {
        h = cr_mix(h ^ nd->c);
        for (uint32_t i = 0; i < nd->c; ++i) h = cr_mix(h ^ f->args.data[nd->b + i]);
    } else {
        h = cr_mix(h ^ nd->a);
        h = cr_mix(h ^ ((uint64_t)nd->b << 1));
        h = cr_mix(h ^ ((uint64_t)nd->c << 2));
    }
    return h;
}

static int cr_fb_same(const cr_flat_t* f, const llg_cr_node_t* x, const llg_cr_node_t* y) {
    if (x->op != y->op || x->sign_flag != y->sign_flag || x->bit_width != y->bit_width) return 0;
    if (x->op == LLG_CR_CONST)
        return memcmp(f->consts.data + x->a, f->consts.data + y->a,
                      CR_WORDS(x->bit_width) * sizeof(uint64_t)) == 0;
    if (x->op == LLG_CR_CONCAT || x->op == LLG_CR_ALL)
        return x->c == y->c &&
               memcmp(f->args.data + x->b, f->args.data + y->b, x->c * sizeof(uint32_t)) == 0;
    return x->a == y->a && x->b == y->b && x->c == y->c;
}

static void cr_fb_rehash(cr_ex_t* ex) {
    size_t cap = ex->hccap ? ex->hccap * 2u : 1024u;
    uint32_t* table = (uint32_t*)cr_alloc(cap, sizeof(uint32_t));
    memset(table, 0xFF, cap * sizeof(uint32_t));
    for (size_t i = 0; i < ex->hccap; ++i) {
        uint32_t n = ex->hc[i];
        if (n == LLG_CR_NONE) continue;
        size_t h = (size_t)(ex->nhash.data[n] & (cap - 1u));
        while (table[h] != LLG_CR_NONE) h = (h + 1u) & (cap - 1u);
        table[h] = n;
    }
    free(ex->hc);
    ex->hc = table;
    ex->hccap = cap;
}

/* Add a node (operands, constant words or argument lists already pushed by
 * the caller), returning an existing equal node instead when there is one;
 * `pushed_consts`/`pushed_args` are then popped. CALL and DIST nodes are
 * never shared. */
static uint32_t cr_fb_intern(cr_ex_t* ex, const llg_cr_node_t* nd) {
    cr_flat_t* f = ex->f;
    int shared = nd->op != LLG_CR_CALL && nd->op != LLG_CR_DIST;
    uint64_t h = cr_fb_hash(f, nd);
    if (shared) {
        if ((ex->hccount + 1u) * 2u > ex->hccap) cr_fb_rehash(ex);
        size_t slot = (size_t)(h & (ex->hccap - 1u));
        while (ex->hc[slot] != LLG_CR_NONE) {
            uint32_t m = ex->hc[slot];
            if (ex->nhash.data[m] == h && cr_fb_same(f, &f->nodes[m], nd)) {
                if (nd->op == LLG_CR_CONST) f->consts.len -= CR_WORDS(nd->bit_width);
                if (nd->op == LLG_CR_CONCAT || nd->op == LLG_CR_ALL) f->args.len -= nd->c;
                return m;
            }
            slot = (slot + 1u) & (ex->hccap - 1u);
        }
        ex->hc[slot] = (uint32_t)f->nodes_len;
        ++ex->hccount;
    }
    if (f->nodes_len >= UINT32_MAX - 4u) cr_fatal("problem too large");
    *CR_VEC_PUSH(f->nodes, f->nodes_len, f->nodes_cap) = *nd;
    cr_u64_push(&ex->nhash, h);
    return (uint32_t)(f->nodes_len - 1u);
}

static uint32_t cr_fb_const(cr_ex_t* ex, const uint64_t* words, uint32_t width, int sign) {
    cr_flat_t* f = ex->f;
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = LLG_CR_CONST;
    nd.bit_width = width;
    nd.sign_flag = (uint8_t)sign;
    nd.a = (uint32_t)f->consts.len;
    for (uint32_t i = 0; i < CR_WORDS(width); ++i) cr_u64_push(&f->consts, words[i]);
    bv_mask(f->consts.data + nd.a, width);
    return cr_fb_intern(ex, &nd);
}

static uint32_t cr_fb_small(cr_ex_t* ex, int64_t value, uint32_t width, int sign) {
    uint64_t* words = (uint64_t*)cr_alloc(CR_WORDS(width), sizeof(uint64_t));
    for (uint32_t i = 0; i < CR_WORDS(width); ++i) words[i] = value < 0 ? UINT64_MAX : 0u;
    words[0] = (uint64_t)value;
    uint32_t n = cr_fb_const(ex, words, width, sign);
    free(words);
    return n;
}

static int cr_fb_is_const(const cr_ex_t* ex, uint32_t x) {
    return x < ex->f->nodes_len && ex->f->nodes[x].op == LLG_CR_CONST;
}

static int cr_fb_truth(const cr_ex_t* ex, uint32_t x) {
    const llg_cr_node_t* nd = &ex->f->nodes[x];
    return !bv_is_zero(ex->f->consts.data + nd->a, nd->bit_width);
}

/* Evaluate `nd` over constant operands with the reference evaluator.
 * Returns the constant node, or LLG_CR_NONE when the operation is an
 * evaluation error (left to classification). */
static uint32_t cr_fb_eval(cr_ex_t* ex, const llg_cr_node_t* nd, const uint32_t* ops,
                           uint32_t nops) {
    cr_flat_t* f = ex->f;
    uint32_t words = 0, maxw = CR_WORDS(nd->bit_width);
    for (uint32_t i = 0; i < nops; ++i) {
        ex->fnodes[i] = f->nodes[ops[i]];
        ex->foff[i] = words;
        uint32_t w = CR_WORDS(f->nodes[ops[i]].bit_width);
        if (w > maxw) maxw = w;
        words += w;
    }
    ex->fnodes[nops] = *nd;
    ex->foff[nops] = words;
    words += CR_WORDS(nd->bit_width);
    llg_cr_node_t* top = &ex->fnodes[nops];
    switch (nd->op) {
    case LLG_CR_REPL:
    case LLG_CR_SLICE:
    case LLG_CR_RESIZE:
    case LLG_CR_NOT:
    case LLG_CR_NEG:
    case LLG_CR_REDAND:
    case LLG_CR_REDOR:
    case LLG_CR_REDXOR:
    case LLG_CR_REDNAND:
    case LLG_CR_REDNOR:
    case LLG_CR_REDXNOR:
    case LLG_CR_LNOT:
        top->a = 0;
        break;
    case LLG_CR_COND:
        top->a = 0; top->b = 1; top->c = 2;
        break;
    default:
        top->a = 0; top->b = 1;
        break;
    }
    ex->fval = (uint64_t*)cr_grow(ex->fval, &ex->fval_cap, words + 1u, sizeof(uint64_t));
    ex->ftmp = (uint64_t*)cr_grow(ex->ftmp, &ex->ftmp_cap, 4u * ((size_t)maxw + 1u), sizeof(uint64_t));
    for (uint32_t i = 0; i < nops; ++i)
        memcpy(ex->fval + ex->foff[i], f->consts.data + f->nodes[ops[i]].a,
               CR_WORDS(f->nodes[ops[i]].bit_width) * sizeof(uint64_t));
    memset(&ex->fp, 0, sizeof(ex->fp));
    ex->fp.nodes = ex->fnodes;
    ex->fp.node_count = nops + 1u;
    memset(&ex->fs, 0, sizeof(ex->fs));
    ex->fs.p = &ex->fp;
    ex->fs.val = ex->fval;
    ex->fs.node_off = ex->foff;
    ex->fs.max_words = maxw;
    ex->fs.tmp_a = ex->ftmp;
    ex->fs.tmp_b = ex->ftmp + (maxw + 1u);
    ex->fs.tmp_c = ex->ftmp + 2u * (maxw + 1u);
    ex->fs.tmp_d = ex->ftmp + 3u * (maxw + 1u);
    if (cr_eval_op(&ex->fs, nops)) return LLG_CR_NONE;
    return cr_fb_const(ex, ex->fval + ex->foff[nops], nd->bit_width, nd->sign_flag);
}

/* A node with operands a, b, c (by the operation's shape), folded when its
 * operands are constants or a known operand decides Figure 18-3 logic. */
static uint32_t cr_fb_node(cr_ex_t* ex, uint32_t op, uint32_t width, int sign, uint32_t a,
                           uint32_t b, uint32_t c) {
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = (uint16_t)op;
    nd.bit_width = width;
    nd.sign_flag = (uint8_t)sign;
    nd.a = a;
    nd.b = b;
    nd.c = c;
    uint32_t ops[3];
    uint32_t nops;
    switch (op) {
    case LLG_CR_REPL:
    case LLG_CR_SLICE:
    case LLG_CR_RESIZE:
    case LLG_CR_NOT:
    case LLG_CR_NEG:
    case LLG_CR_REDAND:
    case LLG_CR_REDOR:
    case LLG_CR_REDXOR:
    case LLG_CR_REDNAND:
    case LLG_CR_REDNOR:
    case LLG_CR_REDXNOR:
    case LLG_CR_LNOT:
        ops[0] = a;
        nops = 1;
        break;
    case LLG_CR_COND:
        ops[0] = a; ops[1] = b; ops[2] = c;
        nops = 3;
        break;
    default:
        ops[0] = a; ops[1] = b;
        nops = 2;
        break;
    }
    int all_const = 1;
    for (uint32_t i = 0; i < nops; ++i) all_const &= cr_fb_is_const(ex, ops[i]);
    if (all_const) {
        uint32_t folded = cr_fb_eval(ex, &nd, ops, nops);
        if (folded != LLG_CR_NONE) return folded;
    } else if (width == 1u) {
        /* A known operand that decides the result (Figure 18-3). */
        int ka = cr_fb_is_const(ex, a), kb = nops > 1 && cr_fb_is_const(ex, b);
        int ta = ka && cr_fb_truth(ex, a), tb = kb && cr_fb_truth(ex, b);
        uint32_t wa = ex->f->nodes[a].bit_width;
        uint32_t wb = nops > 1 ? ex->f->nodes[b].bit_width : 0u;
        switch (op) {
        case LLG_CR_LAND:
            if ((ka && !ta) || (kb && !tb)) return cr_fb_small(ex, 0, 1u, 0);
            if (ka && wb == 1u) return b;
            if (kb && wa == 1u) return a;
            break;
        case LLG_CR_LOR:
            if ((ka && ta) || (kb && tb)) return cr_fb_small(ex, 1, 1u, 0);
            if (ka && wb == 1u) return b;
            if (kb && wa == 1u) return a;
            break;
        case LLG_CR_IMPL:
            if ((ka && !ta) || (kb && tb)) return cr_fb_small(ex, 1, 1u, 0);
            if (ka && wb == 1u) return b;
            break;
        default:
            break;
        }
    }
    if (op == LLG_CR_COND && cr_fb_is_const(ex, a)) {
        uint32_t pick = cr_fb_truth(ex, a) ? b : c;
        const llg_cr_node_t* np = &ex->f->nodes[pick];
        if (np->bit_width == width && np->sign_flag == sign) return pick;
    }
    return cr_fb_intern(ex, &nd);
}

/* Conjunction of `count` 1-bit results (known parts removed; a known false
 * one decides it, Figure 18-3). */
static uint32_t cr_fb_all(cr_ex_t* ex, const uint32_t* xs, size_t count) {
    cr_flat_t* f = ex->f;
    size_t start = f->args.len;
    for (size_t i = 0; i < count; ++i) {
        uint32_t x = xs[i];
        if (cr_fb_is_const(ex, x)) {
            if (!cr_fb_truth(ex, x)) {
                f->args.len = start;
                return cr_fb_small(ex, 0, 1u, 0);
            }
            continue;
        }
        cr_u32_push(&f->args, x);
    }
    size_t kept = f->args.len - start;
    if (kept == 0u) return cr_fb_small(ex, 1, 1u, 0);
    if (kept == 1u && f->nodes[f->args.data[start]].bit_width == 1u) {
        uint32_t only = f->args.data[start];
        f->args.len = start;
        return only;
    }
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = LLG_CR_ALL;
    nd.bit_width = 1u;
    nd.b = (uint32_t)start;
    nd.c = (uint32_t)kept;
    return cr_fb_intern(ex, &nd);
}

static uint32_t* cr_vec_slot(cr_u32vec_t* v, size_t index) {
    if (index >= v->len) {
        v->data = (uint32_t*)cr_grow(v->data, &v->cap, index + 1u, sizeof(uint32_t));
        for (size_t i = v->len; i <= index; ++i) v->data[i] = LLG_CR_NONE;
        v->len = index + 1u;
    }
    return &v->data[index];
}

/* A new flat variable with its origin. */
static uint32_t cr_fx_new_var(cr_ex_t* ex, const char* name, uint32_t width, int sign, int cyclic,
                              uint32_t kind, uint32_t inst, uint32_t idx, uint32_t elem) {
    cr_flat_t* f = ex->f;
    llg_cr_var_t* v = CR_VEC_PUSH(f->vars, f->vars_len, f->vars_cap);
    v->name = name;
    v->bit_width = width;
    v->sign_flag = (uint8_t)sign;
    v->domain_node = LLG_CR_NONE;
    v->cyclic = (uint8_t)cyclic;
    cr_u32_push(&f->var_kind, kind);
    cr_u32_push(&f->var_inst, inst);
    cr_u32_push(&f->var_idx, idx);
    cr_u32_push(&f->var_elem, elem);
    return (uint32_t)(f->vars_len - 1u);
}

static uint32_t cr_fx_var_node(cr_ex_t* ex, uint32_t fv) {
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = LLG_CR_VAR;
    nd.bit_width = ex->f->vars[fv].bit_width;
    nd.sign_flag = ex->f->vars[fv].sign_flag;
    nd.a = fv;
    return cr_fb_intern(ex, &nd);
}

/* Flat variable of template variable `var` of instance `inst` (or global). */
static uint32_t cr_fx_var(cr_ex_t* ex, uint32_t inst, uint32_t var) {
    cr_tpl_t* t = ex->t;
    cr_flat_t* f = ex->f;
    const llg_cr_var_t* tv = &ex->p->vars[var];
    uint32_t* slot;
    if (t->global[var]) {
        slot = &f->global_map[var];
        inst = LLG_CR_NONE;
    } else {
        uint32_t pos = cr_inst_var(t, &t->inst[inst], var);
        slot = &f->var_map[f->var_base[inst] + pos];
    }
    if (*slot == LLG_CR_NONE)
        *slot = cr_fx_new_var(ex, tv->name, tv->bit_width, tv->sign_flag, tv->cyclic,
                              inst == LLG_CR_NONE ? CR_FV_GLOBAL : CR_FV_UNIT, inst, var, 0u);
    return *slot;
}

static uint32_t cr_fx_leaf(cr_ex_t* ex, uint32_t inst, uint32_t leaf) {
    cr_tpl_t* t = ex->t;
    cr_flat_t* f = ex->f;
    const llg_cr_unit_t* u = &ex->p->units[t->inst[inst].unit];
    if (leaf < u->leaf_first || leaf >= u->leaf_first + u->leaf_count)
        cr_fatal("state leaf of another unit");
    uint32_t* slot = &f->leaf_map[f->leaf_base[inst] + (leaf - u->leaf_first)];
    if (*slot == LLG_CR_NONE) {
        llg_cr_leaf_t* l = CR_VEC_PUSH(f->leaves, f->leaves_len, f->leaves_cap);
        *l = ex->p->leaves[leaf];
        cr_u32_push(&f->leaf_inst, inst);
        cr_u32_push(&f->leaf_idx, leaf);
        *slot = (uint32_t)(f->leaves_len - 1u);
    }
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = LLG_CR_STATE;
    nd.bit_width = ex->p->leaves[leaf].bit_width;
    nd.sign_flag = ex->p->leaves[leaf].sign_flag;
    nd.a = *slot;
    return cr_fb_intern(ex, &nd);
}

/* A value read through a null handle: a leaf that is a null-dereference
 * evaluation error (18.5.12 E), so a guard can still eliminate it. */
static uint32_t cr_fx_null(cr_ex_t* ex, uint32_t width, int sign) {
    cr_flat_t* f = ex->f;
    llg_cr_leaf_t* l = CR_VEC_PUSH(f->leaves, f->leaves_len, f->leaves_cap);
    l->name = "null";
    l->bit_width = width;
    l->sign_flag = (uint8_t)sign;
    cr_u32_push(&f->leaf_inst, LLG_CR_NONE);
    cr_u32_push(&f->leaf_idx, LLG_CR_NONE);
    llg_cr_node_t nd;
    memset(&nd, 0, sizeof(nd));
    nd.op = LLG_CR_STATE;
    nd.bit_width = width;
    nd.sign_flag = (uint8_t)sign;
    nd.a = (uint32_t)(f->leaves_len - 1u);
    return cr_fb_intern(ex, &nd);
}

static uint32_t cr_fx_arr(cr_ex_t* ex, uint32_t inst, uint32_t array) {
    const cr_tpl_t* t = ex->t;
    if (inst == CR_X_NULL) return LLG_CR_NONE;
    uint32_t pos = t->arr_pos[(size_t)t->inst[inst].unit * t->ac + array];
    if (pos == LLG_CR_NONE) cr_fatal("array of another unit");
    return ex->f->arr_base[inst] + pos;
}

/* Flat variable of element `e` of array slot `k` (= cr_fx_arr). */
static uint32_t cr_fx_elem(cr_ex_t* ex, uint32_t inst, uint32_t array, uint32_t k, uint32_t e) {
    cr_flat_t* f = ex->f;
    if (f->arr_elem[k] == LLG_CR_NONE) {
        f->arr_elem[k] = (uint32_t)f->elem_map.len;
        if (f->arr_size[k]) cr_vec_slot(&f->elem_map, f->elem_map.len + f->arr_size[k] - 1u);
    }
    uint32_t* slot = &f->elem_map.data[f->arr_elem[k] + e];
    if (*slot == LLG_CR_NONE) {
        const llg_cr_array_t* a = &ex->p->arrays[array];
        *slot = cr_fx_new_var(ex, a->name, a->elem_width, a->elem_signed, 0, CR_FV_ELEM, inst,
                              array, e);
    }
    return *slot;
}

/* The table of every element of array slot `k`. */
static uint32_t cr_fx_table(cr_ex_t* ex, uint32_t inst, uint32_t array, uint32_t k) {
    cr_flat_t* f = ex->f;
    if (f->arr_table[k] != LLG_CR_NONE) return f->arr_table[k];
    size_t start = f->table_items.len;
    for (uint32_t e = 0; e < f->arr_size[k]; ++e) {
        uint32_t node = cr_fx_var_node(ex, cr_fx_elem(ex, inst, array, k, e));
        cr_u32_push(&f->table_items, node);
    }
    llg_cr_table_t* tb = CR_VEC_PUSH(f->tables, f->tables_len, f->tables_cap);
    tb->first = (uint32_t)start;
    tb->count = f->arr_size[k];
    f->arr_table[k] = (uint32_t)(f->tables_len - 1u);
    return f->arr_table[k];
}

static uint32_t cr_fx_size_var(cr_ex_t* ex, uint32_t inst, uint32_t array, uint32_t k) {
    cr_flat_t* f = ex->f;
    if (f->arr_svar[k] == LLG_CR_NONE) {
        uint32_t sv = cr_fx_new_var(ex, ex->p->arrays[array].name, 32u, 1, 0, CR_FV_SIZE, inst,
                                    array, 0u);
        f->arr_svar[k] = sv;
        uint32_t zero = cr_fb_small(ex, 0, 32u, 1);
        uint32_t ge = cr_fb_node(ex, LLG_CR_GE, 1u, 0, cr_fx_var_node(ex, sv), zero, 0u);
        f->vars[sv].domain_node = ge;
    }
    return f->arr_svar[k];
}

/* Element count and declared index base of a loop's array. */
static uint32_t cr_fx_count(cr_ex_t* ex, uint32_t inst, uint32_t array, int* random_size) {
    const cr_tpl_t* t = ex->t;
    *random_size = 0;
    if (array & LLG_CR_SLOT_ARRAY) {
        uint32_t slot = array & ~LLG_CR_SLOT_ARRAY;
        uint32_t pos = t->slot_pos[(size_t)t->inst[inst].unit * t->sc + slot];
        if (pos == LLG_CR_NONE) cr_fatal("slot of another unit");
        return t->inst[inst].slots[pos].count;
    }
    uint32_t k = cr_fx_arr(ex, inst, array);
    uint32_t pos = k - ex->f->arr_base[inst];
    if (ex->phase == CR_PH_SIZES && cr_random_size(t, &t->inst[inst], pos)) *random_size = 1;
    return ex->f->arr_size[k];
}

static int64_t cr_fx_declared(const cr_ex_t* ex, uint32_t array, uint32_t pos) {
    uint8_t kind;
    int32_t left, right;
    if (array & LLG_CR_SLOT_ARRAY) {
        const llg_cr_slot_t* s = &ex->p->slots[array & ~LLG_CR_SLOT_ARRAY];
        kind = s->kind; left = s->left; right = s->right;
    } else {
        const llg_cr_array_t* a = &ex->p->arrays[array];
        kind = a->kind; left = a->left; right = a->right;
    }
    if (kind != LLG_CR_ARRAY_FIXED) return pos;
    return left <= right ? (int64_t)left + pos : (int64_t)left - pos;
}

/* ---- memo ------------------------------------------------------------- */

static size_t cr_memo_hash(uint32_t n, uint32_t inst, uint32_t frame, size_t cap) {
    uint64_t h = cr_mix(((uint64_t)n << 32) ^ inst);
    h = cr_mix(h ^ ((uint64_t)frame << 7));
    return (size_t)(h & (uint64_t)(cap - 1u));
}

static uint32_t cr_memo_frame(const cr_ex_t* ex, uint32_t n) {
    return ex->t->deps[n] ? ex->frame : 0u;
}

static uint32_t cr_memo_get(const cr_ex_t* ex, uint32_t n) {
    if (!ex->mcap) return LLG_CR_NONE;
    uint32_t frame = cr_memo_frame(ex, n);
    size_t h = cr_memo_hash(n, ex->inst, frame, ex->mcap);
    while (ex->mkey[3u * h] != LLG_CR_NONE) {
        if (ex->mkey[3u * h] == n && ex->mkey[3u * h + 1u] == ex->inst &&
            ex->mkey[3u * h + 2u] == frame)
            return ex->mval[h];
        h = (h + 1u) & (ex->mcap - 1u);
    }
    return LLG_CR_NONE;
}

static void cr_memo_put(cr_ex_t* ex, uint32_t n, uint32_t value) {
    if ((ex->mcount + 1u) * 2u > ex->mcap) {
        size_t cap = ex->mcap ? ex->mcap * 2u : 1024u;
        uint32_t* keys = (uint32_t*)cr_alloc(3u * cap, sizeof(uint32_t));
        uint32_t* vals = (uint32_t*)cr_alloc(cap, sizeof(uint32_t));
        memset(keys, 0xFF, 3u * cap * sizeof(uint32_t));
        for (size_t i = 0; i < ex->mcap; ++i) {
            if (ex->mkey[3u * i] == LLG_CR_NONE) continue;
            size_t h = cr_memo_hash(ex->mkey[3u * i], ex->mkey[3u * i + 1u], ex->mkey[3u * i + 2u], cap);
            while (keys[3u * h] != LLG_CR_NONE) h = (h + 1u) & (cap - 1u);
            memcpy(keys + 3u * h, ex->mkey + 3u * i, 3u * sizeof(uint32_t));
            vals[h] = ex->mval[i];
        }
        free(ex->mkey);
        free(ex->mval);
        ex->mkey = keys;
        ex->mval = vals;
        ex->mcap = cap;
    }
    uint32_t frame = cr_memo_frame(ex, n);
    size_t h = cr_memo_hash(n, ex->inst, frame, ex->mcap);
    while (ex->mkey[3u * h] != LLG_CR_NONE) h = (h + 1u) & (ex->mcap - 1u);
    ex->mkey[3u * h] = n;
    ex->mkey[3u * h + 1u] = ex->inst;
    ex->mkey[3u * h + 2u] = frame;
    ex->mval[h] = value;
    ++ex->mcount;
}

static uint32_t cr_ex_expand(cr_ex_t* ex, uint32_t root);

/* Result of operand x (already expanded in this context). */
static uint32_t cr_ex_res(const cr_ex_t* ex, uint32_t x) {
    uint32_t r = cr_memo_get(ex, x);
    if (r == LLG_CR_NONE) cr_fatal("internal error: operand not expanded");
    return r;
}

/* Instance a ref operand names (the expanding instance for LLG_CR_NONE). */
static uint32_t cr_ex_ref(const cr_ex_t* ex, uint32_t x) {
    return x == LLG_CR_NONE ? ex->inst : cr_ex_res(ex, x);
}

static int cr_ex_budget(cr_ex_t* ex) {
    if (ex->tun->expand_nodes && ex->f->nodes_len > ex->tun->expand_nodes) ex->failed = 1;
    return ex->failed;
}

/* FOREACH and REDUCE: the body over every element. */
static uint32_t cr_ex_loop(cr_ex_t* ex, uint32_t n) {
    const llg_cr_node_t* nd = &ex->p->nodes[n];
    const uint32_t* x = ex->p->args + nd->b;
    uint32_t array = x[0], body = x[1], loop = x[3];
    uint32_t inst = cr_ex_ref(ex, x[2]);
    int reduce = nd->op == LLG_CR_REDUCE;
    uint32_t op = reduce ? x[4] : 0u;
    if (inst == CR_X_NULL) return cr_fx_null(ex, nd->bit_width, nd->sign_flag);
    int random_size;
    uint32_t count = cr_fx_count(ex, inst, array, &random_size);
    if (random_size) return CR_X_DROP;
    if (loop >= ex->t->loops || ex->loop_on[loop]) cr_fatal("invalid loop nesting");
    size_t base = ex->items.len;
    uint32_t saved_frame = ex->frame;
    uint32_t result = LLG_CR_NONE;
    ex->loop_on[loop] = 1;
    for (uint32_t pos = 0; pos < count; ++pos) {
        ex->loop_val[loop] = cr_fx_declared(ex, array, pos);
        ex->frame = ++ex->frames;
        uint32_t r = cr_ex_expand(ex, body);
        if (r == CR_X_DROP || cr_ex_budget(ex)) {
            result = CR_X_DROP;
            break;
        }
        if (!reduce) {
            if (cr_fb_is_const(ex, r)) {
                if (cr_fb_truth(ex, r)) continue;
                result = cr_fb_small(ex, 0, 1u, 0);
                break;
            }
        } else {
            const llg_cr_node_t* nr = &ex->f->nodes[r];
            if (op == LLG_CR_LAND || op == LLG_CR_LOR) {
                if (nr->bit_width != 1u) r = cr_fb_node(ex, LLG_CR_REDOR, 1u, 0, r, 0u, 0u);
            } else if (nr->bit_width != nd->bit_width || nr->sign_flag != nd->sign_flag) {
                r = cr_fb_node(ex, LLG_CR_RESIZE, nd->bit_width, nd->sign_flag, r, 0u, 0u);
            }
        }
        cr_u32_push(&ex->items, r);
    }
    ex->loop_on[loop] = 0;
    ex->frame = saved_frame;
    if (result == LLG_CR_NONE) {
        size_t count_items = ex->items.len - base;
        if (!reduce) {
            result = cr_fb_all(ex, ex->items.data + base, count_items);
        } else if (count_items == 0u) {
            /* Identity of the operation (S30-D6). */
            uint64_t* words = (uint64_t*)cr_alloc(CR_WORDS(nd->bit_width), sizeof(uint64_t));
            if (op == LLG_CR_MUL || op == LLG_CR_LAND) words[0] = 1u;
            if (op == LLG_CR_AND)
                for (uint32_t i = 0; i < CR_WORDS(nd->bit_width); ++i) words[i] = UINT64_MAX;
            result = cr_fb_const(ex, words, nd->bit_width, nd->sign_flag);
            free(words);
        } else {
            uint32_t acc = ex->items.data[base];
            uint32_t w = (op == LLG_CR_LAND || op == LLG_CR_LOR) ? 1u : nd->bit_width;
            int sg = (op == LLG_CR_LAND || op == LLG_CR_LOR) ? 0 : nd->sign_flag;
            for (size_t i = 1; i < count_items; ++i)
                acc = cr_fb_node(ex, op, w, sg, acc, ex->items.data[base + i], 0u);
            if (w != nd->bit_width) acc = cr_fb_node(ex, LLG_CR_RESIZE, nd->bit_width, nd->sign_flag, acc, 0u, 0u);
            result = acc;
        }
    }
    ex->items.len = base;
    return result;
}

/* Expand template node n whose operands are expanded. */
static uint32_t cr_ex_node(cr_ex_t* ex, uint32_t n) {
    const llg_cr_problem_t* p = ex->p;
    cr_tpl_t* t = ex->t;
    cr_flat_t* f = ex->f;
    const llg_cr_node_t* nd = &p->nodes[n];
    /* A dropped operand drops the node (size phase). */
    ex->ops.len = 0;
    cr_tpl_operands(p, n, &ex->ops);
    for (size_t i = 0; i < ex->ops.len; ++i)
        if (cr_ex_res(ex, ex->ops.data[i]) == CR_X_DROP) return CR_X_DROP;
    switch (nd->op) {
    case LLG_CR_CONST:
        return cr_fb_const(ex, p->consts + nd->a, nd->bit_width, nd->sign_flag);
    case LLG_CR_VAR:
        return cr_fx_var_node(ex, cr_fx_var(ex, ex->inst, nd->a));
    case LLG_CR_STATE:
        return cr_fx_leaf(ex, ex->inst, nd->a);
    case LLG_CR_REF: {
        uint32_t base = cr_ex_ref(ex, nd->b);
        if (base == CR_X_NULL) return CR_X_NULL;
        uint32_t pos = t->slot_pos[(size_t)t->inst[base].unit * t->sc + nd->a];
        if (pos == LLG_CR_NONE) cr_fatal("slot of another unit");
        const cr_islot_t* sl = &t->inst[base].slots[pos];
        uint32_t index = 0;
        if (nd->c != LLG_CR_NONE) {
            uint32_t ix = cr_ex_res(ex, nd->c);
            if (!cr_fb_is_const(ex, ix)) cr_fatal("handle array index is not a loop index");
            const llg_cr_node_t* ni = &f->nodes[ix];
            int64_t value;
            if (!bv_index(f->consts.data + ni->a, ni->bit_width, ni->sign_flag, &value) ||
                value < 0 || (uint64_t)value >= sl->count)
                return CR_X_NULL;
            index = (uint32_t)value;
        }
        if (index >= sl->count || sl->target[index] == LLG_CR_NONE) return CR_X_NULL;
        return sl->target[index];
    }
    case LLG_CR_ISNULL:
        return cr_fb_small(ex, cr_ex_ref(ex, nd->a) == CR_X_NULL, 1u, 0);
    case LLG_CR_HVAR: {
        uint32_t inst = cr_ex_ref(ex, nd->a);
        if (t->global[nd->b]) return cr_fx_var_node(ex, cr_fx_var(ex, LLG_CR_NONE, nd->b));
        if (inst == CR_X_NULL) return cr_fx_null(ex, nd->bit_width, nd->sign_flag);
        return cr_fx_var_node(ex, cr_fx_var(ex, inst, nd->b));
    }
    case LLG_CR_AELEM: {
        uint32_t inst = cr_ex_ref(ex, nd->c);
        if (inst == CR_X_NULL) return cr_fx_null(ex, nd->bit_width, nd->sign_flag);
        uint32_t k = cr_fx_arr(ex, inst, nd->a);
        if (ex->phase == CR_PH_SIZES && cr_random_size(t, &t->inst[inst], k - f->arr_base[inst]))
            return CR_X_DROP;
        uint32_t ix = cr_ex_res(ex, nd->b);
        if (cr_fb_is_const(ex, ix)) {
            const llg_cr_node_t* ni = &f->nodes[ix];
            int64_t value;
            if (bv_index(f->consts.data + ni->a, ni->bit_width, ni->sign_flag, &value) &&
                value >= 0 && (uint64_t)value < f->arr_size[k])
                return cr_fx_var_node(ex, cr_fx_elem(ex, inst, nd->a, k, (uint32_t)value));
        }
        /* A random, state or out-of-range index: a table (S30-D11). */
        llg_cr_node_t e;
        memset(&e, 0, sizeof(e));
        e.op = LLG_CR_ELEM;
        e.bit_width = p->arrays[nd->a].elem_width;
        e.sign_flag = p->arrays[nd->a].elem_signed;
        e.a = cr_fx_table(ex, inst, nd->a, k);
        e.b = ix;
        return cr_fb_intern(ex, &e);
    }
    case LLG_CR_ASIZE: {
        uint32_t inst = cr_ex_ref(ex, nd->c);
        if (inst == CR_X_NULL) return cr_fx_null(ex, 32u, 1);
        int random_size;
        uint32_t count = cr_fx_count(ex, inst, nd->a, &random_size);
        if (random_size) {
            uint32_t k = cr_fx_arr(ex, inst, nd->a);
            return cr_fx_var_node(ex, cr_fx_size_var(ex, inst, nd->a, k));
        }
        return cr_fb_small(ex, (int64_t)count, 32u, 1);
    }
    case LLG_CR_ITER:
        if (nd->a >= t->loops || !ex->loop_on[nd->a]) cr_fatal("loop index outside its loop");
        return cr_fb_small(ex, ex->loop_val[nd->a], 32u, 1);
    case LLG_CR_FOREACH:
    case LLG_CR_REDUCE:
        return cr_ex_loop(ex, n);
    case LLG_CR_CALL: {
        if (ex->inst != 0u || t->deps[n]) cr_fatal("constraint function call outside unit 0");
        if (ex->called[nd->a]) cr_fatal("constraint function call expanded twice");
        ex->called[nd->a] = 1;
        const llg_cr_call_t* tc = &p->calls[nd->a];
        llg_cr_call_t* fc = &f->calls[nd->a];
        fc->arg_first = (uint32_t)f->args.len;
        fc->arg_count = tc->arg_count;
        for (uint32_t i = 0; i < tc->arg_count; ++i)
            cr_u32_push(&f->args, cr_ex_res(ex, p->args[tc->arg_first + i]));
        llg_cr_node_t c = *nd;
        return cr_fb_intern(ex, &c);
    }
    case LLG_CR_DIST: {
        const llg_cr_dist_t* td = &p->dists[nd->a];
        llg_cr_dist_t* fd = CR_VEC_PUSH(f->dists, f->dists_len, f->dists_cap);
        uint32_t index = (uint32_t)(f->dists_len - 1u);
        fd->expr = cr_ex_res(ex, td->expr);
        fd->guard = td->guard == LLG_CR_NONE ? LLG_CR_NONE : cr_ex_res(ex, td->guard);
        fd->item_first = (uint32_t)f->items_len;
        fd->item_count = td->item_count;
        for (uint32_t i = 0; i < td->item_count; ++i) {
            const llg_cr_dist_item_t* ti = &p->dist_items[td->item_first + i];
            llg_cr_dist_item_t* fi = CR_VEC_PUSH(f->items, f->items_len, f->items_cap);
            *fi = *ti;
            fi->match = cr_ex_res(ex, ti->match);
            fi->lo = cr_ex_res(ex, ti->lo);
            fi->hi = cr_ex_res(ex, ti->hi);
            fi->weight = ti->weight == LLG_CR_NONE ? LLG_CR_NONE : cr_ex_res(ex, ti->weight);
        }
        llg_cr_node_t d = *nd;
        d.a = index;
        return cr_fb_intern(ex, &d);
    }
    case LLG_CR_CONCAT:
    case LLG_CR_ALL: {
        size_t start = f->args.len;
        if (nd->op == LLG_CR_ALL) {
            /* Collect, then fold through cr_fb_all. */
            cr_u32vec_t xs = {0, 0, 0};
            for (uint32_t i = 0; i < nd->c; ++i) cr_u32_push(&xs, cr_ex_res(ex, p->args[nd->b + i]));
            uint32_t r = cr_fb_all(ex, xs.data, xs.len);
            free(xs.data);
            return r;
        }
        for (uint32_t i = 0; i < nd->c; ++i) cr_u32_push(&f->args, cr_ex_res(ex, p->args[nd->b + i]));
        llg_cr_node_t c = *nd;
        c.b = (uint32_t)start;
        return cr_fb_intern(ex, &c);
    }
    case LLG_CR_REPL:
    case LLG_CR_SLICE:
        return cr_fb_node(ex, nd->op, nd->bit_width, nd->sign_flag, cr_ex_res(ex, nd->a), nd->b, 0u);
    case LLG_CR_RESIZE:
    case LLG_CR_NOT:
    case LLG_CR_NEG:
    case LLG_CR_REDAND:
    case LLG_CR_REDOR:
    case LLG_CR_REDXOR:
    case LLG_CR_REDNAND:
    case LLG_CR_REDNOR:
    case LLG_CR_REDXNOR:
    case LLG_CR_LNOT:
        return cr_fb_node(ex, nd->op, nd->bit_width, nd->sign_flag, cr_ex_res(ex, nd->a), 0u, 0u);
    case LLG_CR_COND:
        return cr_fb_node(ex, nd->op, nd->bit_width, nd->sign_flag, cr_ex_res(ex, nd->a),
                          cr_ex_res(ex, nd->b), cr_ex_res(ex, nd->c));
    default:
        return cr_fb_node(ex, nd->op, nd->bit_width, nd->sign_flag, cr_ex_res(ex, nd->a),
                          cr_ex_res(ex, nd->b), 0u);
    }
}

static int cr_u32_cmp(const void* x, const void* y) {
    uint32_t a = *(const uint32_t*)x, b = *(const uint32_t*)y;
    return a < b ? -1 : a > b;
}

/* Expand template node `root` in the current context: collect the nodes
 * of its cone not yet expanded here, then expand them in post-order (loop
 * bodies are expanded by their loop, once per element). */
static uint32_t cr_ex_expand(cr_ex_t* ex, uint32_t root) {
    uint32_t done = cr_memo_get(ex, root);
    if (done != LLG_CR_NONE) return done;
    if (++ex->stamp_now == 0u) {
        memset(ex->stamp, 0, (ex->p->node_count ? ex->p->node_count : 1u) * sizeof(uint32_t));
        ex->stamp_now = 1u;
    }
    size_t base = ex->work.len;
    ex->dfs.len = 0;
    cr_u32_push(&ex->dfs, root);
    while (ex->dfs.len) {
        uint32_t n = ex->dfs.data[--ex->dfs.len];
        if (ex->stamp[n] == ex->stamp_now) continue;
        ex->stamp[n] = ex->stamp_now;
        if (cr_memo_get(ex, n) != LLG_CR_NONE) continue;
        cr_u32_push(&ex->work, n);
        cr_tpl_operands(ex->p, n, &ex->dfs);
    }
    size_t count = ex->work.len - base;
    qsort(ex->work.data + base, count, sizeof(uint32_t), cr_u32_cmp);
    uint32_t result = LLG_CR_NONE;
    for (size_t i = 0; i < count; ++i) {
        uint32_t n = ex->work.data[base + i];
        result = cr_ex_node(ex, n);
        cr_memo_put(ex, n, result);
    }
    ex->work.len = base;
    return result;
}

/* Expand block `block` of instance `inst` into a new flat block. */
static void cr_ex_block(cr_ex_t* ex, uint32_t inst, uint32_t block) {
    const llg_cr_problem_t* p = ex->p;
    cr_flat_t* f = ex->f;
    const llg_cr_unit_t* u = &p->units[ex->t->inst[inst].unit];
    ex->inst = inst;
    ex->frame = 0;
    cr_u32vec_t stack = {0, 0, 0}, conj = {0, 0, 0};
    cr_u32_push(&stack, p->blocks[block].root);
    while (stack.len && !ex->failed) {
        uint32_t n = stack.data[--stack.len];
        const llg_cr_node_t* nd = &p->nodes[n];
        if (nd->op == LLG_CR_ALL) {
            for (uint32_t i = nd->c; i-- > 0;) cr_u32_push(&stack, p->args[nd->b + i]);
            continue;
        }
        if (nd->op == LLG_CR_LAND) {
            cr_u32_push(&stack, nd->b);
            cr_u32_push(&stack, nd->a);
            continue;
        }
        uint32_t r = cr_ex_expand(ex, n);
        cr_ex_budget(ex);
        if (r == CR_X_DROP) continue;
        cr_u32_push(&conj, r);
    }
    uint32_t root = cr_fb_all(ex, conj.data, conj.len);
    llg_cr_block_t* b = CR_VEC_PUSH(f->blocks, f->blocks_len, f->blocks_cap);
    b->name = p->blocks[block].name;
    b->root = root;
    uint32_t fb = (uint32_t)(f->blocks_len - 1u);
    cr_u32_push(&f->block_inst, inst);
    cr_u32_push(&f->block_idx, block);
    f->block_map[f->block_base[inst] + (block - u->block_first)] = fb;
    if (ex->phase == CR_PH_SIZES) {
        /* Size variables this block constrains (a size no enabled block
         * constrains keeps its value). */
        cr_u32vec_t walk = {0, 0, 0};
        uint8_t* seen = (uint8_t*)cr_alloc(f->nodes_len ? f->nodes_len : 1u, 1u);
        cr_u32_push(&walk, root);
        while (walk.len) {
            uint32_t n = walk.data[--walk.len];
            if (seen[n]) continue;
            seen[n] = 1;
            const llg_cr_node_t* nd = &f->nodes[n];
            if (nd->op == LLG_CR_VAR) {
                if (f->var_kind.data[nd->a] == CR_FV_SIZE) {
                    cr_u32_push(&f->size_refs, nd->a);
                    cr_u32_push(&f->size_refs, fb);
                }
                continue;
            }
            /* Flat problems are not complete yet: walk by shape. */
            switch (nd->op) {
            case LLG_CR_CONST:
            case LLG_CR_STATE:
                break;
            case LLG_CR_CALL:
                for (uint32_t i = 0; i < f->calls[nd->a].arg_count; ++i)
                    cr_u32_push(&walk, f->args.data[f->calls[nd->a].arg_first + i]);
                break;
            case LLG_CR_CONCAT:
            case LLG_CR_ALL:
                for (uint32_t i = 0; i < nd->c; ++i) cr_u32_push(&walk, f->args.data[nd->b + i]);
                break;
            case LLG_CR_DIST: {
                const llg_cr_dist_t* d = &f->dists[nd->a];
                cr_u32_push(&walk, d->expr);
                if (d->guard != LLG_CR_NONE) cr_u32_push(&walk, d->guard);
                for (uint32_t i = 0; i < d->item_count; ++i) {
                    const llg_cr_dist_item_t* item = &f->items[d->item_first + i];
                    cr_u32_push(&walk, item->match);
                    cr_u32_push(&walk, item->lo);
                    cr_u32_push(&walk, item->hi);
                    if (item->weight != LLG_CR_NONE) cr_u32_push(&walk, item->weight);
                }
                break;
            }
            case LLG_CR_ELEM: {
                const llg_cr_table_t* tb = &f->tables[nd->a];
                cr_u32_push(&walk, nd->b);
                for (uint32_t i = 0; i < tb->count; ++i)
                    if (f->table_items.data[tb->first + i] != LLG_CR_NONE)
                        cr_u32_push(&walk, f->table_items.data[tb->first + i]);
                break;
            }
            case LLG_CR_REPL:
            case LLG_CR_SLICE:
            case LLG_CR_RESIZE:
            case LLG_CR_NOT:
            case LLG_CR_NEG:
            case LLG_CR_REDAND:
            case LLG_CR_REDOR:
            case LLG_CR_REDXOR:
            case LLG_CR_REDNAND:
            case LLG_CR_REDNOR:
            case LLG_CR_REDXNOR:
            case LLG_CR_LNOT:
                cr_u32_push(&walk, nd->a);
                break;
            case LLG_CR_COND:
                cr_u32_push(&walk, nd->a);
                cr_u32_push(&walk, nd->b);
                cr_u32_push(&walk, nd->c);
                break;
            default:
                cr_u32_push(&walk, nd->a);
                cr_u32_push(&walk, nd->b);
                break;
            }
        }
        free(seen);
        free(walk.data);
    }
    free(stack.data);
    free(conj.data);
}

/* Build the expanded problem of the current instances for `phase`.
 * Returns NULL when a budget is exceeded (`*budget` names it). */
static cr_flat_t* cr_flat_build(cr_tpl_t* t, int phase, const char** budget) {
    const llg_cr_problem_t* p = t->p;
    cr_flat_t* f = (cr_flat_t*)cr_alloc(1u, sizeof(*f));
    f->phase = phase;
    f->ninst = t->count;
    size_t n1 = (size_t)t->count + 1u;
    f->var_base = (uint32_t*)cr_alloc(n1, sizeof(uint32_t));
    f->leaf_base = (uint32_t*)cr_alloc(n1, sizeof(uint32_t));
    f->block_base = (uint32_t*)cr_alloc(n1, sizeof(uint32_t));
    f->arr_base = (uint32_t*)cr_alloc(n1, sizeof(uint32_t));
    size_t vt = 0, lt = 0, bt = 0, at = 0;
    for (uint32_t i = 0; i < t->count; ++i) {
        const llg_cr_unit_t* u = &p->units[t->inst[i].unit];
        f->var_base[i] = (uint32_t)vt;
        f->leaf_base[i] = (uint32_t)lt;
        f->block_base[i] = (uint32_t)bt;
        f->arr_base[i] = (uint32_t)at;
        vt += u->var_count;
        lt += u->leaf_count;
        bt += u->block_count;
        at += u->array_count;
    }
    f->var_base[t->count] = (uint32_t)vt;
    f->arr_base[t->count] = (uint32_t)at;
    f->var_map = (uint32_t*)cr_alloc(vt ? vt : 1u, sizeof(uint32_t));
    f->leaf_map = (uint32_t*)cr_alloc(lt ? lt : 1u, sizeof(uint32_t));
    f->block_map = (uint32_t*)cr_alloc(bt ? bt : 1u, sizeof(uint32_t));
    f->global_map = (uint32_t*)cr_alloc(t->vc ? t->vc : 1u, sizeof(uint32_t));
    f->arr_size = (uint32_t*)cr_alloc(at ? at : 1u, sizeof(uint32_t));
    f->arr_svar = (uint32_t*)cr_alloc(at ? at : 1u, sizeof(uint32_t));
    f->arr_table = (uint32_t*)cr_alloc(at ? at : 1u, sizeof(uint32_t));
    f->arr_elem = (uint32_t*)cr_alloc(at ? at : 1u, sizeof(uint32_t));
    memset(f->var_map, 0xFF, (vt ? vt : 1u) * sizeof(uint32_t));
    memset(f->leaf_map, 0xFF, (lt ? lt : 1u) * sizeof(uint32_t));
    memset(f->block_map, 0xFF, (bt ? bt : 1u) * sizeof(uint32_t));
    memset(f->global_map, 0xFF, (t->vc ? t->vc : 1u) * sizeof(uint32_t));
    memset(f->arr_svar, 0xFF, (at ? at : 1u) * sizeof(uint32_t));
    memset(f->arr_table, 0xFF, (at ? at : 1u) * sizeof(uint32_t));
    memset(f->arr_elem, 0xFF, (at ? at : 1u) * sizeof(uint32_t));
    for (uint32_t i = 0; i < t->count; ++i) {
        const cr_inst_t* in = &t->inst[i];
        for (uint32_t k = 0; k < p->units[in->unit].array_count; ++k)
            f->arr_size[f->arr_base[i] + k] = phase == CR_PH_SIZES ? in->arrays[k].size
                                                                  : in->arrays[k].chosen;
    }
    f->calls = (llg_cr_call_t*)cr_alloc(p->call_count ? p->call_count : 1u, sizeof(llg_cr_call_t));
    for (uint32_t c = 0; c < p->call_count; ++c) {
        f->calls[c] = p->calls[c];
        f->calls[c].arg_first = 0;
        f->calls[c].arg_count = 0;
    }

    cr_ex_t ex;
    memset(&ex, 0, sizeof(ex));
    ex.t = t;
    ex.p = p;
    ex.f = f;
    ex.tun = llg_cr_tunables();
    ex.phase = phase;
    ex.loop_val = (int64_t*)cr_alloc(t->loops ? t->loops : 1u, sizeof(int64_t));
    ex.loop_on = (uint8_t*)cr_alloc(t->loops ? t->loops : 1u, 1u);
    ex.stamp = (uint32_t*)cr_alloc(p->node_count ? p->node_count : 1u, sizeof(uint32_t));
    ex.called = (uint8_t*)cr_alloc(p->call_count ? p->call_count : 1u, 1u);

    if (phase == CR_PH_ELEMS) {
        /* Every variable and random element takes part, constrained or not. */
        for (uint32_t v = 0; v < t->vc; ++v)
            if (t->global[v]) cr_fx_var(&ex, LLG_CR_NONE, v);
        for (uint32_t i = 0; i < t->count; ++i) {
            const cr_inst_t* in = &t->inst[i];
            if (!in->random) continue;
            const llg_cr_unit_t* u = &p->units[in->unit];
            for (uint32_t k = 0; k < u->var_count; ++k) cr_fx_var(&ex, i, cr_unit_vars(p, in->unit)[k]);
            for (uint32_t k = 0; k < u->array_count; ++k)
                if (in->arrays[k].active)
                    cr_fx_table(&ex, i, cr_unit_arrays(p, in->unit)[k], f->arr_base[i] + k);
        }
    }
    for (uint32_t i = 0; i < t->count && !ex.failed; ++i) {
        const cr_inst_t* in = &t->inst[i];
        if (!in->random) continue;
        const llg_cr_unit_t* u = &p->units[in->unit];
        for (uint32_t b = u->block_first; b < u->block_first + u->block_count && !ex.failed; ++b)
            cr_ex_block(&ex, i, b);
    }
    /* Template orders, per instance of the block's unit (18.5.9). */
    for (uint32_t o = 0; o < p->order_count && !ex.failed; ++o) {
        const llg_cr_order_t* ord = &p->orders[o];
        uint32_t unit = t->block_unit[ord->block];
        for (uint32_t i = 0; i < t->count; ++i) {
            const cr_inst_t* in = &t->inst[i];
            if (!in->random || in->unit != unit) continue;
            uint32_t fb = f->block_map[f->block_base[i] + (ord->block - p->units[unit].block_first)];
            uint32_t before = t->global[ord->before] ? f->global_map[ord->before]
                              : f->var_map[f->var_base[i] + cr_inst_var(t, in, ord->before)];
            uint32_t after = t->global[ord->after] ? f->global_map[ord->after]
                             : f->var_map[f->var_base[i] + cr_inst_var(t, in, ord->after)];
            if (fb == LLG_CR_NONE || before == LLG_CR_NONE || after == LLG_CR_NONE) continue;
            llg_cr_order_t* fo = CR_VEC_PUSH(f->orders, f->orders_len, f->orders_cap);
            fo->block = fb;
            fo->before = before;
            fo->after = after;
        }
    }
    /* Domains of the variables, expanded in their instance. */
    for (size_t v = 0; v < f->vars_len && !ex.failed; ++v) {
        uint32_t kind = f->var_kind.data[v];
        if (kind != CR_FV_UNIT && kind != CR_FV_GLOBAL) continue;
        uint32_t tv = f->var_idx.data[v];
        if (p->vars[tv].domain_node == LLG_CR_NONE) continue;
        ex.inst = kind == CR_FV_GLOBAL ? 0u : f->var_inst.data[v];
        ex.frame = 0;
        uint32_t d = cr_ex_expand(&ex, p->vars[tv].domain_node);
        f->vars[v].domain_node = d == CR_X_DROP ? LLG_CR_NONE : d;
    }
    /* Sizes first: every size before every other variable (S30-D5). */
    if (phase == CR_PH_SIZES && !ex.failed) {
        int any = 0;
        for (size_t v = 0; v < f->vars_len; ++v) any |= f->var_kind.data[v] == CR_FV_SIZE;
        if (any) {
            llg_cr_block_t* b = CR_VEC_PUSH(f->blocks, f->blocks_len, f->blocks_cap);
            b->name = "array sizes";
            b->root = cr_fb_small(&ex, 1, 1u, 0);
            uint32_t fb = (uint32_t)(f->blocks_len - 1u);
            cr_u32_push(&f->block_inst, LLG_CR_NONE);
            cr_u32_push(&f->block_idx, LLG_CR_NONE);
            for (size_t sv = 0; sv < f->vars_len; ++sv) {
                if (f->var_kind.data[sv] != CR_FV_SIZE) continue;
                for (size_t v = 0; v < f->vars_len; ++v) {
                    if (f->var_kind.data[v] == CR_FV_SIZE) continue;
                    llg_cr_order_t* fo = CR_VEC_PUSH(f->orders, f->orders_len, f->orders_cap);
                    fo->block = fb;
                    fo->before = (uint32_t)sv;
                    fo->after = (uint32_t)v;
                }
            }
        }
    }
    cr_ex_budget(&ex);
    int failed = ex.failed;
    free(ex.loop_val); free(ex.loop_on); free(ex.stamp); free(ex.called);
    free(ex.mkey); free(ex.mval); free(ex.hc); free(ex.nhash.data);
    free(ex.work.data); free(ex.dfs.data); free(ex.items.data); free(ex.ops.data);
    free(ex.fval); free(ex.ftmp);
    if (failed) {
        *budget = "LLG_RAND_EXPAND_NODES";
        cr_flat_free(f);
        return NULL;
    }
    llg_cr_problem_t* fp = &f->problem;
    fp->name = p->name;
    fp->nodes = f->nodes;
    fp->node_count = (uint32_t)f->nodes_len;
    fp->args = f->args.data;
    fp->arg_count = (uint32_t)f->args.len;
    fp->consts = f->consts.data;
    fp->const_count = (uint32_t)f->consts.len;
    fp->vars = f->vars;
    fp->var_count = (uint32_t)f->vars_len;
    fp->blocks = f->blocks;
    fp->block_count = (uint32_t)f->blocks_len;
    fp->leaves = f->leaves;
    fp->leaf_count = (uint32_t)f->leaves_len;
    fp->calls = f->calls;
    fp->call_count = p->call_count;
    fp->dists = f->dists;
    fp->dist_count = (uint32_t)f->dists_len;
    fp->dist_items = f->items;
    fp->dist_item_count = (uint32_t)f->items_len;
    fp->orders = f->orders;
    fp->order_count = (uint32_t)f->orders_len;
    fp->tables = f->tables;
    fp->table_count = (uint32_t)f->tables_len;
    fp->table_items = f->table_items.data;
    fp->table_item_count = (uint32_t)f->table_items.len;
    f->session = cr_session_new(fp);
    return f;
}

/* Copy the instances' inputs into the expanded session. */
static void cr_flat_feed(cr_tpl_t* t, cr_flat_t* f) {
    const llg_cr_problem_t* p = t->p;
    llg_cr_session_t* fs = f->session;
    llg_cr_begin(fs);
    for (size_t b = 0; b < f->blocks_len; ++b) {
        uint32_t inst = f->block_inst.data[b];
        int enabled = 1;
        if (inst != LLG_CR_NONE) {
            const cr_inst_t* in = &t->inst[inst];
            enabled = in->block_en[f->block_idx.data[b] - p->units[in->unit].block_first];
        }
        llg_cr_set_block(fs, (uint32_t)b, enabled);
    }
    uint8_t* size_on = NULL;
    if (f->size_refs.len) {
        size_on = (uint8_t*)cr_alloc(f->vars_len, 1u);
        for (size_t i = 0; i + 1u < f->size_refs.len; i += 2u)
            if (fs->block_enabled[f->size_refs.data[i + 1u]]) size_on[f->size_refs.data[i]] = 1;
    }
    for (size_t v = 0; v < f->vars_len; ++v) {
        uint32_t kind = f->var_kind.data[v], inst = f->var_inst.data[v];
        uint32_t idx = f->var_idx.data[v];
        if (kind == CR_FV_GLOBAL) {
            llg_cr_set_var(fs, (uint32_t)v, t->g_cur + t->g_off[idx], t->g_active[idx], t->g_input[idx]);
            if (p->vars[idx].cyclic) llg_cr_set_cyclic(fs, (uint32_t)v, t->g_cyc + t->g_cyc_off[idx]);
            continue;
        }
        const cr_inst_t* in = &t->inst[inst];
        if (kind == CR_FV_UNIT) {
            uint32_t pos = cr_inst_var(t, in, idx);
            uint32_t base = t->var_off_base[in->unit];
            llg_cr_set_var(fs, (uint32_t)v, in->var_cur + t->var_off[base + pos],
                           in->random && in->var_active[pos], in->var_input[pos]);
            if (p->vars[idx].cyclic)
                llg_cr_set_cyclic(fs, (uint32_t)v, in->cyc + t->cyc_off[base + pos]);
            continue;
        }
        uint32_t pos = t->arr_pos[(size_t)in->unit * t->ac + idx];
        const cr_iarr_t* arr = &in->arrays[pos];
        if (kind == CR_FV_SIZE) {
            uint64_t size = arr->size;
            llg_cr_set_var(fs, (uint32_t)v, &size, size_on && size_on[v], LLG_CR_INPUT_KNOWN);
            continue;
        }
        uint32_t e = f->var_elem.data[v];
        size_t ew = CR_WORDS(p->arrays[idx].elem_width);
        int active = in->random && arr->active;
        if (e < arr->size)
            llg_cr_set_var(fs, (uint32_t)v, arr->words + (size_t)e * ew, active, arr->input[e]);
        else
            llg_cr_set_var(fs, (uint32_t)v, NULL, active, LLG_CR_INPUT_KNOWN);
    }
    free(size_on);
    for (size_t l = 0; l < f->leaves_len; ++l) {
        uint32_t inst = f->leaf_inst.data[l];
        if (inst == LLG_CR_NONE) {
            llg_cr_set_leaf(fs, (uint32_t)l, NULL, LLG_CR_INPUT_NULL);
            continue;
        }
        const cr_inst_t* in = &t->inst[inst];
        uint32_t leaf = f->leaf_idx.data[l];
        llg_cr_set_leaf(fs, (uint32_t)l, in->leaf_val + t->leaf_off[leaf],
                        in->leaf_input[leaf - p->units[in->unit].leaf_first]);
    }
}

/* Find the expanded problem of the current structure, or build it. */
static llg_cr_status_t cr_tpl_enter(cr_tpl_t* t, int phase, llg_cr_report_t* report) {
    cr_u32vec_t key = {0, 0, 0};
    cr_tpl_key(t, phase, &key);
    cr_flat_t* found = NULL;
    uint32_t victim = 0;
    for (uint32_t i = 0; i < CR_TPL_FLATS; ++i) {
        cr_flat_t* f = t->cache[i];
        if (!f) {
            victim = i;
            continue;
        }
        if (f->key.len == key.len && memcmp(f->key.data, key.data, key.len * sizeof(uint32_t)) == 0) {
            found = f;
            break;
        }
        if (t->cache[victim] && f->used < t->cache[victim]->used) victim = i;
    }
    if (!found) {
        const char* budget = NULL;
        found = cr_flat_build(t, phase, &budget);
        if (!found) {
            free(key.data);
            report->budget = budget;
            return LLG_CR_RESOURCE;
        }
        found->key = key;
        key.data = NULL;
        cr_flat_free(t->cache[victim]);
        t->cache[victim] = found;
    }
    free(key.data);
    found->used = ++t->clock;
    t->flat = found;
    t->phase = phase;
    cr_flat_feed(t, found);
    return LLG_CR_OK;
}

/* Report items of the expanded problem in template terms. */
static void cr_tpl_translate(const cr_flat_t* f, llg_cr_report_t* r) {
    if (r->item == LLG_CR_NONE) return;
    switch (r->reason) {
    case LLG_CR_REASON_NULL:
    case LLG_CR_REASON_XZ_LEAF:
        r->item = r->item < f->leaves_len && f->leaf_inst.data[r->item] != LLG_CR_NONE
                      ? f->leaf_idx.data[r->item]
                      : LLG_CR_NONE;
        break;
    case LLG_CR_REASON_XZ_VAR:
    case LLG_CR_REASON_ORDER:
    case LLG_CR_REASON_SOLVE_BEFORE:
        r->item = r->item < f->vars_len && (f->var_kind.data[r->item] == CR_FV_UNIT ||
                                            f->var_kind.data[r->item] == CR_FV_GLOBAL)
                      ? f->var_idx.data[r->item]
                      : LLG_CR_NONE;
        break;
    case LLG_CR_REASON_XZ_CALL:
        break;
    default:
        r->item = LLG_CR_NONE;
        break;
    }
}

static int cr_has_random_size(const cr_tpl_t* t) {
    for (uint32_t i = 0; i < t->count; ++i) {
        const cr_inst_t* in = &t->inst[i];
        for (uint32_t k = 0; k < t->p->units[in->unit].array_count; ++k)
            if (cr_random_size(t, in, k)) return 1;
    }
    return 0;
}

/* Random elements of the solve (LLG_RAND_ARRAY_ELEMENTS). */
static int cr_elements_ok(const cr_tpl_t* t) {
    uint64_t limit = llg_cr_tunables()->array_elements, total = 0;
    if (!limit) return 1;
    for (uint32_t i = 0; i < t->count; ++i) {
        const cr_inst_t* in = &t->inst[i];
        if (!in->random) continue;
        for (uint32_t k = 0; k < t->p->units[in->unit].array_count; ++k)
            if (in->arrays[k].active) total += in->arrays[k].chosen;
    }
    return total <= limit;
}

static llg_cr_status_t cr_tpl_step(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                                   llg_cr_report_t* report) {
    cr_tpl_t* t = s->tpl;
    llg_cr_report_t local;
    if (!report) report = &local;
    memset(report, 0, sizeof(*report));
    report->item = LLG_CR_NONE;
    llg_cr_status_t status = LLG_CR_OK;
    if (t->phase == CR_PH_DONE) cr_fatal("llg_cr_step after the solve finished");
    if (t->phase == CR_PH_IDLE) {
        if (t->qhead < t->queue.len) cr_fatal("an instance was not fed");
        if (cr_has_random_size(t)) {
            status = cr_tpl_enter(t, CR_PH_SIZES, report);
        } else if (!cr_elements_ok(t)) {
            report->budget = "LLG_RAND_ARRAY_ELEMENTS";
            status = LLG_CR_RESOURCE;
        } else {
            status = cr_tpl_enter(t, CR_PH_ELEMS, report);
        }
    }
    while (status == LLG_CR_OK) {
        cr_flat_t* f = t->flat;
        status = llg_cr_step(f->session, rng, report);
        if (status == LLG_CR_NEED_CALLS) return status;
        if (status != LLG_CR_OK) {
            cr_tpl_translate(f, report);
            break;
        }
        if (t->phase == CR_PH_SIZES) {
            /* The sizes chosen (or kept) become the second expansion's. */
            for (size_t v = 0; v < f->vars_len; ++v) {
                if (f->var_kind.data[v] != CR_FV_SIZE || !f->session->var_active[v]) continue;
                cr_inst_t* in = &t->inst[f->var_inst.data[v]];
                uint32_t pos = t->arr_pos[(size_t)in->unit * t->ac + f->var_idx.data[v]];
                in->arrays[pos].chosen = (uint32_t)llg_cr_value(f->session, (uint32_t)v)[0];
            }
            if (!cr_elements_ok(t)) {
                report->budget = "LLG_RAND_ARRAY_ELEMENTS";
                status = LLG_CR_RESOURCE;
                break;
            }
            status = cr_tpl_enter(t, CR_PH_ELEMS, report);
            continue;
        }
        /* Solved: keep the cycles the solution advanced (S30-D3). */
        for (size_t v = 0; v < f->vars_len; ++v) {
            uint32_t kind = f->var_kind.data[v], idx = f->var_idx.data[v];
            if ((kind != CR_FV_UNIT && kind != CR_FV_GLOBAL) || !t->p->vars[idx].cyclic) continue;
            const uint64_t* cyc = llg_cr_cyclic(f->session, (uint32_t)v);
            llg_cr_iset_cyclic(s, kind == CR_FV_GLOBAL ? LLG_CR_NONE : f->var_inst.data[v], idx, cyc);
        }
        break;
    }
    t->phase = CR_PH_DONE;
    if (status == LLG_CR_RESOURCE && !report->budget) report->budget = "LLG_RAND_REJECT_TRIES";
    return status;
}

/* ---- results ---------------------------------------------------------- */

static const uint64_t* cr_tpl_value(const cr_tpl_t* t, uint32_t inst, uint32_t var) {
    const cr_flat_t* f = t->flat;
    if (var >= t->vc) cr_fatal("variable out of range");
    if (inst == LLG_CR_NONE || t->global[var]) {
        if (!t->global[var]) cr_fatal("not a global variable");
        uint32_t fv = f && f->phase == CR_PH_ELEMS ? f->global_map[var] : LLG_CR_NONE;
        if (fv != LLG_CR_NONE && f->session->var_active[fv]) return llg_cr_value(f->session, fv);
        return t->g_cur + t->g_off[var];
    }
    if (inst >= t->count) cr_fatal("instance out of range");
    const cr_inst_t* in = &t->inst[inst];
    uint32_t pos = cr_inst_var(t, in, var);
    if (f && f->phase == CR_PH_ELEMS && inst < f->ninst) {
        uint32_t fv = f->var_map[f->var_base[inst] + pos];
        if (fv != LLG_CR_NONE && f->session->var_active[fv]) return llg_cr_value(f->session, fv);
    }
    return in->var_cur + t->var_off[t->var_off_base[in->unit] + pos];
}

const uint64_t* llg_cr_ivalue(const llg_cr_session_t* s, uint32_t inst, uint32_t var) {
    if (!s->tpl) cr_fatal("not a problem with units");
    return cr_tpl_value(s->tpl, inst, var);
}

uint32_t llg_cr_array_size(const llg_cr_session_t* s, uint32_t inst, uint32_t array) {
    return cr_inst_array(s, inst, array)->chosen;
}

const uint64_t* llg_cr_elem_value(const llg_cr_session_t* s, uint32_t inst, uint32_t array,
                                  uint32_t index) {
    const cr_tpl_t* t = s->tpl;
    cr_iarr_t* arr = cr_inst_array(s, inst, array);
    const cr_flat_t* f = t->flat;
    if (index >= arr->chosen) cr_fatal("array element out of range");
    if (f && f->phase == CR_PH_ELEMS && inst < f->ninst) {
        uint32_t pos = t->arr_pos[(size_t)t->inst[inst].unit * t->ac + array];
        uint32_t k = f->arr_base[inst] + pos;
        if (f->arr_elem[k] != LLG_CR_NONE && index < f->arr_size[k]) {
            uint32_t fv = f->elem_map.data[f->arr_elem[k] + index];
            if (fv != LLG_CR_NONE && f->session->var_active[fv]) return llg_cr_value(f->session, fv);
        }
    }
    if (index < arr->size) return arr->words + (size_t)index * CR_WORDS(t->p->arrays[array].elem_width);
    cr_fatal("array element without a value");
    return NULL;
}

/* ---- session ---------------------------------------------------------- */

static void cr_tpl_free(cr_tpl_t* t) {
    if (!t) return;
    for (uint32_t i = 0; i < CR_TPL_FLATS; ++i) cr_flat_free(t->cache[i]);
    for (uint32_t i = 0; i < t->cap; ++i) cr_inst_release(&t->inst[i], t);
    free(t->inst);
    free(t->queue.data);
    free(t->hkey); free(t->hval);
    free(t->var_pos); free(t->arr_pos); free(t->slot_pos);
    free(t->var_off); free(t->var_off_base); free(t->cyc_off);
    free(t->leaf_off); free(t->leaf_words); free(t->leaf_unit); free(t->block_unit);
    free(t->global); free(t->g_off); free(t->g_cyc_off); free(t->g_cur);
    free(t->g_active); free(t->g_input); free(t->g_cyc); free(t->deps);
    free(t);
}

static llg_cr_session_t* cr_tpl_session_new(const llg_cr_problem_t* p) {
    llg_cr_session_t* s = (llg_cr_session_t*)cr_alloc(1u, sizeof(*s));
    s->p = p;
    s->tpl = cr_tpl_new(p);
    uint32_t max_args = 1u;
    s->max_words = 1u;
    for (uint32_t c = 0; c < p->call_count; ++c) {
        if (p->calls[c].arg_count > max_args) max_args = p->calls[c].arg_count;
        if (CR_WORDS(p->calls[c].bit_width) > s->max_words) s->max_words = CR_WORDS(p->calls[c].bit_width);
    }
    s->call_args = (const uint64_t**)cr_alloc(max_args, sizeof(const uint64_t*));
    s->tmp_a = (uint64_t*)cr_alloc((size_t)s->max_words + 1u, sizeof(uint64_t));
    return s;
}

/* The expanded session of the solve in progress, or NULL before one. */
static llg_cr_session_t* cr_tpl_flat_session(const llg_cr_session_t* s) {
    const cr_tpl_t* t = s->tpl;
    return t->flat && t->phase != CR_PH_IDLE ? t->flat->session : NULL;
}
