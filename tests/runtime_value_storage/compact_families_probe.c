#include "backend.h"
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "compact_families_dispatch.h"
#include "compact_families_compact_dispatch.h"

#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "family check %s:%d: %s\n", __FILE__, __LINE__, #c);                   \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
static uint64_t checks;
static void compare(sv4_t old, g4_t value, unsigned op) {
    CHECK(old.width == value.width && old.is_signed == value.is_signed);
    int unknown = 0;
    for (size_t i = 0; i < llg_sv4_words(old); ++i) {
        for (unsigned plane = 0; plane < 3; ++plane)
            if (llg_sv4_word(old, i, plane) != llg_gmp_sv4_word(value, i, plane)) {
                fprintf(stderr, "family mismatch op=%u width=%u word=%zu plane=%u\n", op,
                        value.width, i, plane);
                abort();
            }
        unknown |= (old.x[i] | old.z[i]) != 0;
    }
    CHECK(llg_gmp_sv4_is_unknown(value) == unknown);
    CHECK(llg_gmp_sv4_bytes(&value) ==
          (value.width <= 64 ? 0 : 8u * llg_gmp_sv4_words(value) * (unknown ? 2u : 1u)));
    ++checks;
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&value);
    llg_gmp_sv4_destroy(&value);
}
static unsigned digit(unsigned code, unsigned width, unsigned index, int sign) {
    if (index < width)
        return (code >> (2u * index)) & 3u;
    return sign && width ? (code >> (2u * (width - 1u))) & 3u : 0;
}
static unsigned number(unsigned code, unsigned width, int* unknown) {
    unsigned result = 0;
    *unknown = 0;
    for (unsigned i = 0; i < width; ++i) {
        unsigned d = digit(code, width, i, 0);
        if (d >= 2)
            *unknown = 1;
        else
            result |= d << i;
    }
    return result;
}
static int truth(unsigned code, unsigned width) {
    int unknown;
    unsigned n = number(code, width, &unknown);
    return n ? 1 : unknown ? 2 : 0;
}
static int relation(unsigned a, unsigned aw, int sa, unsigned b, unsigned bw, int sb, int ge) {
    int au, bu;
    int av = (int)number(a, aw, &au), bv = (int)number(b, bw, &bu);
    if (au || bu)
        return 2;
    if (sa && sb) {
        if (aw && (av & (1 << (aw - 1))))
            av -= 1 << aw;
        if (bw && (bv & (1 << (bw - 1))))
            bv -= 1 << bw;
    }
    return ge ? av >= bv : av <= bv;
}
static unsigned oracle(unsigned op, unsigned a, unsigned aw, int sa, unsigned b, unsigned bw,
                       int sb, unsigned c, int sc) {
    if (op < 4) {
        int unknown;
        unsigned count = number(b, bw, &unknown), result = 0;
        for (unsigned i = 0; i < aw; ++i) {
            unsigned state = 0;
            if (unknown)
                state = 2;
            else if (op == 0 || op == 2)
                state = i >= count ? digit(a, aw, i - count, 0) : 0;
            else
                state = digit(a, aw, i + count, op == 3 && sa);
            result |= state << (2u * i);
        }
        return result;
    }
    if (op <= 12) {
        unsigned ones = 0, zeros = 0, unknown = 0;
        for (unsigned i = 0; i < aw; ++i) {
            unsigned d = digit(a, aw, i, 0);
            ones += d == 1;
            zeros += d == 0;
            unknown |= d >= 2;
        }
        if (op == 10)
            return ones;
        if (op == 11 || op == 12)
            return ones == 1 || (op == 12 && !ones);
        unsigned t = op < 6    ? zeros     ? 0
                                 : unknown ? 2
                                           : 1
                     : op < 8  ? ones      ? 1
                                 : unknown ? 2
                                           : 0
                     : unknown ? 2
                               : ones % 2;
        return op % 2 && t != 2 ? !t : t;
    }
    if (op <= 16) {
        unsigned width = aw > bw ? aw : bw;
        int sign = op >= 15 && sa && sb, unknown = 0;
        for (unsigned i = 0; i < width; ++i) {
            unsigned x = digit(a, aw, i, sign), y = digit(b, bw, i, sign);
            if (op == 13 && (x >= 2 || y >= 2))
                continue;
            if (op == 14 && (x == 3 || y == 3))
                continue;
            if (op >= 15 && y >= 2)
                continue;
            if (op >= 15 && x >= 2) {
                unknown = 1;
                continue;
            }
            if (x != y)
                return op == 16;
        }
        return unknown ? 2 : op != 16;
    }
    if (op <= 18) {
        int x = truth(a, aw), y = truth(b, bw);
        return op == 17           ? x == 0 || y == 1   ? 1
                                    : x == 1 && y == 0 ? 0
                                                       : 2
               : x == 2 || y == 2 ? 2
                                  : x == y;
    }
    int ge = relation(a, aw, sa, b, bw, sb, 1), le = relation(a, aw, sa, c, bw, sc, 0);
    return !ge || !le ? 0 : ge == 1 && le == 1 ? 1 : 2;
}
static void masks(unsigned code, unsigned width, uint64_t* bits, uint64_t* x, uint64_t* z) {
    *bits = *x = *z = 0;
    for (unsigned i = 0; i < width; ++i) {
        unsigned d = digit(code, width, i, 0);
        if (d == 1)
            *bits |= UINT64_C(1) << i;
        if (d == 2)
            *x |= UINT64_C(1) << i;
        if (d == 3)
            *z |= UINT64_C(1) << i;
    }
}
static void expected(g4_t value, unsigned op, unsigned code, unsigned width, int sign) {
    CHECK(value.width == width && value.is_signed == sign);
    if (op == 10)
        CHECK(llg_gmp_sv4_to_u64(value) == code);
    else
        for (unsigned i = 0; i < width; ++i)
            CHECK(llg_gmp_sv4_state(value, i) == digit(code, width, i, 0));
}
static void exhaustive(void) {
    for (unsigned aw = 0; aw <= 4; ++aw)
        for (unsigned a = 0; a < (1u << (2u * aw)); ++a) {
            uint64_t bits, x, z;
            masks(a, aw, &bits, &x, &z);
            for (int sa = 0; sa < 2; ++sa) {
                masks(a, aw, &bits, &x, &z);
                sv4_t olda = sv4_from_masks(bits, x, z, aw, (int8_t)sa);
                g4_t va = llg_gmp_sv4_from_masks(bits, x, z, aw, (int8_t)sa);
                for (unsigned op = 4; op <= 12; ++op) {
                    g4_t result = compact_eval(op, va, va, va);
                    expected(result, op, oracle(op, a, aw, sa, 0, 0, 0, 0, 0), op == 10 ? 32 : 1,
                             op == 10);
                    compare(evaluate(op, olda, olda, olda), result, op);
                }
                for (unsigned bw = 0; bw <= 4; ++bw)
                    for (unsigned b = 0; b < (1u << (2u * bw)); ++b) {
                        masks(b, bw, &bits, &x, &z);
                        for (int sb = 0; sb < 2; ++sb) {
                            sv4_t oldb = sv4_from_masks(bits, x, z, bw, (int8_t)sb);
                            g4_t vb = llg_gmp_sv4_from_masks(bits, x, z, bw, (int8_t)sb);
                            for (unsigned op = 0; op < 19; ++op) {
                                if (op >= 4 && op <= 12)
                                    continue;
                                g4_t result = compact_eval(op, va, vb, va);
                                expected(result, op, oracle(op, a, aw, sa, b, bw, sb, 0, 0),
                                         op < 4 ? aw : 1, op < 4 ? sa : 0);
                                compare(evaluate(op, olda, oldb, olda), result, op);
                            }
                            sv4_destroy(&oldb);
                        }
                    }
                sv4_destroy(&olda);
            }
        }
}
static void exhaustive_range(void) {
    for (unsigned width = 0; width <= 4; ++width) {
        unsigned size = 1u << (2u * width);
        sv4_t* old = (sv4_t*)calloc(2u * size, sizeof(sv4_t));
        g4_t* values = (g4_t*)calloc(2u * size, sizeof(g4_t));
        CHECK(old && values);
        for (unsigned i = 0; i < 2u * size; ++i) {
            uint64_t bits, x, z;
            masks(i % size, width, &bits, &x, &z);
            old[i] = sv4_from_masks(bits, x, z, width, (int8_t)(i / size));
            values[i] = llg_gmp_sv4_from_masks(bits, x, z, width, (int8_t)(i / size));
        }
        for (unsigned signs = 0; signs < 8; ++signs) {
            int sa = (signs & 1) != 0, sb = (signs & 2) != 0, sc = (signs & 4) != 0;
            unsigned ai = (unsigned)sa * size, bi = (unsigned)sb * size, ci = (unsigned)sc * size;
            for (unsigned a = 0; a < size; ++a)
                for (unsigned b = 0; b < size; ++b)
                    for (unsigned c = 0; c < size; ++c) {
                        g4_t result = llg_gmp_sv4_inside_range(values[ai + a], values[bi + b],
                                                               values[ci + c]);
                        expected(result, 19, oracle(19, a, width, sa, b, width, sb, c, sc), 1, 0);
                        compare(sv4_inside_range(old[ai + a], old[bi + b], old[ci + c]), result,
                                19);
                    }
        }
        sv4_destroy_array(old, 2u * size);
        free(old);
        free(values);
    }
}
static uint64_t random_word(void) {
    static uint64_t seed = UINT64_C(0x42eb510dc1833c77);
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}
static void wide(void) {
    const uint32_t widths[] = {
        1,   2,   7,    8,    31,   32,    33,
        63,  64,  65,   127,  128,  129,   255,
        256, 257, 1023, 1024, 4096, 16384, LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1};
    for (size_t wi = 0; wi < sizeof(widths) / sizeof(widths[0]); ++wi) {
        uint32_t width = widths[wi],
                 other = widths[(wi + 1) % (sizeof(widths) / sizeof(widths[0]))];
        size_t n = ((size_t)(width > other ? width : other) + 63u) / 64u;
        uint64_t* p = (uint64_t*)calloc(6u * n, sizeof(uint64_t));
        CHECK(p);
        for (unsigned pattern = 0; pattern < 10; ++pattern) {
            for (size_t i = 0; i < n; ++i) {
                p[i] = pattern == 0   ? 0
                       : pattern == 1 ? UINT64_MAX
                       : pattern == 2 ? UINT64_C(0xaaaaaaaaaaaaaaaa)
                                      : random_word();
                p[3 * n + i] = pattern < 3 ? p[i] : random_word();
                p[n + i] = pattern >= 4 ? random_word() & random_word() : 0;
                p[2 * n + i] = pattern >= 5 ? random_word() & ~p[n + i] : 0;
                p[4 * n + i] = pattern >= 6 ? random_word() & random_word() : 0;
                p[5 * n + i] = pattern >= 7 ? random_word() & ~p[4 * n + i] : 0;
                p[i] &= ~(p[n + i] | p[2 * n + i]);
                p[3 * n + i] &= ~(p[4 * n + i] | p[5 * n + i]);
            }
            if (pattern == 3) {
                memset(p, 0, 3 * n * 8u);
                p[(width - 1u) / 64u] = UINT64_C(1) << ((width - 1u) % 64u);
            }
            for (int sa = 0; sa < 2; ++sa)
                for (int sb = 0; sb < 2; ++sb) {
                    sv4_t olda = sv4_from_limbs(p, p + n, p + 2 * n, width, (int8_t)sa),
                          oldb = sv4_from_limbs(p + 3 * n, p + 4 * n, p + 5 * n, other, (int8_t)sb);
                    g4_t a = llg_gmp_sv4_from_limbs(p, p + n, p + 2 * n, width, (int8_t)sa),
                         b = llg_gmp_sv4_from_limbs(p + 3 * n, p + 4 * n, p + 5 * n, other,
                                                    (int8_t)sb);
                    for (unsigned op = 4; op < 20; ++op) {
                        compare(evaluate(op, olda, oldb, olda), compact_eval(op, a, b, a), op);
                        compare(evaluate(op, oldb, olda, oldb), compact_eval(op, b, a, b), op);
                    }
                    const uint64_t counts[] = {
                        0, 1, 2, 63, 64, 65, width - 1u, width, (uint64_t)width + 1u, UINT64_MAX};
                    for (unsigned ci = 0; ci < sizeof(counts) / sizeof(counts[0]); ++ci)
                        for (unsigned cs = 0; cs < 4; ++cs) {
                            uint32_t cw = cs == 0 ? 64 : cs == 1 ? 65 : cs == 2 ? 129 : width;
                            sv4_t oldc = sv4_from_u64(counts[ci], cw, (int8_t)sb);
                            g4_t c = llg_gmp_sv4_from_u64(counts[ci], cw, (int8_t)sb);
                            if (cs == 2) {
                                llg_sv4_set_state(&oldc, 128, 1);
                                llg_gmp_sv4_set_state(&c, 128, 1);
                            }
                            for (unsigned op = 0; op < 4; ++op)
                                compare(evaluate(op, olda, oldc, olda), compact_eval(op, a, c, a),
                                        op);
                            sv4_destroy(&oldc);
                            llg_gmp_sv4_destroy(&c);
                        }
                    for (unsigned state = 2; state < 4; ++state) {
                        sv4_t oldc = sv4_zero(129, 0);
                        g4_t c = llg_gmp_sv4_zero(129, 0);
                        llg_sv4_set_state(&oldc, 128, (uint8_t)state);
                        llg_gmp_sv4_set_state(&c, 128, (uint8_t)state);
                        for (unsigned op = 0; op < 4; ++op)
                            compare(evaluate(op, olda, oldc, olda), compact_eval(op, a, c, a), op);
                        sv4_destroy(&oldc);
                        llg_gmp_sv4_destroy(&c);
                    }
                    for (unsigned op = 0; op < 20; ++op) {
                        g4_t dst = llg_gmp_sv4_clone(&a),
                             independent = compact_eval(op, dst, b, dst);
                        uint8_t before = llg_gmp_sv4_state(a, 0);
                        if (independent.width)
                            llg_gmp_sv4_set_state(&independent, 0, (before + 1u) % 4u);
                        CHECK(llg_gmp_sv4_state(a, 0) == before);
                        llg_gmp_sv4_destroy(&independent);
                        llg_gmp_sv4_replace(&dst, compact_eval(op, dst, b, dst));
                        compare(evaluate(op, olda, oldb, olda), dst, op);
                    }
                    sv4_destroy(&olda);
                    sv4_destroy(&oldb);
                    llg_gmp_sv4_destroy(&a);
                    llg_gmp_sv4_destroy(&b);
                }
        }
        free(p);
    }
    for (unsigned top = 0; top < 4; ++top) {
        sv4_t old = sv4_zero(65, 1);
        g4_t v = llg_gmp_sv4_zero(65, 1);
        llg_sv4_set_state(&old, 64, (uint8_t)top);
        llg_gmp_sv4_set_state(&v, 64, (uint8_t)top);
        sv4_t oldc = sv4_from_u64(64, 8, 0);
        g4_t c = llg_gmp_sv4_from_u64(64, 8, 0);
        compare(sv4_ashr(old, oldc), llg_gmp_sv4_ashr(v, c), 3);
        compare(sv4_shl(old, oldc), llg_gmp_sv4_shl(v, c), 0);
        sv4_destroy(&old);
        sv4_destroy(&oldc);
        llg_gmp_sv4_destroy(&v);
    }
}
int main(int argc, char** argv) {
    CHECK(argc == 2);
    if (!strcmp(argv[1], "exhaustive"))
        exhaustive();
    else if (!strcmp(argv[1], "ranges"))
        exhaustive_range();
    else if (!strcmp(argv[1], "wide"))
        wide();
    else
        return 1;
    printf("families %s: %llu checks\n", argv[1], (unsigned long long)checks);
    return 0;
}
