//! U01 executable-node coverage through the public simulator CLI.

use crate::sim_cli;

#[test]
fn reachable_unsupported_primitive_fails_with_source_span() {
    sim_cli::reject_case("u01_coverage", "reachable_udp", "reachable_udp.sv:18:10");
}

#[test]
fn elaborated_away_unsupported_branch_does_not_fail() {
    sim_cli::run_case(
        "u01_coverage",
        "elaborated_away",
        "PASS elaborated_away\n",
        "",
        &[],
    );
}

#[test]
fn invalid_source_is_rejected_before_simulation_coverage() {
    sim_cli::reject_case("u01_coverage", "invalid_source", "Slang reported errors");
}

#[test]
fn coverage_reachable_pattern_executes_with_owned_tag_matching() {
    sim_cli::run_case(
        "feature_completion/g1_02",
        "coverage_reachable_pattern",
        "coverage_reachable_pattern=o0\n",
        "",
        &[],
    );
}

/// G1-02 `coverage_pruned_udp`: an elaboration-pruned unsupported primitive
/// stays irrelevant while a reachable sequential UDP remains an explicit
/// unsupported boundary after combinational UDP support.
#[test]
fn coverage_pruned_udp_does_not_fail_an_unrelated_top() {
    sim_cli::run_case(
        "u01_coverage",
        "elaborated_away",
        "PASS elaborated_away\n",
        "",
        &[],
    );
    sim_cli::reject_case(
        "u01_coverage",
        "reachable_udp",
        "user-defined primitive instance is not supported",
    );
}
