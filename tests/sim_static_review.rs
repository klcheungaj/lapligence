//! Regression cases for the source-only review continuation.
//! Added as future coverage; no compile or test run was made during the review.
#[path = "support/sim.rs"]
mod sim_harness;
#[path = "support/sim_cli.rs"]
mod sim_cli;

#[test]
fn call_arguments() {
    sim_cli::run_case("static_review", "call_arguments", "left=11 right=20 result=30\n", "", &[]);
}

#[test]
fn case_selector_once() {
    sim_cli::run_case("static_review", "case_selector_once", "packed calls=1 hit=5\nreal calls=1 hit=5 items=1\n", "", &[]);
}

#[test]
fn repeat_counts() {
    sim_cli::run_case("static_review", "repeat_counts", "negative=0 unknown=0 highz=0 wide=1\n", "", &[]);
}

#[test]
fn recursive_task_disable() {
    sim_cli::run_case("static_review", "recursive_task_disable", "continued=0\n", "", &[]);
}

#[test]
fn activation_jumps() {
    sim_cli::run_case("static_review", "activation_jumps", "count=1 result=7\n", "", &[]);
}
