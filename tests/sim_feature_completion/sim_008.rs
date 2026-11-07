//! SIM-008: native references and copy-out destinations: output and inout
//! actuals naming container elements, and native record `ref` formals.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_008";

#[test]
fn container_elements_receive_output_and_inout_copy_out() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/element_copy_out.out");
    sim_cli::run_case_backend_parity(SUITE, "element_copy_out", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "neg_expression_element_output",
        "naming a queue, dynamic or associative array element is supported only when the call is a statement",
    );
}

#[test]
fn container_ref_formals_alias_the_caller_container() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/container_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "container_refs", expected, &[], &[]);
}

#[test]
fn element_refs_follow_elements_until_outdated() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "element_refs", expected, &[], &[]);
}

#[test]
fn string_element_refs_follow_elements_until_outdated() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/string_element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "string_element_refs", expected, &[], &[]);
    let invalid =
        include_str!("../fixtures/sim/feature_completion/sim_008/string_element_refs_invalid.out");
    sim_cli::run_case_backend_parity(SUITE, "string_element_refs_invalid", invalid, &[], &[]);
}

#[test]
fn record_element_refs_share_the_element_until_outdated() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_element_refs", expected, &[], &[]);
    let members = include_str!("../fixtures/sim/feature_completion/sim_008/record_member_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_member_refs", members, &[], &[]);
}

#[test]
fn module_record_leaves_bind_ref_formals_by_run_time_index() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/record_leaf_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_leaf_refs", expected, &[], &[]);
    let members =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_leaf_ref_members.out");
    sim_cli::run_case_backend_parity(SUITE, "record_leaf_ref_members", members, &[], &[]);
}

#[test]
fn every_store_through_a_record_ref_formal_reaches_the_element() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_write_paths.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_write_paths", expected, &[], &[]);
}

#[test]
fn module_records_bind_class_method_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_method_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_method_refs", expected, &[], &[]);
    let waits =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_method_ref_waits.out");
    sim_cli::run_case_backend_parity(SUITE, "record_method_ref_waits", waits, &[], &[]);
}

#[test]
fn native_record_ref_formals_alias_subroutine_records() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_refs", expected, &[], &[]);
}

#[test]
fn native_record_ref_formals_bind_module_and_block_records() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/module_record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "module_record_refs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "module_record_refs", expected);
}

#[test]
fn timed_record_refs_share_the_record_with_other_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/timed_record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "timed_record_refs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "timed_record_refs", expected);
}

/// A run that ends at a waited record `ref` actual selecting no element.
fn assert_stopped_at_unselected_element(
    label: &str,
    output: &std::process::Output,
    expected: &str,
) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
    assert!(
        stderr.contains("selects no element of a task that waits on the formal"),
        "{label}: {stderr}"
    );
}

#[test]
fn run_time_record_elements_bind_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_element_select.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_element_select", expected, &[], &[]);
    sim_cli::run_case_checked_matrix(
        SUITE,
        "record_ref_element_select_fatal",
        &[],
        &|label, output| {
            assert_stopped_at_unselected_element(
                label,
                output,
                include_str!(
                    "../fixtures/sim/feature_completion/sim_008/record_ref_element_select_fatal.out"
                ),
            );
        },
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "record_ref_element_select_fatal_column",
        &[],
        &|label, output| {
            assert_stopped_at_unselected_element(
                label,
                output,
                include_str!(
                    "../fixtures/sim/feature_completion/sim_008/record_ref_element_select_fatal_column.out"
                ),
            );
        },
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "record_ref_element_select_fatal_column_nested",
        &[],
        &|label, output| {
            assert_stopped_at_unselected_element(
                label,
                output,
                include_str!(
                    "../fixtures/sim/feature_completion/sim_008/record_ref_element_select_fatal_column_nested.out"
                ),
            );
        },
    );
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_element_large.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_element_large", expected, &[], &[]);
}

/// Limits of the leaf form, which lowers a call once per element: a record
/// with a member array of 16 cells or more selects a cell instead and has
/// neither limit.
#[test]
fn leaf_form_run_time_selections_beyond_the_branch_limit_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_element_select_product",
        "run-time selection among 3375 element combinations of record `ref` actuals in `tb` is not supported; at most 256 are",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_element_select_value",
        "ref actual of native record formal `r` in `tb` must be a whole subroutine record",
    );
}

#[test]
fn record_elements_with_container_members_bind_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_container_member.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_container_member", expected, &[], &[]);
}

#[test]
fn module_records_bind_ref_formals_of_virtual_interface_methods() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_interface_method.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_interface_method", expected, &[], &[]);
}

#[test]
fn unbindable_record_ref_actuals_are_rejected_explicitly() {
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_const_write",
        "cannot assign to read-only variable 'r'",
    );
}

#[test]
fn input_system_functions_store_into_container_elements() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/scan_elements.out");
    sim_cli::run_case_backend_parity(SUITE, "scan_elements", expected, &[], &[]);
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/plusarg_elements.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "plusarg_elements",
        expected,
        &["--append-plusarg", "+N=4", "--append-plusarg", "+H=1f"],
        &[],
    );
    // String and real element destinations are staged since SIM-026
    // (`sim_026/container_destinations`).
}

#[test]
fn ref_formals_mix_with_inputs_and_recursive_forwarding() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/mixed_ref_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "mixed_ref_formals", expected, &[], &[]);
}
