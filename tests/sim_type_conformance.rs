//! IEEE 1364-2001 / 1800-2009 conformance matrices with independent oracles.
//!
//! Expected values do not use the compiler's constant evaluator, the simulator
//! value implementation, or another simulator. Every execution case checks both
//! optimizer modes against the same specification-derived result.

#[path = "sim_type_conformance/integral.rs"]
mod integral;
#[path = "sim_type_conformance/nets.rs"]
mod nets;
#[path = "support/sim.rs"]
mod sim_harness;
#[path = "sim_type_conformance/storage.rs"]
mod storage;

#[path = "support/sim_cli.rs"]
mod sim_cli;

fn run_case(fixture: &str, expected: &str) {
    run_case_with_warnings(fixture, expected, &[]);
}

fn run_case_with_warnings(fixture: &str, expected: &str, warnings: &[&str]) {
    sim_cli::run_case("type_conformance", fixture, expected, "", warnings);
}

fn run_case_with_stderr(fixture: &str, expected: &str, stderr: &str) {
    sim_cli::run_case("type_conformance", fixture, expected, stderr, &[]);
}

fn reject_case(fixture: &str, diagnostic: &str) {
    sim_cli::reject_case("type_conformance", fixture, diagnostic);
}
