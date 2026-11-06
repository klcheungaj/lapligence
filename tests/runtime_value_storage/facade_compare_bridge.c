/* One client of the production value facade, built unchanged against the legacy
 * backend and the compact backend (portable and GMP kernels). It feeds fixed,
 * deterministic inputs to facade operations and prints every result, so
 * facade_compare.py can require identical output from the backends. Only
 * llg_value.h names are used. Each output line starts with the operation it
 * exercises; the runner derives coverage from those names. */
#include "llg_value.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define COUNT_OF(array) (sizeof(array) / sizeof((array)[0]))
enum { PATTERNS = 8, MAX_WORDS = 8 };

static const uint32_t pair_widths[][2] = {{1, 1},   {63, 63}, {64, 64},  {65, 65}, {128, 128},
                                          {1, 64},  {63, 65}, {64, 128}, {65, 1},  {128, 63}};
static const uint32_t unary_widths[] = {1, 63, 64, 65, 128};

static uint64_t mix(uint64_t seed) {
    seed ^= seed >> 33;
    seed *= UINT64_C(0xff51afd7ed558ccd);
    seed ^= seed >> 33;
    seed *= UINT64_C(0xc4ceb9fe1a85ec53);
    return seed ^ (seed >> 33);
}

/* Patterns: 0 zero, 1 all ones, 2 all X, 3 all Z, 4 sign bit only, 5 random
 * known, 6 random with X and Z, 7 alternating bits with X at the least and Z at
 * the most significant bit. The inputs depend only on the arguments. */
static sv4_t make(uint32_t width, int sign, unsigned pattern, unsigned salt) {
    uint64_t bits[MAX_WORDS] = {0}, x[MAX_WORDS] = {0}, z[MAX_WORDS] = {0};
    size_t words = ((size_t)width + 63u) / 64u;
    if (words > MAX_WORDS) {
        fputs("facade_compare_bridge: width exceeds the input generator\n", stderr);
        abort();
    }
    uint64_t seed = mix(((uint64_t)width << 32) ^ ((uint64_t)pattern << 8) ^ salt ^
                        ((uint64_t)(sign != 0) << 20));
    for (size_t i = 0; i < words; ++i) {
        uint64_t random_bits = mix(seed + i * 3u), random_x = mix(seed + i * 3u + 1u),
                 random_z = mix(seed + i * 3u + 2u);
        uint64_t limit = (i + 1u == words && width % 64u) ? (UINT64_C(1) << (width % 64u)) - 1u
                                                          : UINT64_MAX;
        switch (pattern) {
        case 1:
            bits[i] = UINT64_MAX;
            break;
        case 2:
            x[i] = UINT64_MAX;
            break;
        case 3:
            z[i] = UINT64_MAX;
            break;
        case 4:
            bits[i] = (i + 1u == words) ? UINT64_C(1) << ((width - 1u) % 64u) : 0;
            break;
        case 5:
            bits[i] = random_bits;
            break;
        case 6:
            bits[i] = random_bits;
            x[i] = random_x & random_z;
            z[i] = random_z & ~random_x;
            break;
        case 7:
            bits[i] = UINT64_C(0xaaaaaaaaaaaaaaaa);
            break;
        default:
            break;
        }
        bits[i] &= limit;
        x[i] &= limit;
        z[i] &= limit;
    }
    if (pattern == 7) {
        x[0] |= 1u;
        z[(width - 1u) / 64u] |= UINT64_C(1) << ((width - 1u) % 64u);
        if (width == 1u)
            z[0] = 0;
    }
    for (size_t i = 0; i < words; ++i)
        bits[i] &= ~(x[i] | z[i]);
    return sv4_from_limbs(bits, x, z, width, (int8_t)sign);
}

static void put(sv4_t value) {
    printf(" %u/%d", llg_sv4_width(value), (int)llg_sv4_signed(value));
    for (size_t i = llg_sv4_words(value); i-- > 0;)
        printf(":%" PRIx64 ".%" PRIx64 ".%" PRIx64, llg_sv4_word(value, i, LLG_SV4_BITS),
               llg_sv4_word(value, i, LLG_SV4_X), llg_sv4_word(value, i, LLG_SV4_Z));
}
static void put_owner(sv4_t* value) {
    put(*value);
    sv4_destroy(value);
}
static void put_u64(uint64_t value) { printf(" %" PRIx64, value); }

typedef sv4_t (*unary_fn)(sv4_t);
typedef sv4_t (*binary_fn)(sv4_t, sv4_t);
typedef void (*unary_to_fn)(sv4_t*, const sv4_t*);
typedef void (*binary_to_fn)(sv4_t*, const sv4_t*, const sv4_t*);

typedef struct {
    const char* name;
    unary_fn value;
    unary_to_fn into;
} unary_op_t;
typedef struct {
    const char* name;
    binary_fn value;
    binary_to_fn into;
} binary_op_t;

#define UNARY(op) {#op, sv4_##op, sv4_##op##_to}
#define BINARY(op) {#op, sv4_##op, sv4_##op##_to}

static const unary_op_t unary_ops[] = {
    UNARY(neg),         UNARY(bitneg),     UNARY(lognot),      UNARY(reduce_and),
    UNARY(reduce_nand), UNARY(reduce_or),  UNARY(reduce_nor),  UNARY(reduce_xor),
    UNARY(reduce_xnor), UNARY(clog2),      UNARY(countones),   UNARY(repeat_count),
    UNARY(to_two_state)};
static const binary_op_t binary_ops[] = {
    BINARY(add),     BINARY(sub),     BINARY(mul),      BINARY(div),      BINARY(mod),
    BINARY(pow),     BINARY(and),     BINARY(or),       BINARY(xor),      BINARY(xnor),
    BINARY(logand),  BINARY(logor),   BINARY(logimpl),  BINARY(logequiv), BINARY(shl),
    BINARY(shr),     BINARY(ashl),    BINARY(ashr),     BINARY(eq),       BINARY(neq),
    BINARY(case_eq), BINARY(case_neq), BINARY(wild_eq), BINARY(wild_neq), BINARY(casez_eq),
    BINARY(casex_eq), BINARY(lt),     BINARY(le),       BINARY(gt),       BINARY(ge),
    BINARY(concat)};

/* Each operation prints a returning-form line and a destination-form line. The
 * destination forms run into an empty destination, a live destination of
 * another width and a destination that is the operand (and, for binary
 * operations, the left operand, the right operand and both operands). */
static void unary_grid(void) {
    for (size_t op = 0; op < COUNT_OF(unary_ops); ++op)
        for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
            for (int sign = 0; sign < 2; ++sign)
                for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
                    sv4_t a = make(unary_widths[w], sign, pattern, 1);
                    sv4_t result = unary_ops[op].value(a);
                    sv4_t empty = SV4_EMPTY, live = sv4_zero(7, 1), alias = sv4_clone(&a);
                    unary_ops[op].into(&empty, &a);
                    unary_ops[op].into(&live, &a);
                    unary_ops[op].into(&alias, &alias);
                    printf("sv4_%s|%u %d %u|", unary_ops[op].name, unary_widths[w], sign, pattern);
                    put_owner(&result);
                    printf("\nsv4_%s_to|%u %d %u|", unary_ops[op].name, unary_widths[w], sign,
                           pattern);
                    put_owner(&empty);
                    put_owner(&live);
                    put_owner(&alias);
                    put(a);
                    putchar('\n');
                    sv4_destroy(&a);
                }
}

static void binary_grid(void) {
    for (size_t op = 0; op < COUNT_OF(binary_ops); ++op)
        for (size_t pair = 0; pair < COUNT_OF(pair_widths); ++pair)
            for (int sign_a = 0; sign_a < 2; ++sign_a)
                for (int sign_b = 0; sign_b < 2; ++sign_b)
                    for (unsigned pattern_a = 0; pattern_a < PATTERNS; ++pattern_a)
                        for (unsigned step = 0; step < 3; ++step) {
                            unsigned pattern_b = (pattern_a + (step == 0 ? 0u : step == 1 ? 3u : 5u)) % PATTERNS;
                            sv4_t a = make(pair_widths[pair][0], sign_a, pattern_a, 1);
                            sv4_t b = make(pair_widths[pair][1], sign_b, pattern_b, 2);
                            sv4_t result = binary_ops[op].value(a, b);
                            sv4_t empty = SV4_EMPTY, live = sv4_zero(7, 1);
                            sv4_t alias_a = sv4_clone(&a), alias_b = sv4_clone(&b),
                                  alias_both = sv4_clone(&a);
                            binary_ops[op].into(&empty, &a, &b);
                            binary_ops[op].into(&live, &a, &b);
                            binary_ops[op].into(&alias_a, &alias_a, &b);
                            binary_ops[op].into(&alias_b, &a, &alias_b);
                            binary_ops[op].into(&alias_both, &alias_both, &alias_both);
                            printf("sv4_%s|%u/%u %d %d %u %u|", binary_ops[op].name,
                                   pair_widths[pair][0], pair_widths[pair][1], sign_a, sign_b,
                                   pattern_a, pattern_b);
                            put_owner(&result);
                            printf("\nsv4_%s_to|%u/%u %d %d %u %u|", binary_ops[op].name,
                                   pair_widths[pair][0], pair_widths[pair][1], sign_a, sign_b,
                                   pattern_a, pattern_b);
                            put_owner(&empty);
                            put_owner(&live);
                            put_owner(&alias_a);
                            put_owner(&alias_b);
                            put_owner(&alias_both);
                            putchar('\n');
                            sv4_destroy(&a);
                            sv4_destroy(&b);
                        }
}

/* Width, sign and requested-sign conversions, including the zero width. */
static void conversions(void) {
    static const uint32_t targets[] = {0, 1, 63, 64, 65, 128, 130};
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (int sign = 0; sign < 2; ++sign)
            for (unsigned pattern = 0; pattern < PATTERNS; ++pattern)
                for (size_t t = 0; t < COUNT_OF(targets); ++t)
                    for (int target_sign = 0; target_sign < 2; ++target_sign) {
                        sv4_t a = make(unary_widths[w], sign, pattern, 1);
                        sv4_t cast = sv4_cast(a, targets[t], (int8_t)target_sign);
                        sv4_t resize = sv4_resize(a, targets[t], (int8_t)target_sign);
                        sv4_t cast_to = sv4_zero(3, 0), resize_to = SV4_EMPTY;
                        sv4_cast_to(&cast_to, &a, targets[t], (int8_t)target_sign);
                        sv4_resize_to(&resize_to, &a, targets[t], (int8_t)target_sign);
                        printf("sv4_cast|%u %d %u -> %u %d|", unary_widths[w], sign, pattern,
                               targets[t], target_sign);
                        put_owner(&cast);
                        printf("\nsv4_cast_to|%u %d %u -> %u %d|", unary_widths[w], sign, pattern,
                               targets[t], target_sign);
                        put_owner(&cast_to);
                        printf("\nsv4_resize|%u %d %u -> %u %d|", unary_widths[w], sign, pattern,
                               targets[t], target_sign);
                        put_owner(&resize);
                        printf("\nsv4_resize_to|%u %d %u -> %u %d|", unary_widths[w], sign, pattern,
                               targets[t], target_sign);
                        put_owner(&resize_to);
                        putchar('\n');
                        sv4_destroy(&a);
                    }
}

static void onehot_and_repeat(void) {
    static const uint64_t counts[] = {0, 1, 3, 4};
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (int sign = 0; sign < 2; ++sign)
            for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
                sv4_t a = make(unary_widths[w], sign, pattern, 1);
                for (int allow_zero = 0; allow_zero < 2; ++allow_zero) {
                    sv4_t result = sv4_onehot(a, allow_zero);
                    sv4_t into = SV4_EMPTY;
                    sv4_onehot_to(&into, &a, allow_zero);
                    printf("sv4_onehot|%u %d %u %d|", unary_widths[w], sign, pattern, allow_zero);
                    put_owner(&result);
                    printf("\nsv4_onehot_to|%u %d %u %d|", unary_widths[w], sign, pattern,
                           allow_zero);
                    put_owner(&into);
                    putchar('\n');
                }
                for (size_t c = 0; c < COUNT_OF(counts); ++c) {
                    sv4_t result = sv4_repeat(a, counts[c]);
                    sv4_t into = sv4_clone(&a);
                    sv4_repeat_to(&into, &into, counts[c]);
                    printf("sv4_repeat|%u %d %u %" PRIu64 "|", unary_widths[w], sign, pattern,
                           counts[c]);
                    put_owner(&result);
                    printf("\nsv4_repeat_to|%u %d %u %" PRIu64 "|", unary_widths[w], sign, pattern,
                           counts[c]);
                    put_owner(&into);
                    putchar('\n');
                }
                static const uint32_t slices[] = {1, 3, 7, 64};
                for (size_t s = 0; s < COUNT_OF(slices); ++s)
                    for (int right_to_left = 0; right_to_left < 2; ++right_to_left) {
                        sv4_t result = sv4_stream(a, slices[s], right_to_left);
                        sv4_t into = SV4_EMPTY;
                        sv4_stream_to(&into, &a, slices[s], right_to_left);
                        printf("sv4_stream|%u %d %u %u %d|", unary_widths[w], sign, pattern,
                               slices[s], right_to_left);
                        put_owner(&result);
                        printf("\nsv4_stream_to|%u %d %u %u %d|", unary_widths[w], sign, pattern,
                               slices[s], right_to_left);
                        put_owner(&into);
                        putchar('\n');
                    }
                sv4_destroy(&a);
            }
}

static void selections(void) {
    static const int64_t ranges[][2] = {{0, 0}, {62, 0},  {0, 62},  {63, 1},  {64, 64}, {127, 64},
                                        {64, 127}, {130, 120}, {-3, 4}, {5, -3}, {200, 190}};
    static const uint64_t bases[] = {0, 1, 63, 64, 65, 127, 1000};
    static const uint32_t selected_widths[] = {1, 3, 64, 70};
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
            sv4_t a = make(unary_widths[w], pattern & 1, pattern, 1);
            for (size_t r = 0; r < COUNT_OF(ranges); ++r) {
                sv4_t result = sv4_part_select(a, ranges[r][0], ranges[r][1]);
                sv4_t into = SV4_EMPTY;
                sv4_part_select_to(&into, &a, ranges[r][0], ranges[r][1]);
                printf("sv4_part_select|%u %u [%" PRId64 ":%" PRId64 "]|", unary_widths[w], pattern,
                       ranges[r][0], ranges[r][1]);
                put_owner(&result);
                printf("\nsv4_part_select_to|%u %u [%" PRId64 ":%" PRId64 "]|", unary_widths[w],
                       pattern, ranges[r][0], ranges[r][1]);
                put_owner(&into);
                putchar('\n');
                sv4_select_plan_t plan = sv4_select_plan_part(unary_widths[w], ranges[r][0],
                                                              ranges[r][1]);
                for (int reverse = 0; reverse < 2; ++reverse) {
                    sv4_t read = sv4_select_plan_read(a, &plan);
                    sv4_t sliced = sv4_resize(a, plan.width, 0);
                    sv4_t slice = sv4_select_plan_slice(sliced, &plan, reverse);
                    sv4_t read_to = SV4_EMPTY, slice_to = SV4_EMPTY;
                    sv4_select_plan_read_to(&read_to, &a, &plan);
                    sv4_select_plan_slice_to(&slice_to, &sliced, &plan, reverse);
                    sv4_destroy(&sliced);
                    printf("sv4_select_plan_read|%u %u [%" PRId64 ":%" PRId64 "]|", unary_widths[w],
                           pattern, ranges[r][0], ranges[r][1]);
                    put_owner(&read);
                    printf("\nsv4_select_plan_read_to|%u %u [%" PRId64 ":%" PRId64 "]|",
                           unary_widths[w], pattern, ranges[r][0], ranges[r][1]);
                    put_owner(&read_to);
                    printf("\nsv4_select_plan_slice|%u %u [%" PRId64 ":%" PRId64 "] %d|",
                           unary_widths[w], pattern, ranges[r][0], ranges[r][1], reverse);
                    put_owner(&slice);
                    printf("\nsv4_select_plan_slice_to|%u %u [%" PRId64 ":%" PRId64 "] %d|",
                           unary_widths[w], pattern, ranges[r][0], ranges[r][1], reverse);
                    put_owner(&slice_to);
                    putchar('\n');
                }
            }
            for (size_t b = 0; b < COUNT_OF(bases); ++b) {
                sv4_t result = sv4_bit_select(a, bases[b]);
                sv4_t into = SV4_EMPTY;
                sv4_bit_select_to(&into, &a, bases[b]);
                printf("sv4_bit_select|%u %u %" PRIu64 "|", unary_widths[w], pattern, bases[b]);
                put_owner(&result);
                printf("\nsv4_bit_select_to|%u %u %" PRIu64 "|", unary_widths[w], pattern, bases[b]);
                put_owner(&into);
                putchar('\n');
                for (size_t sw = 0; sw < COUNT_OF(selected_widths); ++sw)
                    for (int neg = 0; neg < 2; ++neg) {
                        sv4_t idx = sv4_idx_part_select(a, bases[b], selected_widths[sw], neg);
                        sv4_t idx_to = SV4_EMPTY;
                        sv4_idx_part_select_to(&idx_to, &a, bases[b], selected_widths[sw], neg);
                        sv4_t base = sv4_from_i64(neg ? -(int64_t)bases[b] : (int64_t)bases[b], 65);
                        sv4_t by_value = sv4_idx_part_select_value(a, base, selected_widths[sw], neg);
                        sv4_t by_value_to = SV4_EMPTY;
                        sv4_idx_part_select_value_to(&by_value_to, &a, &base, selected_widths[sw],
                                                     neg);
                        printf("sv4_idx_part_select|%u %u %" PRIu64 " %u %d|", unary_widths[w],
                               pattern, bases[b], selected_widths[sw], neg);
                        put_owner(&idx);
                        printf("\nsv4_idx_part_select_to|%u %u %" PRIu64 " %u %d|", unary_widths[w],
                               pattern, bases[b], selected_widths[sw], neg);
                        put_owner(&idx_to);
                        printf("\nsv4_idx_part_select_value|%u %u %" PRIu64 " %u %d|",
                               unary_widths[w], pattern, bases[b], selected_widths[sw], neg);
                        put_owner(&by_value);
                        printf("\nsv4_idx_part_select_value_to|%u %u %" PRIu64 " %u %d|",
                               unary_widths[w], pattern, bases[b], selected_widths[sw], neg);
                        put_owner(&by_value_to);
                        putchar('\n');
                        sv4_destroy(&base);
                    }
            }
            sv4_destroy(&a);
        }
}

static void constructors(void) {
    static const uint32_t widths[] = {0, 1, 7, 63, 64, 65, 128};
    for (size_t w = 0; w < COUNT_OF(widths); ++w)
        for (int sign = 0; sign < 2; ++sign) {
            uint32_t width = widths[w];
            sv4_t zero = sv4_zero(width, (int8_t)sign), unknown = sv4_x(width, (int8_t)sign);
            sv4_t zero_to = sv4_x(5, 0), unknown_to = SV4_EMPTY;
            sv4_zero_to(&zero_to, width, (int8_t)sign);
            sv4_x_to(&unknown_to, width, (int8_t)sign);
            printf("sv4_zero|%u %d|", width, sign);
            put_owner(&zero);
            printf("\nsv4_zero_to|%u %d|", width, sign);
            put_owner(&zero_to);
            printf("\nsv4_x|%u %d|", width, sign);
            put_owner(&unknown);
            printf("\nsv4_x_to|%u %d|", width, sign);
            put_owner(&unknown_to);
            for (uint8_t bit = 0; bit < 4; ++bit) {
                sv4_t filled = sv4_fill(bit, width, (int8_t)sign), filled_to = SV4_EMPTY;
                sv4_fill_to(&filled_to, bit, width, (int8_t)sign);
                printf("\nsv4_fill|%u %d %u|", width, sign, bit);
                put_owner(&filled);
                printf("\nsv4_fill_to|%u %d %u|", width, sign, bit);
                put_owner(&filled_to);
            }
            static const uint64_t numbers[] = {0, 1, UINT64_C(0x8000000000000000), UINT64_MAX,
                                               UINT64_C(0x123456789abcdef0)};
            for (size_t n = 0; n < COUNT_OF(numbers); ++n) {
                sv4_t unsigned_value = sv4_from_u64(numbers[n], width, (int8_t)sign),
                      unsigned_to = SV4_EMPTY;
                sv4_t signed_value = sv4_from_i64((int64_t)numbers[n], width), signed_to = SV4_EMPTY;
                sv4_from_u64_to(&unsigned_to, numbers[n], width, (int8_t)sign);
                sv4_from_i64_to(&signed_to, (int64_t)numbers[n], width);
                printf("\nsv4_from_u64|%u %d %" PRIx64 "|", width, sign, numbers[n]);
                put_owner(&unsigned_value);
                printf("\nsv4_from_u64_to|%u %d %" PRIx64 "|", width, sign, numbers[n]);
                put_owner(&unsigned_to);
                printf("\nsv4_from_i64|%u %" PRIx64 "|", width, numbers[n]);
                put_owner(&signed_value);
                printf("\nsv4_from_i64_to|%u %" PRIx64 "|", width, numbers[n]);
                put_owner(&signed_to);
            }
            static const uint64_t masks[][3] = {{0, 0, 0},
                                                {UINT64_MAX, 0, 0},
                                                {0, UINT64_MAX, 0},
                                                {0, 0, UINT64_MAX},
                                                {UINT64_C(0x5555555555555555), UINT64_C(0xf0f0f0f0f0f0f0f0),
                                                 UINT64_C(0x0f000f000f000f00)}};
            for (size_t m = 0; m < COUNT_OF(masks); ++m) {
                uint64_t cap = width >= 64 ? UINT64_MAX : (UINT64_C(1) << width) - 1u;
                uint64_t bits = masks[m][0] & cap, x = masks[m][1] & cap,
                         z = masks[m][2] & cap & ~x;
                bits &= ~(x | z);
                sv4_t from_masks = SV4_EMPTY, from_masks_to = SV4_EMPTY, from_limbs_to = SV4_EMPTY;
                if (width <= 64) {
                    sv4_replace(&from_masks, sv4_from_masks(bits, x, z, width, (int8_t)sign));
                    sv4_from_masks_to(&from_masks_to, bits, x, z, width, (int8_t)sign);
                }
                uint64_t limb_bits[3] = {bits, bits ^ cap, 0}, limb_x[3] = {x, 0, 0},
                         limb_z[3] = {z, 0, 0};
                if (width > 64) {
                    uint64_t high = width >= 128 ? UINT64_MAX : (UINT64_C(1) << (width - 64)) - 1u;
                    limb_bits[1] = limb_bits[1] & high;
                    limb_x[1] = x & high;
                    limb_z[1] = z & high & ~limb_x[1];
                    limb_bits[1] &= ~(limb_x[1] | limb_z[1]);
                } else {
                    limb_bits[1] = limb_x[1] = limb_z[1] = 0;
                }
                sv4_t from_limbs = sv4_from_limbs(limb_bits, limb_x, limb_z, width, (int8_t)sign);
                sv4_from_limbs_to(&from_limbs_to, limb_bits, limb_x, limb_z, width, (int8_t)sign);
                printf("\nsv4_from_masks|%u %d %zu|", width, sign, m);
                put_owner(&from_masks);
                printf("\nsv4_from_masks_to|%u %d %zu|", width, sign, m);
                put_owner(&from_masks_to);
                printf("\nsv4_from_limbs|%u %d %zu|", width, sign, m);
                put_owner(&from_limbs);
                printf("\nsv4_from_limbs_to|%u %d %zu|", width, sign, m);
                put_owner(&from_limbs_to);
            }
            putchar('\n');
        }
    static const double reals[] = {0.0, -0.0, 0.5, 1.5, -2.5, 255.0, 4294967296.0, -9223372036854775808.0,
                                   1.0e30, 1.0 / 3.0};
    for (size_t r = 0; r < COUNT_OF(reals); ++r)
        for (int sign = 0; sign < 2; ++sign) {
            sv4_t from_real = SV4_EMPTY, rtoi = SV4_EMPTY, bits = SV4_EMPTY, shortbits = SV4_EMPTY;
            sv4_from_real_to(&from_real, reals[r], 65, (int8_t)sign);
            sv4_rtoi_to(&rtoi, reals[r]);
            sv4_realtobits_to(&bits, reals[r]);
            sv4_shortrealtobits_to(&shortbits, reals[r]);
            sv4_t from_real_ret = sv4_from_real(reals[r], 65, (int8_t)sign);
            printf("sv4_from_real|%a %d|", reals[r], sign);
            put_owner(&from_real_ret);
            printf("\nsv4_from_real_to|%a %d|", reals[r], sign);
            put_owner(&from_real);
            sv4_t rtoi_ret = sv4_rtoi(reals[r]), bits_ret = sv4_realtobits(reals[r]),
                  shortbits_ret = sv4_shortrealtobits(reals[r]);
            printf("\nsv4_rtoi|%a|", reals[r]);
            put_owner(&rtoi_ret);
            printf("\nsv4_rtoi_to|%a|", reals[r]);
            put_owner(&rtoi);
            printf("\nsv4_realtobits|%a|", reals[r]);
            put_owner(&bits_ret);
            printf("\nsv4_realtobits_to|%a|", reals[r]);
            put_owner(&bits);
            printf("\nsv4_shortrealtobits|%a|", reals[r]);
            put_owner(&shortbits_ret);
            printf("\nsv4_shortrealtobits_to|%a|", reals[r]);
            put_owner(&shortbits);
            for (uint64_t unit = 1000; unit <= 1000000; unit *= 1000)
                for (uint64_t precision = 1; precision <= 1000; precision *= 10)
                    printf("\nsv4_real_delay_ticks|%a %" PRIu64 " %" PRIu64 "| %" PRIx64, reals[r], unit,
                           precision, sv4_real_delay_ticks(reals[r], unit, precision));
            putchar('\n');
        }
    sv4_t c = SV4_C(5, 3), s = SV4_S(5, 3), x = SV4_X(65), z = SV4_Z(65), init = SV4_INIT(9, 2, 4, 8, 1);
    printf("SV4_C|");
    put_owner(&c);
    printf("\nSV4_S|");
    put_owner(&s);
    printf("\nSV4_X|");
    put_owner(&x);
    printf("\nSV4_Z|");
    put_owner(&z);
    printf("\nSV4_INIT|");
    put_owner(&init);
    sv4_t empty = SV4_EMPTY;
    printf("\nSV4_EMPTY|");
    put(empty);
    putchar('\n');
}

static void mux_and_ranges(void) {
    for (size_t pair = 0; pair < COUNT_OF(pair_widths); ++pair)
        for (unsigned pattern_a = 0; pattern_a < PATTERNS; ++pattern_a)
            for (unsigned selector = 0; selector < 4; ++selector) {
                sv4_t a = make(pair_widths[pair][0], pattern_a & 1, pattern_a, 1);
                sv4_t b = make(pair_widths[pair][1], (pattern_a >> 1) & 1, (pattern_a + 3) % PATTERNS, 2);
                sv4_t sel = sv4_fill((uint8_t)selector, 1, 0);
                sv4_t result = sv4_mux(sel, a, b), into = SV4_EMPTY;
                sv4_mux_to(&into, &sel, &a, &b);
                printf("sv4_mux|%u/%u %u %u|", pair_widths[pair][0], pair_widths[pair][1], pattern_a,
                       selector);
                put_owner(&result);
                printf("\nsv4_mux_to|%u/%u %u %u|", pair_widths[pair][0], pair_widths[pair][1],
                       pattern_a, selector);
                put_owner(&into);
                sv4_t high = make(pair_widths[pair][1], 1, (pattern_a + selector) % PATTERNS, 3);
                sv4_t inside = sv4_inside_range(a, b, high), inside_to = SV4_EMPTY;
                sv4_inside_range_to(&inside_to, &a, &b, &high);
                printf("\nsv4_inside_range|%u/%u %u %u|", pair_widths[pair][0], pair_widths[pair][1],
                       pattern_a, selector);
                put_owner(&inside);
                printf("\nsv4_inside_range_to|%u/%u %u %u|", pair_widths[pair][0],
                       pair_widths[pair][1], pattern_a, selector);
                put_owner(&inside_to);
                putchar('\n');
                sv4_destroy(&a);
                sv4_destroy(&b);
                sv4_destroy(&sel);
                sv4_destroy(&high);
            }
    static const uint32_t element_widths[] = {1, 8, 32};
    for (size_t e = 0; e < COUNT_OF(element_widths); ++e)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
            uint32_t total = element_widths[e] * (element_widths[e] == 32 ? 4u : 8u);
            sv4_t a = make(total, 0, pattern, 1), b = make(total, 0, (pattern + 1) % PATTERNS, 2);
            sv4_t same = sv4_clone(&a), fallback = make(element_widths[e], 0, (pattern + 2) % PATTERNS, 3);
            sv4_t result = sv4_array_conditional_merge(a, b, fallback), into = SV4_EMPTY;
            sv4_t result_same = sv4_array_conditional_merge(a, same, fallback), same_to = SV4_EMPTY;
            sv4_array_conditional_merge_to(&into, &a, &b, &fallback);
            sv4_array_conditional_merge_to(&same_to, &a, &same, &fallback);
            printf("sv4_array_conditional_merge|%u %u|", element_widths[e], pattern);
            put_owner(&result);
            put_owner(&result_same);
            printf("\nsv4_array_conditional_merge_to|%u %u|", element_widths[e], pattern);
            put_owner(&into);
            put_owner(&same_to);
            putchar('\n');
            sv4_destroy(&a);
            sv4_destroy(&b);
            sv4_destroy(&same);
            sv4_destroy(&fallback);
        }
    sv4_t values[4] = {sv4_from_u64(1, 3, 0), sv4_from_u64(2, 3, 0), sv4_from_u64(4, 3, 0),
                       sv4_from_u64(2, 3, 0)};
    sv4_t fallback = sv4_from_u64(7, 3, 0);
    for (uint64_t current = 0; current < 9; ++current)
        for (int direction = -1; direction <= 1; direction += 2) {
            sv4_t cur = current == 8 ? sv4_x(3, 0) : sv4_from_u64(current, 3, 0);
            sv4_t step = sv4_from_u64(1, 3, 0);
            sv4_t result = sv4_enum_navigate(cur, step, values, 4, fallback, direction), into = SV4_EMPTY;
            sv4_enum_navigate_to(&into, &cur, &step, values, 4, &fallback, direction);
            printf("sv4_enum_navigate|%" PRIu64 " %d|", current, direction);
            put_owner(&result);
            printf("\nsv4_enum_navigate_to|%" PRIu64 " %d|", current, direction);
            put_owner(&into);
            putchar('\n');
            sv4_destroy(&cur);
            sv4_destroy(&step);
        }
    sv4_destroy_array(values, 4);
    sv4_destroy(&fallback);
}

static void udp(void) {
    static const uint8_t rows[] = {1, 2, 1, 4, 4, 2, 7, 7, 0, 3, 1, 1};
    for (unsigned states = 0; states < 16; ++states) {
        sv4_t a = sv4_fill((uint8_t)(states % 4), 1, 0), b = sv4_fill((uint8_t)(states / 4), 1, 0);
        const sv4_t* inputs[2] = {&a, &b};
        sv4_t result = sv4_udp_eval(rows, 4, 2, inputs), into = SV4_EMPTY;
        sv4_udp_eval_to(&into, rows, 4, 2, inputs);
        printf("sv4_udp_eval|%u|", states);
        put_owner(&result);
        printf("\nsv4_udp_eval_to|%u|", states);
        put_owner(&into);
        putchar('\n');
        sv4_destroy(&a);
        sv4_destroy(&b);
    }
}

static void references(void) {
    sv4_t cells[3] = {make(65, 1, 6, 1), make(65, 0, 5, 2), make(65, 0, 3, 3)};
    static const uint8_t kinds[] = {LLG_REF_WHOLE, LLG_REF_BIT, LLG_REF_PART, LLG_REF_INDEXED,
                                    LLG_REF_ARRAY};
    for (size_t kind = 0; kind < COUNT_OF(kinds); ++kind)
        for (int two_state = 0; two_state < 2; ++two_state)
            for (int sign = 0; sign < 2; ++sign) {
                llg_ref_t ref;
                memset(&ref, 0, sizeof(ref));
                ref.base = cells;
                ref.kind = kinds[kind];
                ref.two_state = (uint8_t)two_state;
                ref.is_signed = (int8_t)sign;
                ref.width = kinds[kind] == LLG_REF_WHOLE || kinds[kind] == LLG_REF_ARRAY ? 65u : 5u;
                ref.left = 12;
                ref.right = 8;
                ref.index = kinds[kind] == LLG_REF_ARRAY ? 1u : 63u;
                ref.indexed_width = 5;
                ref.array_size = 3;
                sv4_t result = llg_ref_read(&ref), into = SV4_EMPTY;
                llg_ref_read_to(&into, &ref);
                printf("llg_ref_read|%zu %d %d|", kind, two_state, sign);
                put_owner(&result);
                printf("\nllg_ref_read_to|%zu %d %d|", kind, two_state, sign);
                put_owner(&into);
                putchar('\n');
            }
    sv4_destroy_array(cells, 3);
}

static void owners(void) {
    for (size_t pair = 0; pair < COUNT_OF(pair_widths); ++pair)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
            sv4_t a = make(pair_widths[pair][0], pattern & 1, pattern, 1);
            sv4_t b = make(pair_widths[pair][1], 1, (pattern + 4) % PATTERNS, 2);
            sv4_t copy = sv4_clone(&b), assign = sv4_clone(&b), moved = sv4_clone(&b), replaced = sv4_clone(&b);
            sv4_copy(&copy, &a);
            sv4_assign(&assign, a);
            sv4_t source = sv4_clone(&a);
            sv4_move(&moved, &source);
            sv4_replace(&replaced, sv4_clone(&a));
            sv4_t self_copy = sv4_clone(&a), self_move = sv4_clone(&a);
            sv4_copy(&self_copy, &self_copy);
            sv4_move(&self_move, &self_move);
            printf("sv4_clone|%u %u|", pair_widths[pair][0], pattern);
            sv4_t cloned = sv4_clone(&a);
            put_owner(&cloned);
            printf("\nsv4_copy|%u/%u %u|", pair_widths[pair][0], pair_widths[pair][1], pattern);
            put_owner(&copy);
            put_owner(&self_copy);
            printf("\nsv4_assign|%u/%u %u|", pair_widths[pair][0], pair_widths[pair][1], pattern);
            put_owner(&assign);
            printf("\nsv4_move|%u/%u %u|", pair_widths[pair][0], pair_widths[pair][1], pattern);
            put_owner(&moved);
            put(source);
            put_owner(&self_move);
            printf("\nsv4_replace|%u/%u %u|", pair_widths[pair][0], pair_widths[pair][1], pattern);
            put_owner(&replaced);
            sv4_destroy(&a);
            sv4_destroy(&a);
            printf("\nsv4_destroy|%u %u|", pair_widths[pair][0], pattern);
            put(a);
            sv4_t array[2] = {make(pair_widths[pair][0], 0, pattern, 1), make(pair_widths[pair][1], 1, pattern, 2)};
            sv4_destroy_array(array, 2);
            printf("\nsv4_destroy_array|%u/%u %u|", pair_widths[pair][0], pair_widths[pair][1],
                   pattern);
            put(array[0]);
            put(array[1]);
            putchar('\n');
            sv4_destroy(&b);
            sv4_destroy(&source);
        }
}

static void neutral_bridge(void) {
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (int sign = 0; sign < 2; ++sign)
            for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
                uint32_t width = unary_widths[w];
                sv4_t a = make(width, sign, pattern, 1);
                printf("llg_sv4_width|%u %d %u| %u\n", width, sign, pattern, llg_sv4_width(a));
                printf("llg_sv4_signed|%u %d %u| %d\n", width, sign, pattern, (int)llg_sv4_signed(a));
                printf("llg_sv4_words|%u %d %u| %zu\n", width, sign, pattern, llg_sv4_words(a));
                printf("llg_sv4_word_range|%u|", width);
                for (size_t first = 0; first < 4; ++first)
                    for (size_t count = 0; count < 4; ++count)
                        printf(" %zu", llg_sv4_word_range(a, first, count));
                printf("\nsv4_is_unknown|%u %d %u| %d\n", width, sign, pattern, sv4_is_unknown(a));
                printf("llg_sv4_has_x|%u %d %u| %d\n", width, sign, pattern, llg_sv4_has_x(a));
                printf("llg_sv4_has_z|%u %d %u| %d\n", width, sign, pattern, llg_sv4_has_z(a));
                printf("llg_sv4_state|%u %d %u|", width, sign, pattern);
                for (uint64_t bit = 0; bit < 133; bit += bit < 70 ? 1u : 7u)
                    printf(" %u", llg_sv4_state(a, bit));
                printf("\nllg_sv4_word|%u %d %u|", width, sign, pattern);
                for (size_t word = 0; word < 4; ++word)
                    for (unsigned plane = LLG_SV4_BITS; plane <= LLG_SV4_Z; ++plane)
                        printf(" %" PRIx64, llg_sv4_word(a, word, plane));
                printf("\nllg_sv4_vpi_word|%u %d %u|", width, sign, pattern);
                for (size_t word = 0; word < 4; ++word) {
                    llg_sv4_vpi_word_t vpi = llg_sv4_vpi_word(a, word);
                    printf(" %" PRIx64 ".%" PRIx64, vpi.aval, vpi.bval);
                }
                putchar('\n');
                static const size_t firsts[] = {0, 1, 2, 5};
                for (size_t f = 0; f < COUNT_OF(firsts); ++f) {
                    llg_sv4_word_t words[4];
                    llg_sv4_vpi_word_t vpi_words[4];
                    memset(words, 0xff, sizeof(words));
                    memset(vpi_words, 0xff, sizeof(vpi_words));
                    llg_sv4_export_words(a, firsts[f], words, 4);
                    llg_sv4_export_vpi_words(a, firsts[f], vpi_words, 4);
                    printf("llg_sv4_export_words|%u %d %u %zu|", width, sign, pattern, firsts[f]);
                    for (size_t i = 0; i < 4; ++i)
                        printf(" %" PRIx64 ".%" PRIx64 ".%" PRIx64, words[i].bits, words[i].x, words[i].z);
                    printf("\nllg_sv4_export_vpi_words|%u %d %u %zu|", width, sign, pattern, firsts[f]);
                    for (size_t i = 0; i < 4; ++i)
                        printf(" %" PRIx64 ".%" PRIx64, vpi_words[i].aval, vpi_words[i].bval);
                    putchar('\n');
                    sv4_t target = make(width, sign, 6, 9), vpi_target = make(width, sign, 6, 9);
                    llg_sv4_word_t input[3] = {{UINT64_MAX, 0, 0}, {5, UINT64_C(0xf0), UINT64_C(0xff00)},
                                               {UINT64_C(0x123), UINT64_MAX, UINT64_MAX}};
                    llg_sv4_vpi_word_t vpi_input[3] = {{UINT64_MAX, UINT64_C(0xf0f0)}, {5, 5},
                                                       {UINT64_C(0xabc), UINT64_MAX}};
                    llg_sv4_import_words(&target, firsts[f], input, 3);
                    llg_sv4_import_vpi_words(&vpi_target, firsts[f], vpi_input, 3);
                    printf("llg_sv4_import_words|%u %d %u %zu|", width, sign, pattern, firsts[f]);
                    put_owner(&target);
                    printf("\nllg_sv4_import_vpi_words|%u %d %u %zu|", width, sign, pattern, firsts[f]);
                    put_owner(&vpi_target);
                    putchar('\n');
                }
                sv4_t signed_target = sv4_clone(&a), word_target = make(width, 0, 6, 9),
                      vpi_target = make(width, 0, 6, 9), state_target = make(width, 0, 6, 9);
                llg_sv4_set_signed(&signed_target, 2);
                printf("llg_sv4_set_signed|%u %d %u|", width, sign, pattern);
                put(signed_target);
                llg_sv4_set_signed(&signed_target, 0);
                put_owner(&signed_target);
                for (size_t word = 0; word < 4; ++word) {
                    llg_sv4_set_word(&word_target, word, UINT64_C(0xdeadbeefcafef00d), UINT64_C(0x00ff00ff00ff00ff),
                                     UINT64_C(0x0ff00ff00ff00ff0));
                    llg_sv4_set_vpi_word(&vpi_target, word, (llg_sv4_vpi_word_t){UINT64_C(0xdeadbeefcafef00d), UINT64_C(0xff00ff00ff00ff00)});
                }
                printf("\nllg_sv4_set_word|%u %d %u|", width, sign, pattern);
                put_owner(&word_target);
                printf("\nllg_sv4_set_vpi_word|%u %d %u|", width, sign, pattern);
                put_owner(&vpi_target);
                for (uint64_t bit = 0; bit < 133; bit += 13)
                    for (unsigned state = 0; state < 4; ++state)
                        llg_sv4_set_state(&state_target, bit, state);
                for (uint64_t bit = 0; bit < width; bit += 5)
                    llg_sv4_set_state(&state_target, bit, (unsigned)(bit / 5) % 4u);
                printf("\nllg_sv4_set_state|%u %d %u|", width, sign, pattern);
                put_owner(&state_target);
                putchar('\n');
                sv4_destroy(&a);
            }
    for (unsigned state = 0; state < 4; ++state)
        printf("llg_sv4_state_to_dpi|%u| %u\nllg_sv4_state_from_dpi|%u| %u\n", state,
               llg_sv4_state_to_dpi(state), state, llg_sv4_state_from_dpi(state));
}

static void put_plan(const sv4_select_plan_t* plan) {
    printf(" plan %u %u %u %u %u", plan->storage_width, plan->width, plan->storage_lsb,
           plan->value_lsb, plan->count);
}

/* Writes into a selection: the target is a clone, so every backend starts from
 * the same bits; the value may alias the target. */
static void selection_writes(void) {
    static const int64_t ranges[][2] = {{0, 0}, {62, 0}, {64, 1}, {127, 64}, {64, 127}, {130, 120}, {-3, 4}, {5, -3}};
    static const uint64_t bases[] = {0, 1, 63, 64, 65, 127, 1000};
    static const uint32_t value_widths[] = {1, 3, 64, 70};
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
            uint32_t width = unary_widths[w];
            sv4_t base_value = make(width, pattern & 1, 6, 5);
            for (size_t v = 0; v < COUNT_OF(value_widths); ++v) {
                sv4_t value = make(value_widths[v], (int)(v & 1u), (pattern + 3) % PATTERNS, 6);
                for (size_t r = 0; r < COUNT_OF(ranges); ++r) {
                    sv4_t target = sv4_clone(&base_value);
                    sv4_part_select_set(&target, ranges[r][0], ranges[r][1], value);
                    printf("sv4_part_select_set|%u %u %zu [%" PRId64 ":%" PRId64 "]|", width, pattern, v,
                           ranges[r][0], ranges[r][1]);
                    put_owner(&target);
                    putchar('\n');
                    sv4_select_plan_t plan = sv4_select_plan_part(width, ranges[r][0], ranges[r][1]);
                    target = sv4_clone(&base_value);
                    sv4_t sized = sv4_resize(value, plan.width, 0);
                    sv4_select_plan_set(&target, &plan, sized);
                    sv4_destroy(&sized);
                    printf("sv4_select_plan_part|%u [%" PRId64 ":%" PRId64 "]|", width, ranges[r][0], ranges[r][1]);
                    put_plan(&plan);
                    printf("\nsv4_select_plan_set|%u %u %zu [%" PRId64 ":%" PRId64 "]|", width, pattern, v,
                           ranges[r][0], ranges[r][1]);
                    put_owner(&target);
                    putchar('\n');
                }
                for (size_t b = 0; b < COUNT_OF(bases); ++b) {
                    sv4_t target = sv4_clone(&base_value);
                    sv4_bit_select_set(&target, bases[b], value);
                    printf("sv4_bit_select_set|%u %u %zu %" PRIu64 "|", width, pattern, v, bases[b]);
                    put_owner(&target);
                    for (int neg = 0; neg < 2; ++neg) {
                        sv4_t by_index = sv4_clone(&base_value), by_value = sv4_clone(&base_value);
                        sv4_t base = sv4_from_i64(neg ? -(int64_t)bases[b] : (int64_t)bases[b], 65);
                        sv4_idx_part_select_set(&by_index, bases[b], value_widths[v], neg, value);
                        sv4_idx_part_select_set_value(&by_value, base, value_widths[v], neg, value);
                        printf("\nsv4_idx_part_select_set|%u %u %zu %" PRIu64 " %d|", width, pattern, v, bases[b], neg);
                        put_owner(&by_index);
                        printf("\nsv4_idx_part_select_set_value|%u %u %zu %" PRIu64 " %d|", width, pattern, v,
                               bases[b], neg);
                        put_owner(&by_value);
                        sv4_select_plan_t plan = sv4_select_plan_indexed(width, base, value_widths[v], neg);
                        sv4_select_plan_t bit_plan = sv4_select_plan_bit(width, bases[b]);
                        sv4_select_plan_t stepped = sv4_select_plan_init(width);
                        sv4_select_plan_step(&stepped, base, value_widths[v]);
                        sv4_select_plan_step(&stepped, base, 2);
                        sv4_select_plan_t whole = sv4_select_plan_init(width);
                        printf("\nsv4_select_plan_indexed|%u %" PRIu64 " %u %d|", width, bases[b], value_widths[v], neg);
                        put_plan(&plan);
                        printf("\nsv4_select_plan_bit|%u %" PRIu64 "|", width, bases[b]);
                        put_plan(&bit_plan);
                        printf("\nsv4_select_plan_step|%u %" PRIu64 " %u|", width, bases[b], value_widths[v]);
                        put_plan(&stepped);
                        printf("\nsv4_select_plan_init|%u|", width);
                        put_plan(&whole);
                        sv4_destroy(&base);
                    }
                    putchar('\n');
                }
                sv4_destroy(&value);
            }
            sv4_destroy(&base_value);
        }
}

static void consumers(void) {
    static const unsigned known_patterns[] = {0, 1, 4, 5};
    static const uint32_t counts[] = {0, 1, 2, 64, 65, UINT32_MAX};
    static const uint32_t offsets[] = {0, 1, 31, 63, 64, 127, 200};
    for (size_t pair = 0; pair < COUNT_OF(pair_widths); ++pair)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern)
            for (size_t k = 0; k < COUNT_OF(known_patterns); ++k) {
                uint32_t target_width = pair_widths[pair][0], source_width = pair_widths[pair][1];
                sv4_t target = make(target_width, 0, pattern, 1), source = make(source_width, 0, (pattern + 2) % PATTERNS, 2);
                sv4_t mask = make(target_width, 0, known_patterns[k], 3);
                sv4_t merged = sv4_clone(&target), copied = sv4_clone(&target);
                llg_sv4_masked_merge(&merged, source, mask);
                llg_sv4_masked_copy(&copied, source, mask);
                printf("llg_sv4_masked_merge|%u/%u %u %u|", target_width, source_width, pattern, known_patterns[k]);
                put_owner(&merged);
                printf("\nllg_sv4_masked_copy|%u/%u %u %u|", target_width, source_width, pattern, known_patterns[k]);
                put_owner(&copied);
                printf("\nllg_sv4_masked_same|%u/%u %u %u| %d %d\n", target_width, source_width, pattern, known_patterns[k],
                       llg_sv4_masked_same(target, source, &mask), llg_sv4_masked_same(target, source, NULL));
                sv4_t removed = sv4_clone(&mask), self_merged = sv4_clone(&target), self_copied = sv4_clone(&target);
                llg_sv4_mask_remove(&removed, target);
                llg_sv4_masked_merge(&self_merged, self_merged, self_merged);
                llg_sv4_masked_copy(&self_copied, self_copied, self_copied);
                printf("llg_sv4_mask_remove|%u %u %u|", target_width, pattern, known_patterns[k]);
                put_owner(&removed);
                printf("\nllg_sv4_masked_merge|self %u %u|", target_width, pattern);
                put_owner(&self_merged);
                printf("\nllg_sv4_masked_copy|self %u %u|", target_width, pattern);
                put_owner(&self_copied);
                putchar('\n');
                sv4_destroy(&target);
                sv4_destroy(&source);
                sv4_destroy(&mask);
            }
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
            uint32_t width = unary_widths[w];
            sv4_t value = make(width, 0, pattern, 1);
            for (size_t c = 0; c < COUNT_OF(counts); ++c)
                for (unsigned state = 0; state < 4; ++state)
                    for (size_t o = 0; o < COUNT_OF(offsets); ++o) {
                        sv4_t filled = sv4_clone(&value);
                        llg_sv4_range_fill(&filled, offsets[o], counts[c], state);
                        printf("llg_sv4_range_fill|%u %u %u %u %u|", width, pattern, offsets[o], counts[c], state);
                        put_owner(&filled);
                        putchar('\n');
                    }
            for (size_t o = 0; o < COUNT_OF(offsets); ++o) {
                static const uint32_t source_widths[] = {1, 3, 63, 64};
                for (size_t sw = 0; sw < COUNT_OF(source_widths); ++sw) {
                    if ((uint64_t)offsets[o] + source_widths[sw] > width)
                        continue;
                    sv4_t source = make(source_widths[sw], 0, (pattern + 4) % PATTERNS, 2);
                    sv4_t copied = sv4_clone(&value);
                    llg_sv4_range_copy(&copied, offsets[o], source);
                    printf("llg_sv4_range_copy|%u %u %u %u|", width, pattern, offsets[o], source_widths[sw]);
                    put_owner(&copied);
                    sv4_t slice = sv4_part_select(value, (int64_t)offsets[o] + source_widths[sw] - 1, offsets[o]);
                    printf("\nllg_sv4_range_same|%u %u %u %u| %d %d\n", width, pattern, offsets[o], source_widths[sw],
                           llg_sv4_range_same(value, offsets[o], source), llg_sv4_range_same(value, offsets[o], slice));
                    sv4_destroy(&slice);
                    sv4_destroy(&source);
                }
            }
            for (unsigned digit = 0; digit < 16; digit += 5)
                for (unsigned count = 1; count <= 4; ++count)
                    for (unsigned state = 0; state < 4; ++state) {
                        sv4_t appended = sv4_clone(&value);
                        llg_sv4_append_digit(&appended, count, state, digit);
                        printf("llg_sv4_append_digit|%u %u %u %u %u|", width, pattern, count, state, digit);
                        put_owner(&appended);
                        putchar('\n');
                    }
            sv4_t two_state = sv4_clone(&value), top = sv4_clone(&value);
            llg_sv4_two_state_inplace(&two_state);
            llg_sv4_mask_top(&top);
            printf("llg_sv4_two_state_inplace|%u %u|", width, pattern);
            put_owner(&two_state);
            printf("\nllg_sv4_mask_top|%u %u|", width, pattern);
            put_owner(&top);
            putchar('\n');
            if (!sv4_is_unknown(value)) {
                static const uint32_t factors[] = {0, 1, 10, UINT32_MAX};
                for (size_t f = 0; f < COUNT_OF(factors); ++f) {
                    sv4_t multiplied = sv4_clone(&value);
                    llg_sv4_mul_add_known(&multiplied, factors[f], 7u + (uint32_t)f);
                    printf("llg_sv4_mul_add_known|%u %u %u|", width, pattern, factors[f]);
                    put_owner(&multiplied);
                    putchar('\n');
                }
                sv4_t negated = sv4_clone(&value);
                llg_sv4_negate_known(&negated);
                printf("llg_sv4_negate_known|%u %u|", width, pattern);
                put_owner(&negated);
                putchar('\n');
            }
            size_t record_count = 5;
            for (size_t stride_index = 0; stride_index < 2; ++stride_index) {
                size_t stride = stride_index ? 12 : 8;
                unsigned char raw[16 * 12 + 4];
                unsigned char input[16 * 12 + 4];
                for (size_t i = 0; i < sizeof(raw); ++i) {
                    raw[i] = 0xa5;
                    input[i] = (unsigned char)(i * 37u + pattern);
                }
                llg_sv4_export_vpi32(value, raw, record_count, stride);
                sv4_t imported = sv4_clone(&value);
                llg_sv4_import_vpi32(&imported, input, record_count, stride);
                printf("llg_sv4_export_vpi32|%u %u %zu|", width, pattern, stride);
                for (size_t i = 0; i < record_count * stride; ++i)
                    printf("%02x", raw[i]);
                printf("\nllg_sv4_import_vpi32|%u %u %zu|", width, pattern, stride);
                put_owner(&imported);
                putchar('\n');
            }
            llg_sv4_vpi_word_t snapshot[4];
            llg_sv4_export_vpi_words(value, 0, snapshot, 4);
            printf("llg_sv4_same_vpi_words|%u %u|", width, pattern);
            for (size_t count = 0; count < 5; ++count)
                printf(" %d", llg_sv4_same_vpi_words(value, snapshot, count));
            snapshot[0].aval ^= 1;
            printf(" %d", llg_sv4_same_vpi_words(value, snapshot, 4));
            char text[260];
            printf("\nllg_sv4_export_text|%u %u|", width, pattern);
            for (size_t t = 0; t < 4; ++t) {
                uint32_t text_width = t == 0 ? 0 : t == 1 ? width / 2 : t == 2 ? width : width + 67;
                memset(text, '!', sizeof(text));
                llg_sv4_export_text(value, text_width, text);
                printf(" %s", text);
            }
            printf("\nllg_sv4_plane_slice|%u %u|", width, pattern);
            for (unsigned low = 0; low + 1u <= width; low += 31u)
                for (unsigned count = 1; count <= 64u && low + count <= width; count += 21u)
                    for (unsigned plane = LLG_SV4_BITS; plane <= LLG_SV4_Z; ++plane)
                        printf(" %" PRIx64, llg_sv4_plane_slice(value, low, count, plane));
            putchar('\n');
            sv4_destroy(&value);
        }
}

static void nets_and_records(void) {
    static const uint32_t net_widths[] = {1, 64, 65, 128};
    static const uint8_t strength0[4] = {6, 3, 5, 7}, strength1[4] = {3, 6, 7, 5};
    static const int indices[3] = {3, 1, 0};
    for (size_t w = 0; w < COUNT_OF(net_widths); ++w)
        for (unsigned pattern = 0; pattern < PATTERNS; ++pattern)
            for (int mode = 0; mode < 7; ++mode) {
                uint32_t width = net_widths[w];
                sv4_t drivers[4] = {make(width, 0, pattern, 1), make(width, 0, (pattern + 1) % PATTERNS, 2),
                                    make(width, 0, (pattern + 3) % PATTERNS, 3), make(width, 0, (pattern + 5) % PATTERNS, 4)};
                const sv4_t* pointers[4] = {&drivers[0], &drivers[1], &drivers[2], &drivers[3]};
                sv4_t plain = sv4_resolve(pointers, 4, width, 1, mode);
                sv4_t strong = sv4_resolve_strengths(pointers, strength0, strength1, 4, width, 0, mode);
                uint32_t offset = width > 65 ? 63 : width / 2;
                sv4_t range = sv4_resolve_strengths_range(pointers, strength0, strength1, indices, 3, width, offset,
                                                          width - offset, 1, mode);
                sv4_t no_strength = sv4_resolve_strengths_range(pointers, NULL, strength1, indices, 3, width, offset,
                                                                width - offset, 0, mode);
                printf("sv4_resolve|%u %u %d|", width, pattern, mode);
                put_owner(&plain);
                printf("\nsv4_resolve_strengths|%u %u %d|", width, pattern, mode);
                put_owner(&strong);
                printf("\nsv4_resolve_strengths_range|%u %u %d|", width, pattern, mode);
                put_owner(&range);
                put_owner(&no_strength);
                putchar('\n');
                sv4_destroy_array(drivers, 4);
            }
    static const uint32_t layouts[][4] = {{4, 1, 8, 0}, {1, 64, 3, 0}, {63, 2, 65, 0}, {129, 0, 0, 0}, {1, 1, 1, 0}, {64, 64, 1, 7}};
    for (size_t k = 0; k < COUNT_OF(layouts); ++k) {
        uint32_t period = 0, leaves = 0;
        while (leaves < 4 && layouts[k][leaves])
            period += layouts[k][leaves++];
        for (uint32_t count = 1; count <= 3; count += 2)
            for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
                uint32_t width = period * count;
                sv4_t a = make(width, 0, pattern, 1), b = sv4_clone(&a);
                sv4_t fallback = make(period, 0, (pattern + 2) % PATTERNS, 3), starts = sv4_zero(period, 0);
                for (uint32_t i = 0, at = 0; i < leaves; at += layouts[k][i++])
                    llg_sv4_set_state(&starts, at, 1);
                llg_sv4_set_state(&b, period - 1, (llg_sv4_state(a, period - 1) + 1u) % 2u);
                sv4_t merged = sv4_record_conditional_merge(a, b, fallback, starts);
                printf("sv4_record_conditional_merge|%zu %u %u|", k, count, pattern);
                put_owner(&merged);
                putchar('\n');
                sv4_destroy(&a);
                sv4_destroy(&b);
                sv4_destroy(&fallback);
                sv4_destroy(&starts);
            }
    }
    for (unsigned match = 0; match < 3; ++match)
        for (unsigned tagged = 0; tagged < 2; ++tagged) {
            sv4_t parent = make(8, 0, 5, 1);
            llg_ref_t base;
            memset(&base, 0, sizeof(base));
            base.base = &parent;
            base.kind = LLG_REF_WHOLE;
            base.width = 8;
            llg_ref_tag_check_t check = {sv4_select_plan_init(8), 3, 5, "member"};
            llg_ref_view_t view = {&base, sv4_select_plan_part(8, 4, 0), 1, &check, "test"};
            check.member_index = match == 0 ? (unsigned)sv4_to_u64(parent) >> 5 : 4u;
            if (match == 2)
                llg_sv4_set_state(&parent, 7, 2);
            size_t failed = 99;
            int valid = llg_ref_view_valid(&view, &parent, &failed);
            printf("llg_ref_view_valid|%u %u| %d %zu %d", match, tagged, valid, failed, llg_ref_view_valid(NULL, &parent, NULL));
            check.tag_width = 9;
            printf(" %d\n", llg_ref_view_valid(&view, &parent, &failed));
            sv4_destroy(&parent);
        }
}

static uint64_t double_bits(double value) {
    uint64_t bits = 0;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

/* Destination forms that reuse the payload when the operands allow it. */
static void into_family(void) {
    typedef void (*into_fn)(sv4_t*, sv4_t, sv4_t);
    typedef sv4_t (*returning_fn)(sv4_t, sv4_t);
    static const struct {
        const char* name;
        into_fn into;
        returning_fn value;
    } ops[] = {{"sv4_add_into", sv4_add_into, sv4_add}, {"sv4_sub_into", sv4_sub_into, sv4_sub},
               {"sv4_mul_into", sv4_mul_into, sv4_mul}, {"sv4_and_into", sv4_and_into, sv4_and},
               {"sv4_or_into", sv4_or_into, sv4_or},     {"sv4_xor_into", sv4_xor_into, sv4_xor},
               {"sv4_xnor_into", sv4_xnor_into, sv4_xnor}};
    for (size_t op = 0; op < COUNT_OF(ops); ++op)
        for (size_t pair = 0; pair < COUNT_OF(pair_widths); ++pair)
            for (int sign_a = 0; sign_a < 2; ++sign_a)
                for (unsigned pattern_a = 0; pattern_a < PATTERNS; ++pattern_a)
                    for (unsigned step = 0; step < 3; ++step) {
                        unsigned pattern_b = (pattern_a + (step == 0 ? 0u : step == 1 ? 3u : 5u)) % PATTERNS;
                        sv4_t a = make(pair_widths[pair][0], sign_a, pattern_a, 1);
                        sv4_t b = make(pair_widths[pair][1], sign_a ^ ((int)step & 1), pattern_b, 2);
                        sv4_t empty = SV4_EMPTY, same_width = sv4_zero(pair_widths[pair][0] > pair_widths[pair][1] ? pair_widths[pair][0] : pair_widths[pair][1], 0);
                        sv4_t alias_a = sv4_clone(&a), alias_b = sv4_clone(&b), alias_both = sv4_clone(&a);
                        ops[op].into(&empty, a, b);
                        ops[op].into(&same_width, a, b);
                        ops[op].into(&alias_a, alias_a, b);
                        ops[op].into(&alias_b, a, alias_b);
                        ops[op].into(&alias_both, alias_both, alias_both);
                        sv4_t expected = ops[op].value(a, b);
                        printf("%s|%u/%u %d %u %u|", ops[op].name, pair_widths[pair][0], pair_widths[pair][1], sign_a, pattern_a, pattern_b);
                        put_owner(&expected);
                        put_owner(&empty);
                        put_owner(&same_width);
                        put_owner(&alias_a);
                        put_owner(&alias_b);
                        put_owner(&alias_both);
                        putchar('\n');
                        sv4_destroy(&a);
                        sv4_destroy(&b);
                    }
}

static void scalars(void) {
    for (size_t w = 0; w < COUNT_OF(unary_widths); ++w)
        for (int sign = 0; sign < 2; ++sign)
            for (unsigned pattern = 0; pattern < PATTERNS; ++pattern) {
                uint32_t width = unary_widths[w];
                sv4_t a = make(width, sign, pattern, 1);
                sv4_t other = make(width, sign, (pattern + 5) % PATTERNS, 4);
                char text[4][200], decimal[200];
                static const char formats[4] = {'d', 'h', 'b', 'o'};
                memset(text, 0, sizeof(text));
                memset(decimal, 0, sizeof(decimal));
                for (unsigned f = 0; f < 4; ++f)
                    sv4_format(formats[f], a, text[f], sizeof(text[f]));
                sv4_to_dec_string(a, decimal, sizeof(decimal));
                int64_t index_value = 0x1234;
                int valid_index = sv4_to_index_i64(a, &index_value);
#define SCALAR(name, expr) \
    printf(name "|%u %d %u|", width, sign, pattern), put_u64((uint64_t)(expr)), putchar('\n')
                SCALAR("sv4_to_bool", sv4_to_bool(a));
                SCALAR("sv4_to_u64", sv4_to_u64(a));
                SCALAR("sv4_to_index", sv4_to_index(a));
                SCALAR("sv4_to_i64", sv4_to_i64(a));
                SCALAR("sv4_fits_i64", sv4_fits_i64(a));
                SCALAR("sv4_same", sv4_same(a, other) * 2 + sv4_same(a, a));
                SCALAR("sv4_is_unknown", sv4_is_unknown(a));
                SCALAR("sv4_delay_ticks", sv4_delay_ticks(a, 1));
                SCALAR("sv4_delay_ticks", sv4_delay_ticks(a, 1000));
                SCALAR("sv4_to_real", double_bits(sv4_to_real(a)));
                SCALAR("sv4_bitstoreal", double_bits(sv4_bitstoreal(a)));
                SCALAR("sv4_bitstoshortreal", double_bits(sv4_bitstoshortreal(a)));
#undef SCALAR
                printf("sv4_to_index_i64|%u %d %u| %d %" PRId64 "\n", width, sign, pattern, valid_index,
                       index_value);
                printf("sv4_format|%u %d %u| %s %s %s %s\n", width, sign, pattern, text[0], text[1],
                       text[2], text[3]);
                printf("sv4_to_dec_string|%u %d %u| %s\n", width, sign, pattern, decimal);
                sv4_destroy(&other);
                sv4_destroy(&a);
            }
    static const double doubles[] = {0.0, 0.4, 0.5, -0.5, 1.5e19, -1.0};
    for (size_t i = 0; i < COUNT_OF(doubles); ++i)
        printf("llg_real_to_bool|%a| %d\n", doubles[i], llg_real_to_bool(doubles[i]));
    for (uint32_t width = 0; width < 70; width += 17) {
        sv4_t value = sv4_from_u64(width, 32, 0);
        printf("sv4_checked_width|%u| %u\n", width, sv4_checked_width(value));
        sv4_destroy(&value);
    }
}

/* The first argument names the exercised facade constant for the coverage
 * derivation; LLG_MASK is a macro with a parameter and prints its samples. */
#define CONSTANT(name) printf(#name "| %" PRIx64 "\n", (uint64_t)(name))
static void constants(void) {
    CONSTANT(LLG_REF_WHOLE);
    CONSTANT(LLG_REF_BIT);
    CONSTANT(LLG_REF_PART);
    CONSTANT(LLG_REF_INDEXED);
    CONSTANT(LLG_REF_ARRAY);
    CONSTANT(LLG_REF_QUEUE);
    CONSTANT(LLG_REF_PACKED_PLAN);
    CONSTANT(LLG_REF_COMPOSITE);
    CONSTANT(LLG_REF_VIEW);
    CONSTANT(LLG_REF_TAGGED_VIEW);
    CONSTANT(LLG_RESOLVE_WIRE);
    CONSTANT(LLG_RESOLVE_WAND);
    CONSTANT(LLG_RESOLVE_WOR);
    CONSTANT(LLG_RESOLVE_TRI0);
    CONSTANT(LLG_RESOLVE_TRI1);
    CONSTANT(LLG_RESOLVE_SUPPLY0);
    CONSTANT(LLG_RESOLVE_SUPPLY1);
    CONSTANT(LLG_STRENGTH_HIGHZ);
    CONSTANT(LLG_STRENGTH_SMALL);
    CONSTANT(LLG_STRENGTH_MEDIUM);
    CONSTANT(LLG_STRENGTH_WEAK);
    CONSTANT(LLG_STRENGTH_LARGE);
    CONSTANT(LLG_STRENGTH_PULL);
    CONSTANT(LLG_STRENGTH_STRONG);
    CONSTANT(LLG_STRENGTH_SUPPLY);
    CONSTANT(LLG_SV4_BITS);
    CONSTANT(LLG_SV4_X);
    CONSTANT(LLG_SV4_Z);
    CONSTANT(LLG_SUPPORTED_WIDTH_LIMIT);
    CONSTANT(LLG_VALUE_ABI_VERSION);
    /* Run-time widths: a literal 64 reaches `1ULL << 64` in the dead branch of
     * the legacy macro, which MSVC reports (C4293, and C4127 under /W4). */
    static const uint32_t mask_widths[3] = {1, 63, 64};
    printf("LLG_MASK| %" PRIx64 " %" PRIx64 " %" PRIx64 "\n", (uint64_t)LLG_MASK(mask_widths[0]),
           (uint64_t)LLG_MASK(mask_widths[1]), (uint64_t)LLG_MASK(mask_widths[2]));
}

int main(void) {
    static char buffer[1 << 16];
    setvbuf(stdout, buffer, _IOFBF, sizeof(buffer));
    constants();
    constructors();
    owners();
    neutral_bridge();
    scalars();
    into_family();
    selection_writes();
    consumers();
    nets_and_records();
    conversions();
    unary_grid();
    onehot_and_repeat();
    selections();
    binary_grid();
    mux_and_ranges();
    udp();
    references();
    return fflush(stdout) == 0 ? 0 : 1;
}
