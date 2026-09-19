//! Regression coverage for review findings R02-R08, R10-R12 and R15.
//! Public HDL cases run with and without optimization. C runtime endpoint tests
//! are registered separately in tests/runtime_value_storage/CMakeLists.txt.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "group1_repairs";

#[test]
fn callback_loop_labels_are_unique_per_inline_expansion() {
    sim_cli::run_case(SUITE, "callback_loops", "callback loops passed\n", "", &[]);
}

#[test]
fn callback_real_results_survive_their_private_scope() {
    sim_cli::run_case(SUITE, "callback_real", "real callbacks passed\n", "", &[]);
}
