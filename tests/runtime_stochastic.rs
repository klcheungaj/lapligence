//! Direct runtime boundary coverage for stochastic queues.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

use llg::sim;

const STOCHASTIC_PROBE: &str = r#"
#include "llg_rt.h"

#include <stdio.h>

#define CHECK(self, condition)                                                \
    do {                                                                       \
        if (!(condition)) {                                                    \
            fprintf(stderr, "stochastic queue check failed at line %d: %s\n", \
                    __LINE__, #condition);                                    \
            failed = 1;                                                        \
            llg_rt_request_finish();                                          \
            llg_proc_done(self);                                               \
        }                                                                      \
    } while (0)

static int failed;

static void probe(llg_proc_t* self) {
    sv4_t status = sv4_from_i64(-1, 32);
    sv4_t job = sv4_from_i64(0, 32);
    sv4_t info = sv4_from_i64(0, 32);
    sv4_t stat = sv4_from_i64(0, 32);

    llg_q_initialize(sv4_from_i64(1, 32), sv4_from_i64(1, 32),
                     sv4_from_i64(2, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK);
    llg_q_initialize(sv4_from_i64(2, 32), sv4_from_i64(3, 32),
                     sv4_from_i64(2, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_BAD_TYPE);
    llg_q_initialize(sv4_from_i64(3, 32), sv4_from_i64(1, 32),
                     sv4_from_i64(0, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_BAD_LENGTH);
    llg_q_initialize(sv4_from_i64(1, 32), sv4_from_i64(2, 32),
                     sv4_from_i64(2, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_DUPLICATE_ID);

    llg_q_add(sv4_from_i64(1, 32), sv4_from_i64(1, 32),
              sv4_from_i64(101, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK);
    llg_wait_time(5);
    llg_q_add(sv4_from_i64(1, 32), sv4_from_i64(2, 32),
              sv4_from_i64(202, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK);
    sv4_t full = llg_q_full(sv4_from_i64(1, 32), &status);
    CHECK(self, sv4_to_i64(full) == 1 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_add(sv4_from_i64(1, 32), sv4_from_i64(3, 32),
              sv4_from_i64(303, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_FULL);

    llg_wait_time(5);
    llg_q_remove(sv4_from_i64(1, 32), &job, &info, &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK && sv4_to_i64(job) == 1 &&
          sv4_to_i64(info) == 101);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(2, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 3 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(4, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 10 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(5, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 5 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(6, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 5 && sv4_to_i64(status) == LLG_Q_OK);
    stat = sv4_from_i64(321, 32);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(7, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 321 && sv4_to_i64(status) == LLG_Q_BAD_TYPE);

    llg_wait_time(10);
    llg_q_add(sv4_from_i64(1, 32), sv4_from_i64(3, 32),
              sv4_from_i64(303, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(2, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 7 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(5, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 15 && sv4_to_i64(status) == LLG_Q_OK);
    llg_q_exam(sv4_from_i64(1, 32), sv4_from_i64(6, 32), &stat, &status);
    CHECK(self, sv4_to_i64(stat) == 8 && sv4_to_i64(status) == LLG_Q_OK);

    llg_q_remove(sv4_from_i64(1, 32), &job, &info, &status);
    llg_q_remove(sv4_from_i64(1, 32), &job, &info, &status);
    llg_q_remove(sv4_from_i64(1, 32), &job, &info, &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_EMPTY);
    llg_q_add(sv4_from_i64(99, 32), sv4_from_i64(1, 32),
              sv4_from_i64(1, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_UNKNOWN_ID);
    llg_q_remove(sv4_from_i64(99, 32), &job, &info, &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_UNKNOWN_ID);

    llg_q_initialize(sv4_from_i64(4, 32), sv4_from_i64(2, 32),
                     sv4_from_i64(2, 32), &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK);
    llg_q_add(sv4_from_i64(4, 32), sv4_from_i64(11, 32),
              sv4_from_i64(110, 32), &status);
    llg_q_add(sv4_from_i64(4, 32), sv4_from_i64(12, 32),
              sv4_from_i64(120, 32), &status);
    llg_q_remove(sv4_from_i64(4, 32), &job, &info, &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK && sv4_to_i64(job) == 12 &&
          sv4_to_i64(info) == 120);
    llg_q_remove(sv4_from_i64(4, 32), &job, &info, &status);
    CHECK(self, sv4_to_i64(status) == LLG_Q_OK && sv4_to_i64(job) == 11 &&
          sv4_to_i64(info) == 110);

    llg_rt_request_finish();
    llg_proc_done(self);
}

int main(void) {
    llg_rt_init();
    llg_spawn(probe, "stochastic queue probe");
    llg_rt_run();
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
