//! FIN-001 X1: legal forms `docs/known_issues.md` lists as rejected that had no
//! test. Each stays a compile-time limit; the tests pin its id family and,
//! where lowering has a node, its location. See the fixture readme.

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_audit";

#[test]
fn leaf_form_records_reject_an_out_of_range_select_and_a_handle_member_element() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_leaf_string_select",
        "CODEGEN-002",
        Some("8:5"),
        "cannot resolve array base of select `s`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_leaf_handle_member_element",
        "CODEGEN-003",
        Some("12:5"),
        "assignment between incompatible unpacked aggregate types",
    );
}

#[test]
fn pattern_format_of_a_semaphore_handle_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_pattern_semaphore",
        "CODEGEN-001",
        Some("6:5"),
        "built-in semaphore and mailbox handles have no pattern form yet",
    );
}

/// The two evaluator forms are refused by the C emitter, which has no source
/// node: they stay unlocated (`docs/known_issues.md`).
#[test]
fn deferred_report_limits_are_explicit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_monitor_computed_handle",
        "CODEGEN-001",
        Some("7:3"),
        "native or impure calls in evaluator callbacks",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_strobe_printing_helper",
        "CODEGEN-001",
        Some("9:3"),
        "side-effect-capable evaluator expressions",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_strobe_in_final",
        "CODEGEN-001",
        Some("6:9"),
        "$strobe inside a final block",
    );
}

#[test]
fn helper_lists_reject_an_event_handle_that_is_not_a_named_event() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_helper_event_handle",
        "CODEGEN-002",
        Some("17:13"),
        "an event handle other than a declared named event cannot share an event control",
    );
}

#[test]
fn class_keyed_locator_index_and_effectful_compound_key_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_class_key_find_index",
        "CODEGEN-003",
        Some("12:11"),
        "array method `find_index` in `tb` currently requires packed source and destination elements",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_class_key_call_compound",
        "CODEGEN-009",
        Some("11:5"),
        "read-modify-write of a resizable container element in `tb` requires an index without side effects",
    );
}

/// A call as a future function's argument; reachable, unlike the local
/// variable form the frontend rejects (16.9.3).
#[test]
fn future_function_call_argument_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_future_call_argument",
        "CODEGEN-001",
        Some("10:52"),
        "$future_gclk argument in `tb` is not supported: it must be a static packed or real expression",
    );
}

#[test]
fn process_handle_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_status_wait",
        "CODEGEN-001",
        Some("10:5"),
        "process status() in a wait, event or sensitivity expression in `tb` is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_suspend_function",
        "CODEGEN-001",
        Some("6:5"),
        "process method `suspend` inside a function body in `tb` is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_try_get",
        "CODEGEN-001",
        Some("8:9"),
        "mailbox try_get or try_peek into a process variable in `tb` is not supported",
    );
}

#[test]
fn set_items_of_nested_containers_and_container_calls_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_inside_nested_container",
        "CODEGEN-009",
        Some("8:31"),
        "container element access in tb requires a scalar value",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_inside_container_call",
        "CODEGEN-009",
        Some("10:31"),
        "container result of `g` in `tb` must be assigned whole to a container variable",
    );
}

#[test]
fn computed_handle_packed_select_target_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_computed_handle_select_write",
        "CODEGEN-002",
        Some("13:5"),
        "cannot resolve array base of select `b`",
    );
}

/// Formerly a known issue: elements of an unpacked array property of a
/// computed handle read, write and take a compound assignment.
#[test]
fn computed_handle_array_property_elements_execute() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_audit/computed_handle_element.out");
    sim_cli::run_case(SUITE, "computed_handle_element", expected, "", &[]);
}

#[test]
fn record_call_result_with_a_real_member_in_equality_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_real_call_equality",
        "CODEGEN-001",
        Some("12:32"),
        "real aggregate bit-stream source is not supported",
    );
}

#[test]
fn deferred_actions_with_dynamic_array_or_event_arguments_are_located_limits() {
    let text = "unpacked aggregate, container and event arguments of deferred immediate assertion actions are not supported";
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_dynamic_array_action",
        "CODEGEN-001",
        Some("12:24"),
        text,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_event_action",
        "CODEGEN-001",
        Some("10:24"),
        text,
    );
}

#[test]
fn wait_on_a_property_a_callee_reads_through_a_container_element_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_wait_container_element_property",
        "CODEGEN-002",
        Some("9:12"),
        "class property `x` read by a called subroutine through a handle other than a class-handle input formal",
    );
}
