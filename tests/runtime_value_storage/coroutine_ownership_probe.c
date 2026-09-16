#include "llg_rt.c"
#include "probe.h"

static llg_proc_t* victim;
static unsigned progress;

static void normal_process(llg_proc_t* self) {
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_zero(129, 0));
    sv4_replace(&values[1], sv4_from_u64(1, 1, 0));
    for (unsigned i = 0; i < 1000; ++i) {
        sv4_replace(&values[0], sv4_add(values[0], values[1]));
        llg_wait_time(1);
        CHECK(values[0].bits[0] == i + 1);
        CHECK(value_test_live() == 2);
        ++progress;
    }
    // Normal completion destroys all remaining registered scope owners.
    llg_proc_done(self);
}

static void canceled_process(llg_proc_t* self) {
    (void)self;
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_zero(65537, 0));
    sv4_replace(&values[1], sv4_from_u64(11, 65, 0));
    llg_wait_level(&values[0], values[1]);
    CHECK(0);
}

static void cancellation_process(llg_proc_t* self) {
    llg_wait_time(1);
    CHECK(victim->wait.kind == W_LEVEL && victim->value_scopes);
    llg_kill_proc_tree(victim);
    victim = NULL;
    CHECK(value_test_live() == 0);
    ++progress;
    llg_proc_done(self);
}

int main(void) {
    llg_rt_init();
    llg_spawn(normal_process, "normal scoped values");
    llg_rt_run();
    CHECK(progress == 1000 && value_test_live() == 0);
    for (unsigned i = 0; i < 50; ++i) {
        llg_rt_init();
        victim = llg_spawn(canceled_process, "canceled scoped values");
        llg_spawn(cancellation_process, "cancel suspended owner");
        llg_rt_run();
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    CHECK(progress == 1050);
    // Use the shared helper as a trivial returned-owner check as well.
    expect_number(sv4_from_u64(1, 1, 0), 1);
    puts("real coroutine suspension, completion and cancellation: OK");
    return 0;
}
