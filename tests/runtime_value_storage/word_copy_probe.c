#include "probe.h"
#include <limits.h>
#include <string.h>

static size_t cases;
static const uint32_t widths[] = {1, 7, 63, 64, 65, 127, 128, 129, 1023, 4096};

/* The oracle's own value: one state per bit (0, 1, 2 = X, 3 = Z, as
 * probe_state reports them). Expected results are built here bit by bit and
 * converted once through sv4_from_limbs, so the per-bit reference work never
 * calls the runtime under test; at /Od /RTC1 (MSVC Debug) those per-bit calls
 * made this probe several times slower than the operations it checks. */
typedef struct {
    uint32_t width;
    int8_t is_signed;
    unsigned char* states;
} ref_t;

static ref_t ref_new(uint32_t width, int8_t is_signed, unsigned fill) {
    ref_t ref = {width, is_signed, (unsigned char*)malloc(width ? width : 1u)};
    CHECK(ref.states != NULL);
    memset(ref.states, (int)fill, width);
    return ref;
}

static ref_t ref_of(sv4_t value) {
    ref_t ref = ref_new(value.width, value.is_signed, 0);
    for (uint32_t word = 0; (uint64_t)word * 64u < value.width; ++word) {
        uint64_t bits = PROBE_BITS(value, word), x = PROBE_X(value, word);
        uint64_t z = PROBE_Z(value, word);
        for (uint32_t bit = word * 64u; bit < value.width && bit < word * 64u + 64u; ++bit) {
            uint64_t mask = UINT64_C(1) << (bit % 64u);
            ref.states[bit] = (unsigned char)((x & mask) ? 2 : (z & mask) ? 3 : (bits & mask) != 0);
        }
    }
    return ref;
}

static ref_t ref_clone(const ref_t* ref) {
    ref_t copy = ref_new(ref->width, ref->is_signed, 0);
    memcpy(copy.states, ref->states, ref->width);
    return copy;
}

static void ref_free(ref_t* ref) {
    free(ref->states);
    ref->states = NULL;
}

static unsigned digit(const ref_t* ref, int64_t bit) {
    if (bit < 0 || bit >= ref->width) return 2;
    return ref->states[bit];
}

static void put(ref_t* ref, int64_t bit, unsigned state) {
    if (bit < 0 || bit >= ref->width) return;
    ref->states[bit] = (unsigned char)state;
}

/* A new owner holding `ref`, which the caller keeps. */
static sv4_t ref_value(const ref_t* ref) {
    size_t limbs = (ref->width + 63u) / 64u;
    uint64_t* planes = (uint64_t*)calloc(3u * (limbs ? limbs : 1u), sizeof(uint64_t));
    CHECK(planes != NULL);
    uint64_t *bits = planes, *x = planes + limbs, *z = planes + 2u * limbs;
    for (uint32_t bit = 0; bit < ref->width; ++bit) {
        uint64_t mask = UINT64_C(1) << (bit % 64u);
        switch (ref->states[bit]) {
        case 1: bits[bit / 64u] |= mask; break;
        case 2: x[bit / 64u] |= mask; break;
        case 3: z[bit / 64u] |= mask; break;
        default: break;
        }
    }
    sv4_t value = sv4_from_limbs(bits, x, z, ref->width, ref->is_signed);
    free(planes);
    return value;
}

static ref_t ref_pattern(uint32_t width, unsigned seed) {
    ref_t ref = ref_new(width, (int8_t)(seed % 2), 0);
    uint32_t random = seed + 1;
    for (uint32_t bit = 0; bit < width; ++bit) {
        random = random * UINT32_C(1664525) + UINT32_C(1013904223);
        put(&ref, bit, random >> 30);
    }
    return ref;
}

static sv4_t pattern(uint32_t width, unsigned seed) {
    ref_t ref = ref_pattern(width, seed);
    sv4_t value = ref_value(&ref);
    ref_free(&ref);
    return value;
}

static void equal(sv4_t actual, sv4_t expected) {
    CHECK(actual.width == expected.width && actual.is_signed == expected.is_signed);
    size_t limbs = (actual.width + 63u) / 64u;
    for (size_t limb = 0; limb < limbs; ++limb) {
        CHECK(PROBE_BITS(actual, limb) == PROBE_BITS(expected, limb));
        CHECK(PROBE_X(actual, limb) == PROBE_X(expected, limb));
        CHECK(PROBE_Z(actual, limb) == PROBE_Z(expected, limb));
        CHECK(!(PROBE_X(actual, limb) & PROBE_Z(actual, limb)));
        CHECK(!(PROBE_BITS(actual, limb) & (PROBE_X(actual, limb) | PROBE_Z(actual, limb))));
    }
    if (actual.width % 64u) {
        uint64_t mask = UINT64_MAX << (actual.width % 64u);
        CHECK(!((PROBE_BITS(actual, limbs - 1) | PROBE_X(actual, limbs - 1) |
                 PROBE_Z(actual, limbs - 1)) & mask));
    }
    ++cases;
}

static void result(sv4_t actual, sv4_t expected) {
    equal(actual, expected);
    sv4_destroy(&actual);
    sv4_destroy(&expected);
}

/* Compare `actual` with the oracle `expected`, then release both. */
static void check(sv4_t actual, ref_t* expected) {
    result(actual, ref_value(expected));
    ref_free(expected);
}

static void reference_copy(ref_t* target, int64_t to, const ref_t* source,
                            int64_t from, uint32_t count) {
    for (uint32_t bit = 0; bit < count; ++bit)
        put(target, to + bit, digit(source, from + bit));
}

static void plan_case(sv4_t source, const ref_t* src, uint32_t from, uint32_t to,
                      uint32_t count) {
    sv4_select_plan_t plan = {source.width, source.width, from, to, count};
    ref_t expected = ref_new(source.width, 0, 2);
    reference_copy(&expected, to, src, from, count);
    check(sv4_select_plan_read(source, &plan), &expected);
    for (int reverse = 0; reverse < 2; ++reverse) {
        expected = ref_new(count, 0, 0);
        for (uint32_t bit = 0; bit < count; ++bit)
            put(&expected, bit, digit(src, reverse
                ? source.width - 1u - to - bit : to + bit));
        check(sv4_select_plan_slice(source, &plan, reverse), &expected);
    }
    sv4_t target = sv4_clone(&source);
    expected = ref_clone(src);
    reference_copy(&expected, from, src, to, count);
    sv4_select_plan_set(&target, &plan, target);
    check(target, &expected);
    expected = ref_pattern(source.width, 17);
    target = ref_value(&expected);
    reference_copy(&expected, from, src, to, count);
    sv4_select_plan_set(&target, &plan, source);
    check(target, &expected);
}

static void plans(sv4_t source) {
    ref_t src = ref_of(source);
    for (uint32_t from = 0; from <= 65 && from <= source.width; ++from) {
        for (uint32_t to = 0; to <= 65 && to <= source.width; ++to) {
            uint32_t max = from > to ? from : to;
            plan_case(source, &src, from, to, source.width - max);
        }
    }
    for (uint32_t boundary = 64; boundary < source.width; boundary += 64) {
        for (int delta = -2; delta <= 2; ++delta) {
            uint32_t from = (uint32_t)((int64_t)boundary + delta);
            if (from > source.width) continue;
            for (uint32_t to = 62; to <= 66; ++to) {
                uint32_t max = from > to ? from : to;
                if (max <= source.width)
                    plan_case(source, &src, from, to, source.width - max);
            }
        }
    }
    ref_free(&src);
}

static void concatenate(sv4_t source) {
    ref_t src = ref_of(source);
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        ref_t low_ref = ref_pattern(widths[i], 9);
        sv4_t low = ref_value(&low_ref);
        ref_t expected = ref_new(source.width + low.width, 0, 0);
        reference_copy(&expected, 0, &low_ref, 0, low.width);
        reference_copy(&expected, low.width, &src, 0, source.width);
        check(sv4_concat(source, low), &expected);
        sv4_destroy(&low);
        ref_free(&low_ref);
    }
    const uint32_t repeats[] = {0, 1, 2, 3, 5, 17, 129};
    for (size_t i = 0; i < sizeof(repeats) / sizeof(*repeats); ++i) {
        uint32_t times = repeats[i];
        ref_t expected = ref_new(source.width * times, 0, 0);
        for (uint32_t bit = 0; bit < expected.width; ++bit)
            put(&expected, bit, digit(&src, bit % source.width));
        check(sv4_repeat(source, times), &expected);
    }
    ref_free(&src);
}

static void streaming_and_shifts(sv4_t source) {
    ref_t src = ref_of(source);
    const uint32_t slices[] = {1, 2, 7, 31, 63, 64, 65, 127, 128, 129, 1023, 4096, UINT32_MAX};
    for (size_t i = 0; i < sizeof(slices) / sizeof(*slices); ++i) {
        uint32_t slice = slices[i];
        for (int reverse = 0; reverse < 2; ++reverse) {
            ref_t expected = ref_new(source.width, 0, 0);
            for (uint32_t bit = 0; bit < source.width; ++bit) {
                uint64_t end = ((uint64_t)bit / slice + 1) * slice;
                if (end > source.width) end = source.width;
                uint32_t other = reverse ? source.width - (uint32_t)end + bit % slice : bit;
                put(&expected, other, digit(&src, bit));
            }
            check(sv4_stream(source, slice, reverse), &expected);
        }
        sv4_t amount = sv4_from_u64(slice, 64, 0);
        for (int right = 0; right < 2; ++right) {
            for (int arithmetic = 0; arithmetic < 2; ++arithmetic) {
                ref_t expected = ref_new(source.width, source.is_signed, 0);
                for (uint32_t bit = 0; bit < source.width; ++bit) {
                    int64_t from = right ? (int64_t)bit + slice : (int64_t)bit - slice;
                    unsigned state = from >= 0 && from < source.width ? digit(&src, from) :
                        right && arithmetic && source.is_signed ? digit(&src, source.width - 1u) : 0;
                    put(&expected, bit, state);
                }
                check(right ? (arithmetic ? sv4_ashr(source, amount) : sv4_shr(source, amount))
                            : (arithmetic ? sv4_ashl(source, amount) : sv4_shl(source, amount)), &expected);
            }
        }
        sv4_destroy(&amount);
    }
    for (int state = 0; state < 4; ++state) {
        sv4_t signed_source = sv4_clone(&source);
        signed_source.is_signed = 1;
        probe_put_state(&signed_source, source.width - 1u, (unsigned)state);
        ref_t expected = ref_new(source.width + 65u, 1, 0);
        reference_copy(&expected, 0, &src, 0, source.width - 1u);
        for (uint32_t bit = source.width - 1u; bit < expected.width; ++bit)
            put(&expected, bit, (unsigned)state);
        check(sv4_resize(signed_source, expected.width, 1), &expected);
        sv4_destroy(&signed_source);
    }
    ref_free(&src);
}

static void selects(sv4_t source) {
    ref_t src = ref_of(source);
    ref_t rhs_pattern[sizeof(widths) / sizeof(*widths)][2];
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        rhs_pattern[i][0] = ref_pattern(widths[i] / 2u, 31);
        rhs_pattern[i][1] = ref_pattern(widths[i] / 2u, 19);
    }
    int64_t lows[] = {-(int64_t)source.width - 1, -129, -65, -64, -63, -7, -1,
                     0, 1, 7, 62, 63, 64, 65, 126, 127, 128, 129,
                     (int64_t)source.width - 1, source.width, (int64_t)source.width + 1};
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        uint32_t width = widths[i];
        for (size_t j = 0; j < sizeof(lows) / sizeof(*lows); ++j) {
            int64_t low = lows[j], high = low + width - 1;
            for (int reverse = 0; reverse < 2; ++reverse) {
                int64_t left = reverse ? low : high, right = reverse ? high : low;
                ref_t expected = ref_new(width, 0, 0);
                for (uint32_t bit = 0; bit < width; ++bit)
                    put(&expected, bit, digit(&src, reverse ? high - bit : low + bit));
                check(sv4_part_select(source, left, right), &expected);
                for (int alias = 0; alias < 2; ++alias) {
                    sv4_t target = sv4_clone(&source);
                    const ref_t* rhs_ref = alias ? &src : &rhs_pattern[i][0];
                    sv4_t rhs = alias ? source : ref_value(rhs_ref);
                    expected = ref_clone(&src);
                    for (uint32_t bit = 0; bit < width; ++bit) {
                        int64_t to = reverse ? high - bit : low + bit;
                        put(&expected, to, digit(rhs_ref, (int64_t)rhs.width - width + bit));
                    }
                    sv4_part_select_set(&target, left, right, alias ? target : rhs);
                    check(target, &expected);
                    if (!alias) sv4_destroy(&rhs);
                }
            }
            for (int negative = 0; negative < 2; ++negative) {
                int64_t base_number = negative ? high : low;
                sv4_t base = sv4_from_i64(base_number, 64);
                ref_t expected = ref_new(width, 0, 0);
                for (uint32_t bit = 0; bit < width; ++bit)
                    put(&expected, bit, negative && base_number < 0 ? 2 : digit(&src, low + bit));
                sv4_t expected_value = ref_value(&expected);
                ref_free(&expected);
                if (base_number >= 0) {
                    sv4_t unsigned_read =
                        sv4_idx_part_select(source, (uint64_t)base_number, width, negative);
                    equal(unsigned_read, expected_value);
                    sv4_destroy(&unsigned_read);
                }
                result(sv4_idx_part_select_value(source, base, width, negative), expected_value);
                for (int alias = 0; alias < 2; ++alias) {
                    sv4_t target = sv4_clone(&source);
                    const ref_t* rhs_ref = alias ? &src : &rhs_pattern[i][1];
                    sv4_t rhs = alias ? source : ref_value(rhs_ref);
                    expected = ref_clone(&src);
                    if (!negative || base_number >= 0)
                        reference_copy(&expected, low, rhs_ref, 0, width);
                    sv4_t target_unsigned = sv4_clone(&source);
                    expected_value = ref_value(&expected);
                    ref_free(&expected);
                    sv4_idx_part_select_set_value(&target, base, width, negative, alias ? target : rhs);
                    equal(target, expected_value);
                    sv4_destroy(&target);
                    if (base_number >= 0) {
                        sv4_idx_part_select_set(&target_unsigned, (uint64_t)base_number, width,
                                               negative, alias ? target_unsigned : rhs);
                        equal(target_unsigned, expected_value);
                    }
                    sv4_destroy(&target_unsigned);
                    sv4_destroy(&expected_value);
                    if (!alias) sv4_destroy(&rhs);
                }
                sv4_destroy(&base);
            }
        }
    }
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        ref_free(&rhs_pattern[i][0]);
        ref_free(&rhs_pattern[i][1]);
    }
    ref_free(&src);
}

static void extremes(void) {
    sv4_t source = pattern(129, 3);
    const uint64_t indices[] = {INT64_MAX, (uint64_t)INT64_MAX + 1u, UINT64_MAX - 63u, UINT64_MAX};
    for (size_t i = 0; i < sizeof(indices) / sizeof(*indices); ++i) {
        for (int neg = 0; neg < 2; ++neg) {
            result(sv4_idx_part_select(source, indices[i], 65, neg), sv4_x(65, 0));
            sv4_t target = sv4_clone(&source);
            sv4_idx_part_select_set(&target, indices[i], 65, neg, target);
            result(target, sv4_clone(&source));
        }
    }
    sv4_t bases[] = {sv4_from_i64(INT64_MIN, 64), sv4_from_i64(INT64_MAX, 64),
                     sv4_x(129, 1), sv4_fill(3, 129, 0), sv4_from_u64(1, 129, 0)};
    probe_set_bits(&bases[4], 1, 1);
    for (size_t i = 0; i < sizeof(bases) / sizeof(*bases); ++i) {
        for (int neg = 0; neg < 2; ++neg) {
            result(sv4_idx_part_select_value(source, bases[i], 65, neg), sv4_x(65, 0));
            sv4_t target = sv4_clone(&source);
            sv4_idx_part_select_set_value(&target, bases[i], 65, neg, target);
            result(target, sv4_clone(&source));
        }
        sv4_destroy(&bases[i]);
    }
    const int64_t ends[] = {INT64_MIN, INT64_MIN + 64, INT64_MAX - 64, INT64_MAX};
    for (size_t i = 0; i < sizeof(ends) / sizeof(*ends); ++i) {
        result(sv4_part_select(source, ends[i], ends[i]), sv4_x(1, 0));
        sv4_t target = sv4_clone(&source);
        sv4_part_select_set(&target, ends[i], ends[i], target);
        result(target, sv4_clone(&source));
    }
    sv4_t empty = SV4_EMPTY;
    result(sv4_concat(empty, empty), sv4_zero(0, 0));
    result(sv4_repeat(empty, UINT64_MAX), sv4_zero(0, 0));
    result(sv4_idx_part_select(source, UINT64_MAX, 0, 1), sv4_zero(0, 0));
    sv4_destroy(&source);

    uint64_t bits[] = {UINT64_MAX, UINT64_MAX, 1};
    uint64_t x[] = {UINT64_C(0xaaaaaaaaaaaaaaaa), 0, 1};
    uint64_t z[] = {UINT64_C(0xcccccccccccccccc), UINT64_MAX, 1};
    source = sv4_from_limbs(bits, x, z, 129, 0);
    ref_t src = ref_of(source);
    ref_t expected = ref_new(258, 0, 0);
    reference_copy(&expected, 0, &src, 0, 129);
    reference_copy(&expected, 129, &src, 0, 129);
    check(sv4_concat(source, source), &expected);
    expected = ref_new(65, 0, 0);
    reference_copy(&expected, 0, &src, 63, 65);
    check(sv4_part_select(source, 127, 63), &expected);
    ref_free(&src);
    sv4_destroy(&source);
}

int main(void) {
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        sv4_t source = pattern(widths[i], (unsigned)i + 3u);
        plans(source);
        concatenate(source);
        streaming_and_shifts(source);
        selects(source);
        sv4_destroy(&source);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    extremes();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    printf("word copy oracle: %zu comparisons, zero live owners\n", cases);
    return 0;
}
