//! Process-level checks for fatal C runtime boundary conditions.

use std::process::Command;
use std::time::Duration;

use llg::sim;

use crate::sim_harness;

#[test]
fn scheduler_time_overflow_fails_with_a_diagnostic() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir =
        sim_harness::TempDir::new("runtime-boundary").expect("create runtime boundary directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[
            ("llg_rt_selftest.c", sim::rt::selftest_source()),
            ("selftest_co.h", sim::rt::selftest_support_source()),
        ],
    )
    .expect("runtime boundary probe should compile");

    let output = sim_harness::run_command(
        Command::new(&executable).arg("--time-overflow-probe"),
        sim_harness::MODEL_TIMEOUT,
    )
    .expect("runtime boundary probe should start");

    assert!(
        !output.status.success(),
        "overflow probe unexpectedly succeeded"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("simulation time overflow"),
        "missing overflow diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = sim_harness::run_command(
        Command::new(&executable).arg("--scaled-time-overflow-probe"),
        sim_harness::MODEL_TIMEOUT,
    )
    .expect("scaled-time boundary probe should start");
    assert!(
        !output.status.success(),
        "scaled-time overflow probe unexpectedly succeeded"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("scaled simulation time overflow"),
        "missing scaled-time overflow diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn process_budget_probes_cover_exact_limit_and_invalid_configuration() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("runtime-process-budget")
        .expect("create process-budget directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[
            ("llg_rt_selftest.c", sim::rt::selftest_source()),
            ("selftest_co.h", sim::rt::selftest_support_source()),
        ],
    )
    .expect("process-budget probe should compile");

    let exact = sim_harness::run_command(
        Command::new(&executable)
            .arg("--budget-finite-probe")
            .env("LLG_PROCESS_STEP_LIMIT", "4"),
        Duration::from_secs(10),
    )
    .expect("exact process-budget probe should start");
    assert!(exact.status.success(), "exact limit failed: {exact:?}");
    assert!(exact.stdout.is_empty());
    assert!(
        exact.stderr.is_empty(),
        "unexpected exact-limit output: {exact:?}"
    );

    let below = sim_harness::run_command(
        Command::new(&executable)
            .arg("--budget-finite-probe")
            .env("LLG_PROCESS_STEP_LIMIT", "3"),
        Duration::from_secs(10),
    )
    .expect("below-limit probe should start");
    assert_eq!(
        below.status.code(),
        Some(1),
        "below limit unexpectedly succeeded"
    );
    assert!(
        String::from_utf8_lossy(&below.stderr)
            .contains("nonconvergent zero-time execution in process `selftest.sv:1:1`"),
        "missing below-limit diagnostic: {:?}",
        below
    );

    let infinite = sim_harness::run_command(
        Command::new(&executable)
            .arg("--budget-infinite-probe")
            .env("LLG_PROCESS_STEP_LIMIT", "4"),
        Duration::from_secs(10),
    )
    .expect("infinite process-budget probe should start");
    assert!(
        infinite.status.success(),
        "infinite probe did not report failure: {infinite:?}"
    );
    assert!(
        String::from_utf8_lossy(&infinite.stderr)
            .contains("nonconvergent zero-time execution in process `selftest.sv:2:1`"),
        "missing infinite-loop diagnostic: {:?}",
        infinite
    );

    let invalid = sim_harness::run_command(
        Command::new(&executable)
            .arg("--budget-finite-probe")
            .env("LLG_PROCESS_STEP_LIMIT", "0"),
        Duration::from_secs(10),
    )
    .expect("invalid process-budget probe should start");
    assert_eq!(
        invalid.status.code(),
        Some(1),
        "invalid limit unexpectedly succeeded"
    );
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("invalid LLG_PROCESS_STEP_LIMIT"),
        "missing invalid-limit diagnostic: {:?}",
        invalid
    );
}

#[test]
fn stop_resume_hook_preserves_the_live_scheduler_until_explicit_resume() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir =
        sim_harness::TempDir::new("runtime-stop-resume").expect("create stop-resume directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[
            ("llg_rt_selftest.c", sim::rt::selftest_source()),
            ("selftest_co.h", sim::rt::selftest_support_source()),
        ],
    )
    .expect("stop-resume probe should compile");

    let output = sim_harness::run_command(
        Command::new(&executable).arg("--stop-resume-probe"),
        Duration::from_secs(10),
    )
    .expect("stop-resume probe should start");

    assert!(
        output.status.success(),
        "stop-resume probe failed: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "unexpected probe output: {output:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected probe diagnostics: {output:?}"
    );
}

#[cfg(unix)]
#[test]
fn low_posix_stack_limit_warns_before_simulation() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir =
        sim_harness::TempDir::new("runtime-stack-limit").expect("create stack-limit directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[
            ("llg_rt_selftest.c", sim::rt::selftest_source()),
            ("selftest_co.h", sim::rt::selftest_support_source()),
        ],
    )
    .expect("stack-limit probe should compile");

    let run_with_stack_kib = |kib: u32| {
        let kib_arg = kib.to_string();
        let output = sim_harness::run_command(
            Command::new("sh")
                .args([
                    "-c",
                    "ulimit -s \"$1\" && exec \"$2\" --stop-resume-probe",
                    "llg-stack-limit-probe",
                    kib_arg.as_str(),
                ])
                .arg(&executable),
            Duration::from_secs(10),
        )
        .expect("stack-limit probe should start");
        assert!(
            output.status.success(),
            "stack-limit probe with {kib} KiB failed: {output:?}"
        );
        String::from_utf8_lossy(&output.stderr).into_owned()
    };

    // The default estimate is 640 KiB (`LLG_HOST_STACK_ESTIMATE_BYTES`); a
    // limit equal to it is quiet and the next lower KiB step warns.
    for kib in [512, 639] {
        let stderr = run_with_stack_kib(kib);
        assert!(
            stderr.contains(&format!("host stack limit is {} bytes", kib * 1024))
                && stderr.contains("at least 655360 bytes")
                && stderr.contains("256-call depth guard"),
            "missing low-stack warning at {kib} KiB: {stderr}"
        );
    }
    let stderr = run_with_stack_kib(640);
    assert!(
        !stderr.contains("host stack limit"),
        "stack limit equal to the estimate warned: {stderr}"
    );

    // macOS reports its default 8 MiB main-thread stack as 8176 KiB after
    // reserving a guard page; that default must not warn.
    let stderr = run_with_stack_kib(8176);
    assert!(
        !stderr.contains("host stack limit"),
        "default macOS stack limit warned: {stderr}"
    );
}

/// A report of the coroutine library itself (`llg_co.c`: a synchronous callee
/// that suspends) reaches the model's reporter through the host hook and
/// prints in the message layout as an internal abort, followed by no
/// free-form line of the library's own.
#[test]
fn coroutine_library_reports_through_the_model_reporter() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    const PROBE: &str = r#"#include "llg_rt.h"

static llg_co_status_t suspends(llg_co_frame_t* co, llg_co_chain_t* ch) {
    (void)co;
    (void)ch;
    return LLG_CO_PENDING;
}

static const llg_co_desc_t suspends_desc = {suspends, "suspends", sizeof(llg_co_frame_t),
                                            NULL, 0, 0};

int main(void) {
    static llg_co_sync_t sync;
    (void)llg_co_sync_begin(&sync, &suspends_desc, NULL);
    (void)llg_co_sync_run(&sync);
    return 0;
}
"#;
    let dir = sim_harness::TempDir::new("runtime-coroutine-report")
        .expect("create coroutine report directory");
    let executable = sim::build::build_model_cmake(dir.path(), &[("probe.c", PROBE)])
        .expect("coroutine report probe should compile");
    let output =
        sim_harness::run_command(&mut Command::new(&executable), sim_harness::MODEL_TIMEOUT)
            .expect("coroutine report probe should start");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "probe unexpectedly succeeded");
    assert!(
        sim_harness::has_message(
            &stderr,
            "SIM-030",
            "",
            "invalid coroutine state in llg_co_sync_run: synchronous callee suspended"
        ),
        "missing internal-abort message: {stderr}"
    );
    assert!(!stderr.contains("llg: fatal:"), "{stderr}");
}
