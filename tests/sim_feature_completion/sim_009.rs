//! SIM-009: typed timed-call gaps: events declared in blocks and subroutine
//! bodies, and fork captures of timing-control operands. Oracles are derived
//! by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_009";

#[test]
fn block_and_task_local_events_are_independent() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_009/local_events.out");
    sim_cli::run_case_backend_parity(SUITE, "local_events", expected, &[], &[]);
}
