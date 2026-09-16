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
        if (cycle % 3 == 0 && width > 1) left.x[0] = 2;
        if (cycle % 5 == 0 && width > 2) left.z[0] = 4;
        if (width) left.bits[0] &= ~(left.x[0] | left.z[0]);
        sv4_t before = sv4_clone(&left);
        size_t retained = value_test_live();
        for (size_t i = 0; i < sizeof(binary) / sizeof(*binary); ++i) {
            sv4_t out = binary[i](left, right);
            CHECK(!out.bits || (out.bits != left.bits && out.bits != right.bits));
            CHECK(sv4_same(left, before));
            sv4_destroy(&out);
            CHECK(value_test_live() == retained);
        }
        for (size_t i = 0; i < sizeof(unary) / sizeof(*unary); ++i) {
            sv4_t out = unary[i](left);
            CHECK(!out.bits || out.bits != left.bits);
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
    CHECK(value.bits[0] == 0x12344);
    sv4_idx_part_select_set(&value, 64, 65, 0, value);
    CHECK(value.bits[1] == 0x12344);
    sv4_bit_select_set(&value, 0, value);
    expect_number(sv4_bit_select(value, 64), 0);
    sv4_destroy(&value);
    CHECK(value_test_live() == 0);
}

static void check_wide_and_strings(void) {
    sv4_t wide = sv4_zero(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 0);
    CHECK(sv4_bytes(&wide) == 393216);
    wide.bits[(wide.width - 1u) / 64u] = UINT64_C(1) << ((wide.width - 1u) % 64u);
    expect_number(sv4_countones(wide), 1);
    sv4_t narrow = sv4_from_u64(1, 1, 0);
    sv4_t sum = sv4_add(wide, narrow);
    CHECK(sum.width == wide.width && sum.bits[0] == 1);
    CHECK(wide.bits[0] == 0);
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

int main(void) {
    CHECK(sizeof(sv4_t) < 64);
    check_operations();
    check_selection_aliases();
    check_wide_and_strings();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    printf("value operations and ownership: OK (descriptor=%zu bytes)\n", sizeof(sv4_t));
    return 0;
}
