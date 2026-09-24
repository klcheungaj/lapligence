//! Focused public-pipeline regressions for the synthesizable review findings.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn stateful_static_return_mutations_are_not_event_callbacks() {
    sim_cli::reject_case_with_args(
        "review_bundle",
        "r03_static_return_mutation",
        "static function return is read or is not assigned on every path",
        &["--edition", "2009"],
    );
}

#[test]
fn read_modify_write_return_access_is_classified_as_a_read() {
    sim_cli::reject_case_with_args(
        "review_bundle",
        "r03_static_return_read_modify_write",
        "static function return is read or is not assigned on every path",
        &["--edition", "2009"],
    );
}

#[test]
fn partially_assigned_static_returns_are_not_event_callbacks() {
    sim_cli::reject_case_with_args(
        "review_bundle",
        "r03_static_partial_return",
        "static function return is read or is not assigned on every path",
        &["--edition", "2009"],
    );
}
