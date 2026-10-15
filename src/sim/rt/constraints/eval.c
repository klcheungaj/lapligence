/* ---- exact 2-state bit vectors -----------------------------------------
 *
 * Values are little-endian uint64_t words with every bit at or above the
 * width cleared. These helpers are the reference semantics: rejection
 * sampling and the final verification evaluate constraints with them, and
 * they share no code with the bit-blasted engines.
 */

static void bv_mask(uint64_t* d, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    uint32_t top = w % 64u;
    if (w == 0u) d[0] = 0;
    else if (top) d[n - 1u] &= (UINT64_C(1) << top) - 1u;
}

static int bv_bit(const uint64_t* s, uint32_t i) {
    return (int)((s[i / 64u] >> (i % 64u)) & 1u);
}

static void bv_set_bit(uint64_t* d, uint32_t i, int bit) {
    uint64_t m = UINT64_C(1) << (i % 64u);
    if (bit) d[i / 64u] |= m;
    else d[i / 64u] &= ~m;
}

static int bv_is_zero(const uint64_t* s, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 0; i < n; ++i)
        if (s[i]) return 0;
    return 1;
}

/* d (width dw) = s (width sw) extended by `sgn` or truncated. */
static void bv_extend(uint64_t* d, uint32_t dw, const uint64_t* s, uint32_t sw,
                      int sgn) {
    uint32_t dn = CR_WORDS(dw), sn = CR_WORDS(sw);
    int fill = sgn && sw > 0u && bv_bit(s, sw - 1u);
    for (uint32_t i = 0; i < dn; ++i) d[i] = i < sn ? s[i] : (fill ? UINT64_MAX : 0u);
    if (fill && dw > sw && sw % 64u) d[sn - 1u] |= ~((UINT64_C(1) << (sw % 64u)) - 1u);
    bv_mask(d, dw);
}

static void bv_not(uint64_t* d, const uint64_t* a, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 0; i < n; ++i) d[i] = ~a[i];
    bv_mask(d, w);
}

static void bv_add(uint64_t* d, const uint64_t* a, const uint64_t* b, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    uint64_t carry = 0;
    for (uint32_t i = 0; i < n; ++i) {
        uint64_t x = a[i], s = x + b[i];
        uint64_t c1 = s < x;
        uint64_t t = s + carry;
        uint64_t c2 = t < s;
        d[i] = t;
        carry = c1 | c2;
    }
    bv_mask(d, w);
}

static void bv_sub(uint64_t* d, const uint64_t* a, const uint64_t* b, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    uint64_t borrow = 0;
    for (uint32_t i = 0; i < n; ++i) {
        uint64_t x = a[i], y = b[i];
        uint64_t t = x - y;
        uint64_t b1 = x < y;
        uint64_t r = t - borrow;
        uint64_t b2 = t < borrow;
        d[i] = r;
        borrow = b1 | b2;
    }
    bv_mask(d, w);
}

static void bv_neg(uint64_t* d, const uint64_t* a, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    uint64_t carry = 1;
    for (uint32_t i = 0; i < n; ++i) {
        uint64_t t = ~a[i] + carry;
        carry = carry && t == 0u;
        d[i] = t;
    }
    bv_mask(d, w);
}

/* 64x64 -> 128 multiply without host 128-bit types. */
static void cr_mul64(uint64_t a, uint64_t b, uint64_t* lo, uint64_t* hi) {
    uint64_t a0 = a & 0xffffffffu, a1 = a >> 32, b0 = b & 0xffffffffu, b1 = b >> 32;
    uint64_t p00 = a0 * b0, p01 = a0 * b1, p10 = a1 * b0, p11 = a1 * b1;
    uint64_t mid = (p00 >> 32) + (p01 & 0xffffffffu) + (p10 & 0xffffffffu);
    *lo = (p00 & 0xffffffffu) | (mid << 32);
    *hi = p11 + (p01 >> 32) + (p10 >> 32) + (mid >> 32);
}

/* d = a * b mod 2^w; d must not alias a or b. */
static void bv_mul(uint64_t* d, const uint64_t* a, const uint64_t* b, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 0; i < n; ++i) d[i] = 0;
    for (uint32_t i = 0; i < n; ++i) {
        if (!a[i]) continue;
        uint64_t carry = 0;
        for (uint32_t j = 0; i + j < n; ++j) {
            uint64_t lo, hi;
            cr_mul64(a[i], b[j], &lo, &hi);
            uint64_t s = d[i + j] + lo;
            hi += s < lo;
            uint64_t t = s + carry;
            hi += t < s;
            d[i + j] = t;
            carry = hi;
        }
    }
    bv_mask(d, w);
}

static int bv_ucmp(const uint64_t* a, const uint64_t* b, uint32_t w) {
    for (uint32_t i = CR_WORDS(w); i-- > 0;) {
        if (a[i] != b[i]) return a[i] < b[i] ? -1 : 1;
    }
    return 0;
}

static int bv_scmp(const uint64_t* a, const uint64_t* b, uint32_t w) {
    int sa = w ? bv_bit(a, w - 1u) : 0, sb = w ? bv_bit(b, w - 1u) : 0;
    if (sa != sb) return sa ? -1 : 1;
    return bv_ucmp(a, b, w);
}

/* Unsigned long division, b != 0. q and r must not alias any input. */
static void bv_udivmod(uint64_t* q, uint64_t* r, const uint64_t* a,
                       const uint64_t* b, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 0; i < n; ++i) q[i] = r[i] = 0;
    for (uint32_t bit = w; bit-- > 0;) {
        /* r = (r << 1) | a[bit]; r < b <= 2^w - 1 so r << 1 fits in w+1
         * bits; track the shifted-out top bit explicitly. */
        uint64_t out = w ? (uint64_t)bv_bit(r, w - 1u) : 0u;
        for (uint32_t i = n; i-- > 0;) {
            r[i] = (r[i] << 1) | (i ? r[i - 1u] >> 63 : 0u);
        }
        r[0] |= (uint64_t)bv_bit(a, bit);
        bv_mask(r, w);
        if (out || bv_ucmp(r, b, w) >= 0) {
            bv_sub(r, r, b, w);
            bv_set_bit(q, bit, 1);
        }
    }
}

static void bv_shl(uint64_t* d, const uint64_t* a, uint32_t w, uint64_t amount) {
    uint32_t n = CR_WORDS(w);
    if (amount >= w) {
        for (uint32_t i = 0; i < n; ++i) d[i] = 0;
        return;
    }
    uint32_t ws = (uint32_t)(amount / 64u), bs = (uint32_t)(amount % 64u);
    for (uint32_t i = n; i-- > 0;) {
        uint64_t v = 0;
        if (i >= ws) {
            v = a[i - ws] << bs;
            if (bs && i > ws) v |= a[i - ws - 1u] >> (64u - bs);
        }
        d[i] = v;
    }
    bv_mask(d, w);
}

static void bv_shr(uint64_t* d, const uint64_t* a, uint32_t w, uint64_t amount,
                   int fill) {
    uint32_t n = CR_WORDS(w);
    if (amount >= w) {
        for (uint32_t i = 0; i < n; ++i) d[i] = fill ? UINT64_MAX : 0u;
        bv_mask(d, w);
        return;
    }
    uint32_t ws = (uint32_t)(amount / 64u), bs = (uint32_t)(amount % 64u);
    for (uint32_t i = 0; i < n; ++i) {
        uint64_t v = 0;
        if (i + ws < n) {
            v = a[i + ws] >> bs;
            if (bs && i + ws + 1u < n) v |= a[i + ws + 1u] << (64u - bs);
        }
        d[i] = v;
    }
    if (fill) {
        for (uint32_t bit = w - (uint32_t)amount; bit < w; ++bit) bv_set_bit(d, bit, 1);
    }
    bv_mask(d, w);
}

/* Shift amount as uint64, saturating to UINT64_MAX. */
static uint64_t bv_amount(const uint64_t* s, uint32_t w) {
    uint32_t n = CR_WORDS(w);
    for (uint32_t i = 1; i < n; ++i)
        if (s[i]) return UINT64_MAX;
    return s[0];
}

/* Signed or unsigned index; returns 0 when it does not fit int64. */
static int bv_index(const uint64_t* s, uint32_t w, int sgn, int64_t* out) {
    uint32_t n = CR_WORDS(w);
    int neg = sgn && w && bv_bit(s, w - 1u);
    uint64_t low = s[0];
    if (w < 64u && neg) low |= ~((UINT64_C(1) << w) - 1u);
    for (uint32_t i = 1; i < n; ++i) {
        uint64_t expect = neg ? UINT64_MAX : 0u;
        uint64_t word = s[i];
        if (i == n - 1u && w % 64u && neg) word |= ~((UINT64_C(1) << (w % 64u)) - 1u);
        if (word != expect) return 0;
    }
    if (w > 64u && neg != ((low >> 63) != 0u)) return 0;
    if (!sgn && (low >> 63)) return 0;
    *out = (int64_t)low;
    return 1;
}

/* Number of significant bits of `s` (width w): 0 for zero. */
static uint32_t bn_top_bit(const uint64_t* s, uint32_t w) {
    for (uint32_t i = CR_WORDS(w); i-- > 0;) {
        if (!s[i]) continue;
        uint32_t b = 64u;
        while (!((s[i] >> (b - 1u)) & 1u)) --b;
        return i * 64u + b;
    }
    return 0;
}

/* ---- node evaluation --------------------------------------------------- */

static uint64_t* cr_node_val(llg_cr_session_t* s, uint32_t n);
static int cr_truth(llg_cr_session_t* s, uint32_t x);

/* Whether dist item `item`'s weight is zero (such an item is a constraint
 * that excludes its values, 18.5.4). The weight node is known. */
static int cr_dist_item_zero(llg_cr_session_t* s, const llg_cr_dist_item_t* item) {
    if (item->weight == LLG_CR_NONE) return 0;
    return bv_is_zero(cr_node_val(s, item->weight), s->p->nodes[item->weight].bit_width);
}

/* Value of a dist node whose items are all evaluated: some item of nonzero
 * weight matches. */
static int cr_dist_member(llg_cr_session_t* s, uint32_t dist) {
    const llg_cr_dist_t* d = &s->p->dists[dist];
    for (uint32_t i = 0; i < d->item_count; ++i) {
        const llg_cr_dist_item_t* item = &s->p->dist_items[d->item_first + i];
        if (!cr_dist_item_zero(s, item) && cr_truth(s, item->match)) return 1;
    }
    return 0;
}

static uint64_t* cr_node_val(llg_cr_session_t* s, uint32_t n) {
    return s->val + s->node_off[n];
}

/* Operand `x` extended to `w` bits by its own signedness. Returns a pointer
 * into the node arena when no conversion is needed, else into `tmp`. */
static const uint64_t* cr_operand(llg_cr_session_t* s, uint32_t x, uint32_t w,
                                  uint64_t* tmp) {
    const llg_cr_node_t* nx = &s->p->nodes[x];
    if (nx->bit_width == w) return cr_node_val(s, x);
    bv_extend(tmp, w, cr_node_val(s, x), nx->bit_width, nx->sign_flag);
    return tmp;
}

static int cr_truth(llg_cr_session_t* s, uint32_t x) {
    return !bv_is_zero(cr_node_val(s, x), s->p->nodes[x].bit_width);
}

/* Evaluate one node whose operands all hold values (K, or R with the
 * candidate assignment). Returns 0, or LLG_CR_REASON_DIVZERO / _INDEX when
 * the operation is an evaluation error. VAR/STATE/CALL/CONST are filled by
 * the callers. */
static int cr_eval_op(llg_cr_session_t* s, uint32_t n) {
    const llg_cr_node_t* nd = &s->p->nodes[n];
    uint64_t* d = cr_node_val(s, n);
    uint32_t w = nd->bit_width, nw = CR_WORDS(w);
    const llg_cr_node_t* na = nd->a < s->p->node_count ? &s->p->nodes[nd->a] : NULL;
    switch (nd->op) {
    case LLG_CR_RESIZE:
        bv_extend(d, w, cr_node_val(s, nd->a), na->bit_width, na->sign_flag);
        return 0;
    case LLG_CR_NOT:
        bv_not(d, cr_operand(s, nd->a, w, s->tmp_a), w);
        return 0;
    case LLG_CR_NEG:
        bv_neg(d, cr_operand(s, nd->a, w, s->tmp_a), w);
        return 0;
    case LLG_CR_REDAND:
    case LLG_CR_REDNAND: {
        const uint64_t* a = cr_node_val(s, nd->a);
        int all = 1;
        for (uint32_t i = 0; i < na->bit_width; ++i)
            if (!bv_bit(a, i)) { all = 0; break; }
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = (uint64_t)(nd->op == LLG_CR_REDAND ? all : !all);
        return 0;
    }
    case LLG_CR_REDOR:
    case LLG_CR_REDNOR: {
        int any = cr_truth(s, nd->a);
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = (uint64_t)(nd->op == LLG_CR_REDOR ? any : !any);
        return 0;
    }
    case LLG_CR_REDXOR:
    case LLG_CR_REDXNOR: {
        const uint64_t* a = cr_node_val(s, nd->a);
        uint64_t parity = 0;
        for (uint32_t i = 0; i < CR_WORDS(na->bit_width); ++i) {
            uint64_t x = a[i];
            x ^= x >> 32; x ^= x >> 16; x ^= x >> 8; x ^= x >> 4; x ^= x >> 2; x ^= x >> 1;
            parity ^= x & 1u;
        }
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = nd->op == LLG_CR_REDXOR ? parity : (parity ^ 1u);
        return 0;
    }
    case LLG_CR_LNOT:
    case LLG_CR_LAND:
    case LLG_CR_LOR:
    case LLG_CR_IMPL:
    case LLG_CR_EQUIV:
    case LLG_CR_ALL: {
        int r = 0;
        if (nd->op == LLG_CR_LNOT) r = !cr_truth(s, nd->a);
        else if (nd->op == LLG_CR_LAND) r = cr_truth(s, nd->a) && cr_truth(s, nd->b);
        else if (nd->op == LLG_CR_LOR) r = cr_truth(s, nd->a) || cr_truth(s, nd->b);
        else if (nd->op == LLG_CR_IMPL) r = !cr_truth(s, nd->a) || cr_truth(s, nd->b);
        else if (nd->op == LLG_CR_EQUIV) r = cr_truth(s, nd->a) == cr_truth(s, nd->b);
        else {
            r = 1;
            for (uint32_t i = 0; i < nd->c; ++i)
                if (!cr_truth(s, s->p->args[nd->b + i])) { r = 0; break; }
        }
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = (uint64_t)r;
        return 0;
    }
    case LLG_CR_ADD:
    case LLG_CR_SUB:
    case LLG_CR_MUL:
    case LLG_CR_AND:
    case LLG_CR_OR:
    case LLG_CR_XOR:
    case LLG_CR_XNOR: {
        const uint64_t* a = cr_operand(s, nd->a, w, s->tmp_a);
        const uint64_t* b = cr_operand(s, nd->b, w, s->tmp_b);
        switch (nd->op) {
        case LLG_CR_ADD: bv_add(d, a, b, w); break;
        case LLG_CR_SUB: bv_sub(d, a, b, w); break;
        case LLG_CR_MUL: bv_mul(d, a, b, w); break;
        default:
            for (uint32_t i = 0; i < nw; ++i) {
                uint64_t x = a[i], y = b[i];
                d[i] = nd->op == LLG_CR_AND ? (x & y)
                     : nd->op == LLG_CR_OR  ? (x | y)
                     : nd->op == LLG_CR_XOR ? (x ^ y)
                                            : ~(x ^ y);
            }
            bv_mask(d, w);
        }
        return 0;
    }
    case LLG_CR_DIV:
    case LLG_CR_MOD: {
        const uint64_t* a = cr_operand(s, nd->a, w, s->tmp_a);
        const uint64_t* b = cr_operand(s, nd->b, w, s->tmp_b);
        if (bv_is_zero(b, w)) return LLG_CR_REASON_DIVZERO;
        int sa = nd->sign_flag && w && bv_bit(a, w - 1u);
        int sb = nd->sign_flag && w && bv_bit(b, w - 1u);
        uint64_t* ua = s->tmp_c;
        uint64_t* ub = s->tmp_d;
        if (sa) bv_neg(ua, a, w); else memcpy(ua, a, nw * sizeof(uint64_t));
        if (sb) bv_neg(ub, b, w); else memcpy(ub, b, nw * sizeof(uint64_t));
        /* a and b were copied into ua/ub, so tmp_a and tmp_b are free. */
        uint64_t* q = s->tmp_b;
        uint64_t* r = s->tmp_a;
        bv_udivmod(q, r, ua, ub, w);
        if (nd->op == LLG_CR_DIV) {
            if (sa != sb) bv_neg(d, q, w);
            else memcpy(d, q, nw * sizeof(uint64_t));
        } else {
            if (sa) bv_neg(d, r, w);
            else memcpy(d, r, nw * sizeof(uint64_t));
        }
        return 0;
    }
    case LLG_CR_POW: {
        const llg_cr_node_t* nb = &s->p->nodes[nd->b];
        const uint64_t* a = cr_operand(s, nd->a, w, s->tmp_a);
        const uint64_t* b = cr_node_val(s, nd->b);
        int b_negative = nb->sign_flag && nb->bit_width && bv_bit(b, nb->bit_width - 1u);
        if (b_negative || bv_is_zero(b, nb->bit_width)) {
            /* Table 11-4: op2 zero gives 1; a negative op2 gives 'x for op1
             * zero, 1 for op1 one, +-1 for op1 -1 and 0 otherwise. */
            int a_zero = bv_is_zero(a, w);
            int a_minus_one = nd->sign_flag && w > 0u;
            for (uint32_t bit = 0; a_minus_one && bit < w; ++bit) a_minus_one = bv_bit(a, bit);
            int a_one = !a_zero && !a_minus_one && a[0] == 1u;
            for (uint32_t i = 1; a_one && i < nw; ++i) a_one = a[i] == 0u;
            for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
            if (!b_negative || a_one) {
                d[0] = 1u;
            } else if (a_zero) {
                return LLG_CR_REASON_DIVZERO;
            } else if (a_minus_one) {
                if (bv_bit(b, 0)) for (uint32_t i = 0; i < nw; ++i) d[i] = UINT64_MAX;
                else d[0] = 1u;
            }
            bv_mask(d, w);
            return 0;
        }
        /* Square and multiply over the exponent bits, modulo 2^w. */
        uint64_t* acc = s->tmp_c;
        uint64_t* base = s->tmp_d;
        uint64_t* t = s->tmp_b;
        for (uint32_t i = 0; i < nw; ++i) { acc[i] = 0; base[i] = a[i]; }
        acc[0] = 1u;
        bv_mask(acc, w);
        uint32_t bits = bn_top_bit(b, nb->bit_width);
        for (uint32_t bit = 0; bit < bits; ++bit) {
            if (bv_bit(b, bit)) {
                bv_mul(t, acc, base, w);
                memcpy(acc, t, nw * sizeof(uint64_t));
            }
            if (bit + 1u < bits) {
                bv_mul(t, base, base, w);
                memcpy(base, t, nw * sizeof(uint64_t));
            }
        }
        memcpy(d, acc, nw * sizeof(uint64_t));
        return 0;
    }
    case LLG_CR_DIST: {
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = (uint64_t)cr_dist_member(s, nd->a);
        return 0;
    }
    case LLG_CR_SHL:
    case LLG_CR_SHR:
    case LLG_CR_ASHR: {
        const uint64_t* a = cr_operand(s, nd->a, w, s->tmp_a);
        uint64_t amount = bv_amount(cr_node_val(s, nd->b), s->p->nodes[nd->b].bit_width);
        if (nd->op == LLG_CR_SHL) bv_shl(d, a, w, amount);
        else bv_shr(d, a, w, amount,
                    nd->op == LLG_CR_ASHR && nd->sign_flag && w && bv_bit(a, w - 1u));
        return 0;
    }
    case LLG_CR_EQ:
    case LLG_CR_NE:
    case LLG_CR_LT:
    case LLG_CR_LE:
    case LLG_CR_GT:
    case LLG_CR_GE: {
        const llg_cr_node_t* nb = &s->p->nodes[nd->b];
        uint32_t cw = na->bit_width > nb->bit_width ? na->bit_width : nb->bit_width;
        int sgn = na->sign_flag && nb->sign_flag;
        const uint64_t* a = cr_operand(s, nd->a, cw, s->tmp_a);
        const uint64_t* b = cr_operand(s, nd->b, cw, s->tmp_b);
        int c = sgn ? bv_scmp(a, b, cw) : bv_ucmp(a, b, cw);
        int r = nd->op == LLG_CR_EQ ? c == 0
              : nd->op == LLG_CR_NE ? c != 0
              : nd->op == LLG_CR_LT ? c < 0
              : nd->op == LLG_CR_LE ? c <= 0
              : nd->op == LLG_CR_GT ? c > 0
                                    : c >= 0;
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        d[0] = (uint64_t)r;
        return 0;
    }
    case LLG_CR_COND: {
        uint32_t pick = cr_truth(s, nd->a) ? nd->b : nd->c;
        const uint64_t* v = cr_operand(s, pick, w, s->tmp_a);
        memcpy(d, v, nw * sizeof(uint64_t));
        return 0;
    }
    case LLG_CR_CONCAT: {
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        uint32_t pos = 0;
        for (uint32_t i = nd->c; i-- > 0;) {
            uint32_t x = s->p->args[nd->b + i];
            const uint64_t* v = cr_node_val(s, x);
            uint32_t xw = s->p->nodes[x].bit_width;
            for (uint32_t bit = 0; bit < xw && pos < w; ++bit, ++pos)
                if (bv_bit(v, bit)) bv_set_bit(d, pos, 1);
        }
        return 0;
    }
    case LLG_CR_REPL: {
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        const uint64_t* v = cr_node_val(s, nd->a);
        uint32_t pos = 0;
        for (uint32_t k = 0; k < nd->b; ++k)
            for (uint32_t bit = 0; bit < na->bit_width && pos < w; ++bit, ++pos)
                if (bv_bit(v, bit)) bv_set_bit(d, pos, 1);
        return 0;
    }
    case LLG_CR_SLICE:
    case LLG_CR_DSLICE: {
        int64_t lsb = nd->b;
        if (nd->op == LLG_CR_DSLICE) {
            const llg_cr_node_t* ni = &s->p->nodes[nd->b];
            if (!bv_index(cr_node_val(s, nd->b), ni->bit_width, ni->sign_flag, &lsb))
                return LLG_CR_REASON_INDEX;
            if (lsb < 0 || (uint64_t)lsb + w > na->bit_width) return LLG_CR_REASON_INDEX;
        }
        for (uint32_t i = 0; i < nw; ++i) d[i] = 0;
        const uint64_t* v = cr_node_val(s, nd->a);
        for (uint32_t bit = 0; bit < w; ++bit) {
            uint64_t from = (uint64_t)lsb + bit;
            if (from < na->bit_width && bv_bit(v, (uint32_t)from)) bv_set_bit(d, bit, 1);
        }
        return 0;
    }
    default:
        cr_fatal("invalid node operation");
    }
    return 0;
}

/* ---- classification (IEEE 1800-2009 18.5.12) ---------------------------
 *
 * Every reachable node becomes K (value known before solving), R (depends on
 * an unsolved random variable) or E (evaluation error). Conjunction,
 * disjunction and negation follow Figure 18-3; every other operator is E if
 * any operand is E, else R if any operand is R. A known false conjunct thus
 * eliminates guarded errors, and an R result never has an E operand.
 */

static void cr_set_error(llg_cr_session_t* s, uint32_t n, int reason, uint32_t item) {
    s->status[n] = CR_E;
    s->ereason[n] = (uint8_t)reason;
    s->eitem[n] = item;
}

static void cr_inherit_error(llg_cr_session_t* s, uint32_t n, uint32_t from) {
    cr_set_error(s, n, s->ereason[from], s->eitem[from]);
}

static void cr_copy_input(llg_cr_session_t* s, uint32_t n, const uint64_t* words,
                          uint32_t width, int sgn) {
    const llg_cr_node_t* nd = &s->p->nodes[n];
    bv_extend(cr_node_val(s, n), nd->bit_width, words, width, sgn);
}

static void cr_classify_logic(llg_cr_session_t* s, uint32_t n, const uint32_t* xs,
                              uint32_t count, int disjunction, const int* invert) {
    /* Conjunction: any 0 -> 0, else any E -> E, else any R -> R, else 1.
     * Disjunction: any 1 -> 1, else any E -> E, else any R -> R, else 0.
     * `invert[i]` negates operand i first (for implication). */
    int dominant = 0, error = -1, random = 0;
    for (uint32_t i = 0; i < count; ++i) {
        uint32_t x = xs[i];
        int inv = invert && invert[i];
        if (s->status[x] == CR_K) {
            int t = cr_truth(s, x) ^ inv;
            if (disjunction ? t : !t) dominant = 1;
        } else if (s->status[x] == CR_E) {
            if (error < 0) error = (int)i;
        } else {
            random = 1;
        }
    }
    if (dominant) {
        s->status[n] = CR_K;
        uint64_t* d = cr_node_val(s, n);
        memset(d, 0, CR_WORDS(s->p->nodes[n].bit_width) * sizeof(uint64_t));
        d[0] = (uint64_t)disjunction;
    } else if (error >= 0) {
        cr_inherit_error(s, n, xs[error]);
    } else if (random) {
        s->status[n] = CR_R;
    } else {
        s->status[n] = CR_K;
        uint64_t* d = cr_node_val(s, n);
        memset(d, 0, CR_WORDS(s->p->nodes[n].bit_width) * sizeof(uint64_t));
        d[0] = (uint64_t)!disjunction;
    }
}

static void cr_eval_call(llg_cr_session_t* s, uint32_t n) {
    const llg_cr_node_t* nd = &s->p->nodes[n];
    const llg_cr_call_t* call = &s->p->calls[nd->a];
    uint64_t* result = s->call_val + s->call_off[nd->a];
    if (s->call_state[nd->a] != 2u) {
        /* Arguments are known: the caller evaluates the function between
         * solve steps (llg_cr_step returns LLG_CR_NEED_CALLS). */
        s->call_state[nd->a] = 1u;
        s->need_calls = 1;
        s->status[n] = CR_R;
        return;
    }
    if (s->call_input[nd->a] != LLG_CR_INPUT_KNOWN) {
        cr_set_error(s, n, LLG_CR_REASON_XZ_CALL, nd->a);
        return;
    }
    s->status[n] = CR_K;
    cr_copy_input(s, n, result, call->bit_width, call->sign_flag);
}

/* A dist node: weights must be known and non-negative (S30-D8); the value
 * is the disjunction (Figure 18-3) of the matches of nonzero-weight items. */
static void cr_classify_dist(llg_cr_session_t* s, uint32_t n) {
    const llg_cr_problem_t* p = s->p;
    const llg_cr_dist_t* d = &p->dists[p->nodes[n].a];
    int dominant = 0, error = -1, random = 0;
    for (uint32_t i = 0; i < d->item_count; ++i) {
        const llg_cr_dist_item_t* item = &p->dist_items[d->item_first + i];
        if (item->weight != LLG_CR_NONE) {
            uint32_t w = item->weight;
            if (s->status[w] == CR_E) {
                cr_inherit_error(s, n, w);
                return;
            }
            const llg_cr_node_t* nw = &p->nodes[w];
            if (s->status[w] != CR_K ||
                (nw->sign_flag && bv_bit(cr_node_val(s, w), nw->bit_width - 1u))) {
                cr_set_error(s, n, LLG_CR_REASON_WEIGHT, n);
                return;
            }
            if (cr_dist_item_zero(s, item)) continue;
        }
        uint32_t m = item->match;
        if (s->status[m] == CR_K) {
            if (cr_truth(s, m)) dominant = 1;
        } else if (s->status[m] == CR_E) {
            if (error < 0) error = (int)m;
        } else {
            random = 1;
        }
    }
    uint64_t* v = cr_node_val(s, n);
    if (dominant || (error < 0 && !random)) {
        s->status[n] = CR_K;
        memset(v, 0, CR_WORDS(p->nodes[n].bit_width) * sizeof(uint64_t));
        v[0] = (uint64_t)dominant;
    } else if (error >= 0) {
        cr_inherit_error(s, n, (uint32_t)error);
    } else {
        s->status[n] = CR_R;
    }
}

static void cr_classify_node(llg_cr_session_t* s, uint32_t n);

/* Classify every node not marked CR_SKIP, in post-order. */
static void cr_classify(llg_cr_session_t* s) {
    s->need_calls = 0;
    for (uint32_t n = 0; n < s->p->node_count; ++n) {
        if (s->status[n] == CR_SKIP) continue;
        cr_classify_node(s, n);
    }
}

/* Classify node n from its operands' classification. */
static void cr_classify_node(llg_cr_session_t* s, uint32_t n) {
    const llg_cr_problem_t* p = s->p;
    {
        const llg_cr_node_t* nd = &p->nodes[n];
        switch (nd->op) {
        case LLG_CR_CONST:
            s->status[n] = CR_K;
            cr_copy_input(s, n, p->consts + nd->a, nd->bit_width, nd->sign_flag);
            return;
        case LLG_CR_VAR: {
            uint32_t v = nd->a;
            const llg_cr_var_t* var = &p->vars[v];
            if (s->var_active[v] && !s->var_fixed[v]) {
                s->status[n] = CR_R;
            } else if (s->var_active[v]) {
                s->status[n] = CR_K;
                cr_copy_input(s, n, s->sol + s->var_off[v], var->bit_width, var->sign_flag);
            } else if (s->var_input[v] != LLG_CR_INPUT_KNOWN) {
                cr_set_error(s, n, LLG_CR_REASON_XZ_VAR, v);
            } else {
                s->status[n] = CR_K;
                cr_copy_input(s, n, s->var_cur + s->var_off[v], var->bit_width, var->sign_flag);
            }
            return;
        }
        case LLG_CR_STATE: {
            uint32_t l = nd->a;
            const llg_cr_leaf_t* leaf = &p->leaves[l];
            if (s->leaf_input[l] == LLG_CR_INPUT_NULL) {
                cr_set_error(s, n, LLG_CR_REASON_NULL, l);
            } else if (s->leaf_input[l] != LLG_CR_INPUT_KNOWN) {
                cr_set_error(s, n, LLG_CR_REASON_XZ_LEAF, l);
            } else {
                s->status[n] = CR_K;
                cr_copy_input(s, n, s->leaf_val + s->leaf_off[l], leaf->bit_width,
                              leaf->sign_flag);
            }
            return;
        }
        case LLG_CR_CALL: {
            const llg_cr_call_t* call = &p->calls[nd->a];
            int random = 0, error = -1;
            for (uint32_t i = 0; i < call->arg_count; ++i) {
                uint32_t x = p->args[call->arg_first + i];
                if (s->status[x] == CR_E && error < 0) error = (int)x;
                else if (s->status[x] == CR_R) random = 1;
            }
            if (error >= 0) cr_inherit_error(s, n, (uint32_t)error);
            else if (random) s->status[n] = CR_R;
            else cr_eval_call(s, n);
            return;
        }
        case LLG_CR_LAND:
        case LLG_CR_LOR: {
            uint32_t xs[2] = {nd->a, nd->b};
            cr_classify_logic(s, n, xs, 2u, nd->op == LLG_CR_LOR, NULL);
            return;
        }
        case LLG_CR_IMPL: {
            uint32_t xs[2] = {nd->a, nd->b};
            int inv[2] = {1, 0};
            cr_classify_logic(s, n, xs, 2u, 1, inv);
            return;
        }
        case LLG_CR_ALL:
            cr_classify_logic(s, n, p->args + nd->b, nd->c, 0, NULL);
            return;
        case LLG_CR_DIST:
            cr_classify_dist(s, n);
            return;
        default:
            break;
        }
        /* Generic operators (LNOT behaves the same: E -> E, R -> R). */
        int random = 0, error = -1;
        uint32_t ops[3];
        uint32_t count = 0;
        switch (nd->op) {
        case LLG_CR_CONCAT:
            for (uint32_t i = 0; i < nd->c; ++i) {
                uint32_t x = p->args[nd->b + i];
                if (s->status[x] == CR_E && error < 0) error = (int)x;
                else if (s->status[x] == CR_R) random = 1;
            }
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
            ops[count++] = nd->a;
            break;
        case LLG_CR_COND:
            ops[count++] = nd->a;
            ops[count++] = nd->b;
            ops[count++] = nd->c;
            break;
        default:
            ops[count++] = nd->a;
            ops[count++] = nd->b;
            break;
        }
        for (uint32_t i = 0; i < count; ++i) {
            if (s->status[ops[i]] == CR_E && error < 0) error = (int)ops[i];
            else if (s->status[ops[i]] == CR_R) random = 1;
        }
        if (error >= 0) {
            cr_inherit_error(s, n, (uint32_t)error);
            return;
        }
        /* A known divisor of zero or a known out-of-range index is an error
         * even when the other operand is random. */
        if ((nd->op == LLG_CR_DIV || nd->op == LLG_CR_MOD) && s->status[nd->b] == CR_K &&
            bv_is_zero(cr_node_val(s, nd->b), p->nodes[nd->b].bit_width)) {
            cr_set_error(s, n, LLG_CR_REASON_DIVZERO, n);
            return;
        }
        /* A random index or divisor stays R: the engines exclude the
         * values that make it an error (S30-D4). */
        if (nd->op == LLG_CR_DSLICE && s->status[nd->b] == CR_K) {
            const llg_cr_node_t* ni = &p->nodes[nd->b];
            int64_t lsb;
            if (!bv_index(cr_node_val(s, nd->b), ni->bit_width, ni->sign_flag, &lsb) ||
                lsb < 0 || (uint64_t)lsb + nd->bit_width > p->nodes[nd->a].bit_width) {
                cr_set_error(s, n, LLG_CR_REASON_INDEX, n);
                return;
            }
        }
        if (random) {
            s->status[n] = CR_R;
            return;
        }
        s->status[n] = CR_K;
        int reason = cr_eval_op(s, n);
        if (reason) cr_set_error(s, n, reason, n);
    }
}
