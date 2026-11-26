//! `foreach` loop variables captured by a detached fork
//! ([readme](../fixtures/sim/feature_completion/ki_foreach_fork_capture/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_foreach_fork_capture";

#[test]
fn detached_fork_branches_share_the_foreach_loop_variable() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "forks",
        include_str!("../fixtures/sim/feature_completion/ki_foreach_fork_capture/forks.out"),
        &[],
        &[],
    );
}
