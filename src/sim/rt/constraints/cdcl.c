/* ---- conflict-driven clause learning (search fallback) --------------------
 *
 * Runs only after the exact engine and rejection sampling have both given
 * up on a component. The component's AIG cone is Tseitin-encoded (three
 * clauses per AND node) and searched with two watched literals, first-UIP
 * learning with local minimization, integer VSIDS, phase saving and Luby
 * restarts. Learned clauses are reduced by literal block distance at
 * restarts, together with level-0 simplification.
 *
 * Randomness enters through the initial phases and decision order (drawn
 * from the stream) and the free input bits outside the cone, so repeated
 * calls vary; the result is NOT uniform over the solutions (S29-D10). A
 * finished search without a solution is a proof of unsatisfiability. The
 * conflict budget (LLG_RAND_CONFLICTS) bounds the search; 0 disables it.
 * Everything is integer arithmetic over 32-bit indices, so the search is
 * identical on every platform for the same stream.
 */

#define CR_SAT_NONE UINT32_MAX
#define CR_LV_FALSE 0u
#define CR_LV_TRUE 1u
#define CR_LV_UNDEF 2u
#define CR_ACT_LIMIT (UINT64_C(1) << 60)

typedef struct {
    uint32_t* d;  /* pairs: clause reference, blocker literal */
    uint32_t len, cap;
    int borrowed; /* d points into a shared block (cr_cnf_run): copy before growing */
} cr_watch_t;

typedef struct {
    uint32_t nv;
    uint8_t* assign;      /* CR_LV_* per variable */
    uint8_t* phase;       /* saved polarity (1 = true) */
    uint8_t* seen;
    uint32_t* level;
    uint32_t* reason;     /* clause reference or CR_SAT_NONE */
    uint32_t* trail;
    uint32_t trail_len;
    uint32_t qhead;
    cr_u32vec_t trail_lim;
    /* Clause arena: [size, flags, lits...]; flags bit 0 = learned,
     * bits 1.. = literal block distance. */
    cr_u32vec_t arena;
    cr_u32vec_t learned;  /* references of learned clauses */
    cr_watch_t* watches;  /* per literal */
    /* VSIDS heap (max activity first, ties by smaller index). */
    uint64_t* activity;
    uint64_t inc;
    uint32_t* heap;
    uint32_t* heap_pos;
    uint32_t heap_len;
    /* Scratch. */
    cr_u32vec_t learnt;
    cr_u32vec_t learnt_all;
    uint32_t* level_stamp;
    uint32_t stamp;
    uint64_t conflicts;
} cr_sat_t;

static uint32_t cr_sat_value(const cr_sat_t* s, uint32_t lit) {
    uint8_t a = s->assign[lit >> 1];
    if (a == CR_LV_UNDEF) return CR_LV_UNDEF;
    return (uint32_t)(a ^ (lit & 1u));
}

static uint32_t cr_sat_level_now(const cr_sat_t* s) {
    return (uint32_t)s->trail_lim.len;
}

static int cr_heap_before(const cr_sat_t* s, uint32_t a, uint32_t b) {
    return s->activity[a] > s->activity[b] ||
           (s->activity[a] == s->activity[b] && a < b);
}

static void cr_heap_up(cr_sat_t* s, uint32_t i) {
    uint32_t v = s->heap[i];
    while (i > 0) {
        uint32_t parent = (i - 1u) / 2u;
        if (!cr_heap_before(s, v, s->heap[parent])) break;
        s->heap[i] = s->heap[parent];
        s->heap_pos[s->heap[i]] = i;
        i = parent;
    }
    s->heap[i] = v;
    s->heap_pos[v] = i;
}

static void cr_heap_down(cr_sat_t* s, uint32_t i) {
    uint32_t v = s->heap[i];
    for (;;) {
        uint32_t child = 2u * i + 1u;
        if (child >= s->heap_len) break;
        if (child + 1u < s->heap_len && cr_heap_before(s, s->heap[child + 1u], s->heap[child]))
            ++child;
        if (!cr_heap_before(s, s->heap[child], v)) break;
        s->heap[i] = s->heap[child];
        s->heap_pos[s->heap[i]] = i;
        i = child;
    }
    s->heap[i] = v;
    s->heap_pos[v] = i;
}

static void cr_heap_insert(cr_sat_t* s, uint32_t v) {
    if (s->heap_pos[v] != CR_SAT_NONE) return;
    s->heap[s->heap_len] = v;
    s->heap_pos[v] = s->heap_len;
    cr_heap_up(s, s->heap_len++);
}

static uint32_t cr_heap_pop(cr_sat_t* s) {
    uint32_t v = s->heap[0];
    s->heap_pos[v] = CR_SAT_NONE;
    if (--s->heap_len) {
        s->heap[0] = s->heap[s->heap_len];
        s->heap_pos[s->heap[0]] = 0;
        cr_heap_down(s, 0);
    }
    return v;
}

static void cr_sat_bump(cr_sat_t* s, uint32_t v) {
    s->activity[v] += s->inc;
    if (s->activity[v] > CR_ACT_LIMIT) {
        /* Shifting every activity keeps the heap order (a monotone map). */
        for (uint32_t i = 0; i < s->nv; ++i) s->activity[i] >>= 32;
        s->inc >>= 32;
        if (!s->inc) s->inc = 1u;
    }
    if (s->heap_pos[v] != CR_SAT_NONE) cr_heap_up(s, s->heap_pos[v]);
}

static void cr_watch_push(cr_watch_t* w, uint32_t cref, uint32_t blocker) {
    if (w->len + 2u > w->cap) {
        size_t cap = w->cap;
        if (w->borrowed) {
            uint32_t* own = (uint32_t*)cr_alloc((size_t)w->len + 2u, sizeof(uint32_t));
            if (w->len) memcpy(own, w->d, w->len * sizeof(uint32_t));
            w->d = own;
            w->borrowed = 0;
            cap = (size_t)w->len + 2u;
        }
        w->d = (uint32_t*)cr_grow(w->d, &cap, (size_t)w->len + 2u, sizeof(uint32_t));
        if (cap > UINT32_MAX) cr_fatal("watch list too large");
        w->cap = (uint32_t)cap;
    }
    w->d[w->len++] = cref;
    w->d[w->len++] = blocker;
}

static uint32_t* cr_clause(cr_sat_t* s, uint32_t cref) {
    return s->arena.data + cref;
}

static void cr_sat_attach(cr_sat_t* s, uint32_t cref) {
    uint32_t* c = cr_clause(s, cref);
    cr_watch_push(&s->watches[c[2]], cref, c[3]);
    cr_watch_push(&s->watches[c[3]], cref, c[2]);
}

static uint32_t cr_sat_add(cr_sat_t* s, const uint32_t* lits, uint32_t size, int learned,
                           uint32_t lbd) {
    if (s->arena.len > (size_t)UINT32_MAX - size - 2u) cr_fatal("clause arena too large");
    uint32_t cref = (uint32_t)s->arena.len;
    cr_u32_push(&s->arena, size);
    cr_u32_push(&s->arena, (learned ? 1u : 0u) | (lbd << 1));
    for (uint32_t i = 0; i < size; ++i) cr_u32_push(&s->arena, lits[i]);
    if (learned) cr_u32_push(&s->learned, cref);
    cr_sat_attach(s, cref);
    return cref;
}

static void cr_sat_enqueue(cr_sat_t* s, uint32_t lit, uint32_t reason) {
    uint32_t v = lit >> 1;
    s->assign[v] = (uint8_t)((lit & 1u) ? CR_LV_FALSE : CR_LV_TRUE);
    s->level[v] = cr_sat_level_now(s);
    s->reason[v] = reason;
    s->trail[s->trail_len++] = lit;
}

/* Unit propagation; returns the conflicting clause or CR_SAT_NONE. */
static uint32_t cr_sat_propagate(cr_sat_t* s) {
    while (s->qhead < s->trail_len) {
        uint32_t false_lit = s->trail[s->qhead++] ^ 1u;
        cr_watch_t* w = &s->watches[false_lit];
        uint32_t i = 0, j = 0;
        while (i < w->len) {
            uint32_t cref = w->d[i], blocker = w->d[i + 1u];
            i += 2u;
            if (cr_sat_value(s, blocker) == CR_LV_TRUE) {
                w->d[j++] = cref;
                w->d[j++] = blocker;
                continue;
            }
            uint32_t* c = cr_clause(s, cref);
            uint32_t* lits = c + 2;
            if (lits[0] == false_lit) {
                lits[0] = lits[1];
                lits[1] = false_lit;
            }
            uint32_t first = lits[0];
            if (first != blocker && cr_sat_value(s, first) == CR_LV_TRUE) {
                w->d[j++] = cref;
                w->d[j++] = first;
                continue;
            }
            int moved = 0;
            for (uint32_t k = 2; k < c[0]; ++k) {
                if (cr_sat_value(s, lits[k]) != CR_LV_FALSE) {
                    lits[1] = lits[k];
                    lits[k] = false_lit;
                    cr_watch_push(&s->watches[lits[1]], cref, first);
                    moved = 1;
                    break;
                }
            }
            if (moved) continue;
            w->d[j++] = cref;
            w->d[j++] = first;
            if (cr_sat_value(s, first) == CR_LV_FALSE) {
                while (i < w->len) w->d[j++] = w->d[i++];
                w->len = j;
                s->qhead = s->trail_len;
                return cref;
            }
            cr_sat_enqueue(s, first, cref);
        }
        w->len = j;
    }
    return CR_SAT_NONE;
}

static void cr_sat_cancel(cr_sat_t* s, uint32_t target) {
    if (cr_sat_level_now(s) <= target) return;
    uint32_t keep = s->trail_lim.data[target];
    for (uint32_t i = s->trail_len; i-- > keep;) {
        uint32_t v = s->trail[i] >> 1;
        s->phase[v] = s->assign[v] == CR_LV_TRUE;
        s->assign[v] = CR_LV_UNDEF;
        s->reason[v] = CR_SAT_NONE;
        cr_heap_insert(s, v);
    }
    s->trail_len = keep;
    s->qhead = keep;
    s->trail_lim.len = target;
}

/* First-UIP analysis of `conflict`; leaves the learned clause in s->learnt
 * (asserting literal first, a literal of the backjump level second) and
 * returns the backjump level. */
static uint32_t cr_sat_analyze(cr_sat_t* s, uint32_t conflict, uint32_t* lbd) {
    s->learnt.len = 0;
    s->learnt_all.len = 0;
    cr_u32_push(&s->learnt, 0u);
    uint32_t now = cr_sat_level_now(s);
    uint32_t pending = 0, lit = CR_SAT_NONE;
    uint32_t index = s->trail_len;
    uint32_t cref = conflict;
    do {
        uint32_t* c = cr_clause(s, cref);
        for (uint32_t k = lit == CR_SAT_NONE ? 0u : 1u; k < c[0]; ++k) {
            uint32_t q = c[2 + k];
            uint32_t v = q >> 1;
            if (s->seen[v] || s->level[v] == 0u) continue;
            s->seen[v] = 1;
            cr_sat_bump(s, v);
            if (s->level[v] >= now) ++pending;
            else {
                cr_u32_push(&s->learnt, q);
                cr_u32_push(&s->learnt_all, v);
            }
        }
        while (!s->seen[s->trail[index - 1u] >> 1]) --index;
        lit = s->trail[--index];
        cref = s->reason[lit >> 1];
        s->seen[lit >> 1] = 0;
        --pending;
    } while (pending > 0);
    s->learnt.data[0] = lit ^ 1u;

    /* Local minimization: drop a literal implied by other learned ones. */
    uint32_t out = 1;
    for (uint32_t i = 1; i < s->learnt.len; ++i) {
        uint32_t q = s->learnt.data[i];
        uint32_t r = s->reason[q >> 1];
        int keep = r == CR_SAT_NONE;
        if (!keep) {
            uint32_t* c = cr_clause(s, r);
            for (uint32_t k = 1; k < c[0]; ++k) {
                uint32_t v = c[2 + k] >> 1;
                if (!s->seen[v] && s->level[v] > 0u) { keep = 1; break; }
            }
        }
        if (keep) s->learnt.data[out++] = q;
    }
    s->learnt.len = out;
    for (size_t i = 0; i < s->learnt_all.len; ++i) s->seen[s->learnt_all.data[i]] = 0;

    uint32_t back = 0;
    if (s->learnt.len > 1u) {
        uint32_t best = 1;
        for (uint32_t i = 2; i < s->learnt.len; ++i)
            if (s->level[s->learnt.data[i] >> 1] > s->level[s->learnt.data[best] >> 1]) best = i;
        uint32_t t = s->learnt.data[1];
        s->learnt.data[1] = s->learnt.data[best];
        s->learnt.data[best] = t;
        back = s->level[s->learnt.data[1] >> 1];
    }
    if (++s->stamp == 0u) {
        memset(s->level_stamp, 0, ((size_t)s->nv + 1u) * sizeof(uint32_t));
        s->stamp = 1;
    }
    uint32_t distinct = 0;
    for (uint32_t i = 0; i < s->learnt.len; ++i) {
        uint32_t l = s->level[s->learnt.data[i] >> 1];
        if (s->level_stamp[l] != s->stamp) {
            s->level_stamp[l] = s->stamp;
            ++distinct;
        }
    }
    *lbd = distinct;
    return back;
}

/* At level 0 after full propagation: drop the worse half of the learned
 * clauses (by literal block distance, keeping distance <= 2), drop clauses
 * satisfied at level 0, strip false literals and rebuild the watches. */
static void cr_sat_reduce(cr_sat_t* s) {
    uint32_t hist[33];
    memset(hist, 0, sizeof(hist));
    for (size_t i = 0; i < s->learned.len; ++i) {
        uint32_t lbd = cr_clause(s, s->learned.data[i])[1] >> 1;
        hist[lbd > 32u ? 32u : lbd]++;
    }
    /* Smallest threshold above which at most half of the clauses lie. */
    uint32_t threshold = 32u, above = 0;
    while (threshold > 2u && above + hist[threshold] <= s->learned.len / 2u) {
        above += hist[threshold];
        --threshold;
    }
    cr_u32vec_t arena = {0, 0, 0};
    s->learned.len = 0;
    size_t at = 0;
    while (at < s->arena.len) {
        uint32_t* c = s->arena.data + at;
        uint32_t size = c[0], flags = c[1];
        at += 2u + size;
        uint32_t lbd = flags >> 1;
        if ((flags & 1u) && (lbd > 32u ? 32u : lbd) > threshold) continue;
        int satisfied = 0;
        uint32_t kept = 0;
        for (uint32_t k = 0; k < size; ++k) {
            uint32_t value = cr_sat_value(s, c[2 + k]);
            if (value == CR_LV_TRUE) { satisfied = 1; break; }
            if (value == CR_LV_UNDEF) ++kept;
        }
        if (satisfied) continue;
        if (kept < 2u) cr_fatal("CDCL reduction without full propagation");
        uint32_t cref = (uint32_t)arena.len;
        cr_u32_push(&arena, kept);
        cr_u32_push(&arena, flags);
        for (uint32_t k = 0; k < size; ++k)
            if (cr_sat_value(s, c[2 + k]) == CR_LV_UNDEF) cr_u32_push(&arena, c[2 + k]);
        if (flags & 1u) cr_u32_push(&s->learned, cref);
    }
    free(s->arena.data);
    s->arena = arena;
    for (uint32_t l = 0; l < 2u * s->nv; ++l) s->watches[l].len = 0;
    for (uint32_t i = 0; i < s->trail_len; ++i) s->reason[s->trail[i] >> 1] = CR_SAT_NONE;
    at = 0;
    while (at < s->arena.len) {
        cr_sat_attach(s, (uint32_t)at);
        at += 2u + s->arena.data[at];
    }
}

/* Luby sequence 1 1 2 1 1 2 4 ... (index from 0). */
static uint64_t cr_luby(uint64_t i) {
    uint64_t size = 1, seq = 0;
    while (size < i + 1u) {
        ++seq;
        size = 2u * size + 1u;
    }
    while (size - 1u != i) {
        size = (size - 1u) >> 1;
        --seq;
        i = i % size;
    }
    return UINT64_C(1) << seq;
}

static void cr_sat_free(cr_sat_t* s) {
    if (s->watches)
        for (uint32_t l = 0; l < 2u * s->nv; ++l)
            if (!s->watches[l].borrowed) free(s->watches[l].d);
    free(s->watches);
    free(s->assign); free(s->phase); free(s->seen); free(s->level); free(s->reason);
    free(s->trail); free(s->trail_lim.data); free(s->arena.data); free(s->learned.data);
    free(s->activity); free(s->heap); free(s->heap_pos); free(s->learnt.data);
    free(s->learnt_all.data); free(s->level_stamp);
    memset(s, 0, sizeof(*s));
}

/* A component's search problem, encoded once: the AIG cone of the root
 * numbered depth first from the root and Tseitin-encoded (three clauses per
 * AND node), with the watch lists as the encoding leaves them. A run copies
 * it, so a cached encoding searches exactly like a fresh one. */
typedef struct {
    uint32_t nv;
    uint32_t levels;      /* AIG inputs 1..levels: variable bits */
    uint32_t* input_var;  /* per input: SAT variable or CR_SAT_NONE */
    uint8_t* is_input;    /* per SAT variable: an AIG input (random priority) */
    uint32_t root_lit;    /* SAT literal that must hold, or CR_SAT_NONE (root true) */
    int root_false;
    cr_u32vec_t arena;
    cr_watch_t* watches;  /* per literal, as the encoding leaves them */
    size_t watch_words;
} cr_cnf_t;

static void cr_cnf_free(cr_cnf_t* f) {
    free(f->input_var);
    free(f->is_input);
    free(f->arena.data);
    if (f->watches)
        for (uint32_t l = 0; l < 2u * f->nv; ++l) free(f->watches[l].d);
    free(f->watches);
    memset(f, 0, sizeof(*f));
}

/* Words a cached encoding holds (its memory bound). */
static size_t cr_cnf_words(const cr_cnf_t* f) {
    return f->arena.len + f->watch_words + 4u * (size_t)f->nv;
}

static void cr_cnf_build(cr_cnf_t* f, const cr_aig_t* g, uint32_t root) {
    memset(f, 0, sizeof(*f));
    f->levels = g->inputs;
    f->root_lit = CR_SAT_NONE;
    if (root == CR_FALSE) {
        f->root_false = 1;
        return;
    }
    /* Number the cone: AIG node -> SAT variable. */
    uint32_t* var_of = (uint32_t*)cr_alloc(g->count, sizeof(uint32_t));
    for (size_t n = 0; n < g->count; ++n) var_of[n] = CR_SAT_NONE;
    cr_u32vec_t stack = {0, 0, 0}, cone = {0, 0, 0};
    if (root >> 1) cr_u32_push(&stack, root >> 1);
    while (stack.len) {
        uint32_t n = stack.data[--stack.len];
        if (var_of[n] != CR_SAT_NONE) continue;
        var_of[n] = (uint32_t)cone.len;
        cr_u32_push(&cone, n);
        if (n > g->inputs) {
            cr_u32_push(&stack, g->left[n] >> 1);
            cr_u32_push(&stack, g->right[n] >> 1);
        }
    }
    free(stack.data);
    f->nv = (uint32_t)cone.len;
    uint32_t nv = f->nv ? f->nv : 1u;
    f->is_input = (uint8_t*)cr_alloc(nv, 1u);
    for (uint32_t v = 0; v < f->nv; ++v) f->is_input[v] = cone.data[v] <= g->inputs;
    f->input_var = (uint32_t*)cr_alloc(g->inputs ? g->inputs : 1u, sizeof(uint32_t));
    for (uint32_t i = 0; i < g->inputs; ++i) f->input_var[i] = var_of[i + 1u];

    /* Tseitin clauses: n <-> (l & r), watched in a scratch solver. */
    cr_sat_t s;
    memset(&s, 0, sizeof(s));
    s.nv = f->nv;
    s.watches = (cr_watch_t*)cr_alloc(2u * (size_t)nv, sizeof(cr_watch_t));
    for (uint32_t v = 0; v < f->nv; ++v) {
        uint32_t n = cone.data[v];
        if (n <= g->inputs) continue;
        uint32_t out = 2u * v;
        uint32_t l = 2u * var_of[g->left[n] >> 1] + (g->left[n] & 1u);
        uint32_t r = 2u * var_of[g->right[n] >> 1] + (g->right[n] & 1u);
        uint32_t c1[2] = {out ^ 1u, l}, c2[2] = {out ^ 1u, r}, c3[3] = {out, l ^ 1u, r ^ 1u};
        cr_sat_add(&s, c1, 2u, 0, 0u);
        cr_sat_add(&s, c2, 2u, 0, 0u);
        cr_sat_add(&s, c3, 3u, 0, 0u);
    }
    if (root != CR_TRUE) f->root_lit = 2u * var_of[root >> 1] + (root & 1u);
    f->arena = s.arena;
    f->watches = s.watches;
    for (uint32_t l = 0; l < 2u * f->nv; ++l) f->watch_words += f->watches[l].len;
    s.arena.data = NULL;
    s.watches = NULL;
    cr_sat_free(&s);
    free(cone.data);
    free(var_of);
}

/* Search for an input assignment that makes the encoded root true. On
 * LLG_CR_OK, `bits[i]` holds input i + 1 (BDD level order); inputs outside
 * the cone are drawn uniformly. `budget` must be nonzero. A cached encoding
 * is copied; with `consume` the search takes its clauses and watch lists
 * over instead (the caller frees what is left). */
static llg_cr_status_t cr_cnf_run(cr_cnf_t* f, int consume, const llg_cr_rng_t* rng,
                                  uint64_t budget, uint8_t* bits, uint64_t* conflicts) {
    *conflicts = 0;
    if (f->root_false) return LLG_CR_UNSAT;
    cr_sat_t s;
    memset(&s, 0, sizeof(s));
    s.nv = f->nv;
    uint32_t nv = s.nv ? s.nv : 1u;
    s.assign = (uint8_t*)cr_alloc(nv, 1u);
    s.phase = (uint8_t*)cr_alloc(nv, 1u);
    s.seen = (uint8_t*)cr_alloc(nv, 1u);
    s.level = (uint32_t*)cr_alloc(nv, sizeof(uint32_t));
    s.reason = (uint32_t*)cr_alloc(nv, sizeof(uint32_t));
    s.trail = (uint32_t*)cr_alloc(nv, sizeof(uint32_t));
    s.watches = (cr_watch_t*)cr_alloc(2u * (size_t)nv, sizeof(cr_watch_t));
    s.activity = (uint64_t*)cr_alloc(nv, sizeof(uint64_t));
    s.heap = (uint32_t*)cr_alloc(nv, sizeof(uint32_t));
    s.heap_pos = (uint32_t*)cr_alloc(nv, sizeof(uint32_t));
    s.level_stamp = (uint32_t*)cr_alloc((size_t)nv + 1u, sizeof(uint32_t));
    s.inc = UINT64_C(1) << 20;
    /* The clauses and watch lists as encoded; the lists borrow one copied
     * block until they outgrow their encoded length. */
    uint32_t* watch_block = NULL;
    if (consume) {
        s.arena = f->arena;
        f->arena.data = NULL;
        f->arena.len = f->arena.cap = 0;
        free(s.watches);
        s.watches = f->watches;
        f->watches = NULL;
        if (!s.watches) s.watches = (cr_watch_t*)cr_alloc(2u * (size_t)nv, sizeof(cr_watch_t));
    } else {
        s.arena.data = (uint32_t*)cr_alloc(f->arena.len ? f->arena.len : 1u, sizeof(uint32_t));
        if (f->arena.len) memcpy(s.arena.data, f->arena.data, f->arena.len * sizeof(uint32_t));
        s.arena.len = s.arena.cap = f->arena.len;
        watch_block = (uint32_t*)cr_alloc(f->watch_words ? f->watch_words : 1u, sizeof(uint32_t));
        size_t at = 0;
        for (uint32_t l = 0; l < 2u * s.nv; ++l) {
            const cr_watch_t* w = &f->watches[l];
            if (w->len) memcpy(watch_block + at, w->d, w->len * sizeof(uint32_t));
            s.watches[l].d = watch_block + at;
            s.watches[l].len = s.watches[l].cap = w->len;
            s.watches[l].borrowed = 1;
            at += w->len;
        }
    }
    /* Random phases for every variable; random initial priorities for the
     * inputs, so the first descent is a random input assignment. */
    uint32_t draw = 0;
    for (uint32_t v = 0; v < s.nv; ++v) {
        if (v % 32u == 0u) draw = rng->next(rng->stream);
        s.assign[v] = CR_LV_UNDEF;
        s.phase[v] = (uint8_t)((draw >> (v % 32u)) & 1u);
        s.reason[v] = CR_SAT_NONE;
        s.heap_pos[v] = CR_SAT_NONE;
    }
    for (uint32_t v = 0; v < s.nv; ++v)
        if (f->is_input[v]) s.activity[v] = (rng->next(rng->stream) & 1023u) + 1u;
    for (uint32_t v = 0; v < s.nv; ++v) cr_heap_insert(&s, v);

    llg_cr_status_t result = LLG_CR_RESOURCE;
    if (f->root_lit != CR_SAT_NONE) cr_sat_enqueue(&s, f->root_lit, CR_SAT_NONE);

    uint64_t restarts = 0, since_restart = 0;
    uint64_t max_learned = 2000u + s.nv / 4u;
    for (;;) {
        uint32_t conflict = cr_sat_propagate(&s);
        if (conflict != CR_SAT_NONE) {
            ++s.conflicts;
            ++since_restart;
            if (cr_sat_level_now(&s) == 0u) {
                result = LLG_CR_UNSAT;
                break;
            }
            uint32_t lbd = 0;
            uint32_t back = cr_sat_analyze(&s, conflict, &lbd);
            cr_sat_cancel(&s, back);
            if (s.learnt.len == 1u) {
                cr_sat_enqueue(&s, s.learnt.data[0], CR_SAT_NONE);
            } else {
                uint32_t cref = cr_sat_add(&s, s.learnt.data, (uint32_t)s.learnt.len, 1, lbd);
                cr_sat_enqueue(&s, s.learnt.data[0], cref);
            }
            /* Decay: later bumps weigh about 1/0.94 more. */
            s.inc += s.inc >> 4;
            if (s.inc > CR_ACT_LIMIT) {
                for (uint32_t i = 0; i < s.nv; ++i) s.activity[i] >>= 32;
                s.inc >>= 32;
                if (!s.inc) s.inc = 1u;
            }
            if (s.conflicts >= budget) break;
            continue;
        }
        if (since_restart >= 64u * cr_luby(restarts)) {
            ++restarts;
            since_restart = 0;
            cr_sat_cancel(&s, 0u);
            if (s.learned.len >= max_learned) {
                cr_sat_reduce(&s);
                max_learned += max_learned / 10u;
            }
            continue;
        }
        uint32_t next = CR_SAT_NONE;
        while (s.heap_len) {
            uint32_t v = cr_heap_pop(&s);
            if (s.assign[v] == CR_LV_UNDEF) { next = v; break; }
        }
        if (next == CR_SAT_NONE) {
            result = LLG_CR_OK;
            break;
        }
        cr_u32_push(&s.trail_lim, s.trail_len);
        cr_sat_enqueue(&s, 2u * next + (s.phase[next] ? 0u : 1u), CR_SAT_NONE);
    }

    if (result == LLG_CR_OK) {
        for (uint32_t i = 0; i < f->levels; ++i) {
            uint32_t v = f->input_var[i];
            bits[i] = v != CR_SAT_NONE ? (uint8_t)(s.assign[v] == CR_LV_TRUE)
                                       : (uint8_t)(rng->next(rng->stream) & 1u);
        }
    }
    *conflicts = s.conflicts;
    cr_sat_free(&s);
    free(watch_block);
    return result;
}

/* Search for an input assignment that makes AIG literal `root` true.
 * On LLG_CR_OK, `bits[i]` holds input i + 1 (BDD level order); inputs
 * outside the cone are drawn uniformly. `budget` must be nonzero. */
static llg_cr_status_t cr_cdcl_solve(const cr_aig_t* g, uint32_t root, const llg_cr_rng_t* rng,
                                     uint64_t budget, uint8_t* bits, uint64_t* conflicts) {
    cr_cnf_t f;
    cr_cnf_build(&f, g, root);
    llg_cr_status_t result = cr_cnf_run(&f, 1, rng, budget, bits, conflicts);
    cr_cnf_free(&f);
    return result;
}
