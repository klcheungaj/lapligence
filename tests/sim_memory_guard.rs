//! Executable-level coverage for the simulator driver's shared startup guard.

use std::process::Command;

#[test]
fn llg_installs_the_memory_policy_before_usage_handling() {
    let output = Command::new(env!("CARGO_BIN_EXE_llg"))
        .env("LLG_MEMORY_LIMIT_MB", "not-a-number")
        .env_remove("LLG_MEMORY_WARNING_PERCENT")
        .env_remove("LLG_MEMORY_POLL_MS")
        .env_remove("LLG_MEMORY_ADDRESS_SPACE_LIMIT")
        .output()
        .expect("run llg usage path");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty(), "usage output must stay on stderr");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("memory safeguard: LLG_MEMORY_LIMIT_MB"),
        "simulator startup should report the shared safeguard warning: {stderr}"
    );
    assert!(stderr.contains("usage: llg"));
}
