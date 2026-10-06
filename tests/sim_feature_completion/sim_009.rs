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

#[test]
fn recursive_timed_tasks_keep_native_locals_event_and_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/recursive_activations.out");
    sim_cli::run_case_backend_parity(SUITE, "recursive_activations", expected, &[], &[]);
}

#[test]
fn static_tasks_share_storage_and_automatic_tasks_do_not() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/static_and_automatic_tasks.out");
    sim_cli::run_case_backend_parity(SUITE, "static_and_automatic_tasks", expected, &[], &[]);
}

#[test]
fn event_expressions_on_formals_use_the_typed_call_path() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/formal_event_expressions.out");
    sim_cli::run_case_backend_parity(SUITE, "formal_event_expressions", expected, &[], &[]);
}
