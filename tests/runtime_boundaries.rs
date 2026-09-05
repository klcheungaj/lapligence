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
