/* Handwritten counterpart of owned/control.rs, not generated HDL.
 * Inputs below are borrowed. Every reached clause and result owns its exact
 * payload; skipped clauses neither record an evaluation nor allocate storage. */
#include "probe.h"
#include <string.h>

static sv4_t evaluate(const sv4_t* clauses, size_t count, unsigned* trace) {
    sv4_t result = sv4_from_u64(1, 1, 0);
    for (size_t i = 0; i < count; ++i) {
        if (sv4_to_bool(result)) {
            *trace = *trace * 10u + (unsigned)i + 1u;
            sv4_t value = sv4_clone(&clauses[i]);
            sv4_replace(&value, sv4_reduce_or(value));
            sv4_move(&result, &value);
            sv4_destroy(&value);
        }
    }
    CHECK(result.width == 1 && !result.is_signed);
    return result;
}

static void truth_table(void) {
    sv4_t states[4] = {
        sv4_from_u64(0, 1, 0), sv4_from_u64(1, 1, 0), sv4_x(1, 0), sv4_fill(3, 1, 0)
    };
    for (unsigned a = 0; a < 4; ++a) {
        for (unsigned b = 0; b < 4; ++b) {
            for (unsigned c = 0; c < 4; ++c) {
                const unsigned codes[3] = {a, b, c};
                const sv4_t borrowed[3] = {states[a], states[b], states[c]};
                unsigned trace = 0, expected_trace = 0, expected = 1;
                for (unsigned i = 0; i < 3; ++i) {
                    expected_trace = expected_trace * 10u + i + 1u;
                    expected = codes[i];
                    if (expected != 1) break;
                }
                sv4_t result = evaluate(borrowed, 3, &trace);
                CHECK(trace == expected_trace);
                if (expected < 2) CHECK(!sv4_is_unknown(result) && sv4_to_u64(result) == expected);
                else CHECK(result.x[0] == 1u && result.z[0] == 0u);
                sv4_destroy(&result);
                CHECK(value_test_live() == 4);
            }
        }
    }
    const uint64_t bits[3] = {0, 0, 1};
    const uint64_t unknown[3] = {1, 0, 0};
    sv4_t wide = sv4_from_limbs(bits, unknown, NULL, 129, 1);
    const sv4_t clauses[2] = {wide, states[1]};
    unsigned trace = 0;
    expect_number(evaluate(clauses, 2, &trace), 1);
    CHECK(trace == 12);
    sv4_destroy(&wide);
    CHECK(llg_real_to_bool(0.25) && llg_real_to_bool(-0.25));
    CHECK(!llg_real_to_bool(-0.0));

    /* Early ambiguity, even before false, must reach the existing mux's
     * ambiguous path. Packed and unpacked-array alternatives differ. */
    const sv4_t ambiguous[2] = {states[2], states[0]};
    trace = 0;
    sv4_t selector = evaluate(ambiguous, 2, &trace);
    sv4_t a = sv4_from_u64(0xa5, 8, 0);
    sv4_t b = sv4_from_u64(0xa6, 8, 0);
    sv4_t default_value = sv4_x(8, 0);
    sv4_t packed = sv4_mux(selector, a, b);
    sv4_t array = sv4_array_conditional_merge(a, b, default_value);
    CHECK(trace == 1 && packed.bits[0] == 0xa4 && packed.x[0] == 3);
    CHECK(array.bits[0] == 0 && array.x[0] == 0xff);
    sv4_destroy(&selector); sv4_destroy(&a); sv4_destroy(&b);
    sv4_destroy(&default_value); sv4_destroy(&packed); sv4_destroy(&array);
    for (unsigned i = 0; i < 4; ++i) sv4_destroy(&states[i]);
}

static void ownership(void) {
    sv4_t inputs[3] = {sv4_from_u64(1, 129, 1), sv4_x(65, 0), sv4_zero(257, 0)};
    const size_t live = value_test_live(), bytes = value_test_bytes();
    value_test_reset_stats();
    for (unsigned iteration = 0; iteration < 10000; ++iteration) {
        unsigned trace = 0;
        sv4_t result = evaluate(inputs, 3, &trace);
        CHECK(trace == 12 && result.x[0] == 1);
        CHECK(value_test_live() == live + 1);
        sv4_destroy(&result);
        CHECK(value_test_live() == live && value_test_bytes() == bytes);
    }
    CHECK(value_test_peak_live() <= live + 3);
    /* A first false clause must skip even the 257-bit expression payload. */
    sv4_replace(&inputs[0], sv4_zero(129, 0));
    const size_t before = value_test_allocations();
    unsigned trace = 0;
    expect_number(evaluate(inputs, 3, &trace), 0);
    CHECK(trace == 1 && value_test_allocations() - before == 3);
    for (unsigned i = 0; i < 3; ++i) sv4_destroy(&inputs[i]);
}

int main(int argc, char** argv) {
    CHECK(argc == 2);
    if (strcmp(argv[1], "truth") == 0) truth_table();
    else if (strcmp(argv[1], "ownership") == 0) ownership();
    else CHECK(0);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("sequential predicate probe passed");
    return 0;
}
