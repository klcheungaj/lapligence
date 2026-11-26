//! Executable-level coverage for the simulator driver's shared startup guard.

use std::process::Command;
use std::time::Duration;

use crate::sim_harness;

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
    assert!(
        output.stdout.is_empty(),
        "a failed admission emits no model"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let guard = stderr
        .find("memory safeguard: LLG_MEMORY_LIMIT_MB")
        .unwrap_or_else(|| panic!("missing safeguard diagnostic: {stderr}"));
    let source = stderr
        .find("missing.sv")
        .unwrap_or_else(|| panic!("missing source-admission diagnostic: {stderr}"));
    assert!(
        guard < source,
        "install policy before reading input: {stderr}"
    );
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
    assert!(
        !stderr.contains("memory safeguard"),
        "usage must be side-effect free: {stderr}"
    );
}

/// Run `llg --gen-only` on a small design under `command`'s limits.
#[cfg(target_os = "linux")]
fn generate(mut command: Command, directory: &sim_harness::TempDir) -> (Option<i32>, String) {
    std::fs::write(
        directory.path().join("tb.sv"),
        "`timescale 1ns/1ns\nmodule tb; initial $display(\"ok\"); endmodule\n",
    )
    .expect("write the design");
    let output = sim_harness::run_command(
        command
            .current_dir(directory.path())
            .args(["--gen-only", "tb.sv"]),
        Duration::from_secs(60),
    )
    .expect("generate a small design");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The pipeline's stack is address space the design never uses, so it comes
/// on top of a limit smaller than it.
#[cfg(target_os = "linux")]
#[test]
fn llg_address_space_limit_does_not_count_the_pipeline_stack() {
    const LIMIT_MIB: usize = 512;
    const { assert!(LIMIT_MIB << 20 < llg::stack::PIPELINE_STACK_BYTES) };
    let directory = sim_harness::TempDir::new("memory_pipeline_stack").expect("test directory");
    let mut command = command();
    command
        .env("LLG_MEMORY_LIMIT_MB", LIMIT_MIB.to_string())
        .env("LLG_MEMORY_ADDRESS_SPACE_LIMIT", "1");
    let (status, stderr) = generate(command, &directory);
    assert_eq!(status, Some(0), "{stderr}");
    assert!(stderr.contains("mode=address-space+watchdog"), "{stderr}");
}

/// Address space the process holds untouched when it installs the limit is
/// not memory either: mimalloc, the allocator of musl builds, reserves a
/// 1 GiB arena at its first allocation, which left the limit no room for the
/// watchdog's thread. On glibc the same reserve is a heap that keeps
/// `top_pad` bytes spare; musl ignores the variable and has its arena.
#[cfg(target_os = "linux")]
#[test]
fn llg_address_space_limit_does_not_count_an_allocator_reserve() {
    const LIMIT_MIB: usize = 512;
    const RESERVE: usize = 1 << 30;
    let directory = sim_harness::TempDir::new("memory_allocator_reserve").expect("test directory");
    let mut command = command();
    command
        .env("LLG_MEMORY_LIMIT_MB", LIMIT_MIB.to_string())
        .env("LLG_MEMORY_ADDRESS_SPACE_LIMIT", "1")
        .env("GLIBC_TUNABLES", format!("glibc.malloc.top_pad={RESERVE}"));
    let (status, stderr) = generate(command, &directory);
    assert_eq!(status, Some(0), "{stderr}");
    assert!(stderr.contains("mode=address-space+watchdog"), "{stderr}");
}

/// Without room for the stack nothing is compiled, and `llg` says why.
#[cfg(target_os = "linux")]
#[test]
fn llg_reports_an_address_space_too_small_for_the_pipeline() {
    let directory = sim_harness::TempDir::new("memory_no_pipeline").expect("test directory");
    let mut command = Command::new("sh");
    command.env_remove("LLG_MEMORY_LIMIT_MB").args([
        "-c",
        &format!(
            "ulimit -v {}; exec \"$0\" \"$@\"",
            llg::stack::PIPELINE_STACK_BYTES / 1024
        ),
        env!("CARGO_BIN_EXE_llg"),
    ]);
    let (status, stderr) = generate(command, &directory);
    assert_eq!(status, Some(1), "{stderr}");
    assert!(
        stderr.contains("[HOST-005] cannot start a thread with 1024 MiB of address space"),
        "{stderr}"
    );
}
