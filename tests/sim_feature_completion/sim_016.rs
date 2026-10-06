//! SIM-016: semaphore collections and shared-lifetime synchronization.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_016";

#[test]
fn task_handle_variables_are_shared_with_fork_branches() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_016/shared_handles.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_handles", expected, &[], &[]);
}
