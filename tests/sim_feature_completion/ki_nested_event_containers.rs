//! Queues and arrays of event queues
//! ([readme](../fixtures/sim/feature_completion/ki_nested_event_containers/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_nested_event_containers";

#[test]
fn arrays_of_event_queues_hold_event_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "nested",
        include_str!("../fixtures/sim/feature_completion/ki_nested_event_containers/nested.out"),
        &[],
        &[],
    );
}

#[test]
fn block_and_static_task_arrays_of_event_queues_hold_event_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "blocks",
        include_str!("../fixtures/sim/feature_completion/ki_nested_event_containers/blocks.out"),
        &[],
        &[],
    );
}
