//! Direct C-runtime coverage for the complete event-region state machine.

use std::process::Command;
use std::time::Duration;

use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn region_callbacks_trace_fixed_point_and_read_only_boundaries() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir =
        sim_harness::TempDir::new("runtime-regions").expect("create runtime region directory");
    let executable = sim::build::build_model_cmake(
        dir.path(),
        &[("llg_rt_selftest.c", sim::rt::selftest_source())],
    )
    .expect("region probe should compile");

    let output = sim_harness::run_command(
        Command::new(&executable).arg("--region-probe"),
        Duration::from_secs(10),
    )
    .expect("region probe should start");

    assert!(output.status.success(), "region probe failed: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "unexpected probe output: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("illegal signal write"));
    assert!(stderr.contains("illegal callback scheduling"));
}
