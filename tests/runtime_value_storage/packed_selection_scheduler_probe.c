/* Direct production scheduler checks; no coroutine stack switching. */
#include "llg_rt.c"
#include "probe.h"

static void step(sv4_select_plan_t* plan, int64_t base, uint32_t width) {
    sv4_t index = sv4_from_i64(base, 64);
    sv4_select_plan_step(plan, index, width);
    sv4_destroy(&index);
}
static void masked_nba(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t target = sv4_from_u64(0xa500, 16, 0);
    sv4_t value = sv4_clone(&target), mask = sv4_zero(16, 0), rhs = sv4_from_u64(15, 4, 0);
    sv4_select_plan_t plan = sv4_select_plan_init(16);
    step(&plan, 0, 8); step(&plan, 6, 4);
    sv4_select_plan_set(&value, &plan, rhs);
    sv4_select_plan_set(&mask, &plan, rhs);
    CHECK(mask.bits[0] == 0xc0);
    llg_nba_masked(&target, value, mask, 1);
    sv4_destroy(&value); sv4_destroy(&mask); sv4_destroy(&rhs);
    plan = sv4_select_plan_init(16); /* Plan can change or disappear after issue. */
    step(&plan, 8, 8);
    sv4_replace(&target, sv4_from_u64(0x5a15, 16, 0));
    ++g.now;
    commit_nbas(LLG_REGION_NBA);
    CHECK(target.bits[0] == 0x5ad5); /* Preserve updates outside original selected mask. */
    CHECK(value_test_live() == 1);
    sv4_destroy(&target);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
    puts("packed selection NBA issue mask passed");
}
static void synchronous_input(void) {
    llg_rt_init();
    sv4_t target = sv4_from_u64(0xa500, 16, 0);
    sv4_select_plan_t plan = sv4_select_plan_init(16);
    step(&plan, 0, 8); step(&plan, 6, 4);
    llg_ref_t ref = {0};
    ref.base = &target; ref.width = 4; ref.kind = LLG_REF_PACKED_PLAN; ref.retained = &plan;
    llg_file_input_target_t input = {LLG_FILE_INPUT_PACKED, &ref, NULL, NULL, 0};
    for (int count = 0; count < 1000; ++count) {
        CHECK(llg_string_scanf("f", 1, "%h", &input, 1) == 1);
        CHECK(target.bits[0] == 0xa5c0);
        sv4_t read = llg_ref_read(&ref);
        CHECK(read.bits[0] == 3 && read.x[0] == 12);
        sv4_destroy(&read);
        CHECK(value_test_live() == 1);
    }
    sv4_destroy(&target);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
    puts("packed selection synchronous input passed");
}
int main(int argc, char** argv) {
    if (argc != 2) return 2;
    if (!strcmp(argv[1], "nba")) masked_nba();
    else if (!strcmp(argv[1], "input")) synchronous_input();
    else return 2;
    return 0;
}
