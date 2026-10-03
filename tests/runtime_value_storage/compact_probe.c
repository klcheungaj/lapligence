#include "llg_value.h"
#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "compact check failed at %s:%d: %s\n", __FILE__, __LINE__, #c);        \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
typedef sv4_t (*old_binary)(sv4_t, sv4_t);
typedef g4_t (*new_binary)(g4_t, g4_t);
static old_binary old_ops[] = {sv4_add, sv4_sub, sv4_mul,     sv4_div,      sv4_mod,
                               sv4_pow, sv4_and, sv4_or,      sv4_xor,      sv4_xnor,
                               sv4_eq,  sv4_neq, sv4_case_eq, sv4_case_neq, sv4_lt,
                               sv4_le,  sv4_gt,  sv4_ge,      sv4_logand,   sv4_logor};
static new_binary new_ops[] = {
    llg_gmp_sv4_add, llg_gmp_sv4_sub, llg_gmp_sv4_mul,     llg_gmp_sv4_div,      llg_gmp_sv4_mod,
    llg_gmp_sv4_pow, llg_gmp_sv4_and, llg_gmp_sv4_or,      llg_gmp_sv4_xor,      llg_gmp_sv4_xnor,
    llg_gmp_sv4_eq,  llg_gmp_sv4_neq, llg_gmp_sv4_case_eq, llg_gmp_sv4_case_neq, llg_gmp_sv4_lt,
    llg_gmp_sv4_le,  llg_gmp_sv4_gt,  llg_gmp_sv4_ge,      llg_gmp_sv4_logand,   llg_gmp_sv4_logor};
static uint64_t rng = UINT64_C(0x6256bcde32904411);
static uint64_t random_word(void) {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    return rng;
}
static void compare_result(sv4_t old, g4_t v, const char* where) {
    if (old.width != v.width || old.is_signed != v.is_signed) {
        fprintf(stderr, "shape mismatch %s %u/%u %d/%d\n", where, old.width, v.width, old.is_signed,
                v.is_signed);
        abort();
    }
    size_t n = llg_sv4_words(old);
    int unknown = 0;
    for (size_t i = 0; i < n; ++i) {
        for (unsigned plane = 0; plane < 3; ++plane) {
            uint64_t x = llg_sv4_word(old, i, plane), y = llg_gmp_sv4_word(v, i, plane);
            if (x != y) {
                fprintf(stderr, "mismatch %s width=%u word=%zu plane=%u old=%llx new=%llx\n", where,
                        v.width, i, plane, (unsigned long long)x, (unsigned long long)y);
                abort();
            }
        }
        unknown |= (old.x[i] | old.z[i]) != 0;
    }
    CHECK(llg_gmp_sv4_is_unknown(v) == unknown);
    CHECK(llg_gmp_sv4_bytes(&v) == (v.width <= 64 ? 0 : 8u * n * (unknown ? 2u : 1u)));
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&v);
}
static void random_case(uint32_t aw, uint32_t bw, int signa, int signb, unsigned pattern) {
    size_t an = ((size_t)aw + 63) / 64, bn = ((size_t)bw + 63) / 64, n = an > bn ? an : bn;
    uint64_t* p = (uint64_t*)calloc(6 * n, sizeof(uint64_t));
    CHECK(p);
    uint64_t *a = p, *ax = a + n, *az = ax + n, *b = az + n, *bx = b + n, *bz = bx + n;
    for (size_t i = 0; i < n; ++i) {
        a[i] = random_word();
        b[i] = random_word();
        if (pattern == 0)
            a[i] = b[i] = 0;
        if (pattern == 1)
            a[i] = b[i] = UINT64_MAX;
        if (pattern == 2) {
            a[i] = UINT64_C(0xaaaaaaaaaaaaaaaa);
            b[i] = ~a[i];
        }
        if (pattern == 3) {
            a[i] = i + 1 == an ? UINT64_C(1) << ((aw - 1) % 64) : 0;
            b[i] = i ? 0 : 3;
        }
        if (pattern == 4) {
            a[i] = UINT64_MAX;
            b[i] = i ? 0 : 1;
        }
        if (pattern >= 6) {
            ax[i] = random_word() & random_word();
            az[i] = random_word() & ~ax[i];
            bx[i] = random_word() & random_word();
            bz[i] = random_word() & ~bx[i];
            a[i] &= ~(ax[i] | az[i]);
            b[i] &= ~(bx[i] | bz[i]);
        }
    }
    sv4_t oa = sv4_from_limbs(a, ax, az, aw, (int8_t)signa),
          ob = sv4_from_limbs(b, bx, bz, bw, (int8_t)signb);
    g4_t na = llg_gmp_sv4_from_limbs(a, ax, az, aw, (int8_t)signa),
         nb = llg_gmp_sv4_from_limbs(b, bx, bz, bw, (int8_t)signb);
    for (unsigned op = 0; op < sizeof(old_ops) / sizeof(old_ops[0]); ++op) {
        if (op == 5)
            continue;
        if (aw > 16384 && pattern < 6 && (op == 2))
            continue;
        char name[32];
        snprintf(name, sizeof(name), "op%u %u/%u", op, aw, bw);
        compare_result(old_ops[op](oa, ob), new_ops[op](na, nb), name);
    }
    compare_result(sv4_neg(oa), llg_gmp_sv4_neg(na), "neg");
    compare_result(sv4_bitneg(oa), llg_gmp_sv4_bitneg(na), "not");
    compare_result(sv4_lognot(oa), llg_gmp_sv4_lognot(na), "lognot");
    compare_result(sv4_clog2(oa), llg_gmp_sv4_clog2(na), "clog2");
    compare_result(sv4_resize(oa, bw, (int8_t)signb), llg_gmp_sv4_resize(na, bw, (int8_t)signb),
                   "resize");
    compare_result(sv4_cast(oa, bw, (int8_t)signb), llg_gmp_sv4_cast(na, bw, (int8_t)signb),
                   "cast");
    compare_result(sv4_to_two_state(oa), llg_gmp_sv4_to_two_state(na), "two-state");
    for (unsigned state = 0; state < 3; ++state) {
        sv4_t os = sv4_fill((uint8_t)state, 1, 0);
        g4_t ns = llg_gmp_sv4_fill((uint8_t)state, 1, 0);
        compare_result(sv4_mux(os, oa, ob), llg_gmp_sv4_mux(ns, na, nb), "mux");
        sv4_destroy(&os);
        llg_gmp_sv4_destroy(&ns);
    }
    CHECK(sv4_same(oa, ob) == llg_gmp_sv4_same(na, nb));
    CHECK(sv4_to_bool(oa) == llg_gmp_sv4_to_bool(na));
    CHECK(sv4_to_u64(oa) == llg_gmp_sv4_to_u64(na));
    CHECK(llg_sv4_has_x(oa) == llg_gmp_sv4_has_x(na));
    CHECK(llg_sv4_has_z(oa) == llg_gmp_sv4_has_z(na));
    g4_t clone = llg_gmp_sv4_clone(&na);
    llg_gmp_sv4_copy(&clone, &clone);
    llg_gmp_sv4_move(&clone, &clone);
    if (aw)
        llg_gmp_sv4_set_state(&clone, 0, (llg_gmp_sv4_state(na, 0) + 1) % 4);
    CHECK(llg_gmp_sv4_state(na, 0) == llg_sv4_state(oa, 0));
    llg_gmp_sv4_copy(&clone, &na);
    CHECK(llg_gmp_sv4_same(clone, na));
    llg_gmp_sv4_replace(&clone, llg_gmp_sv4_add(clone, nb));
    compare_result(sv4_add(oa, ob), llg_gmp_sv4_clone(&clone), "destination operand");
    llg_gmp_sv4_move(&clone, &na);
    CHECK(na.width == 0);
    llg_gmp_sv4_destroy(&clone);
    llg_gmp_sv4_destroy(&clone);
    llg_gmp_sv4_destroy(&na);
    llg_gmp_sv4_destroy(&nb);
    sv4_destroy(&oa);
    sv4_destroy(&ob);
    free(p);
}
static void ownership_and_bridge(void) {
    static const g4_t literal = LLG_GMP_SV4_LITERAL(9, 2, 4, 8, 1);
    CHECK(llg_gmp_sv4_state(literal, 1) == 2);
    CHECK(llg_gmp_sv4_state(literal, 2) == 3);
    uint32_t widths[] = {0, 1, 64, 65, 129, LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k) {
        uint32_t w = widths[k];
        size_t n = ((size_t)w + 63) / 64;
        g4_t v = llg_gmp_sv4_x(w, 7), known = llg_gmp_sv4_from_u64(42, w, 0);
        CHECK(v.is_signed == 1);
        llg_gmp_sv4_copy(&v, &known);
        CHECK(!llg_gmp_sv4_is_unknown(v));
        CHECK(llg_gmp_sv4_bytes(&v) == (w <= 64 ? 0 : 8 * n));
        llg_gmp_sv4_set_state(&v, w, 2);
        CHECK(!llg_gmp_sv4_is_unknown(v));
        CHECK(llg_gmp_sv4_state(v, w) == 2);
        if (w) {
            llg_gmp_sv4_set_state(&v, w - 1, 3);
            CHECK(llg_gmp_sv4_has_z(v));
            llg_gmp_sv4_set_state(&v, w - 1, 0);
            CHECK(!llg_gmp_sv4_is_unknown(v));
        }
        llg_gmp_sv4_vpi_word_t out[4], in[4] = {{UINT64_MAX, 3}, {3, 4}, {5, 0}, {0, 0}};
        llg_gmp_sv4_export_vpi_words(v, SIZE_MAX - 1, out, 4);
        for (size_t i = 0; i < 4; ++i)
            CHECK(out[i].aval == 0 && out[i].bval == 0);
        llg_gmp_sv4_import_vpi_words(&v, SIZE_MAX - 1, in, 4);
        CHECK(!llg_gmp_sv4_is_unknown(v));
        llg_gmp_sv4_import_vpi_words(&v, 0, in, 4);
        llg_gmp_sv4_export_vpi_words(v, 0, out, 4);
        for (size_t i = 0; i < 4; ++i) {
            uint64_t mask = i >= n       ? 0
                            : i + 1 == n ? LLG_GMP_MASK(w % 64 ? w % 64 : 64)
                                         : UINT64_MAX;
            CHECK(out[i].aval == (in[i].aval & mask));
            CHECK(out[i].bval == (in[i].bval & mask));
        }
        llg_gmp_sv4_word_t planes[4];
        llg_gmp_sv4_export_words(v, 0, planes, 4);
        g4_t roundtrip = llg_gmp_sv4_zero(w, 0);
        llg_gmp_sv4_import_words(&roundtrip, 0, planes, 4);
        CHECK(llg_gmp_sv4_same(v, roundtrip));
        llg_gmp_sv4_set_signed(&v, -1);
        CHECK(v.is_signed == 1);
        llg_gmp_sv4_import_words(&v, 0, NULL, 0);
        llg_gmp_sv4_export_words(v, 0, NULL, 0);
        llg_gmp_sv4_import_vpi_words(&v, 0, NULL, 0);
        llg_gmp_sv4_export_vpi_words(v, 0, NULL, 0);
        llg_gmp_sv4_destroy(&roundtrip);
        llg_gmp_sv4_destroy(&v);
        llg_gmp_sv4_destroy(&known);
    }
    for (unsigned state = 0; state < 4; ++state)
        CHECK(llg_gmp_sv4_state_from_dpi(llg_gmp_sv4_state_to_dpi(state)) == state);
}
static g4_t small_states(unsigned code, unsigned w, int sign) {
    uint64_t bits = 0, x = 0, z = 0;
    for (unsigned i = 0; i < w; ++i) {
        unsigned s = (code >> (2 * i)) & 3;
        if (s == 1)
            bits |= UINT64_C(1) << i;
        if (s == 2)
            x |= UINT64_C(1) << i;
        if (s == 3)
            z |= UINT64_C(1) << i;
    }
    return llg_gmp_sv4_from_masks(bits, x, z, w, (int8_t)sign);
}
static unsigned state_at(unsigned code, unsigned bit) { return (code >> (2 * bit)) & 3; }
static int truth_code(unsigned code, unsigned w) {
    int unknown = 0;
    for (unsigned i = 0; i < w; ++i) {
        unsigned s = state_at(code, i);
        if (s == 1)
            return 1;
        unknown |= s >= 2;
    }
    return unknown ? 2 : 0;
}
static void check_small(g4_t v, unsigned w, int sign, unsigned expected) {
    CHECK(v.width == w);
    CHECK(v.is_signed == sign);
    for (unsigned i = 0; i < w; ++i)
        CHECK(llg_gmp_sv4_state(v, i) == state_at(expected, i));
}
static unsigned known_code(unsigned n, unsigned w) {
    unsigned code = 0;
    for (unsigned i = 0; i < w; ++i)
        code |= ((n >> i) & 1) << (2 * i);
    return code;
}
static unsigned x_code(unsigned w) {
    unsigned c = 0;
    for (unsigned i = 0; i < w; ++i)
        c |= 2u << (2 * i);
    return c;
}
static void exhaustive(void) {
    size_t checks = 0;
    for (unsigned w = 1; w <= 4; ++w) {
        unsigned count = 1u << (2 * w), mask = (1u << w) - 1;
        for (int sa = 0; sa < 2; ++sa)
            for (int sb = 0; sb < 2; ++sb)
                for (unsigned ac = 0; ac < count; ++ac)
                    for (unsigned bc = 0; bc < count; ++bc) {
                        g4_t a = small_states(ac, w, sa), b = small_states(bc, w, sb);
                        int sign = sa && sb;
                        int au = 0, bu = 0;
                        unsigned av = 0, bv = 0;
                        for (unsigned bit = 0; bit < w; ++bit) {
                            unsigned x = state_at(ac, bit), y = state_at(bc, bit);
                            au |= x >= 2;
                            bu |= y >= 2;
                            av |= (x == 1) << bit;
                            bv |= (y == 1) << bit;
                        }
                        int ai = (int)av, bi = (int)bv;
                        if (sign && (av & (1u << (w - 1))))
                            ai -= (int)(1u << w);
                        if (sign && (bv & (1u << (w - 1))))
                            bi -= (int)(1u << w);
                        int at = truth_code(ac, w), bt = truth_code(bc, w);
                        for (unsigned op = 0; op < 20; ++op) {
                            unsigned ew = w, es = (unsigned)sign, expected = 0;
                            if (op <= 5) {
                                unsigned num = 0;
                                if (op == 5)
                                    es = (unsigned)sa;
                                if (au || bu) {
                                    expected = x_code(w);
                                    goto arithmetic_done;
                                }
                                if (op == 5) {
                                    es = (unsigned)sa;
                                    int exponent = (int)bv;
                                    if (sb && (bv & (1u << (w - 1))))
                                        exponent -= (int)(1u << w);
                                    if (exponent < 0) {
                                        if (!av) {
                                            expected = x_code(w);
                                            goto arithmetic_done;
                                        }
                                        if (sa && av == mask)
                                            num = bv & 1 ? av : 1;
                                        else
                                            num = av == 1 ? 1 : 0;
                                    } else {
                                        num = 1;
                                        for (int i = 0; i < exponent; ++i)
                                            num = (num * av) & mask;
                                    }
                                } else if (op == 0)
                                    num = av + bv;
                                else if (op == 1)
                                    num = av - bv;
                                else if (op == 2)
                                    num = av * bv;
                                else if (bv)
                                    num = (unsigned)(op == 3 ? ai / bi : ai % bi);
                                if (au || bu || ((op == 3 || op == 4) && !bv))
                                    expected = x_code(w);
                                else
                                    expected = known_code(num & mask, w);
                            arithmetic_done:;
                            } else if (op <= 9) {
                                for (unsigned bit = 0; bit < w; ++bit) {
                                    unsigned x = state_at(ac, bit), y = state_at(bc, bit), r;
                                    if (op == 6)
                                        r = x == 0 || y == 0 ? 0 : x == 1 && y == 1 ? 1 : 2;
                                    else if (op == 7)
                                        r = x == 1 || y == 1 ? 1 : x == 0 && y == 0 ? 0 : 2;
                                    else
                                        r = x >= 2 || y >= 2 ? 2 : op == 8 ? x ^ y : !(x ^ y);
                                    expected |= r << (2 * bit);
                                }
                            } else {
                                ew = 1;
                                es = 0;
                                unsigned r = 0;
                                if (op == 10 || op == 11) {
                                    int mismatch = 0;
                                    for (unsigned bit = 0; bit < w; ++bit) {
                                        unsigned x = state_at(ac, bit), y = state_at(bc, bit);
                                        mismatch |= x < 2 && y < 2 && x != y;
                                    }
                                    r = mismatch ? 0 : au || bu ? 2 : 1;
                                    if (op == 11 && r < 2)
                                        r = !r;
                                } else if (op <= 13)
                                    r = op == 12 ? ac == bc : ac != bc;
                                else if (op <= 17)
                                    r = au || bu   ? 2
                                        : op == 14 ? ai < bi
                                        : op == 15 ? ai <= bi
                                        : op == 16 ? ai > bi
                                                   : ai >= bi;
                                else if (op == 18)
                                    r = !at || !bt ? 0 : at == 1 && bt == 1 ? 1 : 2;
                                else
                                    r = at == 1 || bt == 1 ? 1 : !at && !bt ? 0 : 2;
                                expected = r;
                            }
                            check_small(new_ops[op](a, b), ew, (int)es, expected);
                            ++checks;
                        }
                        g4_t cond = llg_gmp_sv4_x(1, 0);
                        unsigned merged = 0, inv = 0;
                        for (unsigned bit = 0; bit < w; ++bit) {
                            unsigned x = state_at(ac, bit), y = state_at(bc, bit);
                            merged |= (x == y && x < 2 ? x : 2) << (2 * bit);
                            inv |= (x >= 2 ? 2 : !x) << (2 * bit);
                        }
                        check_small(llg_gmp_sv4_mux(cond, a, b), w, sign, merged);
                        cond = llg_gmp_sv4_zero(1, 0);
                        check_small(llg_gmp_sv4_mux(cond, a, b), w, sign, bc);
                        cond = llg_gmp_sv4_from_u64(1, 1, 0);
                        check_small(llg_gmp_sv4_mux(cond, a, b), w, sign, ac);
                        check_small(llg_gmp_sv4_bitneg(a), w, sa, inv);
                        check_small(llg_gmp_sv4_neg(a), w, sa,
                                    au ? x_code(w) : known_code((0u - av) & mask, w));
                        check_small(llg_gmp_sv4_lognot(a), 1, 0, at == 2 ? 2 : (unsigned)!at);
                        unsigned clog = 0, t = av ? av - 1 : 0;
                        while (t) {
                            ++clog;
                            t >>= 1;
                        }
                        g4_t c = llg_gmp_sv4_clog2(a);
                        CHECK(c.width == 32 && c.is_signed == 0);
                        CHECK(au ? c.data.small.b == UINT32_MAX
                                 : c.data.small.a == clog && c.data.small.b == 0);
                        check_small(llg_gmp_sv4_to_two_state(a), w, sa, known_code(av, w));
                        checks += 6;
                    }
    }
    printf("exhaustive checks=%zu\n", checks);
}
static void exhaustive_storage(void) {
    for (unsigned w = 0; w <= 4; ++w)
        for (unsigned code = 0; code < (1u << (2 * w)); ++code)
            for (int sign = 0; sign < 2; ++sign) {
                g4_t v = small_states(code, w, sign), clone = llg_gmp_sv4_clone(&v);
                check_small(clone, w, sign, code);
                llg_gmp_sv4_assign(&clone, v);
                llg_gmp_sv4_copy(&clone, &v);
                g4_t moved = LLG_GMP_SV4_EMPTY;
                llg_gmp_sv4_move(&moved, &clone);
                CHECK(clone.width == 0);
                check_small(moved, w, sign, code);
                for (unsigned width = 0; width <= 4; ++width)
                    for (int result_sign = 0; result_sign < 2; ++result_sign) {
                        unsigned cast = 0, resize = 0;
                        for (unsigned bit = 0; bit < width; ++bit) {
                            unsigned current = bit < w     ? state_at(code, bit)
                                               : w && sign ? state_at(code, w - 1)
                                                           : 0;
                            cast |= current << (2 * bit);
                            current = bit < w            ? state_at(code, bit)
                                      : w && result_sign ? state_at(code, w - 1)
                                                         : 0;
                            resize |= current << (2 * bit);
                        }
                        check_small(llg_gmp_sv4_cast(v, width, (int8_t)result_sign), width,
                                    result_sign, cast);
                        check_small(llg_gmp_sv4_resize(v, width, (int8_t)result_sign), width,
                                    result_sign, resize);
                    }
                for (unsigned bit = 0; bit <= w; ++bit)
                    for (unsigned state = 0; state < 4; ++state) {
                        llg_gmp_sv4_copy(&clone, &v);
                        llg_gmp_sv4_set_state(&clone, bit, state);
                        unsigned expected =
                            bit < w ? (code & ~(3u << (2 * bit))) | (state << (2 * bit)) : code;
                        check_small(clone, w, sign, expected);
                        check_small(v, w, sign, code);
                    }
                llg_gmp_sv4_destroy(&v);
                llg_gmp_sv4_destroy(&clone);
                llg_gmp_sv4_destroy(&moved);
            }
    for (unsigned w = 1; w <= 4; ++w)
        for (unsigned bits = 0; bits < (1u << w); ++bits)
            for (unsigned x = 0; x < (1u << w); ++x)
                for (unsigned z = 0; z < (1u << w); ++z) {
                    g4_t v = llg_gmp_sv4_zero(w, 0);
                    llg_gmp_sv4_set_word(&v, 0, bits, x, z);
                    unsigned expected = 0;
                    for (unsigned bit = 0; bit < w; ++bit)
                        expected |= ((x >> bit) & 1   ? 2u
                                     : (z >> bit) & 1 ? 3u
                                                      : (bits >> bit) & 1)
                                    << (2 * bit);
                    check_small(v, w, 0, expected);
                    g4_t literal = llg_gmp_sv4_from_masks(bits, x, z, w, 0);
                    check_small(literal, w, 0, expected);
                }
    puts("exhaustive constructors, conversions, owner transfers and neutral setters passed");
}
int main(int argc, char** argv) {
    if (argc > 1) {
        if (!strcmp(argv[1], "limit"))
            (void)llg_gmp_sv4_zero(LLG_GMP_SUPPORTED_WIDTH_LIMIT, 0);
        else
            (void)llg_gmp_sv4_zero(UINT32_MAX, 0);
        return 1;
    }
    ownership_and_bridge();
    exhaustive();
    exhaustive_storage();
    uint32_t widths[] = {1,    2,    7,    8,    31,    32,
                         33,   63,   64,   65,   127,   128,
                         129,  255,  256,  257,  1023,  1024,
                         4096, 8191, 8192, 8193, 16384, LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1};
    size_t count = sizeof(widths) / sizeof(widths[0]);
    for (size_t i = 0; i < count; ++i)
        for (unsigned pattern = 0; pattern < 10; ++pattern) {
            for (int sa = 0; sa < 2; ++sa)
                for (int sb = 0; sb < 2; ++sb) {
                    random_case(widths[i], widths[i], sa, sb, pattern);
                    random_case(widths[i], widths[i ? i - 1 : 1], sa, sb, pattern);
                }
        }
    uint32_t max = LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1;
    g4_t a = llg_gmp_sv4_zero(max, 0), b = llg_gmp_sv4_from_u64(3, max, 0);
    llg_gmp_sv4_set_state(&a, max - 1, 1);
    g4_t product = llg_gmp_sv4_mul(a, b);
    CHECK(llg_gmp_sv4_same(a, product));
    sv4_t old_a = sv4_zero(max, 0), old_b = sv4_from_u64(3, max, 0);
    llg_sv4_set_state(&old_a, max - 1, 1);
    compare_result(sv4_mul(old_a, old_b), llg_gmp_sv4_clone(&product),
                   "max admitted multiplication");
    sv4_destroy(&old_a);
    sv4_destroy(&old_b);
    llg_gmp_sv4_destroy(&a);
    llg_gmp_sv4_destroy(&b);
    llg_gmp_sv4_destroy(&product);
    printf("compact differential passed; descriptor=%zu bytes; widths=%zu\n", sizeof(g4_t), count);
    return 0;
}
