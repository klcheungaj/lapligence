//! SIM-006: recursive containers in subroutine, block, port and record
//! storage; identity-handle arrays; event containers; associative-array
//! defaults and diagnostics. Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_006";

#[test]
fn containers_cross_subroutine_formals_results_and_locals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/subroutine_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "subroutine_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "subroutine_containers", expected);
}

#[test]
fn record_column_formals_precede_trailing_results() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/record_column_results.out");
    sim_cli::run_case_backend_parity(SUITE, "record_column_results", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_column_results", expected);
}

#[test]
fn block_and_port_containers_keep_their_lifetimes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/procedural_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "procedural_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "procedural_containers", expected);
}

#[test]
fn class_container_properties_are_per_object() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/class_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "class_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "class_containers", expected);
}

#[test]
fn nested_containers_copy_patterns_and_concatenations() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/nested_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "nested_containers", expected);
}

#[test]
fn nested_element_methods_and_whole_uses_stage_the_element() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_element_methods.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_element_methods", expected, &[], &[]);
}

#[test]
fn class_containers_are_reached_through_computed_handles() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/computed_handle_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "computed_handle_containers", expected, &[], &[]);
}

#[test]
fn class_container_forms_of_method_bodies() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/class_container_forms.out");
    sim_cli::run_case_backend_parity(SUITE, "class_container_forms", expected, &[], &[]);
}

#[test]
fn call_results_are_method_receivers_and_select_bases() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/call_result_selects.out");
    sim_cli::run_case_backend_parity(SUITE, "call_result_selects", expected, &[], &[]);
}

#[test]
fn container_actuals_are_built_where_the_call_runs() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/call_container_operands.out");
    sim_cli::run_case_backend_parity(SUITE, "call_container_operands", expected, &[], &[]);
}

#[test]
fn computed_handles_with_no_evaluation_point_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_computed_handle_wait",
        "class container property `q` in `tb` is selected through a handle that is not a variable, formal or property",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_computed_handle_assoc_foreach",
        "`foreach` over `m` in `tb` calls a subroutine for its handle",
    );
}

#[test]
fn nested_elements_change_in_any_expression() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_element_expressions.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_element_expressions", expected, &[], &[]);
}

#[test]
fn nested_elements_pass_whole_to_input_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_element_arguments.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_element_arguments", expected, &[], &[]);
}

#[test]
fn record_elements_copy_resize_delete_and_pop() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/record_elements.out");
    sim_cli::run_case_backend_parity(SUITE, "record_elements", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_elements", expected);
}

#[test]
fn handle_arrays_copy_identities_only() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/handle_arrays.out");
    sim_cli::run_case_backend_parity(SUITE, "handle_arrays", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "handle_arrays", expected);
}

#[test]
fn event_containers_keep_trigger_identity() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/event_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "event_containers", expected, &[], &[]);
    sim_cli::run_case(
        SUITE,
        "event_containers",
        expected,
        concat!(
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
        ),
        &[],
    );
}

#[test]
fn associative_defaults_and_invalid_indices_follow_7_8_6() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/assoc_defaults.out");
    sim_cli::run_case_backend_parity(SUITE, "assoc_defaults", expected, &[], &[]);
    sim_cli::run_case(
        SUITE,
        "assoc_defaults",
        expected,
        concat!(
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
            "llg container warning: invalid associative-array key read\n",
            "llg container warning: invalid associative-array integral key write\n",
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
            "llg container warning: associative-array read of a nonexistent entry returns the default\n",
        ),
        &[],
    );
}

#[test]
fn adopted_container_and_handle_witnesses() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "dynamic_events",
        include_str!("../fixtures/sim/feature_completion/sim_006/dynamic_events.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "queue_events",
        include_str!("../fixtures/sim/feature_completion/sim_006/queue_events.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "associative_events",
        include_str!("../fixtures/sim/feature_completion/sim_006/associative_events.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "process_array",
        include_str!("../fixtures/sim/feature_completion/sim_006/process_array.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "process_formal",
        include_str!("../fixtures/sim/feature_completion/sim_006/process_formal.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "semaphore_array",
        include_str!("../fixtures/sim/feature_completion/sim_006/semaphore_array.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "mailbox_array",
        include_str!("../fixtures/sim/feature_completion/sim_006/mailbox_array.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "virtual_associative",
        include_str!("../fixtures/sim/feature_completion/sim_006/virtual_associative.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "dynamic_call",
        include_str!("../fixtures/sim/feature_completion/sim_006/dynamic_call.out"),
        &[],
        &[],
    );
}

#[test]
fn wildcard_index_traversal_is_rejected_by_the_frontend() {
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_foreach",
        "foreach loops cannot be used with associative arrays that have a wildcard index",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_first",
        "'first' cannot be called with an associative array with wildcard index type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_find",
        "'find_index' cannot be called with an associative array with wildcard index type",
    );
}

#[test]
fn unsupported_container_boundaries_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_nested_two_changes",
        "a statement in `tb` changes two nested container elements of one container",
    );
}

#[test]
fn unpacked_array_concatenations_assign_queues_and_dynamic_arrays() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/queue_concatenation.out");
    sim_cli::run_case_backend_parity(SUITE, "queue_concatenation", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_array_concat",
        "unpacked array concatenation of arrays into a dynamic array",
    );
}

#[test]
fn container_declarations_accept_any_whole_container_initializer() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/container_initializers.out");
    sim_cli::run_case_backend_parity(SUITE, "container_initializers", expected, &[], &[]);
}

#[test]
fn associative_arrays_nest_in_container_elements() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_associative.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_associative", expected, &[], &[]);
}

#[test]
fn nested_associative_arrays_cross_boundaries() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_associative_forms.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_associative_forms", expected, &[], &[]);
}

#[test]
fn associative_arrays_nest_at_any_depth() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_associative_deep.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_associative_deep", expected, &[], &[]);
}

#[test]
fn foreach_walks_every_nested_dimension_kind() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_006/nested_foreach.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_foreach", expected, &[], &[]);
}

#[test]
fn record_element_members_take_compound_writes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/record_element_compound.out");
    sim_cli::run_case_backend_parity(SUITE, "record_element_compound", expected, &[], &[]);
}

#[test]
fn record_fields_of_every_storage_take_compound_writes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/record_field_compound.out");
    sim_cli::run_case_backend_parity(SUITE, "record_field_compound", expected, &[], &[]);
}

#[test]
fn semaphore_try_get_is_a_statement() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/semaphore_try_get_statement.out");
    sim_cli::run_case_backend_parity(SUITE, "semaphore_try_get_statement", expected, &[], &[]);
}

#[test]
fn block_static_locals_wake_fork_events() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/static_block_event.out");
    sim_cli::run_case_backend_parity(SUITE, "static_block_event", expected, &[], &[]);
}

#[test]
fn nested_elements_bind_ref_formals_of_functions() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_ref_actuals.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_ref_actuals", expected, &[], &[]);
}

#[test]
fn subroutine_and_object_containers_wake_waits() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/subroutine_container_waits.out");
    sim_cli::run_case_backend_parity(SUITE, "subroutine_container_waits", expected, &[], &[]);
}

#[test]
fn nested_associative_handles_survive_rewrites() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_associative_handles.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_associative_handles", expected, &[], &[]);
}

#[test]
fn a_task_reference_to_a_nested_queue_is_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_nested_task_ref",
        "ref actual of container formal `t` in `tb` must be a container variable of the same type",
    );
}

#[test]
fn nested_associative_selectors_run_once_and_short_circuit() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_006/nested_associative_selectors.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_associative_selectors", expected, &[], &[]);
}
