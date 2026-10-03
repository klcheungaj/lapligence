#include "backend.h"
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "destination check %d: %s\n", __LINE__, #c);                           \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
static void (*old_ops[])(sv4_t*, sv4_t, sv4_t) = {sv4_add_into, sv4_sub_into, sv4_mul_into};
static void (*new_ops[])(g4_t*, g4_t, g4_t) = {llg_gmp_sv4_add_into, llg_gmp_sv4_sub_into,
                                               llg_gmp_sv4_mul_into};
static uint64_t rng = UINT64_C(0x39d7a631028dc52b);
static uint64_t random_word(void) {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    return rng;
}
static void same(sv4_t a, g4_t b) {
    CHECK(a.width == b.width && a.is_signed == b.is_signed);
    for (size_t i = 0; i < llg_sv4_words(a); ++i)
        for (unsigned p = 0; p < 3; ++p)
            CHECK(llg_sv4_word(a, i, p) == llg_gmp_sv4_word(b, i, p));
    size_t bytes =
        b.width <= 64 ? 0 : llg_gmp_sv4_words(b) * 8u * (llg_gmp_sv4_is_unknown(b) ? 2u : 1u);
    CHECK(llg_gmp_sv4_bytes(&b) == bytes);
    if (b.width > 64) {
        unsigned tail = b.width % 64;
        uint64_t padding = tail ? UINT64_MAX << tail : 0;
        size_t top = llg_gmp_sv4_words(b) - 1;
        CHECK(!(b.data.wide.a[top] & padding));
        CHECK(!b.data.wide.b || !(b.data.wide.b[top] & padding));
    }
}
static uint64_t extend_small(sv4_t v, uint32_t width, int sign) {
    uint64_t result = sv4_to_u64(v);
    if (sign && v.width && (result & (UINT64_C(1) << (v.width - 1))))
        result |= UINT64_MAX << v.width;
    return result & (width ? (UINT64_C(1) << width) - 1 : 0);
}
static void small_oracle(sv4_t a, sv4_t b, unsigned op, g4_t result) {
    uint32_t width = a.width > b.width ? a.width : b.width;
    int sign = a.is_signed && b.is_signed;
    int unknown = sv4_is_unknown(a) || sv4_is_unknown(b);
    uint64_t x = extend_small(a, width, sign), y = extend_small(b, width, sign);
    uint64_t expected = op == 0 ? x + y : op == 1 ? x - y : x * y;
    for (uint32_t bit = 0; bit < width; ++bit)
        CHECK(llg_gmp_sv4_state(result, bit) == (unknown ? 2u : (unsigned)((expected >> bit) & 1)));
}
static void exercise(sv4_t a, sv4_t b, g4_t x, g4_t y, unsigned op, int small) {
    for (unsigned mode = 0; mode < 7; ++mode) {
        sv4_t left = sv4_clone(&a), right = sv4_clone(&b), dest = SV4_EMPTY;
        g4_t lhs = llg_gmp_sv4_clone(&x), rhs = llg_gmp_sv4_clone(&y), out = LLG_GMP_SV4_EMPTY;
        sv4_t* target = &dest;
        g4_t* compact = &out;
        if (mode == 1) {
            dest = sv4_zero(a.width > b.width ? a.width : b.width, 1);
            out = llg_gmp_sv4_zero(dest.width, 1);
        } else if (mode == 2) {
            dest = sv4_fill(3, a.width > b.width ? a.width : b.width, 1);
            out = llg_gmp_sv4_fill(3, dest.width, 1);
        } else if (mode == 3 || mode == 5) {
            target = &left;
            compact = &lhs;
        } else if (mode == 4) {
            target = &right;
            compact = &rhs;
        } else if (mode == 6) {
            dest = sv4_x(67, 1);
            out = llg_gmp_sv4_x(67, 1);
        }
        sv4_t input_b = mode == 5 || mode == 6 ? left : right;
        g4_t input_y = mode == 5 || mode == 6 ? lhs : rhs;
        sv4_t expected = op == 0   ? sv4_add(left, input_b)
                         : op == 1 ? sv4_sub(left, input_b)
                                   : sv4_mul(left, input_b);
        old_ops[op](target, left, input_b);
        new_ops[op](compact, lhs, input_y);
        same(expected, *compact);
        same(*target, *compact);
        if (small)
            small_oracle(a, mode == 5 || mode == 6 ? a : b, op, *compact);
        if (mode != 3 && mode != 5)
            same(a, lhs);
        if (mode != 4)
            same(b, rhs);
        sv4_destroy(&expected);
        if (target != &left)
            sv4_destroy(&left);
        if (target != &right)
            sv4_destroy(&right);
        if (compact != &lhs)
            llg_gmp_sv4_destroy(&lhs);
        if (compact != &rhs)
            llg_gmp_sv4_destroy(&rhs);
        same(*target, *compact);
        sv4_destroy(target);
        llg_gmp_sv4_destroy(compact);
    }
}
static sv4_t pattern(unsigned code, uint32_t width, int sign) {
    sv4_t v = sv4_zero(width, sign);
    for (uint32_t bit = 0; bit < width; ++bit) {
        llg_sv4_set_state(&v, bit, code & 3);
        code >>= 2;
    }
    return v;
}
static g4_t import(sv4_t v) {
    return llg_gmp_sv4_from_limbs(v.bits, v.x, v.z, v.width, v.is_signed);
}
static void exhaustive(void) {
    for (uint32_t aw = 0; aw <= 4; ++aw)
        for (uint32_t bw = 0; bw <= 4; ++bw)
            for (unsigned ac = 0; ac < (1u << (2 * aw)); ++ac)
                for (unsigned bc = 0; bc < (1u << (2 * bw)); ++bc)
                    for (unsigned signs = 0; signs < 4; ++signs) {
                        sv4_t a = pattern(ac, aw, signs & 1), b = pattern(bc, bw, signs >> 1);
                        g4_t x = import(a), y = import(b);
                        for (unsigned op = 0; op < 3; ++op)
                            exercise(a, b, x, y, op, 1);
                        sv4_destroy(&a);
                        sv4_destroy(&b);
                        llg_gmp_sv4_destroy(&x);
                        llg_gmp_sv4_destroy(&y);
                    }
}
static void wide(void) {
    uint32_t widths[] = {1,   63,  64,   65,   66,   127,  128,  129,  255,
                         256, 257, 1024, 4096, 8128, 8129, 8192, 8193, 1048575};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (unsigned shape = 0; shape < 3; ++shape)
            for (unsigned state = 0; state < 4; ++state)
                for (unsigned signs = 0; signs < 4; ++signs) {
                    uint32_t aw = widths[k], bw = shape == 0 ? aw : shape == 1 ? aw - 1 : 4;
                    sv4_t a = sv4_zero(aw, signs & 1), b = sv4_zero(bw, signs >> 1);
                    for (size_t i = 0; i < llg_sv4_words(a); ++i)
                        llg_sv4_set_word(&a, i, random_word(), 0, 0);
                    for (size_t i = 0; i < llg_sv4_words(b); ++i)
                        llg_sv4_set_word(&b, i, random_word(), 0, 0);
                    if (state == 1)
                        llg_sv4_set_state(&a, aw - 1, 2);
                    if (state == 2 && bw)
                        llg_sv4_set_state(&b, bw - 1, 3);
                    if (state == 3) {
                        llg_sv4_set_state(&a, 0, 3);
                        if (bw)
                            llg_sv4_set_state(&b, 0, 2);
                    }
                    g4_t x = import(a), y = import(b);
                    for (unsigned op = 0; op < 3; ++op)
                        if (op != 2 || aw < 1048575 || sv4_is_unknown(a))
                            exercise(a, b, x, y, op, 0);
                    sv4_destroy(&a);
                    sv4_destroy(&b);
                    llg_gmp_sv4_destroy(&x);
                    llg_gmp_sv4_destroy(&y);
                }
}
int main(int argc, char** argv) {
    CHECK(argc == 2);
    if (!strcmp(argv[1], "exhaustive"))
        exhaustive();
    else if (!strcmp(argv[1], "wide"))
        wide();
    else
        return 1;
    puts("arithmetic destinations: oracle, differential, aliases and ownership passed");
    return 0;
}
