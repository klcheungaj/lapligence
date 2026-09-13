//! End-to-end physical-time coverage through the public simulator executable.
//! The checked-in fixtures exercise femtosecond scheduling, the complete
//! standard unit range, checked overflow, and waveform tick/header fidelity.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

fn run_source(source: &Path, optimized: bool) -> (Output, sim_harness::TempDir) {
    let directory = sim_harness::TempDir::new("physical-time-cli").expect("temp dir");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.arg(source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .expect("simulator fixture should run");
    (output, directory)
}

fn run_waveform_fixture(optimized: bool) -> (String, String) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/physical_time/waveform_femtoseconds.sv");
    let (output, directory) = run_source(&source, optimized);
    assert!(
        output.status.success(),
        "waveform fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let vcd = std::fs::read_to_string(directory.path().join("trace.vcd"))
        .expect("waveform fixture should create VCD");
    (String::from_utf8_lossy(&output.stdout).into_owned(), vcd)
}

#[test]
fn physical_time_femtosecond_scopes_schedule_distinct_events() {
    sim_cli::run_case(
        "physical_time",
        "mixed_femtoseconds",
        "one=1\nten=1\nhundred=1\nps=1\nns=1\n",
        "",
        &[],
    );
}

#[test]
fn physical_time_large_second_scopes_do_not_collapse() {
    sim_cli::run_case(
        "physical_time",
        "large_seconds",
        "ten-global=10\nhundred-global=100\n",
        "",
        &[],
    );
}

#[test]
fn physical_time_first_fix_probe_runs_in_both_modes() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/Femtosecond_Delay.sv");
    for optimized in [false, true] {
        let (output, _directory) = run_source(&source, optimized);
        assert!(
            output.status.success(),
            "first-fix probe failed, optimized={optimized}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "CHECK: elapsed=0.001\n"
        );
        assert!(
            output.stderr.is_empty(),
            "first-fix probe emitted diagnostics: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn physical_time_overflow_is_rejected_before_model_build() {
    sim_cli::reject_case(
        "physical_time",
        "overflow",
        "scales past the 64-bit tick range",
    );
}

#[test]
fn physical_time_waveform_uses_femtosecond_ticks_in_both_modes() {
    for optimized in [false, true] {
        let (stdout, vcd) = run_waveform_fixture(optimized);
        assert!(stdout.is_empty(), "unexpected simulator output: {stdout:?}");
        assert!(vcd.contains("$timescale 1fs $end"), "{vcd}");
        assert!(vcd.contains("#10\n"), "{vcd}");
        assert!(!vcd.contains("#1000\n"), "{vcd}");
    }
}
