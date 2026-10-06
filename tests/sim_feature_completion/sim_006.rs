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
        "neg_container_ref_formal",
        "ref formal `q` of resizable container type is not supported (SIM-008)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_container_fork_capture",
        "references native record or container `q` of the enclosing activation",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_class_container_outside",
        "class container property `q` in `tb` is accessible only inside its class's methods (SIM-011)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_nested_element_method",
        "method `push_back` of a nested container element in `tb` is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_container_result_select",
        "container result of `make` in `tb` must be assigned whole to a container variable",
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
