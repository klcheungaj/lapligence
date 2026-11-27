//! FIN-001 X1: forms `docs/known_issues.md` lists that had no test. A form
//! that stays a compile-time limit is pinned by its id family and, where
//! lowering has a node, its location; a form llg now serves runs against a
//! hand-derived output; a known difference in output is pinned so that its
//! fix updates the test. See the fixture readme.

use super::{sim_cli, sim_harness};

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

/// The two evaluator forms are refused by the C emitter, at the report
/// statement the callback carries.
#[test]
fn deferred_report_limits_are_explicit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_monitor_computed_handle",
        "CODEGEN-001",
        Some("9:5"),
        "native or impure calls in evaluator callbacks",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_strobe_printing_helper",
        "CODEGEN-001",
        Some("10:5"),
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

#[test]
fn process_handle_ref_output_and_status_forms_are_located_limits() {
    let ref_actual = "ref actual of process formal `p` in `tb` must be a process variable";
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_ref_property",
        "CODEGEN-009",
        Some("12:5"),
        ref_actual,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_ref_member",
        "CODEGEN-009",
        Some("10:5"),
        ref_actual,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_output_expression",
        "CODEGEN-001",
        Some("11:9"),
        "output or inout process formal `p` bound to a process variable in a call inside an expression in `tb` is not supported",
    );
    let status =
        "process status() in a wait, event or sensitivity expression in `tb` is not supported";
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_status_event",
        "CODEGEN-001",
        Some("8:5"),
        status,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_status_implicit",
        "CODEGEN-001",
        Some("6:10"),
        status,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_status_continuous",
        "CODEGEN-001",
        Some("5:8"),
        status,
    );
}

/// Formerly listed as rejected: `ref` process formals bound to array and
/// queue elements, `await()` in a fork-join_none branch of a function, and
/// status() in an always_comb body, which (like a variable-only sensitivity)
/// is not re-evaluated when the status changes.
#[test]
fn process_handle_element_ref_function_await_and_always_comb_status_execute() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_audit/process_handle_forms.out");
    sim_cli::run_case(SUITE, "process_handle_forms", expected, "", &[]);
}

#[test]
fn real_array_call_operands_and_oversized_elementwise_expressions_are_located_limits() {
    let operand =
        "real-array operand in `tb` must be a real array variable, row or assignment pattern";
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_real_array_call_equality",
        "CODEGEN-009",
        Some("14:10"),
        operand,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_real_array_call_arm",
        "CODEGEN-009",
        Some("14:9"),
        operand,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_real_array_elementwise_limit",
        "CODEGEN-009",
        Some("11:9"),
        "real-array operand in `tb` has 5000 elements; element-wise real-array expressions support at most 4096",
    );
}

#[test]
fn blocking_delayed_native_record_with_a_conditional_source_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_delayed_native_record_conditional",
        "CODEGEN-001",
        Some("12:5"),
        "blocking delayed assignment of this native record shape in `tb` is not supported",
    );
}

#[test]
fn dense_member_record_forms_are_located_limits() {
    // The whole text: the message names the select, not lowering internals.
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "neg_record_call_member_select",
        "CODEGEN-002",
        Some("12:9"),
        "cannot resolve array base of select `a` in `tb`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_queue_row_select",
        "CODEGEN-002",
        Some("12:5"),
        "cannot resolve array base of select `a` in `tb`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_member_array_event_write",
        "CODEGEN-002",
        Some("12:5"),
        "cannot resolve hierarchical assignment LHS `a` in `tb`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_rows_runtime_index",
        "CODEGEN-001",
        Some("13:5"),
        "a whole record with a member array selected at a run-time index is not supported below 16 cells",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_container_element_to_fixed",
        "CODEGEN-009",
        Some("10:13"),
        "container element access in tb requires a scalar value",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_record_ref_member_wait_automatic",
        "CODEGEN-001",
        Some("13:5"),
        "fork branch in `tb` references native record or container `local_r` of the enclosing activation",
    );
}

/// Formerly listed as limits: see the fixture header and the readme.
#[test]
fn dense_member_record_forms_execute() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_audit/record_member_forms.out");
    sim_cli::run_case(SUITE, "record_member_forms", expected, "", &[]);
}

#[test]
fn deferred_action_with_an_associative_array_argument_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_associative_array_action",
        "CODEGEN-001",
        Some("11:36"),
        "unpacked aggregate, container and event arguments of deferred immediate assertion actions are not supported",
    );
}

#[test]
fn string_declared_in_a_function_fork_branch_is_a_located_limit() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_string_in_function_fork",
        "CODEGEN-001",
        Some("6:25"),
        "string/class signals are not supported: `s` in `c`",
    );
}

#[test]
fn procedural_assertion_saved_values_outside_attempts_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_checker_const_cast",
        "CODEGEN-009",
        Some("7:39"),
        "a const cast in a procedural concurrent assertion takes its value when the assertion is queued",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_procedural_disable_iff_automatic",
        "CODEGEN-009",
        Some("9:5"),
        "the `disable iff` condition of this procedural concurrent assertion reads an automatic variable or a const cast",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_procedural_clock_iff_const",
        "CODEGEN-009",
        Some("8:5"),
        "the clock's `iff` condition of this procedural concurrent assertion reads an automatic variable or a const cast",
    );
}

#[test]
fn randomization_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_assoc_handles",
        "CODEGEN-001",
        Some("12:21"),
        "randomization: array `m` is not supported in constraints: associative arrays are not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_assoc_wide_key",
        "CODEGEN-001",
        Some("11:21"),
        "randomization: array `m` is not supported in constraints: an associative array needs an integral index type of at most 64 bits",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_assoc_class_key",
        "CODEGEN-001",
        Some("13:21"),
        "randomization: array `m` is not supported in constraints: an associative array needs an integral index type of at most 64 bits",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_nested_queue",
        "CODEGEN-001",
        Some("10:21"),
        "randomization: array `q` is not supported in constraints: its elements are not packed integral values",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_solve_element_index",
        "CODEGEN-009",
        Some("8:53"),
        "randomization: a `solve...before` array element needs a constant index inside the array",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_inside_row",
        "CODEGEN-001",
        Some("7:30"),
        "this use of a row of a multidimensional class property in `tb` is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_rand_element_mode_packed",
        "CODEGEN-001",
        Some("9:5"),
        "randomization: `rand_mode` of this member is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_randomize_event_control",
        "CODEGEN-001",
        Some("8:7"),
        "function calls in evaluated event controls are not supported in `tb`: method call `randomize`",
    );
}

#[test]
fn uvm_example_difference_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_prefix_increment_container_member",
        "CODEGEN-002",
        Some("13:11"),
        "cannot resolve array base of select `count` in `tb`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_clocking_block_sequence",
        "CODEGEN-009",
        Some("13:10"),
        "concurrent assertions require one explicit signal clock",
    );
}

#[test]
fn typename_of_a_type_parameter_prints_the_resolved_type() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_audit/typename_type_parameter.out");
    sim_cli::run_case(SUITE, "typename_type_parameter", expected, "", &[]);
}

/// A user VPI system function called from a static declaration initializer
/// runs, since the application starts before static initialization
/// (S43-D12). A known issue stays pinned: the delayed put its calltf
/// schedules matures at time 1 instead of stopping the run with the S13-D5
/// error; its fix updates this test.
#[test]
fn vpi_function_called_from_a_static_initializer_runs_its_delayed_put() {
    let directory = sim_harness::TempDir::new("ki-audit-vpi").expect("temporary directory");
    let Some(library) =
        super::sim_040::plugin_library(SUITE, &directory, "vpi_static_initializer", &[])
    else {
        return;
    };
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "vpi_static_initializer",
        &["--define-system-task", "function int $sched()"],
        &[("LLG_VPI_PLUGIN", &library)],
        &|label, output| {
            let seen = sim_cli::observed_output(output);
            assert!(output.status.success(), "{label}: {seen}");
            assert_eq!(output.stdout, b"x=5 y=7\n", "{label}: {seen}");
            assert!(
                !String::from_utf8_lossy(&output.stderr).contains("VPI error"),
                "{label}: {seen}"
            );
        },
    );
}

/// FIN-001 closing audit: forms of sections added since the X1 audit that
/// had no test. Each stays a located compile-time limit.
#[test]
fn closing_audit_container_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_packed_member_queue_read",
        "CODEGEN-001",
        Some("10:20"),
        "hierarchical reference `hi` is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_packed_member_dynamic_write",
        "CODEGEN-002",
        Some("10:5"),
        "cannot resolve hierarchical assignment LHS `lo`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_packed_member_assoc_increment",
        "CODEGEN-002",
        Some("10:5"),
        "cannot resolve hierarchical assignment LHS `hi`",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_real_array_item",
        "CODEGEN-009",
        Some("7:10"),
        "resizable container assignment in `tb` requires a compatible array",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_event_array_item",
        "CODEGEN-009",
        Some("7:10"),
        "resizable container assignment in `tb` requires a compatible array",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_rows_into_nested_queue",
        "CODEGEN-003",
        Some("8:11"),
        "unpacked array concatenation item in `tb` is an array whose elements are not of the element type of the target",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_queue_rows_into_fixed",
        "CODEGEN-003",
        Some("9:10"),
        "dynamic or queue array to fixed unpacked-array assignment in `tb` requires a runtime-compatible fixed size",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_real_fixed_target",
        "CODEGEN-003",
        Some("7:11"),
        "dynamic or queue array to fixed unpacked-array assignment in `tb` requires a runtime-compatible fixed size",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_concat_associative_fixed_element",
        "CODEGEN-006",
        Some("7:5"),
        "native record assignment in `tb` has no record source",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_real_array_to_queue",
        "CODEGEN-003",
        Some("7:10"),
        "bit-stream cast destination requires a packed container element",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_inside_conditional_nested",
        "CODEGEN-009",
        Some("10:35"),
        "resizable container assignment in `tb` requires a compatible array",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_chandle_key_find_index",
        "CODEGEN-003",
        Some("8:9"),
        "array method `find_index` in `tb` currently requires packed source and destination elements",
    );
}

#[test]
fn closing_audit_event_and_randomize_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_vif_event_queue_trigger",
        "CODEGEN-002",
        Some("14:8"),
        "cannot resolve named event expression",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_vif_event_no_instance",
        "CODEGEN-002",
        Some("10:20"),
        "cannot resolve named event expression",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_class_event_array_actual",
        "CODEGEN-002",
        Some("13:7"),
        "an element of an event array of a class object or a virtual interface cannot be an output, inout or ref event actual",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_randomize_continuous",
        "CODEGEN-001",
        Some("9:14"),
        "a randomize call used as a value is not supported in a wait condition, an event control or outside procedural code",
    );
}

/// Forms the known issues listed as rejected that run; expected values are
/// derived by hand in the readme.
#[test]
fn closing_audit_served_forms_run() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_audit/closing_served_forms.out");
    sim_cli::run_case_backend_parity(SUITE, "closing_served_forms", expected, &[], &[]);
}

/// Two located limits of sections added since the X1 audit whose messages
/// no section named: a nested assignment to an unpacked value selected with
/// a call, and a handle set member that calls a function.
#[test]
fn closing_audit_operand_and_handle_set_forms_are_located_limits() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_nested_assignment_unpacked_call_target",
        "CODEGEN-001",
        Some("13:11"),
        "an assignment operator to an unpacked value selected with a call or an assignment in an operand of an expression in `tb` is not supported",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_inside_handle_call_member",
        "CODEGEN-009",
        Some("12:21"),
        "handle inside set in `tb` must list handle variables, properties, elements, arrays or null without side effects",
    );
}
