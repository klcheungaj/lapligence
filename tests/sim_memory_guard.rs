//! Executable-level coverage for the simulator driver's shared startup guard.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .env("LLG_MEMORY_LIMIT_MB", "not-a-number")
        .env_remove("LLG_MEMORY_WARNING_PERCENT")
        .env_remove("LLG_MEMORY_POLL_MS")
        .env_remove("LLG_MEMORY_ADDRESS_SPACE_LIMIT");
    command
}

#[test]
fn llg_installs_the_memory_policy_before_source_admission() {
    let directory = sim_harness::TempDir::new("memory_source_admission")
        .expect("create isolated source-admission directory");
    let missing = directory.path().join("missing.sv");
    let output = sim_harness::run_command(
        command()
            .current_dir(directory.path())
            .arg("--gen-only")
            .arg(&missing),
        Duration::from_secs(10),
    )
    .expect("run llg source-admission path");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "a failed admission emits no model");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let guard = stderr.find("memory safeguard: LLG_MEMORY_LIMIT_MB")
        .unwrap_or_else(|| panic!("missing safeguard diagnostic: {stderr}"));
    let source = stderr.find("missing.sv")
        .unwrap_or_else(|| panic!("missing source-admission diagnostic: {stderr}"));
    assert!(guard < source, "install policy before reading input: {stderr}");
    assert!(!stderr.contains("usage: llg"), "valid CLI syntax: {stderr}");
}

#[test]
fn llg_usage_handling_does_not_install_the_memory_policy() {
    let output = sim_harness::run_command(&mut command(), Duration::from_secs(10))
        .expect("run llg usage path");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty(), "usage output must stay on stderr");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage: llg"));
    assert!(!stderr.contains("memory safeguard"), "usage must be side-effect free: {stderr}");
}
