/* Hand-authored C11 output-shape tests. These exercise actual libaco switching
 * and must not run under sanitizers until libaco has fiber-switch hooks. */
#include "llg_rt.c"
#include "probe.h"

static sv4_t* retained_target;
static unsigned progress;

static void delayed_producer(llg_proc_t* self) {
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
    llg_proc_done(self);
}

static void delayed_observer(llg_proc_t* self) {
    llg_wait_time(2);
    CHECK(retained_target != NULL);
    expect_number(sv4_clone(retained_target), 17);
    CHECK(all_value_scopes && !all_value_scopes->active);
    llg_wait_time(2);
    CHECK(all_value_scopes == NULL);
    retained_target = NULL;
    ++progress;
    llg_proc_done(self);
}

static sv4_t yielding_call(sv4_t input) {
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* local = llg_value_scope_values(llg_value_scope_begin(2));
    sv4_copy(&local[0], &input);
    sv4_replace(&local[1], sv4_from_u64(1, 65, 0));
    llg_wait_time(1);
    sv4_replace(&local[0], sv4_add(local[0], local[1]));
    sv4_t result = sv4_clone(&local[0]);
    llg_value_scopes_end_since(base);
    return result;
}

static void stop_resume_process(llg_proc_t* self) {
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* slots = llg_value_scope_values(llg_value_scope_begin(2));
    sv4_replace(&slots[0], sv4_from_u64(7, 65, 0));
    sv4_replace(&slots[1], yielding_call(slots[0]));
    sv4_destroy(&slots[0]);
    expect_number(sv4_clone(&slots[1]), 8);
    ++progress;
    llg_rt_stop();
    expect_number(sv4_clone(&slots[1]), 8);
    sv4_replace(&slots[0], yielding_call(slots[1]));
    expect_number(sv4_clone(&slots[0]), 9);
    ++progress;
    llg_value_scopes_end_since(base);
    llg_proc_done(self);
}

static void finish_process(llg_proc_t* self) {
    (void)self;
    sv4_t* local = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(local, sv4_zero(65537, 0));
    llg_rt_finish();
}

int main(void) {
    for (unsigned cycle = 0; cycle < 40; ++cycle) {
        llg_rt_init();
        llg_spawn(delayed_producer, "return with delayed local target");
        llg_spawn(delayed_observer, "observe retained cell");
        llg_rt_run();
        CHECK(all_value_scopes == NULL && value_test_live() == 0);
    }
    CHECK(progress == 40);
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_EXIT));
    for (unsigned cycle = 0; cycle < 20; ++cycle) {
        llg_rt_init();
        llg_spawn(stop_resume_process, "stop after yielding call");
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
    llg_spawn(stop_resume_process, "close suspended owner");
    llg_rt_run();
    CHECK(llg_rt_is_suspended() && value_test_live() == 1);
    llg_rt_cleanup();
    llg_rt_cleanup();
    CHECK(all_value_scopes == NULL && value_test_live() == 0);
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_RESUME));
    llg_rt_init();
    llg_spawn(finish_process, "nonreturning finish");
    llg_rt_run();
    CHECK(all_value_scopes == NULL && value_test_live() == 0);
    CHECK(value_test_bytes() == 0);
    puts("P05 real coroutine return, yielding calls, stop/resume, close and finish: OK");
    return 0;
}
