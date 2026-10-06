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
fn local_event_controls_allow_native_formals_and_recursion() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/event_local_native.out");
    sim_cli::run_case_backend_parity(SUITE, "event_local_native", expected, &[], &[]);
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

#[test]
fn expanded_ref_event_controls_wake_on_branch_writes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/expanded_ref_wakeups.out");
    sim_cli::run_case_backend_parity(SUITE, "expanded_ref_wakeups", expected, &[], &[]);
}

#[test]
fn expanded_element_ref_event_controls_keep_their_element() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/expanded_element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "expanded_element_refs", expected, &[], &[]);
}

#[test]
fn expanded_qualifiers_read_string_and_handle_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/expanded_native_qualifiers.out");
    sim_cli::run_case_backend_parity(SUITE, "expanded_native_qualifiers", expected, &[], &[]);
}

#[test]
fn event_controls_on_subroutine_strings_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_event_control",
        "event control on subroutine string or handle `tag` in `tb` is not supported (SIM-009)",
    );
}

#[test]
fn recursive_tasks_with_string_qualifiers_take_the_typed_path() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/recursive_native_qualifiers.out");
    sim_cli::run_case_backend_parity(SUITE, "recursive_native_qualifiers", expected, &[], &[]);
}
