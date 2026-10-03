#include "internal.h"

static gmp4_t bitwise(gmp4_t a, gmp4_t b, unsigned operation) {
    uint32_t width = g4_max_width(a, b);
    int8_t sign = a.is_signed && b.is_signed;
    int unknown_storage = g4_has_b(a) || g4_has_b(b);
    gmp4_t result = g4_new(width, sign, unknown_storage);
    size_t n = g4_count(width);
    if (width > 64 && !unknown_storage && a.width == width && b.width == width) {
        if (operation == 0)
            mpn_and_n(result.data.wide.a, a.data.wide.a, b.data.wide.a, (mp_size_t)n);
        else if (operation == 1)
            mpn_ior_n(result.data.wide.a, a.data.wide.a, b.data.wide.a, (mp_size_t)n);
        else if (operation == 2)
            mpn_xor_n(result.data.wide.a, a.data.wide.a, b.data.wide.a, (mp_size_t)n);
        else mpn_xnor_n(result.data.wide.a, a.data.wide.a, b.data.wide.a, (mp_size_t)n);
        result.data.wide.a[n - 1u] &= g4_mask(width, n - 1u);
        return result;
    }
    for (size_t i = 0; i < n; ++i) {
        mp_limb_t aa = g4_extended(a, width, sign, i, 0);
        mp_limb_t ab = g4_extended(a, width, sign, i, 1);
        mp_limb_t ba = g4_extended(b, width, sign, i, 0);
        mp_limb_t bb = g4_extended(b, width, sign, i, 1);
        mp_limb_t known_one, unknown;
        if (operation == 0) {
            mp_limb_t zero = (~aa & ~ab) | (~ba & ~bb);
            known_one = (aa & ~ab) & (ba & ~bb);
            unknown = ~zero & ~known_one;
        } else if (operation == 1) {
            known_one = (aa & ~ab) | (ba & ~bb);
            mp_limb_t zero = (~aa & ~ab) & (~ba & ~bb);
            unknown = ~zero & ~known_one;
        } else {
            unknown = ab | bb;
            known_one = (operation == 2 ? aa ^ ba : ~(aa ^ ba)) & ~unknown;
        }
        /* Bitwise operations produce X, never Z, at unknown result bits. */
        g4_put(&result, i, 0, known_one | unknown);
        g4_put(&result, i, 1, unknown);
    }
    return result;
}
gmp4_t gmp4_and(gmp4_t a, gmp4_t b) { return bitwise(a, b, 0); }
gmp4_t gmp4_or(gmp4_t a, gmp4_t b) { return bitwise(a, b, 1); }
gmp4_t gmp4_xor(gmp4_t a, gmp4_t b) { return bitwise(a, b, 2); }
gmp4_t gmp4_xnor(gmp4_t a, gmp4_t b) { return bitwise(a, b, 3); }
gmp4_t gmp4_bitneg(gmp4_t value) {
    gmp4_t result = g4_new(value.width, value.is_signed, g4_has_b(value));
    for (size_t i = 0; i < g4_count(value.width); ++i) {
        mp_limb_t b = g4_limb(value, i, 1);
        g4_put(&result, i, 0, ~g4_limb(value, i, 0) | b);
        g4_put(&result, i, 1, b);
    }
    return result;
}
static gmp4_t equal(gmp4_t a, gmp4_t b, int literal) {
    uint32_t width = g4_max_width(a, b);
    int sign = a.is_signed && b.is_signed, unknown = 0;
    for (size_t i = 0; i < g4_count(width); ++i) {
        mp_limb_t aa = g4_extended(a, width, sign, i, 0);
        mp_limb_t ab = g4_extended(a, width, sign, i, 1);
        mp_limb_t ba = g4_extended(b, width, sign, i, 0);
        mp_limb_t bb = g4_extended(b, width, sign, i, 1);
        if (literal ? aa != ba || ab != bb : ((aa ^ ba) & ~(ab | bb)) != 0)
            return gmp4_from_u64(0, 1, 0);
        unknown |= (ab | bb) != 0;
    }
    return unknown && !literal ? gmp4_x(1, 0) : gmp4_from_u64(1, 1, 0);
}
gmp4_t gmp4_eq(gmp4_t a, gmp4_t b) { return equal(a, b, 0); }
gmp4_t gmp4_case_eq(gmp4_t a, gmp4_t b) { return equal(a, b, 1); }
gmp4_t gmp4_lognot(gmp4_t value) {
    int truth = g4_truth(value);
    return truth == 2 ? gmp4_x(1, 0) : gmp4_from_u64(truth == 0, 1, 0);
}
gmp4_t gmp4_logand(gmp4_t a, gmp4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return !x || !y ? gmp4_zero(1, 0) :
        x == 1 && y == 1 ? gmp4_from_u64(1, 1, 0) : gmp4_x(1, 0);
}
gmp4_t gmp4_logor(gmp4_t a, gmp4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return x == 1 || y == 1 ? gmp4_from_u64(1, 1, 0) :
        !x && !y ? gmp4_zero(1, 0) : gmp4_x(1, 0);
}
gmp4_t gmp4_mux(gmp4_t condition, gmp4_t a, gmp4_t b) {
    uint32_t width = g4_max_width(a, b);
    int8_t sign = a.is_signed && b.is_signed;
    int truth = g4_truth(condition);
    if (truth < 2) return gmp4_resize(truth ? a : b, width, sign);
    gmp4_t result = g4_new(width, sign, 1);
    for (size_t i = 0; i < g4_count(width); ++i) {
        mp_limb_t aa = g4_extended(a, width, sign, i, 0);
        mp_limb_t ba = g4_extended(b, width, sign, i, 0);
        /* Supplied 1800-2009 Table 11-20: Z/Z also merges to X. */
        mp_limb_t unknown = g4_extended(a, width, sign, i, 1) |
                            g4_extended(b, width, sign, i, 1) | (aa ^ ba);
        g4_put(&result, i, 0, (aa & ba) | unknown);
        g4_put(&result, i, 1, unknown);
    }
    return result;
}
