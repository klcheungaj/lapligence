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

#[test]
fn level_waits_on_ref_formals_follow_bound_actuals() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_009/ref_level_waits.out");
    sim_cli::run_case_backend_parity(SUITE, "ref_level_waits", expected, &[], &[]);
}

#[test]
fn typed_bodies_follow_whole_ref_actuals() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_009/typed_ref_waits.out");
    sim_cli::run_case_backend_parity(SUITE, "typed_ref_waits", expected, &[], &[]);
}

#[test]
fn recursive_element_and_property_ref_waits_take_the_typed_path() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/typed_element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "typed_element_refs", expected, &[], &[]);
}

#[test]
fn named_event_lists_and_implicit_sensitivity_follow_ref_formals() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_009/typed_event_lists.out");
    sim_cli::run_case_backend_parity(SUITE, "typed_event_lists", expected, &[], &[]);
}

#[test]
fn string_and_handle_ref_waits_follow_the_actual_marker() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_009/typed_string_handle_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "typed_string_handle_refs", expected, &[], &[]);
}

#[test]
fn waited_string_refs_to_fork_visible_automatics_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_shared_string_ref_wait",
        "`ref` actual of string or handle formal `s` of task `ws`, which waits on it, is a class property or an automatic variable a fork branch can change; its changes publish no marker the task can wait on (SIM-009)",
    );
}

#[test]
fn waited_handle_refs_to_class_properties_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_handle_property_ref_wait",
        "`ref` actual of string or handle formal `h` of task `wh`, which waits on it, is a class property or an automatic variable a fork branch can change; its changes publish no marker the task can wait on (SIM-009)",
    );
}

#[test]
fn timing_controls_in_functions_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_function_timing",
        "neg_function_timing.sv:5:5 statements that pass time are not allowed in this context",
    );
}

#[test]
fn nonblocking_assignments_to_ref_formals_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_nba",
        "neg_ref_nba.sv:6:7 nonblocking assignment to automatic variable 'r' is not allowed",
    );
}

#[test]
fn ref_formals_in_nonblocking_event_controls_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_event_capture",
        "neg_automatic_event_capture.sv:7:20 cannot refer to automatic variable 'r' from intra-assignment event control of nonblocking assignment",
    );
}
