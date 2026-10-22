/* ---- layered and weighted exact sampling ---------------------------------
 *
 * SIM-029's exact engine draws uniformly over a component's solutions.
 * `randc` (18.4.2), `solve...before` (18.5.9) and `dist` (18.5.4) change that
 * measure (design: SIM-030 decisions S30-D1 to S30-D3):
 *
 * - Variables are split into ordered layers. Layer i is drawn from
 *   Q_i = exists(later layers). F with the earlier layers' bits fixed, so a
 *   value of an earlier layer is weighted by its own layer only, never by the
 *   number of completions in later layers.
 * - A dist constraint adds one selector level per weighted item below the
 *   variable bits; a selector's 1-literal carries the item's integer weight
 *   (per-literal weighted model counting), every variable bit weighs 1.
 * - A layer drawn by item (an unconditional dist, S30-D1) first chooses an
 *   item by its mass; its selector then weighs 1 and the others 0 while the
 *   layer is drawn.
 *
 * Counts depend on the fixed bits, so they are recomputed for each layer of
 * each sample over the compact model's nodes. Decisions draw a fresh
 * uniform integer below each node's weighted count, which is exact.
 */

typedef struct {
    uint32_t levels;
    uint32_t limbs;
    uint32_t layer_count;
    uint8_t* layer;          /* per level */
    uint8_t* selector;       /* per level: a dist selector bit */
    uint64_t* weight;        /* per level: limbs words, the 1-literal weight */
    uint32_t* cyclic;        /* per layer: randc variable or LLG_CR_NONE */
    /* Per layer: the selector levels [pick_first, pick_first + pick_count)
     * of the dist drawn by item there (S30-D1; the first is the guard-fail
     * selector, the others' weights are the item masses), or pick_count 0. */
    uint32_t* pick_first;
    uint32_t* pick_count;
} cr_layout_t;

static void cr_layout_free(cr_layout_t* l) {
    free(l->layer);
    free(l->selector);
    free(l->weight);
    free(l->cyclic);
    free(l->pick_first);
    free(l->pick_count);
    memset(l, 0, sizeof(*l));
}

static void cr_layout_copy(cr_layout_t* d, const cr_layout_t* s) {
    *d = *s;
    uint32_t levels = s->levels ? s->levels : 1u;
    d->layer = (uint8_t*)cr_alloc(levels, 1u);
    d->selector = (uint8_t*)cr_alloc(levels, 1u);
    d->weight = (uint64_t*)cr_alloc((size_t)levels * s->limbs, sizeof(uint64_t));
    d->cyclic = (uint32_t*)cr_alloc(s->layer_count ? s->layer_count : 1u, sizeof(uint32_t));
    memcpy(d->layer, s->layer, s->levels);
    memcpy(d->selector, s->selector, s->levels);
    memcpy(d->weight, s->weight, (size_t)s->levels * s->limbs * sizeof(uint64_t));
    memcpy(d->cyclic, s->cyclic, s->layer_count * sizeof(uint32_t));
    d->pick_first = d->pick_count = NULL;
    if (s->pick_first) {
        d->pick_first = (uint32_t*)cr_alloc(s->layer_count, sizeof(uint32_t));
        d->pick_count = (uint32_t*)cr_alloc(s->layer_count, sizeof(uint32_t));
        memcpy(d->pick_first, s->pick_first, s->layer_count * sizeof(uint32_t));
        memcpy(d->pick_count, s->pick_count, s->layer_count * sizeof(uint32_t));
    }
}

/* d = a * b (limbs words, the product must fit). d must not alias. */
static void bn_mul(uint64_t* d, const uint64_t* a, const uint64_t* b, uint32_t limbs) {
    for (uint32_t i = 0; i < limbs; ++i) d[i] = 0;
    for (uint32_t i = 0; i < limbs; ++i) {
        if (!a[i]) continue;
        uint64_t carry = 0;
        for (uint32_t j = 0; i + j < limbs; ++j) {
            uint64_t lo, hi;
            cr_mul64(a[i], b[j], &lo, &hi);
            uint64_t t = d[i + j] + lo;
            hi += t < lo;
            uint64_t u = t + carry;
            hi += u < t;
            d[i + j] = u;
            carry = hi;
        }
    }
}

typedef struct {
    const cr_exact_t* e;
    const cr_layout_t* l;
    uint32_t layer;          /* layer being drawn */
    const int8_t* fixed;     /* per level: -1 or the fixed bit */
    const uint64_t* weight;  /* per level, limbs words: the 1-literal weights in use */
    uint64_t* cnt;           /* per node, limbs words */
    uint32_t* count_prefix;  /* per level + 1: counted variable levels before */
    uint64_t* t1;
    uint64_t* t2;
} cr_wcount_t;

static int cr_level_counted(const cr_wcount_t* c, uint32_t level) {
    return c->l->layer[level] == c->layer && c->fixed[level] < 0;
}

/* out = product over the counted levels in [from, to) of (1 + weight): a
 * variable bit gives 2, a selector 1 + its weight. */
static void cr_gap(cr_wcount_t* c, uint64_t* out, uint32_t from, uint32_t to) {
    uint32_t limbs = c->l->limbs;
    for (uint32_t i = 0; i < limbs; ++i) out[i] = 0;
    out[0] = 1;
    if (from >= to) return;
    uint32_t vars = c->count_prefix[to] - c->count_prefix[from];
    for (uint32_t level = from; level < to; ++level) {
        if (!c->l->selector[level] || !cr_level_counted(c, level)) continue;
        const uint64_t* w = c->weight + (size_t)level * limbs;
        memcpy(c->t1, w, limbs * sizeof(uint64_t));
        uint64_t one[1] = {1};
        bn_add(c->t1, one, 1u);
        bn_mul(c->t2, out, c->t1, limbs);
        memcpy(out, c->t2, limbs * sizeof(uint64_t));
    }
    bn_shl(c->t1, out, vars, limbs);
    memcpy(out, c->t1, limbs * sizeof(uint64_t));
}

/* Weight of taking `bit` at node n: w_bit * gap * count(child). */
static void cr_branch_weight(cr_wcount_t* c, uint64_t* out, uint32_t n, int bit) {
    const cr_exact_t* e = c->e;
    uint32_t limbs = c->l->limbs;
    uint32_t child = bit ? e->hi[n] : e->lo[n];
    uint32_t level = e->level[n];
    uint64_t* gap = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
    cr_gap(c, gap, level + 1u, e->level[child]);
    bn_mul(out, gap, c->cnt + (size_t)child * limbs, limbs);
    if (bit && c->l->selector[level]) {
        memcpy(gap, out, limbs * sizeof(uint64_t));
        bn_mul(out, gap, c->weight + (size_t)level * limbs, limbs);
    }
    free(gap);
}

static void cr_wcount_prepare(cr_wcount_t* c) {
    const cr_exact_t* e = c->e;
    uint32_t limbs = c->l->limbs;
    c->count_prefix[0] = 0;
    for (uint32_t level = 0; level < e->levels; ++level)
        c->count_prefix[level + 1u] =
            c->count_prefix[level] + (cr_level_counted(c, level) && !c->l->selector[level]);
    memset(c->cnt, 0, (size_t)e->nodes * limbs * sizeof(uint64_t));
    c->cnt[limbs] = 1u; /* terminal 1 */
    uint64_t* w = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
    for (uint32_t n = 2; n < e->nodes; ++n) {
        uint64_t* d = c->cnt + (size_t)n * limbs;
        int fixed = c->fixed[e->level[n]];
        for (int bit = 0; bit < 2; ++bit) {
            if (fixed >= 0 && fixed != bit) continue;
            cr_branch_weight(c, w, n, bit);
            bn_add(d, w, limbs);
        }
    }
    free(w);
}

/* Draw the counted levels of the current layer below `root` into bits[]. */
static int cr_wcount_draw(cr_wcount_t* c, uint32_t root, const llg_cr_rng_t* rng, uint8_t* bits) {
    const cr_exact_t* e = c->e;
    uint32_t limbs = c->l->limbs;
    uint64_t* total = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
    uint64_t* w0 = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
    uint64_t* r = (uint64_t*)cr_alloc(limbs, sizeof(uint64_t));
    cr_gap(c, total, 0u, e->level[root]);
    bn_mul(w0, total, c->cnt + (size_t)root * limbs, limbs);
    int ok = bn_bits(w0, limbs) != 0;
    uint32_t level = 0;
    uint32_t n = root;
    while (ok) {
        for (; level < e->level[n]; ++level) {
            if (!cr_level_counted(c, level)) continue;
            if (c->l->selector[level]) cr_fatal("weighted selector skipped on a solution path");
            bits[level] = (uint8_t)(rng->next(rng->stream) & 1u);
        }
        if (n < 2u) break;
        int fixed = c->fixed[level];
        if (fixed >= 0) {
            n = fixed ? e->hi[n] : e->lo[n];
            ++level;
            continue;
        }
        cr_branch_weight(c, w0, n, 0);
        memcpy(total, c->cnt + (size_t)n * limbs, limbs * sizeof(uint64_t));
        bn_uniform(rng, r, total, limbs);
        int bit = bn_cmp(r, w0, limbs) >= 0;
        bits[level++] = (uint8_t)bit;
        n = bit ? e->hi[n] : e->lo[n];
    }
    if (ok && n != 1u) cr_fatal("weighted sampling reached the false terminal");
    free(total);
    free(w0);
    free(r);
    return ok;
}

/* Every value of the cyclic variable drawn in the current layer (its bit
 * positions in level_bit) that Q has a path for with the fixed bits. The
 * recursion depth is bounded by the level count (LLG_RAND_BDD_BITS). */
static void cr_enum_values(const cr_wcount_t* c, uint32_t n, uint32_t level, uint64_t value,
                           const uint32_t* level_bit, cr_u64vec_t* out) {
    const cr_exact_t* e = c->e;
    if (n == 0u) return;
    if (level == e->levels) {
        if (n == 1u) cr_u64_push(out, value);
        return;
    }
    uint64_t bit = UINT64_C(1) << level_bit[level];
    if (n >= 2u && e->level[n] == level) {
        int fixed = c->fixed[level];
        if (fixed >= 0) {
            cr_enum_values(c, fixed ? e->hi[n] : e->lo[n], level + 1u, value, level_bit, out);
            return;
        }
        cr_enum_values(c, e->lo[n], level + 1u, value, level_bit, out);
        cr_enum_values(c, e->hi[n], level + 1u, value | bit, level_bit, out);
        return;
    }
    cr_enum_values(c, n, level + 1u, value, level_bit, out);
    if (cr_level_counted(c, level)) cr_enum_values(c, n, level + 1u, value | bit, level_bit, out);
}

/* ---- randc cycle state --------------------------------------------------- */

static uint64_t cr_mix(uint64_t h) {
    h ^= h >> 33;
    h *= UINT64_C(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h *= UINT64_C(0xc4ceb9fe1a85ec53);
    h ^= h >> 33;
    return h;
}

static uint64_t cr_hash_text(const char* text) {
    uint64_t h = UINT64_C(0xcbf29ce484222325);
    for (const char* p = text ? text : ""; *p; ++p) h = (h ^ (uint8_t)*p) * UINT64_C(0x100000001b3);
    return h;
}

static uint64_t* cr_cyc_state(llg_cr_session_t* s, uint32_t var) {
    return s->cyc + s->cyc_off[var];
}

static int cr_cyc_used(llg_cr_session_t* s, uint32_t var, uint64_t value) {
    return (int)((cr_cyc_state(s, var)[value / 64u] >> (value % 64u)) & 1u);
}

/* Start a new cycle when the signature differs (S30-D3). */
static void cr_cyc_check(llg_cr_session_t* s, uint32_t var, uint64_t signature) {
    uint32_t words = LLG_CR_CYCLIC_WORDS(s->p->vars[var].bit_width);
    uint64_t* st = cr_cyc_state(s, var);
    if (st[words - 1u] != signature) {
        memset(st, 0, (words - 1u) * sizeof(uint64_t));
        st[words - 1u] = signature;
    }
}

static void cr_cyc_reset(llg_cr_session_t* s, uint32_t var) {
    uint32_t words = LLG_CR_CYCLIC_WORDS(s->p->vars[var].bit_width);
    memset(cr_cyc_state(s, var), 0, (words - 1u) * sizeof(uint64_t));
}

static void cr_cyc_mark(llg_cr_session_t* s, uint32_t var, uint64_t value) {
    cr_cyc_state(s, var)[value / 64u] |= UINT64_C(1) << (value % 64u);
}

/* Values of a width-w cyclic variable held in bitmap word i. */
static uint64_t cr_cyc_word_mask(uint32_t w, uint32_t i) {
    (void)i;
    return w >= 6u ? UINT64_MAX : ((UINT64_C(1) << (1u << w)) - 1u);
}

static uint32_t cr_popcount64(uint64_t x) {
    uint32_t n = 0;
    for (; x; x &= x - 1u) ++n;
    return n;
}

/* A cyclic variable without constraints: uniform among its unused values. */
static void cr_cyc_free_draw(llg_cr_session_t* s, uint32_t var, const llg_cr_rng_t* rng) {
    uint32_t w = s->p->vars[var].bit_width;
    uint32_t words = LLG_CR_CYCLIC_WORDS(w) - 1u;
    cr_cyc_check(s, var, cr_mix(cr_hash_text(s->p->vars[var].name) ^ UINT64_C(0x6672656576617200)));
    uint64_t* st = cr_cyc_state(s, var);
    uint64_t unused = 0;
    for (uint32_t i = 0; i < words; ++i) unused += cr_popcount64(~st[i] & cr_cyc_word_mask(w, i));
    if (!unused) {
        cr_cyc_reset(s, var);
        unused = w >= 6u ? (uint64_t)words * 64u : (UINT64_C(1) << w);
    }
    uint64_t bound[2] = {unused, 0}, r[2];
    bn_uniform(rng, r, bound, 2u);
    for (uint32_t i = 0; i < words; ++i) {
        uint64_t free_bits = ~st[i] & cr_cyc_word_mask(w, i);
        uint32_t n = cr_popcount64(free_bits);
        if (r[0] >= n) {
            r[0] -= n;
            continue;
        }
        for (uint32_t b = 0; b < 64u; ++b) {
            if (!((free_bits >> b) & 1u)) continue;
            if (r[0]-- == 0u) {
                uint64_t v = (uint64_t)i * 64u + b;
                uint64_t* d = s->sol + s->var_off[var];
                d[0] = v;
                cr_cyc_mark(s, var, v);
                return;
            }
        }
    }
    cr_fatal("cyclic draw found no unused value");
}
