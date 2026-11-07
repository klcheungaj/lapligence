//! SYN-021 finite packed tagged-union contexts through the public CLI.
use std::path::Path;

use crate::sim_cli;
use crate::sim_harness;

#[test]
fn struct_payload_construction_ref_and_nba_work_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn021_tagged_union",
        "struct_contexts",
        "PASS syn021_struct_contexts\n",
        "INFO: struct_contexts.sv:126:9: [SIM-001] (tb @ 1 ns) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn tagged_union_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "struct_contexts",
        "not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

#[test]
fn invalid_tagged_constructors_and_nonpacked_member_are_rejected() {
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "invalid_member_constructor",
        "no member named 'missing'",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "invalid_void_constructor",
        "cannot be assigned to type 'void'",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "nonpacked_member",
        "packed members must be of integral type",
        &["--edition", "sv2009"],
    );
}

fn reject_fixture_tagged_access(
    fixture_file: &str,
    member: &str,
    expected_stdout: &str,
    location: &str,
    finish: (&str, &str),
    optimized: bool,
) {
    let fixture = fixture_file.strip_suffix(".sv").expect("SV fixture");
    let output = sim_cli::invoke_with_env(
        "syn021_tagged_union",
        fixture,
        optimized,
        &["--edition", "sv2009"],
        &[],
        &[],
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn021_tagged_union")
        .join(fixture_file);
    let name = sim_harness::file_name(&source);
    let (finish_at, finish_time) = finish;
    let expected_stderr = format!(
        "ERROR: {name}:{location}: [SIM-009] access to inactive tagged-union member {member}\n\
         INFO: {name}:{finish_at}: [SIM-001] (tb @ {finish_time}) $finish called\n\
         INFO: [SIM-000] 1 error(s), 0 warning(s), 1 info(s)\n"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "{fixture}, optimized={optimized}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected_stdout,
        "{fixture}, optimized={optimized}"
    );
    assert_eq!(
        sim_harness::strip_lint_reports(&output.stderr),
        expected_stderr,
        "{fixture}, optimized={optimized}"
    );
}

#[test]
fn nested_wrong_tags_report_exact_read_and_write_locations() {
    for optimized in [false, true] {
        reject_fixture_tagged_access(
            "nested_inactive_read.sv",
            "A",
            "AFTER_NESTED_READ calls=1 got=xx active_B=5a\n",
            "16:15",
            ("18:9", "0 ps"),
            optimized,
        );
        reject_fixture_tagged_access(
            "nested_inactive_write.sv",
            "Data",
            "AFTER_NESTED_WRITE calls=1 active_Other=155\n",
            "15:9",
            ("17:9", "0 ps"),
            optimized,
        );
    }
}

#[test]
fn wrong_tag_at_nba_issue_stays_invalid_after_retagging() {
    for optimized in [false, true] {
        reject_fixture_tagged_access(
            "nba_wrong_at_issue.sv",
            "A",
            "AFTER_WRONG_ISSUE active_A=11\n",
            "7:9",
            ("13:9", "1 ns"),
            optimized,
        );
    }
}
