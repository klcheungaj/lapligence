//! File-based simulator acceptance tests through the public executable.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use super::sim_harness;

fn invoke_with_args(suite: &str, fixture: &str, optimized: bool, args: &[&str]) -> Output {
    invoke_with_env(suite, fixture, optimized, args, &[], &[])
}

/// Invoke a fixture after source files which must precede it in a merged
/// compilation unit. The paths are still checked-in fixture stems, so the
/// public CLI observes the same admission and file ordering as a user command.
pub(crate) fn invoke_with_source_prefix(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    optimized: bool,
    args: &[&str],
) -> Output {
    invoke_with_source_prefix_and_runtime_args(suite, fixture, prefix, optimized, args, &[])
}

fn invoke_with_source_prefix_and_runtime_args(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    optimized: bool,
    args: &[&str],
    runtime_args: &[&str],
) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(suite);
    let source = root.join(format!("{fixture}.sv"));
    assert!(source.is_file(), "missing fixture: {}", source.display());
    let prefix_paths: Vec<_> = prefix
        .iter()
        .map(|stem| {
            let path = root.join(format!("{stem}.sv"));
            assert!(path.is_file(), "missing fixture: {}", path.display());
            path
        })
        .collect();
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.args(args);
    for path in prefix_paths {
        command.arg(path);
    }
    command.arg(source);
    if !runtime_args.is_empty() {
        command.arg("--").args(runtime_args);
    }
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("{suite}/{fixture}, optimized={optimized}: {error}"))
}

/// Invoke one checked-in fixture after copying its independent memory-file
/// inputs into the child working directory. Memory tasks resolve relative
/// paths from that directory, so the files must be installed after creating
/// the isolated directory and before launching the public CLI.
fn invoke_with_files(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    files: &[(&str, &str)],
) -> Output {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(suite)
        .join(format!("{fixture}.sv"));
    assert!(source.is_file(), "missing fixture: {}", source.display());
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    for (name, contents) in files {
        std::fs::write(directory.path().join(name), contents)
            .unwrap_or_else(|error| panic!("{suite}/{fixture}: write {name}: {error}"));
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.args(args).arg(source);
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("{suite}/{fixture}, optimized={optimized}: {error}"))
}

/// Invoke one checked-in simulator fixture with explicit child-process
/// environment controls. `remove_env` is applied after `envs`, so tests can
/// guarantee that a host configuration variable is absent even when the test
/// runner inherited it.
pub(crate) fn invoke_with_env(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    envs: &[(&str, &str)],
    remove_env: &[&str],
) -> Output {
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
    command.args(args);
    command.arg(source);
    command.envs(envs.iter().copied());
    for variable in remove_env {
        command.env_remove(variable);
    }
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("{suite}/{fixture}, optimized={optimized}: {error}"))
}

fn invoke(suite: &str, fixture: &str, optimized: bool) -> Output {
    invoke_with_args(suite, fixture, optimized, &[])
}

fn invoke_with_runtime_args(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    runtime_args: &[&str],
) -> Output {
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
    command.args(args).arg(source).arg("--").args(runtime_args);
    sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("{suite}/{fixture}, optimized={optimized}: {error}"))
}

fn assert_case_output(
    output: Output,
    label: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
) {
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

pub(crate) fn run_case(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
) {
    run_case_with_args(
        suite,
        fixture,
        expected,
        expected_stderr,
        expected_warnings,
        &[],
    );
}

pub(crate) fn run_case_with_runtime_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    runtime_args: &[&str],
) {
    run_case_with_cli_and_runtime_args(
        suite,
        fixture,
        expected,
        expected_stderr,
        &[],
        runtime_args,
    );
}

pub(crate) fn run_case_with_cli_and_runtime_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    args: &[&str],
    runtime_args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_runtime_args(suite, fixture, optimized, args, runtime_args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, &[]);
    }
}

pub(crate) fn run_case_with_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_files(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
    files: &[(&str, &str)],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_files(suite, fixture, optimized, args, files);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_source_prefix(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_source_prefix(suite, fixture, prefix, optimized, args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_source_prefix_and_runtime_args(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    expected: &str,
    expected_stderr: &str,
    args: &[&str],
    runtime_args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_source_prefix_and_runtime_args(
            suite,
            fixture,
            prefix,
            optimized,
            args,
            runtime_args,
        );
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, &[]);
    }
}

pub(crate) fn reject_case(suite: &str, fixture: &str, diagnostic: &str) {
    reject_case_with_args(suite, fixture, diagnostic, &[]);
}

pub(crate) fn reject_case_with_args(suite: &str, fixture: &str, diagnostic: &str, args: &[&str]) {
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
    }
}

pub(crate) fn reject_case_with_exact_stderr(
    suite: &str,
    fixture: &str,
    expected_stderr: &str,
    args: &[&str],
) {
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert_eq!(stderr, expected_stderr, "{fixture}: unexpected diagnostic");
    }
}

pub(crate) fn reject_case_with_runtime_args(
    suite: &str,
    fixture: &str,
    diagnostic: &str,
    runtime_args: &[&str],
) {
    for optimized in [false, true] {
        let output = invoke_with_runtime_args(suite, fixture, optimized, &[], runtime_args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
    }
}
