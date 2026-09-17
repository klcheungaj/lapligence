/* Real-coroutine nonlocal exits must release runtime snapshots as well as
 * generated temporaries. Test before/after writing the callback result. */
#include "llg_rt.c"
#include "probe.h"

static sv4_t signal_value = SV4_EMPTY;
static sv4_t next_value = SV4_EMPTY;
static unsigned evaluations;
static int finish_mode;

static void evaluate(sv4_t* out, void* context) {
    (void)context;
    ++evaluations;
    if (evaluations == 1 && finish_mode == 4) llg_rt_finish();
    if (evaluations == 2 && finish_mode == 1) llg_rt_finish();
    sv4_copy(out, &signal_value);
    if (evaluations == 2 && finish_mode == 2) llg_rt_finish();
}

static void condition(sv4_t* out, void* context) {
    (void)context;
    sv4_copy(out, &next_value);
    llg_rt_finish();
}

static void waiting_process(llg_proc_t* self) {
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
        llg_wait_expressions(pair, 2);
    } else {
        llg_wait_expressions(&spec, 1);
    }
    llg_proc_done(self);
}

static void writing_process(llg_proc_t* self) {
    llg_wait_time(1);
    llg_ba(&signal_value, next_value);
    llg_proc_done(self);
}

static void force_evaluate(sv4_t* out) {
    sv4_copy(out, &next_value);
    llg_rt_finish();
}

static void forcing_process(llg_proc_t* self) {
    const llg_force_part_t part = {&signal_value, NULL, 128, 0, 129, 0, 0};
    llg_force_expr_parts(&part, 1, 0, 0, force_evaluate, NULL, 0);
    llg_proc_done(self);
}

int main(void) {
    for (finish_mode = 0; finish_mode < 7; ++finish_mode) {
        for (unsigned repeat = 0; repeat < 8; ++repeat) {
            llg_rt_init();
            evaluations = 0;
            sv4_replace(&signal_value, sv4_zero(129, 0));
            sv4_replace(&next_value, sv4_from_u64(1, 129, 0));
            if (finish_mode == 3) {
                llg_spawn(forcing_process, "force-callback-finish");
            } else {
                llg_spawn(waiting_process, "event-waiter");
                llg_spawn(writing_process, "event-writer");
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
