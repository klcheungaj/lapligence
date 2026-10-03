#include "probe.h"
#include <limits.h>
#include <string.h>

static size_t cases;
static const uint32_t widths[] = {1, 7, 63, 64, 65, 127, 128, 129, 1023, 4096};

static unsigned digit(sv4_t value, int64_t bit) {
    if (bit < 0 || bit >= value.width) return 2;
    return probe_state(value, (uint64_t)bit);
}

static void put(sv4_t* value, int64_t bit, unsigned state) {
    if (bit < 0 || bit >= value->width) return;
    probe_put_state(value, (uint64_t)bit, state);
}

static sv4_t pattern(uint32_t width, unsigned seed) {
    sv4_t value = sv4_zero(width, (int8_t)(seed % 2));
    uint32_t random = seed + 1;
    for (uint32_t bit = 0; bit < width; ++bit) {
        random = random * UINT32_C(1664525) + UINT32_C(1013904223);
        put(&value, bit, random >> 30);
    }
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

static void reference_copy(sv4_t* target, int64_t to, sv4_t source,
                            int64_t from, uint32_t count) {
    for (uint32_t bit = 0; bit < count; ++bit)
        put(target, to + bit, digit(source, from + bit));
}

static void plan_case(sv4_t source, uint32_t from, uint32_t to, uint32_t count) {
    sv4_select_plan_t plan = {source.width, source.width, from, to, count};
    sv4_t expected = sv4_x(source.width, 0);
    reference_copy(&expected, to, source, from, count);
    result(sv4_select_plan_read(source, &plan), expected);
    for (int reverse = 0; reverse < 2; ++reverse) {
        expected = sv4_zero(count, 0);
        for (uint32_t bit = 0; bit < count; ++bit)
            put(&expected, bit, digit(source, reverse
                ? source.width - 1u - to - bit : to + bit));
        result(sv4_select_plan_slice(source, &plan, reverse), expected);
    }
    sv4_t target = sv4_clone(&source);
    expected = sv4_clone(&source);
    reference_copy(&expected, from, source, to, count);
    sv4_select_plan_set(&target, &plan, target);
    result(target, expected);
    target = pattern(source.width, 17);
    expected = sv4_clone(&target);
    reference_copy(&expected, from, source, to, count);
    sv4_select_plan_set(&target, &plan, source);
    result(target, expected);
}

static void plans(sv4_t source) {
    for (uint32_t from = 0; from <= 65 && from <= source.width; ++from) {
        for (uint32_t to = 0; to <= 65 && to <= source.width; ++to) {
            uint32_t max = from > to ? from : to;
            plan_case(source, from, to, source.width - max);
        }
    }
    for (uint32_t boundary = 64; boundary < source.width; boundary += 64) {
        for (int delta = -2; delta <= 2; ++delta) {
            uint32_t from = (uint32_t)((int64_t)boundary + delta);
            if (from > source.width) continue;
            for (uint32_t to = 62; to <= 66; ++to) {
                uint32_t max = from > to ? from : to;
                if (max <= source.width)
                    plan_case(source, from, to, source.width - max);
            }
        }
    }
}

static void concatenate(sv4_t source) {
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        sv4_t low = pattern(widths[i], 9);
        sv4_t expected = sv4_zero(source.width + low.width, 0);
        reference_copy(&expected, 0, low, 0, low.width);
        reference_copy(&expected, low.width, source, 0, source.width);
        result(sv4_concat(source, low), expected);
        sv4_destroy(&low);
    }
    const uint32_t repeats[] = {0, 1, 2, 3, 5, 17, 129};
    for (size_t i = 0; i < sizeof(repeats) / sizeof(*repeats); ++i) {
        uint32_t times = repeats[i];
        sv4_t expected = sv4_zero(source.width * times, 0);
        for (uint32_t bit = 0; bit < expected.width; ++bit)
            put(&expected, bit, digit(source, bit % source.width));
        result(sv4_repeat(source, times), expected);
    }
}

static void streaming_and_shifts(sv4_t source) {
    const uint32_t slices[] = {1, 2, 7, 31, 63, 64, 65, 127, 128, 129, 1023, 4096, UINT32_MAX};
    for (size_t i = 0; i < sizeof(slices) / sizeof(*slices); ++i) {
        uint32_t slice = slices[i];
        for (int reverse = 0; reverse < 2; ++reverse) {
            for (int inverse = 0; inverse < 2; ++inverse) {
                sv4_t expected = sv4_zero(source.width, 0);
                for (uint32_t bit = 0; bit < source.width; ++bit) {
                    uint64_t end = ((uint64_t)bit / slice + 1) * slice;
                    if (end > source.width) end = source.width;
                    uint32_t other = reverse ? source.width - (uint32_t)end + bit % slice : bit;
                    if (inverse) put(&expected, bit, digit(source, other));
                    else put(&expected, other, digit(source, bit));
                }
                result(inverse ? sv4_unstream(source, slice, reverse)
                               : sv4_stream(source, slice, reverse), expected);
            }
        }
        sv4_t amount = sv4_from_u64(slice, 64, 0);
        for (int right = 0; right < 2; ++right) {
            for (int arithmetic = 0; arithmetic < 2; ++arithmetic) {
                sv4_t expected = sv4_zero(source.width, source.is_signed);
                for (uint32_t bit = 0; bit < source.width; ++bit) {
                    int64_t from = right ? (int64_t)bit + slice : (int64_t)bit - slice;
                    unsigned state = from >= 0 && from < source.width ? digit(source, from) :
                        right && arithmetic && source.is_signed ? digit(source, source.width - 1u) : 0;
                    put(&expected, bit, state);
                }
                result(right ? (arithmetic ? sv4_ashr(source, amount) : sv4_shr(source, amount))
                             : (arithmetic ? sv4_ashl(source, amount) : sv4_shl(source, amount)), expected);
            }
        }
        sv4_destroy(&amount);
    }
    for (int state = 0; state < 4; ++state) {
        sv4_t signed_source = sv4_clone(&source);
        signed_source.is_signed = 1;
        put(&signed_source, source.width - 1u, (unsigned)state);
        sv4_t expected = sv4_zero(source.width + 65u, 1);
        reference_copy(&expected, 0, signed_source, 0, source.width);
        for (uint32_t bit = source.width; bit < expected.width; ++bit)
            put(&expected, bit, (unsigned)state);
        result(sv4_resize(signed_source, expected.width, 1), expected);
        sv4_destroy(&signed_source);
    }
}

static void selects(sv4_t source) {
    int64_t lows[] = {-(int64_t)source.width - 1, -129, -65, -64, -63, -7, -1,
                     0, 1, 7, 62, 63, 64, 65, 126, 127, 128, 129,
                     (int64_t)source.width - 1, source.width, (int64_t)source.width + 1};
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        uint32_t width = widths[i];
        for (size_t j = 0; j < sizeof(lows) / sizeof(*lows); ++j) {
            int64_t low = lows[j], high = low + width - 1;
            for (int reverse = 0; reverse < 2; ++reverse) {
                int64_t left = reverse ? low : high, right = reverse ? high : low;
                sv4_t expected = sv4_zero(width, 0);
                for (uint32_t bit = 0; bit < width; ++bit)
                    put(&expected, bit, digit(source, reverse ? high - bit : low + bit));
                result(sv4_part_select(source, left, right), expected);
                for (int alias = 0; alias < 2; ++alias) {
                    sv4_t target = sv4_clone(&source);
                    sv4_t rhs = alias ? source : pattern(width / 2u, 31);
                    expected = sv4_clone(&source);
                    for (uint32_t bit = 0; bit < width; ++bit) {
                        int64_t to = reverse ? high - bit : low + bit;
                        put(&expected, to, digit(rhs, (int64_t)rhs.width - width + bit));
                    }
                    sv4_part_select_set(&target, left, right, alias ? target : rhs);
                    result(target, expected);
                    if (!alias) sv4_destroy(&rhs);
                }
            }
            for (int negative = 0; negative < 2; ++negative) {
                int64_t base_number = negative ? high : low;
                sv4_t base = sv4_from_i64(base_number, 64);
                sv4_t expected = sv4_zero(width, 0);
                for (uint32_t bit = 0; bit < width; ++bit)
                    put(&expected, bit, negative && base_number < 0 ? 2 : digit(source, low + bit));
                sv4_t expected_unsigned = sv4_clone(&expected);
                result(sv4_idx_part_select_value(source, base, width, negative), expected);
                if (base_number >= 0)
                    result(sv4_idx_part_select(source, (uint64_t)base_number, width, negative), expected_unsigned);
                else sv4_destroy(&expected_unsigned);
                for (int alias = 0; alias < 2; ++alias) {
                    sv4_t target = sv4_clone(&source);
                    sv4_t rhs = alias ? source : pattern(width / 2u, 19);
                    expected = sv4_clone(&source);
                    if (!negative || base_number >= 0)
                        reference_copy(&expected, low, rhs, 0, width);
                    sv4_t target_unsigned = sv4_clone(&source);
                    sv4_t expected_unsigned_write = sv4_clone(&expected);
                    sv4_idx_part_select_set_value(&target, base, width, negative, alias ? target : rhs);
                    result(target, expected);
                    if (base_number >= 0) {
                        sv4_idx_part_select_set(&target_unsigned, (uint64_t)base_number, width,
                                               negative, alias ? target_unsigned : rhs);
                        equal(target_unsigned, expected_unsigned_write);
                    }
                    sv4_destroy(&target_unsigned);
                    sv4_destroy(&expected_unsigned_write);
                    if (!alias) sv4_destroy(&rhs);
                }
                sv4_destroy(&base);
            }
        }
    }
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
    sv4_t expected = sv4_zero(258, 0);
    reference_copy(&expected, 0, source, 0, 129);
    reference_copy(&expected, 129, source, 0, 129);
    result(sv4_concat(source, source), expected);
    expected = sv4_zero(65, 0);
    reference_copy(&expected, 0, source, 63, 65);
    result(sv4_part_select(source, 127, 63), expected);
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
