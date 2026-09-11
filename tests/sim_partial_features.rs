//! Regression coverage for completing existing simulator features.

#[path = "sim_partial_features/dynamic_delays.rs"]
mod dynamic_delays;
#[path = "sim_partial_features/events.rs"]
mod events;
#[path = "sim_partial_features/inertial.rs"]
mod inertial;
#[path = "sim_partial_features/ports.rs"]
mod ports;
#[path = "sim_partial_features/select_ranges.rs"]
mod select_ranges;
#[path = "support/sim.rs"]
mod sim_harness;
#[path = "sim_partial_features/timing.rs"]
mod timing;

#[path = "support/sim_cli.rs"]
mod sim_cli;

fn run_case(fixture: &str, expected: &str) {
    run_case_with_stderr(fixture, expected, "");
}

fn run_case_with_stderr(fixture: &str, expected: &str, stderr: &str) {
    sim_cli::run_case("partial_features", fixture, expected, stderr, &[]);
}

fn reject_case(fixture: &str, diagnostic: &str) {
    sim_cli::reject_case("partial_features", fixture, diagnostic);
}
