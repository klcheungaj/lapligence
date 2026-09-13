//! Checked-in simulator fixtures for repeated wait-free processes and
//! controlled zero-time nonconvergence.

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

fn invoke(fixture: &str, optimized: bool, env: &[(&str, &str)]) -> Output {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/nonconvergence")
        .join(format!("{fixture}.sv"));
    assert!(source.is_file(), "missing fixture: {}", source.display());
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    for name in [
        "LLG_ZERO_LOOP_LIMIT",
        "LLG_PROCESS_STEP_LIMIT",
        "LLG_NONCONVERGENCE_LIMIT",
    ] {
        command.env_remove(name);
    }
    command.envs(env.iter().copied());
    command.arg(source);
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("nonconvergence/{fixture}, optimized={optimized}: {error}"))
}

fn assert_success(fixture: &str, expected: &[u8]) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke(fixture, optimized, &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{fixture}, optimized={optimized}: {stderr}"
        );
        assert_eq!(
            output.stdout, expected,
            "{fixture}, optimized={optimized}: {stderr}"
        );
        assert!(
            stderr.lines().all(|line| line.starts_with("Warning: ")),
            "{stderr}"
        );
    }
}

#[test]
fn finite_zero_time_loop_completes_in_both_modes() {
    assert_success("finite_zero_time_loop", b"finite=200000\n");
}

#[test]
fn wait_free_always_reports_source_bearing_nonconvergence() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke(
            "wait_free_always",
            optimized,
            &[("LLG_PROCESS_STEP_LIMIT", "8")],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(output.stdout.is_empty(), "unexpected stdout: {output:?}");
        assert!(
            stderr.contains("nonconvergent zero-time execution"),
            "missing nonconvergence diagnostic: {stderr}"
        );
        assert!(
            stderr.contains("wait_free_always.sv"),
            "diagnostic lacks source location: {stderr}"
        );
    }
}

#[test]
fn always_comb_and_latch_keep_time_zero_behavior() {
    assert_success(
        "always_kinds_time_zero",
        b"initial comb=1 latch=0\nupdated comb=0 latch=0\n",
    );
}

#[test]
fn budget_limit_boundary_and_configuration_are_checked() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let exact = invoke(
            "finite_zero_time_loop",
            optimized,
            &[("LLG_PROCESS_STEP_LIMIT", "200000")],
        );
        assert!(exact.status.success(), "exact boundary failed: {exact:?}");
        assert_eq!(exact.stdout, b"finite=200000\n");

        let below = invoke(
            "finite_zero_time_loop",
            optimized,
            &[("LLG_PROCESS_STEP_LIMIT", "199999")],
        );
        let below_stderr = String::from_utf8_lossy(&below.stderr);
        assert_eq!(below.status.code(), Some(1), "{below_stderr}");
        assert!(
            below_stderr.contains("nonconvergent zero-time execution"),
            "missing boundary diagnostic: {below_stderr}"
        );

        for (name, value) in [
            ("LLG_PROCESS_STEP_LIMIT", "0"),
            ("LLG_PROCESS_STEP_LIMIT", "18446744073709551616"),
            ("LLG_PROCESS_STEP_LIMIT", "not-a-number"),
            ("LLG_ZERO_LOOP_LIMIT", "0"),
            ("LLG_ZERO_LOOP_LIMIT", "18446744073709551616"),
            ("LLG_ZERO_LOOP_LIMIT", "not-a-number"),
        ] {
            let invalid = invoke("finite_zero_time_loop", optimized, &[(name, value)]);
            let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
            assert_eq!(
                invalid.status.code(),
                Some(1),
                "{name}={value}: {invalid_stderr}"
            );
            assert!(
                invalid_stderr.contains(&format!("invalid {name}")),
                "{name}={value}: missing config diagnostic: {invalid_stderr}"
            );
        }

        let shared = invoke(
            "finite_zero_time_loop",
            optimized,
            &[("LLG_ZERO_LOOP_LIMIT", "200000")],
        );
        assert!(
            shared.status.success(),
            "shared scheduler/process limit failed: {:?}",
            String::from_utf8_lossy(&shared.stderr)
        );

        let alias = invoke(
            "finite_zero_time_loop",
            optimized,
            &[("LLG_NONCONVERGENCE_LIMIT", "200000")],
        );
        assert!(alias.status.success(), "limit alias failed: {alias:?}");
        assert_eq!(alias.stdout, b"finite=200000\n");

        let invalid_alias = invoke(
            "finite_zero_time_loop",
            optimized,
            &[("LLG_NONCONVERGENCE_LIMIT", "0")],
        );
        let invalid_alias_stderr = String::from_utf8_lossy(&invalid_alias.stderr);
        assert_eq!(
            invalid_alias.status.code(),
            Some(1),
            "{invalid_alias_stderr}"
        );
        assert!(
            invalid_alias_stderr.contains("invalid LLG_NONCONVERGENCE_LIMIT"),
            "missing alias config diagnostic: {invalid_alias_stderr}"
        );
    }
}
