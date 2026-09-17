#include "llg_value.h"
#include "test_value_temporaries.h"

/* A 1024-bit vector, not a model-global storage capacity. */
enum { VECTOR_WIDTH = 1024, VECTOR_LIMBS = 16 };

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CHECK(condition)                                                        \
    do {                                                                        \
        if (!(condition)) {                                                     \
            fprintf(stderr, "runtime value check failed at line %d: %s\n",     \
                    __LINE__, #condition);                                      \
            return 1;                                                           \
        }                                                                       \
    } while (0)

static int state_at(sv4_t value, unsigned bit) {
    unsigned limb = bit / 64u;
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    if ((value.x[limb] & mask) != 0) return 2;
    if ((value.z[limb] & mask) != 0) return 3;
    return (value.bits[limb] & mask) != 0;
}

static int check_wide_four_state_ops(void) {
    uint64_t bits[VECTOR_LIMBS] = {0};
    uint64_t x[VECTOR_LIMBS] = {0};
    uint64_t z[VECTOR_LIMBS] = {0};
    for (unsigned i = 0; i < VECTOR_LIMBS; i++) {
        x[i] = UINT64_C(1) << (i % 61u);
        z[i] = UINT64_C(1) << ((i + 17u) % 61u);
        bits[i] = (UINT64_C(0xa55aa55a01234567) ^ ((uint64_t)i << 32))
                  & ~(x[i] | z[i]);
    }

    sv4_t mixed = test_value(sv4_from_limbs(bits, x, z, VECTOR_WIDTH, 0));
    sv4_t ones = test_value(sv4_fill(1, VECTOR_WIDTH, 0));
    sv4_t zeros = test_value(sv4_fill(0, VECTOR_WIDTH, 0));
    sv4_t and_result = test_value(sv4_and(ones, mixed));
    sv4_t or_result = test_value(sv4_or(zeros, mixed));
    sv4_t xor_result = test_value(sv4_xor(ones, mixed));
    for (unsigned i = 0; i < VECTOR_LIMBS; i++) {
        uint64_t unknown = x[i] | z[i];
        CHECK(and_result.bits[i] == bits[i]);
        CHECK(and_result.x[i] == unknown);
        CHECK(and_result.z[i] == 0);
        CHECK(or_result.bits[i] == bits[i]);
        CHECK(or_result.x[i] == unknown);
        CHECK(or_result.z[i] == 0);
        CHECK(xor_result.bits[i] == (~bits[i] & ~unknown));
        CHECK(xor_result.x[i] == unknown);
        CHECK(xor_result.z[i] == 0);
    }

    sv4_t one = test_value(sv4_from_u64(1, VECTOR_WIDTH, 0));
    sv4_t sum = test_value(sv4_add(ones, one));
    sv4_t difference = test_value(sv4_sub(zeros, one));
    CHECK(sum.width == VECTOR_WIDTH && !sv4_is_unknown(sum));
    CHECK(difference.width == VECTOR_WIDTH && !sv4_is_unknown(difference));
    for (unsigned i = 0; i < VECTOR_LIMBS; i++) {
        CHECK(sum.bits[i] == 0);
        CHECK(difference.bits[i] == UINT64_MAX);
    }

    sv4_t unknown_sum = test_value(sv4_add(mixed, one));
    for (unsigned i = 0; i < VECTOR_LIMBS; i++) {
        CHECK(unknown_sum.bits[i] == 0);
        CHECK(unknown_sum.x[i] == UINT64_MAX);
        CHECK(unknown_sum.z[i] == 0);
    }

    return 0;
}

static int check_signed_resize(void) {
    sv4_t negative = test_value(sv4_from_i64(-2, 8));
    sv4_t extended = test_value(sv4_resize(negative, 130, 1));
    CHECK(extended.width == 130 && extended.is_signed);
    CHECK(state_at(extended, 0) == 0);
    for (unsigned bit = 1; bit < 130; bit++) CHECK(state_at(extended, bit) == 1);
    CHECK((extended.bits[2] & ~UINT64_C(3)) == 0);

    uint64_t x_bits[VECTOR_LIMBS] = {0};
    uint64_t z_bits[VECTOR_LIMBS] = {0};
    x_bits[0] = UINT64_C(1) << 7;
    z_bits[0] = UINT64_C(1) << 7;
    sv4_t x_sign = test_value(sv4_from_limbs(NULL, x_bits, NULL, 8, 1));
    sv4_t z_sign = test_value(sv4_from_limbs(NULL, NULL, z_bits, 8, 1));
    x_sign = test_value(sv4_resize(x_sign, 130, 1));
    z_sign = test_value(sv4_resize(z_sign, 130, 1));
    for (unsigned bit = 7; bit < 130; bit++) {
        CHECK(state_at(x_sign, bit) == 2);
        CHECK(state_at(z_sign, bit) == 3);
    }

    sv4_t signed_source = test_value(sv4_from_i64(-128, 8));
    sv4_t unsigned_source = test_value(sv4_from_u64(0x80, 8, 0));
    sv4_t cast_unsigned = test_value(sv4_cast(signed_source, 16, 0));
    sv4_t cast_signed = test_value(sv4_cast(unsigned_source, 16, 1));
    CHECK(cast_unsigned.bits[0] == UINT64_C(0xff80));
    CHECK(cast_unsigned.is_signed == 0);
    CHECK(cast_signed.bits[0] == UINT64_C(0x0080));
    CHECK(cast_signed.is_signed == 1);

    sv4_t target_signed_resize = test_value(sv4_resize(unsigned_source, 16, 1));
    CHECK(target_signed_resize.bits[0] == UINT64_C(0xff80));
    return 0;
}

static int check_queries(void) {
    uint64_t bits[VECTOR_LIMBS] = {0};
    uint64_t x[VECTOR_LIMBS] = {0};
    uint64_t z[VECTOR_LIMBS] = {0};
    for (unsigned i = 0; i < VECTOR_LIMBS; i++) {
        bits[i] = UINT64_C(0x89);
        x[i] = UINT64_C(0x100);
        z[i] = UINT64_C(0x200);
    }
    sv4_t many = test_value(sv4_from_limbs(bits, x, z, VECTOR_WIDTH, 0));
    CHECK(sv4_to_i64(test_value(sv4_countones(many))) == 48);
    CHECK(sv4_to_u64(test_value(sv4_onehot(many, 0))) == 0);
    CHECK(sv4_to_u64(test_value(sv4_onehot(many, 1))) == 0);
    CHECK(sv4_is_unknown(many));

    sv4_t only_unknown = test_value(sv4_from_limbs(NULL, x, z, VECTOR_WIDTH, 0));
    CHECK(sv4_to_i64(test_value(sv4_countones(only_unknown))) == 0);
    CHECK(sv4_to_u64(test_value(sv4_onehot(only_unknown, 0))) == 0);
    CHECK(sv4_to_u64(test_value(sv4_onehot(only_unknown, 1))) == 1);
    only_unknown.bits[15] = UINT64_C(1) << 63;
    CHECK(sv4_to_u64(test_value(sv4_onehot(only_unknown, 0))) == 1);
    CHECK(sv4_to_u64(test_value(sv4_onehot(only_unknown, 1))) == 1);

    uint64_t edge_bits[VECTOR_LIMBS] = {0};
    edge_bits[1] = UINT64_C(1) | (UINT64_C(1) << 63);
    sv4_t width_65 = test_value(sv4_from_limbs(edge_bits, NULL, NULL, 65, 0));
    CHECK(sv4_to_i64(test_value(sv4_countones(width_65))) == 1);
    CHECK(!sv4_is_unknown(width_65));
    return 0;
}

static void set_state(sv4_t* value, unsigned bit, int state) {
    unsigned limb = bit / 64u;
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
}

static int check_normalized(sv4_t value) {
    unsigned limbs = value.width / 64u + (value.width % 64u != 0);
    for (unsigned i = 0; i < limbs; i++) {
        uint64_t mask = UINT64_MAX;
        if (i >= limbs) mask = 0;
        else if (i + 1 == limbs && value.width % 64u != 0)
            mask = (UINT64_C(1) << (value.width % 64u)) - 1;
        CHECK((value.x[i] & value.z[i]) == 0);
        CHECK((value.bits[i] & (value.x[i] | value.z[i])) == 0);
        CHECK(((value.bits[i] | value.x[i] | value.z[i]) & ~mask) == 0);
    }
    return 0;
}

static int check_net_resolution(void) {
    static const int modes[7] = {
        LLG_RESOLVE_WIRE, LLG_RESOLVE_WAND, LLG_RESOLVE_WOR,
        LLG_RESOLVE_TRI0, LLG_RESOLVE_TRI1,
        LLG_RESOLVE_SUPPLY0, LLG_RESOLVE_SUPPLY1
    };
    static const int expected[7][16] = {
        {0, 2, 2, 0, 2, 1, 2, 1, 2, 2, 2, 2, 0, 1, 2, 3},
        {0, 0, 0, 0, 0, 1, 2, 1, 0, 2, 2, 2, 0, 1, 2, 3},
        {0, 1, 2, 0, 1, 1, 1, 1, 2, 1, 2, 2, 0, 1, 2, 3},
        {0, 2, 2, 0, 2, 1, 2, 1, 2, 2, 2, 2, 0, 1, 2, 0},
        {0, 2, 2, 0, 2, 1, 2, 1, 2, 2, 2, 2, 0, 1, 2, 1},
        {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0},
        {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1},
    };

    /* Pin every scalar pair, including pull fallback and supply dominance. */
    for (int mode = 0; mode < 7; mode++) {
        for (int left = 0; left < 4; left++) {
            for (int right = 0; right < 4; right++) {
                sv4_t a = test_value(sv4_fill((uint8_t)left, 1, 0));
                sv4_t b = test_value(sv4_fill((uint8_t)right, 1, 0));
                const sv4_t* drivers[2] = {&a, &b};
                sv4_t result = test_value(sv4_resolve(drivers, 2, 1, 0, modes[mode]));
                CHECK(state_at(result, 0) == expected[mode][left * 4 + right]);
                CHECK(check_normalized(result) == 0);
            }
        }
    }

    /* Exercise partial, multi-limb, and maximum-width top masks. */
    static const uint16_t widths[3] = {65, 130, VECTOR_WIDTH};
    for (unsigned w = 0; w < 3; w++) {
        uint16_t width = widths[w];
        sv4_t a = test_value(sv4_fill(0, width, 0));
        sv4_t b = test_value(sv4_fill(0, width, 0));
        for (unsigned bit = 0; bit < width; bit++) {
            set_state(&a, bit, (int)(bit % 4u));
            set_state(&b, bit, (int)((bit / 4u) % 4u));
        }
        const sv4_t* drivers[2] = {&a, &b};
        for (int mode = 0; mode < 7; mode++) {
            sv4_t result = test_value(sv4_resolve(drivers, 2, width, 1, modes[mode]));
            CHECK(result.width == width && result.is_signed == 1);
            for (unsigned bit = 0; bit < width; bit++) {
                int pair = state_at(a, bit) * 4 + state_at(b, bit);
                CHECK(state_at(result, bit) == expected[mode][pair]);
            }
            CHECK(check_normalized(result) == 0);
        }
    }

    /* Driverless wire/wired modes are Z; pull/supply modes have defaults. */
    static const uint16_t empty_widths[4] = {1, 65, 130, VECTOR_WIDTH};
    static const int empty_expected[7] = {3, 3, 3, 0, 1, 0, 1};
    for (unsigned w = 0; w < 4; w++) {
        uint16_t width = empty_widths[w];
        for (int mode = 0; mode < 7; mode++) {
            sv4_t result = test_value(sv4_resolve(NULL, 0, width, 1, modes[mode]));
            CHECK(result.width == width && result.is_signed == 1);
            for (unsigned bit = 0; bit < width; bit++)
                CHECK(state_at(result, bit) == empty_expected[mode]);
            CHECK(check_normalized(result) == 0);
        }
    }

    sv4_t one = test_value(sv4_fill(1, VECTOR_WIDTH, 0));
    const sv4_t* sparse_drivers[2] = {NULL, &one};
    sv4_t sparse = test_value(sv4_resolve(sparse_drivers, 2, VECTOR_WIDTH, 0,
                               LLG_RESOLVE_WIRE));
    for (unsigned bit = 0; bit < VECTOR_WIDTH; bit++)
        CHECK(state_at(sparse, bit) == 1);
    CHECK(check_normalized(sparse) == 0);

    return 0;
}

static int check_strength_resolution(void) {
    struct strength_case {
        int mode;
        int left_state;
        uint8_t left0;
        uint8_t left1;
        int right_state;
        uint8_t right0;
        uint8_t right1;
        int expected;
    };
    static const struct strength_case cases[] = {
        /* Wire: known endpoints, equal-strength conflicts, and Z neutrality. */
        {LLG_RESOLVE_WIRE, 0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_PULL, LLG_STRENGTH_PULL, 0},
        {LLG_RESOLVE_WIRE, 0, LLG_STRENGTH_PULL, LLG_STRENGTH_PULL,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 1},
        {LLG_RESOLVE_WIRE, 0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        {LLG_RESOLVE_WIRE, 0, LLG_STRENGTH_HIGHZ, LLG_STRENGTH_HIGHZ,
         3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 3},
        {LLG_RESOLVE_WIRE, 2, LLG_STRENGTH_STRONG, LLG_STRENGTH_WEAK,
         0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 0},
        {LLG_RESOLVE_WIRE, 2, LLG_STRENGTH_WEAK, LLG_STRENGTH_STRONG,
         0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        {LLG_RESOLVE_WIRE, 2, LLG_STRENGTH_WEAK, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 1},
        {LLG_RESOLVE_WIRE, 2, LLG_STRENGTH_STRONG, LLG_STRENGTH_WEAK,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        {LLG_RESOLVE_WIRE, 2, LLG_STRENGTH_STRONG, LLG_STRENGTH_WEAK,
         3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        /* Wired rules retain the same strength ordering but break ties. */
        {LLG_RESOLVE_WAND, 0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 0},
        {LLG_RESOLVE_WOR, 0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 1},
        {LLG_RESOLVE_WAND, 2, LLG_STRENGTH_WEAK, LLG_STRENGTH_STRONG,
         0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 0},
        {LLG_RESOLVE_WAND, 2, LLG_STRENGTH_STRONG, LLG_STRENGTH_WEAK,
         0, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 0},
        {LLG_RESOLVE_WOR, 2, LLG_STRENGTH_STRONG, LLG_STRENGTH_WEAK,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 1},
        {LLG_RESOLVE_WOR, 2, LLG_STRENGTH_WEAK, LLG_STRENGTH_STRONG,
         1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 1},
        /* Implicit pull/supply defaults are strength-bearing sources. */
        {LLG_RESOLVE_TRI0, 3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 0},
        {LLG_RESOLVE_TRI0, 1, LLG_STRENGTH_PULL, LLG_STRENGTH_PULL,
         3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        {LLG_RESOLVE_TRI0, 1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         3, LLG_STRENGTH_PULL, LLG_STRENGTH_PULL, 1},
        {LLG_RESOLVE_SUPPLY0, 1, LLG_STRENGTH_SUPPLY, LLG_STRENGTH_SUPPLY,
         3, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG, 2},
        {LLG_RESOLVE_SUPPLY0, 1, LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG,
         3, LLG_STRENGTH_SUPPLY, LLG_STRENGTH_SUPPLY, 0},
    };

    for (unsigned i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
        const struct strength_case* test = &cases[i];
        sv4_t left = test_value(sv4_fill((uint8_t)test->left_state, 1, 0));
        sv4_t right = test_value(sv4_fill((uint8_t)test->right_state, 1, 0));
        const sv4_t* drivers[2] = {&left, &right};
        uint8_t strength0[2] = {test->left0, test->right0};
        uint8_t strength1[2] = {test->left1, test->right1};
        sv4_t result = test_value(sv4_resolve_strengths(
            drivers, strength0, strength1, 2, 1, 0, test->mode));
        CHECK(state_at(result, 0) == test->expected);
        CHECK(check_normalized(result) == 0);
    }

    /* A single driver is also checked at wide boundaries so strength-aware
     * resolution cannot regress into scalar-only limb handling. */
    sv4_t wide_one = test_value(sv4_fill(1, 130, 0));
    const sv4_t* wide_drivers[1] = {&wide_one};
    uint8_t strong[1] = {LLG_STRENGTH_STRONG};
    sv4_t result = test_value(sv4_resolve_strengths(
        wide_drivers, strong, strong, 1, 130, 0, LLG_RESOLVE_WIRE));
    for (unsigned bit = 0; bit < 130; bit++) CHECK(state_at(result, bit) == 1);
    CHECK(check_normalized(result) == 0);
    return 0;
}

static int check_numeric_conversions(void) {
    uint64_t wide_bits[VECTOR_LIMBS] = {0};
    wide_bits[2] = 1;
    sv4_t wide = test_value(sv4_from_limbs(wide_bits, NULL, NULL, 192, 0));
    CHECK(sv4_to_real(wide) == ldexp(1.0, 128));
    CHECK(sv4_to_real(test_value(sv4_fill(1, 130, 1))) == -1.0);

    uint64_t packed_bits[VECTOR_LIMBS] = {UINT64_C(0xf)};
    uint64_t packed_x[VECTOR_LIMBS] = {UINT64_C(0x2)};
    uint64_t packed_z[VECTOR_LIMBS] = {UINT64_C(0x8)};
    sv4_t four_state = test_value(sv4_from_limbs(packed_bits, packed_x, packed_z, 4, 0));
    CHECK(sv4_to_real(four_state) == 5.0);

    CHECK(sv4_to_i64(test_value(sv4_from_real(12.5, 16, 1))) == 13);
    CHECK(sv4_to_i64(test_value(sv4_from_real(-2.5, 16, 1))) == -3);
    CHECK(sv4_to_i64(test_value(sv4_rtoi(12.75))) == 12);
    CHECK(sv4_to_i64(test_value(sv4_rtoi(-12.75))) == -12);
    CHECK(sv4_is_unknown(test_value(sv4_from_real(INFINITY, 32, 1))));
    CHECK(sv4_is_unknown(test_value(sv4_from_real(NAN, 32, 1))));
    CHECK(sv4_is_unknown(test_value(sv4_rtoi(-INFINITY))));

    double real_value = -13.25;
    uint64_t real_bits = 0;
    memcpy(&real_bits, &real_value, sizeof(real_bits));
    sv4_t encoded_real = test_value(sv4_realtobits(real_value));
    CHECK(encoded_real.width == 64 && encoded_real.bits[0] == real_bits);
    CHECK(sv4_bitstoreal(encoded_real) == real_value);
    encoded_real.x[0] |= UINT64_C(1);
    encoded_real.z[0] |= UINT64_C(2);
    real_bits &= ~UINT64_C(3);
    double masked_real = 0.0;
    memcpy(&masked_real, &real_bits, sizeof(masked_real));
    CHECK(sv4_bitstoreal(encoded_real) == masked_real);

    double short_value = 1.5;
    float narrowed = (float)short_value;
    uint32_t short_bits = 0;
    memcpy(&short_bits, &narrowed, sizeof(short_bits));
    sv4_t encoded_short = test_value(sv4_shortrealtobits(short_value));
    CHECK(encoded_short.width == 32 && encoded_short.bits[0] == short_bits);
    CHECK(sv4_bitstoshortreal(encoded_short) == short_value);
    encoded_short.x[0] |= UINT64_C(1);
    encoded_short.z[0] |= UINT64_C(2);
    short_bits &= ~UINT32_C(3);
    memcpy(&narrowed, &short_bits, sizeof(narrowed));
    CHECK(sv4_bitstoshortreal(encoded_short) == (double)narrowed);

    CHECK(llg_real_to_bool(0.0) == 0);
    CHECK(llg_real_to_bool(-0.0) == 0);
    CHECK(llg_real_to_bool(-0.25) == 1);
    return 0;
}

static int check_partial_selects(void) {
    sv4_t value = test_value(sv4_from_u64(0xa5, 8, 0));
    sv4_t high = test_value(sv4_part_select(value, 9, 6));
    CHECK(high.width == 4);
    CHECK(high.bits[0] == 2 && high.x[0] == 12 && high.z[0] == 0);
    sv4_t low = test_value(sv4_part_select(value, 1, -2));
    CHECK(low.width == 4);
    CHECK(low.bits[0] == 4 && low.x[0] == 3 && low.z[0] == 0);
    sv4_t outside = test_value(sv4_part_select(value, -1, -4));
    CHECK(outside.width == 4 && outside.x[0] == 15);
    sv4_t reversed = test_value(sv4_part_select(value, 6, 9));
    CHECK(reversed.width == 4);
    CHECK(reversed.bits[0] == 4 && reversed.x[0] == 3);
    return 0;
}

static int check_delay_conversion(void) {
    CHECK(sv4_delay_ticks(test_value(sv4_from_u64(3, 8, 0)), 100) == 300);
    CHECK(sv4_delay_ticks(test_value(sv4_from_i64(-2, 8)), 1) == UINT64_MAX - 1);
    CHECK(sv4_delay_ticks(test_value(sv4_from_i64(-2, 129)), 1) == UINT64_MAX - 1);
    CHECK(sv4_delay_ticks(test_value(sv4_from_u64(UINT64_MAX, 64, 0)), 1) == UINT64_MAX);
    CHECK(sv4_delay_ticks(test_value(sv4_x(129, 0)), 100) == 0);
    CHECK(sv4_delay_ticks(test_value(sv4_fill(3, 65, 0)), 100) == 0);
    CHECK(sv4_real_delay_ticks(0.24, 1000, 100) == 200);
    CHECK(sv4_real_delay_ticks(0.25, 1000, 100) == 300);
    CHECK(sv4_real_delay_ticks(0.049, 1000, 100) == 0);
    CHECK(sv4_real_delay_ticks(0.05, 1000, 100) == 100);
    return 0;
}

static int check_enum_navigation(void) {
    sv4_t values[4] = {
        test_value(sv4_from_i64(-2, 8)),
        test_value(sv4_from_i64(3, 8)),
        test_value(sv4_from_i64(3, 8)),
        test_value(sv4_from_i64(9, 8)),
    };
    sv4_t invalid = test_value(sv4_x(8, 1));
    sv4_t current = test_value(sv4_from_i64(3, 8));
    sv4_t one = test_value(sv4_from_u64(1, 32, 0));
    sv4_t five = test_value(sv4_from_u64(5, 32, 0));
    sv4_t unknown_step = test_value(sv4_x(32, 0));

    /* Duplicate values use the last declaration, matching the lowering
     * policy and making the alias result deterministic. */
    CHECK(sv4_to_i64(test_value(sv4_enum_navigate(
              current, one, values, 4, invalid, 1))) == 9);
    CHECK(sv4_to_i64(test_value(sv4_enum_navigate(
              current, five, values, 4, invalid, -1))) == 3);
    CHECK(sv4_to_i64(test_value(sv4_enum_navigate(
              current, unknown_step, values, 4, invalid, 1))) == 3);
    CHECK(sv4_same(test_value(sv4_enum_navigate(
              test_value(sv4_x(8, 1)), one, values, 4, invalid, 1)), invalid));
    return 0;
}

static int check_negative_powers(void) {
    sv4_t minus_one = test_value(sv4_resize(test_value(sv4_from_u64(UINT64_MAX, 64, 1)), 65, 1));
    sv4_t odd = test_value(sv4_from_u64(UINT64_MAX, 64, 1));
    sv4_t even = test_value(sv4_from_u64(UINT64_MAX - 1, 64, 1));
    CHECK(sv4_to_bool(test_value(sv4_case_eq(test_value(sv4_pow(minus_one, odd)), minus_one))));
    CHECK(sv4_to_bool(test_value(sv4_case_eq(test_value(sv4_pow(minus_one, even)),
                                test_value(sv4_from_u64(1, 65, 1))))));
    sv4_t minus_two = test_value(sv4_clone(&minus_one));
    minus_two.bits[0] &= ~UINT64_C(1);
    CHECK(sv4_to_bool(test_value(sv4_case_eq(test_value(sv4_pow(minus_two, odd)),
                                test_value(sv4_from_u64(0, 65, 1))))));
    return 0;
}

static int check_logical_relations(void) {
    CHECK(sv4_same(test_value(sv4_logimpl(test_value(SV4_C(0, 1)), test_value(SV4_X(1)))), test_value(SV4_C(1, 1))));
    CHECK(sv4_same(test_value(sv4_logimpl(test_value(SV4_C(1, 1)), test_value(SV4_C(0, 1)))), test_value(SV4_C(0, 1))));
    CHECK(sv4_same(test_value(sv4_logimpl(test_value(SV4_X(1)), test_value(SV4_C(1, 1)))), test_value(SV4_C(1, 1))));
    CHECK(sv4_is_unknown(test_value(sv4_logimpl(test_value(SV4_Z(1)), test_value(SV4_C(0, 1))))));
    CHECK(sv4_same(test_value(sv4_logequiv(test_value(SV4_C(0, 1)), test_value(SV4_C(0, 1)))), test_value(SV4_C(1, 1))));
    CHECK(sv4_same(test_value(sv4_logequiv(test_value(SV4_C(0, 1)), test_value(SV4_C(1, 1)))), test_value(SV4_C(0, 1))));
    CHECK(sv4_is_unknown(test_value(sv4_logequiv(test_value(SV4_X(1)), test_value(SV4_C(0, 1))))));
    return 0;
}

int main(void) {
    if (atexit(test_values_clear) != 0) return 2;
    CHECK(test_values_run(check_wide_four_state_ops) == 0);
    CHECK(test_values_run(check_signed_resize) == 0);
    CHECK(test_values_run(check_queries) == 0);
    CHECK(test_values_run(check_net_resolution) == 0);
    CHECK(test_values_run(check_strength_resolution) == 0);
    CHECK(test_values_run(check_numeric_conversions) == 0);
    CHECK(test_values_run(check_negative_powers) == 0);
    CHECK(test_values_run(check_logical_relations) == 0);
    CHECK(test_values_run(check_partial_selects) == 0);
    CHECK(test_values_run(check_delay_conversion) == 0);
    CHECK(test_values_run(check_enum_navigation) == 0);
    puts("runtime value isolation ok");
    return 0;
}
