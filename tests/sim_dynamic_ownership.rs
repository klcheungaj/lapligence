//! P07 acceptance through the public HDL frontend and both optimizer modes.
//! Migration rejections are failures here, not expected or silently skipped cases.

#[path = "support/sim.rs"]
mod sim_harness;
#[path = "support/sim_cli.rs"]
mod sim_cli;

fn run(fixture: &str, expected: &str) {
    sim_cli::run_case("dynamic_ownership", fixture, expected, "", &[]);
}

#[test]
fn numeric_loop() {
    run("numeric_loop", "1007\n");
}

#[test]
fn branch_side_effects() {
    run("branch_side_effects", "23 0\n1\n");
}

#[test]
fn wider_intermediate_than_declared_storage() {
    run("wide_intermediate", "7168 4096\n");
}

#[test]
fn mixed_width_loop() {
    run("mixed_width_loop", "8192 1\n");
}

#[test]
fn selected_nba_captures_before_mutation() {
    run("selected_nba_capture", "0\n42\n");
}

#[test]
fn recursive_return() {
    run("recursive_return", "2176\n");
}

#[test]
fn task_values_survive_yield_and_copyout() {
    run("yielding_task", "47\n");
}

#[test]
fn finish_cancels_live_process() {
    run("finish_cleanup", "finish\n");
}
