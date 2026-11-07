//! Checked runtime diagnostics and single-evaluation selected accesses for
//! packed tagged unions.
use std::path::Path;

use crate::sim_cli;
use crate::sim_harness;

fn run(fixture: &str, optimized: bool) -> std::process::Output {
    sim_cli::invoke_with_env(
        "review_bundle",
        fixture,
        optimized,
        &["--edition", "sv2009"],
        &[],
        &[],
    )
}

fn assert_inactive_access(
    fixture: &str,
    member: &str,
    expected_stdout: &str,
    source_location: &str,
    finish: (&str, &str),
    optimized: bool,
) {
    let output = run(fixture, optimized);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        matches!(output.status.code(), Some(1)),
        "{fixture}: expected exit code 1, got {:?}, stderr={stderr}",
        output.status.code()
    );
    assert_eq!(
        stdout, expected_stdout,
        "{fixture}: unexpected post-error output"
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/review_bundle")
        .join(format!("{fixture}.sv"));
    let name = sim_harness::file_name(&source);
    let (finish_at, finish_time) = finish;
    let expected_stderr = format!(
        "ERROR: {name}:{source_location}: [SIM-009] access to inactive tagged-union member {member}\n\
         INFO: {name}:{finish_at}: [SIM-001] (tb @ {finish_time}) $finish called\n\
         INFO: [SIM-000] 1 error(s), 0 warning(s), 1 info(s)\n"
    );
    assert_eq!(
        sim_harness::strip_lint_reports(&output.stderr),
        expected_stderr,
        "{fixture}: unexpected runtime diagnostic or source location"
    );
}

#[test]
fn inactive_tagged_member_reads_and_writes_report_runtime_errors() {
    for optimized in [false, true] {
        assert_inactive_access(
            "r01_tagged_inactive_read",
            "B",
            "AFTER_INACTIVE_READ got=xx\n",
            "8:11",
            ("10:5", "0 ps"),
            optimized,
        );
        assert_inactive_access(
            "r01_tagged_inactive_write",
            "B",
            "AFTER_INACTIVE_WRITE active_A=11\n",
            "7:5",
            ("9:5", "0 ps"),
            optimized,
        );
        assert_inactive_access(
            "r02_tagged_selected_wrong_write",
            "A",
            "AFTER_INACTIVE_SELECTED_NBA calls=1 active_B=55\n",
            "14:5",
            ("19:5", "1 ns"),
            optimized,
        );
        assert_inactive_access(
            "r01_tagged_ref_inactive_write",
            "A",
            "AFTER_INACTIVE_REF_WRITE active_B=55\n",
            "6:5",
            ("12:5", "0 ps"),
            optimized,
        );
    }
}

#[test]
fn tagged_member_receiver_and_selected_nba_indices_are_evaluated_once() {
    for optimized in [false, true] {
        let read = run("r02_tagged_selector_once", optimized);
        assert!(
            read.status.success(),
            "selector read optimized={optimized}: {}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&read.stdout),
            "PASS r02_tagged_selector_once\n"
        );

        let write = run("r02_tagged_selected_write", optimized);
        assert!(
            write.status.success(),
            "selected NBA optimized={optimized}: {}",
            String::from_utf8_lossy(&write.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&write.stdout),
            "PASS r02_tagged_selected_write calls=1\n"
        );

        let reference = run("r01_tagged_ref_active", optimized);
        assert!(
            reference.status.success(),
            "tagged ref optimized={optimized}: {}",
            String::from_utf8_lossy(&reference.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&reference.stdout),
            "PASS r01_tagged_ref_active calls=1\n"
        );
    }
}

#[test]
fn repeated_nested_tagged_guards_release_value_owners() {
    // Functional success alone does not establish leak freedom. This test
    // inherits LLG_CFLAGS/ASAN_OPTIONS for the generated-model sanitizer lane.
    sim_cli::run_case_with_args(
        "review_bundle",
        "n05_tagged_guard_stress",
        "PASS n05_tagged_guard_stress\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
