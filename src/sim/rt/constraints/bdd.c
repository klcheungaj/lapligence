/* ---- reduced ordered BDDs with exact counting --------------------------
 *
 * The exact engine converts a component's AIG into an ROBDD (variable order:
 * bit positions most significant first, interleaving the component's
 * variables), counts its models exactly with multi-word integers and draws
 * one uniformly distributed model by unranking a uniform integer below the
 * model count (IEEE 1800-2009 18.5.9: every legal combination is equally
 * probable). Recursion depth is bounded by the level count, which the
 * LLG_RAND_BDD_BITS tunable bounds.
 */

typedef struct {
    uint32_t* level;
    uint32_t* lo;
    uint32_t* hi;
    size_t count, cap;
    uint32_t* table;
    size_t table_cap;    /* slots in use (the hash range) */
    size_t table_room;   /* slots allocated */
    uint64_t* cache_key;
    uint32_t* cache_val;
    size_t cache_cap;    /* entries in use (the hash range) */
    size_t cache_room;   /* entries allocated */
    uint32_t levels;
    uint64_t limit;
    uint64_t steps;      /* recursive apply steps, bounded by 32 * limit */
    int overflow;
} cr_bdd_t;

/* Initial table sizes. The computed table only ever grows with the unique
 * table, so these also fix every build's hit pattern and step count. */
#define CR_BDD_TABLE_INIT 2048u
#define CR_BDD_CACHE_INIT 16384u

/* The buffers of the last finished build, reused by the next one: an exact
 * rebuild on every call (state that changes per call) would otherwise
 * allocate and free about 200 KiB each time, which the C library may hand
 * back to the system and fault in again. Builds never overlap (the runtime
 * is single-threaded and no user code runs inside one), so one set serves
 * every session. Buffers above CR_BDD_POOL_KEEP_BYTES in total are freed
 * instead, so a large build (a budget overflow reaches tens of MiB) does not
 * pin its tables for the rest of the run. */
#define CR_BDD_POOL_KEEP_BYTES ((size_t)2u << 20)

typedef struct {
    uint32_t* level;
    uint32_t* lo;
    uint32_t* hi;
    size_t cap;
    uint32_t* table;
    size_t table_room;
    uint64_t* cache_key;
    uint32_t* cache_val;
    size_t cache_room;
} cr_bdd_pool_t;

static cr_bdd_pool_t cr_bdd_pool;

static void cr_bdd_pool_release(void) {
    free(cr_bdd_pool.level);
    free(cr_bdd_pool.lo);
    free(cr_bdd_pool.hi);
    free(cr_bdd_pool.table);
    free(cr_bdd_pool.cache_key);
    free(cr_bdd_pool.cache_val);
    memset(&cr_bdd_pool, 0, sizeof(cr_bdd_pool));
}

static uint32_t cr_bdd_hash(uint32_t l, uint32_t lo, uint32_t hi) {
    uint64_t h = ((uint64_t)lo << 32) ^ hi ^ ((uint64_t)l * UINT64_C(0x9e3779b97f4a7c15));
    h *= UINT64_C(0xbf58476d1ce4e5b9);
    return (uint32_t)(h >> 32);
}

/* An empty computed table of `entries` (a key of 0 is no operation). */
static void cr_bdd_cache_reset(cr_bdd_t* m, size_t entries) {
    if (m->cache_room < entries) {
        free(m->cache_key);
        free(m->cache_val);
        m->cache_key = (uint64_t*)cr_alloc(entries, sizeof(uint64_t));
        m->cache_val = (uint32_t*)cr_alloc(entries, sizeof(uint32_t));
        m->cache_room = entries;
    } else {
        memset(m->cache_key, 0, entries * sizeof(uint64_t));
    }
    m->cache_cap = entries;
}

static void cr_bdd_init(cr_bdd_t* m, uint32_t levels, uint64_t limit) {
    memset(m, 0, sizeof(*m));
    m->levels = levels;
    m->limit = limit < 2u ? 2u : limit;
    /* Take the pooled buffers; contents beyond what is reset are never
     * read (nodes at or above `count`, cache values without their key). */
    m->level = cr_bdd_pool.level;
    m->lo = cr_bdd_pool.lo;
    m->hi = cr_bdd_pool.hi;
    m->cap = cr_bdd_pool.cap;
    m->table = cr_bdd_pool.table;
    m->table_room = cr_bdd_pool.table_room;
    m->cache_key = cr_bdd_pool.cache_key;
    m->cache_val = cr_bdd_pool.cache_val;
    m->cache_room = cr_bdd_pool.cache_room;
    memset(&cr_bdd_pool, 0, sizeof(cr_bdd_pool));
    size_t c1 = m->cap, c2 = m->cap;
    m->level = (uint32_t*)cr_grow(m->level, &m->cap, 1024u, sizeof(uint32_t));
    m->lo = (uint32_t*)cr_grow(m->lo, &c1, m->cap, sizeof(uint32_t));
    m->hi = (uint32_t*)cr_grow(m->hi, &c2, m->cap, sizeof(uint32_t));
    m->level[0] = m->level[1] = levels;
    m->count = 2u;
    m->table_cap = CR_BDD_TABLE_INIT;
    if (m->table_room < m->table_cap) {
        free(m->table);
        m->table = (uint32_t*)cr_alloc(m->table_cap, sizeof(uint32_t));
        m->table_room = m->table_cap;
    } else {
        memset(m->table, 0, m->table_cap * sizeof(uint32_t));
    }
    cr_bdd_cache_reset(m, CR_BDD_CACHE_INIT);
}

/* Return the buffers to the pool, or free them when they are large. */
static void cr_bdd_free(cr_bdd_t* m) {
    size_t bytes = m->cap * 3u * sizeof(uint32_t) + m->table_room * sizeof(uint32_t) +
                   m->cache_room * (sizeof(uint64_t) + sizeof(uint32_t));
    if (bytes <= CR_BDD_POOL_KEEP_BYTES) {
        cr_bdd_pool_release();
        cr_bdd_pool.level = m->level;
        cr_bdd_pool.lo = m->lo;
        cr_bdd_pool.hi = m->hi;
        cr_bdd_pool.cap = m->cap;
        cr_bdd_pool.table = m->table;
        cr_bdd_pool.table_room = m->table_room;
        cr_bdd_pool.cache_key = m->cache_key;
        cr_bdd_pool.cache_val = m->cache_val;
        cr_bdd_pool.cache_room = m->cache_room;
    } else {
        free(m->level);
        free(m->lo);
        free(m->hi);
        free(m->table);
        free(m->cache_key);
        free(m->cache_val);
    }
    memset(m, 0, sizeof(*m));
}

static void cr_bdd_rehash(cr_bdd_t* m) {
    size_t cap = m->table_cap * 2u;
    uint32_t* table = (uint32_t*)cr_alloc(cap, sizeof(uint32_t));
    for (size_t n = 2; n < m->count; ++n) {
        size_t slot = cr_bdd_hash(m->level[n], m->lo[n], m->hi[n]) & (cap - 1u);
        while (table[slot]) slot = (slot + 1u) & (cap - 1u);
        table[slot] = (uint32_t)n;
    }
    free(m->table);
    m->table = table;
    m->table_cap = cap;
    m->table_room = cap;
    /* Keep the computed table proportional to the node count; growing it
     * empties it. */
    if (m->cache_cap < m->table_cap * 2u) cr_bdd_cache_reset(m, m->table_cap * 2u);
}

static uint32_t cr_bdd_mk(cr_bdd_t* m, uint32_t l, uint32_t lo, uint32_t hi) {
    if (lo == hi) return lo;
    if (m->overflow) return 0;
    size_t slot = cr_bdd_hash(l, lo, hi) & (m->table_cap - 1u);
    while (m->table[slot]) {
        uint32_t n = m->table[slot];
        if (m->level[n] == l && m->lo[n] == lo && m->hi[n] == hi) return n;
        slot = (slot + 1u) & (m->table_cap - 1u);
    }
    if ((uint64_t)m->count >= m->limit || m->count >= (size_t)(UINT32_MAX >> 2)) {
        m->overflow = 1;
        return 0;
    }
    size_t c1 = m->cap, c2 = m->cap;
    m->level = (uint32_t*)cr_grow(m->level, &m->cap, m->count + 1u, sizeof(uint32_t));
    m->lo = (uint32_t*)cr_grow(m->lo, &c1, m->cap, sizeof(uint32_t));
    m->hi = (uint32_t*)cr_grow(m->hi, &c2, m->cap, sizeof(uint32_t));
    uint32_t n = (uint32_t)m->count++;
    m->level[n] = l;
    m->lo[n] = lo;
    m->hi[n] = hi;
    m->table[slot] = n;
    if (m->count * 2u > m->table_cap) cr_bdd_rehash(m);
    return n;
}

static uint64_t cr_bdd_key(uint32_t op, uint32_t a, uint32_t b) {
    return ((uint64_t)op << 62) | ((uint64_t)a << 31) | (uint64_t)b;
}

static int cr_bdd_lookup(cr_bdd_t* m, uint64_t key, uint32_t* out) {
    size_t slot = (size_t)((key * UINT64_C(0x9e3779b97f4a7c15)) >> 20) & (m->cache_cap - 1u);
    if (m->cache_key[slot] == key) {
        *out = m->cache_val[slot];
        return 1;
    }
    return 0;
}

static void cr_bdd_store(cr_bdd_t* m, uint64_t key, uint32_t value) {
    size_t slot = (size_t)((key * UINT64_C(0x9e3779b97f4a7c15)) >> 20) & (m->cache_cap - 1u);
    m->cache_key[slot] = key;
    m->cache_val[slot] = value;
}

/* The node limit bounds memory; the step limit bounds time when the
 * computed table cannot hold the working set. Both report the same
 * tunable. */
static int cr_bdd_step(cr_bdd_t* m) {
    if (++m->steps > m->limit * 32u) m->overflow = 1;
    return !m->overflow;
}

static uint32_t cr_bdd_not(cr_bdd_t* m, uint32_t f) {
    if (f < 2u) return f ^ 1u;
    if (!cr_bdd_step(m)) return 0;
    uint64_t key = cr_bdd_key(2u, f, 0u);
    uint32_t r;
    if (cr_bdd_lookup(m, key, &r)) return r;
    uint32_t l = m->level[f], lo = m->lo[f], hi = m->hi[f];
    uint32_t nlo = cr_bdd_not(m, lo);
    uint32_t nhi = cr_bdd_not(m, hi);
    r = cr_bdd_mk(m, l, nlo, nhi);
    if (!m->overflow) cr_bdd_store(m, key, r);
    return r;
}

static uint32_t cr_bdd_and(cr_bdd_t* m, uint32_t f, uint32_t g) {
    if (f == 0u || g == 0u) return 0u;
    if (f == 1u) return g;
    if (g == 1u || f == g) return f;
    if (!cr_bdd_step(m)) return 0;
    if (f > g) { uint32_t t = f; f = g; g = t; }
    uint64_t key = cr_bdd_key(1u, f, g);
    uint32_t r;
    if (cr_bdd_lookup(m, key, &r)) return r;
    uint32_t lf = m->level[f], lg = m->level[g];
    uint32_t l = lf < lg ? lf : lg;
    uint32_t f0 = lf == l ? m->lo[f] : f, f1 = lf == l ? m->hi[f] : f;
    uint32_t g0 = lg == l ? m->lo[g] : g, g1 = lg == l ? m->hi[g] : g;
    uint32_t r0 = cr_bdd_and(m, f0, g0);
    uint32_t r1 = cr_bdd_and(m, f1, g1);
    r = cr_bdd_mk(m, l, r0, r1);
    if (!m->overflow) cr_bdd_store(m, key, r);
    return r;
}

static uint32_t cr_bdd_or(cr_bdd_t* m, uint32_t f, uint32_t g) {
    return cr_bdd_not(m, cr_bdd_and(m, cr_bdd_not(m, f), cr_bdd_not(m, g)));
}

/* Existential quantification of the levels marked in `quant`; `tag`
 * identifies that set in the computed table (one tag per layer). */
static uint32_t cr_bdd_exists(cr_bdd_t* m, uint32_t f, const uint8_t* quant, uint32_t tag) {
    if (f < 2u) return f;
    if (!cr_bdd_step(m)) return 0;
    uint64_t key = cr_bdd_key(3u, f, tag);
    uint32_t r;
    if (cr_bdd_lookup(m, key, &r)) return r;
    uint32_t l = m->level[f], lo = m->lo[f], hi = m->hi[f];
    uint32_t rlo = cr_bdd_exists(m, lo, quant, tag);
    uint32_t rhi = cr_bdd_exists(m, hi, quant, tag);
    r = quant[l] ? cr_bdd_or(m, rlo, rhi) : cr_bdd_mk(m, l, rlo, rhi);
    if (!m->overflow) cr_bdd_store(m, key, r);
    return r;
}

/* Convert the AIG cone of `out` (inputs 1..levels map to levels 0..levels-1). */
static uint32_t cr_bdd_from_aig(cr_bdd_t* m, const cr_aig_t* g, uint32_t out) {
    uint32_t root = out >> 1;
    uint32_t* bdd = (uint32_t*)cr_alloc(g->count, sizeof(uint32_t));
    uint8_t* need = (uint8_t*)cr_alloc(g->count, 1u);
    need[root] = 1;
    for (size_t n = g->count; n-- > (size_t)g->inputs + 1u;) {
        if (!need[n]) continue;
        need[g->left[n] >> 1] = 1;
        need[g->right[n] >> 1] = 1;
    }
    bdd[0] = 0;
    for (uint32_t i = 1; i <= g->inputs; ++i)
        if (need[i]) bdd[i] = cr_bdd_mk(m, i - 1u, 0u, 1u);
    for (size_t n = (size_t)g->inputs + 1u; n < g->count && !m->overflow; ++n) {
        if (!need[n]) continue;
        uint32_t a = bdd[g->left[n] >> 1], b = bdd[g->right[n] >> 1];
        if (g->left[n] & 1u) a = cr_bdd_not(m, a);
        if (g->right[n] & 1u) b = cr_bdd_not(m, b);
        bdd[n] = cr_bdd_and(m, a, b);
    }
    uint32_t result = bdd[root];
    if (out & 1u) result = cr_bdd_not(m, result);
    free(bdd);
    free(need);
    return m->overflow ? 0u : result;
}

/* ---- multi-word naturals for model counts ------------------------------ */

static void bn_shl(uint64_t* d, const uint64_t* a, uint32_t k, uint32_t limbs) {
    uint32_t ws = k / 64u, bs = k % 64u;
    for (uint32_t i = limbs; i-- > 0;) {
        uint64_t v = 0;
        if (i >= ws) {
            v = a[i - ws] << bs;
            if (bs && i > ws) v |= a[i - ws - 1u] >> (64u - bs);
        }
        d[i] = v;
    }
}

static void bn_shr(uint64_t* d, uint32_t k, uint32_t limbs) {
    uint32_t ws = k / 64u, bs = k % 64u;
    for (uint32_t i = 0; i < limbs; ++i) {
        uint64_t v = 0;
        if (i + ws < limbs) {
            v = d[i + ws] >> bs;
            if (bs && i + ws + 1u < limbs) v |= d[i + ws + 1u] << (64u - bs);
        }
        d[i] = v;
    }
}

static void bn_add(uint64_t* d, const uint64_t* a, uint32_t limbs) {
    uint64_t carry = 0;
    for (uint32_t i = 0; i < limbs; ++i) {
        uint64_t s = d[i] + a[i];
        uint64_t c1 = s < d[i];
        uint64_t t = s + carry;
        carry = c1 | (t < s);
        d[i] = t;
    }
}

static void bn_sub(uint64_t* d, const uint64_t* a, uint32_t limbs) {
    uint64_t borrow = 0;
    for (uint32_t i = 0; i < limbs; ++i) {
        uint64_t x = d[i], y = a[i];
        uint64_t t = x - y;
        uint64_t b1 = x < y;
        uint64_t r = t - borrow;
        borrow = b1 | (t < borrow);
        d[i] = r;
    }
}

static int bn_cmp(const uint64_t* a, const uint64_t* b, uint32_t limbs) {
    for (uint32_t i = limbs; i-- > 0;)
        if (a[i] != b[i]) return a[i] < b[i] ? -1 : 1;
    return 0;
}

static uint32_t bn_bits(const uint64_t* a, uint32_t limbs) {
    for (uint32_t i = limbs; i-- > 0;) {
        if (!a[i]) continue;
        uint32_t b = 64u;
        while (!((a[i] >> (b - 1u)) & 1u)) --b;
        return i * 64u + b;
    }
    return 0;
}

/* Uniform integer in [0, bound), bound > 0: rejection over 32-bit draws. */
static void bn_uniform(const llg_cr_rng_t* rng, uint64_t* out, const uint64_t* bound,
                       uint32_t limbs) {
    uint32_t bits = bn_bits(bound, limbs);
    for (;;) {
        for (uint32_t i = 0; i < limbs; ++i) out[i] = 0;
        for (uint32_t chunk = 0; chunk * 32u < bits; ++chunk) {
            uint64_t word = rng->next(rng->stream);
            out[chunk / 2u] |= word << (32u * (chunk % 2u));
        }
        if (bits % 64u) out[(bits - 1u) / 64u] &= (UINT64_C(1) << (bits % 64u)) - 1u;
        for (uint32_t i = (bits + 63u) / 64u; i < limbs; ++i) out[i] = 0;
        if (bn_cmp(out, bound, limbs) < 0) return;
    }
}

/* ---- compact exact model -------------------------------------------------
 *
 * Only the nodes reachable from the root survive; node 0/1 are terminals.
 * Children have larger levels, so nodes are stored deepest first.
 */

typedef struct {
    uint32_t nodes;
    uint32_t* level;
    uint32_t* lo;
    uint32_t* hi;
    uint64_t* count;      /* nodes * limbs (uniform counts of roots[0]) */
    uint32_t limbs;
    uint32_t root;
    uint32_t levels;
    uint32_t* roots;      /* every root kept (layer projections) */
    uint32_t root_count;
} cr_exact_t;

static void cr_exact_free(cr_exact_t* e) {
    free(e->level);
    free(e->lo);
    free(e->hi);
    free(e->count);
    free(e->roots);
    memset(e, 0, sizeof(*e));
}

/* Compact the nodes reachable from `roots` (roots[0] is the model; the
 * others are its layer projections) and count roots[0]'s models. */
static void cr_exact_build(cr_exact_t* e, const cr_bdd_t* m, const uint32_t* roots,
                           uint32_t root_count) {
    memset(e, 0, sizeof(*e));
    e->levels = m->levels;
    e->limbs = m->levels / 64u + 2u;
    uint32_t root = roots[0];
    /* Reachable nodes in post-order (children first). */
    uint32_t* map = (uint32_t*)cr_alloc(m->count, sizeof(uint32_t));
    for (size_t i = 0; i < m->count; ++i) map[i] = UINT32_MAX;
    map[0] = 0;
    map[1] = 1;
    cr_u32vec_t order = {0, 0, 0};
    cr_u32vec_t stack = {0, 0, 0};
    for (uint32_t i = root_count; i-- > 0;)
        if (roots[i] >= 2u) cr_u32_push(&stack, roots[i]);
    uint8_t* expanded = (uint8_t*)cr_alloc(m->count, 1u);
    while (stack.len) {
        uint32_t n = stack.data[stack.len - 1u];
        if (map[n] != UINT32_MAX) { stack.len--; continue; }
        if (!expanded[n]) {
            expanded[n] = 1;
            if (map[m->hi[n]] == UINT32_MAX) cr_u32_push(&stack, m->hi[n]);
            if (map[m->lo[n]] == UINT32_MAX) cr_u32_push(&stack, m->lo[n]);
            continue;
        }
        stack.len--;
        map[n] = (uint32_t)(order.len + 2u);
        cr_u32_push(&order, n);
    }
    free(expanded);
    free(stack.data);
    e->nodes = (uint32_t)order.len + 2u;
    e->level = (uint32_t*)cr_alloc(e->nodes, sizeof(uint32_t));
    e->lo = (uint32_t*)cr_alloc(e->nodes, sizeof(uint32_t));
    e->hi = (uint32_t*)cr_alloc(e->nodes, sizeof(uint32_t));
    e->count = (uint64_t*)cr_alloc((size_t)e->nodes * e->limbs, sizeof(uint64_t));
    e->level[0] = e->level[1] = e->levels;
    e->count[e->limbs] = 1u; /* terminal 1 */
    uint64_t* tmp = (uint64_t*)cr_alloc(e->limbs, sizeof(uint64_t));
    for (size_t i = 0; i < order.len; ++i) {
        uint32_t n = order.data[i], c = (uint32_t)i + 2u;
        e->level[c] = m->level[n];
        e->lo[c] = map[m->lo[n]];
        e->hi[c] = map[m->hi[n]];
        uint64_t* dst = e->count + (size_t)c * e->limbs;
        uint32_t kids[2] = {e->lo[c], e->hi[c]};
        for (int k = 0; k < 2; ++k) {
            uint32_t child = kids[k];
            bn_shl(tmp, e->count + (size_t)child * e->limbs,
                   e->level[child] - e->level[c] - 1u, e->limbs);
            bn_add(dst, tmp, e->limbs);
        }
    }
    e->root = map[root];
    e->root_count = root_count;
    e->roots = (uint32_t*)cr_alloc(root_count, sizeof(uint32_t));
    for (uint32_t i = 0; i < root_count; ++i) e->roots[i] = map[roots[i]];
    free(tmp);
    free(order.data);
    free(map);
}

/* Draw one model uniformly; bits[level] receives each level's value. `tmp`
 * holds 3 * e->limbs words of scratch. */
static void cr_exact_sample(const cr_exact_t* e, const llg_cr_rng_t* rng, uint8_t* bits,
                            uint64_t* tmp) {
    uint32_t limbs = e->limbs;
    uint64_t* total = tmp;
    uint64_t* r = tmp + limbs;
    uint64_t* w = tmp + 2u * (size_t)limbs;
    bn_shl(total, e->count + (size_t)e->root * limbs, e->level[e->root], limbs);
    bn_uniform(rng, r, total, limbs);
    /* Levels skipped above a node take their values from the low bits of r;
     * the remainder ranks the models below the node. */
    uint32_t level = 0;
    uint32_t n = e->root;
    for (;;) {
        for (; level < e->level[n]; ++level) {
            bits[level] = (uint8_t)(r[0] & 1u);
            bn_shr(r, 1u, limbs);
        }
        if (n < 2u) break;
        uint32_t lo = e->lo[n];
        bn_shl(w, e->count + (size_t)lo * limbs, e->level[lo] - e->level[n] - 1u, limbs);
        if (bn_cmp(r, w, limbs) < 0) {
            bits[level++] = 0;
            n = lo;
        } else {
            bn_sub(r, w, limbs);
            bits[level++] = 1;
            n = e->hi[n];
        }
    }
    if (n != 1u) cr_fatal("exact sampling reached the false terminal");
}
