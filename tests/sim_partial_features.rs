//! Regression coverage for completing existing simulator features.

#[path = "sim_partial_features/activation_frames.rs"]
mod activation_frames;
#[path = "sim_partial_features/display.rs"]
mod display;
#[path = "sim_partial_features/dynamic_delays.rs"]
mod dynamic_delays;
#[path = "sim_partial_features/evaluated_events.rs"]
mod evaluated_events;
#[path = "sim_partial_features/events.rs"]
mod events;
#[path = "sim_partial_features/finish.rs"]
mod finish;
#[path = "sim_partial_features/inertial.rs"]
mod inertial;
#[path = "sim_partial_features/intra_assignment_event_sources.rs"]
mod intra_assignment_event_sources;
#[path = "sim_partial_features/intra_assignment_events.rs"]
mod intra_assignment_events;
#[path = "sim_partial_features/nonblocking_events.rs"]
mod nonblocking_events;
#[path = "sim_partial_features/ports.rs"]
mod ports;
#[path = "sim_partial_features/real_sensitivity.rs"]
mod real_sensitivity;
#[path = "sim_partial_features/regions.rs"]
mod regions;
#[path = "sim_partial_features/select_ranges.rs"]
mod select_ranges;
#[path = "sim_partial_features/severity.rs"]
mod severity;
#[path = "support/sim.rs"]
mod sim_harness;
#[path = "sim_partial_features/system.rs"]
mod system;
#[path = "sim_partial_features/system_functions.rs"]
mod system_functions;
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

fn reject_case_with_args(fixture: &str, diagnostic: &str, args: &[&str]) {
    sim_cli::reject_case_with_args("partial_features", fixture, diagnostic, args);
}
