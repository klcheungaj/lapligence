//! The order in which forked processes start
//! ([readme](../fixtures/sim/feature_completion/ki_fork_order/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_fork_order";

#[test]
fn a_fork_without_statements_is_not_waited_for() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "empty",
        include_str!("../fixtures/sim/feature_completion/ki_fork_order/empty.out"),
        &[],
        &[],
    );
}
