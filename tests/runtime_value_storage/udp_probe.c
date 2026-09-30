#include "probe.h"

static unsigned state(const sv4_t* value) {
    if (value->x[0] & 1u) return 2;
    if (value->z[0] & 1u) return 3;
    return (unsigned)(value->bits[0] & 1u);
}

static void check(const uint8_t* rows, size_t count, unsigned a, unsigned b,
                  unsigned expected) {
    sv4_t left = sv4_fill((uint8_t)a, 1, 0);
    sv4_t right = sv4_fill((uint8_t)b, 1, 0);
    const sv4_t* inputs[] = {&left, &right};
    size_t live = value_test_live();
    sv4_t result = sv4_udp_eval(rows, count, 2, inputs);
    CHECK(result.width == 1 && !result.is_signed);
    CHECK(state(&result) == expected);
    CHECK(result.bits != left.bits && result.bits != right.bits);
    CHECK(value_test_live() == live + 1);
    CHECK(state(&left) == a && state(&right) == b);
    sv4_destroy(&result);
    CHECK(value_test_live() == live);
    sv4_destroy(&left);
    sv4_destroy(&right);
}

int main(void) {
    static const uint8_t masks[] = {1, 2, 4, 3, 7};
    static const unsigned matches[5][4] = {
        {1, 0, 0, 0}, {0, 1, 0, 0}, {0, 0, 1, 1},
        {1, 1, 0, 0}, {1, 1, 1, 1}
    };
    for (size_t mask = 0; mask < 5; ++mask) {
        uint8_t row[] = {masks[mask], 7, 1};
        for (unsigned a = 0; a < 4; ++a)
            for (unsigned b = 0; b < 4; ++b)
                check(row, 1, a, b, matches[mask][a] ? 1 : 2);
    }
    static const uint8_t overlap[] = {3, 7, 0, 7, 7, 1};
    static const uint8_t outputs[] = {1, 1, 0, 2, 1, 1, 4, 1, 2};
    for (unsigned a = 0; a < 4; ++a) {
        check(outputs, 3, a, 0, a < 2 ? a : 2);
        check(outputs, 3, a, 1, 2);
        check(NULL, 0, a, 0, 2);
    }
    for (size_t repeat = 0; repeat < 10000; ++repeat) {
        check(overlap, 2, 0, 3, 0);
        check(overlap, 2, 2, 3, 1);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    puts("UDP masks and ownership ok");
    return 0;
}
