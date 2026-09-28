#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static llg_proc_t* victim;
static unsigned progress;

typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t* scope;
    sv4_t* values;
    unsigned index;
} ownership_frame_t;
LLG_CO_ROOT_FRAME_OK(ownership_frame_t);

LLG_PROBE_PROCESS(normal_process, ownership_frame_t, 1) {
    LLG_PROBE_BEGIN(ownership_frame_t, 1);
    F->scope = llg_value_scope_begin(2);
    F->values = llg_value_scope_values(F->scope);
    sv4_replace(&F->values[0], sv4_zero(129, 0));
    sv4_replace(&F->values[1], sv4_from_u64(1, 1, 0));
    for (F->index = 0; F->index < 1000; ++F->index) {
        sv4_replace(&F->values[0], sv4_add(F->values[0], F->values[1]));
        LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
        CHECK(F->values[0].bits[0] == F->index + 1);
        CHECK(value_test_live() == 2);
        ++progress;
    }
    // Normal completion destroys all remaining registered scope owners.
    LLG_PROBE_DONE();
}

LLG_PROBE_PROCESS(canceled_process, ownership_frame_t, 1) {
    LLG_PROBE_BEGIN(ownership_frame_t, 1);
    F->scope = llg_value_scope_begin(2);
    F->values = llg_value_scope_values(F->scope);
    sv4_replace(&F->values[0], sv4_zero(65537, 0));
    sv4_replace(&F->values[1], sv4_from_u64(11, 65, 0));
    LLG_PROBE_AWAIT(1, llg_arm_level(self, &F->values[0], F->values[1]));
    CHECK(0);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(cancellation_process, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(victim->wait.kind == W_LEVEL && victim->value_scopes);
    llg_kill_proc_tree(victim);
    victim = NULL;
    CHECK(value_test_live() == 0);
    ++progress;
    LLG_PROBE_DONE();
}

int main(void) {
    llg_rt_init();
    llg_spawn(&normal_process_desc, "normal scoped values");
    llg_rt_run();
    CHECK(progress == 1000 && value_test_live() == 0);
    for (unsigned i = 0; i < 50; ++i) {
        llg_rt_init();
        victim = llg_spawn(&canceled_process_desc, "canceled scoped values");
        llg_spawn(&cancellation_process_desc, "cancel suspended owner");
        llg_rt_run();
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    CHECK(progress == 1050);
    // Use the shared helper as a trivial returned-owner check as well.
    expect_number(sv4_from_u64(1, 1, 0), 1);
    puts("real coroutine suspension, completion and cancellation: OK");
    return 0;
}
