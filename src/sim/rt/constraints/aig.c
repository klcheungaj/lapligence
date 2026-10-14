/* ---- and-inverter graph and bit-blasting --------------------------------
 *
 * A component's residual constraints are bit-blasted once into a
 * structurally hashed AIG. The exact engine (BDD) and the search fallback
 * (CDCL) both start from it; the word-level evaluator above stays the
 * independent reference that verifies their solutions.
 *
 * Literals are (node << 1) | negated. Node 0 is constant false, nodes
 * 1..inputs are the component's variable bits in BDD level order.
 */

#define CR_FALSE 0u
#define CR_TRUE 1u

typedef struct {
    uint32_t* left;
    uint32_t* right;
    size_t count, cap;
    uint32_t* table;     /* open addressing, node index or 0 */
    size_t table_cap;
    uint32_t inputs;
    uint64_t limit;
    int overflow;
} cr_aig_t;

static uint32_t cr_aig_hash(uint32_t a, uint32_t b) {
    uint64_t h = ((uint64_t)a << 32) ^ b;
    h *= UINT64_C(0x9e3779b97f4a7c15);
    return (uint32_t)(h >> 32);
}

static void cr_aig_init(cr_aig_t* g, uint32_t inputs, uint64_t limit) {
    memset(g, 0, sizeof(*g));
    g->inputs = inputs;
    g->limit = limit;
    size_t need = (size_t)inputs + 1u;
    g->left = (uint32_t*)cr_grow(NULL, &g->cap, need, sizeof(uint32_t));
    size_t cap2 = 0;
    g->right = (uint32_t*)cr_grow(NULL, &cap2, g->cap, sizeof(uint32_t));
    g->count = need;
    g->table_cap = 1024u;
    g->table = (uint32_t*)cr_alloc(g->table_cap, sizeof(uint32_t));
}

static void cr_aig_free(cr_aig_t* g) {
    free(g->left);
    free(g->right);
    free(g->table);
    memset(g, 0, sizeof(*g));
}

static void cr_aig_rehash(cr_aig_t* g) {
    size_t cap = g->table_cap * 2u;
    uint32_t* table = (uint32_t*)cr_alloc(cap, sizeof(uint32_t));
    for (size_t n = (size_t)g->inputs + 1u; n < g->count; ++n) {
        size_t slot = cr_aig_hash(g->left[n], g->right[n]) & (cap - 1u);
        while (table[slot]) slot = (slot + 1u) & (cap - 1u);
        table[slot] = (uint32_t)n;
    }
    free(g->table);
    g->table = table;
    g->table_cap = cap;
}

static uint32_t cr_and(cr_aig_t* g, uint32_t a, uint32_t b) {
    if (a > b) { uint32_t t = a; a = b; b = t; }
    if (a == CR_FALSE) return CR_FALSE;
    if (a == CR_TRUE) return b;
    if (a == b) return a;
    if ((a ^ 1u) == b) return CR_FALSE;
    if (g->overflow) return CR_FALSE;
    size_t slot = cr_aig_hash(a, b) & (g->table_cap - 1u);
    while (g->table[slot]) {
        uint32_t n = g->table[slot];
        if (g->left[n] == a && g->right[n] == b) return n << 1;
        slot = (slot + 1u) & (g->table_cap - 1u);
    }
    if ((uint64_t)g->count >= g->limit || g->count >= (size_t)(UINT32_MAX >> 2)) {
        g->overflow = 1;
        return CR_FALSE;
    }
    size_t cap = g->cap;
    g->left = (uint32_t*)cr_grow(g->left, &g->cap, g->count + 1u, sizeof(uint32_t));
    g->right = (uint32_t*)cr_grow(g->right, &cap, g->cap, sizeof(uint32_t));
    uint32_t n = (uint32_t)g->count++;
    g->left[n] = a;
    g->right[n] = b;
    g->table[slot] = n;
    if (g->count * 2u > g->table_cap) cr_aig_rehash(g);
    return n << 1;
}

static uint32_t cr_or(cr_aig_t* g, uint32_t a, uint32_t b) {
    return cr_and(g, a ^ 1u, b ^ 1u) ^ 1u;
}

static uint32_t cr_xor(cr_aig_t* g, uint32_t a, uint32_t b) {
    return cr_or(g, cr_and(g, a, b ^ 1u), cr_and(g, a ^ 1u, b));
}

static uint32_t cr_mux(cr_aig_t* g, uint32_t c, uint32_t t, uint32_t f) {
    if (t == f) return t;
    return cr_or(g, cr_and(g, c, t), cr_and(g, c ^ 1u, f));
}

/* ---- word-level blasting helpers ---------------------------------------- */

static uint32_t* cr_lits(uint32_t width) {
    return (uint32_t*)cr_alloc(width ? width : 1u, sizeof(uint32_t));
}

static uint32_t cr_blast_any(cr_aig_t* g, const uint32_t* a, uint32_t w) {
    uint32_t r = CR_FALSE;
    for (uint32_t i = 0; i < w; ++i) r = cr_or(g, r, a[i]);
    return r;
}

static void cr_blast_extend(uint32_t* out, uint32_t w, const uint32_t* a,
                            uint32_t aw, int sgn) {
    for (uint32_t i = 0; i < w; ++i)
        out[i] = i < aw ? a[i] : (sgn && aw ? a[aw - 1u] : CR_FALSE);
}

static void cr_blast_add(cr_aig_t* g, uint32_t* out, const uint32_t* a,
                         const uint32_t* b, uint32_t w, uint32_t carry) {
    for (uint32_t i = 0; i < w; ++i) {
        uint32_t x = cr_xor(g, a[i], b[i]);
        uint32_t sum = cr_xor(g, x, carry);
        carry = cr_or(g, cr_and(g, a[i], b[i]), cr_and(g, x, carry));
        out[i] = sum;
    }
}

static void cr_blast_sub(cr_aig_t* g, uint32_t* out, const uint32_t* a,
                         const uint32_t* b, uint32_t w) {
    uint32_t* nb = cr_lits(w);
    for (uint32_t i = 0; i < w; ++i) nb[i] = b[i] ^ 1u;
    cr_blast_add(g, out, a, nb, w, CR_TRUE);
    free(nb);
}

static void cr_blast_neg(cr_aig_t* g, uint32_t* out, const uint32_t* a, uint32_t w) {
    uint32_t* zero = cr_lits(w);
    for (uint32_t i = 0; i < w; ++i) zero[i] = CR_FALSE;
    cr_blast_sub(g, out, zero, a, w);
    free(zero);
}

/* a < b, unsigned. */
static uint32_t cr_blast_ult(cr_aig_t* g, const uint32_t* a, const uint32_t* b,
                             uint32_t w) {
    uint32_t lt = CR_FALSE;
    for (uint32_t i = 0; i < w; ++i) {
        uint32_t here = cr_and(g, a[i] ^ 1u, b[i]);
        uint32_t same = cr_xor(g, a[i], b[i]) ^ 1u;
        lt = cr_or(g, here, cr_and(g, same, lt));
    }
    return lt;
}

static uint32_t cr_blast_eq(cr_aig_t* g, const uint32_t* a, const uint32_t* b,
                            uint32_t w) {
    uint32_t eq = CR_TRUE;
    for (uint32_t i = 0; i < w; ++i) eq = cr_and(g, eq, cr_xor(g, a[i], b[i]) ^ 1u);
    return eq;
}

static void cr_blast_mul(cr_aig_t* g, uint32_t* out, const uint32_t* a,
                         const uint32_t* b, uint32_t w) {
    uint32_t* acc = cr_lits(w);
    uint32_t* part = cr_lits(w);
    for (uint32_t i = 0; i < w; ++i) acc[i] = CR_FALSE;
    for (uint32_t i = 0; i < w && !g->overflow; ++i) {
        if (b[i] == CR_FALSE) continue;
        for (uint32_t j = 0; j < w; ++j) part[j] = j < i ? CR_FALSE : cr_and(g, a[j - i], b[i]);
        cr_blast_add(g, acc, acc, part, w, CR_FALSE);
    }
    memcpy(out, acc, w * sizeof(uint32_t));
    free(acc);
    free(part);
}

/* Restoring division of unsigned a by b (b known nonzero by classification). */
static void cr_blast_udivmod(cr_aig_t* g, uint32_t* q, uint32_t* r,
                             const uint32_t* a, const uint32_t* b, uint32_t w) {
    uint32_t* rem = cr_lits(w + 1u);
    uint32_t* div = cr_lits(w + 1u);
    uint32_t* diff = cr_lits(w + 1u);
    for (uint32_t i = 0; i <= w; ++i) {
        rem[i] = CR_FALSE;
        div[i] = i < w ? b[i] : CR_FALSE;
    }
    for (uint32_t bit = w; bit-- > 0 && !g->overflow;) {
        for (uint32_t i = w; i > 0; --i) rem[i] = rem[i - 1u];
        rem[0] = a[bit];
        cr_blast_sub(g, diff, rem, div, w + 1u);
        uint32_t ge = cr_blast_ult(g, rem, div, w + 1u) ^ 1u;
        q[bit] = ge;
        for (uint32_t i = 0; i <= w; ++i) rem[i] = cr_mux(g, ge, diff[i], rem[i]);
    }
    for (uint32_t i = 0; i < w; ++i) r[i] = rem[i];
    free(rem);
    free(div);
    free(diff);
}

static void cr_blast_shift(cr_aig_t* g, uint32_t* out, const uint32_t* a, uint32_t w,
                           const uint32_t* amount, uint32_t aw, int left, uint32_t fill) {
    uint32_t* cur = cr_lits(w);
    uint32_t* next = cr_lits(w);
    memcpy(cur, a, w * sizeof(uint32_t));
    uint32_t over = CR_FALSE;
    for (uint32_t k = 0; k < aw; ++k) {
        uint64_t step = k < 63u ? (UINT64_C(1) << k) : UINT64_MAX;
        if (step >= w) {
            over = cr_or(g, over, amount[k]);
            continue;
        }
        for (uint32_t i = 0; i < w; ++i) {
            uint32_t moved;
            if (left) moved = i >= step ? cur[i - (uint32_t)step] : CR_FALSE;
            else moved = (uint64_t)i + step < w ? cur[i + (uint32_t)step] : fill;
            next[i] = cr_mux(g, amount[k], moved, cur[i]);
        }
        memcpy(cur, next, w * sizeof(uint32_t));
    }
    for (uint32_t i = 0; i < w; ++i) out[i] = cr_mux(g, over, left ? CR_FALSE : fill, cur[i]);
    free(cur);
    free(next);
}

/* ---- blasting a component ----------------------------------------------- */

typedef struct {
    cr_aig_t aig;
    uint32_t* lit_off;      /* per node offset into lits, UINT32_MAX if none */
    cr_u32vec_t lits;
} cr_blast_t;

static const uint32_t* cr_node_lits(llg_cr_session_t* s, cr_blast_t* b, uint32_t n) {
    if (b->lit_off[n] == UINT32_MAX) {
        /* A K operand becomes constant literals. */
        const llg_cr_node_t* nd = &s->p->nodes[n];
        if (s->status[n] != CR_K) cr_fatal("blasting a node without literals");
        b->lit_off[n] = (uint32_t)b->lits.len;
        const uint64_t* v = cr_node_val(s, n);
        for (uint32_t i = 0; i < nd->width; ++i)
            cr_u32_push(&b->lits, bv_bit(v, i) ? CR_TRUE : CR_FALSE);
        if (nd->width == 0u) cr_u32_push(&b->lits, CR_FALSE);
    }
    return b->lits.data + b->lit_off[n];
}

/* Copy node `x`'s literals into a fresh array extended to w bits. */
static uint32_t* cr_operand_lits(llg_cr_session_t* s, cr_blast_t* b, uint32_t x,
                                 uint32_t w) {
    const llg_cr_node_t* nx = &s->p->nodes[x];
    uint32_t* out = cr_lits(w);
    cr_blast_extend(out, w, cr_node_lits(s, b, x), nx->width, nx->is_signed);
    return out;
}

static uint32_t cr_operand_truth(llg_cr_session_t* s, cr_blast_t* b, uint32_t x) {
    return cr_blast_any(&b->aig, cr_node_lits(s, b, x), s->p->nodes[x].width);
}

/* Blast R node n (all its R operands are already blasted). */
static void cr_blast_node(llg_cr_session_t* s, cr_blast_t* b, uint32_t n,
                          const uint32_t* var_base, const uint32_t* bit_input) {
    const llg_cr_problem_t* p = s->p;
    const llg_cr_node_t* nd = &p->nodes[n];
    cr_aig_t* g = &b->aig;
    uint32_t w = nd->width;
    uint32_t* out = cr_lits(w);
    for (uint32_t i = 0; i < w; ++i) out[i] = CR_FALSE;
    const llg_cr_node_t* na = nd->a < p->node_count ? &p->nodes[nd->a] : NULL;
    switch (nd->op) {
    case LLG_CR_VAR: {
        const llg_cr_var_t* var = &p->vars[nd->a];
        uint32_t* bits = cr_lits(var->width);
        for (uint32_t i = 0; i < var->width; ++i)
            bits[i] = bit_input[var_base[nd->a] + i] << 1;
        cr_blast_extend(out, w, bits, var->width, var->is_signed);
        free(bits);
        break;
    }
    case LLG_CR_RESIZE:
        cr_blast_extend(out, w, cr_node_lits(s, b, nd->a), na->width, na->is_signed);
        break;
    case LLG_CR_NOT:
    case LLG_CR_NEG: {
        uint32_t* a = cr_operand_lits(s, b, nd->a, w);
        if (nd->op == LLG_CR_NOT) for (uint32_t i = 0; i < w; ++i) out[i] = a[i] ^ 1u;
        else cr_blast_neg(g, out, a, w);
        free(a);
        break;
    }
    case LLG_CR_REDAND:
    case LLG_CR_REDNAND: {
        const uint32_t* a = cr_node_lits(s, b, nd->a);
        uint32_t r = CR_TRUE;
        for (uint32_t i = 0; i < na->width; ++i) r = cr_and(g, r, a[i]);
        out[0] = nd->op == LLG_CR_REDAND ? r : r ^ 1u;
        break;
    }
    case LLG_CR_REDOR:
    case LLG_CR_REDNOR: {
        uint32_t r = cr_operand_truth(s, b, nd->a);
        out[0] = nd->op == LLG_CR_REDOR ? r : r ^ 1u;
        break;
    }
    case LLG_CR_REDXOR:
    case LLG_CR_REDXNOR: {
        const uint32_t* a = cr_node_lits(s, b, nd->a);
        uint32_t r = CR_FALSE;
        for (uint32_t i = 0; i < na->width; ++i) r = cr_xor(g, r, a[i]);
        out[0] = nd->op == LLG_CR_REDXOR ? r : r ^ 1u;
        break;
    }
    case LLG_CR_LNOT:
        out[0] = cr_operand_truth(s, b, nd->a) ^ 1u;
        break;
    case LLG_CR_LAND:
        out[0] = cr_and(g, cr_operand_truth(s, b, nd->a), cr_operand_truth(s, b, nd->b));
        break;
    case LLG_CR_LOR:
        out[0] = cr_or(g, cr_operand_truth(s, b, nd->a), cr_operand_truth(s, b, nd->b));
        break;
    case LLG_CR_IMPL:
        out[0] = cr_or(g, cr_operand_truth(s, b, nd->a) ^ 1u, cr_operand_truth(s, b, nd->b));
        break;
    case LLG_CR_EQUIV:
        out[0] = cr_xor(g, cr_operand_truth(s, b, nd->a), cr_operand_truth(s, b, nd->b)) ^ 1u;
        break;
    case LLG_CR_ALL: {
        uint32_t r = CR_TRUE;
        for (uint32_t i = 0; i < nd->c; ++i)
            r = cr_and(g, r, cr_operand_truth(s, b, p->args[nd->b + i]));
        out[0] = r;
        break;
    }
    case LLG_CR_ADD:
    case LLG_CR_SUB:
    case LLG_CR_MUL:
    case LLG_CR_AND:
    case LLG_CR_OR:
    case LLG_CR_XOR:
    case LLG_CR_XNOR:
    case LLG_CR_DIV:
    case LLG_CR_MOD: {
        uint32_t* a = cr_operand_lits(s, b, nd->a, w);
        uint32_t* c = cr_operand_lits(s, b, nd->b, w);
        switch (nd->op) {
        case LLG_CR_ADD: cr_blast_add(g, out, a, c, w, CR_FALSE); break;
        case LLG_CR_SUB: cr_blast_sub(g, out, a, c, w); break;
        case LLG_CR_MUL: cr_blast_mul(g, out, a, c, w); break;
        case LLG_CR_AND: for (uint32_t i = 0; i < w; ++i) out[i] = cr_and(g, a[i], c[i]); break;
        case LLG_CR_OR: for (uint32_t i = 0; i < w; ++i) out[i] = cr_or(g, a[i], c[i]); break;
        case LLG_CR_XOR: for (uint32_t i = 0; i < w; ++i) out[i] = cr_xor(g, a[i], c[i]); break;
        case LLG_CR_XNOR: for (uint32_t i = 0; i < w; ++i) out[i] = cr_xor(g, a[i], c[i]) ^ 1u; break;
        default: {
            /* Signed operands divide magnitudes and fix the signs. */
            uint32_t sa = nd->is_signed && w ? a[w - 1u] : CR_FALSE;
            uint32_t sb = nd->is_signed && w ? c[w - 1u] : CR_FALSE;
            uint32_t* ua = cr_lits(w);
            uint32_t* ub = cr_lits(w);
            uint32_t* t = cr_lits(w);
            uint32_t* q = cr_lits(w);
            uint32_t* r = cr_lits(w);
            cr_blast_neg(g, t, a, w);
            for (uint32_t i = 0; i < w; ++i) ua[i] = cr_mux(g, sa, t[i], a[i]);
            cr_blast_neg(g, t, c, w);
            for (uint32_t i = 0; i < w; ++i) ub[i] = cr_mux(g, sb, t[i], c[i]);
            cr_blast_udivmod(g, q, r, ua, ub, w);
            if (nd->op == LLG_CR_DIV) {
                uint32_t flip = cr_xor(g, sa, sb);
                cr_blast_neg(g, t, q, w);
                for (uint32_t i = 0; i < w; ++i) out[i] = cr_mux(g, flip, t[i], q[i]);
            } else {
                cr_blast_neg(g, t, r, w);
                for (uint32_t i = 0; i < w; ++i) out[i] = cr_mux(g, sa, t[i], r[i]);
            }
            free(ua); free(ub); free(t); free(q); free(r);
        }
        }
        free(a);
        free(c);
        break;
    }
    case LLG_CR_SHL:
    case LLG_CR_SHR:
    case LLG_CR_ASHR: {
        uint32_t* a = cr_operand_lits(s, b, nd->a, w);
        const llg_cr_node_t* nb = &p->nodes[nd->b];
        uint32_t fill = nd->op == LLG_CR_ASHR && nd->is_signed && w ? a[w - 1u] : CR_FALSE;
        cr_blast_shift(g, out, a, w, cr_node_lits(s, b, nd->b), nb->width,
                       nd->op == LLG_CR_SHL, fill);
        free(a);
        break;
    }
    case LLG_CR_EQ:
    case LLG_CR_NE:
    case LLG_CR_LT:
    case LLG_CR_LE:
    case LLG_CR_GT:
    case LLG_CR_GE: {
        const llg_cr_node_t* nb = &p->nodes[nd->b];
        uint32_t cw = na->width > nb->width ? na->width : nb->width;
        int sgn = na->is_signed && nb->is_signed;
        uint32_t* a = cr_operand_lits(s, b, nd->a, cw);
        uint32_t* c = cr_operand_lits(s, b, nd->b, cw);
        if (sgn && cw) {
            a[cw - 1u] ^= 1u;
            c[cw - 1u] ^= 1u;
        }
        uint32_t r;
        switch (nd->op) {
        case LLG_CR_EQ: r = cr_blast_eq(g, a, c, cw); break;
        case LLG_CR_NE: r = cr_blast_eq(g, a, c, cw) ^ 1u; break;
        case LLG_CR_LT: r = cr_blast_ult(g, a, c, cw); break;
        case LLG_CR_GE: r = cr_blast_ult(g, a, c, cw) ^ 1u; break;
        case LLG_CR_GT: r = cr_blast_ult(g, c, a, cw); break;
        default: r = cr_blast_ult(g, c, a, cw) ^ 1u; break;
        }
        out[0] = r;
        free(a);
        free(c);
        break;
    }
    case LLG_CR_COND: {
        uint32_t c = cr_operand_truth(s, b, nd->a);
        uint32_t* t = cr_operand_lits(s, b, nd->b, w);
        uint32_t* f = cr_operand_lits(s, b, nd->c, w);
        for (uint32_t i = 0; i < w; ++i) out[i] = cr_mux(g, c, t[i], f[i]);
        free(t);
        free(f);
        break;
    }
    case LLG_CR_CONCAT: {
        uint32_t pos = 0;
        for (uint32_t i = nd->c; i-- > 0;) {
            uint32_t x = p->args[nd->b + i];
            uint32_t xw = p->nodes[x].width;
            const uint32_t* v = cr_node_lits(s, b, x);
            for (uint32_t bit = 0; bit < xw && pos < w; ++bit, ++pos) out[pos] = v[bit];
        }
        break;
    }
    case LLG_CR_REPL: {
        const uint32_t* v = cr_node_lits(s, b, nd->a);
        uint32_t pos = 0;
        for (uint32_t k = 0; k < nd->b; ++k)
            for (uint32_t bit = 0; bit < na->width && pos < w; ++bit, ++pos) out[pos] = v[bit];
        break;
    }
    case LLG_CR_SLICE:
    case LLG_CR_DSLICE: {
        uint64_t lsb = nd->b;
        if (nd->op == LLG_CR_DSLICE) {
            int64_t index = 0;
            const llg_cr_node_t* ni = &p->nodes[nd->b];
            if (!bv_index(cr_node_val(s, nd->b), ni->width, ni->is_signed, &index) || index < 0)
                cr_fatal("dynamic select index escaped classification");
            lsb = (uint64_t)index;
        }
        const uint32_t* v = cr_node_lits(s, b, nd->a);
        for (uint32_t bit = 0; bit < w; ++bit)
            out[bit] = lsb + bit < na->width ? v[lsb + bit] : CR_FALSE;
        break;
    }
    default:
        cr_fatal("unexpected random node in bit-blasting");
    }
    b->lit_off[n] = (uint32_t)b->lits.len;
    for (uint32_t i = 0; i < w; ++i) cr_u32_push(&b->lits, out[i]);
    if (w == 0u) cr_u32_push(&b->lits, CR_FALSE);
    free(out);
}
