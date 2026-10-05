#include "probe.h"
#include "llg_string.h"
#include <string.h>

typedef sv4_t (*binary_fn)(sv4_t, sv4_t);
typedef sv4_t (*unary_fn)(sv4_t);
static void check_operations(void) {
    const binary_fn binary[] = {
        sv4_add, sv4_sub, sv4_mul, sv4_div, sv4_mod, sv4_pow,
        sv4_and, sv4_or, sv4_xor, sv4_xnor, sv4_logand, sv4_logor,
        sv4_logimpl, sv4_logequiv, sv4_shl, sv4_shr, sv4_ashl, sv4_ashr,
        sv4_eq, sv4_neq, sv4_case_eq, sv4_case_neq, sv4_wild_eq,
        sv4_wild_neq, sv4_casez_eq, sv4_casex_eq, sv4_lt, sv4_le,
        sv4_gt, sv4_ge, sv4_concat
    };
    const unary_fn unary[] = {
        sv4_neg, sv4_bitneg, sv4_lognot, sv4_reduce_and, sv4_reduce_nand,
        sv4_reduce_or, sv4_reduce_nor, sv4_reduce_xor, sv4_reduce_xnor,
        sv4_to_two_state, sv4_clog2, sv4_countones, sv4_repeat_count
    };
    const uint32_t widths[] = {0, 1, 63, 64, 65, 129, 257};
    for (unsigned cycle = 0; cycle < 250; ++cycle) {
        uint32_t width = widths[cycle % (sizeof(widths) / sizeof(*widths))];
        sv4_t left = sv4_from_i64(-13, width);
        sv4_t right = sv4_from_u64(3, width, (int8_t)(cycle & 1));
        if (cycle % 3 == 0 && width > 1) probe_put_state(&left, 1, 2);
        if (cycle % 5 == 0 && width > 2) probe_put_state(&left, 2, 3);
        sv4_t before = sv4_clone(&left);
        size_t retained = value_test_live();
        for (size_t i = 0; i < sizeof(binary) / sizeof(*binary); ++i) {
            sv4_t out = binary[i](left, right);
            CHECK(probe_distinct(&out, left) && probe_distinct(&out, right));
            CHECK(sv4_same(left, before));
            sv4_destroy(&out);
            CHECK(value_test_live() == retained);
        }
        for (size_t i = 0; i < sizeof(unary) / sizeof(*unary); ++i) {
            sv4_t out = unary[i](left);
            CHECK(probe_distinct(&out, left));
            CHECK(sv4_same(left, before));
            sv4_destroy(&out);
            CHECK(value_test_live() == retained);
        }
        sv4_t out = sv4_resize(left, width, left.is_signed);
        sv4_replace(&out, sv4_cast(out, width + 70, 1));
        sv4_replace(&out, sv4_mux(right, left, before));
        sv4_replace(&out, sv4_stream(left, 3, 1));
        sv4_replace(&out, sv4_unstream(out, 3, 1));
        CHECK(sv4_same(out, left));
        sv4_replace(&out, sv4_repeat(left, 2));
        sv4_replace(&out, sv4_part_select(left, 130, -2));
        sv4_replace(&out, sv4_idx_part_select(left, 5, 140, 0));
        sv4_replace(&out, sv4_bit_select(left, 0));
        sv4_replace(&out, sv4_inside_range(left, right, before));
        const sv4_t* drivers[] = {&left, &before};
        sv4_replace(&out, sv4_resolve(drivers, 2, width, 0, LLG_RESOLVE_WIRE));
        sv4_destroy(&out);
        char text[512];
        sv4_format('d', left, text, sizeof(text));
        sv4_format('b', left, text, sizeof(text));
        (void)sv4_to_real(left);
        CHECK(sv4_same(left, before));
        CHECK(value_test_live() == retained);
        sv4_destroy(&left);
        sv4_destroy(&right);
        sv4_destroy(&before);
        CHECK(value_test_live() == 0);
    }
}

static void check_selection_aliases(void) {
    sv4_t value = sv4_from_u64(0x1234, 129, 0);
    sv4_part_select_set(&value, 132, 4, value);
    CHECK(PROBE_BITS(value, 0) == 0x12344);
    sv4_idx_part_select_set(&value, 64, 65, 0, value);
    CHECK(PROBE_BITS(value, 1) == 0x12344);
    sv4_bit_select_set(&value, 0, value);
    expect_number(sv4_bit_select(value, 64), 0);
    sv4_destroy(&value);
    CHECK(value_test_live() == 0);
}

static void check_wide_and_strings(void) {
    sv4_t wide = sv4_zero(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 0);
    CHECK(sv4_bytes(&wide) == probe_payload_bytes(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 0));
    probe_set_bits(&wide, (wide.width - 1u) / 64u, UINT64_C(1) << ((wide.width - 1u) % 64u));
    expect_number(sv4_countones(wide), 1);
    sv4_t narrow = sv4_from_u64(1, 1, 0);
    sv4_t sum = sv4_add(wide, narrow);
    CHECK(sum.width == wide.width && PROBE_BITS(sum, 0) == 1);
    CHECK(PROBE_BITS(wide, 0) == 0);
    sv4_destroy(&sum);
    sv4_destroy(&narrow);
    sv4_destroy(&wide);
    for (int i = 0; i < 1000; ++i) {
        sv4_t packed = sv4_from_u64(UINT64_C(0x41424344), 65, 0);
        llg_string_t text = llg_string_from_packed(packed);
        sv4_t back = llg_string_to_packed(llg_string_clone(&text), 65, 0);
        CHECK(sv4_same(packed, back));
        sv4_destroy(&back);
        sv4_destroy(&packed);
        llg_string_destroy(&text);
        CHECK(value_test_live() == 0);
    }
}

static void check_arithmetic_destinations(void) {
    const uint32_t widths[] = {1, 63, 64, 65, 128, 129, 257};
    for (size_t i = 0; i < sizeof(widths) / sizeof(*widths); ++i) {
        uint32_t width = widths[i];
        sv4_t a = sv4_from_u64(5, width, 0);
        sv4_t b = sv4_from_u64(3, width, 0);
        sv4_t out = sv4_zero(width, 0);
        size_t allocations = value_test_allocations();
        uint64_t mask = width < 64 ? (UINT64_C(1) << width) - 1 : UINT64_MAX;
        sv4_add_into(&a, a, b);
        CHECK(sv4_to_u64(a) == (8 & mask));
        sv4_sub_into(&b, a, b);
        CHECK(sv4_to_u64(b) == (5 & mask));
        sv4_add_into(&a, a, a);
        CHECK(sv4_to_u64(a) == (16 & mask));
        sv4_mul_into(&out, a, b);
        CHECK(sv4_to_u64(out) == (80 & mask));
        CHECK(value_test_allocations() ==
              allocations + (width > 64 ? probe_kernel_scratch_allocations() : 0));
        sv4_mul_into(&a, a, b);
        CHECK(sv4_to_u64(a) == (80 & mask));
        llg_sv4_set_state(&b, 0, 3);
        allocations = value_test_allocations();
        sv4_add_into(&a, a, b);
        for (uint32_t bit = 0; bit < width; ++bit) CHECK(llg_sv4_state(a, bit) == 2);
        sv4_sub_into(&a, a, a);
        sv4_mul_into(&a, a, a);
        CHECK(value_test_allocations() == allocations);
        sv4_destroy(&a);
        sv4_destroy(&b);
        sv4_destroy(&out);
    }
    sv4_t carry = sv4_fill(1, 128, 0);
    sv4_t one = sv4_from_u64(1, 128, 0);
    size_t allocations = value_test_allocations();
    sv4_add_into(&carry, carry, one);
    for (uint32_t bit = 0; bit < 128; ++bit) CHECK(llg_sv4_state(carry, bit) == 0);
    sv4_sub_into(&carry, carry, one);
    for (uint32_t bit = 0; bit < 128; ++bit) CHECK(llg_sv4_state(carry, bit) == 1);
    CHECK(value_test_allocations() == allocations);
    sv4_destroy(&carry);
    sv4_destroy(&one);
    sv4_t narrow = sv4_from_i64(-1, 64);
    sv4_t wide = sv4_from_u64(2, 65, 1);
    sv4_add_into(&narrow, narrow, wide);
    CHECK(llg_sv4_width(narrow) == 65 && llg_sv4_signed(narrow));
    CHECK(sv4_to_u64(narrow) == 1 && llg_sv4_state(narrow, 64) == 0);
    sv4_sub_into(&narrow, narrow, wide);
    CHECK(sv4_to_u64(narrow) == UINT64_MAX && llg_sv4_state(narrow, 64) == 1);
    sv4_destroy(&narrow);
    sv4_destroy(&wide);
    CHECK(value_test_live() == 0);
}

int main(void) {
    CHECK(sizeof(sv4_t) < 64);
    check_operations();
    check_arithmetic_destinations();
    check_selection_aliases();
    check_wide_and_strings();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    printf("value operations and ownership: OK (descriptor=%zu bytes)\n", sizeof(sv4_t));
    return 0;
}
