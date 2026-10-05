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
