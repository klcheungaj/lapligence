// llg_rt_selftest.c — self-tests for the llg runtime: sv4 value semantics
// (vectors mirrored from src/core/elab.rs unit tests) plus scheduler behavior
// (delay ordering, NBA visibility, ping-pong via signal events).
//
// Build: gcc -std=c11 -O2 llg_rt_selftest.c llg_rt.c aco.c acosw.S -o selftest

#include "llg_rt.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;

static llg_event_object_t lifecycle_event_object;
static llg_event_t lifecycle_event = { &lifecycle_event_object };
static int lifecycle_triggered_inside_run;

#define CHECK(cond)                                                        \
    do {                                                                   \
        if (!(cond)) {                                                     \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                    \
        }                                                                  \
    } while (0)

// sv4 helpers mirroring the elab.rs test helpers.
static sv4_t b4(const char* s) { // MSB-first bit string with 0/1/x/z
    sv4_t v = SV4_INIT(0, 0, 0, 0, 0);
    int n = (int)strlen(s);
    for (int i = 0; i < n; i++) {
        int lsb = n - 1 - i;
        char c = s[i];
        if (c == 'x') {
            v.x[0] |= 1ULL << lsb;
        } else if (c == 'z') {
            v.z[0] |= 1ULL << lsb;
        } else if (c == '1') {
            v.bits[0] |= 1ULL << lsb;
        }
    }
    v.width = (uint16_t)n;
    return v;
}

static uint64_t u(sv4_t v) { return sv4_to_u64(v); }
static int isx(sv4_t v) { return sv4_is_unknown(v); }
static double real_from_bits(uint64_t bits) {
    double value;
    memcpy(&value, &bits, sizeof(value));
    return value;
}

// Wide-value helpers: build from raw limbs (LSB-first).
static sv4_t w128(uint64_t lo, uint64_t hi) {
    uint64_t b[2] = { lo, hi }, x[2] = { 0, 0 }, z[2] = { 0, 0 };
    return sv4_from_limbs(b, x, z, 128, 0);
}

static int w_same(sv4_t a, uint64_t lo, uint64_t hi) {
    return a.width == 128 && !sv4_is_unknown(a) && a.bits[0] == lo &&
           a.bits[1] == hi;
}

static void test_sv4_ops(void) {
    CHECK(sv4_fits_i64(b4("0")));
    CHECK(sv4_fits_i64(b4("1111")));
    CHECK(!sv4_fits_i64(b4("x")));
    CHECK(!sv4_fits_i64(sv4_from_limbs(
        (uint64_t[]){1, 1}, NULL, NULL, 128, 0)));
    CHECK(sv4_fits_i64(sv4_from_limbs(
        (uint64_t[]){UINT64_MAX, UINT64_MAX}, NULL, NULL, 128, 1)));
    // Packed/real conversions: all limbs participate, signed values keep
    // two's complement semantics, and real-to-integer rounds to nearest.
    CHECK(sv4_to_real(w128(0, 1)) == 18446744073709551616.0);
    CHECK(sv4_to_real(b4("1x01")) == 9.0);
    {
        sv4_t minus_one = w128(UINT64_MAX, UINT64_MAX);
        minus_one.is_signed = 1;
        CHECK(sv4_to_real(minus_one) == -1.0);
    }
    CHECK(sv4_to_i64(sv4_from_real(2.5, 32, 1)) == 3);
    CHECK(sv4_to_i64(sv4_from_real(-2.5, 32, 1)) == -3);
    CHECK(sv4_to_u64(sv4_from_real(2.49, 32, 0)) == 2);
    CHECK(sv4_to_u64(sv4_from_real(-2.5, 8, 0)) == 253);
    CHECK(sv4_to_i64(sv4_from_real(130.0, 8, 1)) == -126);
    CHECK(!llg_real_to_bool(0.0));
    CHECK(llg_real_to_bool(INFINITY));
    CHECK(llg_real_to_bool(NAN));
    // add: carry drop, unsigned
    CHECK(sv4_same(sv4_add(b4("1001"), b4("0001")), b4("1010")));
    // add: x propagation
    CHECK(isx(sv4_add(b4("10x1"), b4("0001"))));
    CHECK(sv4_same(sv4_add(b4("10x1"), b4("0001")), SV4_X(4)));
    // signed add: 4'b1111 + 4'b0001 = 4'b0000
    {
        sv4_t a = b4("1111");
        a.is_signed = 1;
        sv4_t b = b4("0001");
        b.is_signed = 1;
        sv4_t r = sv4_add(a, b);
        CHECK(sv4_same(r, b4("0000")));
        CHECK(r.is_signed == 1);
    }
    // sub
    CHECK(sv4_same(sv4_sub(b4("1010"), b4("0011")), b4("0111")));
    // mul truncated to max operand width
    CHECK(sv4_same(sv4_mul(b4("0110"), b4("0011")), b4("0010")));
    // div / rem
    CHECK(u(sv4_div(b4("00001010"), b4("00000011"))) == 3);
    CHECK(u(sv4_mod(b4("00001010"), b4("00000011"))) == 1);
    CHECK(u(sv4_div(SV4_C(10, 32), SV4_C(3, 32))) == 3);
    CHECK(u(sv4_mod(SV4_C(10, 32), SV4_C(3, 32))) == 1);
    // Signedness and common-width coercion apply to all binary arithmetic.
    {
        sv4_t signed_narrow = b4("1111"); // -1 in four bits
        signed_narrow.is_signed = 1;
        sv4_t signed_wide = SV4_S(1, 8);
        CHECK(sv4_same(sv4_add(signed_narrow, signed_wide), SV4_C(0, 8)));
        CHECK(sv4_same(sv4_sub(signed_narrow, signed_wide), SV4_S(0xfe, 8)));
        CHECK(sv4_same(sv4_mul(signed_narrow, signed_wide), SV4_S(0xff, 8)));
        CHECK(sv4_add(signed_narrow, signed_wide).is_signed == 1);
        CHECK(sv4_sub(signed_narrow, signed_wide).is_signed == 1);
        CHECK(sv4_mul(signed_narrow, signed_wide).is_signed == 1);
        CHECK(sv4_to_i64(sv4_div(signed_narrow, signed_wide)) == -1);
        CHECK(sv4_to_i64(sv4_mod(signed_narrow, signed_wide)) == 0);
        CHECK(sv4_div(signed_narrow, signed_wide).is_signed == 1);

        sv4_t unsigned_wide = SV4_C(1, 8);
        CHECK(sv4_same(sv4_add(signed_narrow, unsigned_wide), SV4_C(16, 8)));
        CHECK(sv4_same(sv4_sub(signed_narrow, unsigned_wide), SV4_C(14, 8)));
        CHECK(sv4_same(sv4_mul(signed_narrow, unsigned_wide), SV4_C(15, 8)));
        CHECK(sv4_add(signed_narrow, unsigned_wide).is_signed == 0);
        CHECK(sv4_sub(signed_narrow, unsigned_wide).is_signed == 0);
        CHECK(sv4_mul(signed_narrow, unsigned_wide).is_signed == 0);
        CHECK(sv4_to_u64(sv4_div(signed_narrow, unsigned_wide)) == 15);
        CHECK(sv4_to_u64(sv4_mod(signed_narrow, unsigned_wide)) == 0);
        CHECK(sv4_div(signed_narrow, unsigned_wide).is_signed == 0);
    }
    // Mixed signed/unsigned -7 / 2 is unsigned (32'hffff_fff9 / 2).
    CHECK(sv4_to_u64(sv4_div(SV4_S((uint64_t)(int64_t)-7, 32), SV4_C(2, 32))) ==
          2147483644ULL);
    CHECK(sv4_to_u64(sv4_mod(SV4_S((uint64_t)(int64_t)-7, 32), SV4_C(2, 32))) == 1);
    CHECK(sv4_div(SV4_S((uint64_t)(int64_t)-7, 32), SV4_C(2, 32)).is_signed == 0);
    // Avoid the host C overflow for the fixed-width INT64_MIN / -1 result.
    {
        sv4_t min = SV4_S(1ULL << 63, 64);
        sv4_t neg_one = SV4_S(UINT64_MAX, 64);
        CHECK(sv4_same(sv4_div(min, neg_one), min));
        CHECK(sv4_same(sv4_mod(min, neg_one), SV4_S(0, 64)));
    }
    // div by zero -> X
    CHECK(isx(sv4_div(SV4_C(10, 32), SV4_C(0, 32))));
    // shifts
    CHECK(sv4_same(sv4_shl(b4("0001"), SV4_C(2, 8)), b4("0100")));
    CHECK(sv4_same(sv4_shr(b4("1000"), SV4_C(1, 8)), b4("0100")));
    CHECK(sv4_same(sv4_ashl(b4("0001"), SV4_C(2, 8)), b4("0100")));
    CHECK(sv4_same(sv4_shl(b4("0001"), SV4_C(4, 8)), b4("0000")));
    // Known X/Z bits on the LHS move positionally; only an unknown shift
    // amount makes the entire result X.
    CHECK(sv4_same(sv4_shl(b4("10z1"), SV4_C(1, 4)), b4("0z10")));
    CHECK(sv4_same(sv4_shr(b4("10z1"), SV4_C(1, 4)), b4("010z")));
    CHECK(sv4_same(sv4_shr(b4("x001"), SV4_C(1, 4)), b4("0x00")));
    CHECK(sv4_same(sv4_shl(b4("0001"), b4("0x01")), SV4_X(4)));
    {
        sv4_t a = b4("10000000");
        a.is_signed = 1;
        CHECK(sv4_same(sv4_ashr(a, SV4_C(4, 8)), b4("11111000")));
    }
    {
        sv4_t z = b4("z001");
        z.is_signed = 1;
        CHECK(sv4_same(sv4_ashr(z, SV4_C(1, 4)), b4("zz00")));
        CHECK(sv4_same(sv4_ashr(b4("z001"), SV4_C(1, 4)), b4("0z00")));
        CHECK(sv4_same(sv4_ashr(z, SV4_C(4, 4)), b4("zzzz")));
    }
    // concat
    CHECK(sv4_same(sv4_concat(b4("1010"), b4("0001")), b4("10100001")));
    CHECK(sv4_same(sv4_repeat(b4("1010"), 2), b4("10101010")));
    // comparisons
    CHECK(isx(sv4_eq(b4("10x0"), b4("10x0"))));
    CHECK(u(sv4_case_eq(b4("10x0"), b4("10x0"))) == 1);
    CHECK(u(sv4_case_neq(b4("10x0"), b4("10x0"))) == 0);
    CHECK(u(sv4_lt(SV4_C(3, 8), SV4_C(5, 8))) == 1);
    CHECK(u(sv4_le(SV4_C(5, 8), SV4_C(3, 8))) == 0);
    CHECK(u(sv4_gt(SV4_C(9, 8), SV4_C(5, 8))) == 1);
    CHECK(u(sv4_ge(SV4_C(5, 8), SV4_C(5, 8))) == 1);
    CHECK(u(sv4_eq(SV4_C(1, 8), SV4_C(1, 8))) == 1);
    CHECK(u(sv4_neq(SV4_C(1, 8), SV4_C(2, 8))) == 1);
    CHECK(isx(sv4_lt(b4("1x"), b4("01"))));
    // Comparisons use max width and a common signed type only when both
    // operands are signed; all comparison results remain 1-bit unsigned.
    {
        sv4_t signed_narrow = b4("1111");
        signed_narrow.is_signed = 1;
        sv4_t signed_wide = SV4_S(0, 8);
        CHECK(u(sv4_lt(signed_narrow, signed_wide)) == 1);
        CHECK(u(sv4_eq(signed_narrow, signed_wide)) == 0);
        CHECK(sv4_lt(signed_narrow, signed_wide).is_signed == 0);
        sv4_t unsigned_wide = SV4_C(0, 8);
        CHECK(u(sv4_lt(signed_narrow, unsigned_wide)) == 0);
        CHECK(u(sv4_gt(signed_narrow, unsigned_wide)) == 1);
        sv4_t signed_case_wide = b4("zzzzz001");
        signed_case_wide.is_signed = 1;
        CHECK(u(sv4_case_eq(sv4_from_limbs(
                   (uint64_t[]){1}, NULL, (uint64_t[]){8}, 4, 1),
                   signed_case_wide)) == 1);
        CHECK(u(sv4_case_eq(sv4_from_limbs(
                   (uint64_t[]){1}, NULL, (uint64_t[]){8}, 4, 1),
                   b4("00000001"))) == 0);
    }
    // reductions
    CHECK(u(sv4_reduce_and(b4("1111"))) == 1);
    CHECK(u(sv4_reduce_and(b4("1011"))) == 0);
    CHECK(u(sv4_reduce_or(b4("0000"))) == 0);
    CHECK(u(sv4_reduce_or(b4("0100"))) == 1);
    CHECK(u(sv4_reduce_xor(b4("1010"))) == 0);
    CHECK(u(sv4_reduce_xor(b4("1011"))) == 1);
    CHECK(isx(sv4_reduce_and(b4("1x11"))));
    CHECK(u(sv4_reduce_and(b4("0x11"))) == 0);
    CHECK(u(sv4_reduce_and(b4("0z11"))) == 0);
    CHECK(u(sv4_reduce_or(b4("1x00"))) == 1);
    CHECK(u(sv4_reduce_or(b4("1z00"))) == 1);
    CHECK(isx(sv4_reduce_xor(b4("10x1"))));
    CHECK(isx(sv4_reduce_xor(b4("10z1"))));
    CHECK(u(sv4_reduce_nand(b4("1111"))) == 0);
    CHECK(u(sv4_reduce_nor(b4("0000"))) == 1);
    CHECK(u(sv4_reduce_xnor(b4("1010"))) == 1);
    // logical ops
    CHECK(u(sv4_lognot(SV4_C(0, 8))) == 1);
    CHECK(u(sv4_lognot(SV4_C(3, 8))) == 0);
    CHECK(u(sv4_logand(SV4_C(3, 8), SV4_C(0, 8))) == 0);
    CHECK(u(sv4_logand(SV4_C(3, 8), SV4_C(4, 8))) == 1);
    CHECK(u(sv4_logor(SV4_C(0, 8), SV4_C(0, 8))) == 0);
    CHECK(u(sv4_logor(SV4_C(0, 8), SV4_C(5, 8))) == 1);
    CHECK(u(sv4_lognot(b4("1x"))) == 0);
    CHECK(u(sv4_logand(b4("1x"), b4("1"))) == 1);
    CHECK(isx(sv4_logor(b4("00"), b4("0x"))));
    CHECK(u(sv4_lognot(b4("1x00"))) == 0);
    CHECK(u(sv4_lognot(b4("1z00"))) == 0);
    CHECK(u(sv4_logand(b4("0"), b4("x"))) == 0);
    CHECK(u(sv4_logand(b4("0"), b4("z"))) == 0);
    CHECK(u(sv4_logor(b4("1"), b4("x"))) == 1);
    CHECK(u(sv4_logor(b4("1"), b4("z"))) == 1);
    CHECK(isx(sv4_logand(b4("0x"), b4("1"))));
    CHECK(isx(sv4_logor(b4("0x"), b4("0"))));
    // bitwise ops
    CHECK(sv4_same(sv4_and(b4("1100"), b4("1010")), b4("1000")));
    CHECK(sv4_same(sv4_or(b4("1100"), b4("1010")), b4("1110")));
    CHECK(sv4_same(sv4_xor(b4("1100"), b4("1010")), b4("0110")));
    CHECK(sv4_same(sv4_xnor(b4("1100"), b4("1010")), b4("1001")));
    CHECK(sv4_same(sv4_bitneg(b4("1010")), b4("0101")));
    CHECK(sv4_same(sv4_bitneg(b4("10xz")), b4("01xx")));
    // 0 dominates AND; 1 dominates OR
    CHECK(sv4_same(sv4_and(b4("0x"), b4("1x")), b4("0x")));
    CHECK(sv4_same(sv4_or(b4("1x"), b4("0x")), b4("1x")));
    // Bitwise operands are resized to max width using a common signed type.
    {
        sv4_t signed_narrow = b4("1111");
        signed_narrow.is_signed = 1;
        CHECK(sv4_same(sv4_and(signed_narrow, SV4_C(0xf0, 8)), b4("00000000")));
        CHECK(sv4_same(sv4_or(signed_narrow, SV4_C(0, 8)), b4("00001111")));
        CHECK(sv4_same(sv4_xor(signed_narrow, SV4_C(0, 8)), b4("00001111")));
        CHECK(sv4_same(sv4_xnor(signed_narrow, SV4_C(0, 8)), b4("11110000")));
        CHECK(sv4_or(signed_narrow, SV4_S(0, 8)).is_signed == 1);
        CHECK(sv4_same(sv4_or(signed_narrow, SV4_S(0, 8)), b4("11111111")));
        sv4_t x = b4("x001");
        x.is_signed = 1;
        CHECK(sv4_same(sv4_or(x, SV4_S(0, 8)), b4("xxxxx001")));
    }
    // unary minus
    {
        sv4_t a = b4("0010");
        a.is_signed = 1;
        CHECK(sv4_same(sv4_neg(a), b4("1110")));
    }
    CHECK(sv4_same(sv4_neg(b4("0011")), b4("1101")));
    // power
    CHECK(u(sv4_pow(SV4_C(2, 32), SV4_C(8, 32))) == 256);
    CHECK(u(sv4_pow(SV4_C(0, 32), SV4_C(0, 32))) == 1);
    // Power keeps the base width/signedness; the exponent is self-determined.
    CHECK(sv4_same(sv4_pow(SV4_C(3, 4), SV4_C(2, 8)), SV4_C(9, 4)));
    {
        sv4_t signed_base = SV4_S((uint64_t)(int64_t)-2, 4);
        sv4_t r = sv4_pow(signed_base, SV4_C(3, 8));
        CHECK(sv4_same(r, SV4_S(8, 4)) && r.is_signed == 1);
        CHECK(sv4_same(sv4_pow(SV4_S(2, 4), SV4_S(UINT64_MAX, 8)), SV4_S(0, 4)));
    }
    // clog2
    CHECK(u(sv4_clog2(SV4_C(256, 32))) == 8);
    CHECK(u(sv4_clog2(SV4_C(0, 32))) == 0);
    CHECK(u(sv4_clog2(SV4_C(1, 32))) == 0);
    CHECK(u(sv4_clog2(SV4_C(255, 32))) == 8);
    CHECK(u(sv4_clog2(SV4_C(3, 32))) == 2);
    CHECK(u(sv4_clog2(SV4_C(2, 32))) == 1);
    // resize sign/zero/fill
    {
        sv4_t s = b4("1000");
        s.is_signed = 1;
        CHECK(sv4_same(sv4_resize(s, 8, 1), b4("11111000")));
        CHECK(sv4_same(sv4_resize(s, 8, 0), b4("00001000")));
        sv4_t x = b4("x00");
        x.is_signed = 1;
        CHECK(sv4_same(sv4_resize(x, 6, 1), b4("xxxx00")));
        sv4_t z = b4("z00");
        z.is_signed = 1;
        CHECK(sv4_same(sv4_resize(z, 6, 1), b4("zzzz00")));
        CHECK(sv4_same(sv4_resize(b4("1001"), 2, 0), b4("01")));
        CHECK(u(sv4_resize(SV4_C(7, 4), 8, 0)) == 7);
        CHECK(sv4_same(sv4_fill(1, 8, 0), b4("11111111")));
        CHECK(isx(sv4_fill(2, 8, 0)));
    }
    // value-preserving cast: extension follows the SOURCE's signedness
    // (LRM 1800-2009 §6.24.1 / §10.7)
    {
        sv4_t s = b4("1000");
        s.is_signed = 1;
        sv4_t ffu = SV4_C(0xff, 8); // unsigned 255
        CHECK(u(sv4_cast(ffu, 16, 1)) == 255);   // int'(8'hFF) == 255
        CHECK(sv4_same(sv4_cast(s, 8, 1), b4("11111000")));      // sign-ext kept
        CHECK(u(sv4_cast(s, 16, 0)) == 65528);                   // -8 -> unsigned wide
        CHECK(sv4_same(sv4_cast(b4("1001"), 8, 0), b4("00001001")));
        CHECK(sv4_same(sv4_cast(b4("1010"), 3, 1), b4("010")));  // narrowing truncates
    }
    // conditional op
    CHECK(sv4_same(sv4_mux(b4("1"), b4("1010"), b4("0101")), b4("1010")));
    CHECK(sv4_same(sv4_mux(b4("0"), b4("1010"), b4("0101")), b4("0101")));
    CHECK(sv4_same(sv4_mux(b4("x"), b4("1010"), b4("1010")), b4("1010")));
    CHECK(sv4_same(sv4_mux(b4("x"), b4("1010"), b4("0101")), SV4_X(4)));
    CHECK(sv4_same(sv4_mux(b4("x"), b4("1010"), b4("1000")), b4("10x0")));
    CHECK(sv4_same(sv4_mux(b4("z"), b4("1010"), b4("1000")), b4("10x0")));
    CHECK(sv4_same(sv4_mux(b4("x"), b4("10xz"), b4("10xz")), b4("10xz")));
    CHECK(sv4_same(sv4_mux(b4("x"), b4("10xz"), b4("10zz")), b4("10xz")));
    {
        sv4_t signed_narrow = b4("1111");
        signed_narrow.is_signed = 1;
        CHECK(sv4_same(sv4_mux(b4("1"), signed_narrow, SV4_C(0, 8)),
                       b4("00001111")));
        CHECK(sv4_mux(b4("1"), signed_narrow, SV4_C(0, 8)).is_signed == 0);
        CHECK(sv4_same(sv4_mux(b4("x"), signed_narrow, SV4_C(0, 8)),
                       b4("0000xxxx")));
        CHECK(sv4_same(sv4_mux(b4("1x"), b4("1010"), b4("1000")),
                       b4("1010")));
    }
    // selects
    CHECK(u(sv4_bit_select(b4("1010"), 1)) == 1);
    CHECK(u(sv4_bit_select(b4("1010"), 0)) == 0);
    CHECK(sv4_same(sv4_part_select(b4("10100101"), 5, 2), b4("1001")));
    CHECK(sv4_same(sv4_part_select(b4("10100101"), 2, 5), b4("1001")));
    {
        sv4_t out = sv4_part_select(b4("10100101"), 4294967297LL, 4294967296LL);
        CHECK(out.width == 2 && sv4_is_unknown(out));
    }
    CHECK(u(sv4_idx_part_select(b4("10100101"), 2, 3, 0)) == 1); // [2 +: 3] = 001
    CHECK(u(sv4_idx_part_select(b4("10100101"), 5, 3, 1)) == 4); // [5 -: 3] = 100
    {
        sv4_t t = b4("0000");
        sv4_bit_select_set(&t, 2, SV4_C(1, 1));
        CHECK(sv4_same(t, b4("0100")));
        sv4_part_select_set(&t, 3, 2, b4("11"));
        CHECK(sv4_same(t, b4("1100")));
    }
    // to_i64 two's complement
    {
        sv4_t s = b4("1000");
        CHECK(sv4_to_i64(s) == -8);
        CHECK(sv4_to_u64(b4("10xz")) == 8); // unknown bits read as 0
        CHECK(isx(b4("10xz")));
    }
    // format
    {
        char buf[64];
        sv4_format('b', SV4_C(5, 8), buf, sizeof(buf));
        CHECK(strcmp(buf, "00000101") == 0);
        sv4_format('h', SV4_C(0xAB, 8), buf, sizeof(buf));
        CHECK(strcmp(buf, "ab") == 0);
        sv4_format('h', b4("1010x001"), buf, sizeof(buf));
        CHECK(strcmp(buf, "ax") == 0);
        sv4_format('d', SV4_C(18, 9), buf, sizeof(buf));
        CHECK(strcmp(buf, "18") == 0);
        sv4_format('o', SV4_C(18, 8), buf, sizeof(buf));
        CHECK(strcmp(buf, "022") == 0);
        sv4_format('d', SV4_X(8), buf, sizeof(buf));
        CHECK(strcmp(buf, "x") == 0);
        // X/Z distinguished: %b prints 'z' for Z bits
        sv4_format('b', b4("10z1"), buf, sizeof(buf));
        CHECK(strcmp(buf, "10z1") == 0);
        sv4_format('b', b4("10xz"), buf, sizeof(buf));
        CHECK(strcmp(buf, "10xz") == 0);
        // %h: any X in a nibble -> 'x'; any Z but no X -> 'z'
        sv4_format('h', b4("zzzzxxxx"), buf, sizeof(buf));
        CHECK(strcmp(buf, "zx") == 0);
        sv4_format('h', b4("zzzzzzzzxxxxxxxx"), buf, sizeof(buf));
        CHECK(strcmp(buf, "zzxx") == 0);
        sv4_format('h', b4("zx01"), buf, sizeof(buf));
        CHECK(strcmp(buf, "x") == 0); // X wins over Z in a mixed nibble
        // %o: 3-bit groups, same rule
        sv4_format('o', b4("1z0"), buf, sizeof(buf));
        CHECK(strcmp(buf, "z") == 0);
        sv4_format('o', b4("1x0"), buf, sizeof(buf));
        CHECK(strcmp(buf, "x") == 0);
        // %d prints 'x' for any X or Z
        sv4_format('d', b4("10z1"), buf, sizeof(buf));
        CHECK(strcmp(buf, "x") == 0);
    }
    // casez/casex wildcard matching (per-bit rules, LRM 12.5.1)
    {
        // casez: ?/z in the ITEM is a don't-care
        CHECK(u(sv4_casez_eq(b4("1000"), b4("1z0z"))) == 1); // ? bits don't-care
        CHECK(u(sv4_casez_eq(b4("1110"), b4("1z0z"))) == 0); // known 0 vs sel 1
        // casez: x in the item matches a selector x only
        CHECK(u(sv4_casez_eq(b4("1x00"), b4("1x0z"))) == 1); // item x vs sel x
        CHECK(u(sv4_casez_eq(b4("1010"), b4("1x0z"))) == 0); // item x vs sel 1
        CHECK(u(sv4_casez_eq(b4("1000"), b4("100z"))) == 1); // plain known match
        // casex: x/z/? in the ITEM are don't-cares
        CHECK(u(sv4_casex_eq(b4("1001"), b4("1x0z"))) == 1);
        CHECK(u(sv4_casex_eq(b4("1000"), b4("1x0z"))) == 1);
        // casex: a selector x/z is a don't-care against a known item bit
        CHECK(u(sv4_casex_eq(b4("1x0z"), b4("1000"))) == 1);
        CHECK(u(sv4_casex_eq(b4("1100"), b4("1000"))) == 0); // opposite known bit
        // matches are never X
        CHECK(!isx(sv4_casez_eq(b4("1x0z"), b4("1x0z"))));
        CHECK(!isx(sv4_casex_eq(b4("1x0z"), b4("1x0z"))));
    }
    // === / !==: X, Z and known bits are compared literally
    {
        CHECK(u(sv4_case_eq(b4("10xz"), b4("10xz"))) == 1); // X==X, Z==Z
        CHECK(u(sv4_case_eq(b4("10xz"), b4("10zz"))) == 0); // X != Z
        CHECK(u(sv4_case_eq(b4("10zz"), b4("10zz"))) == 1); // Z == Z
        CHECK(u(sv4_case_neq(b4("10xz"), b4("10zz"))) == 1);
    }
    // Z behaves as X in expression ops, but flows through copy ops
    {
        CHECK(isx(sv4_eq(b4("10z1"), b4("10z1"))));         // == with z -> X
        CHECK(isx(sv4_add(b4("10z1"), b4("0001"))));        // z -> x in arithmetic
        CHECK(isx(sv4_mux(b4("z"), b4("1010"), b4("0101")))); // z select -> all-X
        CHECK(sv4_same(sv4_mux(b4("1"), b4("10z1"), b4("0000")), b4("10z1")));
        CHECK(sv4_same(sv4_concat(b4("10z1"), b4("0x01")), b4("10z10x01")));
        CHECK(sv4_same(sv4_resize(b4("10z1"), 8, 0), b4("000010z1")));
        CHECK(sv4_same(sv4_bit_select(b4("10z1"), 1), SV4_Z(1))); // z select -> z
        CHECK(sv4_same(sv4_bit_select(b4("1x0z"), 2), SV4_X(1))); // x select -> x
        CHECK(sv4_same(sv4_part_select(b4("10z1"), 2, 0), b4("0z1")));
        CHECK(sv4_same(sv4_fill(3, 4, 0), b4("zzzz")));
        CHECK(isx(sv4_fill(3, 4, 0)));
    }
}

// ── Wide (limb-based) value tests ─────────────────────────────────────────────

static void test_sv4_wide(void) {
    char buf[512];
    // Mixed-width signed operands must be resized before wide-limb arithmetic.
    {
        sv4_t narrow = b4("1111"); // -1 in four bits
        narrow.is_signed = 1;
        sv4_t wide_one = w128(1, 0);
        wide_one.is_signed = 1;
        CHECK(w_same(sv4_add(narrow, wide_one), 0, 0));
        CHECK(w_same(sv4_sub(narrow, wide_one), ~0ULL - 1, ~0ULL));
        CHECK(w_same(sv4_mul(narrow, wide_one), ~0ULL, ~0ULL));
    }
    // 128-bit add with carry across limbs: (2^64-1) + 1 = 2^64
    CHECK(w_same(sv4_add(w128(~0ULL, 0), w128(1, 0)), 0, 1));
    // full-width wrap: 2^128 - 1 + 1 = 0
    CHECK(w_same(sv4_add(w128(~0ULL, ~0ULL), w128(1, 0)), 0, 0));
    // add x propagation at 128 bits
    CHECK(isx(sv4_add(w128(1, 0), sv4_x(128, 0))));
    // sub with borrow across limbs: 2^64 - 1 = 2^64-1
    CHECK(w_same(sv4_sub(w128(0, 1), w128(1, 0)), ~0ULL, 0));
    // 128-bit mul, schoolbook: (2^64-1)^2 mod 2^128 = 1 + 2^64*(2^64-2)
    CHECK(w_same(sv4_mul(w128(~0ULL, 0), w128(~0ULL, 0)), 1, ~0ULL - 1));
    // 2^64 * 2^64 mod 2^128 = 0
    CHECK(w_same(sv4_mul(w128(0, 1), w128(0, 1)), 0, 0));
    CHECK(isx(sv4_mul(w128(1, 0), sv4_x(128, 0))));
    // div/mod/pow on operands wider than 64 bits -> all-X
    CHECK(w_same(sv4_div(w128(100, 0), SV4_C(3, 32)), 33, 0));
    CHECK(w_same(sv4_mod(w128(100, 0), SV4_C(3, 32)), 1, 0));
    CHECK(w_same(sv4_pow(w128(100, 0), SV4_C(2, 32)), 10000, 0));
    // 128-bit compares: 2^100 vs 2^100 - 1
    {
        sv4_t a = w128(0, 1ULL << 36);              // 2^100
        sv4_t b = w128(~0ULL, (1ULL << 36) - 1);    // 2^100 - 1
        CHECK(u(sv4_gt(a, b)) == 1);
        CHECK(u(sv4_lt(b, a)) == 1);
        CHECK(u(sv4_eq(a, a)) == 1);
        CHECK(u(sv4_eq(a, b)) == 0);
        CHECK(u(sv4_ge(a, a)) == 1);
        CHECK(u(sv4_le(a, a)) == 1);
    }
    // signed 128-bit compare: -2^127 < 0
    {
        sv4_t a = w128(0, 0x8000000000000000ULL);
        a.is_signed = 1;
        sv4_t b = w128(0, 0);
        b.is_signed = 1;
        CHECK(u(sv4_lt(a, b)) == 1);
        CHECK(u(sv4_gt(b, a)) == 1);
    }
    // 100-bit shifts across the limb boundary
    {
        sv4_t v = sv4_from_u64(1ULL << 63, 100, 0);
        sv4_t r = sv4_shl(v, SV4_C(1, 8));
        CHECK(r.width == 100 && !sv4_is_unknown(r));
        CHECK(r.bits[0] == 0 && r.bits[1] == 1); // bit 64
        CHECK(sv4_same(sv4_shr(r, SV4_C(1, 8)), v));
        CHECK(u(sv4_shl(v, SV4_C(100, 8))) == 0); // amount == width -> 0
        CHECK(!sv4_is_unknown(sv4_shl(v, SV4_C(100, 8))));
        // arithmetic right shift sign-fills across the boundary: 2^99 >>> 5
        uint64_t vb[2] = { 0, 1ULL << 35 };
        uint64_t vx[2] = { 0, 0 }, vz[2] = { 0, 0 };
        sv4_t neg = sv4_from_limbs(vb, vx, vz, 100, 1);
        sv4_t ar = sv4_ashr(neg, SV4_C(5, 8));
        CHECK(!sv4_is_unknown(ar) && ar.bits[1] == (63ULL << 30) && ar.bits[0] == 0);
        // shift amount unknown -> all-X
        CHECK(isx(sv4_shl(v, SV4_X(8))));
    }
    // concat 64 + 64 -> 128, hi above lo
    CHECK(w_same(sv4_concat(SV4_C(1, 64), SV4_C(2, 64)), 2, 1));
    // repeat: 128-bit pattern twice -> 256 bits; maximum-width requests remain valid
    {
        sv4_t r = sv4_repeat(w128(0x1111111111111111ULL, 0x2222222222222222ULL), 2);
        CHECK(r.width == 256);
        CHECK(r.bits[0] == 0x1111111111111111ULL && r.bits[1] == 0x2222222222222222ULL);
        CHECK(r.bits[2] == 0x1111111111111111ULL && r.bits[3] == 0x2222222222222222ULL);
        sv4_t boundary_repeat = sv4_repeat(SV4_C(1, 1), LLG_MAX_WIDTH);
        CHECK(sv4_same(boundary_repeat, sv4_fill(1, LLG_MAX_WIDTH, 0)));
        sv4_t half = sv4_fill(1, LLG_MAX_WIDTH / 2, 0);
        CHECK(sv4_same(sv4_concat(half, half), sv4_fill(1, LLG_MAX_WIDTH, 0)));
    }
    // part-select spanning the limb boundary (bits 63..70)
    {
        sv4_t v = w128(0, 1ULL << 6); // bit 70
        CHECK(u(sv4_part_select(v, 70, 63)) == 128); // {v[70]..v[63]}
        CHECK(u(sv4_part_select(v, 63, 70)) == 1);   // {v[63]..v[70]}
        CHECK(u(sv4_idx_part_select(v, 64, 8, 0)) == 64); // [64 +: 8] = {v[71]..v[64]}
        CHECK(isx(sv4_part_select(v, 200, 190)));    // out-of-range -> X
        sv4_t t = w128(0, 0);
        sv4_part_select_set(&t, 70, 63, SV4_C(128, 8));
        CHECK(w_same(t, 0, 1ULL << 6));
    }
    // sign/zero extend 64 -> 128
    {
        sv4_t s = SV4_S(0x8000000000000000ULL, 64);
        sv4_t r = sv4_resize(s, 128, 1);
        CHECK(r.bits[0] == 0x8000000000000000ULL && r.bits[1] == ~0ULL);
        CHECK(!sv4_is_unknown(r));
        CHECK(sv4_resize(s, 128, 0).bits[1] == 0);
    }
    // shrink 128 -> 64 keeps the low limb
    {
        sv4_t r = sv4_resize(w128(0xF, 0x123456789ULL), 64, 0);
        CHECK(r.width == 64 && !sv4_is_unknown(r) && r.bits[0] == 0xF);
    }
    // resize preserves unknown bits
    {
        uint64_t b[2] = { 0x7, 0 }, x[2] = { 0x8, 0 }, z[2] = { 0, 0 };
        sv4_t r = sv4_resize(sv4_from_limbs(b, x, z, 128, 0), 64, 0);
        CHECK(sv4_is_unknown(r) && r.x[0] == 0x8);
    }
    // %b / %h of a 128-bit value (exact strings)
    {
        sv4_format('h', w128(0x0123456789ABCDEFULL, 0xFEDCBA9876543210ULL),
                   buf, sizeof(buf));
        CHECK(strcmp(buf, "fedcba98765432100123456789abcdef") == 0);
        sv4_format('b', w128(0x0123456789ABCDEFULL, 0xFEDCBA9876543210ULL),
                   buf, sizeof(buf));
        CHECK(strcmp(buf,
            "1111111011011100101110101001100001110110010101000011001000010000"
            "0000000100100011010001010110011110001001101010111100110111101111")
            == 0);
        // bit 70 unknown -> the nibble covering bits 68..71 prints 'x'
        uint64_t b[2] = { 0x0123456789ABCDEFULL, 0xFEDCBA9876543210ULL };
        uint64_t x[2] = { 0, 1ULL << 6 }, z[2] = { 0, 0 };
        sv4_format('h', sv4_from_limbs(b, x, z, 128, 0), buf, sizeof(buf));
        CHECK(strcmp(buf, "fedcba98765432x00123456789abcdef") == 0);
        // %o of 128 bits: 43 digits, value 1 -> 42 zeros then '1'
        char expect_o[44];
        memset(expect_o, '0', 42);
        expect_o[42] = '1';
        expect_o[43] = 0;
        sv4_format('o', w128(1, 0), buf, sizeof(buf));
        CHECK(strcmp(buf, expect_o) == 0);
    }
    // sv4_to_dec_string of 2^100, 2^64 and all-X
    {
        sv4_to_dec_string(w128(0, 1ULL << 36), buf, sizeof(buf));
        CHECK(strcmp(buf, "1267650600228229401496703205376") == 0);
        sv4_to_dec_string(w128(0, 1), buf, sizeof(buf));
        CHECK(strcmp(buf, "18446744073709551616") == 0);
        sv4_to_dec_string(sv4_x(128, 0), buf, sizeof(buf));
        CHECK(strcmp(buf, "x") == 0);
        sv4_format('d', w128(0, 1ULL << 36), buf, sizeof(buf));
        CHECK(strcmp(buf, "1267650600228229401496703205376") == 0);
    }
    // 128-bit mux
    {
        sv4_t a = w128(1, 2), b = w128(3, 4);
        CHECK(sv4_same(sv4_mux(SV4_C(1, 1), a, b), a));
        CHECK(sv4_same(sv4_mux(SV4_C(0, 1), a, b), b));
        sv4_t mx = sv4_mux(SV4_X(1), a, b);
        CHECK(sv4_is_unknown(mx) && mx.width == 128);
        CHECK(sv4_same(sv4_mux(SV4_X(1), a, a), a));
    }
    // sv4_same across limbs (bits and xz in the high limb)
    {
        sv4_t a = w128(0x1111111111111111ULL, 0x2222222222222222ULL);
        CHECK(sv4_same(a, w128(0x1111111111111111ULL, 0x2222222222222222ULL)));
        CHECK(!sv4_same(a, w128(0x1111111111111111ULL, 0x2222222222222223ULL)));
        uint64_t b[2] = { 0x1111111111111111ULL, 0x2222222222222222ULL };
        uint64_t x[2] = { 0, 1 }, z[2] = { 0, 0 };
        CHECK(!sv4_same(a, sv4_from_limbs(b, x, z, 128, 0)));
    }
    // wide bitwise ops
    {
        sv4_t a = w128(0x0F0F0F0F0F0F0F0FULL, 0xFFFF0000FFFF0000ULL);
        sv4_t b = w128(0x00FF00FF00FF00FFULL, 0xFF00FF00FF00FF00ULL);
        CHECK(w_same(sv4_and(a, b), 0x000F000F000F000FULL, 0xFF000000FF000000ULL));
        CHECK(w_same(sv4_or(a, b), 0x0FFF0FFF0FFF0FFFULL, 0xFFFFFF00FFFFFF00ULL));
        CHECK(w_same(sv4_xor(a, b), 0x0FF00FF00FF00FF0ULL, 0x00FFFF0000FFFF00ULL));
    }
    // wide reductions and clog2
    CHECK(u(sv4_reduce_or(w128(0, 1))) == 1);
    CHECK(u(sv4_reduce_and(sv4_fill(1, 100, 0))) == 1);
    CHECK(u(sv4_reduce_xor(w128(3, 0))) == 0);
    CHECK(u(sv4_clog2(w128(0, 1ULL << 36))) == 100); // clog2(2^100)
    CHECK(u(sv4_clog2(w128(~0ULL, (1ULL << 36) - 1))) == 100); // clog2(2^100 - 1)
}

// ── Deterministic cross-check vector table ────────────────────────────────────
//
// The expected values are computed by `core::elab::Value` (Rust); the loop in
// `check_vector_table` verifies the C `sv4_*` ops agree with them, so the two
// 4-state implementations are cross-checked on identical inputs.  Regenerate
// with:
//
//   cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
//
// Field order: op, a_bits/a_x/a_z/a_w/a_s, b_bits/b_x/b_z/b_w/b_s,
// c_bits/c_x/c_z/c_w/c_s (mux selector / resize target), e_bits/e_x/e_z/e_w/e_s
// (expected).  All values fit limb 0 (width <= 64).

enum {
    V_ADD,
    V_SUB,
    V_MUL,
    V_DIV,
    V_MOD,
    V_POW,
    V_SHL,
    V_SHR,
    V_EQ,
    V_LT,
    V_MUX,
    V_RESIZE,
    V_CAST,
    V_CONCAT,
    V_CASEZ,
    V_CASEX,
};

typedef struct {
    int op;
    uint64_t a_bits, a_x, a_z;
    uint16_t a_w;
    int8_t a_s;
    uint64_t b_bits, b_x, b_z;
    uint16_t b_w;
    int8_t b_s;
    uint64_t c_bits, c_x, c_z;
    uint16_t c_w;
    int8_t c_s;
    uint64_t e_bits, e_x, e_z;
    uint16_t e_w;
    int8_t e_s;
} sv4_vec_t;

static const sv4_vec_t VECTORS[] = {
    // Generated by tests/property_elab.rs gen_c_vectors — do not edit by hand.
    { V_ADD, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xaULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_ADD, 0x9ULL, 0x2ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xfULL, 0x0ULL, 4, 0 },
    { V_ADD, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 1 },
    { V_ADD, 0xffULL, 0x0ULL, 0x0ULL, 8, 0, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_ADD, 0xfULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x10ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SUB, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x3ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x7ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_SUB, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x2ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xffULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SUB, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x2ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xffULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_SUB, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x9ULL, 0x2ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xfULL, 0x0ULL, 4, 0 },
    { V_MUL, 0x6ULL, 0x0ULL, 0x0ULL, 4, 0, 0x3ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x2ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_MUL, 0x2ULL, 0x0ULL, 0x0ULL, 8, 0, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x6ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_MUL, 0x0ULL, 0x80ULL, 0x0ULL, 8, 0, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xffULL, 0x0ULL, 8, 0 },
    { V_MUL, 0x0ULL, 0x0ULL, 0x80ULL, 8, 0, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xffULL, 0x0ULL, 8, 0 },
    { V_MUL, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x2ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xfeULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_ADD, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_ADD, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x10ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SUB, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xfeULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_SUB, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xeULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_MUL, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xffULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_MUL, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xfULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_DIV, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xffULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_DIV, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xfULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_MOD, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_MOD, 0xfULL, 0x0ULL, 0x0ULL, 4, 1, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_POW, 0x3ULL, 0x0ULL, 0x0ULL, 4, 0, 0x2ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_POW, 0xeULL, 0x0ULL, 0x0ULL, 4, 1, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1 },
    { V_POW, 0x2ULL, 0x0ULL, 0x0ULL, 4, 1, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 1 },
    { V_DIV, 0x8000000000000000ULL, 0x0ULL, 0x0ULL, 64, 1, 0xffffffffffffffffULL, 0x0ULL, 0x0ULL, 64, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x8000000000000000ULL, 0x0ULL, 0x0ULL, 64, 1 },
    { V_MOD, 0x8000000000000000ULL, 0x0ULL, 0x0ULL, 64, 1, 0xffffffffffffffffULL, 0x0ULL, 0x0ULL, 64, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 64, 1 },
    { V_SHL, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x2ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x4ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_SHL, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x4ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_SHL, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x8ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SHL, 0x1ULL, 0x8ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x2ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_SHL, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x8ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xfULL, 0x0ULL, 4, 0 },
    { V_SHR, 0x80ULL, 0x0ULL, 0x0ULL, 8, 0, 0x1ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x40ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SHR, 0x80ULL, 0x0ULL, 0x0ULL, 8, 0, 0x8ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_SHR, 0x80ULL, 0x0ULL, 0x0ULL, 8, 0, 0x1ULL, 0x80ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0xffULL, 0x0ULL, 8, 0 },
    { V_EQ, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_EQ, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x6ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_EQ, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x1ULL, 0x4ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x1ULL, 0x0ULL, 1, 0 },
    { V_EQ, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0x5ULL, 0x0ULL, 0x0ULL, 8, 0, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0x1ULL, 0x0ULL, 0x0ULL, 8, 1, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_LT, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x80ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x1ULL, 0x0ULL, 1, 0 },
    { V_LT, 0xffffffffffffffffULL, 0x0ULL, 0x0ULL, 64, 0, 0x1ULL, 0x0ULL, 0x0ULL, 64, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_MUX, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0, 0xaULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_MUX, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_MUX, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x1ULL, 0x0ULL, 1, 0, 0xaULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_MUX, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x1ULL, 0x0ULL, 1, 0, 0x0ULL, 0xfULL, 0x0ULL, 4, 0 },
    { V_MUX, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x5ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x1ULL, 1, 0, 0x0ULL, 0xfULL, 0x0ULL, 4, 0 },
    { V_MUX, 0x9ULL, 0x0ULL, 0x2ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0, 0x9ULL, 0x0ULL, 0x2ULL, 4, 0 },
    { V_RESIZE, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0xf8ULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_RESIZE, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x8ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_RESIZE, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 2, 0, 0x1ULL, 0x0ULL, 0x0ULL, 2, 0 },
    { V_RESIZE, 0x9ULL, 0x0ULL, 0x2ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x9ULL, 0x0ULL, 0x2ULL, 8, 0 },
    { V_RESIZE, 0x9ULL, 0x2ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x9ULL, 0x2ULL, 0x0ULL, 8, 0 },
    { V_RESIZE, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 1, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1 },
    { V_CAST, 0xffULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 16, 1, 0xffULL, 0x0ULL, 0x0ULL, 16, 1 },
    { V_CAST, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0x9ULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_CAST, 0xffULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 16, 0, 0xffffULL, 0x0ULL, 0x0ULL, 16, 0 },
    { V_CAST, 0xfeULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 16, 0, 0xfffeULL, 0x0ULL, 0x0ULL, 16, 0 },
    { V_CAST, 0x5ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0x5ULL, 0x0ULL, 0x0ULL, 8, 1 },
    { V_CAST, 0x3ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x3ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_CAST, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 1, 0x8ULL, 0x0ULL, 0x0ULL, 4, 1 },
    { V_CAST, 0xaULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 4, 0, 0xaULL, 0x0ULL, 0x0ULL, 4, 0 },
    { V_CAST, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 3, 1, 0x2ULL, 0x0ULL, 0x0ULL, 3, 1 },
    { V_CAST, 0x7ULL, 0x0ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 2, 0, 0x3ULL, 0x0ULL, 0x0ULL, 2, 0 },
    { V_CAST, 0x9ULL, 0x0ULL, 0x2ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0xf9ULL, 0x0ULL, 0x2ULL, 8, 1 },
    { V_CAST, 0x9ULL, 0x2ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0xf9ULL, 0x2ULL, 0x0ULL, 8, 1 },
    { V_CAST, 0x9ULL, 0x0ULL, 0x2ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0x9ULL, 0x0ULL, 0x2ULL, 8, 1 },
    { V_CAST, 0x0ULL, 0x8ULL, 0x0ULL, 4, 1, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 1, 0x0ULL, 0xf8ULL, 0x0ULL, 8, 1 },
    { V_CONCAT, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x1ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xa1ULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_CONCAT, 0xffULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 8, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xff00ULL, 0x0ULL, 0x0ULL, 16, 0 },
    { V_CONCAT, 0x9ULL, 0x0ULL, 0x2ULL, 4, 0, 0x1ULL, 0x4ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x91ULL, 0x4ULL, 0x20ULL, 8, 0 },
    { V_CONCAT, 0x0ULL, 0x0ULL, 0x0ULL, 4, 0, 0xfULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0xfULL, 0x0ULL, 0x0ULL, 8, 0 },
    { V_CASEZ, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x0ULL, 0x5ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0xeULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x0ULL, 0x5ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0x8ULL, 0x4ULL, 0x0ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0xaULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0x8ULL, 0x0ULL, 0x0ULL, 8, 0, 0x8ULL, 0x0ULL, 0x5ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0x8ULL, 0x0ULL, 0x4ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEZ, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0xcULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0x9ULL, 0x0ULL, 0x0ULL, 4, 0, 0x8ULL, 0x0ULL, 0x0ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x0ULL, 0x0ULL, 0x0ULL, 1, 0 },
    { V_CASEX, 0x8ULL, 0x0ULL, 0x0ULL, 8, 0, 0x8ULL, 0x4ULL, 0x1ULL, 4, 0, 0x0ULL, 0x0ULL, 0x0ULL, 0, 0, 0x1ULL, 0x0ULL, 0x0ULL, 1, 0 },
};
static const int N_VECTORS = (int)(sizeof(VECTORS) / sizeof(VECTORS[0]));

static void check_vector_table(void) {
    for (int i = 0; i < N_VECTORS; i++) {
        const sv4_vec_t* v = &VECTORS[i];
        sv4_t a = SV4_INIT(v->a_bits, v->a_x, v->a_z, v->a_w, v->a_s);
        sv4_t b = SV4_INIT(v->b_bits, v->b_x, v->b_z, v->b_w, v->b_s);
        sv4_t c = SV4_INIT(v->c_bits, v->c_x, v->c_z, v->c_w, v->c_s);
        sv4_t got;
        switch (v->op) {
            case V_ADD: got = sv4_add(a, b); break;
            case V_SUB: got = sv4_sub(a, b); break;
            case V_MUL: got = sv4_mul(a, b); break;
            case V_DIV: got = sv4_div(a, b); break;
            case V_MOD: got = sv4_mod(a, b); break;
            case V_POW: got = sv4_pow(a, b); break;
            case V_SHL: got = sv4_shl(a, b); break;
            case V_SHR: got = sv4_shr(a, b); break;
            case V_EQ: got = sv4_eq(a, b); break;
            case V_LT: got = sv4_lt(a, b); break;
            case V_MUX: got = sv4_mux(c, a, b); break;
            case V_RESIZE: got = sv4_resize(a, v->c_w, v->c_s); break;
            case V_CAST: got = sv4_cast(a, v->c_w, v->c_s); break;
            case V_CONCAT: got = sv4_concat(a, b); break;
            case V_CASEZ: got = sv4_casez_eq(a, b); break;
            case V_CASEX: got = sv4_casex_eq(a, b); break;
            default: fprintf(stderr, "FAIL vector %d: unknown op %d\n", i, v->op); failures++; continue;
        }
        sv4_t exp = SV4_INIT(v->e_bits, v->e_x, v->e_z, v->e_w, v->e_s);
        if (!sv4_same(got, exp)) {
            fprintf(stderr,
                    "FAIL vector %d (op %d): got %016llx/%016llx/%016llx w=%u, "
                    "expected %016llx/%016llx/%016llx w=%u\n",
                    i, v->op,
                    (unsigned long long)got.bits[0], (unsigned long long)got.x[0],
                    (unsigned long long)got.z[0], (unsigned)got.width,
                    (unsigned long long)exp.bits[0], (unsigned long long)exp.x[0],
                    (unsigned long long)exp.z[0], (unsigned)exp.width);
            failures++;
        }
    }
    if (failures == 0) printf("vector table (%d vectors): ok\n", N_VECTORS);
}

// ── Scheduler tests ───────────────────────────────────────────────────────────

static sv4_t s_a = SV4_C(0, 1), s_b = SV4_C(0, 1); // ping-pong signals
static int n_a, n_b;
static int ping_done;

static void proc_ping(llg_proc_t* self) {
    for (;;) {
        sv4_t* sg[] = { &s_b }; llg_wait_any(sg, 1);
        n_a++;
        llg_ba(&s_a, sv4_bitneg(s_a));
        if (n_a >= 50) {
            llg_rt_finish();
            llg_proc_done(self);
            return;
        }
    }
}

static void proc_pong(llg_proc_t* self) {
    for (;;) {
        sv4_t* sg[] = { &s_a }; llg_wait_any(sg, 1);
        n_b++;
        llg_ba(&s_b, sv4_bitneg(s_b));
    }
}

static void proc_kick(llg_proc_t* self) {
    llg_ba(&s_a, SV4_C(0, 1));
    llg_ba(&s_b, SV4_C(0, 1));
    llg_ba(&s_a, SV4_C(1, 1));
    llg_proc_done(self);
}

static double real_dependency_value;
static int real_dependency_wakes;
static int real_expression_wakes;

static void real_expression_eval(double* out, void* context) {
    (void)context;
    *out = real_dependency_value + 1.0;
}

static void real_dependency_waiter(llg_proc_t* self) {
    for (int i = 0; i < 3; i++) {
        llg_wait_dependency_t dependency = { NULL, &real_dependency_value };
        llg_wait_any_dependencies(&dependency, 1);
        real_dependency_wakes++;
    }
    llg_proc_done(self);
}

static void real_expression_waiter(llg_proc_t* self) {
    llg_wait_dependency_t dependency = { NULL, &real_dependency_value };
    llg_expr_event_spec_t expression = {
        .real_eval = real_expression_eval,
        .kind = LLG_EV_ANY,
        .dependencies = &dependency,
        .n_dependencies = 1,
        .real = 1,
    };
    for (int i = 0; i < 3; i++) {
        llg_wait_expressions(&expression, 1);
        real_expression_wakes++;
    }
    llg_proc_done(self);
}

static void real_dependency_writer(llg_proc_t* self) {
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, 0.0);
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, 1.0);
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, 1.0);
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, -0.0);
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, real_from_bits(0x7ff8000000000000ULL));
    llg_wait_time(1);
    llg_ba_d(&real_dependency_value, real_from_bits(0x7ff8000000000000ULL));
    llg_proc_done(self);
}

static void test_real_dependencies(void) {
    llg_rt_init();
    real_dependency_value = 0.0;
    real_dependency_wakes = 0;
    real_expression_wakes = 0;
    llg_spawn(real_dependency_waiter, "real-dependency-waiter");
    llg_spawn(real_expression_waiter, "real-expression-waiter");
    llg_spawn(real_dependency_writer, "real-dependency-writer");
    llg_rt_run();
    CHECK(real_dependency_wakes == 3);
    CHECK(real_expression_wakes == 3);
}

// Delay ordering: two processes with staggered #delays must wake in time order.
static uint64_t order_log[8];
static int order_n;
static int delay_ok;

static void proc_delay_a(llg_proc_t* self) {
    llg_wait_time(10);
    order_log[order_n++] = llg_time();
    llg_wait_time(5);
    order_log[order_n++] = llg_time();
    llg_proc_done(self);
}

static void proc_delay_b(llg_proc_t* self) {
    llg_wait_time(5);
    order_log[order_n++] = llg_time();
    llg_wait_time(10);
    order_log[order_n++] = llg_time();
    llg_proc_done(self);
}

// NBA visibility: writer does `a = 1` (blocking) then `b <= 1` (NBA) at t=1.
// A reader woken by the edge of `a` must still see the OLD `b` (0); a level
// waiter on b must only fire after the NBA region commits.
static sv4_t n_sig_a = SV4_C(0, 1), n_sig_b = SV4_C(0, 1);
static int nba_read_old_ok;
static int nba_level_ok;

static void proc_nba_write(llg_proc_t* self) {
    llg_wait_time(1);
    llg_ba(&n_sig_a, SV4_C(1, 1));
    llg_nba(&n_sig_b, SV4_C(1, 1));
    llg_wait_time(1);
    nba_level_ok = nba_level_ok && u(n_sig_b) == 1; // committed by now
    llg_rt_finish();
    llg_proc_done(self);
}

static void proc_nba_read(llg_proc_t* self) {
    llg_wait_edge(&n_sig_a, 1);
    // NBA region for t=1 has NOT run yet: b must still hold its old value 0.
    nba_read_old_ok = u(n_sig_b) == 0;
    llg_wait_level(&n_sig_b, SV4_C(1, 1));
    nba_level_ok = u(n_sig_b) == 1;
    llg_proc_done(self);
}

static void lifecycle_trigger_proc(llg_proc_t* self) {
    llg_event_trigger(&lifecycle_event);
    lifecycle_triggered_inside_run = llg_event_triggered(&lifecycle_event);
    llg_rt_finish();
    llg_proc_done(self);
}

static void test_event_triggered_lifecycle(void) {
    // Event objects are generated as static storage. A completed run must
    // invalidate their persistent state before a later run starts, including
    // when the trigger happened at time zero.
    llg_rt_init();
    memset(&lifecycle_event_object, 0, sizeof(lifecycle_event_object));
    lifecycle_event.object = &lifecycle_event_object;
    lifecycle_triggered_inside_run = 0;
    llg_spawn(lifecycle_trigger_proc, "event-lifecycle");
    llg_rt_run();
    CHECK(lifecycle_triggered_inside_run);
    CHECK(!llg_event_triggered(&lifecycle_event));

    llg_rt_init();
    CHECK(!llg_event_triggered(&lifecycle_event));
    llg_rt_cleanup();
}

static void test_scheduler(void) {
    // ping-pong
    llg_rt_init();
    llg_spawn(proc_ping, "ping");
    llg_spawn(proc_pong, "pong");
    llg_spawn(proc_kick, "kick");
    llg_rt_run();
    ping_done = 1;
    CHECK(n_a > 0 && n_b > 0);
    CHECK(n_a >= 50);           // ping finished after its own count
    CHECK(n_a - n_b <= 1 && n_b - n_a <= 1);

    // delay ordering
    llg_rt_init();
    order_n = 0;
    llg_spawn(proc_delay_a, "da");
    llg_spawn(proc_delay_b, "db");
    llg_rt_run();
    CHECK(order_n == 4);
    if (order_n == 4) {
        CHECK(order_log[0] == 5 && order_log[1] == 10 && order_log[2] == 15 &&
              order_log[3] == 15);
    }
    delay_ok = 1;

    // NBA visibility
    llg_rt_init();
    nba_read_old_ok = 0;
    nba_level_ok = 0;
    llg_spawn(proc_nba_write, "nw");
    llg_spawn(proc_nba_read, "nr");
    llg_rt_run();
    CHECK(nba_read_old_ok);
    CHECK(nba_level_ok);
}

// ── fork/join tests ───────────────────────────────────────────────────────────

// (1) fork...join: two children waiting #5 / #10.  The parent resumes only
// after BOTH have finished, in completion order (5, 10, then the parent).
static sv4_t fj1_sig_a = SV4_C(0, 1), fj1_sig_b = SV4_C(0, 1);
static uint64_t fj1_log[8];
static int fj1_log_n;
static int fj1_ok;

static void fj1_child_a(llg_proc_t* self) {
    llg_wait_time(5);
    llg_ba(&fj1_sig_a, SV4_C(1, 1));
    fj1_log[fj1_log_n++] = llg_time();
    llg_proc_done(self);
}

static void fj1_child_b(llg_proc_t* self) {
    llg_wait_time(10);
    llg_ba(&fj1_sig_b, SV4_C(1, 1));
    fj1_log[fj1_log_n++] = llg_time();
    llg_proc_done(self);
}

static void fj1_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN);
    llg_fork(fj1_child_a, "fj1a", grp);
    llg_fork(fj1_child_b, "fj1b", grp);
    llg_join(grp); // resumes at t=10, only after both children finished
    fj1_log[fj1_log_n++] = llg_time();
    fj1_ok = u(fj1_sig_a) == 1 && u(fj1_sig_b) == 1;
    llg_rt_finish();
    llg_proc_done(self);
}

// (2) join_any: the parent resumes on the FIRST completion; the other child
// keeps running and both signals end up written.
static sv4_t fj2_sig_a = SV4_C(0, 1), fj2_sig_b = SV4_C(0, 1);
static int fj2_ok;

static void fj2_child_a(llg_proc_t* self) {
    llg_wait_time(5);
    llg_ba(&fj2_sig_a, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj2_child_b(llg_proc_t* self) {
    llg_wait_time(10);
    llg_ba(&fj2_sig_b, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj2_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_ANY);
    llg_fork(fj2_child_a, "fj2a", grp);
    llg_fork(fj2_child_b, "fj2b", grp);
    llg_join(grp); // wakes at t=5 on the first completion
    int first_ok = u(fj2_sig_a) == 1 && u(fj2_sig_b) == 0;
    llg_wait_fork(); // blocks until the whole group completes (t=10)
    fj2_ok = first_ok && u(fj2_sig_a) == 1 && u(fj2_sig_b) == 1;
    llg_rt_finish();
    llg_proc_done(self);
}

// (3) join_none: llg_join returns immediately (children still pending) and
// the children complete on their own.
static sv4_t fj3_sig_a = SV4_C(0, 1), fj3_sig_b = SV4_C(0, 1);
static int fj3_immediate_ok, fj3_done_ok;

static void fj3_child_a(llg_proc_t* self) {
    llg_wait_time(5);
    llg_ba(&fj3_sig_a, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj3_child_b(llg_proc_t* self) {
    llg_wait_time(10);
    llg_ba(&fj3_sig_b, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj3_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(fj3_child_a, "fj3a", grp);
    llg_fork(fj3_child_b, "fj3b", grp);
    uint64_t t0 = llg_time();
    llg_join(grp); // returns immediately, no yield
    fj3_immediate_ok = (llg_time() == t0) && u(fj3_sig_a) == 0 && u(fj3_sig_b) == 0;
    llg_wait_time(15); // children finish on their own
    fj3_done_ok = u(fj3_sig_a) == 1 && u(fj3_sig_b) == 1;
    llg_rt_finish();
    llg_proc_done(self);
}

// (4) wait_fork after join_none: blocks until the group completes.
static sv4_t fj4_sig_a = SV4_C(0, 1), fj4_sig_b = SV4_C(0, 1);
static int fj4_ok;

static void fj4_child_a(llg_proc_t* self) {
    llg_wait_time(5);
    llg_ba(&fj4_sig_a, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj4_child_b(llg_proc_t* self) {
    llg_wait_time(10);
    llg_ba(&fj4_sig_b, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj4_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(fj4_child_a, "fj4a", grp);
    llg_fork(fj4_child_b, "fj4b", grp);
    llg_join(grp);      // immediate (join_none)
    llg_wait_fork();    // must block until the slowest child (t=10)
    fj4_ok = (llg_time() == 10) && u(fj4_sig_a) == 1 && u(fj4_sig_b) == 1;
    llg_rt_finish();
    llg_proc_done(self);
}

// (5) disable_fork: a child waiting on a long delay is killed; its signal
// stays unset and the simulation ends without hanging (no $finish below: the
// run loop must terminate on its own once the killed child's wait is gone).
static sv4_t fj5_sig = SV4_C(0, 1);
static int fj5_ok;

static void fj5_child(llg_proc_t* self) {
    llg_wait_time(100);
    llg_ba(&fj5_sig, SV4_C(1, 1));
    llg_proc_done(self);
}

static void fj5_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(fj5_child, "fj5", grp);
    llg_wait_time(5);   // let the child register its #100 wait
    llg_disable_fork(); // kill the child
    llg_wait_time(5);   // t=10: well past the child's #100 wakeup would have been
    fj5_ok = u(fj5_sig) == 0;
    llg_proc_done(self); // no finish: sim must end because nothing is left
}

// (6) a killed child's pending NBA is NOT committed: the child records a
// non-blocking assignment, wakes the parent, then is killed before the NBA
// region runs.  fj6_sig must stay 0.
static sv4_t fj6_go = SV4_C(0, 1), fj6_sig = SV4_C(0, 1);
static int fj6_ok;

static void fj6_child(llg_proc_t* self) {
    llg_nba(&fj6_sig, SV4_C(1, 1)); // pending non-blocking assignment
    llg_ba(&fj6_go, SV4_C(1, 1));   // wake the parent before being killed
    llg_wait_time(100);
    llg_ba(&fj6_sig, SV4_C(1, 1));  // never reached
    llg_proc_done(self);
}

static void fj6_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(fj6_child, "fj6", grp);
    llg_wait_edge(&fj6_go, 1); // t=0: child recorded its NBA and signaled us
    llg_disable_fork();        // kill it before the NBA region commits
    llg_wait_time(5);
    fj6_ok = u(fj6_sig) == 0;   // the pending NBA must never be committed
    llg_rt_finish();
    llg_proc_done(self);
}

// (7) nested join_none: a completed child stays alive while its detached
// descendant still uses the child as its fork-group parent.
static int fj7_descendant_done, fj7_ok;

static void fj7_grandchild(llg_proc_t* self) {
    llg_wait_time(2);
    fj7_descendant_done = 1;
    llg_proc_done(self);
}

static void fj7_child(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(fj7_grandchild, "fj7g", grp);
    llg_join(grp);
    llg_proc_done(self);
}

static void fj7_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN);
    llg_fork(fj7_child, "fj7c", grp);
    llg_join(grp);
    llg_wait_time(1);
    int parent_retained = llg_rt_process_count() == 3;
    llg_wait_time(2);
    fj7_ok = parent_retained && fj7_descendant_done &&
             llg_rt_process_count() == 1 && llg_time() == 3;
    llg_rt_finish();
    llg_proc_done(self);
}

// (8) completed fork slots are reusable across more than the process-table
// capacity when only one child is live at a time.
static int fj8_count, fj8_ok;

static void fj8_child(llg_proc_t* self) {
    llg_wait_time(1);
    fj8_count++;
    llg_proc_done(self);
}

static void fj8_parent(llg_proc_t* self) {
    const int total = LLG_MAX_PROCS + 64;
    for (int i = 0; i < total; i++) {
        llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN);
        llg_fork(fj8_child, "fj8c", grp);
        llg_join(grp);
    }
    fj8_ok = fj8_count == total;
    llg_rt_finish();
    llg_proc_done(self);
}

// (9) an empty fork group is finalized even though no child can issue the
// completion callback; a following wait_fork therefore returns immediately.
static int fj9_ok;

static void fj9_parent(llg_proc_t* self) {
    llg_fork_group_t* grp = llg_fork_group_new(LLG_JOIN_NONE);
    llg_join(grp);
    uint64_t before = llg_time();
    llg_wait_fork();
    fj9_ok = llg_time() == before && llg_rt_process_count() == 1;
    llg_rt_finish();
    llg_proc_done(self);
}

static void test_fork_join(void) {
    // (1) fork...join, children in completion order
    llg_rt_init();
    fj1_log_n = 0;
    fj1_ok = 0;
    llg_spawn(fj1_parent, "fj1p");
    llg_rt_run();
    CHECK(fj1_ok);
    if (fj1_log_n == 3) {
        CHECK(fj1_log[0] == 5 && fj1_log[1] == 10 && fj1_log[2] == 10);
    } else {
        CHECK(fj1_log_n == 3); // log length
    }

    // (2) join_any
    llg_rt_init();
    fj2_ok = 0;
    llg_spawn(fj2_parent, "fj2p");
    llg_rt_run();
    CHECK(fj2_ok);

    // (3) join_none
    llg_rt_init();
    fj3_immediate_ok = 0;
    fj3_done_ok = 0;
    llg_spawn(fj3_parent, "fj3p");
    llg_rt_run();
    CHECK(fj3_immediate_ok);
    CHECK(fj3_done_ok);

    // (4) wait_fork after join_none
    llg_rt_init();
    fj4_ok = 0;
    llg_spawn(fj4_parent, "fj4p");
    llg_rt_run();
    CHECK(fj4_ok);

    // (5) disable_fork
    llg_rt_init();
    fj5_ok = 0;
    llg_spawn(fj5_parent, "fj5p");
    llg_rt_run();
    CHECK(fj5_ok);

    // (6) killed child's pending NBA is not committed
    llg_rt_init();
    fj6_ok = 0;
    llg_spawn(fj6_parent, "fj6p");
    llg_rt_run();
    CHECK(fj6_ok);

    // (7) a detached grandchild outlives its completed parent safely
    llg_rt_init();
    fj7_descendant_done = 0;
    fj7_ok = 0;
    llg_spawn(fj7_parent, "fj7p");
    llg_rt_run();
    CHECK(fj7_ok);

    // (8) cumulative children reuse completed process-table slots
    llg_rt_init();
    fj8_count = 0;
    fj8_ok = 0;
    llg_spawn(fj8_parent, "fj8p");
    llg_rt_run();
    CHECK(fj8_ok);

    // (9) an empty join_none group does not block a following wait_fork
    llg_rt_init();
    fj9_ok = 0;
    llg_spawn(fj9_parent, "fj9p");
    llg_rt_run();
    CHECK(fj9_ok);

    if (failures == 0) printf("fork/join: ok\n");
}

// ── Collapsed inout-net tests ─────────────────────────────────────────────────

static void test_llg_net(void) {
    // Resolution vectors (LRM wire/tri, equal strengths, 1-bit):
    //   z+z -> z, z+0 -> 0, z+1 -> 1, 0+1 -> x, x+0 -> x, equal -> same,
    //   all-z -> z.
    {
        sv4_t d0 = SV4_Z(1), d1 = SV4_Z(1);
        llg_net_t net = { SV4_Z(1), 1, 0, LLG_RESOLVE_WIRE, 2,
                          { &d0, &d1 }, { 6, 6 }, { 6, 6 } };
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_Z(1))); // z+z -> z
        d0 = SV4_C(0, 1);
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_C(0, 1))); // z+0 -> 0
        d1 = SV4_C(1, 1);
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_X(1))); // 0+1 -> x
        d0 = SV4_X(1);
        d1 = SV4_C(0, 1);
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_X(1))); // x+0 -> x
        d0 = SV4_C(1, 1);
        d1 = SV4_C(1, 1);
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_C(1, 1))); // equal -> same
        d0 = SV4_Z(1);
        d1 = SV4_Z(1);
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_Z(1))); // all-z -> z
    }
    // No driver -> all-Z.
    {
        llg_net_t net = { SV4_Z(4), 4, 0, LLG_RESOLVE_WIRE, 0,
                          { NULL }, { 0 }, { 0 } };
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_Z(4)));
    }
    // Driver-slot early-out: an unchanged slot write must not re-resolve or
    // disturb the resolved value.
    {
        sv4_t d0 = SV4_Z(8), d1 = SV4_Z(8);
        llg_net_t net = { SV4_Z(8), 8, 0, LLG_RESOLVE_WIRE, 2,
                          { &d0, &d1 }, { 6, 6 }, { 6, 6 } };
        llg_net_resolve(&net);
        sv4_t before = net.resolved;
        llg_net_write(&net, 0, SV4_Z(8)); // same value -> early-out
        CHECK(sv4_same(net.resolved, before));
        CHECK(sv4_same(d0, SV4_Z(8)));
    }
    // Write path: 0x5a onto a Z slot resolves to 0x5a; a repeat write is an
    // early-out (the resolved cell is untouched).
    {
        sv4_t d0 = SV4_Z(8), d1 = SV4_Z(8);
        llg_net_t net = { SV4_Z(8), 8, 0, LLG_RESOLVE_WIRE, 2,
                          { &d0, &d1 }, { 6, 6 }, { 6, 6 } };
        llg_net_resolve(&net);
        llg_net_write(&net, 0, SV4_C(0x5a, 8));
        CHECK(sv4_same(d0, SV4_C(0x5a, 8)));
        CHECK(sv4_same(net.resolved, SV4_C(0x5a, 8)));
        llg_net_write(&net, 0, SV4_C(0x5a, 8));
        CHECK(sv4_same(net.resolved, SV4_C(0x5a, 8)));
    }
    // Multi-bit: 0x0a + 0x0b conflicts only in bit 0 (low nibble) -> 0x0a
    // with bit 0 X, which %h prints as "0x".
    {
        sv4_t d0 = SV4_C(0x0a, 8), d1 = SV4_C(0x0b, 8);
        llg_net_t net = { SV4_Z(8), 8, 0, LLG_RESOLVE_WIRE, 2,
                          { &d0, &d1 }, { 6, 6 }, { 6, 6 } };
        llg_net_resolve(&net);
        char buf[64];
        sv4_format('h', net.resolved, buf, sizeof(buf));
        CHECK(strcmp(buf, "0x") == 0);
    }
    // A Z driver loses to a known driver: z+0x5a -> 0x5a (already covered
    // above); a known driver loses to X: 0x5a + X -> X.
    {
        sv4_t d0 = SV4_C(0x5a, 8), d1 = SV4_X(8);
        llg_net_t net = { SV4_Z(8), 8, 0, LLG_RESOLVE_WIRE, 2,
                          { &d0, &d1 }, { 6, 6 }, { 6, 6 } };
        llg_net_resolve(&net);
        CHECK(sv4_same(net.resolved, SV4_X(8)));
    }
}

// ── force / release tests ─────────────────────────────────────────────────────

static sv4_t f_sig = SV4_C(0, 8);

static sv4_t f_live_target = SV4_C(0, 1);
static sv4_t f_live_source = SV4_C(1, 1);

static void f_live_eval(sv4_t* out) {
    *out = f_live_source;
}

static void test_force_live_expression(void) {
    llg_rt_init();
    f_live_target = SV4_C(0, 1);
    f_live_source = SV4_C(1, 1);
    llg_force_read_t reads[] = {{&f_live_source, NULL, 0}};
    llg_force_part_t part = {&f_live_target, NULL, 0, 0, 1, 0, 0};
    llg_force_expr_parts(&part, 1, 0, 0, f_live_eval, reads, 1);
    CHECK(u(f_live_target) == 1);
    llg_ba(&f_live_source, SV4_C(0, 1));
    CHECK(u(f_live_target) == 0);
    llg_release_parts(&part, 1, 0, 0);
    CHECK(u(f_live_target) == 0);
}

static void test_force_release(void) {
    // A procedural variable retains the currently forced value on release.
    llg_rt_init();
    f_sig = SV4_C(0x11, 8);
    llg_force(&f_sig, SV4_C(0xff, 8));
    CHECK(u(f_sig) == 0xff);
    llg_ba(&f_sig, SV4_C(0x22, 8)); // dropped while forced
    CHECK(u(f_sig) == 0xff);
    llg_release(&f_sig);
    CHECK(u(f_sig) == 0xff); // no stale pre-force value is restored

    // force twice: the second force replaces the live value and release keeps
    // that replacement.
    llg_rt_init();
    f_sig = SV4_C(0x11, 8);
    llg_force(&f_sig, SV4_C(0xaa, 8));
    llg_force(&f_sig, SV4_C(0xbb, 8));
    CHECK(u(f_sig) == 0xbb);
    llg_ba(&f_sig, SV4_C(0xcc, 8)); // still dropped
    CHECK(u(f_sig) == 0xbb);
    llg_release(&f_sig);
    CHECK(u(f_sig) == 0xbb); // the latest forced value is retained

    // releasing an unforced signal is a no-op (LRM 10.6.2)
    llg_rt_init();
    f_sig = SV4_C(0x55, 8);
    llg_release(&f_sig);
    CHECK(u(f_sig) == 0x55);

    if (failures == 0) printf("force/release: ok\n");
}

// NBA to a forced target is dropped at commit: the writer forces x, records
// `x <= 8'h5a`, and the NBA region must NOT overwrite the forced value.
static sv4_t f_nba_sig = SV4_C(0, 8);
static int f_nba_ok;

static void f_nba_writer(llg_proc_t* self) {
    llg_force(&f_nba_sig, SV4_C(0xff, 8));
    llg_nba(&f_nba_sig, SV4_C(0x5a, 8));
    llg_wait_time(1); // the NBA region commits before t=1
    f_nba_ok = u(f_nba_sig) == 0xff;
    llg_rt_finish();
    llg_proc_done(self);
}

static void test_force_nba_dropped(void) {
    llg_rt_init();
    f_nba_sig = SV4_C(0, 8);
    f_nba_ok = 0;
    llg_spawn(f_nba_writer, "fnba");
    llg_rt_run();
    CHECK(f_nba_ok);
}

// A `wait_any` on the signal fires when `force` writes the forced value.
static sv4_t f_wait_sig = SV4_C(0, 8);
static int f_wait_woken;
static uint64_t f_wait_seen;

static void f_wait_consumer(llg_proc_t* self) {
    sv4_t* sg[] = { &f_wait_sig };
    llg_wait_any(sg, 1);
    f_wait_woken = 1;
    f_wait_seen = u(f_wait_sig);
    llg_rt_finish();
    llg_proc_done(self);
}

static void f_wait_producer(llg_proc_t* self) {
    llg_wait_time(1); // let the consumer register its wait first
    llg_force(&f_wait_sig, SV4_C(0xaa, 8));
    llg_proc_done(self);
}

static void test_force_wakes_waiters(void) {
    llg_rt_init();
    f_wait_sig = SV4_C(0, 8);
    f_wait_woken = 0;
    f_wait_seen = 0;
    llg_spawn(f_wait_consumer, "fwc");
    llg_spawn(f_wait_producer, "fwp");
    llg_rt_run();
    CHECK(f_wait_woken);
    CHECK(f_wait_seen == 0xaa);
}

// Boundary probe run by a separate Rust subprocess test: the first delay
// reaches the largest scheduler timestamp and the second must terminate with
// the runtime's explicit overflow diagnostic instead of wrapping to zero.
static void time_overflow_proc(llg_proc_t* self) {
    llg_wait_time(UINT64_MAX);
    llg_wait_time(1);
    llg_proc_done(self);
}

static llg_inertial_t* inertial_handle;
static sv4_t inertial_target;

static void inertial_producer(llg_proc_t* self) {
    llg_inertial_assign(&inertial_handle, &inertial_target, SV4_C(1, 1), 2, 2, 2);
    llg_proc_done(self);
}

static void inertial_observer(llg_proc_t* self) {
    llg_wait_time(3);
    CHECK(sv4_same(inertial_target, SV4_C(1, 1)));
    llg_rt_finish();
    llg_proc_done(self);
}

static void test_inertial_lifetime(void) {
    for (int run = 0; run < 2; run++) {
        llg_rt_init();
        inertial_target = sv4_x(1, 0);
        llg_spawn(inertial_producer, "inertial-producer");
        llg_spawn(inertial_observer, "inertial-observer");
        llg_rt_run();
        CHECK(inertial_handle == NULL);
    }
    llg_rt_init();
    inertial_target = sv4_x(1, 0);
    llg_inertial_assign(&inertial_handle, &inertial_target, SV4_C(0, 1), 100, 100, 100);
    CHECK(inertial_handle != NULL);
    llg_rt_init();
    CHECK(inertial_handle == NULL);
    llg_rt_cleanup();
}

static int run_time_overflow_probe(void) {
    llg_rt_init();
    llg_spawn(time_overflow_proc, "time-overflow");
    llg_rt_run();
    return 2; // the second wait must abort before the scheduler returns
}

static void scaled_time_overflow_proc(llg_proc_t* self) {
    llg_wait_time(UINT64_MAX);
    (void)llg_time_scaled(2, 1);
    llg_proc_done(self);
}

static int run_scaled_time_overflow_probe(void) {
    llg_rt_init();
    llg_spawn(scaled_time_overflow_proc, "scaled-time-overflow");
    llg_rt_run();
    return 2; // llg_time_scaled must abort before the scheduler returns
}

static int budget_finite_count;

static void budget_finite_proc(llg_proc_t* self) {
    for (int i = 0; i < 4; i++) {
        llg_budget_point("selftest.sv:1:1");
        budget_finite_count++;
    }
    llg_rt_finish();
    llg_proc_done(self);
}

static void budget_infinite_proc(llg_proc_t* self) {
    (void)self;
    for (;;) llg_budget_point("selftest.sv:2:1");
}

static int run_budget_finite_probe(void) {
    llg_rt_init();
    budget_finite_count = 0;
    llg_spawn(budget_finite_proc, "budget-finite");
    llg_rt_run();
    return !llg_rt_failed() && budget_finite_count == 4 ? 0 : 1;
}

static int run_budget_infinite_probe(void) {
    llg_rt_init();
    llg_spawn(budget_infinite_proc, "budget-infinite");
    llg_rt_run();
    return llg_rt_failed() ? 0 : 1;
}

static void time_scaled_rounding_proc(llg_proc_t* self) {
    CHECK(llg_time_scaled(1, 10) == 0);
    llg_wait_time(14);
    CHECK(llg_time_scaled(1, 10) == 1);
    llg_wait_time(1);
    CHECK(llg_time_scaled(1, 10) == 2);
    llg_wait_time(1);
    CHECK(llg_time_scaled(1, 10) == 2);
    llg_rt_finish();
    llg_proc_done(self);
}

static void test_time_scaled_rounding(void) {
    llg_rt_init();
    llg_spawn(time_scaled_rounding_proc, "time-scaled-rounding");
    llg_rt_run();
}

static void test_activation_frames(void) {
    sv4_t target = SV4_C(0, 8);
    double real_target = 1.25;
    llg_frame_t* frame = llg_frame_new(3);
    llg_frame_capture_value(frame, 0, SV4_C(0x5a, 8));
    llg_frame_alias_value(frame, 1, &target);
    llg_frame_capture_real(frame, 2, real_target);
    CHECK(sv4_same(llg_frame_read_value(frame, 0), SV4_C(0x5a, 8)));
    CHECK(sv4_same(llg_frame_read_value(frame, 1), SV4_C(0, 8)));
    CHECK(llg_frame_slot_kind(frame, 2) == LLG_FRAME_REAL);
    CHECK(llg_frame_read_real(frame, 2) == 1.25);
    llg_frame_retain(frame);
    llg_frame_release(frame);
    llg_frame_write_value(frame, 1, SV4_C(0xa5, 8));
    llg_frame_write_real(frame, 2, 2.5);
    CHECK(sv4_same(target, SV4_C(0xa5, 8)));
    CHECK(llg_frame_read_real(frame, 2) == 2.5);
    llg_frame_release(frame);

    llg_frame_t* source = llg_frame_new(1);
    llg_frame_capture_real(source, 0, 3.5);
    llg_frame_t* alias = llg_frame_new(1);
    llg_frame_alias_slot(alias, 0, source, 0);
    llg_frame_release(source);
    CHECK(llg_frame_read_real(alias, 0) == 3.5);
    llg_frame_write_real(alias, 0, 4.5);
    CHECK(llg_frame_read_real(alias, 0) == 4.5);
    llg_frame_release(alias);
}

static sv4_t activation_cancel_target;

static void activation_cancel_child(llg_proc_t* self) {
    llg_wait_time(100);
    llg_frame_write_value(llg_proc_frame(self), 0, SV4_C(1, 1));
    llg_proc_done(self);
}

static void activation_cancel_parent(llg_proc_t* self) {
    llg_frame_t* frame = llg_frame_new(1);
    llg_frame_alias_value(frame, 0, &activation_cancel_target);
    llg_fork_group_t* group = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork_with_frame(activation_cancel_child, "activation-cancel-child", group, frame);
    llg_frame_release(frame);
    llg_disable_fork();
    llg_rt_request_finish();
    llg_proc_done(self);
}

static void test_activation_frame_cancellation(void) {
    llg_rt_init();
    activation_cancel_target = SV4_C(0, 1);
    llg_spawn(activation_cancel_parent, "activation-cancel-parent");
    llg_rt_run();
    CHECK(sv4_same(activation_cancel_target, SV4_C(0, 1)));
    CHECK(!llg_rt_failed());
}

static int region_trace[LLG_REGION_COUNT];
static int region_trace_args[LLG_REGION_COUNT];
static int region_trace_n;
static int region_reentry_seen;
static int region_timed_seen;
static int region_pre_postponed_reentry_seen;
static int region_observed_reactive_seen;
static sv4_t region_sample_signal;
static sv4_t region_resume_signal;
static int region_resume_seen;

static void region_trace_callback(void* data) {
    int expected = *(const int*)data;
    int actual = (int)llg_current_region();
    CHECK(actual == expected);
    region_trace[actual]++;
    region_trace_n++;
    if (actual == LLG_REGION_PREPONED) {
        const sv4_t* sampled = llg_sampled_value(&region_sample_signal);
        CHECK(sampled && sv4_same(*sampled, SV4_C(0, 1)));
    }
    if (actual == LLG_REGION_REACTIVE) {
        CHECK(llg_schedule_region_callback(
            LLG_REGION_ACTIVE, region_trace_callback,
            &region_trace_args[LLG_REGION_ACTIVE]) == 1);
    }
    if (actual == LLG_REGION_POSTPONED_PLI) llg_rt_request_finish();
}

static void region_read_only_callback(void* data) {
    (void)data;
    llg_ba(&region_sample_signal, SV4_C(1, 1));
}

static void region_illegal_schedule_callback(void* data) {
    (void)data;
    CHECK(llg_schedule_region_callback(
              LLG_REGION_ACTIVE, region_trace_callback,
              &region_trace_args[LLG_REGION_ACTIVE]) == 0);
}

static void region_pre_postponed_write_callback(void* data) {
    (void)data;
    llg_ba(&region_sample_signal, SV4_C(1, 1));
}

static void region_pre_postponed_waiter(llg_proc_t* self) {
    llg_wait_level(&region_sample_signal, SV4_C(1, 1));
    CHECK(llg_current_region() == LLG_REGION_ACTIVE);
    region_pre_postponed_reentry_seen = 1;
    llg_proc_done(self);
}

static void region_reactive_followup(void* data) {
    (void)data;
    CHECK(llg_current_region() == LLG_REGION_REACTIVE);
    region_observed_reactive_seen = 1;
    CHECK(llg_schedule_region_callback(
              LLG_REGION_ACTIVE, region_trace_callback,
              &region_trace_args[LLG_REGION_ACTIVE]) == 1);
}

static void region_observed_callback(void* data) {
    (void)data;
    CHECK(llg_current_region() == LLG_REGION_OBSERVED);
    CHECK(llg_schedule_region_callback(
              LLG_REGION_REACTIVE, region_reactive_followup, NULL) == 1);
}

static void region_resume_write(void* data) {
    (void)data;
    llg_ba(&region_resume_signal, SV4_C(1, 1));
}

static void region_resume_proc(llg_proc_t* self) {
    CHECK(llg_schedule_region_callback(
              LLG_REGION_ACTIVE, region_resume_write, NULL) == 1);
    llg_wait_resume_in_region(LLG_REGION_REACTIVE);
    llg_wait_any((sv4_t*[]){&region_resume_signal}, 1);
    CHECK(llg_current_region() == LLG_REGION_REACTIVE);
    region_resume_seen++;
    llg_rt_request_finish();
    llg_proc_done(self);
}

static void region_timed_callback(void* data) {
    int expected = *(const int*)data;
    CHECK(llg_current_region() == (llg_region_t)expected);
    CHECK(llg_time() == 1);
    region_timed_seen++;
    llg_rt_request_finish();
}

static int run_region_probe(void) {
    llg_rt_init();
    region_sample_signal = SV4_C(0, 1);
    llg_sampled_register(&region_sample_signal);
    memset(region_trace, 0, sizeof(region_trace));
    region_trace_n = 0;
    region_reentry_seen = 0;
    for (int i = 0; i < LLG_REGION_COUNT; i++) {
        region_trace_args[i] = i;
        CHECK(llg_schedule_region_callback(
                  (llg_region_t)i, region_trace_callback, &region_trace_args[i]) == 1);
    }
    CHECK(llg_register_pli_callback(
              LLG_REGION_PRE_ACTIVE_PLI, region_trace_callback,
              &region_trace_args[LLG_REGION_PRE_ACTIVE_PLI]) == 1);
    llg_rt_run();
    for (int i = 0; i < LLG_REGION_COUNT; i++) CHECK(region_trace[i] >= 1);
    CHECK(region_trace[LLG_REGION_ACTIVE] == 2);
    CHECK(region_trace_n == LLG_REGION_COUNT + 2);
    region_reentry_seen = region_trace[LLG_REGION_ACTIVE] == 2;
    CHECK(region_reentry_seen);
    CHECK(!llg_rt_failed());

    llg_rt_init();
    region_resume_signal = SV4_C(0, 1);
    region_resume_seen = 0;
    llg_spawn(region_resume_proc, "explicit-region-resume");
    llg_rt_run();
    CHECK(region_resume_seen == 1);
    CHECK(!llg_rt_failed());

    llg_rt_init();
    region_timed_seen = 0;
    region_trace_args[LLG_REGION_ACTIVE] = LLG_REGION_ACTIVE;
    CHECK(llg_schedule_region_callback_after(
              LLG_REGION_ACTIVE, region_timed_callback,
              &region_trace_args[LLG_REGION_ACTIVE], 1) == 1);
    llg_rt_run();
    CHECK(region_timed_seen == 1);
    CHECK(!llg_rt_failed());

    llg_rt_init();
    memset(region_trace, 0, sizeof(region_trace));
    region_observed_reactive_seen = 0;
    region_trace_args[LLG_REGION_POSTPONED_PLI] = LLG_REGION_POSTPONED_PLI;
    CHECK(llg_schedule_region_callback(
              LLG_REGION_OBSERVED, region_observed_callback, NULL) == 1);
    CHECK(llg_schedule_region_callback(
              LLG_REGION_POSTPONED_PLI, region_trace_callback,
              &region_trace_args[LLG_REGION_POSTPONED_PLI]) == 1);
    llg_rt_run();
    CHECK(region_observed_reactive_seen);
    CHECK(region_trace[LLG_REGION_ACTIVE] == 1);
    CHECK(!llg_rt_failed());

    llg_rt_init();
    region_sample_signal = SV4_C(0, 1);
    region_pre_postponed_reentry_seen = 0;
    llg_spawn(region_pre_postponed_waiter, "pre-postponed-waiter");
    CHECK(llg_schedule_region_callback(
              LLG_REGION_PRE_POSTPONED, region_pre_postponed_write_callback, NULL) == 1);
    CHECK(llg_schedule_region_callback(
              LLG_REGION_POSTPONED_PLI, region_trace_callback,
              &region_trace_args[LLG_REGION_POSTPONED_PLI]) == 1);
    llg_rt_run();
    CHECK(sv4_same(region_sample_signal, SV4_C(1, 1)));
    CHECK(region_pre_postponed_reentry_seen);
    CHECK(!llg_rt_failed());

    llg_rt_init();
    region_sample_signal = SV4_C(0, 1);
    llg_schedule_region_callback(
        LLG_REGION_OBSERVED, region_read_only_callback, NULL);
    llg_rt_run();
    CHECK(llg_rt_failed());
    CHECK(sv4_same(region_sample_signal, SV4_C(0, 1)));

    llg_rt_init();
    CHECK(llg_schedule_region_callback(
              LLG_REGION_OBSERVED, region_illegal_schedule_callback, NULL) == 1);
    llg_rt_run();
    CHECK(llg_rt_failed());
    return failures == 0 ? 0 : 1;
}

int main(int argc, char** argv) {
    if (argc == 2 && strcmp(argv[1], "--time-overflow-probe") == 0)
        return run_time_overflow_probe();
    if (argc == 2 && strcmp(argv[1], "--scaled-time-overflow-probe") == 0)
        return run_scaled_time_overflow_probe();
    if (argc == 2 && strcmp(argv[1], "--budget-finite-probe") == 0)
        return run_budget_finite_probe();
    if (argc == 2 && strcmp(argv[1], "--budget-infinite-probe") == 0)
        return run_budget_infinite_probe();
    if (argc == 2 && strcmp(argv[1], "--region-probe") == 0)
        return run_region_probe();
    test_sv4_ops();
    test_sv4_wide();
    check_vector_table();
    test_real_dependencies();
    test_llg_net();
    test_scheduler();
    test_event_triggered_lifecycle();
    test_fork_join();
    test_force_release();
    test_force_live_expression();
    test_force_nba_dropped();
    test_force_wakes_waiters();
    test_inertial_lifetime();
    test_time_scaled_rounding();
    test_activation_frames();
    test_activation_frame_cancellation();
    if (failures == 0) {
        printf("llg_rt selftest: all ok\n");
        return 0;
    }
    fprintf(stderr, "llg_rt selftest: %d failure(s)\n", failures);
    return 1;
}
