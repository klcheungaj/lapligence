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
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_element_refs_invalid",
        invalid,
        &["--no-index-checks"],
        &[],
    );
}

#[test]
fn record_element_refs_share_the_element_until_outdated() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_element_refs", expected, &[], &[]);
    let members = include_str!("../fixtures/sim/feature_completion/sim_008/record_member_refs.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_member_refs",
        members,
        &["--no-index-checks"],
        &[],
    );
}

#[test]
fn module_record_leaves_bind_ref_formals_by_run_time_index() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/record_leaf_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_leaf_refs", expected, &[], &[]);
    let members =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_leaf_ref_members.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_leaf_ref_members",
        members,
        &["--no-index-checks"],
        &[],
    );
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

const UNSELECTED_OUTPUT: &str = "before\nwoke 7\nselected\n";
const UNSELECTED_DIAGNOSTIC: &str =
    "index of a `ref` actual selects no element of a task that waits on the formal";

/// Every element kind a `ref` formal can bind: the first call selects an
/// element and its waiter wakes, the second selects none and ends the run
/// with exit status 1, before the callee starts.
#[test]
fn waited_ref_actuals_that_select_no_element_stop_the_run() {
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_record_array",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_record_queue",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_record_dynamic",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_packed_array",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_packed_queue",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_packed_dynamic",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_int_array",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_real_array",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_string_array",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "ref_wait_unselected_string_queue",
        &[],
        UNSELECTED_OUTPUT,
        UNSELECTED_DIAGNOSTIC,
    );
}

/// With index checks off (S07-D15), a callee that does not wait keeps the
/// default; with them on, the first call stops the run.
#[test]
fn unwaited_ref_actuals_that_select_no_element_read_the_default() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/ref_unselected_unwaited.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "ref_unselected_unwaited",
        expected,
        &["--no-index-checks"],
        &[],
    );
}

#[test]
fn run_time_record_elements_bind_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_element_select.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_ref_element_select",
        expected,
        &["--no-index-checks"],
        &[],
    );
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

/// FIN-001 R3: a run-time index below a constant one (`o.m[1].a[k]`) binds
/// a record `ref` formal and selects an operand, in both storage forms.
#[test]
fn run_time_elements_below_a_constant_element_bind_ref_formals() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_element_select_nested.out"
    );
    for size in ["N=2", "N=16"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "record_ref_element_select_nested",
            expected,
            &["-D", size],
            &[],
        );
    }
}

/// FIN-001 R3: a function called inside an expression of an assignment, a
/// `return` or a system-task statement binds a run-time selected record
/// element; below 16 cells the statement lowers once per element.
#[test]
fn run_time_record_elements_bind_ref_formals_of_calls_in_expressions() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_element_select_expression.out"
    );
    for size in ["N=3", "N=16"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "record_ref_element_select_expression",
            expected,
            &["--no-index-checks", "-D", size],
            &[],
        );
        // With index checks on, the index selecting no element stops the
        // run at the actual (S07-D15).
        let before = expected
            .rsplit_once("none")
            .map_or(expected, |(before, _)| before);
        sim_cli::run_case_checked_matrix(
            SUITE,
            "record_ref_element_select_expression",
            &["-D", size],
            &|label, output| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
                assert_eq!(String::from_utf8_lossy(&output.stdout), before, "{label}");
                assert!(
                    stderr.contains("record_ref_element_select_expression.sv:57:11")
                        && stderr.contains("index of a `ref` actual selects no element;"),
                    "{label}: {stderr}"
                );
            },
        );
    }
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
        "run-time selected record element as the `ref` actual of native record formal `r` of a function called outside an assignment without timing control, a `return` or a system-task statement",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_element_select_effect",
        "run-time selected record element as a `ref` actual of function `f` after a call or an assignment operator at",
    );
}

#[test]
fn record_elements_with_container_members_bind_ref_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_container_member.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_ref_container_member",
        expected,
        &["--no-index-checks"],
        &[],
    );
}

/// FIN-001 R3: an element or member of a subroutine record whose type has
/// container members takes the record's companion containers, or mirrors of
/// the nested slots of a member array of 16 elements. A task waiting on such
/// a formal rejects.
#[test]
fn subroutine_record_elements_with_container_members_bind_ref_formals() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_subroutine_container_member.out"
    );
    for size in ["N=2", "N=16"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "record_ref_subroutine_container_member",
            expected,
            &["-D", size],
            &[],
        );
    }
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_subroutine_container_wait",
        "task `waiter` waits on record `ref` formal `r`, whose actual is a member of a subroutine record with queue or dynamic-array members",
    );
}

#[test]
fn module_records_bind_ref_formals_of_virtual_interface_methods() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/record_ref_interface_method.out");
    sim_cli::run_case_backend_parity(SUITE, "record_ref_interface_method", expected, &[], &[]);
}

/// FIN-001 R2: tasks waiting on a record `ref` formal bound to an element
/// holding a string resume in the per-cell form; the column form (16 cells)
/// has no change to follow and rejects at the call instead of waiting
/// forever.
#[test]
fn waited_record_refs_bound_to_native_elements() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_wait_native_elements.out"
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_ref_wait_native_elements",
        expected,
        &["-D", "N=3"],
        &[],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "record_ref_wait_native_elements",
        "record_ref_wait_native_elements.sv:27:9 in `tb`",
        &["-D", "N=16"],
    );
}

/// FIN-001 R2: virtual-interface methods waiting on a record `ref` formal
/// bound to an element. The column form (16 cells) has no change to follow
/// for an element holding a string and rejects at the call.
#[test]
fn virtual_interface_methods_wait_on_element_bound_record_refs() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_interface_element_waits.out"
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_ref_interface_element_waits",
        expected,
        &["-D", "N=3"],
        &[],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "record_ref_interface_element_waits",
        "record_ref_interface_element_waits.sv:43:11 in `tb`",
        &["-D", "N=16"],
    );
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

/// Waits on `ref` formals bound to elements and members of elements of
/// subroutine records and of queue elements wake at each store into what they
/// bind (S09-D4): the owner's own member, element and whole-record stores, a
/// sibling branch's, and stores through other `ref` formals; stores into
/// other elements and members wake nothing. Both record storage forms.
#[test]
fn ref_waits_on_record_elements_wake_at_every_store() {
    let subroutine = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_subroutine_element_wake.out"
    );
    let container = include_str!(
        "../fixtures/sim/feature_completion/sim_008/record_ref_container_member_wake.out"
    );
    for size in ["N=2", "N=16"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "record_ref_subroutine_element_wake",
            subroutine,
            &["-D", size],
            &[],
        );
        sim_cli::run_case_backend_parity(
            SUITE,
            "record_ref_container_member_wake",
            container,
            &["-D", size],
            &[],
        );
    }
}
