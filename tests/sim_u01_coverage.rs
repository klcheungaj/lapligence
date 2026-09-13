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
