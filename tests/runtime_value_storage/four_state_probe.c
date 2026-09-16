/* Independent per-bit truth tables; do not reuse the limb arithmetic as an oracle. */
#include "probe.h"
#include <string.h>

static uint64_t seed = UINT64_C(0x4c4c475037);
static size_t checked_results;
static uint64_t next_random(void) {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}

static unsigned state(sv4_t value, uint32_t bit) {
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    size_t limb = bit / 64u;
    if (value.x[limb] & mask) return 2;
    if (value.z[limb] & mask) return 3;
    return (value.bits[limb] & mask) != 0;
}

static void put(sv4_t* value, uint32_t bit, unsigned digit) {
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    size_t limb = bit / 64u;
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (digit == 1) value->bits[limb] |= mask;
    if (digit == 2) value->x[limb] |= mask;
    if (digit == 3) value->z[limb] |= mask;
}

static unsigned extended(sv4_t value, uint32_t bit, int is_signed) {
    if (bit < value.width) return state(value, bit);
    return is_signed && value.width ? state(value, value.width - 1u) : 0;
}

static unsigned truth(sv4_t value) {
    unsigned result = 0;
    for (uint32_t bit = 0; bit < value.width; ++bit) {
        unsigned digit = state(value, bit);
        if (digit == 1) return 1;
        if (digit > 1) result = 2;
    }
    return result;
}

static const unsigned and_table[4][4] = {
    {0, 0, 0, 0}, {0, 1, 2, 2}, {0, 2, 2, 2}, {0, 2, 2, 2}
};
static const unsigned or_table[4][4] = {
    {0, 1, 2, 2}, {1, 1, 1, 1}, {2, 1, 2, 2}, {2, 1, 2, 2}
};
static const unsigned xor_table[4][4] = {
    {0, 1, 2, 2}, {1, 0, 2, 2}, {2, 2, 2, 2}, {2, 2, 2, 2}
};

static void check_shape(sv4_t result, uint32_t width, int is_signed,
                        sv4_t left, sv4_t right) {
    CHECK(result.width == width && result.is_signed == is_signed);
    CHECK(!result.bits || (result.bits != left.bits && result.bits != right.bits));
    CHECK(sv4_bytes(&result) == ((size_t)width + 63u) / 64u * 24u);
    if (!width) {
        CHECK(!result.bits && !result.x && !result.z);
    } else if (width % 64u) {
        size_t last = (width - 1u) / 64u;
        uint64_t unused = UINT64_MAX << (width % 64u);
        CHECK(((result.bits[last] | result.x[last] | result.z[last]) & unused) == 0);
    }
    ++checked_results;
}

static void check_scalar(sv4_t result, unsigned expected, sv4_t left, sv4_t right) {
    check_shape(result, 1, 0, left, right);
    CHECK(state(result, 0) == expected);
    sv4_destroy(&result);
}

static void check_pair(sv4_t left, sv4_t right) {
    sv4_t before_left = sv4_clone(&left);
    sv4_t before_right = sv4_clone(&right);
    size_t baseline = value_test_live();
    uint32_t width = left.width > right.width ? left.width : right.width;
    int is_signed = left.is_signed && right.is_signed;
    typedef sv4_t (*binary_fn)(sv4_t, sv4_t);
    const binary_fn functions[] = {sv4_and, sv4_or, sv4_xor, sv4_xnor};
    for (unsigned op = 0; op < 4; ++op) {
        sv4_t result = functions[op](left, right);
        check_shape(result, width, is_signed, left, right);
        for (uint32_t bit = 0; bit < width; ++bit) {
            unsigned a = extended(left, bit, is_signed);
            unsigned b = extended(right, bit, is_signed);
            unsigned expected = op == 0 ? and_table[a][b] :
                                op == 1 ? or_table[a][b] : xor_table[a][b];
            if (op == 3 && expected < 2) expected ^= 1u;
            CHECK(state(result, bit) == expected);
        }
        sv4_destroy(&result);
    }
    unsigned a = truth(left), b = truth(right);
    unsigned not_a = a < 2 ? a ^ 1u : 2;
    check_scalar(sv4_logand(left, right), and_table[a][b], left, right);
    check_scalar(sv4_logor(left, right), or_table[a][b], left, right);
    check_scalar(sv4_logimpl(left, right), or_table[not_a][b], left, right);
    check_scalar(sv4_logequiv(left, right), a == 2 || b == 2 ? 2 : a == b, left, right);
    check_scalar(sv4_lognot(left), not_a, left, right);

    unsigned equal = 1, case_equal = 1, wild_equal = 1;
    for (uint32_t bit = 0; bit < width; ++bit) {
        unsigned x = extended(left, bit, is_signed);
        unsigned y = extended(right, bit, is_signed);
        if (x != y) case_equal = 0;
        if (x < 2 && y < 2 && x != y) equal = 0;
        else if (equal && (x > 1 || y > 1)) equal = 2;
        if (y > 1) continue;
        if (x < 2 && x != y) wild_equal = 0;
        else if (wild_equal && x > 1) wild_equal = 2;
    }
    check_scalar(sv4_eq(left, right), equal, left, right);
    check_scalar(sv4_neq(left, right), equal == 2 ? 2 : equal ^ 1u, left, right);
    check_scalar(sv4_case_eq(left, right), case_equal, left, right);
    check_scalar(sv4_case_neq(left, right), case_equal ^ 1u, left, right);
    check_scalar(sv4_wild_eq(left, right), wild_equal, left, right);
    check_scalar(sv4_wild_neq(left, right), wild_equal == 2 ? 2 : wild_equal ^ 1u, left, right);

    for (unsigned digit = 0; digit < 5; ++digit) {
        sv4_t selector = sv4_zero(2, 0);
        put(&selector, 0, digit < 4 ? digit : 1);
        if (digit == 4) put(&selector, 1, 2); /* known one dominates an X */
        sv4_t result = sv4_mux(selector, left, right);
        check_shape(result, width, is_signed, left, right);
        for (uint32_t bit = 0; bit < width; ++bit) {
            unsigned x = extended(left, bit, is_signed), y = extended(right, bit, is_signed);
            unsigned expected = digit == 0 ? y : (digit == 1 || digit == 4) ? x : x == y ? x : 2;
            CHECK(state(result, bit) == expected);
        }
        sv4_destroy(&result);
        sv4_destroy(&selector);
    }
    for (int sign = 0; sign < 2; ++sign) {
        for (int cast = 0; cast < 2; ++cast) {
            sv4_t result = cast ? sv4_cast(left, right.width, (int8_t)sign) :
                                  sv4_resize(left, right.width, (int8_t)sign);
            check_shape(result, right.width, sign, left, right);
            for (uint32_t bit = 0; bit < right.width; ++bit)
                CHECK(state(result, bit) == extended(left, bit, cast ? left.is_signed : sign));
            sv4_destroy(&result);
        }
    }
    CHECK(sv4_same(left, before_left) && sv4_same(right, before_right));
    CHECK(value_test_live() == baseline);
    sv4_destroy(&before_left);
    sv4_destroy(&before_right);
}

static void check_selects(sv4_t source) {
    const int64_t bases[] = {INT64_MIN, -2, -1, 0, 1, 63, 64, INT64_MAX};
    for (size_t j = 0; j < sizeof(bases) / sizeof(*bases); ++j) {
        for (int neg = 0; neg < 2; ++neg) {
            sv4_t base = sv4_from_i64(bases[j], 65);
            sv4_t result = sv4_idx_part_select_value(source, base, 65, neg);
            check_shape(result, 65, 0, source, base);
            for (uint32_t bit = 0; bit < 65; ++bit) {
                int64_t offset = neg ? (int64_t)bit - 64 : (int64_t)bit;
                int overflow = (offset > 0 && bases[j] > INT64_MAX - offset) ||
                               (offset < 0 && bases[j] < INT64_MIN - offset);
                int64_t index = overflow ? -1 : bases[j] + offset;
                unsigned expected = index >= 0 && (uint64_t)index < source.width ?
                                    state(source, (uint32_t)index) : 2;
                CHECK(state(result, bit) == expected);
            }
            sv4_destroy(&result);
            sv4_destroy(&base);
        }
    }
    /* The source aliases the destination: expected writes always read the old copy. */
    for (uint32_t base = 0; base < 66; base += 13) {
        sv4_t target = sv4_clone(&source);
        sv4_idx_part_select_set(&target, base, source.width, 0, target);
        check_shape(target, source.width, source.is_signed, source, source);
        for (uint32_t bit = 0; bit < source.width; ++bit)
            CHECK(state(target, bit) == state(source, bit < base ? bit : bit - base));
        sv4_destroy(&target);
    }
}

int main(void) {
    /* Exhaustive one-bit pairs, including every signedness combination. */
    for (unsigned a = 0; a < 4; ++a) {
        for (unsigned b = 0; b < 4; ++b) {
            for (unsigned signs = 0; signs < 4; ++signs) {
                sv4_t left = sv4_zero(1, (int8_t)(signs & 1u));
                sv4_t right = sv4_zero(1, (int8_t)(signs >> 1u));
                put(&left, 0, a); put(&right, 0, b);
                check_pair(left, right);
                sv4_destroy(&left); sv4_destroy(&right);
            }
        }
    }
    const uint32_t widths[] = {0, 1, 2, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 511, 1023, 4097};
    for (unsigned cycle = 0; cycle < 512; ++cycle) {
        sv4_t left = sv4_zero(widths[next_random() % 16u], (int8_t)(cycle & 1u));
        sv4_t right = sv4_zero(widths[next_random() % 16u], (int8_t)((cycle >> 1u) & 1u));
        for (uint32_t bit = 0; bit < left.width; ++bit) put(&left, bit, (unsigned)(next_random() % 4u));
        for (uint32_t bit = 0; bit < right.width; ++bit) put(&right, bit, (unsigned)(next_random() % 4u));
        check_pair(left, right);
        check_selects(left);
        sv4_destroy(&left); sv4_destroy(&right);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    printf("independent four-state oracle: %zu value results; zero live allocations\n", checked_results);
    return 0;
}
