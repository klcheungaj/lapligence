/* Production streaming selector and destination preflight contracts. */
#include "llg_container.h"
#include "probe.h"
#include <stdint.h>
#include <string.h>

static void check_bounds(int kind, int64_t base, int64_t extent,
                         int64_t expected_right, size_t expected_count) {
    sv4_t first = sv4_from_i64(base, 64);
    sv4_t second = sv4_from_i64(extent, 64);
    int64_t left = 0, right = 0;
    size_t count = 0;
    llg_fixed_stream_bounds(kind, first, second, &left, &right, &count);
    CHECK(left == base && right == expected_right && count == expected_count);
    CHECK(llg_stream_selector_width(kind, first, second, 8) == expected_count * 8);
    CHECK(llg_fixed_stream_width(kind, first, second, 8) == expected_count * 8);
    CHECK(llg_fixed_stream_index_at(left, right, 0) == base);
    CHECK(llg_fixed_stream_index_at(left, right, count - 1) == expected_right);
    CHECK(llg_fixed_stream_target_in_bounds(left, right, left, right, count));
    CHECK(llg_fixed_stream_target_in_bounds(right, left, left, right, count));
    llg_stream_require_bits((int64_t)(count * 8), (uint32_t)(count * 8));
    llg_stream_require_bits((int64_t)(count * 8 + 8), (uint32_t)(count * 8));
    sv4_destroy(&first);
    sv4_destroy(&second);
    CHECK(value_test_live() == 0);
}

int main(int argc, char** argv) {
    if (argc == 1) {
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_PLUS, INT64_MAX, 1, INT64_MAX, 1);
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_MINUS, INT64_MIN, 1, INT64_MIN, 1);
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_PLUS, INT64_MIN, 2, INT64_MIN + 1, 2);
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_MINUS, INT64_MAX, 2, INT64_MAX - 1, 2);
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_PLUS, -2, 4, 1, 4);
        check_bounds(LLG_STREAM_SELECTOR_INDEXED_MINUS, 5, 4, 2, 4);
        check_bounds(LLG_STREAM_SELECTOR_RANGE, 2, -2, -2, 5);
        CHECK(!llg_fixed_stream_target_in_bounds(-2, 1, -3, 0, 4));
        CHECK(!llg_fixed_stream_target_in_bounds(1, -2, 0, 2, 3));
        CHECK(!llg_fixed_stream_target_in_bounds(1, -2, 0, -1, 0));
        for (int unknown_second = 0; unknown_second < 2; ++unknown_second) {
            sv4_t unknown = sv4_x(64, 1);
            sv4_t known = sv4_from_i64(0, 64);
            int64_t left, right;
            size_t count;
            llg_fixed_stream_bounds(LLG_STREAM_SELECTOR_RANGE,
                                    unknown_second ? known : unknown,
                                    unknown_second ? unknown : known,
                                    &left, &right, &count);
            CHECK(count == 0);
            CHECK(!llg_fixed_stream_target_in_bounds(-2, 1, left, right, count));
            sv4_destroy(&unknown);
            sv4_destroy(&known);
        }
        CHECK(value_test_live() == 0);
        llg_stream_require_bits(0, 0);
        puts("stream preflight and endpoint checks passed");
        return 0;
    }
    const char* mode = argv[1];
    if (strcmp(mode, "short") == 0) {
        llg_stream_require_bits(15, 16);
    } else if (strcmp(mode, "multi-short") == 0) {
        llg_stream_require_bits(24, 16);
        llg_stream_require_bits(8, 16);
    } else if (strcmp(mode, "negative-remaining") == 0) {
        llg_stream_require_bits(-1, 0);
    } else {
        int kind = LLG_STREAM_SELECTOR_INDEXED_PLUS;
        int64_t base = 0, extent = 1;
        if (strcmp(mode, "plus-overflow") == 0) {
            base = INT64_MAX;
            extent = 2;
        } else if (strcmp(mode, "minus-overflow") == 0) {
            kind = LLG_STREAM_SELECTOR_INDEXED_MINUS;
            base = INT64_MIN;
            extent = 2;
        } else if (strcmp(mode, "zero-width") == 0) {
            extent = 0;
        } else if (strcmp(mode, "negative-width") == 0) {
            extent = -1;
        } else {
            return 2;
        }
        sv4_t first = sv4_from_i64(base, 64);
        sv4_t second = sv4_from_i64(extent, 64);
        int64_t left, right;
        size_t count;
        llg_fixed_stream_bounds(kind, first, second, &left, &right, &count);
        sv4_destroy(&first);
        sv4_destroy(&second);
    }
    /* Every explicit mode above is a rejection test. */
    return 0;
}
