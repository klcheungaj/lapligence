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

#[test]
fn output_inout_and_ref_event_formals_use_the_typed_call_path() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_009/event_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "event_formals", expected, &[], &[]);
}

#[test]
fn expanded_tasks_with_native_formals_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_event_local_native",
        "task `t` with native record or container formals needs caller-environment expansion, which is not supported in `tb`",
    );
}
