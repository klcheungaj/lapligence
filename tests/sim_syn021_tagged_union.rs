//! SYN-021 finite packed tagged-union contexts through the public CLI.
use std::path::Path;

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn struct_payload_construction_ref_and_nba_work_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn021_tagged_union",
        "struct_contexts",
        "PASS syn021_struct_contexts\n",
        "llg: $finish at time 1000 at tb:126:9\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn tagged_union_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "struct_contexts",
        "not available in IEEE 2001",
        &["--edition", "2001"],
    );
}

#[test]
fn invalid_tagged_constructors_and_nonpacked_member_are_rejected() {
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "invalid_member_constructor",
        "no member named 'missing'",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "invalid_void_constructor",
        "cannot be assigned to type 'void'",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn021_tagged_union",
        "nonpacked_member",
        "packed members must be of integral type",
        &["--edition", "2009"],
    );
}

fn reject_fixture_tagged_access(
    fixture_file: &str,
    member: &str,
    expected_stdout: &str,
    location: &str,
    finish: &str,
    optimized: bool,
) {
    let fixture = fixture_file.strip_suffix(".sv").expect("SV fixture");
    let output = sim_cli::invoke_with_env(
        "syn021_tagged_union",
        fixture,
        optimized,
        &["--edition", "2009"],
        &[],
        &[],
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn021_tagged_union")
        .join(fixture_file);
    let expected_stderr = format!(
        "llg: runtime error: access to inactive tagged-union member {member} at {}:{location}\nllg: $finish at {finish}\n",
        source.display()
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
        String::from_utf8_lossy(&output.stderr),
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
            "time 0 at tb:18:9",
            optimized,
        );
        reject_fixture_tagged_access(
            "nested_inactive_write.sv",
            "Data",
            "AFTER_NESTED_WRITE calls=1 active_Other=155\n",
            "15:9",
            "time 0 at tb:17:9",
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
            "time 1000 at tb:13:9",
            optimized,
        );
    }
}
