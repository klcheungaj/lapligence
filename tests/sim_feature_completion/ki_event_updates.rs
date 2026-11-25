//! Nonblocking assignments under an event control
//! ([readme](../fixtures/sim/feature_completion/ki_event_updates/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_event_updates";

#[test]
fn a_clocking_event_queues_updates() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "clocking",
        include_str!("../fixtures/sim/feature_completion/ki_event_updates/clocking.out"),
        &[],
        &[],
    );
}
