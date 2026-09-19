/* R09/R14 runtime contracts. This is a hand-written transcription of owned
 * formal/reference operations, NOT C emitted by an executed Rust compiler. */
#include "llg_rt.c"
#include "probe.h"

static sv4_select_plan_t member_plan(uint32_t root_width, int64_t base, uint32_t width) {
    sv4_select_plan_t plan = sv4_select_plan_init(root_width);
    sv4_t index = sv4_from_i64(base, 64);
    sv4_select_plan_step(&plan, index, width);
    sv4_destroy(&index);
    return plan;
}

static void increment_reference_member(llg_ref_t* reference,
                                        const sv4_select_plan_t* plan,
                                        uint32_t width, int two_state) {
    llg_value_scope_t* scope = llg_value_scope_begin(4);
    sv4_t* value = llg_value_scope_values(scope);
    value[0] = llg_ref_read(reference);
    if (two_state) sv4_replace(&value[0], sv4_to_two_state(value[0]));
    value[1] = sv4_select_plan_read(value[0], plan);
    value[2] = sv4_from_u64(1, width, 0);
    value[3] = sv4_add(value[1], value[2]);
    if (two_state) sv4_replace(&value[3], sv4_to_two_state(value[3]));
    /* Reload the unconverted parent: two-state conversion of a member must
     * not destroy unrelated four-state fields in the containing union. */
    sv4_replace(&value[0], llg_ref_read(reference));
    sv4_select_plan_set(&value[0], plan, value[3]);
    llg_ref_write(reference, value[0]);
    llg_value_scope_end(scope);
}

static sv4_t private_value_call(sv4_t borrowed) {
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* input = llg_value_scope_values(scope);
    input[0] = sv4_clone(&borrowed);
    llg_ref_t local = {0};
    local.kind = LLG_REF_WHOLE;
    local.base = &input[0];
    local.width = 16;
    sv4_select_plan_t plan = member_plan(16, 0, 8);
    increment_reference_member(&local, &plan, 8, 0);
    sv4_t result = sv4_clone(&input[0]);
    llg_value_scope_end(scope);
    return result;
}

int main(void) {
    llg_rt_init();
    sv4_t original = sv4_from_u64(0x1304, 16, 0);
    const size_t baseline = value_test_live();
    const size_t baseline_bytes = value_test_bytes();
    for (unsigned i = 0; i < 4096; ++i) {
        expect_number(private_value_call(original), 0x1305);
        CHECK(sv4_to_u64(original) == 0x1304);
        CHECK(value_test_live() == baseline);
        CHECK(value_test_bytes() == baseline_bytes);
        CHECK(value_scope_count == 0);
    }
    llg_ref_t ref = {0};
    ref.kind = LLG_REF_WHOLE;
    ref.base = &original;
    ref.width = 16;
    sv4_select_plan_t low = member_plan(16, 0, 8);
    increment_reference_member(&ref, &low, 8, 0);
    expect_number(llg_ref_read(&ref), 0x1305);
    CHECK(sv4_to_u64(original) == 0x1305);

    sv4_replace(&original, sv4_x(16, 0));
    increment_reference_member(&ref, &low, 8, 1);
    expect_number(sv4_part_select(original, 7, 0), 1);
    sv4_t high = sv4_part_select(original, 15, 8);
    CHECK(sv4_is_unknown(high));
    sv4_destroy(&high);

    /* A clipped nested selection cannot modify the neighboring field. */
    sv4_replace(&original, sv4_from_u64(0xa500, 16, 0));
    sv4_t index = sv4_from_i64(6, 64);
    sv4_select_plan_step(&low, index, 4);
    sv4_destroy(&index);
    sv4_t ones = sv4_from_u64(15, 4, 0);
    sv4_t updated = llg_ref_read(&ref);
    sv4_select_plan_set(&updated, &low, ones);
    llg_ref_write(&ref, updated);
    CHECK(sv4_to_u64(original) == 0xa5c0);
    sv4_destroy(&updated);
    sv4_destroy(&ones);
    sv4_destroy(&original);
    llg_rt_cleanup();
    CHECK(value_scope_count == 0);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("packed formal owner/reference contracts passed");
    return 0;
}
