//! Direct runtime boundary coverage for stochastic queues.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

use llg::sim;

const STOCHASTIC_PROBE: &str = r#"
#define LLG_MODEL_PROCESS_ABI 3
#include "llg_rt.h"

#include <stdio.h>
#include <stdlib.h>

static int failed;

typedef struct {
    llg_co_frame_t co;
    sv4_t status;
    sv4_t job;
    sv4_t info;
    sv4_t stat;
    sv4_t full;
} probe_frame_t;
LLG_CO_ROOT_FRAME_OK(probe_frame_t);

static const int64_t argument_numbers[] = {
    0, 1, 2, 3, 4, 5, 6, 7, 11, 12, 99, 101, 110, 120, 202, 303, 321
};
static sv4_t argument_values[sizeof(argument_numbers) / sizeof(argument_numbers[0])];

static sv4_t i32(int64_t number) {
    for (size_t i = 0; i < sizeof(argument_numbers) / sizeof(argument_numbers[0]); ++i)
        if (argument_numbers[i] == number) return argument_values[i];
    abort();
}

static void probe_values_destroy(probe_frame_t* F) {
    sv4_destroy(&F->status);
    sv4_destroy(&F->job);
    sv4_destroy(&F->info);
    sv4_destroy(&F->stat);
    sv4_destroy(&F->full);
}

#define CHECK(condition)                                                      \
    do {                                                                       \
        if (!(condition)) {                                                    \
            fprintf(stderr, "stochastic queue check failed at line %d: %s\n", \
                    __LINE__, #condition);                                    \
            failed = 1;                                                        \
            probe_values_destroy(F);                                          \
            llg_rt_request_finish();                                          \
            return LLG_CO_EXIT;                                                \
        }                                                                      \
    } while (0)

static llg_co_status_t probe(llg_co_frame_t*, llg_co_chain_t*);
static const llg_co_site_t probe_sites[4] = {{0}};
static const llg_co_desc_t probe_desc = {
    probe, "stochastic queue probe", sizeof(probe_frame_t), probe_sites, 4, 0
};

static llg_co_status_t probe(llg_co_frame_t* co, llg_co_chain_t* ch) {
    probe_frame_t* F = (probe_frame_t*)co;
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);
    LLG_CO_DISPATCH_BEGIN(co)
    LLG_CO_RESUME_CASE(1)
    LLG_CO_RESUME_CASE(2)
    LLG_CO_RESUME_CASE(3)
    LLG_CO_DISPATCH_END(co)
    F->status = sv4_from_i64(-1, 32);
    F->job = sv4_from_i64(0, 32);
    F->info = sv4_from_i64(0, 32);
    F->stat = sv4_from_i64(0, 32);
    F->full = (sv4_t)SV4_EMPTY;

    llg_q_initialize(i32(1), i32(1), i32(2), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_initialize(i32(2), i32(3), i32(2), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_BAD_TYPE);
    llg_q_initialize(i32(3), i32(1), i32(0), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_BAD_LENGTH);
    llg_q_initialize(i32(1), i32(2), i32(2), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_DUPLICATE_ID);

    llg_q_add(i32(1), i32(1), i32(101), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK);
    LLG_CO_AWAIT(co, ch, 1, llg_arm_time(self, 5));
    llg_q_add(i32(1), i32(2), i32(202), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK);
    sv4_replace(&F->full, llg_q_full(i32(1), &F->status));
    CHECK(sv4_to_i64(F->full) == 1 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_add(i32(1), i32(3), i32(303), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_FULL);

    LLG_CO_AWAIT(co, ch, 2, llg_arm_time(self, 5));
    llg_q_remove(i32(1), &F->job, &F->info, &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK && sv4_to_i64(F->job) == 1 &&
          sv4_to_i64(F->info) == 101);
    llg_q_exam(i32(1), i32(2), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 3 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(4), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 10 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(5), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 5 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(6), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 5 && sv4_to_i64(F->status) == LLG_Q_OK);
    sv4_assign(&F->stat, i32(321));
    llg_q_exam(i32(1), i32(7), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 321 && sv4_to_i64(F->status) == LLG_Q_BAD_TYPE);

    LLG_CO_AWAIT(co, ch, 3, llg_arm_time(self, 10));
    llg_q_add(i32(1), i32(3), i32(303), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(2), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 7 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(5), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 15 && sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_exam(i32(1), i32(6), &F->stat, &F->status);
    CHECK(sv4_to_i64(F->stat) == 8 && sv4_to_i64(F->status) == LLG_Q_OK);

    llg_q_remove(i32(1), &F->job, &F->info, &F->status);
    llg_q_remove(i32(1), &F->job, &F->info, &F->status);
    llg_q_remove(i32(1), &F->job, &F->info, &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_EMPTY);
    llg_q_add(i32(99), i32(1), i32(1), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_UNKNOWN_ID);
    llg_q_remove(i32(99), &F->job, &F->info, &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_UNKNOWN_ID);

    llg_q_initialize(i32(4), i32(2), i32(2), &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK);
    llg_q_add(i32(4), i32(11), i32(110), &F->status);
    llg_q_add(i32(4), i32(12), i32(120), &F->status);
    llg_q_remove(i32(4), &F->job, &F->info, &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK && sv4_to_i64(F->job) == 12 &&
          sv4_to_i64(F->info) == 120);
    llg_q_remove(i32(4), &F->job, &F->info, &F->status);
    CHECK(sv4_to_i64(F->status) == LLG_Q_OK && sv4_to_i64(F->job) == 11 &&
          sv4_to_i64(F->info) == 110);

    probe_values_destroy(F);
    llg_rt_request_finish();
    return LLG_CO_EXIT;
}

int main(void) {
    llg_rt_init();
    for (size_t i = 0; i < sizeof(argument_numbers) / sizeof(argument_numbers[0]); ++i)
        argument_values[i] = sv4_from_i64(argument_numbers[i], 32);
    llg_spawn(&probe_desc, "stochastic queue probe");
    llg_rt_run();
    llg_rt_cleanup();
    sv4_destroy_array(argument_values,
                      sizeof(argument_numbers) / sizeof(argument_numbers[0]));
    return failed;
}
"#;

#[test]
fn stochastic_queue_runtime_boundary_compiles_and_runs() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("runtime-stochastic").expect("create temp directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[("runtime_stochastic_probe.c", STOCHASTIC_PROBE)],
    )
    .expect("stochastic runtime probe should compile");
    let output = sim_harness::run_command(&mut Command::new(&executable), Duration::from_secs(60))
        .expect("stochastic runtime probe should run");
    assert!(
        output.status.success(),
        "stochastic probe failed: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "unexpected probe stdout: {output:?}"
    );
}
