#include "internal.h"

uint64_t gmp4_word(gmp4_t value, size_t word, unsigned plane) {
    if (plane > 2) g4_fail("invalid plane");
    if (word >= ((size_t)value.width + 63u) / 64u) return 0;
    uint64_t a, b;
    if (value.width <= 64) { a = value.data.small.a; b = value.data.small.b; }
    else {
#if GMP_NUMB_BITS == 64
        a = g4_limb(value, word, 0); b = g4_limb(value, word, 1);
#else
        a = (uint64_t)g4_limb(value, 2u * word, 0) |
            ((uint64_t)g4_limb(value, 2u * word + 1u, 0) << 32);
        b = (uint64_t)g4_limb(value, 2u * word, 1) |
            ((uint64_t)g4_limb(value, 2u * word + 1u, 1) << 32);
#endif
    }
    return plane == 0 ? a & ~b : plane == 1 ? a & b : ~a & b;
}
uint64_t gmp4_to_u64(gmp4_t value) { return gmp4_word(value, 0, 0); }
unsigned gmp4_get_bit(gmp4_t value, uint64_t bit) {
    if (bit >= value.width) return 2;
    size_t limb = (size_t)(bit / GMP_NUMB_BITS);
    unsigned shift = (unsigned)(bit % GMP_NUMB_BITS);
    unsigned a = (unsigned)((g4_limb(value, limb, 0) >> shift) & 1u);
    unsigned b = (unsigned)((g4_limb(value, limb, 1) >> shift) & 1u);
    return b ? a ? 2u : 3u : a;
}
void gmp4_set_bit(gmp4_t *value, uint64_t bit, unsigned state) {
    if (state > 3) g4_fail("invalid scalar state");
    if (bit >= value->width) return;
    if (state >= 2) g4_promote(value);
    size_t limb = (size_t)(bit / GMP_NUMB_BITS);
    mp_limb_t mask = (mp_limb_t)1u << (bit % GMP_NUMB_BITS);
    mp_limb_t a = g4_limb(*value, limb, 0) & ~mask;
    mp_limb_t b = g4_limb(*value, limb, 1) & ~mask;
    if (state == 1 || state == 2) a |= mask;
    if (state >= 2) b |= mask;
    g4_put(value, limb, 0, a); g4_put(value, limb, 1, b);
}
void gmp4_set_word(gmp4_t *value, size_t word,
                   uint64_t bits, uint64_t x, uint64_t z) {
    if (word >= ((size_t)value->width + 63u) / 64u) return;
    uint32_t remaining = value->width - (uint32_t)(word * 64u);
    uint64_t mask = g4_mask64(remaining);
    x &= mask;
    z &= mask & ~x;
    uint64_t b = x | z;
    uint64_t a = (bits & mask & ~b) | x;
    if (b) g4_promote(value);
    size_t first = word * (64u / GMP_NUMB_BITS);
    for (size_t i = 0; i < 64u / GMP_NUMB_BITS; ++i) {
        if (first + i >= g4_count(value->width)) break;
        unsigned shift = (unsigned)(i * GMP_NUMB_BITS);
        g4_put(value, first + i, 0, (mp_limb_t)(a >> shift));
        g4_put(value, first + i, 1, (mp_limb_t)(b >> shift));
    }
}
int gmp4_is_unknown(gmp4_t value) {
    if (value.width <= 64) return value.data.small.b != 0;
    if (!value.data.wide.b) return 0;
    for (size_t i = 0; i < g4_count(value.width); ++i)
        if (value.data.wide.b[i]) return 1;
    return 0;
}
int g4_truth(gmp4_t value) {
    int unknown = 0;
    for (size_t i = 0; i < g4_count(value.width); ++i) {
        mp_limb_t a = g4_limb(value, i, 0), b = g4_limb(value, i, 1);
        if (a & ~b) return 1;
        unknown |= b != 0;
    }
    return unknown ? 2 : 0;
}
int gmp4_to_bool(gmp4_t value) { return g4_truth(value) == 1; }
int gmp4_same(gmp4_t a, gmp4_t b) {
    size_t n = g4_count(g4_max_width(a, b));
    for (size_t i = 0; i < n; ++i)
        if (g4_limb(a, i, 0) != g4_limb(b, i, 0) ||
            g4_limb(a, i, 1) != g4_limb(b, i, 1)) return 0;
    return 1;
}
static gmp4_t resize_extended(gmp4_t value, uint32_t width, int8_t sign, int extend) {
    gmp4_t result = g4_new(width, sign, g4_has_b(value));
    for (size_t i = 0; i < g4_count(width); ++i) {
        g4_put(&result, i, 0, g4_extended(value, width, extend, i, 0));
        g4_put(&result, i, 1, g4_extended(value, width, extend, i, 1));
    }
    return result;
}
gmp4_t gmp4_resize(gmp4_t value, uint32_t width, int8_t sign) {
    return resize_extended(value, width, sign, sign != 0);
}
gmp4_t gmp4_cast(gmp4_t value, uint32_t width, int8_t sign) {
    return resize_extended(value, width, sign, value.is_signed);
}
gmp4_t gmp4_to_two_state(gmp4_t value) {
    gmp4_t result = gmp4_zero(value.width, value.is_signed);
    for (size_t i = 0; i < g4_count(value.width); ++i)
        g4_put(&result, i, 0, g4_limb(value, i, 0) & ~g4_limb(value, i, 1));
    return result;
}
