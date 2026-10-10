use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_103";

const BOUNDS_REPORT: &str =
    "llg: fixed streaming target selector is unknown, empty, or outside declared bounds\n";

/// Run a fixture whose run-time streaming errors make it exit 1. Its results
/// are on stderr, interleaved with the reports; compare that stream exactly
/// in both HDL optimizer modes and on every available value backend.
fn assert_runtime_errors(fixture: &str, expected: &str) {
    sim_cli::run_case_checked_matrix(SUITE, fixture, &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let runtime: String = stderr
            .lines()
            .filter(|line| !crate::sim_harness::is_compile_report_line(line))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert!(output.stdout.is_empty(), "{label}: {output:?}");
        assert_eq!(runtime, expected, "{label}");
    });
}

#[test]
fn runtime_with_ranges_copy_out_from_statement_calls() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_103/copyout_with.out");
    sim_cli::run_case(SUITE, "copyout_with", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "copyout_with", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "copyout_with", expected);
}

#[test]
fn copy_out_ranges_outside_the_target_write_the_valid_part_and_report() {
    assert_runtime_errors(
        "copyout_bounds",
        &format!(
            "{BOUNDS_REPORT}partial 00 11 22\n\
             {BOUNDS_REPORT}low 22 33\n\
             {BOUNDS_REPORT}unknown 22 33\n"
        ),
    );
    sim_cli::reject_case(SUITE, "copyout_bounds", "outside declared bounds");
}

#[test]
fn mixed_state_with_targets_convert_member_wise() {
    assert_runtime_errors(
        "mixed_state",
        &format!(
            "runtime 0 x | a b | 0 z\n\
             constant 0 z | 5 x | 0 z\n\
             {BOUNDS_REPORT}bounds 0 z | 5 x | 0 x\n\
             queued 1 2 | 0 1 | 0 2\n\
             reversed 0 1 | 0 z | 0 2\n\
             copyout 0 1 | 0 9 | 0 3\n\
             local 10 010 x 11 | z0 100 z 01 | 00 000 0 00\n"
        ),
    );
    sim_cli::reject_case(SUITE, "mixed_state", "outside declared bounds");
}

#[test]
fn oversized_streams_take_runtime_dense_and_nested_operands() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_103/descriptor_stream.out");
    sim_cli::run_case(SUITE, "descriptor_stream", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_stream", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_stream", expected);
}

#[test]
fn dense_rows_join_descriptor_patterns_and_scatter() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_103/dense_patterns.out");
    sim_cli::run_case(SUITE, "dense_patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "dense_patterns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "dense_patterns", expected);
}

#[test]
fn former_rtl_015_negatives_execute() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "rtl015_mixed_state_target",
        include_str!("../fixtures/sim/feature_completion/rtl_103/rtl015_mixed_state_target.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "rtl015_runtime_with_output",
        include_str!("../fixtures/sim/feature_completion/rtl_103/rtl015_runtime_with_output.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "rtl015_descriptor_runtime_with",
        include_str!(
            "../fixtures/sim/feature_completion/rtl_103/rtl015_descriptor_runtime_with.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn neg_copy_out_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_copyout_dependence",
        "output copy-out streaming `with` selector reads a target unpacked earlier",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_expression_copyout",
        "not supported for a function call inside an expression",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_inout_stream",
        "streaming operator can only be used in an assignment or bit-stream cast argument",
    );
}

#[test]
fn neg_descriptor_stream_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_stream_exceeds",
        "llg runtime fatal: fixed stream exceeds destination",
    );
}
