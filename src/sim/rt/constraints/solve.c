/* ---- sessions ------------------------------------------------------------ */

struct cr_cache_entry {
    cr_u64vec_t key;
    cr_exact_t exact;
    uint32_t* level_var;
    uint32_t* level_bit;
    int used;
    int failed;   /* the exact engine exceeded its budget for this key */
};

#define CR_CACHE_ENTRIES 8u

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
    for (uint32_t v = 0; v < p->var_count; ++v) {
        if (p->vars[v].bit_width == 0u) cr_fatal("zero-width variable");
        if (p->vars[v].domain_node != LLG_CR_NONE && p->vars[v].domain_node >= p->node_count)
            cr_fatal("domain node out of range");
    }
}

llg_cr_session_t* llg_cr_session_get(const llg_cr_problem_t* p, llg_cr_session_t** slot) {
    if (slot && *slot) return *slot;
    cr_validate_problem(p);
    llg_cr_session_t* s = (llg_cr_session_t*)cr_alloc(1u, sizeof(*s));
    s->p = p;
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
    s->next_session = cr_all_sessions;
    cr_all_sessions = s;
    if (slot) *slot = s;
    return s;
}

static void cr_session_free(llg_cr_session_t* s) {
    for (uint32_t i = 0; i < CR_CACHE_ENTRIES; ++i) {
        cr_cache_entry_t* e = &s->cache[i];
        free(e->key.data);
        cr_exact_free(&e->exact);
        free(e->level_var);
        free(e->level_bit);
    }
    free(s->cache);
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
}

void llg_cr_set_var(llg_cr_session_t* s, uint32_t var, const uint64_t* words,
                    int active, int input_state) {
    if (var >= s->p->var_count) cr_fatal("variable out of range");
    uint32_t n = CR_WORDS(s->p->vars[var].bit_width);
    uint64_t* d = s->var_cur + s->var_off[var];
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
    bv_mask(d, s->p->vars[var].bit_width);
    s->var_active[var] = (uint8_t)(active != 0);
    s->var_input[var] = (uint8_t)input_state;
}

void llg_cr_set_block(llg_cr_session_t* s, uint32_t block, int enabled) {
    if (block >= s->p->block_count) cr_fatal("block out of range");
    s->block_enabled[block] = (uint8_t)(enabled != 0);
}

void llg_cr_set_leaf(llg_cr_session_t* s, uint32_t leaf, const uint64_t* words,
                     int input_state) {
    if (leaf >= s->p->leaf_count) cr_fatal("state leaf out of range");
    uint32_t n = CR_WORDS(s->p->leaves[leaf].bit_width);
    uint64_t* d = s->leaf_val + s->leaf_off[leaf];
    for (uint32_t i = 0; i < n; ++i) d[i] = words ? words[i] : 0u;
    bv_mask(d, s->p->leaves[leaf].bit_width);
    s->leaf_input[leaf] = (uint8_t)input_state;
}

const uint64_t* llg_cr_value(const llg_cr_session_t* s, uint32_t var) {
    return s->sol + s->var_off[var];
}

uint32_t llg_cr_var_words(const llg_cr_session_t* s, uint32_t var) {
    return CR_WORDS(s->p->vars[var].bit_width);
}

/* ---- solve helpers -------------------------------------------------------- */

static void cr_push_operands(const llg_cr_problem_t* p, uint32_t n, cr_u32vec_t* out) {
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
        cr_push_operands(p, n, stack);
    }
}

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
    uint32_t components;
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
        cr_push_operands(s->p, n, stack);
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
        cr_push_operands(s->p, n, scratch);
        for (size_t i = 0; i < scratch->len; ++i) {
            uint32_t x = scratch->data[i];
            if (s->status[x] != CR_K) continue;
            cr_u64_push(key, ((uint64_t)n << 32) | x);
            const uint64_t* v = cr_node_val(s, x);
            for (uint32_t w = 0; w < CR_WORDS(s->p->nodes[x].bit_width); ++w) cr_u64_push(key, v[w]);
        }
    }
}

/* Take the next cache slot (round robin), releasing what it held. */
static cr_cache_entry_t* cr_cache_slot(llg_cr_session_t* s) {
    cr_cache_entry_t* e = &s->cache[s->cache_next];
    s->cache_next = (s->cache_next + 1u) % CR_CACHE_ENTRIES;
    free(e->key.data);
    cr_exact_free(&e->exact);
    free(e->level_var);
    free(e->level_bit);
    memset(e, 0, sizeof(*e));
    return e;
}

static void cr_remember_failure(llg_cr_session_t* s, cr_u64vec_t* key) {
    cr_cache_entry_t* e = cr_cache_slot(s);
    e->key = *key;
    key->data = NULL;
    key->len = key->cap = 0;
    e->used = 1;
    e->failed = 1;
}

static void cr_assign_from_levels(llg_cr_session_t* s, const uint8_t* bits, uint32_t levels,
                                  const uint32_t* level_var, const uint32_t* level_bit) {
    for (uint32_t l = 0; l < levels; ++l)
        bv_set_bit(s->sol + s->var_off[level_var[l]], level_bit[l], bits[l]);
}

static void cr_sample_exact(cr_solve_t* st, const cr_exact_t* e, const uint32_t* level_var,
                            const uint32_t* level_bit) {
    uint8_t* bits = (uint8_t*)cr_alloc(e->levels ? e->levels : 1u, 1u);
    cr_exact_sample(e, st->rng, bits);
    cr_assign_from_levels(st->s, bits, e->levels, level_var, level_bit);
    free(bits);
}

/* Evaluate the cone's R nodes with the candidate values in `sol`. */
static int cr_eval_cone(llg_cr_session_t* s, const uint8_t* in_cone, const uint32_t* conj,
                        uint32_t ncon) {
    const llg_cr_problem_t* p = s->p;
    for (uint32_t n = 0; n < p->node_count; ++n) {
        if (!in_cone[n]) continue;
        const llg_cr_node_t* nd = &p->nodes[n];
        if (nd->op == LLG_CR_VAR) {
            const llg_cr_var_t* var = &p->vars[nd->a];
            cr_copy_input(s, n, s->sol + s->var_off[nd->a], var->bit_width, var->sign_flag);
        } else if (cr_eval_op(s, n)) {
            return 0; /* an evaluation error never satisfies a constraint */
        }
    }
    for (uint32_t i = 0; i < ncon; ++i)
        if (!cr_truth(s, conj[i])) return 0;
    return 1;
}

static llg_cr_status_t cr_solve_component(cr_solve_t* st, const uint32_t* vars, uint32_t nvars,
                                          const uint32_t* conj, uint32_t ncon) {
    llg_cr_session_t* s = st->s;
    const llg_cr_problem_t* p = s->p;
    st->components++;
    uint64_t bits = 0;
    uint32_t maxw = 0;
    for (uint32_t i = 0; i < nvars; ++i) {
        bits += p->vars[vars[i]].bit_width;
        if (p->vars[vars[i]].bit_width > maxw) maxw = p->vars[vars[i]].bit_width;
    }
    uint8_t* in_cone = (uint8_t*)cr_alloc(p->node_count ? p->node_count : 1u, 1u);
    cr_u32vec_t stack = {0, 0, 0};
    cr_mark_cone(s, conj, ncon, in_cone, &stack);
    llg_cr_status_t result = LLG_CR_RESOURCE;
    int done = 0;

    if (bits <= st->t->bdd_bits) {
        cr_u64vec_t key = {0, 0, 0};
        cr_cache_key(s, conj, ncon, vars, nvars, in_cone, &key, &stack);
        int known_failure = 0;
        for (uint32_t i = 0; i < CR_CACHE_ENTRIES && !done && !known_failure; ++i) {
            cr_cache_entry_t* e = &s->cache[i];
            if (!e->used || e->key.len != key.len ||
                memcmp(e->key.data, key.data, key.len * sizeof(uint64_t)) != 0)
                continue;
            if (e->failed) {
                /* Same inputs as an earlier overflow: skip straight to the
                 * fallbacks, as a rebuild would overflow identically. */
                known_failure = 1;
                st->budget = st->budget ? st->budget : "LLG_RAND_BDD_NODES";
                continue;
            }
            cr_sample_exact(st, &e->exact, e->level_var, e->level_bit);
            st->report->engines |= LLG_CR_ENGINE_BDD | LLG_CR_ENGINE_CACHE;
            result = LLG_CR_OK;
            done = 1;
        }
        if (!done && !known_failure) {
            /* Level order: bit positions most significant first, variables
             * interleaved in index order. */
            uint32_t levels = (uint32_t)bits;
            uint32_t* level_var = (uint32_t*)cr_alloc(levels ? levels : 1u, sizeof(uint32_t));
            uint32_t* level_bit = (uint32_t*)cr_alloc(levels ? levels : 1u, sizeof(uint32_t));
            uint32_t* var_base = (uint32_t*)cr_alloc(p->var_count, sizeof(uint32_t));
            uint32_t* bit_input = (uint32_t*)cr_alloc(levels ? levels : 1u, sizeof(uint32_t));
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
                    level_var[l] = v;
                    level_bit[l] = pos;
                    bit_input[var_base[v] + pos] = l + 1u;
                    ++l;
                }
            }
            cr_blast_t blast;
            uint64_t aig_limit = st->t->bdd_nodes > (UINT64_MAX / 8u) ? UINT64_MAX : st->t->bdd_nodes * 8u;
            if (aig_limit < 4096u) aig_limit = 4096u;
            cr_aig_init(&blast.aig, levels, aig_limit + levels);
            blast.lit_off = (uint32_t*)cr_alloc(p->node_count ? p->node_count : 1u, sizeof(uint32_t));
            for (uint32_t n = 0; n < p->node_count; ++n) blast.lit_off[n] = UINT32_MAX;
            blast.lits.data = NULL;
            blast.lits.len = blast.lits.cap = 0;
            for (uint32_t n = 0; n < p->node_count && !blast.aig.overflow; ++n)
                if (in_cone[n]) cr_blast_node(s, &blast, n, var_base, bit_input);
            uint32_t out = CR_TRUE;
            for (uint32_t i = 0; i < ncon && !blast.aig.overflow; ++i)
                out = cr_and(&blast.aig, out, cr_operand_truth(s, &blast, conj[i]));
            if (!blast.aig.overflow) {
                if (out == CR_FALSE) {
                    result = LLG_CR_UNSAT;
                    done = 1;
                } else {
                    cr_bdd_t m;
                    cr_bdd_init(&m, levels, st->t->bdd_nodes);
                    uint32_t root = cr_bdd_from_aig(&m, &blast.aig, out);
                    st->bdd_nodes += m.count;
                    if (!m.overflow) {
                        if (root == 0u) {
                            result = LLG_CR_UNSAT;
                        } else {
                            cr_cache_entry_t* e = cr_cache_slot(s);
                            e->key = key;
                            key.data = NULL;
                            key.len = key.cap = 0;
                            cr_exact_build(&e->exact, &m, root);
                            e->level_var = level_var;
                            e->level_bit = level_bit;
                            level_var = level_bit = NULL;
                            e->used = 1;
                            cr_sample_exact(st, &e->exact, e->level_var, e->level_bit);
                            result = LLG_CR_OK;
                        }
                        st->report->engines |= LLG_CR_ENGINE_BDD;
                        done = 1;
                    } else {
                        cr_remember_failure(s, &key);
                        if (!st->budget) st->budget = "LLG_RAND_BDD_NODES";
                    }
                    cr_bdd_free(&m);
                }
            } else {
                cr_remember_failure(s, &key);
                if (!st->budget) st->budget = "LLG_RAND_BDD_NODES";
            }
            cr_aig_free(&blast.aig);
            free(blast.lit_off);
            free(blast.lits.data);
            free(level_var);
            free(level_bit);
            free(var_base);
            free(bit_input);
        }
        free(key.data);
    } else if (!st->budget) {
        st->budget = "LLG_RAND_BDD_BITS";
    }

    if (!done) {
        /* Uniform rejection: every accepted draw is uniform over the
         * solutions because draws cover each variable's whole domain. */
        for (uint64_t attempt = 0; attempt < st->t->reject_tries; ++attempt) {
            for (uint32_t i = 0; i < nvars; ++i)
                cr_draw_bits(st->rng, s->sol + s->var_off[vars[i]], p->vars[vars[i]].bit_width);
            st->reject_tries++;
            if (cr_eval_cone(s, in_cone, conj, ncon)) {
                st->report->engines |= LLG_CR_ENGINE_REJECT;
                result = LLG_CR_OK;
                done = 1;
                break;
            }
        }
        if (!done) st->budget = st->budget ? st->budget : "LLG_RAND_REJECT_TRIES";
    }
    free(stack.data);
    free(in_cone);
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

static void cr_mark_reachable(llg_cr_session_t* s, uint8_t* reach) {
    const llg_cr_problem_t* p = s->p;
    cr_u32vec_t stack = {0, 0, 0};
    for (uint32_t b = 0; b < p->block_count; ++b)
        if (s->block_enabled[b]) cr_u32_push(&stack, p->blocks[b].root);
    for (uint32_t v = 0; v < p->var_count; ++v)
        if (s->var_active[v] && p->vars[v].domain_node != LLG_CR_NONE)
            cr_u32_push(&stack, p->vars[v].domain_node);
    while (stack.len) {
        uint32_t n = stack.data[--stack.len];
        if (reach[n]) continue;
        reach[n] = 1;
        cr_push_operands(p, n, &stack);
    }
    free(stack.data);
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
            "bdd_nodes=%llu reject_tries=%llu\n",
            st->s->p->name ? st->s->p->name : "?", names[status],
            r->engines ? "" : "none",
            (r->engines & LLG_CR_ENGINE_DIRECT) ? "direct," : "",
            (r->engines & LLG_CR_ENGINE_BDD) ? "bdd," : "",
            (r->engines & LLG_CR_ENGINE_CACHE) ? "cache," : "",
            (r->engines & LLG_CR_ENGINE_REJECT) ? "reject," : "",
            (r->engines & LLG_CR_ENGINE_CDCL) ? "cdcl," : "",
            st->components, (unsigned long long)st->bdd_nodes,
            (unsigned long long)st->reject_tries);
}

static llg_cr_status_t cr_solve_inner(cr_solve_t* st, uint8_t* reach);

void llg_cr_begin(llg_cr_session_t* s) {
    for (uint32_t v = 0; v < s->p->var_count; ++v) s->var_fixed[v] = 0;
    for (uint32_t c = 0; c < s->p->call_count; ++c) s->call_state[c] = 0;
    s->engines = 0;
}

llg_cr_status_t llg_cr_step(llg_cr_session_t* s, const llg_cr_rng_t* rng,
                            llg_cr_report_t* report) {
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
    uint8_t* reach = (uint8_t*)cr_alloc(s->p->node_count ? s->p->node_count : 1u, 1u);
    llg_cr_status_t status = cr_solve_inner(&st, reach);
    free(reach);
    s->engines = report->engines;
    if (status == LLG_CR_RESOURCE) report->budget = st.budget ? st.budget : "LLG_RAND_REJECT_TRIES";
    if (st.t->stats && status != LLG_CR_NEED_CALLS) cr_print_stats(&st, status);
    return status;
}

int llg_cr_call_ready(const llg_cr_session_t* s, uint32_t call) {
    return call < s->p->call_count && s->call_state[call] == 1u;
}

const uint64_t* llg_cr_call_arg(const llg_cr_session_t* s, uint32_t call, uint32_t arg) {
    const llg_cr_call_t* c = &s->p->calls[call];
    if (arg >= c->arg_count) cr_fatal("call argument out of range");
    return s->val + s->node_off[s->p->args[c->arg_first + arg]];
}

void llg_cr_set_call_result(llg_cr_session_t* s, uint32_t call, const uint64_t* words,
                            int input_state) {
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
    cr_mark_reachable(s, reach);

    cr_u32vec_t conj = {0, 0, 0}, stack = {0, 0, 0}, support = {0, 0, 0}, calls = {0, 0, 0};
    cr_u32vec_t layer_conj = {0, 0, 0}, comp_vars = {0, 0, 0}, comp_conj = {0, 0, 0};
    uint32_t vars = p->var_count ? p->var_count : 1u;
    uint8_t* has_pred = (uint8_t*)cr_alloc(vars, 1u);
    uint8_t* in_p = (uint8_t*)cr_alloc(vars, 1u);
    uint8_t* in_layer = (uint8_t*)cr_alloc(vars, 1u);
    uint32_t* uf = (uint32_t*)cr_alloc(vars, sizeof(uint32_t));
    uint32_t* conj_rep = NULL;
    size_t conj_rep_cap = 0;
    llg_cr_status_t status = LLG_CR_OK;

    for (;;) {
        cr_prepare_classify(s, reach);
        cr_classify(s);
        if (s->need_calls) {
            status = LLG_CR_NEED_CALLS;
            break;
        }
        status = cr_roots(st, &conj, &stack);
        if (status != LLG_CR_OK) break;

        /* Function-argument priorities (18.5.11). */
        memset(has_pred, 0, vars);
        memset(in_p, 0, vars);
        int pending = 0;
        for (size_t k = 0; k < conj.len; ++k) {
            calls.len = 0;
            support.len = 0;
            cr_support(s, conj.data[k], 0, &support, &calls, &stack);
            /* Each pending call (and nested ones) orders its argument
             * variables before the variables that use its result. */
            for (size_t ci = 0; ci < calls.len; ++ci) {
                uint32_t call_node = calls.data[ci];
                pending = 1;
                for (size_t i = 0; i < support.len; ++i) has_pred[support.data[i]] = 1;
                cr_u32vec_t args_direct = {0, 0, 0}, args_all = {0, 0, 0}, nested = {0, 0, 0};
                cr_u32vec_t sub = {0, 0, 0};
                cr_push_operands(p, call_node, &sub);
                for (size_t ai = 0; ai < sub.len; ++ai) {
                    cr_support(s, sub.data[ai], 1, &args_all, NULL, &stack);
                    cr_support(s, sub.data[ai], 0, &args_direct, &nested, &stack);
                }
                for (size_t i = 0; i < args_all.len; ++i) in_p[args_all.data[i]] = 1;
                if (nested.len)
                    for (size_t i = 0; i < args_direct.len; ++i) has_pred[args_direct.data[i]] = 1;
                /* Nested calls are revisited as their own pending calls. */
                for (size_t ni = 0; ni < nested.len; ++ni) cr_u32_push(&calls, nested.data[ni]);
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

        /* Conjuncts of this layer: no pending call, support inside it. */
        layer_conj.len = 0;
        for (size_t k = 0; k < conj.len; ++k) {
            calls.len = 0;
            support.len = 0;
            cr_support(s, conj.data[k], 0, &support, &calls, &stack);
            if (calls.len) continue;
            int inside = 1;
            for (size_t i = 0; i < support.len; ++i)
                if (!in_layer[support.data[i]]) { inside = 0; break; }
            if (inside) cr_u32_push(&layer_conj, conj.data[k]);
        }

        /* Components by union-find over conjunct supports. */
        for (uint32_t v = 0; v < p->var_count; ++v) uf[v] = v;
        conj_rep = (uint32_t*)cr_grow(conj_rep, &conj_rep_cap, layer_conj.len + 1u, sizeof(uint32_t));
        for (size_t k = 0; k < layer_conj.len; ++k) {
            support.len = 0;
            cr_support(s, layer_conj.data[k], 0, &support, NULL, &stack);
            uint32_t first = UINT32_MAX;
            for (size_t i = 0; i < support.len; ++i) {
                uint32_t v = support.data[i];
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
        for (size_t k = 0; k < layer_conj.len; ++k) {
            uint32_t v = conj_rep[k];
            while (uf[v] != v) v = uf[v];
            conj_rep[k] = v;
        }

        /* Bucket variables and conjuncts by component (representative = the
         * smallest variable index), then solve components in that order. */
        uint32_t* bucket = (uint32_t*)cr_alloc((size_t)vars + 1u, sizeof(uint32_t));
        uint32_t* conj_bucket = (uint32_t*)cr_alloc((size_t)vars + 1u, sizeof(uint32_t));
        for (uint32_t v = 0; v < p->var_count; ++v) {
            if (!in_layer[v]) continue;
            uint32_t r = v;
            while (uf[r] != r) r = uf[r];
            uf[v] = r;
            bucket[r + 1u]++;
        }
        for (size_t k = 0; k < layer_conj.len; ++k) conj_bucket[conj_rep[k] + 1u]++;
        for (uint32_t v = 0; v < p->var_count; ++v) {
            bucket[v + 1u] += bucket[v];
            conj_bucket[v + 1u] += conj_bucket[v];
        }
        comp_vars.len = 0;
        comp_vars.data = (uint32_t*)cr_grow(comp_vars.data, &comp_vars.cap, layer_vars + 1u, sizeof(uint32_t));
        comp_conj.data = (uint32_t*)cr_grow(comp_conj.data, &comp_conj.cap, layer_conj.len + 1u, sizeof(uint32_t));
        {
            uint32_t* fill = (uint32_t*)cr_alloc((size_t)vars + 1u, sizeof(uint32_t));
            memcpy(fill, bucket, ((size_t)vars + 1u) * sizeof(uint32_t));
            for (uint32_t v = 0; v < p->var_count; ++v)
                if (in_layer[v]) comp_vars.data[fill[uf[v]]++] = v;
            memcpy(fill, conj_bucket, ((size_t)vars + 1u) * sizeof(uint32_t));
            for (size_t k = 0; k < layer_conj.len; ++k)
                comp_conj.data[fill[conj_rep[k]]++] = layer_conj.data[k];
            free(fill);
        }
        for (uint32_t r = 0; r < p->var_count && status == LLG_CR_OK; ++r) {
            uint32_t vfirst = bucket[r], vcount = bucket[r + 1u] - bucket[r];
            if (!vcount) continue;
            uint32_t cfirst = conj_bucket[r], ccount = conj_bucket[r + 1u] - conj_bucket[r];
            if (!ccount) {
                for (uint32_t i = 0; i < vcount; ++i) {
                    uint32_t v = comp_vars.data[vfirst + i];
                    cr_draw_bits(st->rng, s->sol + s->var_off[v], p->vars[v].bit_width);
                }
                st->report->engines |= LLG_CR_ENGINE_DIRECT;
                continue;
            }
            status = cr_solve_component(st, comp_vars.data + vfirst, vcount,
                                        comp_conj.data + cfirst, ccount);
        }
        free(bucket);
        free(conj_bucket);
        if (status != LLG_CR_OK) break;
        for (uint32_t v = 0; v < p->var_count; ++v)
            if (in_layer[v]) s->var_fixed[v] = 1;
        if (pending) continue;

        /* Verify the complete assignment with the reference evaluator. */
        cr_prepare_classify(s, reach);
        cr_classify(s);
        llg_cr_report_t saved = *st->report;
        if (cr_roots(st, &conj, &stack) != LLG_CR_OK || conj.len)
            cr_fatal("internal error: a solution failed verification");
        *st->report = saved;
        break;
    }
    free(conj.data); free(stack.data); free(support.data); free(calls.data);
    free(layer_conj.data); free(comp_vars.data); free(comp_conj.data);
    free(has_pred); free(in_p); free(in_layer); free(uf); free(conj_rep);
    return status;
}
