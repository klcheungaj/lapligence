//! Process-level checks for fatal C runtime boundary conditions.

use std::process::Command;
use std::time::Duration;

use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

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
        &[("llg_rt_selftest.c", sim::rt::selftest_source())],
    )
    .expect("runtime boundary probe should compile");

    let output = sim_harness::run_command(
        Command::new(&executable).arg("--time-overflow-probe"),
        Duration::from_secs(10),
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
        Duration::from_secs(10),
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
        &[("llg_rt_selftest.c", sim::rt::selftest_source())],
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
        &[("llg_rt_selftest.c", sim::rt::selftest_source())],
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
