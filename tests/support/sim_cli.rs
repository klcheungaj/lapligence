//! File-based simulator acceptance tests through the public executable.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use super::sim_harness;

fn invoke(suite: &str, fixture: &str, optimized: bool) -> Output {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(suite)
        .join(format!("{fixture}.sv"));
    assert!(source.is_file(), "missing fixture: {}", source.display());
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.arg(source);
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("{suite}/{fixture}, optimized={optimized}: {error}"))
}

pub(crate) fn run_case(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke(suite, fixture, optimized);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{label}: {stderr}");
        let actual = String::from_utf8_lossy(&output.stdout);
        if actual != expected {
            let line = actual
                .lines()
                .zip(expected.lines())
                .position(|(actual, expected)| actual != expected)
                .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
            panic!(
                "{label}: line {}: expected {:?}, got {:?} ({} versus {} lines)\n{stderr}",
                line + 1,
                expected.lines().nth(line),
                actual.lines().nth(line),
                expected.lines().count(),
                actual.lines().count()
            );
        }
        let mut warnings = Vec::new();
        let mut runtime_stderr = String::new();
        for line in stderr.lines() {
            if let Some(warning) = line.strip_prefix("llg: warning: ") {
                warnings.push(warning);
            } else if !line.starts_with("Warning: ") {
                // Legal conformance probes intentionally provoke frontend
                // width/sign/range warnings; lowering warnings remain exact.
                runtime_stderr.push_str(line);
                runtime_stderr.push('\n');
            }
        }
        warnings.sort_unstable();
        let mut expected_warnings = expected_warnings.to_vec();
        expected_warnings.sort_unstable();
        assert_eq!(warnings, expected_warnings, "{label}");
        assert_eq!(runtime_stderr, expected_stderr, "{label}");
    }
}

pub(crate) fn reject_case(suite: &str, fixture: &str, diagnostic: &str) {
    for optimized in [false, true] {
        let output = invoke(suite, fixture, optimized);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
    }
}
