//! U01 executable-node coverage through the public simulator CLI.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

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

/// G1-02 `coverage_reachable_pattern`: a reached pattern-matching case is a
/// located feature rejection, not an empty ordinary case that reaches
/// lowering.
#[test]
fn coverage_reachable_pattern_reports_its_source_span() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "feature_completion/g1_02",
            "coverage_reachable_pattern",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "optimized={optimized}: pattern case produced stdout: {output:?}"
        );
        assert!(
            stderr.contains("unsupported executable node `PatternCase`"),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            stderr.contains("coverage_reachable_pattern.sv:10:9"),
            "optimized={optimized}: {stderr}"
        );
    }
}

/// G1-02 `coverage_pruned_udp`: an elaboration-pruned unsupported primitive
/// stays irrelevant while a surviving one still fails until G2-40.
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
