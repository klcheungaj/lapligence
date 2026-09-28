/* Real-coroutine nonlocal exits must release runtime snapshots as well as
 * generated temporaries. Test before/after writing the callback result. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static sv4_t signal_value = SV4_EMPTY;
static sv4_t next_value = SV4_EMPTY;
static unsigned evaluations;
static int finish_mode;

static void evaluate(sv4_t* out, void* context) {
    (void)context;
    ++evaluations;
    if (evaluations == 1 && finish_mode == 4) {
        llg_rt_finish();
        if (llg_rt_exiting()) return;
    }
    if (evaluations == 2 && finish_mode == 1) {
        llg_rt_finish();
        if (llg_rt_exiting()) return;
    }
    sv4_copy(out, &signal_value);
    if (evaluations == 2 && finish_mode == 2) {
        llg_rt_finish();
        if (llg_rt_exiting()) return;
    }
}

static void condition(sv4_t* out, void* context) {
    (void)context;
    sv4_copy(out, &next_value);
    llg_rt_finish();
}

static llg_co_arm_t arm_waiting_process(llg_proc_t* self) {
    sv4_t* reads[] = {&signal_value};
    llg_expr_event_spec_t spec = {0};
    spec.eval = evaluate;
    spec.reads = reads;
    spec.n_reads = 1;
    spec.kind = LLG_EV_ANY;
    if (finish_mode == 5 || finish_mode == 6) spec.condition = condition;
    if (finish_mode == 6) {
        // Runtime ownership is per field, not per distinct pointer.
        llg_frame_t* shared = llg_frame_new(1);
        spec.eval_context = shared;
        spec.condition_context = shared;
        llg_frame_retain(shared);
    }
    if (finish_mode == 4) {
        llg_expr_event_spec_t pair[] = {spec, spec};
        // Distinct contexts expose partial adoption; each owns a packed slot.
        pair[0].eval_context = llg_frame_new(1);
        pair[1].eval_context = llg_frame_new(1);
        return llg_arm_expressions(self, pair, 2);
    } else {
        return llg_arm_expressions(self, &spec, 1);
    }
}

LLG_PROBE_SIMPLE_PROCESS(waiting_process, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, arm_waiting_process(self));
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(writing_process, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_ba(&signal_value, next_value);
    LLG_PROBE_DONE();
}

static void force_evaluate(sv4_t* out) {
    sv4_copy(out, &next_value);
    llg_rt_finish();
}

LLG_PROBE_SIMPLE_PROCESS(forcing_process, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    const llg_force_part_t part = {&signal_value, NULL, 128, 0, 129, 0, 0};
    llg_force_expr_parts(&part, 1, 0, 0, force_evaluate, NULL, 0);
    LLG_CO_EXIT_CHECK(ch);
    LLG_PROBE_DONE();
}

int main(void) {
    for (finish_mode = 0; finish_mode < 7; ++finish_mode) {
        for (unsigned repeat = 0; repeat < 8; ++repeat) {
            llg_rt_init();
            evaluations = 0;
            sv4_replace(&signal_value, sv4_zero(129, 0));
            sv4_replace(&next_value, sv4_from_u64(1, 129, 0));
            if (finish_mode == 3) {
                llg_spawn(&forcing_process_desc, "force-callback-finish");
            } else {
                llg_spawn(&waiting_process_desc, "event-waiter");
                llg_spawn(&writing_process_desc, "event-writer");
            }
            llg_rt_run();
            CHECK(finish_mode == 3 || evaluations == (finish_mode == 4 ? 1u : 2u));
            llg_rt_cleanup();
            sv4_destroy(&signal_value);
            sv4_destroy(&next_value);
            CHECK(value_test_live() == 0);
            CHECK(value_test_bytes() == 0);
            CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
            CHECK(value_scope_index == NULL && value_scope_count == 0);
        }
    }
    puts("callback finish ownership: OK");
    return 0;
}
