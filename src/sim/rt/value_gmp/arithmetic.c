#include "internal.h"

static g4_t multiply(g4_t a, g4_t b) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    size_t n = ((size_t)w + 63u) / 64u;
    int mixed = a.width != w || b.width != w;
    size_t product = g4_product_words(n);
    /* One result allocation includes any temporary tail and normalized inputs.
     * The 65-bit/equal-width path allocates exactly the two result words. */
    uint64_t* p = llg_gmp_sv4_alloc(product + (mixed ? 2u * n : 0));
    const uint64_t *x = g4_a(&a), *y = g4_a(&b);
    if (mixed) {
        llg_gmp_sv4_extend_plane(p + product, w, x, a.width, s);
        llg_gmp_sv4_extend_plane(p + product + n, w, y, b.width, s);
        x = p + product;
        y = x + n;
    }
    llg_gmp_sv4_kernel_mul(p, x, y, n);
    p[n - 1] &= g4_topmask(w);
    if (product != n || mixed) {
        uint64_t* q = (uint64_t*)realloc(p, n * 8u);
        if (!q)
            llg_gmp_sv4_fail("allocation failed");
        p = q;
    }
    g4_t out = LLG_GMP_SV4_EMPTY;
    out.width = w;
    out.is_signed = s;
    out.data.wide.a = p;
    out.data.wide.b = NULL;
    return out;
}
/* Known equal-width operands need neither normalization nor a copied left
 * operand. Every allocated word is written before the result is published. */
g4_t llg_gmp_sv4_binary_known_equal(const uint64_t* lhs, const uint64_t* rhs, uint32_t width,
                                    int8_t sign, unsigned op) {
    size_t n = ((size_t)width + 63u) / 64u;
    uint64_t* result = (uint64_t*)malloc(n * sizeof(uint64_t));
    if (!result)
        llg_gmp_sv4_fail("allocation failed");
    if (op <= 1) {
        uint64_t carry = op == 1;
        for (size_t i = 0; i < n; ++i) {
            uint64_t x = lhs[i], y = op == 1 ? ~rhs[i] : rhs[i];
            uint64_t sum = x + y, r = sum + carry;
            carry = (sum < x) | (r < sum);
            result[i] = r;
        }
    } else {
        for (size_t i = 0; i < n; ++i)
            result[i] = op == 3   ? lhs[i] & rhs[i]
                        : op == 4 ? lhs[i] | rhs[i]
                        : op == 5 ? lhs[i] ^ rhs[i]
                                  : ~(lhs[i] ^ rhs[i]);
    }
    result[n - 1] &= g4_topmask(width);
    g4_t out = LLG_GMP_SV4_EMPTY;
    out.width = width;
    out.is_signed = sign;
    out.data.wide.a = result;
    return out;
}
g4_t llg_gmp_sv4_binary_wide(g4_t a, g4_t b, unsigned op) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    int unknown = llg_gmp_sv4_is_unknown(a) || llg_gmp_sv4_is_unknown(b);
    if (op <= 2 && unknown)
        return llg_gmp_sv4_x(w, s);
    if (op == 2)
        return multiply(a, b);
    g4_t out = llg_gmp_sv4_new(w, s, op >= 3 && unknown);
    llg_gmp_sv4_extend_value(&out, a, s);
    size_t n = llg_gmp_sv4_words(out);
    g4_span rhs = g4_span_of(&b, s);
    uint64_t *aa = out.data.wide.a, *ab = out.data.wide.b;
    if (op <= 1) {
        uint64_t carry = op == 1;
        for (size_t i = 0; i < n; ++i) {
            uint64_t y = G4_SPAN_A(rhs, i);
            if (op == 1)
                y = ~y;
            uint64_t t = aa[i] + y, c = t < aa[i], r = t + carry;
            carry = c | (r < t);
            aa[i] = r;
        }
    } else if (!unknown) {
        for (size_t i = 0; i < n; ++i) {
            uint64_t y = G4_SPAN_A(rhs, i);
            aa[i] = op == 3 ? aa[i] & y : op == 4 ? aa[i] | y : op == 5 ? aa[i] ^ y : ~(aa[i] ^ y);
        }
    } else {
        for (size_t i = 0; i < n; ++i) {
            uint64_t x = aa[i], xb = ab[i], y = G4_SPAN_A(rhs, i), yb = G4_SPAN_B(rhs, i), ones, u;
            if (op == 3) {
                ones = x & ~xb & y & ~yb;
                u = ~(((~x & ~xb) | (~y & ~yb)) | ones);
            } else if (op == 4) {
                ones = (x & ~xb) | (y & ~yb);
                u = ~((~x & ~xb & ~y & ~yb) | ones);
            } else {
                u = xb | yb;
                ones = (op == 5 ? x ^ y : ~(x ^ y)) & ~u;
            }
            aa[i] = ones | u;
            ab[i] = u;
        }
    }
    llg_gmp_sv4_finish(&out);
    return out;
}
g4_t llg_gmp_sv4_unary_wide(g4_t v, int bitwise) {
    if (!bitwise && llg_gmp_sv4_is_unknown(v))
        return llg_gmp_sv4_x(v.width, v.is_signed);
    g4_t out = llg_gmp_sv4_new(v.width, v.is_signed, bitwise && llg_gmp_sv4_is_unknown(v));
    size_t n = llg_gmp_sv4_words(v);
    const uint64_t *a = v.data.wide.a, *b = v.data.wide.b;
    uint64_t* r = out.data.wide.a;
    uint64_t carry = 1;
    if (bitwise && b) {
        memcpy(out.data.wide.b, b, n * 8u);
        for (size_t i = 0; i < n; ++i)
            r[i] = ~a[i] | b[i];
    } else if (bitwise) {
        for (size_t i = 0; i < n; ++i)
            r[i] = ~a[i];
    } else
        for (size_t i = 0; i < n; ++i) {
            uint64_t x = ~a[i];
            r[i] = x + carry;
            carry = r[i] < x;
        }
    llg_gmp_sv4_finish(&out);
    return out;
}
static void negate_words(uint64_t* a, size_t n, uint32_t w) {
    uint64_t carry = 1;
    for (size_t i = 0; i < n; ++i) {
        uint64_t x = ~a[i], r = x + carry;
        carry = r < x;
        a[i] = r;
    }
    if (n)
        a[n - 1] &= g4_topmask(w);
}
g4_t llg_gmp_sv4_divmod_wide(g4_t a, g4_t b, int remainder) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (llg_gmp_sv4_is_unknown(a) || llg_gmp_sv4_is_unknown(b))
        return llg_gmp_sv4_x(w, s);
    size_t n = ((size_t)w + 63) / 64;
    uint64_t *operands = llg_gmp_sv4_alloc(2u * n), *x = operands, *y = x + n;
    llg_gmp_sv4_extend_plane(x, w, g4_a(&a), a.width, s);
    llg_gmp_sv4_extend_plane(y, w, g4_a(&b), b.width, s);
    int nonzero = 0;
    for (size_t i = 0; i < n; ++i)
        nonzero |= y[i] != 0;
    if (!nonzero) {
        free(operands);
        return llg_gmp_sv4_x(w, s);
    }
    int xn = s && ((x[n - 1] >> ((w - 1) % 64)) & 1), yn = s && ((y[n - 1] >> ((w - 1) % 64)) & 1);
    if (xn)
        negate_words(x, n, w);
    if (yn)
        negate_words(y, n, w);
    g4_t out = llg_gmp_sv4_new(w, s, 0);
    llg_gmp_sv4_kernel_div(out.data.wide.a, x, y, n, remainder);
    if (remainder ? xn : xn != yn)
        negate_words(out.data.wide.a, n, w);
    free(operands);
    llg_gmp_sv4_finish(&out);
    return out;
}
g4_t llg_gmp_sv4_pow_wide(g4_t a, g4_t b) {
    if (llg_gmp_sv4_is_unknown(a) || llg_gmp_sv4_is_unknown(b))
        return llg_gmp_sv4_x(a.width, a.is_signed);
    size_t an = llg_gmp_sv4_words(a), bn = llg_gmp_sv4_words(b);
    const uint64_t *ap = g4_a(&a), *bp = g4_a(&b);
    int zero = 1, one = an && ap[0] == 1, minus_one = a.is_signed && a.width;
    for (size_t i = 0; i < an; ++i) {
        zero &= ap[i] == 0;
        if (i)
            one &= ap[i] == 0;
        minus_one &= ap[i] == (i + 1 == an ? g4_topmask(a.width) : UINT64_MAX);
    }
    int negative = b.is_signed && b.width && ((bp[bn - 1] >> ((b.width - 1) % 64)) & 1);
    if (negative) {
        if (zero)
            return llg_gmp_sv4_x(a.width, a.is_signed);
        if (minus_one)
            return bp[0] & 1 ? llg_gmp_sv4_clone(&a)
                             : llg_gmp_sv4_from_u64(1, a.width, a.is_signed);
        return llg_gmp_sv4_from_u64(one ? 1 : 0, a.width, a.is_signed);
    }
    if (!an)
        return llg_gmp_sv4_zero(0, a.is_signed);
    if (a.width <= 64) {
        uint64_t r = 1, base = ap[0];
        while (bn && bp[bn - 1] == 0)
            --bn;
        for (size_t i = 0; i < bn; ++i)
            for (unsigned bit = 0; bit < 64; ++bit) {
                if ((bp[i] >> bit) & 1)
                    r *= base;
                base *= base;
            }
        return llg_gmp_sv4_from_u64(r, a.width, a.is_signed);
    }
    size_t product = g4_product_words(an);
    uint64_t *scratch = llg_gmp_sv4_alloc(2u * an + product), *r = scratch, *base = r + an,
             *temp = base + an;
    r[0] = 1;
    memcpy(base, ap, an * 8u);
    while (bn && bp[bn - 1] == 0)
        --bn;
    for (size_t i = 0; i < bn; ++i) {
        uint64_t exponent = bp[i];
        unsigned limit = 64;
        if (i + 1 == bn) {
            limit = 0;
            uint64_t t = exponent;
            while (t) {
                ++limit;
                t >>= 1;
            }
        }
        for (unsigned bit = 0; bit < limit; ++bit) {
            if ((exponent >> bit) & 1) {
                llg_gmp_sv4_kernel_mul(temp, r, base, an);
                temp[an - 1] &= g4_topmask(a.width);
                memcpy(r, temp, an * 8u);
            }
            if (i + 1 != bn || bit + 1 != limit) {
                llg_gmp_sv4_kernel_mul(temp, base, base, an);
                temp[an - 1] &= g4_topmask(a.width);
                memcpy(base, temp, an * 8u);
            }
        }
    }
    /* Publish only the exact-width result, releasing the operation's scratch. */
    uint64_t* p = (uint64_t*)realloc(scratch, an * 8u);
    if (!p)
        llg_gmp_sv4_fail("allocation failed");
    g4_t out = LLG_GMP_SV4_EMPTY;
    out.width = a.width;
    out.is_signed = a.is_signed;
    out.data.wide.a = p;
    return out;
}
g4_t llg_gmp_sv4_clog2_wide(g4_t v) {
    if (llg_gmp_sv4_is_unknown(v))
        return llg_gmp_sv4_x(32, 0);
    size_t n = llg_gmp_sv4_words(v);
    const uint64_t* p = v.data.wide.a;
    while (n && !p[n - 1])
        --n;
    if (!n)
        return llg_gmp_sv4_from_u64(0, 32, 0);
    uint64_t top = p[n - 1];
    unsigned bit = 0;
    while (top >>= 1)
        ++bit;
    uint32_t r = (uint32_t)((n - 1) * 64 + bit);
    int extra = (p[n - 1] & (p[n - 1] - 1)) != 0;
    for (size_t i = 0; i + 1 < n; ++i)
        extra |= p[i] != 0;
    return llg_gmp_sv4_from_u64(r + (uint32_t)extra, 32, 0);
}
