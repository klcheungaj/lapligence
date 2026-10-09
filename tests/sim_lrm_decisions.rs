//! Portable LRM decision cases (IEEE 1800-2009). Every fixture under
//! `tests/fixtures/sim/lrm_decisions/` is plain SystemVerilog with top `tb`,
//! no tool-specific options, and its exact expected stdout in `<case>.out`;
//! the header comment quotes the clause each line rests on, so the case can be
//! rerun unchanged on another simulator. Each runs through the public CLI on
//! every value backend in both optimizer modes.

use crate::sim_cli;

const SUITE: &str = "lrm_decisions";

fn run_decision(case: &str, expected: &str) {
    sim_cli::run_case_checked_matrix(SUITE, case, &[], &|label, output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{label}: stdout differs from the clause-derived expectation"
        );
    });
}

#[test]
fn aa_d1_implication_one_result() {
    run_decision(
        "AA-D1_implication_one_result",
        include_str!("fixtures/sim/lrm_decisions/AA-D1_implication_one_result.out"),
    );
}

#[test]
fn aa_n1_implication_result_time() {
    run_decision(
        "AA-N1_implication_result_time",
        include_str!("fixtures/sim/lrm_decisions/AA-N1_implication_result_time.out"),
    );
}

#[test]
fn aa_d3_cover_sequence_all_matches() {
    run_decision(
        "AA-D3_cover_sequence_all_matches",
        include_str!("fixtures/sim/lrm_decisions/AA-D3_cover_sequence_all_matches.out"),
    );
}

#[test]
fn aa_d4_leading_range_empty() {
    run_decision(
        "AA-D4_leading_range_empty",
        include_str!("fixtures/sim/lrm_decisions/AA-D4_leading_range_empty.out"),
    );
}

#[test]
fn aa_d5_match_multiplicity() {
    run_decision(
        "AA-D5_match_multiplicity",
        include_str!("fixtures/sim/lrm_decisions/AA-D5_match_multiplicity.out"),
    );
}

#[test]
fn aa_d2_default_clocking_iff() {
    run_decision(
        "AA-D2_default_clocking_iff",
        include_str!("fixtures/sim/lrm_decisions/AA-D2_default_clocking_iff.out"),
    );
}
