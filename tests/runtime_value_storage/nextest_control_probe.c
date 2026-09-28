/* Handwritten C11 counterparts of emitted ownership/cancellation edges.
 * Cargo tests in owned/tests/nextest_regressions.rs separately invoke the real
 * Rust emitter. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static sv4_t result = SV4_EMPTY;
static unsigned completed;
static unsigned unexpected_copyout;

typedef struct {
    llg_co_frame_t co;
    sv4_t* output;
    llg_value_scope_t* base;
    sv4_t* temps;
    llg_value_scope_t* activation_mark;
    llg_activation_t* activation;
    llg_value_scope_t* body_mark;
    sv4_t* local;
} staged_task_frame_t;
LLG_CO_ROOT_FRAME_OK(staged_task_frame_t);

LLG_PROBE_PROCESS(staged_task, staged_task_frame_t, 1) {
    LLG_PROBE_BEGIN(staged_task_frame_t, 1);
    F->base = llg_value_scope_mark();
    F->temps = llg_value_scope_values(llg_value_scope_begin(1));
    F->activation_mark = llg_value_scope_mark();
    F->activation = llg_activation_enter(100, 1);
    F->body_mark = llg_value_scope_mark();
    F->local = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(F->local, sv4_zero(65537, 0));
    sv4_replace(&F->temps[0], sv4_from_u64(42, 65, 0));
    llg_ba(F->output, F->temps[0]);
    sv4_destroy(&F->temps[0]);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 2));
    if (!llg_activation_cancelled()) ++unexpected_copyout;
    llg_value_scopes_end_since(F->body_mark);
    llg_activation_exit(F->activation);
    llg_value_scopes_end_since(F->activation_mark);
    llg_value_scopes_end_since(F->base);
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t* base;
    llg_value_scope_t* activation_mark;
    llg_activation_t* activation;
    llg_value_scope_t* body_mark;
    sv4_t* temporary;
    staged_task_frame_t task;
} caller_frame_t;
LLG_CO_ROOT_FRAME_OK(caller_frame_t);

LLG_PROBE_PROCESS(caller, caller_frame_t, 1) {
    LLG_PROBE_BEGIN(caller_frame_t, 1);
    (void)&staged_task_desc;
    F->base = llg_value_scope_mark();
    F->activation_mark = llg_value_scope_mark();
    F->activation = llg_activation_enter(100, 1);
    F->body_mark = llg_value_scope_mark();
    F->temporary = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(F->temporary, sv4_x(65, 0));
    F->task.output = F->temporary;
    LLG_CO_CALL(co, ch, 1, staged_task, (llg_co_frame_t*)&F->task);
    /* Both caller and callee activations share the disable target. The
     * callee has exited, but the caller must still skip output copyout. */
    if (!llg_activation_cancelled()) {
        llg_ba(&result, *F->temporary);
        ++unexpected_copyout;
    }
    llg_value_scopes_end_since(F->body_mark);
    llg_activation_exit(F->activation);
    llg_value_scopes_end_since(F->activation_mark);
    CHECK(sv4_to_u64(result) == 7);
    CHECK(llg_time() == 1);
    CHECK(!llg_activation_cancelled());
    ++completed;
    llg_value_scopes_end_since(F->base);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(canceller, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_disable_target(self, 100, 1);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(escaped_activation, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    llg_value_scope_t* base = llg_value_scope_mark();
    llg_activation_t* outer = llg_activation_enter(101, 1);
    sv4_t* kept = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(kept, sv4_from_u64(23, 65, 0));
    {
        llg_value_scope_t* inner_mark = llg_value_scope_mark();
        llg_activation_t* inner = llg_activation_enter(102, 1);
        sv4_t* discarded = llg_value_scope_values(llg_value_scope_begin(1));
        sv4_replace(discarded, sv4_zero(65537, 0));
        llg_activation_exit(inner);
        llg_value_scopes_end_since(inner_mark);
        goto escaped;
    }
escaped: ;
    llg_disable_target(self, 102, 1);
    CHECK(!llg_activation_cancelled());
    CHECK(sv4_to_u64(*kept) == 23);
    llg_activation_exit(outer);
    llg_value_scopes_end_since(base);
    ++completed;
    LLG_PROBE_DONE();
}

static void display_snapshot(llg_fmt_arg_t* out, void* context) {
    (void)context;
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* temporary = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_copy(temporary, &result);
    llg_fmt_arg_t args[1] = {{0}};
    args[0].kind = LLG_FMT_PACKED;
    args[0].time_unit_fs = 1;
    sv4_move(&args[0].value.packed, temporary);
    out[0] = args[0];
    args[0] = (llg_fmt_arg_t){0};
    llg_value_scopes_end_since(base);
}

static void force_snapshot(sv4_t* out) {
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* temporary = llg_value_scope_values(llg_value_scope_begin(1));
    sv4_replace(temporary, sv4_from_u64(66, 65, 0));
    sv4_move(out, temporary);
    llg_value_scopes_end_since(base);
}

typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t* base;
    sv4_t* temporary;
} runtime_tasks_frame_t;
LLG_CO_ROOT_FRAME_OK(runtime_tasks_frame_t);

LLG_PROBE_PROCESS(runtime_tasks, runtime_tasks_frame_t, 2) {
    LLG_PROBE_BEGIN(runtime_tasks_frame_t, 2);
    F->base = llg_value_scope_mark();
    F->temporary = llg_value_scope_values(llg_value_scope_begin(1));
    static llg_inertial_t* driver;
    CHECK(driver == NULL); /* cleanup resets the static handle across starts */
    sv4_replace(F->temporary, sv4_from_u64(42, 65, 0));
    llg_inertial_assign(&driver, &result, *F->temporary, 2, 2, 2);
    sv4_destroy(F->temporary);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(sv4_to_u64(result) == 7);
    LLG_PROBE_AWAIT(2, llg_arm_time(self, 2));
    CHECK(sv4_to_u64(result) == 42);
    llg_strobe_typed("strobe=%0d", 1, display_snapshot, "probe");
    const llg_force_part_t parts[] = {{ &result, NULL, 64, 0, 65, 0, 0 }};
    llg_force_expr_parts(parts, 1, 0, 0, force_snapshot, NULL, 0);
    CHECK(sv4_to_u64(result) == 66);
    llg_release_parts(parts, 1, 0, 0);
    CHECK(sv4_to_u64(result) == 66);
    ++completed;
    llg_value_scopes_end_since(F->base);
    LLG_PROBE_DONE();
}

int main(void) {
    for (unsigned cycle = 0; cycle < 8; ++cycle) {
        llg_rt_init();
        sv4_replace(&result, sv4_from_u64(7, 65, 0));
        llg_spawn(&caller_desc, "cancelled output");
        llg_spawn(&canceller_desc, "cancel");
        llg_spawn(&escaped_activation_desc, "activation jump");
        llg_rt_run();
        CHECK(unexpected_copyout == 0);
        CHECK(g.activations == NULL && all_value_scopes == NULL);
        llg_rt_cleanup();
        sv4_destroy(&result);
        CHECK(value_test_live() == 0);

        llg_rt_init();
        sv4_replace(&result, sv4_from_u64(7, 65, 0));
        llg_spawn(&runtime_tasks_desc, "numeric runtime tasks");
        llg_rt_run();
        CHECK(g.activations == NULL && all_value_scopes == NULL);
        llg_rt_cleanup();
        sv4_destroy(&result);
        CHECK(value_test_live() == 0);
    }
    CHECK(completed == 24);
    puts("nextest control ownership ok");
    return 0;
}
