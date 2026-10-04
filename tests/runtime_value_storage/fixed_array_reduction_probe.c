/* Handwritten counterpart of owned/fixed_array_reductions.rs, not emitted HDL.
 * Exercises borrowed cell/payload reads, typed maps, first-element seeding and
 * exact-width per-iteration owners against the production value operations. */
#include "probe.h"
#include <string.h>

static sv4_t combine(unsigned operation, sv4_t left, sv4_t right) {
    switch (operation) {
        case 0: return sv4_add(left, right);
        case 1: return sv4_mul(left, right);
        case 2: return sv4_and(left, right);
        case 3: return sv4_or(left, right);
        case 4: return sv4_xor(left, right);
        default: CHECK(0); return sv4_x(1, 0);
    }
}

static sv4_t fold(const sv4_t* cells, const sv4_t* payload, uint64_t count,
                  uint32_t element_width, uint32_t result_width, int8_t is_signed,
                  unsigned operation, int map_kind, int32_t left, int descending) {
    CHECK(count != 0 && (cells != NULL) != (payload != NULL));
    sv4_t result = SV4_EMPTY;
    for (uint64_t ordinal = 0; ordinal < count; ++ordinal) {
        sv4_t item = cells ? sv4_clone(&cells[ordinal]) :
            sv4_part_select(*payload,
                (int64_t)((count - ordinal) * element_width - 1),
                (int64_t)((count - ordinal - 1) * element_width));
        sv4_replace(&item, sv4_cast(item, element_width, is_signed));
        sv4_t index = sv4_from_u64((uint64_t)((int64_t)left +
            (descending ? -(int64_t)ordinal : (int64_t)ordinal)), 32, 1);
        sv4_t mapped = sv4_clone(&item);
        if (map_kind == 1) {
            sv4_replace(&mapped, sv4_cast(mapped, result_width, is_signed));
            sv4_replace(&mapped, sv4_to_two_state(mapped));
        } else if (map_kind == 2) {
            sv4_assign(&mapped, index);
        } else if (map_kind == 3) {
            sv4_replace(&mapped, sv4_fill(1, 1, 0));
        }
        sv4_replace(&mapped, sv4_cast(mapped, result_width, is_signed));
        if (ordinal == 0) sv4_assign(&result, mapped);
        else sv4_replace(&result, combine(operation, result, mapped));
        sv4_destroy(&mapped);
        sv4_destroy(&index);
        sv4_destroy(&item);
    }
    CHECK(result.width == result_width && result.is_signed == is_signed);
    return result;
}

static void values(void) {
    sv4_t cells[2] = {sv4_from_u64(200, 8, 0), sv4_from_u64(56, 8, 0)};
    expect_number(fold(cells, NULL, 2, 8, 8, 0, 0, 0, 0, 0), 0);
    expect_number(fold(cells, NULL, 2, 8, 32, 0, 0, 1, 0, 0), 256);
    sv4_replace(&cells[0], sv4_from_u64(2, 8, 0));
    sv4_replace(&cells[1], sv4_from_u64(3, 8, 0));
    const uint64_t expected[5] = {5, 6, 2, 3, 1};
    for (unsigned op = 0; op < 5; ++op)
        expect_number(fold(cells, NULL, 2, 8, 8, 0, op, 0, 0, 0), expected[op]);
    expect_number(fold(cells, NULL, 2, 8, 32, 1, 0, 2, -2, 0), UINT32_C(0xfffffffd));
    expect_number(fold(cells, NULL, 2, 8, 32, 1, 0, 2, 3, 1), 5);
    expect_number(fold(cells, NULL, 1, 8, 32, 1, 0, 2, INT32_MIN, 0), UINT32_C(0x80000000));
    expect_number(fold(cells, NULL, 1, 8, 32, 1, 0, 2, INT32_MAX, 1), UINT32_C(0x7fffffff));
    expect_number(fold(cells, NULL, 2, 8, 1, 0, 0, 3, 0, 0), 0);
    sv4_replace(&cells[0], sv4_from_u64(0xfd, 8, 1));
    sv4_replace(&cells[1], sv4_from_u64(0xfc, 8, 1));
    expect_number(fold(cells, NULL, 2, 8, 8, 1, 0, 0, 0, 0), 0xf9);
    expect_number(fold(cells, NULL, 2, 8, 64, 1, 0, 1, 0, 0), UINT64_C(0xfffffffffffffff9));
    sv4_replace(&cells[0], sv4_fill(3, 8, 0));
    for (unsigned op = 0; op < 5; ++op) {
        sv4_t result = fold(cells, NULL, 1, 8, 8, 0, op, 0, 0, 0);
        CHECK(PROBE_BITS(result, 0) == 0 && PROBE_X(result, 0) == 0 && PROBE_Z(result, 0) == 0xff);
        sv4_destroy(&result);
    }
    sv4_replace(&cells[1], sv4_zero(8, 0));
    expect_number(fold(cells, NULL, 2, 8, 8, 0, 2, 0, 0, 0), 0);
    expect_number(fold(cells, NULL, 2, 8, 32, 1, 0, 1, 0, 0), 0);
    sv4_t result = fold(cells, NULL, 2, 8, 8, 0, 1, 0, 0, 0);
    CHECK(PROBE_X(result, 0) == 0xff);
    sv4_destroy(&result);
    sv4_destroy(&cells[0]); sv4_destroy(&cells[1]);

    sv4_t payload = sv4_from_u64(0x050a0f14, 32, 0);
    sv4_t rows[2] = {SV4_EMPTY, SV4_EMPTY};
    for (uint64_t row = 0; row < 2; ++row) {
        sv4_t slice = sv4_part_select(payload, (int64_t)((2 - row) * 16 - 1),
                                     (int64_t)((1 - row) * 16));
        rows[row] = fold(NULL, &slice, 2, 8, 32, 1, 0, 1, -1, 0);
        sv4_destroy(&slice);
    }
    expect_number(fold(rows, NULL, 2, 32, 32, 1, 0, 0, 1, 1), 50);
    CHECK(PROBE_BITS(payload, 0) == UINT64_C(0x050a0f14));
    sv4_destroy(&rows[0]); sv4_destroy(&rows[1]); sv4_destroy(&payload);

    for (uint32_t width = 65; width <= 129; width += 64) {
        sv4_t wide[2] = {sv4_fill(1, width, 0), sv4_from_u64(2, width, 0)};
        expect_number(fold(wide, NULL, 2, width, width, 0, 0, 0, 0, 0), 1);
        result = fold(wide, NULL, 2, width, width, 0, 1, 0, 0, 0);
        CHECK(PROBE_BITS(result, 0) == UINT64_MAX - 1);
        CHECK(PROBE_BITS(result, (width - 1) / 64) == 1);
        CHECK(!sv4_is_unknown(result));
        CHECK(sv4_bytes(&result) == probe_payload_bytes(width, 0));
        sv4_destroy(&result); sv4_destroy(&wide[0]); sv4_destroy(&wide[1]);
    }
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("fixed-array reduction values: ok");
}

static void ownership(void) {
    const size_t count = 200000; /* 1,600,000 bits: larger than one packed payload. */
    sv4_t* cells = calloc(count, sizeof(*cells));
    CHECK(cells != NULL);
    for (size_t i = 0; i < count; ++i) cells[i] = sv4_from_u64(1, 8, 0);
    const size_t baseline = value_test_live();
    const size_t baseline_bytes = value_test_bytes();
    value_test_reset_stats();
    sv4_t result = fold(cells, NULL, count, 8, 8, 0, 0, 0, 0, 0);
    CHECK(PROBE_BITS(result, 0) == count % 256);
    CHECK(value_test_live() == baseline + probe_owner_allocations(8));
    CHECK(value_test_peak_live() <= baseline + 6);
    for (size_t round = 0; round < 10000; ++round) {
        sv4_replace(&result, fold(cells, NULL, 2, 8, 32, 0, 0, 1, 0, 0));
        CHECK(PROBE_BITS(result, 0) == 2);
        CHECK(value_test_live() == baseline + probe_owner_allocations(32));
    }
    sv4_replace(&cells[0], sv4_from_u64(99, 8, 0));
    CHECK(PROBE_BITS(result, 0) == 2); /* No alias of the receiver or map temporary. */
    sv4_destroy(&result);
    CHECK(value_test_live() == baseline && value_test_bytes() == baseline_bytes);
    for (size_t i = 0; i < count; ++i) sv4_destroy(&cells[i]);
    free(cells);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("fixed-array reduction ownership: ok");
}

int main(int argc, char** argv) {
    CHECK(argc == 2);
    if (strcmp(argv[1], "values") == 0) values();
    else if (strcmp(argv[1], "ownership") == 0) ownership();
    else CHECK(0);
    return 0;
}
