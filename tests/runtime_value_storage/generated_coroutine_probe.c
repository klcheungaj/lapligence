/* Hand-authored C11 output-shape tests for explicit coroutine frames. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static sv4_t* retained_target;
static unsigned progress;

LLG_PROBE_SIMPLE_PROCESS(delayed_producer, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* value = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(value, sv4_from_u64(17, 65, 0));
    llg_value_scope_t* cell = llg_value_scope_begin(1);
    retained_target = llg_value_scope_values(cell);
    sv4_replace(retained_target, sv4_zero(65, 0));
    llg_nba_after(retained_target, *value, 1);
    value->bits[0] = 23;
    llg_nba_after(retained_target, *value, 3);
    llg_value_scopes_end_since(base);
    /* Both lexical cells and coroutine disappear before the first write. */
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(delayed_observer, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 2));
    CHECK(retained_target != NULL);
    expect_number(sv4_clone(retained_target), 17);
    CHECK(all_value_scopes && !all_value_scopes->active);
    LLG_PROBE_AWAIT(2, llg_arm_time(self, 2));
    CHECK(all_value_scopes == NULL);
    retained_target = NULL;
    ++progress;
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    sv4_t input;
    sv4_t* result;
    llg_value_scope_t* base;
    sv4_t* local;
} yielding_call_frame_t;
LLG_CO_ROOT_FRAME_OK(yielding_call_frame_t);

LLG_PROBE_PROCESS(yielding_call, yielding_call_frame_t, 1) {
    LLG_PROBE_BEGIN(yielding_call_frame_t, 1);
    F->base = llg_value_scope_mark();
    F->local = llg_value_scope_values(llg_value_scope_begin(2));
    sv4_copy(&F->local[0], &F->input);
    sv4_replace(&F->local[1], sv4_from_u64(1, 65, 0));
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    sv4_replace(&F->local[0], sv4_add(F->local[0], F->local[1]));
    sv4_replace(F->result, sv4_clone(&F->local[0]));
    llg_value_scopes_end_since(F->base);
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t* base;
    sv4_t* slots;
    yielding_call_frame_t call;
} stop_resume_frame_t;
LLG_CO_ROOT_FRAME_OK(stop_resume_frame_t);

LLG_PROBE_PROCESS(stop_resume_process, stop_resume_frame_t, 3) {
    LLG_PROBE_BEGIN(stop_resume_frame_t, 3);
    (void)&yielding_call_desc;
    F->base = llg_value_scope_mark();
    F->slots = llg_value_scope_values(llg_value_scope_begin(2));
    sv4_replace(&F->slots[0], sv4_from_u64(7, 65, 0));
    F->call.input = F->slots[0];
    F->call.result = &F->slots[1];
    LLG_CO_CALL(co, ch, 1, yielding_call, (llg_co_frame_t*)&F->call);
    sv4_destroy(&F->slots[0]);
    expect_number(sv4_clone(&F->slots[1]), 8);
    ++progress;
    LLG_PROBE_AWAIT(2, llg_arm_stop(self, 0, NULL));
    expect_number(sv4_clone(&F->slots[1]), 8);
    F->call.input = F->slots[1];
    F->call.result = &F->slots[0];
    LLG_CO_CALL(co, ch, 3, yielding_call, (llg_co_frame_t*)&F->call);
    expect_number(sv4_clone(&F->slots[0]), 9);
    ++progress;
    llg_value_scopes_end_since(F->base);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(finish_process, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    sv4_t* local = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(local, sv4_zero(65537, 0));
    llg_rt_finish();
    LLG_PROBE_EXIT();
}

int main(void) {
    for (unsigned cycle = 0; cycle < 40; ++cycle) {
        llg_rt_init();
        llg_spawn(&delayed_producer_desc, "return with delayed local target");
        llg_spawn(&delayed_observer_desc, "observe retained cell");
        llg_rt_run();
        CHECK(all_value_scopes == NULL && value_test_live() == 0);
    }
    CHECK(progress == 40);
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_EXIT));
    for (unsigned cycle = 0; cycle < 20; ++cycle) {
        llg_rt_init();
        llg_spawn(&stop_resume_process_desc, "stop after yielding call");
        llg_rt_run();
        CHECK(llg_rt_is_suspended());
        CHECK(value_test_live() == 1 && all_value_scopes != NULL);
        CHECK(llg_rt_resume());
        llg_rt_run();
        CHECK(!llg_rt_is_suspended() && value_test_live() == 0);
        CHECK(all_value_scopes == NULL);
    }
    CHECK(progress == 80);
    /* Closing an intentionally suspended model cancels its owners, rather
     * than expecting its C function to resume and reach cleanup statements. */
    llg_rt_init();
    llg_spawn(&stop_resume_process_desc, "close suspended owner");
    llg_rt_run();
    CHECK(llg_rt_is_suspended() && value_test_live() == 1);
    llg_rt_cleanup();
    llg_rt_cleanup();
    CHECK(all_value_scopes == NULL && value_test_live() == 0);
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_RESUME));
    llg_rt_init();
    llg_spawn(&finish_process_desc, "nonreturning finish");
    llg_rt_run();
    CHECK(all_value_scopes == NULL && value_test_live() == 0);
    CHECK(value_test_bytes() == 0);
    puts("P05 real coroutine return, yielding calls, stop/resume, close and finish: OK");
    return 0;
}
