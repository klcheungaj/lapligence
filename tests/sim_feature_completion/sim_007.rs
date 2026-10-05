//! SIM-007: native aggregates, tagged values and pattern expressions. Fixed
//! arrays of strings, records with string/real/handle leaves and their
//! patterns, slices, conditionals, equality, calls and nonblocking writes.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_007";

#[test]
fn native_arrays_patterns_slices_calls_and_nbas() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_arrays.out");
    sim_cli::run_case_backend_parity(SUITE, "native_arrays", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_arrays", expected);
}

#[test]
fn record_handle_members_copy_identity_through_calls_and_arrays() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/handle_records.out");
    sim_cli::run_case_backend_parity(SUITE, "handle_records", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "handle_records", expected);
}

#[test]
fn native_tagged_unions_match_bind_and_check_members() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_tagged.out");
    sim_cli::run_case_backend_parity(SUITE, "native_tagged", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_tagged", expected);
}

/// Expected stderr with `{source}` standing for the fixture's absolute path,
/// which run-time tagged-access reports carry as their source location.
fn source_stderr(fixture: &str, template: &str) -> String {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    template.replace("{source}", &super::sim_harness::source_display(&source))
}

#[test]
fn inactive_native_tagged_members_report_and_store_nothing() {
    let expected = source_stderr(
        "native_tagged_inactive",
        "llg: runtime error: access to inactive tagged-union member S at {source}:10:28\n\
         llg: runtime error: access to inactive tagged-union member S at {source}:12:34\n\
         llg: runtime error: access to inactive tagged-union member F at {source}:12:39\n\
         llg: runtime error: access to inactive tagged-union member S at {source}:13:9\n\
         llg: runtime error: access to inactive tagged-union member I at {source}:17:9\n",
    );
    sim_cli::run_case_checked_matrix(SUITE, "native_tagged_inactive", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "1 []\n2 [] 0.0\n3 9\n4 kept\n",
            "{label}"
        );
        assert_eq!(stderr, expected, "{label}");
    });
}

#[test]
fn adopted_native_value_witnesses() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_tagged_native",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_tagged_native.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_binding",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_binding.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_array_pattern",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_array_pattern.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_record_slice",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_record_slice.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_array_return",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_array_return.out"),
        &[],
        &[],
    );
}

#[test]
fn run_time_member_indices_select_native_record_elements() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/native_member_select.out");
    sim_cli::run_case_backend_parity(SUITE, "native_member_select", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_select_limit", "xy []\n", &[], &[]);
    sim_cli::reject_case_with_args(
        SUITE,
        "bad_member_select_limit",
        "run-time index into a native record member array of 65 elements in `tb` is not supported; at most 64 are",
        &[],
    );
}

#[test]
fn record_elements_of_containers_compare_member_wise() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_element_equality.out");
    sim_cli::run_case_backend_parity(SUITE, "record_element_equality", expected, &[], &[]);
}
