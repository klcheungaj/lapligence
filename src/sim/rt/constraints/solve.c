/* ---- sessions ------------------------------------------------------------ */

/* Problems with units (expand.c, after this file). */
static void cr_tpl_free(cr_tpl_t* t);
static llg_cr_session_t* cr_tpl_session_new(const llg_cr_problem_t* p);
static void cr_tpl_begin(llg_cr_session_t* s);
static llg_cr_status_t cr_tpl_step(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                                   llg_cr_report_t* report);
static const uint64_t* cr_tpl_value(const cr_tpl_t* t, uint32_t inst, uint32_t var);
static llg_cr_session_t* cr_tpl_flat_session(const llg_cr_session_t* s);
static uint64_t* cr_cyc_slot(const llg_cr_session_t* s, uint32_t inst, uint32_t var);

struct cr_cache_entry {
    cr_u64vec_t key;
    cr_exact_t exact;
    uint32_t* level_var;
    uint32_t* level_bit;
    uint64_t used; /* last use (0: free), for least-recently-used replacement */
    int failed;   /* the exact engine exceeded its budget for this key */
    int layered;  /* sample with `layout` (layers, weights, randc) */
    cr_layout_t layout;
};

/* Exact models kept per session. An expanded problem has a component per
 * array element or object, so the cache must hold every component one call
 * solves, or each call would rebuild (or re-overflow) the evicted ones. */
#define CR_CACHE_ENTRIES 32u

static void cr_validate_problem(const llg_cr_problem_t* p) {
    for (uint32_t n = 0; n < p->node_count; ++n) {
        const llg_cr_node_t* nd = &p->nodes[n];
        if (nd->op == 0 || nd->op >= LLG_CR_OP_LIMIT) cr_fatal("invalid node operation");
        if (nd->bit_width == 0u) cr_fatal("zero-width node");
        switch (nd->op) {
        case LLG_CR_CONST:
            if ((uint64_t)nd->a + CR_WORDS(nd->bit_width) > p->const_count) cr_fatal("constant out of range");
            break;
        case LLG_CR_VAR:
            if (nd->a >= p->var_count) cr_fatal("variable out of range");
            break;
        case LLG_CR_STATE:
            if (nd->a >= p->leaf_count) cr_fatal("state leaf out of range");
            break;
        case LLG_CR_CALL: {
            if (nd->a >= p->call_count) cr_fatal("call out of range");
            const llg_cr_call_t* c = &p->calls[nd->a];
            if ((uint64_t)c->arg_first + c->arg_count > p->arg_count) cr_fatal("call arguments out of range");
            for (uint32_t i = 0; i < c->arg_count; ++i)
                if (p->args[c->arg_first + i] >= n) cr_fatal("call argument after its call");
            break;
        }
        case LLG_CR_CONCAT:
        case LLG_CR_ALL:
            if ((uint64_t)nd->b + nd->c > p->arg_count) cr_fatal("operand list out of range");
            for (uint32_t i = 0; i < nd->c; ++i)
                if (p->args[nd->b + i] >= n) cr_fatal("operand after its node");
            break;
        case LLG_CR_DIST: {
            if (nd->a >= p->dist_count) cr_fatal("dist out of range");
            const llg_cr_dist_t* d = &p->dists[nd->a];
            if (d->expr >= n || (d->guard != LLG_CR_NONE && d->guard >= n))
                cr_fatal("dist operand after its node");
            if ((uint64_t)d->item_first + d->item_count > p->dist_item_count)
                cr_fatal("dist items out of range");
            for (uint32_t i = 0; i < d->item_count; ++i) {
                const llg_cr_dist_item_t* item = &p->dist_items[d->item_first + i];
                if (item->match >= n || item->lo >= n || item->hi >= n ||
                    (item->weight != LLG_CR_NONE && item->weight >= n))
                    cr_fatal("dist item operand after its node");
            }
            break;
        }
        case LLG_CR_ELEM: {
            if (nd->a >= p->table_count || nd->b >= n) cr_fatal("element operand out of range");
            const llg_cr_table_t* t = &p->tables[nd->a];
            if ((uint64_t)t->first + t->count > p->table_item_count) cr_fatal("table out of range");
            for (uint32_t i = 0; i < t->count; ++i)
                if (p->table_items[t->first + i] != LLG_CR_NONE && p->table_items[t->first + i] >= n)
                    cr_fatal("table entry after its node");
            break;
        }
        case LLG_CR_REF:
        case LLG_CR_HVAR:
        case LLG_CR_AELEM:
        case LLG_CR_ASIZE:
        case LLG_CR_ITER:
        case LLG_CR_FOREACH:
        case LLG_CR_REDUCE:
        case LLG_CR_ISNULL:
            cr_fatal("template operation in a problem without units");
            break;
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
            if (nd->a >= n) cr_fatal("operand after its node");
            break;
        case LLG_CR_COND:
            if (nd->a >= n || nd->b >= n || nd->c >= n) cr_fatal("operand after its node");
            break;
        default:
            if (nd->a >= n || nd->b >= n) cr_fatal("operand after its node");
            break;
        }
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

/* A session of a problem without units, not linked into the release list
 * (expanded problems belong to their template's session). */
static llg_cr_session_t* cr_session_new(const llg_cr_problem_t* p) {
    cr_validate_problem(p);
    llg_cr_session_t* s = (llg_cr_session_t*)cr_alloc(1u, sizeof(*s));
    s->p = p;
    s->site = s;
    uint32_t nodes = p->node_count ? p->node_count : 1u;
    s->node_off = (uint32_t*)cr_alloc((size_t)nodes + 1u, sizeof(uint32_t));
    uint64_t words = 0;
    s->max_words = 1u;
    for (uint32_t n = 0; n < p->node_count; ++n) {
        s->node_off[n] = (uint32_t)words;
        uint32_t w = CR_WORDS(p->nodes[n].bit_width);
        if (w > s->max_words) s->max_words = w;
        words += w;
        if (words > UINT32_MAX) cr_fatal("problem too large");
    }
    s->val = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->status = (uint8_t*)cr_alloc(nodes, 1u);
    s->ereason = (uint8_t*)cr_alloc(nodes, 1u);
    s->eitem = (uint32_t*)cr_alloc(nodes, sizeof(uint32_t));
    s->stamp = (uint32_t*)cr_alloc(nodes, sizeof(uint32_t));
    uint32_t vars = p->var_count ? p->var_count : 1u;
    s->var_off = (uint32_t*)cr_alloc((size_t)vars + 1u, sizeof(uint32_t));
    words = 0;
    for (uint32_t v = 0; v < p->var_count; ++v) {
        s->var_off[v] = (uint32_t)words;
        uint32_t w = CR_WORDS(p->vars[v].bit_width);
        if (w > s->max_words) s->max_words = w;
        words += w;
        if (words > UINT32_MAX) cr_fatal("problem too large");
    }
    s->total_var_words = (uint32_t)words;
    s->var_cur = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->sol = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->var_active = (uint8_t*)cr_alloc(vars, 1u);
    s->var_input = (uint8_t*)cr_alloc(vars, 1u);
    s->var_fixed = (uint8_t*)cr_alloc(vars, 1u);
    s->block_enabled = (uint8_t*)cr_alloc(p->block_count ? p->block_count : 1u, 1u);
    uint32_t leaves = p->leaf_count ? p->leaf_count : 1u;
    s->leaf_off = (uint32_t*)cr_alloc(leaves, sizeof(uint32_t));
    words = 0;
    for (uint32_t l = 0; l < p->leaf_count; ++l) {
        s->leaf_off[l] = (uint32_t)words;
        words += CR_WORDS(p->leaves[l].bit_width);
    }
    s->leaf_val = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->leaf_input = (uint8_t*)cr_alloc(leaves, 1u);
    uint32_t calls = p->call_count ? p->call_count : 1u;
    s->call_off = (uint32_t*)cr_alloc(calls, sizeof(uint32_t));
    words = 0;
    uint32_t max_args = 1u;
    for (uint32_t c = 0; c < p->call_count; ++c) {
        s->call_off[c] = (uint32_t)words;
        words += CR_WORDS(p->calls[c].bit_width);
        if (CR_WORDS(p->calls[c].bit_width) > s->max_words) s->max_words = CR_WORDS(p->calls[c].bit_width);
        if (p->calls[c].arg_count > max_args) max_args = p->calls[c].arg_count;
    }
    s->call_val = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->call_state = (uint8_t*)cr_alloc(calls, 1u);
    s->call_input = (uint8_t*)cr_alloc(calls, 1u);
    s->call_args = (const uint64_t**)cr_alloc(max_args, sizeof(const uint64_t*));
    s->tmp_a = (uint64_t*)cr_alloc((size_t)s->max_words + 1u, sizeof(uint64_t));
    s->tmp_b = (uint64_t*)cr_alloc((size_t)s->max_words + 1u, sizeof(uint64_t));
    s->tmp_c = (uint64_t*)cr_alloc((size_t)s->max_words + 1u, sizeof(uint64_t));
    s->tmp_d = (uint64_t*)cr_alloc((size_t)s->max_words + 1u, sizeof(uint64_t));
    s->cache = (cr_cache_entry_t*)cr_alloc(CR_CACHE_ENTRIES, sizeof(cr_cache_entry_t));
    s->cyc_off = (uint32_t*)cr_alloc(vars, sizeof(uint32_t));
    words = 0;
    for (uint32_t v = 0; v < p->var_count; ++v) {
        s->cyc_off[v] = (uint32_t)words;
        if (p->vars[v].cyclic) words += LLG_CR_CYCLIC_WORDS(p->vars[v].bit_width);
    }
    s->cyc = (uint64_t*)cr_alloc(words ? words : 1u, sizeof(uint64_t));
    s->node_hash = (uint64_t*)cr_alloc(nodes, sizeof(uint64_t));
    s->scratch.reach = (uint8_t*)cr_alloc(nodes, 1u);
    s->scratch.in_cone = (uint8_t*)cr_alloc(nodes, 1u);
    s->scratch.var_flags = (uint8_t*)cr_alloc(3u * (size_t)vars, 1u);
    s->scratch.var_words = (uint32_t*)cr_alloc(4u * ((size_t)vars + 1u), sizeof(uint32_t));
    return s;
}

static void cr_scratch_free(cr_scratch_t* x) {
    free(x->reach); free(x->in_cone); free(x->var_flags); free(x->var_words);
    free(x->conj.data); free(x->stack.data); free(x->support.data); free(x->calls.data);
    free(x->layer_conj.data); free(x->comp_vars.data); free(x->comp_conj.data);
    free(x->conj_rep.data); free(x->comp_stack.data); free(x->key.data);
    free(x->bits); free(x->limbs);
    memset(x, 0, sizeof(*x));
}

llg_cr_session_t* llg_cr_session_get(const llg_cr_problem_t* p, llg_cr_session_t** slot) {
    if (slot && *slot) return *slot;
    llg_cr_session_t* s = p->unit_count ? cr_tpl_session_new(p) : cr_session_new(p);
    s->next_session = cr_all_sessions;
    cr_all_sessions = s;
    if (slot) *slot = s;
    return s;
}

static void cr_session_free(llg_cr_session_t* s) {
    if (s->tpl) {
        cr_tpl_free(s->tpl);
        free((void*)s->call_args);
        free(s->tmp_a);
        free(s);
        return;
    }
    for (uint32_t i = 0; i < CR_CACHE_ENTRIES; ++i) {
        cr_cache_entry_t* e = &s->cache[i];
        free(e->key.data);
        cr_exact_free(&e->exact);
        free(e->level_var);
        free(e->level_bit);
        cr_layout_free(&e->layout);
    }
    free(s->cache);
    cr_scratch_free(&s->scratch);
    free(s->cyc_off);
    free(s->cyc);
    free(s->node_hash);
    free(s->node_off); free(s->val); free(s->status); free(s->ereason);
    free(s->eitem); free(s->stamp);
    free(s->var_off); free(s->var_cur); free(s->sol); free(s->var_active);
    free(s->var_input); free(s->var_fixed); free(s->block_enabled);
    free(s->leaf_off); free(s->leaf_val); free(s->leaf_input);
    free(s->call_off); free(s->call_val); free(s->call_state); free(s->call_input);
    free((void*)s->call_args);
    free(s->tmp_a); free(s->tmp_b); free(s->tmp_c); free(s->tmp_d);
    free(s);
}

void llg_cr_session_release_all(void) {
    while (cr_all_sessions) {
        llg_cr_session_t* next = cr_all_sessions->next_session;
        cr_session_free(cr_all_sessions);
        cr_all_sessions = next;
    }
    cr_bdd_pool_release();
}

void llg_cr_set_var(llg_cr_session_t* s, uint32_t var, const uint64_t* words,
                    int active, int input_state) {
    if (s->tpl) {
        llg_cr_iset_var(s, 0u, var, words, active, input_state);
        return;
    }
    if (var >= s->p->var_count) cr_fatal("variable out of range");
    uint32_t n = CR_WORDS(s->p->vars[var].bit_width);
    uint64_t* d = s->var_cur + s->var_off[var];
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
    bv_mask(d, s->p->vars[var].bit_width);
    s->var_active[var] = (uint8_t)(active != 0);
    s->var_input[var] = (uint8_t)input_state;
}

void llg_cr_set_block(llg_cr_session_t* s, uint32_t block, int enabled) {
    if (s->tpl) {
        llg_cr_iset_block(s, 0u, block, enabled);
        return;
    }
    if (block >= s->p->block_count) cr_fatal("block out of range");
    s->block_enabled[block] = (uint8_t)(enabled != 0);
}

void llg_cr_set_leaf(llg_cr_session_t* s, uint32_t leaf, const uint64_t* words,
                     int input_state) {
    if (s->tpl) {
        llg_cr_iset_leaf(s, 0u, leaf, words, input_state);
        return;
    }
    if (leaf >= s->p->leaf_count) cr_fatal("state leaf out of range");
    uint32_t n = CR_WORDS(s->p->leaves[leaf].bit_width);
    uint64_t* d = s->leaf_val + s->leaf_off[leaf];
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
    bv_mask(d, s->p->leaves[leaf].bit_width);
    s->leaf_input[leaf] = (uint8_t)input_state;
}

void llg_cr_set_cyclic(llg_cr_session_t* s, uint32_t var, const uint64_t* words) {
    if (s->tpl) {
        llg_cr_iset_cyclic(s, 0u, var, words);
        return;
    }
    if (var >= s->p->var_count || !s->p->vars[var].cyclic) cr_fatal("not a randc variable");
    uint32_t n = LLG_CR_CYCLIC_WORDS(s->p->vars[var].bit_width);
    uint64_t* d = s->cyc + s->cyc_off[var];
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
}

const uint64_t* llg_cr_cyclic(const llg_cr_session_t* s, uint32_t var) {
    if (s->tpl) return cr_cyc_slot(s, 0u, var);
    return s->cyc + s->cyc_off[var];
}

const uint64_t* llg_cr_value(const llg_cr_session_t* s, uint32_t var) {
    if (s->tpl) return cr_tpl_value(s->tpl, 0u, var);
    return s->sol + s->var_off[var];
}

uint32_t llg_cr_var_words(const llg_cr_session_t* s, uint32_t var) {
    return CR_WORDS(s->p->vars[var].bit_width);
}

/* ---- solve helpers -------------------------------------------------------- */

static void cr_push_dist_operands(const llg_cr_problem_t* p, const llg_cr_node_t* nd,
                                  cr_u32vec_t* out) {
    const llg_cr_dist_t* d = &p->dists[nd->a];
    cr_u32_push(out, d->expr);
    if (d->guard != LLG_CR_NONE) cr_u32_push(out, d->guard);
    for (uint32_t i = 0; i < d->item_count; ++i) {
        const llg_cr_dist_item_t* item = &p->dist_items[d->item_first + i];
        cr_u32_push(out, item->match);
        cr_u32_push(out, item->lo);
        cr_u32_push(out, item->hi);
        if (item->weight != LLG_CR_NONE) cr_u32_push(out, item->weight);
    }
}

/* An element's operands: the index and every entry, or, for a classified
 * node (`live`) with a known index, only the index and the selected entry,
 * so a known index does not join the other entries' variables into one
 * component. */
static void cr_push_elem_operands(const llg_cr_problem_t* p, llg_cr_session_t* live,
                                  const llg_cr_node_t* nd, cr_u32vec_t* out) {
    const llg_cr_table_t* t = &p->tables[nd->a];
    cr_u32_push(out, nd->b);
    if (live && live->status[nd->b] == CR_K) {
        const llg_cr_node_t* ni = &p->nodes[nd->b];
        int64_t index;
        if (bv_index(cr_node_val(live, nd->b), ni->bit_width, ni->sign_flag, &index) && index >= 0 &&
            (uint64_t)index < t->count && p->table_items[t->first + (uint32_t)index] != LLG_CR_NONE)
            cr_u32_push(out, p->table_items[t->first + (uint32_t)index]);
        return;
    }
    for (uint32_t i = 0; i < t->count; ++i)
        if (p->table_items[t->first + i] != LLG_CR_NONE) cr_u32_push(out, p->table_items[t->first + i]);
}

/* The rare dist and element cases stay out of line so this switch remains
 * small enough to inline into the traversals that run on every solve. */
static void cr_push_node(const llg_cr_problem_t* p, llg_cr_session_t* live, uint32_t n,
                         cr_u32vec_t* out) {
    const llg_cr_node_t* nd = &p->nodes[n];
    switch (nd->op) {
    case LLG_CR_CONST:
    case LLG_CR_VAR:
    case LLG_CR_STATE:
        return;
    case LLG_CR_CALL: {
        const llg_cr_call_t* c = &p->calls[nd->a];
        for (uint32_t i = 0; i < c->arg_count; ++i) cr_u32_push(out, p->args[c->arg_first + i]);
        return;
    }
    case LLG_CR_CONCAT:
    case LLG_CR_ALL:
        for (uint32_t i = 0; i < nd->c; ++i) cr_u32_push(out, p->args[nd->b + i]);
        return;
    case LLG_CR_DIST:
        cr_push_dist_operands(p, nd, out);
        return;
    case LLG_CR_ELEM:
        cr_push_elem_operands(p, live, nd, out);
        return;
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
        cr_u32_push(out, nd->a);
        return;
    case LLG_CR_COND:
        cr_u32_push(out, nd->a);
        cr_u32_push(out, nd->b);
        cr_u32_push(out, nd->c);
        return;
    default:
        cr_u32_push(out, nd->a);
        cr_u32_push(out, nd->b);
        return;
    }
}

/* Every structural operand of node n. */
static void cr_push_operands(const llg_cr_problem_t* p, uint32_t n, cr_u32vec_t* out) {
    cr_push_node(p, NULL, n, out);
}

/* Operands a classified node depends on (see cr_push_elem_operands). */
static void cr_push_live(llg_cr_session_t* s, uint32_t n, cr_u32vec_t* out) {
    cr_push_node(s->p, s, n, out);
}

static uint32_t cr_next_stamp(llg_cr_session_t* s) {
    if (++s->stamp_now == 0u) {
        memset(s->stamp, 0, (s->p->node_count ? s->p->node_count : 1u) * sizeof(uint32_t));
        s->stamp_now = 1u;
    }
    return s->stamp_now;
}

/* Collect the random variables below `root`, not entering pending (R) calls
 * when `through_calls` is 0. Pending calls met are appended to `calls`. */
static void cr_support(llg_cr_session_t* s, uint32_t root, int through_calls,
                       cr_u32vec_t* vars, cr_u32vec_t* calls, cr_u32vec_t* stack) {
    const llg_cr_problem_t* p = s->p;
    uint32_t mark = cr_next_stamp(s);
    stack->len = 0;
    cr_u32_push(stack, root);
    while (stack->len) {
        uint32_t n = stack->data[--stack->len];
        if (s->status[n] != CR_R || s->stamp[n] == mark) continue;
        s->stamp[n] = mark;
        const llg_cr_node_t* nd = &p->nodes[n];
        if (nd->op == LLG_CR_VAR) {
            cr_u32_push(vars, nd->a);
            continue;
        }
        if (nd->op == LLG_CR_CALL && !through_calls) {
            if (calls) cr_u32_push(calls, n);
            continue;
        }
        cr_push_live(s, n, stack);
    }
}

/* Name of the synthetic block that orders array sizes before the other
 * variables (S30-D5); compared by address to tell it from user orders. */
static const char cr_sizes_block_name[] = "array sizes";

static void cr_draw_bits(const llg_cr_rng_t* rng, uint64_t* d, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 0; i < n; ++i) d[i] = 0;
    for (uint32_t chunk = 0; chunk * 32u < w; ++chunk)
        d[chunk / 2u] |= (uint64_t)rng->next(rng->stream) << (32u * (chunk % 2u));
    bv_mask(d, w);
}

typedef struct {
    llg_cr_session_t* s;
    const llg_cr_rng_t* rng;
    llg_cr_report_t* report;
    const llg_cr_tunables_t* t;
    const char* budget;   /* first budget exhausted in this solve */
    uint64_t bdd_nodes;   /* statistics */
    uint64_t reject_tries;
    uint64_t conflicts;
    uint32_t components;
    /* solve-before height of each variable (longest order path to a sink
     * over the whole problem), or NULL without orders (S30-D2). */
    uint32_t* height;
} cr_solve_t;

/* Mark the R nodes below the conjuncts (ascending index order is post-order). */
static void cr_mark_cone(llg_cr_session_t* s, const uint32_t* conj, uint32_t count,
                         uint8_t* in_cone, cr_u32vec_t* stack) {
    stack->len = 0;
    for (uint32_t i = 0; i < count; ++i) cr_u32_push(stack, conj[i]);
    while (stack->len) {
        uint32_t n = stack->data[--stack->len];
        if (s->status[n] != CR_R || in_cone[n]) continue;
        in_cone[n] = 1;
        cr_push_live(s, n, stack);
    }
}

/* Exact-cache key: conjuncts, variables and every K operand of the cone. */
static void cr_cache_key(llg_cr_session_t* s, const uint32_t* conj, uint32_t ncon,
                         const uint32_t* vars, uint32_t nvars, const uint8_t* in_cone,
                         cr_u64vec_t* key, cr_u32vec_t* scratch) {
    key->len = 0;
    cr_u64_push(key, ncon);
    for (uint32_t i = 0; i < ncon; ++i) cr_u64_push(key, conj[i]);
    cr_u64_push(key, nvars);
    for (uint32_t i = 0; i < nvars; ++i) cr_u64_push(key, vars[i]);
    for (uint32_t n = 0; n < s->p->node_count; ++n) {
        if (!in_cone[n]) continue;
        scratch->len = 0;
        cr_push_live(s, n, scratch);
        for (size_t i = 0; i < scratch->len; ++i) {
            uint32_t x = scratch->data[i];
            if (s->status[x] != CR_K) continue;
            cr_u64_push(key, ((uint64_t)n << 32) | x);
            const uint64_t* v = cr_node_val(s, x);
            for (uint32_t w = 0; w < CR_WORDS(s->p->nodes[x].bit_width); ++w) cr_u64_push(key, v[w]);
        }
    }
}

/* Take the next never-used slot, or else the least recently used one,
 * releasing what it held. Slots fill in index order, so lookups scan only
 * the first cache_count entries. */
static cr_cache_entry_t* cr_cache_slot(llg_cr_session_t* s) {
    cr_cache_entry_t* e;
    if (s->cache_count < CR_CACHE_ENTRIES) {
        e = &s->cache[s->cache_count++];
    } else {
        e = &s->cache[0];
        for (uint32_t i = 1; i < CR_CACHE_ENTRIES; ++i)
            if (s->cache[i].used < e->used) e = &s->cache[i];
    }
    free(e->key.data);
    cr_exact_free(&e->exact);
    free(e->level_var);
    free(e->level_bit);
    cr_layout_free(&e->layout);
    memset(e, 0, sizeof(*e));
    e->used = ++s->cache_clock;
    return e;
}

/* A cache entry's own copy of `key` (the scratch key is reused). */
static void cr_cache_set_key(cr_cache_entry_t* e, const cr_u64vec_t* key) {
    e->key.data = (uint64_t*)cr_alloc(key->len ? key->len : 1u, sizeof(uint64_t));
    if (key->len) memcpy(e->key.data, key->data, key->len * sizeof(uint64_t));
    e->key.len = e->key.cap = key->len;
}

static void cr_remember_failure(llg_cr_session_t* s, const cr_u64vec_t* key) {
    cr_cache_entry_t* e = cr_cache_slot(s);
    cr_cache_set_key(e, key);
    e->failed = 1;
}

static void cr_assign_from_levels(llg_cr_session_t* s, const uint8_t* bits, uint32_t levels,
                                  const uint32_t* level_var, const uint32_t* level_bit) {
    for (uint32_t l = 0; l < levels; ++l)
        if (level_var[l] != LLG_CR_NONE)
            bv_set_bit(s->sol + s->var_off[level_var[l]], level_bit[l], bits[l]);
}

static void cr_sample_exact(cr_solve_t* st, const cr_exact_t* e, const uint32_t* level_var,
                            const uint32_t* level_bit) {
    cr_scratch_t* x = &st->s->scratch;
    x->bits = (uint8_t*)cr_grow(x->bits, &x->bits_cap, e->levels ? e->levels : 1u, 1u);
    x->limbs = (uint64_t*)cr_grow(x->limbs, &x->limbs_cap, 3u * (size_t)e->limbs, sizeof(uint64_t));
    cr_exact_sample(e, st->rng, x->bits, x->limbs);
    cr_assign_from_levels(st->s, x->bits, e->levels, level_var, level_bit);
}

/* Evaluate the cone with the candidate values in `sol`: the component's
 * variables are fixed while the cone is classified again, so errors follow
 * the same Figure 18-3 tables as verification (an error conjunct never
 * holds), then the cone's R statuses are restored. */
static int cr_eval_cone(llg_cr_session_t* s, const uint8_t* in_cone, const uint32_t* conj,
                        uint32_t ncon, const uint32_t* vars, uint32_t nvars) {
    const llg_cr_problem_t* p = s->p;
    for (uint32_t i = 0; i < nvars; ++i) s->var_fixed[vars[i]] = 1;
    for (uint32_t n = 0; n < p->node_count; ++n)
        if (in_cone[n]) cr_classify_node(s, n);
    int ok = 1;
    for (uint32_t i = 0; i < ncon && ok; ++i)
        ok = s->status[conj[i]] == CR_K && cr_truth(s, conj[i]);
    for (uint32_t i = 0; i < nvars; ++i) s->var_fixed[vars[i]] = 0;
    for (uint32_t n = 0; n < p->node_count; ++n)
        if (in_cone[n]) s->status[n] = CR_R;
    return ok;
}

/* ---- component layering (S30-D1 to S30-D3) ------------------------------- */

/* Layers, weights and cyclic variables of one component. */
typedef struct {
    uint32_t* layer;        /* per problem variable (component members only) */
    uint32_t layer_count;
    uint32_t* cyclic;       /* per layer: randc variable or LLG_CR_NONE; NULL when plain */
    cr_u32vec_t dists;      /* R dist nodes in the cone */
    uint32_t* dist_layer;   /* per entry of dists */
    uint32_t selectors;     /* selector levels: one per weighted item + one per dist */
    int layered;            /* not the plain uniform measure */
} cr_plan_t;

static void cr_plan_free(cr_plan_t* plan) {
    if (!plan->layer && !plan->cyclic && !plan->dists.data && !plan->dist_layer) return;
    free(plan->layer);
    free(plan->cyclic);
    free(plan->dists.data);
    free(plan->dist_layer);
    memset(plan, 0, sizeof(*plan));
}

/* Number of values of dist item `item` ([lo:hi] in its comparison type) as
 * a natural in `out` (limbs words); 0 when hi < lo. */
static void cr_item_size(llg_cr_session_t* s, const llg_cr_dist_item_t* item, uint64_t* out,
                         uint32_t limbs) {
    const llg_cr_node_t* nl = &s->p->nodes[item->lo];
    uint32_t w = nl->bit_width;
    uint32_t words = CR_WORDS(w + 1u);
    uint64_t* lo = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    uint64_t* hi = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    uint64_t* d = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    int sgn = item->lo_signed;
    bv_extend(lo, w + 1u, cr_node_val(s, item->lo), w, sgn);
    bv_extend(hi, w + 1u, cr_node_val(s, item->hi), s->p->nodes[item->hi].bit_width, sgn);
    for (uint32_t i = 0; i < limbs; ++i) out[i] = 0;
    int less = sgn ? bv_scmp(hi, lo, w + 1u) < 0 : bv_ucmp(hi, lo, w + 1u) < 0;
    if (!less) {
        bv_sub(d, hi, lo, w + 1u);
        for (uint32_t i = 0; i < words && i < limbs; ++i) out[i] = d[i];
        uint64_t one[1] = {1};
        bn_add(out, one, 1u);
    }
    free(lo);
    free(hi);
    free(d);
}

/* Number of distinct values in the nonzero-weight items of `dist` as a
 * natural in `out` (limbs words). Lowering gives every item of a dist one
 * comparison type, so the items are intervals of one ordered domain. */
static void cr_dist_support(llg_cr_session_t* s, const llg_cr_dist_t* dist, uint64_t* out,
                            uint32_t limbs) {
    const llg_cr_problem_t* p = s->p;
    for (uint32_t i = 0; i < limbs; ++i) out[i] = 0;
    if (!dist->item_count) return;
    uint32_t w = p->nodes[p->dist_items[dist->item_first].lo].bit_width;
    uint32_t words = CR_WORDS(w + 1u);
    uint32_t n = 0;
    uint64_t* lo = (uint64_t*)cr_alloc((size_t)dist->item_count * words, sizeof(uint64_t));
    uint64_t* hi = (uint64_t*)cr_alloc((size_t)dist->item_count * words, sizeof(uint64_t));
    for (uint32_t i = 0; i < dist->item_count; ++i) {
        const llg_cr_dist_item_t* item = &p->dist_items[dist->item_first + i];
        if (cr_dist_item_zero(s, item)) continue;
        uint64_t* l = lo + (size_t)n * words;
        uint64_t* h = hi + (size_t)n * words;
        bv_extend(l, w, cr_node_val(s, item->lo), p->nodes[item->lo].bit_width, item->lo_signed);
        bv_extend(h, w, cr_node_val(s, item->hi), p->nodes[item->hi].bit_width, item->lo_signed);
        /* Signed bounds order like unsigned ones with the sign bit flipped. */
        if (item->lo_signed) {
            bv_set_bit(l, w - 1u, !bv_bit(l, w - 1u));
            bv_set_bit(h, w - 1u, !bv_bit(h, w - 1u));
        }
        if (bv_ucmp(h, l, w) < 0) continue;
        /* Insertion by lower bound. */
        uint32_t k = n;
        while (k > 0 && bv_ucmp(lo + (size_t)(k - 1u) * words, l, w) > 0) --k;
        if (k != n) {
            uint64_t* tl = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
            uint64_t* th = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
            memcpy(tl, l, words * sizeof(uint64_t));
            memcpy(th, h, words * sizeof(uint64_t));
            memmove(lo + (size_t)(k + 1u) * words, lo + (size_t)k * words,
                    (size_t)(n - k) * words * sizeof(uint64_t));
            memmove(hi + (size_t)(k + 1u) * words, hi + (size_t)k * words,
                    (size_t)(n - k) * words * sizeof(uint64_t));
            memcpy(lo + (size_t)k * words, tl, words * sizeof(uint64_t));
            memcpy(hi + (size_t)k * words, th, words * sizeof(uint64_t));
            free(tl);
            free(th);
        }
        ++n;
    }
    /* Merge overlapping intervals and add their sizes (w + 1 bits). */
    uint64_t* cl = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    uint64_t* ch = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    uint64_t* d = (uint64_t*)cr_alloc(words, sizeof(uint64_t));
    uint64_t one[1] = {1};
    for (uint32_t i = 0; i <= n; ++i) {
        const uint64_t* l = lo + (size_t)i * words;
        const uint64_t* h = hi + (size_t)i * words;
        if (i > 0 && i < n && bv_ucmp(l, ch, w) <= 0) {
            if (bv_ucmp(h, ch, w) > 0) memcpy(ch, h, words * sizeof(uint64_t));
            continue;
        }
        if (i > 0) {
            for (uint32_t k = 0; k < words; ++k) d[k] = 0;
            bv_sub(d, ch, cl, w + 1u);
            for (uint32_t k = 0; k < words && k < limbs; ++k) {
                uint64_t add[1] = {d[k]};
                bn_add(out + k, add, 1u);
            }
            bn_add(out, one, 1u);
        }
        if (i < n) {
            memcpy(cl, l, words * sizeof(uint64_t));
            memcpy(ch, h, words * sizeof(uint64_t));
        }
    }
    free(cl);
    free(ch);
    free(d);
    free(lo);
    free(hi);
}

/* Assign layers: randc variables first (one layer each), then the
 * solve-before DAG as late as possible (18.5.9 L30075-30080). Returns
 * LLG_CR_ERROR for a circular order. */
static llg_cr_status_t cr_plan_build(cr_solve_t* st, cr_plan_t* plan, const uint32_t* vars,
                                     uint32_t nvars, const uint8_t* in_cone) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    memset(plan, 0, sizeof(*plan));
    /* The plain uniform measure needs no per-variable layers: one layer, no
     * cyclic variable and no selector. This keeps unweighted components free
     * of per-solve allocation and node scans. */
    int plain = !st->height && !p->dist_count;
    for (uint32_t i = 0; i < nvars && plain; ++i) plain = !p->vars[vars[i]].cyclic;
    if (plain) {
        plan->layer_count = 1u;
        return LLG_CR_OK;
    }
    plan->layer = (uint32_t*)cr_alloc(p->var_count ? p->var_count : 1u, sizeof(uint32_t));
    uint32_t cyclic = 0;
    for (uint32_t i = 0; i < nvars; ++i)
        if (p->vars[vars[i]].cyclic) plan->layer[vars[i]] = cyclic++;
    /* Ordered layers: the distinct global heights of the members, highest
     * first; heights come from every order of the problem, so an order
     * through another component still places its variable early. */
    uint32_t max_height = 0;
    uint32_t* height = (uint32_t*)cr_alloc(p->var_count ? p->var_count : 1u, sizeof(uint32_t));
    if (st->height) {
        uint32_t* distinct = (uint32_t*)cr_alloc(nvars ? nvars : 1u, sizeof(uint32_t));
        uint32_t count = 0;
        for (uint32_t i = 0; i < nvars; ++i) {
            if (p->vars[vars[i]].cyclic) continue;
            uint32_t h = st->height[vars[i]], k = 0;
            while (k < count && distinct[k] != h) ++k;
            if (k == count) distinct[count++] = h;
        }
        for (uint32_t i = 0; i < nvars; ++i) {
            if (p->vars[vars[i]].cyclic) continue;
            uint32_t rank = 0;
            for (uint32_t k = 0; k < count; ++k) rank += distinct[k] < st->height[vars[i]];
            height[vars[i]] = rank;
            if (rank > max_height) max_height = rank;
        }
        free(distinct);
    }
    for (uint32_t i = 0; i < nvars; ++i)
        if (!p->vars[vars[i]].cyclic) plan->layer[vars[i]] = cyclic + max_height - height[vars[i]];
    plan->layer_count = cyclic + max_height + 1u;
    plan->cyclic = (uint32_t*)cr_alloc(plan->layer_count, sizeof(uint32_t));
    for (uint32_t l = 0; l < plan->layer_count; ++l) plan->cyclic[l] = LLG_CR_NONE;
    for (uint32_t i = 0; i < nvars; ++i)
        if (p->vars[vars[i]].cyclic) plan->cyclic[plan->layer[vars[i]]] = vars[i];
    /* Weighted dists: each weighs the layer of its latest variable. */
    cr_u32vec_t support = {0, 0, 0}, stack = {0, 0, 0};
    for (uint32_t n = 0; n < p->node_count; ++n) {
        if (!in_cone[n] || p->nodes[n].op != LLG_CR_DIST) continue;
        const llg_cr_dist_t* d = &p->dists[p->nodes[n].a];
        uint32_t items = 0;
        for (uint32_t i = 0; i < d->item_count; ++i)
            if (!cr_dist_item_zero(s, &p->dist_items[d->item_first + i])) ++items;
        support.len = 0;
        cr_support(s, n, 0, &support, NULL, &stack);
        uint32_t layer = 0;
        for (size_t i = 0; i < support.len; ++i)
            if (plan->layer[support.data[i]] > layer) layer = plan->layer[support.data[i]];
        cr_u32_push(&plan->dists, n);
        plan->dist_layer = (uint32_t*)realloc(plan->dist_layer, plan->dists.len * sizeof(uint32_t));
        if (!plan->dist_layer) cr_fatal("out of memory");
        plan->dist_layer[plan->dists.len - 1u] = layer;
        plan->selectors += items + 1u;
    }
    free(support.data);
    free(stack.data);
    free(height);
    plan->layered = plan->layer_count > 1u || plan->dists.len > 0u;
    return LLG_CR_OK;
}

/* A component bit-blasted into one AIG, shared by the exact engine and the
 * CDCL fallback. Level l (BDD order: bit positions most significant first,
 * variables interleaved in index order, then the dist selectors) is AIG
 * input l + 1. A selector level has level_var LLG_CR_NONE. */
typedef struct {
    cr_blast_t blast;
    uint32_t* level_var;
    uint32_t* level_bit;
    uint32_t levels;
    uint32_t out;       /* AIG literal of the conjunction */
    int built;
    int overflow;       /* the AIG exceeded its node bound */
    cr_layout_t layout; /* filled for a layered plan */
} cr_comp_aig_t;

/* Literal of a guard (truth and no error); true without a guard. */
static uint32_t cr_guard_lit(llg_cr_session_t* s, cr_blast_t* b, uint32_t guard) {
    if (guard == LLG_CR_NONE) return CR_TRUE;
    if (s->status[guard] == CR_K) return cr_truth(s, guard) ? CR_TRUE : CR_FALSE;
    return cr_conjunct_lit(s, b, guard);
}

static void cr_comp_aig_build(cr_solve_t* st, cr_comp_aig_t* c, const uint32_t* vars,
                              uint32_t nvars, const uint32_t* conj, uint32_t ncon,
                              const uint8_t* in_cone, const cr_plan_t* plan) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    uint64_t bits = 0;
    uint32_t maxw = 0;
    for (uint32_t i = 0; i < nvars; ++i) {
        bits += p->vars[vars[i]].bit_width;
        if (p->vars[vars[i]].bit_width > maxw) maxw = p->vars[vars[i]].bit_width;
    }
    uint32_t selectors = plan && plan->layered ? plan->selectors : 0u;
    if (bits + selectors >= (uint64_t)(UINT32_MAX >> 2)) {
        c->built = c->overflow = 1;
        return;
    }
    uint32_t var_levels = (uint32_t)bits;
    uint32_t levels = var_levels + selectors;
    c->levels = levels;
    c->level_var = (uint32_t*)cr_alloc(levels ? levels : 1u, sizeof(uint32_t));
    c->level_bit = (uint32_t*)cr_alloc(levels ? levels : 1u, sizeof(uint32_t));
    uint32_t* var_base = (uint32_t*)cr_alloc(p->var_count, sizeof(uint32_t));
    uint32_t* bit_input = (uint32_t*)cr_alloc(var_levels ? var_levels : 1u, sizeof(uint32_t));
    uint32_t base = 0;
    for (uint32_t i = 0; i < nvars; ++i) {
        var_base[vars[i]] = base;
        base += p->vars[vars[i]].bit_width;
    }
    uint32_t l = 0;
    for (uint32_t pos = maxw; pos-- > 0;) {
        for (uint32_t i = 0; i < nvars; ++i) {
            uint32_t v = vars[i];
            if (pos >= p->vars[v].bit_width) continue;
            c->level_var[l] = v;
            c->level_bit[l] = pos;
            bit_input[var_base[v] + pos] = l + 1u;
            ++l;
        }
    }
    for (uint32_t k = 0; k < selectors; ++k) {
        c->level_var[var_levels + k] = LLG_CR_NONE;
        c->level_bit[var_levels + k] = k;
    }
    uint64_t aig_limit = st->t->bdd_nodes > (UINT64_MAX / 8u) ? UINT64_MAX : st->t->bdd_nodes * 8u;
    if (aig_limit < 4096u) aig_limit = 4096u;
    cr_aig_init(&c->blast.aig, levels, aig_limit + levels);
    c->blast.lit_off = (uint32_t*)cr_alloc(p->node_count ? p->node_count : 1u, sizeof(uint32_t));
    c->blast.err = (uint32_t*)cr_alloc(p->node_count ? p->node_count : 1u, sizeof(uint32_t));
    for (uint32_t n = 0; n < p->node_count; ++n) c->blast.lit_off[n] = UINT32_MAX;
    c->blast.lits.data = NULL;
    c->blast.lits.len = c->blast.lits.cap = 0;
    for (uint32_t n = 0; n < p->node_count && !c->blast.aig.overflow; ++n)
        if (in_cone[n]) cr_blast_node(s, &c->blast, n, var_base, bit_input);
    uint32_t out = CR_TRUE;
    for (uint32_t i = 0; i < ncon && !c->blast.aig.overflow; ++i)
        out = cr_and(&c->blast.aig, out, cr_conjunct_lit(s, &c->blast, conj[i]));
    if (selectors && !c->blast.aig.overflow) {
        /* Layout: layers per level, selector weights (S30-D1). */
        cr_layout_t* lay = &c->layout;
        lay->levels = levels;
        lay->layer_count = plan->layer_count;
        uint32_t weight_bits = 0;
        for (size_t d = 0; d < plan->dists.len; ++d) {
            const llg_cr_dist_t* dist = &p->dists[p->nodes[plan->dists.data[d]].a];
            for (uint32_t i = 0; i < dist->item_count; ++i) {
                const llg_cr_dist_item_t* item = &p->dist_items[dist->item_first + i];
                weight_bits += p->nodes[item->lo].bit_width + 2u;
                if (item->weight != LLG_CR_NONE) weight_bits += p->nodes[item->weight].bit_width;
            }
            /* The support size and the sum over the items. */
            if (dist->item_count)
                weight_bits += p->nodes[p->dist_items[dist->item_first].lo].bit_width + 1u;
            weight_bits += 34u;
        }
        uint32_t limbs = (var_levels + 2u * weight_bits) / 64u + 3u;
        lay->limbs = limbs;
        lay->layer = (uint8_t*)cr_alloc(levels ? levels : 1u, 1u);
        lay->selector = (uint8_t*)cr_alloc(levels ? levels : 1u, 1u);
        lay->weight = (uint64_t*)cr_alloc((size_t)(levels ? levels : 1u) * limbs, sizeof(uint64_t));
        lay->cyclic = (uint32_t*)cr_alloc(plan->layer_count, sizeof(uint32_t));
        memcpy(lay->cyclic, plan->cyclic, plan->layer_count * sizeof(uint32_t));
        for (uint32_t lv = 0; lv < var_levels; ++lv) lay->layer[lv] = (uint8_t)plan->layer[c->level_var[lv]];
        uint64_t* scale = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
        uint64_t* size = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
        uint64_t* t = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
        uint32_t level = var_levels;
        cr_aig_t* g = &c->blast.aig;
        for (size_t d = 0; d < plan->dists.len && !g->overflow; ++d) {
            uint32_t dnode = plan->dists.data[d];
            const llg_cr_dist_t* dist = &p->dists[p->nodes[dnode].a];
            /* Scale: the product of the :/ range sizes, so every per-value
             * weight w or w/n is an integer (S30-D1). */
            for (uint32_t i = 0; i < limbs; ++i) scale[i] = 0;
            scale[0] = 1;
            for (uint32_t i = 0; i < dist->item_count; ++i) {
                const llg_cr_dist_item_t* item = &p->dist_items[dist->item_first + i];
                if (!item->per_range || cr_dist_item_zero(s, item)) continue;
                cr_item_size(s, item, size, limbs);
                if (!bn_bits(size, limbs)) continue;
                bn_mul(t, scale, size, limbs);
                memcpy(scale, t, limbs * sizeof(uint64_t));
            }
            /* Density relative to uniform over the support S (S30-D1): a
             * value v where the guard holds weighs |S| * w(v) / W, where w(v)
             * sums the matching items' per-value weights (w for `:=`, w / n
             * for `:/`) and W = sum over v of w(v); where the guard fails it
             * weighs 1. Everything is multiplied by W * scale to stay
             * integral. */
            uint64_t* support = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
            uint64_t* mass = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
            cr_dist_support(s, dist, support, limbs);
            uint32_t sel_first = level + 1u;
            for (uint32_t i = 0; i < dist->item_count; ++i) {
                const llg_cr_dist_item_t* item = &p->dist_items[dist->item_first + i];
                if (cr_dist_item_zero(s, item)) continue;
                uint64_t* w = lay->weight + (size_t)(sel_first++) * limbs;
                for (uint32_t k = 0; k < limbs; ++k) w[k] = 0;
                if (item->weight == LLG_CR_NONE) {
                    w[0] = 1;
                } else {
                    const uint64_t* wv = cr_node_val(s, item->weight);
                    for (uint32_t k = 0; k < CR_WORDS(p->nodes[item->weight].bit_width) && k < limbs; ++k)
                        w[k] = wv[k];
                }
                /* The item's mass times scale: w * n * scale for `:=`,
                 * w * scale for `:/`. */
                for (uint32_t k = 0; k < limbs; ++k) t[k] = 0;
                bn_mul(t, w, scale, limbs);
                if (!item->per_range) {
                    cr_item_size(s, item, size, limbs);
                    uint64_t* prod = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
                    bn_mul(prod, t, size, limbs);
                    memcpy(t, prod, limbs * sizeof(uint64_t));
                    free(prod);
                }
                bn_add(mass, t, limbs);
                /* Per-value weight times scale: w * scale, divided by this
                 * item's size for `:/` (the product of the other sizes). */
                for (uint32_t k = 0; k < limbs; ++k) t[k] = 0;
                t[0] = 1;
                for (uint32_t j = 0; j < dist->item_count; ++j) {
                    const llg_cr_dist_item_t* other = &p->dist_items[dist->item_first + j];
                    if (!other->per_range || cr_dist_item_zero(s, other)) continue;
                    if (j == i) continue;
                    cr_item_size(s, other, size, limbs);
                    if (!bn_bits(size, limbs)) continue;
                    uint64_t* prod = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
                    bn_mul(prod, t, size, limbs);
                    memcpy(t, prod, limbs * sizeof(uint64_t));
                    free(prod);
                }
                if (item->per_range) {
                    cr_item_size(s, item, size, limbs);
                    if (!bn_bits(size, limbs)) for (uint32_t k = 0; k < limbs; ++k) t[k] = 0;
                }
                uint64_t* prod = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
                bn_mul(prod, w, t, limbs);
                bn_mul(w, prod, support, limbs);
                free(prod);
            }
            uint32_t guard = cr_guard_lit(s, &c->blast, dist->guard);
            uint32_t first = level;
            /* Selector 0: the guard does not hold; weight W * scale (1 when
             * every weight is zero: then no item can hold). */
            uint32_t s0 = (level + 1u) << 1;
            lay->selector[level] = 1;
            lay->layer[level] = (uint8_t)plan->dist_layer[d];
            if (!bn_bits(mass, limbs)) mass[0] = 1;
            memcpy(lay->weight + (size_t)level * limbs, mass, limbs * sizeof(uint64_t));
            out = cr_and(g, out, cr_or(g, s0 ^ 1u, guard ^ 1u));
            ++level;
            for (uint32_t i = 0; i < dist->item_count; ++i) {
                const llg_cr_dist_item_t* item = &p->dist_items[dist->item_first + i];
                if (cr_dist_item_zero(s, item)) continue;
                uint32_t sel = (level + 1u) << 1;
                lay->selector[level] = 1;
                lay->layer[level] = (uint8_t)plan->dist_layer[d];
                out = cr_and(g, out, cr_or(g, sel ^ 1u,
                                          cr_and(g, guard, cr_conjunct_lit(s, &c->blast, item->match))));
                ++level;
            }
            free(support);
            free(mass);
            /* Exactly one selector of this dist. */
            uint32_t any = CR_FALSE;
            for (uint32_t a = first; a < level; ++a) {
                uint32_t la = (a + 1u) << 1;
                any = cr_or(g, any, la);
                for (uint32_t b2 = a + 1u; b2 < level; ++b2)
                    out = cr_and(g, out, cr_and(g, la, (b2 + 1u) << 1) ^ 1u);
            }
            out = cr_and(g, out, any);
        }
        free(scale);
        free(size);
        free(t);
    }
    c->out = out;
    c->built = 1;
    c->overflow = c->blast.aig.overflow;
    free(var_base);
    free(bit_input);
}

static void cr_comp_aig_free(cr_comp_aig_t* c) {
    if (c->blast.aig.left || c->blast.aig.table) cr_aig_free(&c->blast.aig);
    free(c->blast.lit_off);
    free(c->blast.err);
    free(c->blast.lits.data);
    free(c->level_var);
    free(c->level_bit);
    cr_layout_free(&c->layout);
    memset(c, 0, sizeof(*c));
}

/* Structural hash of node n (operators, widths, constants, variable, state
 * and call names; never state values), memoised per session. */
static uint64_t cr_node_hash(llg_cr_session_t* s, uint32_t n) {
    if (s->node_hash[n]) return s->node_hash[n];
    const llg_cr_problem_t* p = s->p;
    const llg_cr_node_t* nd = &p->nodes[n];
    uint64_t h = cr_mix(((uint64_t)nd->op << 40) ^ ((uint64_t)nd->sign_flag << 32) ^ nd->bit_width);
    switch (nd->op) {
    case LLG_CR_CONST:
        for (uint32_t i = 0; i < CR_WORDS(nd->bit_width); ++i) h = cr_mix(h ^ p->consts[nd->a + i]);
        break;
    case LLG_CR_VAR: h = cr_mix(h ^ cr_hash_text(p->vars[nd->a].name)); break;
    case LLG_CR_STATE: h = cr_mix(h ^ cr_hash_text(p->leaves[nd->a].name)); break;
    case LLG_CR_CALL: h = cr_mix(h ^ cr_hash_text(p->calls[nd->a].name)); break;
    case LLG_CR_SLICE:
    case LLG_CR_REPL: h = cr_mix(h ^ nd->b); break;
    case LLG_CR_ELEM: h = cr_mix(h ^ p->tables[nd->a].count); break;
    default: break;
    }
    cr_u32vec_t ops = {0, 0, 0};
    cr_push_operands(p, n, &ops);
    for (size_t i = 0; i < ops.len; ++i) h = cr_mix(h + cr_node_hash(s, ops.data[i]) * (i + 1u));
    free(ops.data);
    if (!h) h = 1u;
    s->node_hash[n] = h;
    return h;
}

/* randc signature of a component: its conjuncts' structure (S30-D3). */
static uint64_t cr_component_signature(llg_cr_session_t* s, const uint32_t* conj, uint32_t ncon) {
    uint64_t sum = 0;
    for (uint32_t i = 0; i < ncon; ++i) sum += cr_mix(cr_node_hash(s, conj[i]));
    return cr_mix(sum ^ ncon);
}

#define CR_CYCLIC_TRIES 64u

/* Draw one sample from a layered exact model (S30-D1 to S30-D3). Returns
 * LLG_CR_OK or LLG_CR_UNSAT (no value of a layer has a completion). */
static llg_cr_status_t cr_layered_sample(cr_solve_t* st, const cr_exact_t* e,
                                         const cr_layout_t* lay, const uint32_t* level_var,
                                         const uint32_t* level_bit, uint64_t signature) {
    llg_cr_session_t* s = st->s;
    uint32_t levels = e->levels;
    int8_t* fixed = (int8_t*)cr_alloc(levels ? levels : 1u, 1u);
    uint8_t* bits = (uint8_t*)cr_alloc(levels ? levels : 1u, 1u);
    for (uint32_t l = 0; l < levels; ++l) fixed[l] = -1;
    cr_wcount_t c;
    memset(&c, 0, sizeof(c));
    c.e = e;
    c.l = lay;
    c.fixed = fixed;
    c.cnt = (uint64_t*)cr_alloc((size_t)e->nodes * lay->limbs, sizeof(uint64_t));
    c.count_prefix = (uint32_t*)cr_alloc((size_t)levels + 1u, sizeof(uint32_t));
    c.t1 = (uint64_t*)cr_alloc(lay->limbs, sizeof(uint64_t));
    c.t2 = (uint64_t*)cr_alloc(lay->limbs, sizeof(uint64_t));
    llg_cr_status_t status = LLG_CR_OK;
    for (uint32_t layer = 0; layer < lay->layer_count && status == LLG_CR_OK; ++layer) {
        uint32_t root = layer + 1u < lay->layer_count ? e->roots[layer + 1u] : e->roots[0];
        c.layer = layer;
        cr_wcount_prepare(&c);
        uint32_t var = lay->cyclic[layer];
        if (var == LLG_CR_NONE) {
            if (!cr_wcount_draw(&c, root, st->rng, bits)) status = LLG_CR_UNSAT;
        } else {
            cr_cyc_check(s, var, cr_mix(signature ^ cr_hash_text(s->p->vars[var].name)));
            uint64_t value = 0;
            int found = 0;
            for (uint32_t t = 0; t < CR_CYCLIC_TRIES && !found; ++t) {
                if (!cr_wcount_draw(&c, root, st->rng, bits)) {
                    status = LLG_CR_UNSAT;
                    break;
                }
                value = 0;
                for (uint32_t l = 0; l < levels; ++l)
                    if (lay->layer[l] == layer && fixed[l] < 0 && bits[l])
                        value |= UINT64_C(1) << level_bit[l];
                found = !cr_cyc_used(s, var, value);
            }
            if (status == LLG_CR_OK && !found) {
                cr_u64vec_t values = {0, 0, 0};
                cr_enum_values(&c, root, 0u, 0u, level_bit, &values);
                uint64_t unused = 0;
                for (size_t i = 0; i < values.len; ++i) unused += !cr_cyc_used(s, var, values.data[i]);
                if (!unused) {
                    /* Nothing left in this cycle can satisfy the constraints:
                     * a new cycle starts (18.4.2 L29445-29447). */
                    cr_cyc_reset(s, var);
                    unused = values.len;
                }
                if (!unused) {
                    status = LLG_CR_UNSAT;
                } else {
                    uint64_t bound[2] = {unused, 0}, r[2];
                    bn_uniform(st->rng, r, bound, 2u);
                    for (size_t i = 0; i < values.len; ++i) {
                        if (cr_cyc_used(s, var, values.data[i])) continue;
                        if (r[0]-- == 0u) {
                            value = values.data[i];
                            break;
                        }
                    }
                }
                free(values.data);
            }
            if (status == LLG_CR_OK) {
                cr_cyc_mark(s, var, value);
                for (uint32_t l = 0; l < levels; ++l)
                    if (lay->layer[l] == layer && fixed[l] < 0)
                        bits[l] = (uint8_t)((value >> level_bit[l]) & 1u);
            }
        }
        for (uint32_t l = 0; l < levels; ++l)
            if (lay->layer[l] == layer && fixed[l] < 0) fixed[l] = (int8_t)bits[l];
    }
    if (status == LLG_CR_OK)
        for (uint32_t l = 0; l < levels; ++l)
            if (level_var[l] != LLG_CR_NONE)
                bv_set_bit(s->sol + s->var_off[level_var[l]], level_bit[l], fixed[l] > 0);
    free(fixed);
    free(bits);
    free(c.cnt);
    free(c.count_prefix);
    free(c.t1);
    free(c.t2);
    return status;
}

/* Build the projections Q_i = exists(levels of later layers). F for every
 * layer but the last; roots[0] = F, roots[1 + i] = Q_i. */
static int cr_layer_roots(cr_bdd_t* m, uint32_t root, const cr_layout_t* lay, uint32_t* roots) {
    roots[0] = root;
    uint8_t* quant = (uint8_t*)cr_alloc(lay->levels ? lay->levels : 1u, 1u);
    for (uint32_t layer = 0; layer + 1u < lay->layer_count; ++layer) {
        for (uint32_t l = 0; l < lay->levels; ++l) quant[l] = lay->layer[l] > layer;
        roots[layer + 1u] = cr_bdd_exists(m, root, quant, layer);
        if (m->overflow) break;
    }
    free(quant);
    return !m->overflow;
}

static int cr_order_live(const llg_cr_session_t* s, const llg_cr_order_t* o);

/* The fallback engines keep the constraints but not the distribution
 * measure (S30-D9). Say so once per call site when a component with a
 * `dist` weight or a `solve...before` order reaches them, naming the budget
 * that sent it there. */
static void cr_warn_fallback_measure(const cr_solve_t* st, const cr_plan_t* plan,
                                     const uint32_t* vars, uint32_t nvars) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    int measured = plan->dists.len > 0;
    for (uint32_t o = 0; o < p->order_count && !measured; ++o) {
        const llg_cr_order_t* ord = &p->orders[o];
        if (!cr_order_live(s, ord) || p->blocks[ord->block].name == cr_sizes_block_name) continue;
        for (uint32_t i = 0; i < nvars; ++i)
            measured |= vars[i] == ord->before || vars[i] == ord->after;
    }
    if (!measured || s->site->fallback_warned) return;
    s->site->fallback_warned = 1;
    const char* budget = st->budget ? st->budget : "LLG_RAND_BDD_NODES";
    fprintf(stderr,
            "llg: warning: %s: a constraint component exceeds the exact solver budget "
            "(%s), so the fallback solvers ignore its dist weights and solve...before "
            "orders (IEEE 1800-2009 18.5.4, 18.5.9); the constraints still hold. Raise %s "
            "to keep them.\n",
            p->name ? p->name : "randomize()", budget, budget);
}

static llg_cr_status_t cr_solve_component(cr_solve_t* st, const uint32_t* vars, uint32_t nvars,
                                          const uint32_t* conj, uint32_t ncon) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    st->components++;
    uint64_t bits = 0;
    for (uint32_t i = 0; i < nvars; ++i) bits += p->vars[vars[i]].bit_width;
    uint8_t* in_cone = s->scratch.in_cone;
    cr_u32vec_t* stack = &s->scratch.comp_stack;
    cr_mark_cone(s, conj, ncon, in_cone, stack);
    cr_plan_t plan;
    llg_cr_status_t result = cr_plan_build(st, &plan, vars, nvars, in_cone);
    if (result != LLG_CR_OK) {
        cr_plan_free(&plan);
        memset(in_cone, 0, p->node_count);
        return result;
    }
    int cyclic = 0;
    for (uint32_t l = 0; plan.cyclic && l < plan.layer_count; ++l) cyclic |= plan.cyclic[l] != LLG_CR_NONE;
    uint64_t signature = cyclic ? cr_component_signature(s, conj, ncon) : 0u;
    result = LLG_CR_RESOURCE;
    int done = 0;
    cr_comp_aig_t aig;
    memset(&aig, 0, sizeof(aig));

    if (bits + plan.selectors <= st->t->bdd_bits) {
        cr_u64vec_t* key = &s->scratch.key;
        cr_cache_key(s, conj, ncon, vars, nvars, in_cone, key, stack);
        if (plan.layered)
            for (uint32_t i = 0; i < nvars; ++i) cr_u64_push(key, ((uint64_t)1 << 63) | plan.layer[vars[i]]);
        int known_failure = 0;
        for (uint32_t i = 0; i < s->cache_count && !done && !known_failure; ++i) {
            cr_cache_entry_t* e = &s->cache[i];
            if (!e->used || e->key.len != key->len ||
                memcmp(e->key.data, key->data, key->len * sizeof(uint64_t)) != 0)
                continue;
            e->used = ++s->cache_clock;
            if (e->failed) {
                /* Same inputs as an earlier overflow: skip straight to the
                 * fallbacks, as a rebuild would overflow identically. */
                known_failure = 1;
                st->budget = st->budget ? st->budget : "LLG_RAND_BDD_NODES";
                continue;
            }
            if (e->layered)
                result = cr_layered_sample(st, &e->exact, &e->layout, e->level_var, e->level_bit,
                                           signature);
            else {
                cr_sample_exact(st, &e->exact, e->level_var, e->level_bit);
                result = LLG_CR_OK;
            }
            st->report->engines |= LLG_CR_ENGINE_BDD | LLG_CR_ENGINE_CACHE;
            done = 1;
        }
        if (!done && !known_failure) {
            cr_comp_aig_build(st, &aig, vars, nvars, conj, ncon, in_cone, &plan);
            if (aig.overflow) {
                cr_remember_failure(s, key);
                if (!st->budget) st->budget = "LLG_RAND_BDD_NODES";
            } else if (aig.out == CR_FALSE) {
                result = LLG_CR_UNSAT;
                done = 1;
            } else {
                cr_bdd_t m;
                cr_bdd_init(&m, aig.levels, st->t->bdd_nodes);
                uint32_t root = cr_bdd_from_aig(&m, &aig.blast.aig, aig.out);
                uint32_t* roots = (uint32_t*)cr_alloc(plan.layer_count + 1u, sizeof(uint32_t));
                uint32_t root_count = 1u;
                if (!m.overflow && root != 0u && plan.layered) {
                    if (!aig.layout.levels) {
                        /* Layers without dists: no selector levels. */
                        aig.layout.levels = aig.levels;
                        aig.layout.layer_count = plan.layer_count;
                        aig.layout.limbs = aig.levels / 64u + 3u;
                        aig.layout.layer = (uint8_t*)cr_alloc(aig.levels ? aig.levels : 1u, 1u);
                        aig.layout.selector = (uint8_t*)cr_alloc(aig.levels ? aig.levels : 1u, 1u);
                        aig.layout.weight = (uint64_t*)cr_alloc(
                            (size_t)(aig.levels ? aig.levels : 1u) * aig.layout.limbs, sizeof(uint64_t));
                        aig.layout.cyclic = (uint32_t*)cr_alloc(plan.layer_count, sizeof(uint32_t));
                        memcpy(aig.layout.cyclic, plan.cyclic, plan.layer_count * sizeof(uint32_t));
                        for (uint32_t l = 0; l < aig.levels; ++l)
                            aig.layout.layer[l] = (uint8_t)plan.layer[aig.level_var[l]];
                    }
                    cr_layer_roots(&m, root, &aig.layout, roots);
                    root_count = plan.layer_count;
                } else {
                    roots[0] = root;
                }
                st->bdd_nodes += m.count;
                if (!m.overflow) {
                    if (root == 0u) {
                        result = LLG_CR_UNSAT;
                    } else {
                        cr_cache_entry_t* e = cr_cache_slot(s);
                        cr_cache_set_key(e, key);
                        cr_exact_build(&e->exact, &m, roots, root_count);
                        /* The entry keeps copies of the level maps. */
                        e->level_var = (uint32_t*)cr_alloc(aig.levels ? aig.levels : 1u, sizeof(uint32_t));
                        e->level_bit = (uint32_t*)cr_alloc(aig.levels ? aig.levels : 1u, sizeof(uint32_t));
                        memcpy(e->level_var, aig.level_var, aig.levels * sizeof(uint32_t));
                        memcpy(e->level_bit, aig.level_bit, aig.levels * sizeof(uint32_t));
                        if (plan.layered) {
                            e->layered = 1;
                            cr_layout_copy(&e->layout, &aig.layout);
                            result = cr_layered_sample(st, &e->exact, &e->layout, e->level_var,
                                                       e->level_bit, signature);
                        } else {
                            cr_sample_exact(st, &e->exact, e->level_var, e->level_bit);
                            result = LLG_CR_OK;
                        }
                    }
                    st->report->engines |= LLG_CR_ENGINE_BDD;
                    done = 1;
                } else {
                    cr_remember_failure(s, key);
                    if (!st->budget) st->budget = "LLG_RAND_BDD_NODES";
                }
                free(roots);
                cr_bdd_free(&m);
            }
        }
    } else if (!st->budget) {
        st->budget = "LLG_RAND_BDD_BITS";
    }

    if (!done && cyclic) {
        /* A randc cycle needs the exact model (S30-D9). */
        st->budget = st->budget ? st->budget : "LLG_RAND_BDD_NODES";
        done = 1;
        result = LLG_CR_RESOURCE;
    }

    if (!done) cr_warn_fallback_measure(st, &plan, vars, nvars);

    if (!done) {
        /* Uniform rejection: every accepted draw is uniform over the
         * solutions because draws cover each variable's whole domain.
         * dist weights and solve-before orders are not applied (S30-D9). */
        for (uint64_t attempt = 0; attempt < st->t->reject_tries; ++attempt) {
            for (uint32_t i = 0; i < nvars; ++i)
                cr_draw_bits(st->rng, s->sol + s->var_off[vars[i]], p->vars[vars[i]].bit_width);
            st->reject_tries++;
            if (cr_eval_cone(s, in_cone, conj, ncon, vars, nvars)) {
                st->report->engines |= LLG_CR_ENGINE_REJECT;
                result = LLG_CR_OK;
                done = 1;
                break;
            }
        }
        if (!done) st->budget = st->budget ? st->budget : "LLG_RAND_REJECT_TRIES";
    }

    if (!done) {
        /* Search fallback (not uniform, S29-D10): complete, so its budget is
         * the one to raise. It shares the AIG and its node bound. */
        if (!aig.built) cr_comp_aig_build(st, &aig, vars, nvars, conj, ncon, in_cone, &plan);
        if (aig.overflow) {
            st->budget = "LLG_RAND_BDD_NODES";
        } else if (st->t->conflicts == 0u) {
            st->budget = "LLG_RAND_CONFLICTS";
        } else {
            uint8_t* model = (uint8_t*)cr_alloc(aig.levels ? aig.levels : 1u, 1u);
            uint64_t conflicts = 0;
            llg_cr_status_t found = cr_cdcl_solve(&aig.blast.aig, aig.out, st->rng,
                                                  st->t->conflicts, model, &conflicts);
            st->conflicts += conflicts;
            st->report->engines |= LLG_CR_ENGINE_CDCL;
            if (found == LLG_CR_OK) {
                cr_assign_from_levels(s, model, aig.levels, aig.level_var, aig.level_bit);
                result = LLG_CR_OK;
            } else if (found == LLG_CR_UNSAT) {
                result = LLG_CR_UNSAT;
            } else {
                st->budget = "LLG_RAND_CONFLICTS";
            }
            free(model);
        }
    }
    if (aig.built) cr_comp_aig_free(&aig);
    cr_plan_free(&plan);
    memset(in_cone, 0, p->node_count);
    return result;
}

/* Flatten conjunctions of the R root into conjuncts. */
static void cr_flatten(llg_cr_session_t* s, uint32_t root, cr_u32vec_t* conj, cr_u32vec_t* stack) {
    const llg_cr_problem_t* p = s->p;
    stack->len = 0;
    cr_u32_push(stack, root);
    while (stack->len) {
        uint32_t n = stack->data[--stack->len];
        if (s->status[n] != CR_R) continue; /* known true parts */
        const llg_cr_node_t* nd = &p->nodes[n];
        if (nd->op == LLG_CR_ALL) {
            for (uint32_t i = nd->c; i-- > 0;) cr_u32_push(stack, p->args[nd->b + i]);
        } else if (nd->op == LLG_CR_LAND) {
            cr_u32_push(stack, nd->b);
            cr_u32_push(stack, nd->a);
        } else {
            cr_u32_push(conj, n);
        }
    }
}

static void cr_mark_reachable(llg_cr_session_t* s, uint8_t* reach, cr_u32vec_t* stack) {
    const llg_cr_problem_t* p = s->p;
    stack->len = 0;
    for (uint32_t b = 0; b < p->block_count; ++b)
        if (s->block_enabled[b]) cr_u32_push(stack, p->blocks[b].root);
    for (uint32_t v = 0; v < p->var_count; ++v)
        if (s->var_active[v] && p->vars[v].domain_node != LLG_CR_NONE)
            cr_u32_push(stack, p->vars[v].domain_node);
    while (stack->len) {
        uint32_t n = stack->data[--stack->len];
        if (reach[n]) continue;
        reach[n] = 1;
        cr_push_operands(p, n, stack);
    }
}

/* Gather the root items. Returns OK, UNSAT or ERROR. */
static llg_cr_status_t cr_roots(cr_solve_t* st, cr_u32vec_t* conj, cr_u32vec_t* stack) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    int unsat = 0;
    conj->len = 0;
    uint32_t count = p->block_count + p->var_count;
    for (uint32_t i = 0; i < count; ++i) {
        uint32_t root;
        if (i < p->block_count) {
            if (!s->block_enabled[i]) continue;
            root = p->blocks[i].root;
        } else {
            uint32_t v = i - p->block_count;
            if (!s->var_active[v] || p->vars[v].domain_node == LLG_CR_NONE) continue;
            root = p->vars[v].domain_node;
        }
        if (s->status[root] == CR_E) {
            st->report->reason = s->ereason[root];
            st->report->item = s->eitem[root];
            return LLG_CR_ERROR;
        }
        if (s->status[root] == CR_K) {
            if (!cr_truth(s, root)) unsat = 1;
            continue;
        }
        cr_flatten(s, root, conj, stack);
    }
    return unsat ? LLG_CR_UNSAT : LLG_CR_OK;
}

static void cr_prepare_classify(llg_cr_session_t* s, const uint8_t* reach) {
    for (uint32_t n = 0; n < s->p->node_count; ++n) s->status[n] = reach[n] ? CR_R : CR_SKIP;
}

static void cr_print_stats(const cr_solve_t* st, llg_cr_status_t status) {
    static const char* names[] = {"ok", "unsat", "resource", "error"};
    const llg_cr_report_t* r = st->report;
    fprintf(stderr,
            "llg: randomize stats: problem=%s status=%s engines=%s%s%s%s%s%s components=%u "
            "bdd_nodes=%llu reject_tries=%llu conflicts=%llu\n",
            st->s->p->name ? st->s->p->name : "?", names[status],
            r->engines ? "" : "none",
            (r->engines & LLG_CR_ENGINE_DIRECT) ? "direct," : "",
            (r->engines & LLG_CR_ENGINE_BDD) ? "bdd," : "",
            (r->engines & LLG_CR_ENGINE_CACHE) ? "cache," : "",
            (r->engines & LLG_CR_ENGINE_REJECT) ? "reject," : "",
            (r->engines & LLG_CR_ENGINE_CDCL) ? "cdcl," : "",
            st->components, (unsigned long long)st->bdd_nodes,
            (unsigned long long)st->reject_tries, (unsigned long long)st->conflicts);
}

static llg_cr_status_t cr_solve_inner(cr_solve_t* st, uint8_t* reach);

/* Order an edge applies to: from an enabled block, between two distinct
 * active non-randc variables (18.5.9 L30061-30062 excludes randc). */
static int cr_order_live(const llg_cr_session_t* s, const llg_cr_order_t* o) {
    const llg_cr_problem_t* p = s->p;
    return s->block_enabled[o->block] && o->before != o->after && s->var_active[o->before] &&
           s->var_active[o->after] && !p->vars[o->before].cyclic && !p->vars[o->after].cyclic;
}

/* Heights of the solve-before DAG: a variable's longest path to a sink.
 * Unordered variables have height 0 and so join the last ordered set, as
 * late as possible (18.5.9 L30075-30080, S30-D2). */
static llg_cr_status_t cr_order_heights(cr_solve_t* st) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    uint32_t live = 0;
    for (uint32_t o = 0; o < p->order_count; ++o) live += (uint32_t)cr_order_live(s, &p->orders[o]);
    if (!live) return LLG_CR_OK;
    st->height = (uint32_t*)cr_alloc(p->var_count, sizeof(uint32_t));
    for (uint32_t round = 0;; ++round) {
        int changed = 0;
        for (uint32_t o = 0; o < p->order_count; ++o) {
            const llg_cr_order_t* ord = &p->orders[o];
            if (!cr_order_live(s, ord)) continue;
            if (st->height[ord->before] < st->height[ord->after] + 1u) {
                if (round > p->var_count) {
                    /* Still growing after more rounds than variables: a
                     * circular order (18.5.9 L30071-30072). */
                    st->report->reason = LLG_CR_REASON_SOLVE_BEFORE;
                    st->report->item = ord->before;
                    return LLG_CR_ERROR;
                }
                st->height[ord->before] = st->height[ord->after] + 1u;
                changed = 1;
            }
        }
        if (!changed) return LLG_CR_OK;
    }
}

void llg_cr_begin(llg_cr_session_t* s) {
    if (s->tpl) {
        cr_tpl_begin(s);
        return;
    }
    for (uint32_t v = 0; v < s->p->var_count; ++v) s->var_fixed[v] = 0;
    for (uint32_t c = 0; c < s->p->call_count; ++c) s->call_state[c] = 0;
    s->engines = 0;
}

llg_cr_status_t llg_cr_step(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                            llg_cr_report_t* report) {
    if (s->tpl) return cr_tpl_step(s, rng, report);
    llg_cr_report_t local;
    if (!report) report = &local;
    memset(report, 0, sizeof(*report));
    report->item = LLG_CR_NONE;
    report->engines = s->engines;
    cr_solve_t st;
    memset(&st, 0, sizeof(st));
    st.s = s;
    st.rng = rng;
    st.report = report;
    st.t = llg_cr_tunables();
    llg_cr_status_t status = cr_order_heights(&st);
    if (status == LLG_CR_OK) {
        uint8_t* reach = s->scratch.reach;
        memset(reach, 0, s->p->node_count);
        status = cr_solve_inner(&st, reach);
    }
    free(st.height);
    s->engines = report->engines;
    if (status == LLG_CR_RESOURCE) report->budget = st.budget ? st.budget : "LLG_RAND_REJECT_TRIES";
    if (st.t->stats && status != LLG_CR_NEED_CALLS) cr_print_stats(&st, status);
    return status;
}

int llg_cr_call_ready(const llg_cr_session_t* s, uint32_t call) {
    if (s->tpl) {
        llg_cr_session_t* flat = cr_tpl_flat_session(s);
        return flat ? llg_cr_call_ready(flat, call) : 0;
    }
    return call < s->p->call_count && s->call_state[call] == 1u;
}

const uint64_t* llg_cr_call_arg(const llg_cr_session_t* s, uint32_t call, uint32_t arg) {
    if (s->tpl) {
        llg_cr_session_t* flat = cr_tpl_flat_session(s);
        if (!flat) cr_fatal("no expanded problem");
        return llg_cr_call_arg(flat, call, arg);
    }
    const llg_cr_call_t* c = &s->p->calls[call];
    if (arg >= c->arg_count) cr_fatal("call argument out of range");
    return s->val + s->node_off[s->p->args[c->arg_first + arg]];
}

void llg_cr_set_call_result(llg_cr_session_t* s, uint32_t call, const uint64_t* words,
                            int input_state) {
    if (s->tpl) {
        llg_cr_session_t* flat = cr_tpl_flat_session(s);
        if (!flat) cr_fatal("no expanded problem");
        llg_cr_set_call_result(flat, call, words, input_state);
        return;
    }
    const llg_cr_call_t* c = &s->p->calls[call];
    uint64_t* result = s->call_val + s->call_off[call];
    uint32_t n = CR_WORDS(c->bit_width);
    for (uint32_t i = 0; i < n; ++i) result[i] = words ? words[i] : 0u;
    bv_mask(result, c->bit_width);
    s->call_input[call] = (uint8_t)input_state;
    s->call_state[call] = 2u;
}

llg_cr_status_t llg_cr_solve(llg_cr_session_t* s, const llg_cr_rng_t* rng, llg_cr_call_fn calls,
                             void* context, llg_cr_report_t* report) {
    llg_cr_begin(s);
    for (;;) {
        llg_cr_status_t status = llg_cr_step(s, rng, report);
        if (status != LLG_CR_NEED_CALLS) return status;
        if (!calls) cr_fatal("constraint function call without a thunk");
        for (uint32_t c = 0; c < s->p->call_count; ++c) {
            if (!llg_cr_call_ready(s, c)) continue;
            const llg_cr_call_t* call = &s->p->calls[c];
            for (uint32_t i = 0; i < call->arg_count; ++i) s->call_args[i] = llg_cr_call_arg(s, c, i);
            uint64_t* result = s->tmp_a;
            memset(result, 0, ((size_t)s->max_words + 1u) * sizeof(uint64_t));
            int state = calls(context, c, s->call_args, result);
            llg_cr_set_call_result(s, c, result, state);
        }
    }
}

static llg_cr_status_t cr_solve_inner(cr_solve_t* st, uint8_t* reach) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    cr_scratch_t* x = &s->scratch;
    cr_u32vec_t* conj = &x->conj;
    cr_u32vec_t* stack = &x->stack;
    cr_u32vec_t* support = &x->support;
    cr_u32vec_t* calls = &x->calls;
    cr_u32vec_t* layer_conj = &x->layer_conj;
    cr_u32vec_t* comp_vars = &x->comp_vars;
    cr_u32vec_t* comp_conj = &x->comp_conj;
    cr_mark_reachable(s, reach, stack);

    uint32_t vars = p->var_count ? p->var_count : 1u;
    uint8_t* has_pred = x->var_flags;
    uint8_t* in_p = x->var_flags + vars;
    uint8_t* in_layer = x->var_flags + 2u * (size_t)vars;
    uint32_t* uf = x->var_words;
    uint32_t* bucket = x->var_words + ((size_t)vars + 1u);
    uint32_t* conj_bucket = x->var_words + 2u * ((size_t)vars + 1u);
    uint32_t* fill = x->var_words + 3u * ((size_t)vars + 1u);
    llg_cr_status_t status = LLG_CR_OK;

    for (;;) {
        cr_prepare_classify(s, reach);
        cr_classify(s);
        if (s->need_calls) {
            status = LLG_CR_NEED_CALLS;
            break;
        }
        status = cr_roots(st, conj, stack);
        if (status != LLG_CR_OK) break;

        /* Function-argument priorities (18.5.11); a problem without calls
         * has none. */
        int pending = 0;
        if (p->call_count) {
            memset(has_pred, 0, vars);
            memset(in_p, 0, vars);
        }
        for (size_t k = 0; k < conj->len && p->call_count; ++k) {
            calls->len = 0;
            support->len = 0;
            cr_support(s, conj->data[k], 0, support, calls, stack);
            /* Each pending call (and nested ones) orders its argument
             * variables before the variables that use its result. */
            for (size_t ci = 0; ci < calls->len; ++ci) {
                uint32_t call_node = calls->data[ci];
                pending = 1;
                for (size_t i = 0; i < support->len; ++i) has_pred[support->data[i]] = 1;
                cr_u32vec_t args_direct = {0, 0, 0}, args_all = {0, 0, 0}, nested = {0, 0, 0};
                cr_u32vec_t sub = {0, 0, 0};
                cr_push_operands(p, call_node, &sub);
                for (size_t ai = 0; ai < sub.len; ++ai) {
                    cr_support(s, sub.data[ai], 1, &args_all, NULL, stack);
                    cr_support(s, sub.data[ai], 0, &args_direct, &nested, stack);
                }
                for (size_t i = 0; i < args_all.len; ++i) in_p[args_all.data[i]] = 1;
                if (nested.len)
                    for (size_t i = 0; i < args_direct.len; ++i) has_pred[args_direct.data[i]] = 1;
                /* Nested calls are revisited as their own pending calls. */
                for (size_t ni = 0; ni < nested.len; ++ni) cr_u32_push(calls, nested.data[ni]);
                free(args_direct.data);
                free(args_all.data);
                free(nested.data);
                free(sub.data);
            }
        }

        memset(in_layer, 0, vars);
        uint32_t layer_vars = 0;
        if (!pending) {
            for (uint32_t v = 0; v < p->var_count; ++v)
                if (s->var_active[v] && !s->var_fixed[v]) { in_layer[v] = 1; ++layer_vars; }
        } else {
            for (uint32_t v = 0; v < p->var_count; ++v)
                if (in_p[v] && !has_pred[v] && !s->var_fixed[v]) { in_layer[v] = 1; ++layer_vars; }
            if (!layer_vars) {
                st->report->reason = LLG_CR_REASON_ORDER;
                for (uint32_t v = 0; v < p->var_count; ++v)
                    if (in_p[v]) { st->report->item = v; break; }
                status = LLG_CR_ERROR;
                break;
            }
        }

        /* Conjuncts of this layer: no pending call, support inside it.
         * Without pending calls that is every conjunct: an R variable is
         * active and not fixed, which is exactly the layer. */
        const cr_u32vec_t* lconj = conj;
        if (pending) {
            layer_conj->len = 0;
            for (size_t k = 0; k < conj->len; ++k) {
                calls->len = 0;
                support->len = 0;
                cr_support(s, conj->data[k], 0, support, calls, stack);
                if (calls->len) continue;
                int inside = 1;
                for (size_t i = 0; i < support->len; ++i)
                    if (!in_layer[support->data[i]]) { inside = 0; break; }
                if (inside) cr_u32_push(layer_conj, conj->data[k]);
            }
            lconj = layer_conj;
        }

        /* Components by union-find over conjunct supports. */
        for (uint32_t v = 0; v < p->var_count; ++v) uf[v] = v;
        x->conj_rep.data = (uint32_t*)cr_grow(x->conj_rep.data, &x->conj_rep.cap, lconj->len + 1u,
                                              sizeof(uint32_t));
        uint32_t* conj_rep = x->conj_rep.data;
        for (size_t k = 0; k < lconj->len; ++k) {
            support->len = 0;
            cr_support(s, lconj->data[k], 0, support, NULL, stack);
            uint32_t first = UINT32_MAX;
            for (size_t i = 0; i < support->len; ++i) {
                uint32_t v = support->data[i];
                while (uf[v] != v) v = uf[v] = uf[uf[v]];
                if (first == UINT32_MAX) first = v;
                else if (v != first) {
                    if (v < first) { uf[first] = v; first = v; }
                    else uf[v] = first;
                }
            }
            if (first == UINT32_MAX) cr_fatal("random constraint without variables");
            conj_rep[k] = first;
        }
        for (size_t k = 0; k < lconj->len; ++k) {
            uint32_t v = conj_rep[k];
            while (uf[v] != v) v = uf[v];
            conj_rep[k] = v;
        }

        /* Bucket variables and conjuncts by component (representative = the
         * smallest variable index), then solve components in that order. */
        memset(bucket, 0, ((size_t)vars + 1u) * sizeof(uint32_t));
        memset(conj_bucket, 0, ((size_t)vars + 1u) * sizeof(uint32_t));
        for (uint32_t v = 0; v < p->var_count; ++v) {
            if (!in_layer[v]) continue;
            uint32_t r = v;
            while (uf[r] != r) r = uf[r];
            uf[v] = r;
            bucket[r + 1u]++;
        }
        for (size_t k = 0; k < lconj->len; ++k) conj_bucket[conj_rep[k] + 1u]++;
        for (uint32_t v = 0; v < p->var_count; ++v) {
            bucket[v + 1u] += bucket[v];
            conj_bucket[v + 1u] += conj_bucket[v];
        }
        comp_vars->len = 0;
        comp_vars->data = (uint32_t*)cr_grow(comp_vars->data, &comp_vars->cap, layer_vars + 1u, sizeof(uint32_t));
        comp_conj->data = (uint32_t*)cr_grow(comp_conj->data, &comp_conj->cap, lconj->len + 1u, sizeof(uint32_t));
        memcpy(fill, bucket, ((size_t)vars + 1u) * sizeof(uint32_t));
        for (uint32_t v = 0; v < p->var_count; ++v)
            if (in_layer[v]) comp_vars->data[fill[uf[v]]++] = v;
        memcpy(fill, conj_bucket, ((size_t)vars + 1u) * sizeof(uint32_t));
        for (size_t k = 0; k < lconj->len; ++k)
            comp_conj->data[fill[conj_rep[k]]++] = lconj->data[k];
        for (uint32_t r = 0; r < p->var_count && status == LLG_CR_OK; ++r) {
            uint32_t vfirst = bucket[r], vcount = bucket[r + 1u] - bucket[r];
            if (!vcount) continue;
            uint32_t cfirst = conj_bucket[r], ccount = conj_bucket[r + 1u] - conj_bucket[r];
            if (!ccount) {
                for (uint32_t i = 0; i < vcount; ++i) {
                    uint32_t v = comp_vars->data[vfirst + i];
                    if (p->vars[v].cyclic)
                        cr_cyc_free_draw(s, v, st->rng);
                    else
                        cr_draw_bits(st->rng, s->sol + s->var_off[v], p->vars[v].bit_width);
                }
                st->report->engines |= LLG_CR_ENGINE_DIRECT;
                continue;
            }
            status = cr_solve_component(st, comp_vars->data + vfirst, vcount,
                                        comp_conj->data + cfirst, ccount);
        }
        if (status != LLG_CR_OK) break;
        for (uint32_t v = 0; v < p->var_count; ++v)
            if (in_layer[v]) s->var_fixed[v] = 1;
        if (pending) continue;

        /* Verify the complete assignment with the reference evaluator. */
        cr_prepare_classify(s, reach);
        cr_classify(s);
        llg_cr_report_t saved = *st->report;
        if (cr_roots(st, conj, stack) != LLG_CR_OK || conj->len)
            cr_fatal("internal error: a solution failed verification");
        *st->report = saved;
        break;
    }
    return status;
}
