/* Independent array-element oracle and exact-width ownership checks. */
#include "probe.h"
#include <string.h>

static unsigned state(sv4_t value, uint32_t bit) { return probe_state(value, bit); }

static void put(sv4_t* value, uint32_t bit, unsigned digit) {
    probe_put_state(value, bit, digit);
}

static void check_merge(sv4_t left, sv4_t right, sv4_t fallback) {
    sv4_t left_before = sv4_clone(&left);
    sv4_t right_before = sv4_clone(&right);
    sv4_t default_before = sv4_clone(&fallback);
    size_t live = value_test_live();
    size_t allocations = value_test_allocations();
    sv4_t result = sv4_array_conditional_merge(left, right, fallback);
    CHECK(value_test_live() == live + probe_owner_allocations(left.width));
    CHECK(value_test_allocations() == allocations + probe_owner_allocations(left.width));
    CHECK(result.width == left.width && result.is_signed == 0);
    CHECK(probe_distinct(&result, left) && probe_distinct(&result, right) &&
          probe_distinct(&result, fallback));
    CHECK(sv4_bytes(&result) == probe_payload_bytes(left.width, sv4_is_unknown(result)));
    for (uint32_t start = 0; start < left.width; start += fallback.width) {
        unsigned equal = 1;
        for (uint32_t j = 0; j < fallback.width; ++j) {
            unsigned a = state(left_before, start + j);
            unsigned b = state(right_before, start + j);
            equal &= a < 2 && b < 2 && a == b;
        }
        for (uint32_t j = 0; j < fallback.width; ++j) {
            unsigned expected = equal ? state(left_before, start + j) : state(default_before, j);
            CHECK(state(result, start + j) == expected);
        }
    }
    if (result.width % 64u) {
        uint64_t outside = UINT64_MAX << (result.width % 64u);
        size_t last = (result.width - 1u) / 64u;
        CHECK(((PROBE_BITS(result, last) | PROBE_X(result, last) | PROBE_Z(result, last)) & outside) == 0);
    }
    CHECK(probe_same(left, left_before));
    CHECK(probe_same(right, right_before));
    CHECK(probe_same(fallback, default_before));
    sv4_destroy(&result);
    CHECK(value_test_live() == live);
    sv4_destroy(&left_before);
    sv4_destroy(&right_before);
    sv4_destroy(&default_before);
}

static void check_packed_control(void) {
    sv4_t left = sv4_from_u64(0xa5, 8, 0);
    sv4_t right = sv4_from_u64(0xa6, 8, 0);
    sv4_t selector = sv4_x(1, 0);
    sv4_t fallback = sv4_x(8, 0);
    sv4_t packed = sv4_mux(selector, left, right);
    sv4_t array = sv4_array_conditional_merge(left, right, fallback);
    CHECK(PROBE_BITS(packed, 0) == 0xa4 && PROBE_X(packed, 0) == 3 && PROBE_Z(packed, 0) == 0);
    CHECK(PROBE_BITS(array, 0) == 0 && PROBE_X(array, 0) == 0xff && PROBE_Z(array, 0) == 0);
    sv4_destroy(&left);
    sv4_destroy(&right);
    sv4_destroy(&selector);
    sv4_destroy(&fallback);
    sv4_destroy(&packed);
    sv4_destroy(&array);
}

static void check_width(uint32_t stride, uint32_t count) {
    uint32_t width = stride * count;
    sv4_t left = sv4_zero(width, 0);
    sv4_t right = sv4_zero(width, 0);
    sv4_t fallback = sv4_x(stride, 0);
    for (uint32_t bit = 0; bit < width; ++bit) {
        put(&left, bit, (bit * 17u + 3u) % 2u);
        put(&right, bit, state(left, bit));
    }
    check_merge(left, right, fallback);
    check_merge(left, left, fallback); /* Borrowed operands may alias. */
    /* Change one bit in one element, including above bit 63. */
    put(&right, width - 1u, state(right, width - 1u) ^ 1u);
    check_merge(left, right, fallback);
    /* Nested element defaults can mix two-state zero and four-state X. */
    for (uint32_t bit = 0; bit < stride; ++bit)
        put(&fallback, bit, bit % 3u ? 2u : 0u);
    check_merge(left, right, fallback);
    for (uint32_t bit = 0; bit < width; ++bit) {
        if (bit % 13u == 0) {
            put(&left, bit, 2);
            put(&right, bit, 2);
        } else if (bit % 17u == 0) {
            put(&left, bit, 3);
            put(&right, bit, 3);
        }
    }
    check_merge(left, right, fallback); /* Case-equal X/Z is not logical true. */
    sv4_replace(&fallback, sv4_zero(stride, 0));
    check_merge(left, right, fallback);
    sv4_destroy(&left);
    sv4_destroy(&right);
    sv4_destroy(&fallback);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
}

static void check_replacement_plateau(void) {
    sv4_t left = sv4_from_u64(0xa55a, 16, 0);
    sv4_t right = sv4_from_u64(0xa65a, 16, 0);
    sv4_t fallback = sv4_x(8, 0);
    sv4_t result = SV4_EMPTY;
    for (unsigned iteration = 0; iteration < 10000; ++iteration) {
        sv4_replace(&result, sv4_array_conditional_merge(left, right, fallback));
        CHECK(value_test_live() ==
              3 * probe_owner_allocations(16) + probe_owner_allocations(8));
        CHECK(PROBE_BITS(result, 0) == 0x5a && PROBE_X(result, 0) == 0xff00);
    }
    sv4_destroy(&left);
    sv4_destroy(&right);
    sv4_destroy(&fallback);
    CHECK(PROBE_BITS(result, 0) == 0x5a && PROBE_X(result, 0) == 0xff00);
    sv4_destroy(&result);
}

int main(int argc, char** argv) {
    if (argc == 2) {
        sv4_t left = sv4_zero(strcmp(argv[1], "empty") == 0 ? 0 : 16, 0);
        sv4_t right = sv4_zero(strcmp(argv[1], "unequal") == 0 ? 8 : left.width, 0);
        uint32_t stride = strcmp(argv[1], "zero") == 0 ? 0 :
                          strcmp(argv[1], "remainder") == 0 ? 7 : 8;
        sv4_t fallback = sv4_zero(stride, 0);
        sv4_t result = sv4_array_conditional_merge(left, right, fallback);
        sv4_destroy(&result);
        sv4_destroy(&left);
        sv4_destroy(&right);
        sv4_destroy(&fallback);
        return 3; /* A malformed shape must fail with the runtime diagnostic. */
    }
    check_packed_control();
    const uint32_t widths[] = {1, 7, 8, 31, 32, 33, 63, 64, 65, 67, 127, 129, 257};
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
        check_width(widths[i], 1);
        check_width(widths[i], 3);
    }
    check_width(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 1);
    check_width(65537u, 3);
    check_replacement_plateau();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("array conditional merge passed");
    return 0;
}
