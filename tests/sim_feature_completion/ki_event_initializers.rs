//! Declaration initializers of arrays of events
//! ([readme](../fixtures/sim/feature_completion/ki_event_initializers/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_event_initializers";

#[test]
fn array_initializers_assign_their_elements() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "arrays",
        include_str!("../fixtures/sim/feature_completion/ki_event_initializers/arrays.out"),
        &[],
        &[],
    );
}

#[test]
fn block_event_containers_hold_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "blocks",
        include_str!("../fixtures/sim/feature_completion/ki_event_initializers/blocks.out"),
        &[],
        &[],
    );
}
