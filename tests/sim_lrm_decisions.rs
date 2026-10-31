//! Portable LRM decision cases (IEEE 1800-2009). Every fixture under
//! `tests/fixtures/sim/lrm_decisions/` is plain SystemVerilog with top `tb`,
//! no tool-specific options, and its exact expected stdout in `<case>.out`;
//! the header comment quotes the clause and states the decision, so the case
//! can be rerun unchanged on another simulator.
//! Each runs through the public CLI in both optimizer modes on the legacy,
//! compact/portable and compact/GMP value backends.
use crate::sim_cli;

const SUITE: &str = "lrm_decisions";

/// Exact stdout on every backend and optimizer mode, with identical stderr
/// across backends.
fn run_decision(case: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, case, expected, &[], &[]);
}

#[test]
fn aa_d1_implication_one_result() {
    run_decision(
        "AA-D1_implication_one_result",
        include_str!("fixtures/sim/lrm_decisions/AA-D1_implication_one_result.out"),
    );
}

#[test]
fn aa_n1_implication_result_time() {
    run_decision(
        "AA-N1_implication_result_time",
        include_str!("fixtures/sim/lrm_decisions/AA-N1_implication_result_time.out"),
    );
}

#[test]
fn aa_d3_cover_sequence_all_matches() {
    run_decision(
        "AA-D3_cover_sequence_all_matches",
        include_str!("fixtures/sim/lrm_decisions/AA-D3_cover_sequence_all_matches.out"),
    );
}

#[test]
fn aa_d4_leading_range_empty() {
    run_decision(
        "AA-D4_leading_range_empty",
        include_str!("fixtures/sim/lrm_decisions/AA-D4_leading_range_empty.out"),
    );
}

#[test]
fn aa_d5_match_multiplicity() {
    run_decision(
        "AA-D5_match_multiplicity",
        include_str!("fixtures/sim/lrm_decisions/AA-D5_match_multiplicity.out"),
    );
}

#[test]
fn aa_d2_default_clocking_iff() {
    run_decision(
        "AA-D2_default_clocking_iff",
        include_str!("fixtures/sim/lrm_decisions/AA-D2_default_clocking_iff.out"),
    );
}

#[test]
fn aa_d6_event_clock_gate_instant() {
    run_decision(
        "AA-D6_event_clock_gate_instant",
        include_str!("fixtures/sim/lrm_decisions/AA-D6_event_clock_gate_instant.out"),
    );
}

#[test]
fn aa_d7_real_sampled_illegal() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/AA-D7_real_sampled_illegal.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "AA-D7_real_sampled_illegal",
        "$past of a real expression is illegal",
    );
}

#[test]
fn aa_i1_nested_program() {
    run_decision(
        "AA-I1_nested_program",
        include_str!("fixtures/sim/lrm_decisions/AA-I1_nested_program.out"),
    );
}

#[test]
fn b1_string_property_wait() {
    run_decision(
        "B1_string_property_wait",
        include_str!("fixtures/sim/lrm_decisions/B1_string_property_wait.out"),
    );
}

#[test]
fn b2_function_member_wait() {
    run_decision(
        "B2_function_member_wait",
        include_str!("fixtures/sim/lrm_decisions/B2_function_member_wait.out"),
    );
}

#[test]
fn b3_resume_resensitize() {
    run_decision(
        "B3_resume_resensitize",
        include_str!("fixtures/sim/lrm_decisions/B3_resume_resensitize.out"),
    );
}

#[test]
fn b8_context_import_strobe() {
    let directory = crate::sim_harness::TempDir::new("lrm-b8").expect("temporary directory");
    let Some(library) = dpi_companion(&directory, "B8_context_import_strobe") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "B8_context_import_strobe",
        include_str!("fixtures/sim/lrm_decisions/B8_context_import_strobe.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn b4_handle_write_events() {
    run_decision(
        "B4_handle_write_events",
        include_str!("fixtures/sim/lrm_decisions/B4_handle_write_events.out"),
    );
}

#[test]
fn ab_o1_with_evaluation_order() {
    run_decision(
        "AB-O1_with_evaluation_order",
        include_str!("fixtures/sim/lrm_decisions/AB-O1_with_evaluation_order.out"),
    );
}

#[test]
fn ab_o2_unique_order() {
    run_decision(
        "AB-O2_unique_order",
        include_str!("fixtures/sim/lrm_decisions/AB-O2_unique_order.out"),
    );
}

#[test]
fn ab_o3_kill_woken_receiver() {
    run_decision(
        "AB-O3_kill_woken_receiver",
        include_str!("fixtures/sim/lrm_decisions/AB-O3_kill_woken_receiver.out"),
    );
}

#[test]
fn ab_n1_suspended_queue_waiter() {
    run_decision(
        "AB-N1_suspended_queue_waiter",
        include_str!("fixtures/sim/lrm_decisions/AB-N1_suspended_queue_waiter.out"),
    );
}

#[test]
fn ab_n4_semaphore_waiter_order() {
    run_decision(
        "AB-N4_semaphore_waiter_order",
        include_str!("fixtures/sim/lrm_decisions/AB-N4_semaphore_waiter_order.out"),
    );
}

#[test]
fn ab_n2_always_comb_class_reference() {
    run_decision(
        "AB-N2_always_comb_class_reference",
        include_str!("fixtures/sim/lrm_decisions/AB-N2_always_comb_class_reference.out"),
    );
}

#[test]
fn ab_n3_disable_branch_status() {
    run_decision(
        "AB-N3_disable_branch_status",
        include_str!("fixtures/sim/lrm_decisions/AB-N3_disable_branch_status.out"),
    );
}

#[test]
fn s14_d1_real_repeat_count() {
    run_decision(
        "S14-D1_real_repeat_count",
        include_str!("fixtures/sim/lrm_decisions/S14-D1_real_repeat_count.out"),
    );
}

#[test]
fn s14_d2_nonpositive_repeat_immediate() {
    run_decision(
        "S14-D2_nonpositive_repeat_immediate",
        include_str!("fixtures/sim/lrm_decisions/S14-D2_nonpositive_repeat_immediate.out"),
    );
}

#[test]
fn s14_d3_nb_trigger_target_at_issue() {
    run_decision(
        "S14-D3_nb_trigger_target_at_issue",
        include_str!("fixtures/sim/lrm_decisions/S14-D3_nb_trigger_target_at_issue.out"),
    );
}

#[test]
fn s14_d4_wait_order_first_triggered() {
    run_decision(
        "S14-D4_wait_order_first_triggered",
        include_str!("fixtures/sim/lrm_decisions/S14-D4_wait_order_first_triggered.out"),
    );
}

#[test]
fn s14_d5_class_property_nba() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/S14-D5_class_property_nba.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "S14-D5_class_property_nba",
        "nonblocking assignment to class property `x`",
    );
}

/// The failure ends the run at once, without final procedures.
#[test]
fn s14_d6_wait_order_error_ends_run() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S14-D6_wait_order_error_ends_run.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S14-D6_wait_order_error_ends_run",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "wait_order: events triggered out of order",
            );
        },
    );
}

#[test]
fn s23_d1_release_reestablishes_continuous() {
    run_decision(
        "S23-D1_release_reestablishes_continuous",
        include_str!("fixtures/sim/lrm_decisions/S23-D1_release_reestablishes_continuous.out"),
    );
}

#[test]
fn s23_d2_force_non_singular() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/S23-D2_force_non_singular.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "S23-D2_force_non_singular",
        "is an unpacked structure or union, which is not a singular variable (IEEE 1800-2009 10.6.2)",
    );
}

#[test]
fn s23_d3_net_member_select() {
    run_decision(
        "S23-D3_net_member_select",
        include_str!("fixtures/sim/lrm_decisions/S23-D3_net_member_select.out"),
    );
}

#[test]
fn s23_d4_partial_release() {
    run_decision(
        "S23-D4_partial_release",
        include_str!("fixtures/sim/lrm_decisions/S23-D4_partial_release.out"),
    );
}

#[test]
fn s20_1_string_stream_bytes() {
    run_decision(
        "S20-1_string_stream_bytes",
        include_str!("fixtures/sim/lrm_decisions/S20-1_string_stream_bytes.out"),
    );
}

#[test]
fn s20_2_greedy_dynamic_targets() {
    run_decision(
        "S20-2_greedy_dynamic_targets",
        include_str!("fixtures/sim/lrm_decisions/S20-2_greedy_dynamic_targets.out"),
    );
}

#[test]
fn s20_3_dynamic_target_fill() {
    run_decision(
        "S20-3_dynamic_target_fill",
        include_str!("fixtures/sim/lrm_decisions/S20-3_dynamic_target_fill.out"),
    );
}

#[test]
fn s20_4_with_range_resize() {
    run_decision(
        "S20-4_with_range_resize",
        include_str!("fixtures/sim/lrm_decisions/S20-4_with_range_resize.out"),
    );
}

#[test]
fn s20_5_reverse_unpack_consumed() {
    run_decision(
        "S20-5_reverse_unpack_consumed",
        include_str!("fixtures/sim/lrm_decisions/S20-5_reverse_unpack_consumed.out"),
    );
}

#[test]
fn s13_d1_copy_out_target_at_return() {
    run_decision(
        "S13-D1_copy_out_target_at_return",
        include_str!("fixtures/sim/lrm_decisions/S13-D1_copy_out_target_at_return.out"),
    );
}

#[test]
fn r05_d1_aggregate_conditional_leaves() {
    run_decision(
        "R05-D1_aggregate_conditional_leaves",
        include_str!("fixtures/sim/lrm_decisions/R05-D1_aggregate_conditional_leaves.out"),
    );
}

#[test]
fn r11_d1_sibling_port_net_types() {
    run_decision(
        "R11-D1_sibling_port_net_types",
        include_str!("fixtures/sim/lrm_decisions/R11-D1_sibling_port_net_types.out"),
    );
}

#[test]
fn s20_7_unpack_uneven_slices() {
    run_decision(
        "S20-7_unpack_uneven_slices",
        include_str!("fixtures/sim/lrm_decisions/S20-7_unpack_uneven_slices.out"),
    );
}

/// Negative at run time: the output before the error is portable.
#[test]
fn s20_6_dynamic_cast_whole_elements() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S20-6_dynamic_cast_whole_elements.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S20-6_dynamic_cast_whole_elements",
        &[],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(
                    "bit stream size does not match a whole number of destination elements"
                ),
                "{label}: {output:?}"
            );
        },
    );
}

#[test]
fn s24_d1_signed_decimal_field_width() {
    run_decision(
        "S24-D1_signed_decimal_field_width",
        include_str!("fixtures/sim/lrm_decisions/S24-D1_signed_decimal_field_width.out"),
    );
}

#[test]
fn s26_d1_scan_selector_snapshot() {
    run_decision(
        "S26-D1_scan_selector_snapshot",
        include_str!("fixtures/sim/lrm_decisions/S26-D1_scan_selector_snapshot.out"),
    );
}

#[test]
fn s26_d2_invalid_descriptor_input() {
    run_decision(
        "S26-D2_invalid_descriptor_input",
        include_str!("fixtures/sim/lrm_decisions/S26-D2_invalid_descriptor_input.out"),
    );
}

#[test]
fn s26_d2_fread_ends_run() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S26-D2_fread_ends_run.out");
    sim_cli::run_case_checked_matrix(SUITE, "S26-D2_fread_ends_run", &[], &|label, output| {
        runtime_error(label, output, expected, "$fread: invalid file descriptor");
    });
}

#[test]
fn s26_d21_fgets_nothing_read() {
    run_decision(
        "S26-D21_fgets_nothing_read",
        include_str!("fixtures/sim/lrm_decisions/S26-D21_fgets_nothing_read.out"),
    );
}

#[test]
fn s26_d22_real_field_overflow() {
    run_decision(
        "S26-D22_real_field_overflow",
        include_str!("fixtures/sim/lrm_decisions/S26-D22_real_field_overflow.out"),
    );
}

#[test]
fn s26_d3_unformatted_words() {
    run_decision(
        "S26-D3_unformatted_words",
        include_str!("fixtures/sim/lrm_decisions/S26-D3_unformatted_words.out"),
    );
}

#[test]
fn s26_d4_scan_time_scaling() {
    run_decision(
        "S26-D4_scan_time_scaling",
        include_str!("fixtures/sim/lrm_decisions/S26-D4_scan_time_scaling.out"),
    );
}

#[test]
fn s26_d5_scan_strength() {
    run_decision(
        "S26-D5_scan_strength",
        include_str!("fixtures/sim/lrm_decisions/S26-D5_scan_strength.out"),
    );
}

#[test]
fn s26_d6_integer_scan_into_real() {
    run_decision(
        "S26-D6_integer_scan_into_real",
        include_str!("fixtures/sim/lrm_decisions/S26-D6_integer_scan_into_real.out"),
    );
}

#[test]
fn s26_d7_fread_short_last_word() {
    run_decision(
        "S26-D7_fread_short_last_word",
        include_str!("fixtures/sim/lrm_decisions/S26-D7_fread_short_last_word.out"),
    );
}

#[test]
fn s26_d8_fgets_packed_capacity() {
    run_decision(
        "S26-D8_fgets_packed_capacity",
        include_str!("fixtures/sim/lrm_decisions/S26-D8_fgets_packed_capacity.out"),
    );
}

#[test]
fn s26_d9_scan_string_into_byte_array() {
    run_decision(
        "S26-D9_scan_string_into_byte_array",
        include_str!("fixtures/sim/lrm_decisions/S26-D9_scan_string_into_byte_array.out"),
    );
}

#[test]
fn s26_d10_suppressed_binary_conversion() {
    run_decision(
        "S26-D10_suppressed_binary_conversion",
        include_str!("fixtures/sim/lrm_decisions/S26-D10_suppressed_binary_conversion.out"),
    );
}

#[test]
fn s26_d11_seek_clears_error() {
    run_decision(
        "S26-D11_seek_clears_error",
        include_str!("fixtures/sim/lrm_decisions/S26-D11_seek_clears_error.out"),
    );
}

#[test]
fn s26_d12_packed_scan_source() {
    run_decision(
        "S26-D12_packed_scan_source",
        include_str!("fixtures/sim/lrm_decisions/S26-D12_packed_scan_source.out"),
    );
}

#[test]
fn s26_d13_scope_conversion_counts() {
    run_decision(
        "S26-D13_scope_conversion_counts",
        include_str!("fixtures/sim/lrm_decisions/S26-D13_scope_conversion_counts.out"),
    );
}

#[test]
fn s26_d14_ungetc_returns_zero() {
    run_decision(
        "S26-D14_ungetc_returns_zero",
        include_str!("fixtures/sim/lrm_decisions/S26-D14_ungetc_returns_zero.out"),
    );
}

#[test]
fn s26_d15_fwrite_z_encoding() {
    run_decision(
        "S26-D15_fwrite_z_encoding",
        include_str!("fixtures/sim/lrm_decisions/S26-D15_fwrite_z_encoding.out"),
    );
}

#[test]
fn s32_d1_implicit_finish_immediate() {
    run_decision(
        "S32-D1_implicit_finish_immediate",
        include_str!("fixtures/sim/lrm_decisions/S32-D1_implicit_finish_immediate.out"),
    );
}

#[test]
fn s32_d2_bind_program_into_interface() {
    run_decision(
        "S32-D2_bind_program_into_interface",
        include_str!("fixtures/sim/lrm_decisions/S32-D2_bind_program_into_interface.out"),
    );
}

#[test]
fn s25_d1_monitor_flag_persists() {
    run_decision(
        "S25-D1_monitor_flag_persists",
        include_str!("fixtures/sim/lrm_decisions/S25-D1_monitor_flag_persists.out"),
    );
}

#[test]
fn s25_d2_multiple_fmonitor() {
    run_decision(
        "S25-D2_multiple_fmonitor",
        include_str!("fixtures/sim/lrm_decisions/S25-D2_multiple_fmonitor.out"),
    );
}

#[test]
fn s25_d3_value_returns_within_slot() {
    run_decision(
        "S25-D3_value_returns_within_slot",
        include_str!("fixtures/sim/lrm_decisions/S25-D3_value_returns_within_slot.out"),
    );
}

#[test]
fn s25_d4_static_subroutine_reports() {
    run_decision(
        "S25-D4_static_subroutine_reports",
        include_str!("fixtures/sim/lrm_decisions/S25-D4_static_subroutine_reports.out"),
    );
}

#[test]
fn s25_d5_pending_report_at_finish() {
    run_decision(
        "S25-D5_pending_report_at_finish",
        include_str!("fixtures/sim/lrm_decisions/S25-D5_pending_report_at_finish.out"),
    );
}

#[test]
fn s25_d6_report_order_in_slot() {
    run_decision(
        "S25-D6_report_order_in_slot",
        include_str!("fixtures/sim/lrm_decisions/S25-D6_report_order_in_slot.out"),
    );
}

#[test]
fn s25_d7_output_to_closed_descriptors() {
    run_decision(
        "S25-D7_output_to_closed_descriptors",
        include_str!("fixtures/sim/lrm_decisions/S25-D7_output_to_closed_descriptors.out"),
    );
}

#[test]
fn s28_d1_implicit_random_seed() {
    run_decision(
        "S28-D1_implicit_random_seed",
        include_str!("fixtures/sim/lrm_decisions/S28-D1_implicit_random_seed.out"),
    );
}

#[test]
fn s28_d2_seed_writeback_width() {
    run_decision(
        "S28-D2_seed_writeback_width",
        include_str!("fixtures/sim/lrm_decisions/S28-D2_seed_writeback_width.out"),
    );
}

#[test]
fn s28_d3_unknown_legacy_seed() {
    run_decision(
        "S28-D3_unknown_legacy_seed",
        include_str!("fixtures/sim/lrm_decisions/S28-D3_unknown_legacy_seed.out"),
    );
}

#[test]
fn s28_d4_object_creation_draw() {
    run_decision(
        "S28-D4_object_creation_draw",
        include_str!("fixtures/sim/lrm_decisions/S28-D4_object_creation_draw.out"),
    );
}

#[test]
fn s28_d5_shuffle_thread_stream() {
    run_decision(
        "S28-D5_shuffle_thread_stream",
        include_str!("fixtures/sim/lrm_decisions/S28-D5_shuffle_thread_stream.out"),
    );
}

#[test]
fn s28_d6_shallow_copy_random_state() {
    run_decision(
        "S28-D6_shallow_copy_random_state",
        include_str!("fixtures/sim/lrm_decisions/S28-D6_shallow_copy_random_state.out"),
    );
}

#[test]
fn s28_d7_instance_streams() {
    run_decision(
        "S28-D7_instance_streams",
        include_str!("fixtures/sim/lrm_decisions/S28-D7_instance_streams.out"),
    );
}

#[test]
fn s28_d8_two_state_random_arguments() {
    run_decision(
        "S28-D8_two_state_random_arguments",
        include_str!("fixtures/sim/lrm_decisions/S28-D8_two_state_random_arguments.out"),
    );
}

#[test]
fn s36_d1_report_per_execution() {
    run_decision(
        "S36-D1_report_per_execution",
        include_str!("fixtures/sim/lrm_decisions/S36-D1_report_per_execution.out"),
    );
}

#[test]
fn s36_d2_flush_point_kinds() {
    run_decision(
        "S36-D2_flush_point_kinds",
        include_str!("fixtures/sim/lrm_decisions/S36-D2_flush_point_kinds.out"),
    );
}

#[test]
fn s36_d3_receiver_at_maturity() {
    run_decision(
        "S36-D3_receiver_at_maturity",
        include_str!("fixtures/sim/lrm_decisions/S36-D3_receiver_at_maturity.out"),
    );
}

#[test]
fn s36_d4_final_procedure_reports() {
    run_decision(
        "S36-D4_final_procedure_reports",
        include_str!("fixtures/sim/lrm_decisions/S36-D4_final_procedure_reports.out"),
    );
}

#[test]
fn s36_d5_kill_spares_matured() {
    run_decision(
        "S36-D5_kill_spares_matured",
        include_str!("fixtures/sim/lrm_decisions/S36-D5_kill_spares_matured.out"),
    );
}

#[test]
fn s36_d6_pending_reports_at_finish() {
    run_decision(
        "S36-D6_pending_reports_at_finish",
        include_str!("fixtures/sim/lrm_decisions/S36-D6_pending_reports_at_finish.out"),
    );
}

#[test]
fn s36_d7_module_item_sensitivity() {
    run_decision(
        "S36-D7_module_item_sensitivity",
        include_str!("fixtures/sim/lrm_decisions/S36-D7_module_item_sensitivity.out"),
    );
}

#[test]
fn s36_d8_dynamic_ref_actual() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/S36-D8_dynamic_ref_actual.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "S36-D8_dynamic_ref_actual",
        "cannot pass a dynamic variable to ref argument `x`",
    );
}

#[test]
fn s38_d1_end_of_simulation_strength() {
    run_decision(
        "S38-D1_end_of_simulation_strength",
        include_str!("fixtures/sim/lrm_decisions/S38-D1_end_of_simulation_strength.out"),
    );
}

#[test]
fn s38_d2_async_abort_sampled() {
    run_decision(
        "S38-D2_async_abort_sampled",
        include_str!("fixtures/sim/lrm_decisions/S38-D2_async_abort_sampled.out"),
    );
}

#[test]
fn s38_d3_pending_operand_vacuous() {
    run_decision(
        "S38-D3_pending_operand_vacuous",
        include_str!("fixtures/sim/lrm_decisions/S38-D3_pending_operand_vacuous.out"),
    );
}

#[test]
fn s38_d4_expect_inline_action() {
    run_decision(
        "S38-D4_expect_inline_action",
        include_str!("fixtures/sim/lrm_decisions/S38-D4_expect_inline_action.out"),
    );
}

#[test]
fn s38_d5_default_disable_scope() {
    run_decision(
        "S38-D5_default_disable_scope",
        include_str!("fixtures/sim/lrm_decisions/S38-D5_default_disable_scope.out"),
    );
}

#[test]
fn s38_d6_initial_assertion_single_attempt() {
    run_decision(
        "S38-D6_initial_assertion_single_attempt",
        include_str!("fixtures/sim/lrm_decisions/S38-D6_initial_assertion_single_attempt.out"),
    );
}

#[test]
fn s38_d7_action_blocks_independent() {
    run_decision(
        "S38-D7_action_blocks_independent",
        include_str!("fixtures/sim/lrm_decisions/S38-D7_action_blocks_independent.out"),
    );
}

#[test]
fn s38_d8_until_with_vacuity() {
    run_decision(
        "S38-D8_until_with_vacuity",
        include_str!("fixtures/sim/lrm_decisions/S38-D8_until_with_vacuity.out"),
    );
}

#[test]
fn s38_d9_sync_abort_new_attempt() {
    run_decision(
        "S38-D9_sync_abort_new_attempt",
        include_str!("fixtures/sim/lrm_decisions/S38-D9_sync_abort_new_attempt.out"),
    );
}

#[test]
fn s38_d10_cover_vacuous_success() {
    run_decision(
        "S38-D10_cover_vacuous_success",
        include_str!("fixtures/sim/lrm_decisions/S38-D10_cover_vacuous_success.out"),
    );
}

#[test]
fn s38_d11_assert_vacuous_pass_action() {
    run_decision(
        "S38-D11_assert_vacuous_pass_action",
        include_str!("fixtures/sim/lrm_decisions/S38-D11_assert_vacuous_pass_action.out"),
    );
}

#[test]
fn s38_d12_end_of_simulation_fail_before_final() {
    run_decision(
        "S38-D12_end_of_simulation_fail_before_final",
        include_str!("fixtures/sim/lrm_decisions/S38-D12_end_of_simulation_fail_before_final.out"),
    );
}

#[test]
fn s21_d1_builtin_legal_overload() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/S21-D1_builtin_legal_overload.out").is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "S21-D1_builtin_legal_overload",
        "operator '+' is already legal for the types of this overload prototype and cannot be overloaded",
    );
}

#[test]
fn s21_d2_record_result_overload_admitted() {
    run_decision(
        "S21-D2_record_result_overload_admitted",
        include_str!("fixtures/sim/lrm_decisions/S21-D2_record_result_overload_admitted.out"),
    );
}

#[test]
fn s34_d1_cycle_drive_maturation() {
    run_decision(
        "S34-D1_cycle_drive_maturation",
        include_str!("fixtures/sim/lrm_decisions/S34-D1_cycle_drive_maturation.out"),
    );
}

#[test]
fn s34_d2_clocking_net_driver() {
    run_decision(
        "S34-D2_clocking_net_driver",
        include_str!("fixtures/sim/lrm_decisions/S34-D2_clocking_net_driver.out"),
    );
}

#[test]
fn s34_d3_vif_drive_bound_at_issue() {
    run_decision(
        "S34-D3_vif_drive_bound_at_issue",
        include_str!("fixtures/sim/lrm_decisions/S34-D3_vif_drive_bound_at_issue.out"),
    );
}

#[test]
fn s34_d4_observed_wakeup_before_reactive() {
    run_decision(
        "S34-D4_observed_wakeup_before_reactive",
        include_str!("fixtures/sim/lrm_decisions/S34-D4_observed_wakeup_before_reactive.out"),
    );
}

/// Build the `.c` companion of a DPI-C decision case into a shared library in
/// `directory` and return the path for `--dpi-lib`. On Windows it is a DLL
/// linked against the import library of the case's own model, so its
/// `svdpi.h` and export calls bind to `sim.exe`, and the path is the DLL's
/// import library. `None` (with a SKIP note) only on a Windows host whose C
/// compiler is not MSVC.
fn dpi_companion(directory: &crate::sim_harness::TempDir, case: &str) -> Option<String> {
    if cfg!(windows) && !crate::c_compiler::is_msvc(&crate::c_compiler::host_c_compiler()) {
        eprintln!("SKIP: Windows DLL companions are built with MSVC only");
        return None;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join("tests/fixtures/sim/lrm_decisions");
    let import = cfg!(windows).then(|| {
        sim_cli::model_import_library(&fixtures.join(format!("{case}.sv")), &[], directory.path())
    });
    let library = crate::c_compiler::shared_library(
        &fixtures.join(format!("{case}.c")),
        directory.path(),
        case,
        &[],
        import.as_deref(),
    )
    .unwrap_or_else(|error| panic!("{case}.c: {error}"));
    Some(library.link.to_string_lossy().into_owned())
}

/// [`run_decision`] for a DPI-C case with a `.c` companion.
fn run_dpi_decision(case: &str, expected: &str) {
    let directory = crate::sim_harness::TempDir::new("lrm-s40").expect("temporary directory");
    let Some(library) = dpi_companion(&directory, case) else {
        return;
    };
    sim_cli::run_case_backend_parity(SUITE, case, expected, &["--dpi-lib", &library], &[]);
}

#[test]
fn s40_d1_output_initial_values() {
    run_dpi_decision(
        "S40-D1_output_initial_values",
        include_str!("fixtures/sim/lrm_decisions/S40-D1_output_initial_values.out"),
    );
}

#[test]
fn s40_d2_aliased_copy_out() {
    run_dpi_decision(
        "S40-D2_aliased_copy_out",
        include_str!("fixtures/sim/lrm_decisions/S40-D2_aliased_copy_out.out"),
    );
}

#[test]
fn s40_d3_open_array_queries() {
    run_dpi_decision(
        "S40-D3_open_array_queries",
        include_str!("fixtures/sim/lrm_decisions/S40-D3_open_array_queries.out"),
    );
}

#[test]
fn s40_d4_invalid_element_access() {
    run_dpi_decision(
        "S40-D4_invalid_element_access",
        include_str!("fixtures/sim/lrm_decisions/S40-D4_invalid_element_access.out"),
    );
}

/// A wrong number of indices ends the run when the import returns; the
/// final procedures run.
#[test]
fn s40_d6_wrong_index_count_ends_run() {
    run_dpi_fatal_decision(
        "S40-D6_wrong_index_count_ends_run",
        include_str!("fixtures/sim/lrm_decisions/S40-D6_wrong_index_count_ends_run.out"),
        "llg: DPI-C open array element access with 1 index(es) to an array with 2 unpacked \
         dimension(s)",
    );
}

#[test]
fn s40_d5_task_disable_protocol() {
    let case = "S40-D5_task_disable_protocol";
    let directory = crate::sim_harness::TempDir::new("lrm-s40-d5").expect("temporary directory");
    let Some(library) = dpi_companion(&directory, case) else {
        return;
    };
    let expected = include_str!("fixtures/sim/lrm_decisions/S40-D5_task_disable_protocol.out");
    sim_cli::run_case_checked_matrix(SUITE, case, &["--dpi-lib", &library], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(
                "llg: DPI-C import task `d5_task` returned nonzero, but nothing disabled it"
            ),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn s41_d1_context_scope_chain() {
    run_dpi_decision(
        "S41-D1_context_scope_chain",
        include_str!("fixtures/sim/lrm_decisions/S41-D1_context_scope_chain.out"),
    );
}

#[test]
fn s41_d2_export_copy_back() {
    run_dpi_decision(
        "S41-D2_export_copy_back",
        include_str!("fixtures/sim/lrm_decisions/S41-D2_export_copy_back.out"),
    );
}

#[test]
fn s41_d3_noncontext_has_no_scope() {
    run_dpi_decision(
        "S41-D3_noncontext_has_no_scope",
        include_str!("fixtures/sim/lrm_decisions/S41-D3_noncontext_has_no_scope.out"),
    );
}

#[test]
fn s41_d4_scope_routine_arguments() {
    run_dpi_decision(
        "S41-D4_scope_routine_arguments",
        include_str!("fixtures/sim/lrm_decisions/S41-D4_scope_routine_arguments.out"),
    );
}

#[test]
fn s41_d5_disabled_outputs() {
    run_dpi_decision(
        "S41-D5_disabled_outputs",
        include_str!("fixtures/sim/lrm_decisions/S41-D5_disabled_outputs.out"),
    );
}

/// [`run_dpi_decision`] for a case that stops with a fatal DPI-C protocol
/// error after its expected output; `message` is the stderr report.
fn run_dpi_fatal_decision(case: &str, expected: &str, message: &str) {
    let directory = crate::sim_harness::TempDir::new("lrm-s41").expect("temporary directory");
    let Some(library) = dpi_companion(&directory, case) else {
        return;
    };
    sim_cli::run_case_checked_matrix(SUITE, case, &["--dpi-lib", &library], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn s41_d6_noncontext_export_call() {
    run_dpi_fatal_decision(
        "S41-D6_noncontext_export_call",
        include_str!("fixtures/sim/lrm_decisions/S41-D6_noncontext_export_call.out"),
        "llg: DPI-C export `d6_inc` was called from a noncontext import",
    );
}

#[test]
fn s41_d7_export_outside_scope() {
    run_dpi_fatal_decision(
        "S41-D7_export_outside_scope",
        include_str!("fixtures/sim/lrm_decisions/S41-D7_export_outside_scope.out"),
        "llg: DPI-C export `d7_inc` is not exported from the current scope `tb.u`",
    );
}

#[test]
fn s41_d8_task_from_import_function() {
    run_dpi_decision(
        "S41-D8_task_from_import_function",
        include_str!("fixtures/sim/lrm_decisions/S41-D8_task_from_import_function.out"),
    );
}

#[test]
fn a18_d1_kill_unwinds_suspended_chain() {
    run_dpi_decision(
        "A18-D1_kill_unwinds_suspended_chain",
        include_str!("fixtures/sim/lrm_decisions/A18-D1_kill_unwinds_suspended_chain.out"),
    );
}

#[test]
fn a18_d2_finish_leaves_chain_suspended() {
    run_dpi_decision(
        "A18-D2_finish_leaves_chain_suspended",
        include_str!("fixtures/sim/lrm_decisions/A18-D2_finish_leaves_chain_suspended.out"),
    );
}

#[test]
fn a18_d3_waiting_task_from_import_function() {
    run_dpi_fatal_decision(
        "A18-D3_waiting_task_from_import_function",
        include_str!("fixtures/sim/lrm_decisions/A18-D3_waiting_task_from_import_function.out"),
        "llg: DPI-C exported task `a3_wait` can consume time, but its foreign caller cannot \
         suspend",
    );
}

#[test]
fn s09_d1_string_ref_wait() {
    run_decision(
        "S09-D1_string_ref_wait",
        include_str!("fixtures/sim/lrm_decisions/S09-D1_string_ref_wait.out"),
    );
}

#[test]
fn s09_d2_handle_ref_event() {
    run_decision(
        "S09-D2_handle_ref_event",
        include_str!("fixtures/sim/lrm_decisions/S09-D2_handle_ref_event.out"),
    );
}

#[test]
fn s09_d3_ref_implicit_sensitivity() {
    run_decision(
        "S09-D3_ref_implicit_sensitivity",
        include_str!("fixtures/sim/lrm_decisions/S09-D3_ref_implicit_sensitivity.out"),
    );
}

#[test]
fn s09_d4_class_property_ref_wait() {
    run_decision(
        "S09-D4_class_property_ref_wait",
        include_str!("fixtures/sim/lrm_decisions/S09-D4_class_property_ref_wait.out"),
    );
}

#[test]
fn s09_d5_handle_ref_store_immediate() {
    run_decision(
        "S09-D5_handle_ref_store_immediate",
        include_str!("fixtures/sim/lrm_decisions/S09-D5_handle_ref_store_immediate.out"),
    );
}

#[test]
fn s09_d6_vif_ref_rebinding() {
    run_decision(
        "S09-D6_vif_ref_rebinding",
        include_str!("fixtures/sim/lrm_decisions/S09-D6_vif_ref_rebinding.out"),
    );
}

#[test]
fn s09_d7_fork_shared_native_wait() {
    run_decision(
        "S09-D7_fork_shared_native_wait",
        include_str!("fixtures/sim/lrm_decisions/S09-D7_fork_shared_native_wait.out"),
    );
}

#[test]
fn s27_d1_load_wakes_changed_words() {
    run_decision(
        "S27-D1_load_wakes_changed_words",
        include_str!("fixtures/sim/lrm_decisions/S27-D1_load_wakes_changed_words.out"),
    );
}

#[test]
fn s27_d2_word_is_unsized_literal() {
    run_decision(
        "S27-D2_word_is_unsized_literal",
        include_str!("fixtures/sim/lrm_decisions/S27-D2_word_is_unsized_literal.out"),
    );
}

#[test]
fn s27_d3_two_state_unknown_digits() {
    run_decision(
        "S27-D3_two_state_unknown_digits",
        include_str!("fixtures/sim/lrm_decisions/S27-D3_two_state_unknown_digits.out"),
    );
}

#[test]
fn s27_d4_enum_word_membership() {
    run_decision(
        "S27-D4_enum_word_membership",
        include_str!("fixtures/sim/lrm_decisions/S27-D4_enum_word_membership.out"),
    );
}

#[test]
fn s27_d5_malformed_word_ends_load() {
    run_decision(
        "S27-D5_malformed_word_ends_load",
        include_str!("fixtures/sim/lrm_decisions/S27-D5_malformed_word_ends_load.out"),
    );
}

#[test]
fn s27_d6_file_address_outside_range() {
    run_decision(
        "S27-D6_file_address_outside_range",
        include_str!("fixtures/sim/lrm_decisions/S27-D6_file_address_outside_range.out"),
    );
}

#[test]
fn s27_d7_address_outside_memory() {
    run_decision(
        "S27-D7_address_outside_memory",
        include_str!("fixtures/sim/lrm_decisions/S27-D7_address_outside_memory.out"),
    );
}

#[test]
fn s27_d8_words_past_the_end() {
    run_decision(
        "S27-D8_words_past_the_end",
        include_str!("fixtures/sim/lrm_decisions/S27-D8_words_past_the_end.out"),
    );
}

#[test]
fn s27_d9_missing_file() {
    run_decision(
        "S27-D9_missing_file",
        include_str!("fixtures/sim/lrm_decisions/S27-D9_missing_file.out"),
    );
}

#[test]
fn s27_d10_resizable_memory_size() {
    run_decision(
        "S27-D10_resizable_memory_size",
        include_str!("fixtures/sim/lrm_decisions/S27-D10_resizable_memory_size.out"),
    );
}

#[test]
fn s27_d11_dump_order_without_addresses() {
    run_decision(
        "S27-D11_dump_order_without_addresses",
        include_str!("fixtures/sim/lrm_decisions/S27-D11_dump_order_without_addresses.out"),
    );
}

#[test]
fn s27_d12_rejected_dump_keeps_file() {
    run_decision(
        "S27-D12_rejected_dump_keeps_file",
        include_str!("fixtures/sim/lrm_decisions/S27-D12_rejected_dump_keeps_file.out"),
    );
}

#[test]
fn s31_d1_exact_weight_sum() {
    run_decision(
        "S31-D1_exact_weight_sum",
        include_str!("fixtures/sim/lrm_decisions/S31-D1_exact_weight_sum.out"),
    );
}

#[test]
fn s31_d2_weight_precision_self_determined() {
    run_decision(
        "S31-D2_weight_precision_self_determined",
        include_str!("fixtures/sim/lrm_decisions/S31-D2_weight_precision_self_determined.out"),
    );
}

#[test]
fn s31_d3_zero_weights() {
    run_decision(
        "S31-D3_zero_weights",
        include_str!("fixtures/sim/lrm_decisions/S31-D3_zero_weights.out"),
    );
}

/// A negative weight ends the run at once, without final procedures.
#[test]
fn s31_d4_negative_weight_ends_run() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S31-D4_negative_weight_ends_run.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S31-D4_negative_weight_ends_run",
        &[],
        &|label, output| {
            runtime_error(label, output, expected, "weight 0 is negative");
        },
    );
}

#[test]
fn s31_d5_unknown_weight_bits() {
    run_decision(
        "S31-D5_unknown_weight_bits",
        include_str!("fixtures/sim/lrm_decisions/S31-D5_unknown_weight_bits.out"),
    );
}

#[test]
fn s31_d6_weight_evaluation_order() {
    run_decision(
        "S31-D6_weight_evaluation_order",
        include_str!("fixtures/sim/lrm_decisions/S31-D6_weight_evaluation_order.out"),
    );
}

#[test]
fn s27_d13_memory_name_evaluated_once() {
    run_decision(
        "S27-D13_memory_name_evaluated_once",
        include_str!("fixtures/sim/lrm_decisions/S27-D13_memory_name_evaluated_once.out"),
    );
}

#[test]
fn s27_d14_resizable_slice_outside_size() {
    run_decision(
        "S27-D14_resizable_slice_outside_size",
        include_str!("fixtures/sim/lrm_decisions/S27-D14_resizable_slice_outside_size.out"),
    );
}

#[test]
fn s27_d15_associative_load_entries() {
    run_decision(
        "S27-D15_associative_load_entries",
        include_str!("fixtures/sim/lrm_decisions/S27-D15_associative_load_entries.out"),
    );
}

#[test]
fn s27_d16_associative_address_cast() {
    run_decision(
        "S27-D16_associative_address_cast",
        include_str!("fixtures/sim/lrm_decisions/S27-D16_associative_address_cast.out"),
    );
}

#[test]
fn s27_d17_associative_dump_addresses() {
    run_decision(
        "S27-D17_associative_dump_addresses",
        include_str!("fixtures/sim/lrm_decisions/S27-D17_associative_dump_addresses.out"),
    );
}

#[test]
fn s27_d18_associative_wildcard_rejected() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/S27-D18_associative_wildcard_rejected.out")
            .is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "S27-D18_associative_wildcard_rejected",
        "requires an associative array with an index of integral type, not a wildcard `[*]` index",
    );
}

#[test]
fn s22_d1_handle_initializers_before_processes() {
    run_decision(
        "S22-D1_handle_initializers_before_processes",
        include_str!("fixtures/sim/lrm_decisions/S22-D1_handle_initializers_before_processes.out"),
    );
}

#[test]
fn s22_d2_static_locals_initialize_once() {
    run_decision(
        "S22-D2_static_locals_initialize_once",
        include_str!("fixtures/sim/lrm_decisions/S22-D2_static_locals_initialize_once.out"),
    );
}

#[test]
fn s22_d3_initializer_deferred_effect_illegal() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!(
        "fixtures/sim/lrm_decisions/S22-D3_initializer_deferred_effect_illegal.out"
    )
    .is_empty());
    sim_cli::reject_case(
        SUITE,
        "S22-D3_initializer_deferred_effect_illegal",
        "calls `watch`, whose fork at ",
    );
}

#[test]
fn s22_d4_const_method_write_illegal() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/S22-D4_const_method_write_illegal.out").is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "S22-D4_const_method_write_illegal",
        "`push_back` modifies `limits`, which is declared const",
    );
}

#[test]
fn s22_d5_implicit_static_initializer() {
    run_decision(
        "S22-D5_implicit_static_initializer",
        include_str!("fixtures/sim/lrm_decisions/S22-D5_implicit_static_initializer.out"),
    );
}

#[test]
fn s22_d6_let_name_binding() {
    run_decision(
        "S22-D6_let_name_binding",
        include_str!("fixtures/sim/lrm_decisions/S22-D6_let_name_binding.out"),
    );
}

#[test]
fn s22_d7_static_property_initializers() {
    run_decision(
        "S22-D7_static_property_initializers",
        include_str!("fixtures/sim/lrm_decisions/S22-D7_static_property_initializers.out"),
    );
}

#[test]
fn s27_d19_variable_slice_position() {
    run_decision(
        "S27-D19_variable_slice_position",
        include_str!("fixtures/sim/lrm_decisions/S27-D19_variable_slice_position.out"),
    );
}

#[test]
fn s27_d20_queue_slice_bounds() {
    run_decision(
        "S27-D20_queue_slice_bounds",
        include_str!("fixtures/sim/lrm_decisions/S27-D20_queue_slice_bounds.out"),
    );
}

#[test]
fn s27_d21_container_row_memory() {
    run_decision(
        "S27-D21_container_row_memory",
        include_str!("fixtures/sim/lrm_decisions/S27-D21_container_row_memory.out"),
    );
}

#[test]
fn s27_d22_container_of_arrays_rejected() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/S27-D22_container_of_arrays_rejected.out")
            .is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "S27-D22_container_of_arrays_rejected",
        "$readmemh memory_name cannot be a whole dynamic, queue or associative array of unpacked arrays, or a slice of one; name one of its elements, at ",
    );
}

#[test]
fn s27_d23_associative_row_load_creates() {
    run_decision(
        "S27-D23_associative_row_load_creates",
        include_str!("fixtures/sim/lrm_decisions/S27-D23_associative_row_load_creates.out"),
    );
}

#[test]
fn s27_d24_associative_row_dump_missing() {
    run_decision(
        "S27-D24_associative_row_dump_missing",
        include_str!("fixtures/sim/lrm_decisions/S27-D24_associative_row_dump_missing.out"),
    );
}

#[test]
fn s27_d25_associative_row_dump_no_addresses() {
    run_decision(
        "S27-D25_associative_row_dump_no_addresses",
        include_str!("fixtures/sim/lrm_decisions/S27-D25_associative_row_dump_no_addresses.out"),
    );
}

/// Run-time error after portable output: exact stdout, exit status 1 and
/// `diagnostic` on stderr.
fn runtime_error(label: &str, output: &std::process::Output, expected: &str, diagnostic: &str) {
    assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(diagnostic),
        "{label}: {output:?}"
    );
}

#[test]
fn s29_d1_unknown_state_ends_run() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S29-D1_unknown_state_ends_run.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S29-D1_unknown_state_ends_run",
        &[],
        &|label, output| {
            runtime_error(label, output, expected, "X or Z value in a constraint");
        },
    );
}

#[test]
fn s29_d2_enum_named_values() {
    run_decision(
        "S29-D2_enum_named_values",
        include_str!("fixtures/sim/lrm_decisions/S29-D2_enum_named_values.out"),
    );
}

#[test]
fn s29_d3_scope_randomize_thread_stream() {
    run_decision(
        "S29-D3_scope_randomize_thread_stream",
        include_str!("fixtures/sim/lrm_decisions/S29-D3_scope_randomize_thread_stream.out"),
    );
}

#[test]
fn s29_d4_inside_uniform_per_value() {
    run_decision(
        "S29-D4_inside_uniform_per_value",
        include_str!("fixtures/sim/lrm_decisions/S29-D4_inside_uniform_per_value.out"),
    );
}

#[test]
fn s29_d5_unsat_warns_and_continues() {
    run_decision(
        "S29-D5_unsat_warns_and_continues",
        include_str!("fixtures/sim/lrm_decisions/S29-D5_unsat_warns_and_continues.out"),
    );
}

#[test]
fn s29_d6_failed_call_keeps_values() {
    run_decision(
        "S29-D6_failed_call_keeps_values",
        include_str!("fixtures/sim/lrm_decisions/S29-D6_failed_call_keeps_values.out"),
    );
}

/// Superseded by S30-D4: a random select index is solved.
#[test]
fn s29_d7_random_index_select() {
    run_decision(
        "S29-D7_random_index_select",
        include_str!("fixtures/sim/lrm_decisions/S29-D7_random_index_select.out"),
    );
}

#[test]
fn s29_d8_hook_order() {
    run_decision(
        "S29-D8_hook_order",
        include_str!("fixtures/sim/lrm_decisions/S29-D8_hook_order.out"),
    );
}

#[test]
fn s29_d9_null_receiver() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S29-D9_null_receiver.out");
    sim_cli::run_case_checked_matrix(SUITE, "S29-D9_null_receiver", &[], &|label, output| {
        runtime_error(
            label,
            output,
            expected,
            "null class handle access: randomize() at tb:",
        );
    });
}

#[test]
fn s29_d10_search_not_uniform() {
    run_decision(
        "S29-D10_search_not_uniform",
        include_str!("fixtures/sim/lrm_decisions/S29-D10_search_not_uniform.out"),
    );
}

#[test]
fn s30_d1_dist_density() {
    run_decision(
        "S30-D1_dist_density",
        include_str!("fixtures/sim/lrm_decisions/S30-D1_dist_density.out"),
    );
}

#[test]
fn s30_d2_solve_before_layers() {
    run_decision(
        "S30-D2_solve_before_layers",
        include_str!("fixtures/sim/lrm_decisions/S30-D2_solve_before_layers.out"),
    );
}

#[test]
fn s30_d3_randc_cycles() {
    run_decision(
        "S30-D3_randc_cycles",
        include_str!("fixtures/sim/lrm_decisions/S30-D3_randc_cycles.out"),
    );
}

#[test]
fn s30_d4_random_divisor_excluded() {
    run_decision(
        "S30-D4_random_divisor_excluded",
        include_str!("fixtures/sim/lrm_decisions/S30-D4_random_divisor_excluded.out"),
    );
}

#[test]
fn s30_d5_sizes_first() {
    run_decision(
        "S30-D5_sizes_first",
        include_str!("fixtures/sim/lrm_decisions/S30-D5_sizes_first.out"),
    );
}

#[test]
fn s30_d6_empty_reductions() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S30-D6_empty_reductions.out");
    sim_cli::run_case_checked_matrix(SUITE, "S30-D6_empty_reductions", &[], &|label, output| {
        runtime_error(
            label,
            output,
            expected,
            "and(), or() or xor() of an empty array in a constraint; randomize returns 0",
        );
    });
}

#[test]
fn s30_d7_hook_order() {
    run_decision(
        "S30-D7_hook_order",
        include_str!("fixtures/sim/lrm_decisions/S30-D7_hook_order.out"),
    );
}

#[test]
fn s30_d8_negative_weight_unsigned() {
    run_decision(
        "S30-D8_negative_weight_unsigned",
        include_str!("fixtures/sim/lrm_decisions/S30-D8_negative_weight_unsigned.out"),
    );
}

#[test]
fn s30_d11_array_index_outside() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S30-D11_array_index_outside.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S30-D11_array_index_outside",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "select index out of range in a constraint",
            );
        },
    );
}

#[test]
fn s30_d10_dist_random_weight() {
    run_decision(
        "S30-D10_dist_random_weight",
        include_str!("fixtures/sim/lrm_decisions/S30-D10_dist_random_weight.out"),
    );
}

#[test]
fn s39_d1_action_controls() {
    run_decision(
        "S39-D1_action_controls",
        include_str!("fixtures/sim/lrm_decisions/S39-D1_action_controls.out"),
    );
}

#[test]
fn s39_d2_deferred_controls_at_maturity() {
    run_decision(
        "S39-D2_deferred_controls_at_maturity",
        include_str!("fixtures/sim/lrm_decisions/S39-D2_deferred_controls_at_maturity.out"),
    );
}

#[test]
fn s39_d3_immediate_assertion_controls() {
    run_decision(
        "S39-D3_immediate_assertion_controls",
        include_str!("fixtures/sim/lrm_decisions/S39-D3_immediate_assertion_controls.out"),
    );
}

#[test]
fn s39_d4_expect_action_controls() {
    run_decision(
        "S39-D4_expect_action_controls",
        include_str!("fixtures/sim/lrm_decisions/S39-D4_expect_action_controls.out"),
    );
}

#[test]
fn s39_d5_matched_coincident_tick() {
    run_decision(
        "S39-D5_matched_coincident_tick",
        include_str!("fixtures/sim/lrm_decisions/S39-D5_matched_coincident_tick.out"),
    );
}

#[test]
fn s39_d6_endpoint_ignores_assertoff() {
    run_decision(
        "S39-D6_endpoint_ignores_assertoff",
        include_str!("fixtures/sim/lrm_decisions/S39-D6_endpoint_ignores_assertoff.out"),
    );
}

#[test]
fn s39_d7_triggered_active_read() {
    run_decision(
        "S39-D7_triggered_active_read",
        include_str!("fixtures/sim/lrm_decisions/S39-D7_triggered_active_read.out"),
    );
}

#[test]
fn s39_d8_match_call_reactive() {
    run_decision(
        "S39-D8_match_call_reactive",
        include_str!("fixtures/sim/lrm_decisions/S39-D8_match_call_reactive.out"),
    );
}

#[test]
fn s39_d9_property_local_attempt_start() {
    run_decision(
        "S39-D9_property_local_attempt_start",
        include_str!("fixtures/sim/lrm_decisions/S39-D9_property_local_attempt_start.out"),
    );
}

#[test]
fn s39_d10_match_item_scope_names() {
    run_decision(
        "S39-D10_match_item_scope_names",
        include_str!("fixtures/sim/lrm_decisions/S39-D10_match_item_scope_names.out"),
    );
}

#[test]
fn ah_d1_initial_block_assertions_single_attempt() {
    run_decision(
        "AH-D1_initial_block_assertions_single_attempt",
        include_str!(
            "fixtures/sim/lrm_decisions/AH-D1_initial_block_assertions_single_attempt.out"
        ),
    );
}

#[test]
fn f2_d1_program_concurrent_assertions() {
    run_decision(
        "F2-D1_program_concurrent_assertions",
        include_str!("fixtures/sim/lrm_decisions/F2-D1_program_concurrent_assertions.out"),
    );
}

#[test]
fn f2_d2_unit_default_timescale() {
    run_decision(
        "F2-D2_unit_default_timescale",
        include_str!("fixtures/sim/lrm_decisions/F2-D2_unit_default_timescale.out"),
    );
}

/// Build the `.c` VPI application of a decision case into a static archive
/// that the model links (`--dpi-lib`); its `vlog_startup_routines` table runs
/// at startup. Static linking works on every host, so these cases need no
/// run-time plugin loader. `prototypes` declare the system tasks/functions.
fn run_vpi_decision(case: &str, prototypes: &[&str], expected: &str) {
    run_vpi_decision_with_env(case, prototypes, expected, &[]);
}

/// [`run_vpi_decision`] with runtime environment `envs` on every run.
fn run_vpi_decision_with_env(
    case: &str,
    prototypes: &[&str],
    expected: &str,
    envs: &[(&str, &str)],
) {
    let directory = crate::sim_harness::TempDir::new("lrm-s42").expect("temporary directory");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/lrm_decisions/{case}.c"));
    let archive = crate::c_compiler::static_archive(&source, directory.path(), case, &[])
        .unwrap_or_else(|error| panic!("{case}: {error}"));
    let archive = archive.to_string_lossy().into_owned();
    let mut args = Vec::new();
    for prototype in prototypes {
        args.extend(["--define-system-task", prototype]);
    }
    args.extend(["--dpi-lib", archive.as_str()]);
    sim_cli::run_case_backend_parity(SUITE, case, expected, &args, envs);
}

#[test]
fn s42_d1_put_value_returns_null() {
    run_vpi_decision(
        "S42-D1_put_value_returns_null",
        &["task $d1()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D1_put_value_returns_null.out"),
    );
}

#[test]
fn s42_d2_chk_error_level() {
    run_vpi_decision(
        "S42-D2_chk_error_level",
        &["task $d2()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D2_chk_error_level.out"),
    );
}

#[test]
fn s42_d3_function_default_zero() {
    run_vpi_decision(
        "S42-D3_function_default_zero",
        &["function int $d3()", "task $d3_late()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D3_function_default_zero.out"),
    );
}

#[test]
fn s42_d4_sizetf_once() {
    run_vpi_decision(
        "S42-D4_sizetf_once",
        &[
            "function logic [5:0] $d4(input logic [7:0] x)",
            "task $d4_report()",
        ],
        include_str!("fixtures/sim/lrm_decisions/S42-D4_sizetf_once.out"),
    );
}

#[test]
fn s42_d5_delayed_put_active() {
    run_vpi_decision(
        "S42-D5_delayed_put_active",
        &["task $d5()", "task $d5_zero()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D5_delayed_put_active.out"),
    );
}

#[test]
fn s42_d6_decimal_string_xz() {
    run_vpi_decision(
        "S42-D6_decimal_string_xz",
        &["task $d6()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D6_decimal_string_xz.out"),
    );
}

#[test]
fn s42_d7_index_out_of_range() {
    run_vpi_decision(
        "S42-D7_index_out_of_range",
        &["task $d7()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D7_index_out_of_range.out"),
    );
}

#[test]
fn s42_d8_size_of_real_bits() {
    run_vpi_decision(
        "S42-D8_size_of_real_bits",
        &["task $d8()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D8_size_of_real_bits.out"),
    );
}

#[test]
fn s42_d9_real_value_formats() {
    run_vpi_decision(
        "S42-D9_real_value_formats",
        &["task $d9()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D9_real_value_formats.out"),
    );
}

#[test]
fn s42_d10_userdata_per_call_site() {
    run_vpi_decision(
        "S42-D10_userdata_per_call_site",
        &["task $d10()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D10_userdata_per_call_site.out"),
    );
}

#[test]
fn s42_d11_name_lookup_scope() {
    run_vpi_decision(
        "S42-D11_name_lookup_scope",
        &["task $d11()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D11_name_lookup_scope.out"),
    );
}

#[test]
fn s42_d12_iteration_order() {
    run_vpi_decision(
        "S42-D12_iteration_order",
        &["task $d12()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D12_iteration_order.out"),
    );
}

#[test]
fn s42_d13_scalar_format_vector() {
    run_vpi_decision(
        "S42-D13_scalar_format_vector",
        &["task $d13()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D13_scalar_format_vector.out"),
    );
}

// SIM-043: VPI callbacks, control and Postponed system functions.

#[test]
fn s43_d1_read_write_synch_before_nba() {
    run_vpi_decision(
        "S43-D1_read_write_synch_before_nba",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D1_read_write_synch_before_nba.out"),
    );
}

#[test]
fn s43_d2_time_callback_delay_relative() {
    run_vpi_decision(
        "S43-D2_time_callback_delay_relative",
        &["task $d2()"],
        include_str!("fixtures/sim/lrm_decisions/S43-D2_time_callback_delay_relative.out"),
    );
}

#[test]
fn s43_d3_next_sim_time_slot() {
    run_vpi_decision(
        "S43-D3_next_sim_time_slot",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D3_next_sim_time_slot.out"),
    );
}

#[test]
fn s43_d4_one_shot_callback_handle() {
    run_vpi_decision(
        "S43-D4_one_shot_callback_handle",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D4_one_shot_callback_handle.out"),
    );
}

#[test]
fn s43_d5_read_only_rejects_puts() {
    run_vpi_decision(
        "S43-D5_read_only_rejects_puts",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D5_read_only_rejects_puts.out"),
    );
}

#[test]
fn s43_d6_value_change_dispatch() {
    run_vpi_decision(
        "S43-D6_value_change_dispatch",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D6_value_change_dispatch.out"),
    );
}

#[test]
fn s43_d7_select_and_element_watch() {
    run_vpi_decision(
        "S43-D7_select_and_element_watch",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D7_select_and_element_watch.out"),
    );
}

#[test]
fn s43_d8_control_finish_after_callback() {
    run_vpi_decision(
        "S43-D8_control_finish_after_callback",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D8_control_finish_after_callback.out"),
    );
}

#[test]
fn s43_d10_control_from_start_of_simulation() {
    run_vpi_decision(
        "S43-D10_control_from_start_of_simulation",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D10_control_from_start_of_simulation.out"),
    );
}

#[test]
fn s43_d9_postponed_systf_read_only() {
    run_vpi_decision(
        "S43-D9_postponed_systf_read_only",
        &["function int $bump(input int v)"],
        include_str!("fixtures/sim/lrm_decisions/S43-D9_postponed_systf_read_only.out"),
    );
}

#[test]
fn a20_d1_mcd_file_descriptor() {
    run_vpi_decision(
        "A20-D1_mcd_file_descriptor",
        &["task $d1(input integer fd)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D1_mcd_file_descriptor.out"),
    );
}

#[test]
fn a20_d2_mcd_name_standard_streams() {
    run_vpi_decision(
        "A20-D2_mcd_name_standard_streams",
        &["task $d2(input integer fd)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D2_mcd_name_standard_streams.out"),
    );
}

#[test]
fn a20_d3_mcd_printf_channels() {
    run_vpi_decision(
        "A20-D3_mcd_printf_channels",
        &["task $d3(input integer a, input integer b)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D3_mcd_printf_channels.out"),
    );
}

#[test]
fn a20_d4_mcd_close_unopened() {
    run_vpi_decision(
        "A20-D4_mcd_close_unopened",
        &["task $d4()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D4_mcd_close_unopened.out"),
    );
}

#[test]
fn a20_d5_value_array_errors() {
    run_vpi_decision(
        "A20-D5_value_array_errors",
        &["task $d5()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D5_value_array_errors.out"),
    );
}

#[test]
fn a20_d6_value_array_raw_two_state() {
    run_vpi_decision(
        "A20-D6_value_array_raw_two_state",
        &["task $d6()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D6_value_array_raw_two_state.out"),
    );
}

#[test]
fn a20_d7_value_array_propagate_off() {
    run_vpi_decision(
        "A20-D7_value_array_propagate_off",
        &["task $d7()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D7_value_array_propagate_off.out"),
    );
}

#[test]
fn a20_d8_value_array_net_array() {
    run_vpi_decision(
        "A20-D8_value_array_net_array",
        &["task $d8()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D8_value_array_net_array.out"),
    );
}

#[test]
fn a20_d9_force_release_callbacks() {
    run_vpi_decision(
        "A20-D9_force_release_callbacks",
        &["task $d9()", "task $d9_vpi()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D9_force_release_callbacks.out"),
    );
}

#[test]
fn a20_d10_disable_callback() {
    run_vpi_decision(
        "A20-D10_disable_callback",
        &["task $d10()", "task $d10_call()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D10_disable_callback.out"),
    );
}

#[test]
fn a20_d11_control_reset_interactive() {
    run_vpi_decision(
        "A20-D11_control_reset_interactive",
        &["task $d11()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D11_control_reset_interactive.out"),
    );
}

#[test]
fn a19_d1_enum_string_value() {
    run_vpi_decision(
        "A19-D1_enum_string_value",
        &["task $d1()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D1_enum_string_value.out"),
    );
}

#[test]
fn a19_d2_typedef_alias() {
    run_vpi_decision(
        "A19-D2_typedef_alias",
        &["task $d2()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D2_typedef_alias.out"),
    );
}

#[test]
fn a19_d3_unpacked_struct_elements() {
    run_vpi_decision(
        "A19-D3_unpacked_struct_elements",
        &["task $d3()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D3_unpacked_struct_elements.out"),
    );
}

#[test]
fn a19_d4_typespec_identity() {
    run_vpi_decision(
        "A19-D4_typespec_identity",
        &["task $d4()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D4_typespec_identity.out"),
    );
}

#[test]
fn a19_d5_cont_assign_value() {
    run_vpi_decision(
        "A19-D5_cont_assign_value",
        &["task $d5()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D5_cont_assign_value.out"),
    );
}

#[test]
fn a19_d6_primitive_names() {
    run_vpi_decision(
        "A19-D6_primitive_names",
        &["task $d6()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D6_primitive_names.out"),
    );
}

#[test]
fn a19_d7_structure_order() {
    run_vpi_decision(
        "A19-D7_structure_order",
        &["task $d7()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D7_structure_order.out"),
    );
}

#[test]
fn a19_d8_named_event_callback() {
    run_vpi_decision(
        "A19-D8_named_event_callback",
        &["task $d8_watch()", "task $d8_put()", "task $d8_delay()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D8_named_event_callback.out"),
    );
}

#[test]
fn a19_d9_modpath_objects() {
    run_vpi_decision(
        "A19-D9_modpath_objects",
        &["task $d9()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D9_modpath_objects.out"),
    );
}

#[test]
fn a19_d10_tchk_objects() {
    run_vpi_decision(
        "A19-D10_tchk_objects",
        &["task $d10()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D10_tchk_objects.out"),
    );
}

#[test]
fn a19_d11_decompile_format() {
    run_vpi_decision(
        "A19-D11_decompile_format",
        &["task $d11()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D11_decompile_format.out"),
    );
}

#[test]
fn a19_d12_expression_values() {
    run_vpi_decision(
        "A19-D12_expression_values",
        &["task $d12()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D12_expression_values.out"),
    );
}

#[test]
fn a19_d13_statement_shapes() {
    run_vpi_decision(
        "A19-D13_statement_shapes",
        &["task $d13()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D13_statement_shapes.out"),
    );
}

#[test]
fn a19_d21_dynamic_values() {
    run_vpi_decision(
        "A19-D21_dynamic_values",
        &["task $d21()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D21_dynamic_values.out"),
    );
}

#[test]
fn a19_d22_object_reclaim() {
    // A collection at every safe point after an allocation reclaims each
    // object in the time step it becomes unreachable.
    run_vpi_decision_with_env(
        "A19-D22_object_reclaim",
        &["task $d22_setup()", "task $d22_hold()", "task $d22_check()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D22_object_reclaim.out"),
        &[("LLG_GC_STRESS", "1"), ("LLG_GC_VERIFY", "1")],
    );
}

#[test]
fn a19_d23_dynamic_elements() {
    run_vpi_decision(
        "A19-D23_dynamic_elements",
        &["task $d23_hold()", "task $d23_check()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D23_dynamic_elements.out"),
    );
}

#[test]
fn a19_d24_select_names() {
    run_vpi_decision(
        "A19-D24_select_names",
        &["task $d24(input logic [3:0] p, input logic q, input logic [3:0] r, input logic [7:0] m, input logic n, input logic [7:0] v)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D24_select_names.out"),
    );
}

#[test]
fn a19_d25_string_int_put() {
    run_vpi_decision(
        "A19-D25_string_int_put",
        &["task $d25()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D25_string_int_put.out"),
    );
}

#[test]
fn a19_d26_wrong_object_queries() {
    run_vpi_decision(
        "A19-D26_wrong_object_queries",
        &["task $d26()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D26_wrong_object_queries.out"),
    );
}

#[test]
fn a19_d27_statement_scopes() {
    run_vpi_decision(
        "A19-D27_statement_scopes",
        &["task $d27()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D27_statement_scopes.out"),
    );
}

#[test]
fn a19_d28_index_and_packed_relations() {
    run_vpi_decision(
        "A19-D28_index_and_packed_relations",
        &["task $d28()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D28_index_and_packed_relations.out"),
    );
}

#[test]
fn a19_d29_programs_and_packages() {
    run_vpi_decision(
        "A19-D29_programs_and_packages",
        &["task $d29()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D29_programs_and_packages.out"),
    );
}

#[test]
fn a19_d30_clocking_blocks() {
    run_vpi_decision(
        "A19-D30_clocking_blocks",
        &["task $d30()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D30_clocking_blocks.out"),
    );
}

#[test]
fn a19_d31_assertion_details() {
    run_vpi_decision(
        "A19-D31_assertion_details",
        &["task $d31()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D31_assertion_details.out"),
    );
}

#[test]
fn a19_d32_expression_relations() {
    run_vpi_decision(
        "A19-D32_expression_relations",
        &["task $d32()", "function int $d32f(int)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D32_expression_relations.out"),
    );
}

#[test]
fn a19_d36_class_definitions() {
    run_vpi_decision(
        "A19-D36_class_definitions",
        &["task $d36()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D36_class_definitions.out"),
    );
}

#[test]
fn a19_d37_constraints() {
    run_vpi_decision(
        "A19-D37_constraints",
        &["task $d37()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D37_constraints.out"),
    );
}

#[test]
fn a19_d38_udp_definitions() {
    run_vpi_decision(
        "A19-D38_udp_definitions",
        &["task $d38()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D38_udp_definitions.out"),
    );
}

#[test]
fn a19_d39_time_queue_and_callback_lists() {
    run_vpi_decision(
        "A19-D39_time_queue_and_callback_lists",
        &["task $d39(input int)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D39_time_queue_and_callback_lists.out"),
    );
}

#[test]
fn a19_d40_static_lifetime_and_allocation() {
    run_vpi_decision(
        "A19-D40_static_lifetime_and_allocation",
        &["task $d40()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D40_static_lifetime_and_allocation.out"),
    );
}

#[test]
fn a19_d41_instance_definition_properties() {
    run_vpi_decision(
        "A19-D41_instance_definition_properties",
        &["task $d41()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D41_instance_definition_properties.out"),
    );
}

#[test]
fn a19_d33_drivers_and_loads() {
    run_vpi_decision(
        "A19-D33_drivers_and_loads",
        &["task $d33(input integer pass)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D33_drivers_and_loads.out"),
    );
}

#[test]
fn a19_d34_net_fanout_and_assignment_bits() {
    run_vpi_decision(
        "A19-D34_net_fanout_and_assignment_bits",
        &["task $d34()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D34_net_fanout_and_assignment_bits.out"),
    );
}

#[test]
fn a19_d35_strengths_and_declaration_properties() {
    run_vpi_decision(
        "A19-D35_strengths_and_declaration_properties",
        &["task $d35()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D35_strengths_and_declaration_properties.out"),
    );
}

#[test]
fn a19_d42_declaration_objects() {
    run_vpi_decision(
        "A19-D42_declaration_objects",
        &["task $d42()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D42_declaration_objects.out"),
    );
}

#[test]
fn a19_d43_uses() {
    run_vpi_decision(
        "A19-D43_uses",
        &["task $d43()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D43_uses.out"),
    );
}

#[test]
fn a19_d44_frames_and_threads() {
    run_vpi_decision(
        "A19-D44_frames_and_threads",
        &["task $d44(input int)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D44_frames_and_threads.out"),
    );
}

#[test]
fn a19_d45_class_event_properties() {
    run_vpi_decision(
        "A19-D45_class_event_properties",
        &["task $d45()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D45_class_event_properties.out"),
    );
}

#[test]
fn a19_d46_statement_toggle_objects() {
    run_vpi_decision(
        "A19-D46_statement_toggle_objects",
        &["task $d46(input integer phase)"],
        include_str!("fixtures/sim/lrm_decisions/A19-D46_statement_toggle_objects.out"),
    );
}

#[test]
fn a19_d47_bits_switches_and_port_lines() {
    run_vpi_decision(
        "A19-D47_bits_switches_and_port_lines",
        &["task $d47()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D47_bits_switches_and_port_lines.out"),
    );
}

#[test]
fn a19_d48_typed_nets() {
    run_vpi_decision(
        "A19-D48_typed_nets",
        &["task $d48()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D48_typed_nets.out"),
    );
}

#[test]
fn a19_d49_scope_typedefs_and_memories() {
    run_vpi_decision(
        "A19-D49_scope_typedefs_and_memories",
        &["task $d49()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D49_scope_typedefs_and_memories.out"),
    );
}

#[test]
fn a19_d50_select_port_connections() {
    run_vpi_decision(
        "A19-D50_select_port_connections",
        &["task $d50()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D50_select_port_connections.out"),
    );
}

#[test]
fn a19_d51_package_definition_properties() {
    run_vpi_decision(
        "A19-D51_package_definition_properties",
        &["task $d51()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D51_package_definition_properties.out"),
    );
}

#[test]
fn a19_d52_dpi_and_return_properties() {
    run_vpi_decision(
        "A19-D52_dpi_and_return_properties",
        &["task $d52()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D52_dpi_and_return_properties.out"),
    );
}

#[test]
fn a19_d53_function_return_variables() {
    run_vpi_decision(
        "A19-D53_function_return_variables",
        &["task $d53()"],
        include_str!("fixtures/sim/lrm_decisions/A19-D53_function_return_variables.out"),
    );
}

#[test]
fn a20_d12_delay_counts() {
    run_vpi_decision(
        "A20-D12_delay_counts",
        &["task $d12(input integer stage)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D12_delay_counts.out"),
    );
}

#[test]
fn a20_d13_delay_flags() {
    run_vpi_decision(
        "A20-D13_delay_flags",
        &["task $d13(input integer stage)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D13_delay_flags.out"),
    );
}

#[test]
fn a20_d14_delay_objects() {
    run_vpi_decision(
        "A20-D14_delay_objects",
        &["task $d14(input integer stage)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D14_delay_objects.out"),
    );
}

#[test]
fn a20_d15_tchk_violation_callback() {
    run_vpi_decision(
        "A20-D15_tchk_violation_callback",
        &["task $d15()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D15_tchk_violation_callback.out"),
    );
}

#[test]
fn a20_d16_delay_expressions() {
    run_vpi_decision(
        "A20-D16_delay_expressions",
        &["task $d16()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D16_delay_expressions.out"),
    );
}

#[test]
fn a20_d17_port_delays() {
    run_vpi_decision(
        "A20-D17_port_delays",
        &["task $d17(input integer stage)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D17_port_delays.out"),
    );
}

#[test]
fn a20_d18_intermodule_paths() {
    run_vpi_decision(
        "A20-D18_intermodule_paths",
        &["task $d18()"],
        include_str!("fixtures/sim/lrm_decisions/A20-D18_intermodule_paths.out"),
    );
}

#[test]
fn a20_d19_runtime_delays() {
    run_vpi_decision(
        "A20-D19_runtime_delays",
        &["task $d19(input integer stage)"],
        include_str!("fixtures/sim/lrm_decisions/A20-D19_runtime_delays.out"),
    );
}

#[test]
fn a21_d1_input_port_shares_actual_variable() {
    run_decision(
        "A21-D1_input_port_shares_actual_variable",
        include_str!("fixtures/sim/lrm_decisions/A21-D1_input_port_shares_actual_variable.out"),
    );
}

#[test]
fn a17_d1_attempt_callbacks() {
    run_vpi_decision(
        "A17-D1_attempt_callbacks",
        &["task $d1()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D1_attempt_callbacks.out"),
    );
}

#[test]
fn a17_d4_assertion_relations() {
    run_vpi_decision(
        "A17-D4_assertion_relations",
        &["task $d4()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D4_assertion_relations.out"),
    );
}

#[test]
fn a17_d5_instance_callbacks() {
    run_vpi_decision(
        "A17-D5_instance_callbacks",
        &["task $d5()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D5_instance_callbacks.out"),
    );
}

#[test]
fn a17_d6_assertion_collections() {
    run_vpi_decision(
        "A17-D6_assertion_collections",
        &["task $d6(input integer n)"],
        include_str!("fixtures/sim/lrm_decisions/A17-D6_assertion_collections.out"),
    );
}

#[test]
fn a17_d7_future_disabled_start() {
    run_vpi_decision(
        "A17-D7_future_disabled_start",
        &["task $d7()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D7_future_disabled_start.out"),
    );
}

#[test]
fn a17_d8_reset_past_sampling() {
    run_vpi_decision(
        "A17-D8_reset_past_sampling",
        &["task $d8()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D8_reset_past_sampling.out"),
    );
}

#[test]
fn a17_d9_tick_order() {
    run_vpi_decision(
        "A17-D9_tick_order",
        &["task $d9()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D9_tick_order.out"),
    );
}

#[test]
fn a17_d10_system_start() {
    run_vpi_decision(
        "A17-D10_system_start",
        &[
            "task $d10_initial()",
            "task $d10_woken()",
            "task $d10_zero()",
            "task $d10_late()",
        ],
        include_str!("fixtures/sim/lrm_decisions/A17-D10_system_start.out"),
    );
}

#[test]
fn a17_d11_fsm_pragmas() {
    run_decision(
        "A17-D11_fsm_pragmas",
        include_str!("fixtures/sim/lrm_decisions/A17-D11_fsm_pragmas.out"),
    );
}

#[test]
fn a17_d12_fsm_state_visits() {
    run_decision(
        "A17-D12_fsm_state_visits",
        include_str!("fixtures/sim/lrm_decisions/A17-D12_fsm_state_visits.out"),
    );
}

#[test]
fn a17_d13_fsm_objects() {
    run_vpi_decision(
        "A17-D13_fsm_objects",
        &["task $d13()"],
        include_str!("fixtures/sim/lrm_decisions/A17-D13_fsm_objects.out"),
    );
}

#[test]
fn a17_d2_control_callbacks() {
    run_vpi_decision(
        "A17-D2_control_callbacks",
        &["task $d2()", "task $d2_step(input integer n)"],
        include_str!("fixtures/sim/lrm_decisions/A17-D2_control_callbacks.out"),
    );
}

#[test]
fn a17_d3_system_control() {
    run_vpi_decision(
        "A17-D3_system_control",
        &["task $d3()", "task $d3_step(input integer n)"],
        include_str!("fixtures/sim/lrm_decisions/A17-D3_system_control.out"),
    );
}

#[test]
fn s44_d1_covergroup_type_per_declaration() {
    run_decision(
        "S44-D1_covergroup_type_per_declaration",
        include_str!("fixtures/sim/lrm_decisions/S44-D1_covergroup_type_per_declaration.out"),
    );
}

#[test]
fn s44_d2_signed_auto_bin_order() {
    run_decision(
        "S44-D2_signed_auto_bin_order",
        include_str!("fixtures/sim/lrm_decisions/S44-D2_signed_auto_bin_order.out"),
    );
}

#[test]
fn s44_d3_dropped_instance_keeps_counting() {
    run_decision(
        "S44-D3_dropped_instance_keeps_counting",
        include_str!("fixtures/sim/lrm_decisions/S44-D3_dropped_instance_keeps_counting.out"),
    );
}

#[test]
fn s44_d4_type_without_instances() {
    run_decision(
        "S44-D4_type_without_instances",
        include_str!("fixtures/sim/lrm_decisions/S44-D4_type_without_instances.out"),
    );
}

#[test]
fn s47_d1_dumpvars_end_of_step() {
    run_decision(
        "S47-D1_dumpvars_end_of_step",
        include_str!("fixtures/sim/lrm_decisions/S47-D1_dumpvars_end_of_step.out"),
    );
}

#[test]
fn s47_d2_one_record_per_step() {
    run_decision(
        "S47-D2_one_record_per_step",
        include_str!("fixtures/sim/lrm_decisions/S47-D2_one_record_per_step.out"),
    );
}

#[test]
fn s47_d3_dumpvars_calls_combine() {
    run_decision(
        "S47-D3_dumpvars_calls_combine",
        include_str!("fixtures/sim/lrm_decisions/S47-D3_dumpvars_calls_combine.out"),
    );
}

#[test]
fn s47_d4_dumpvars_levels_without_scope() {
    run_decision(
        "S47-D4_dumpvars_levels_without_scope",
        include_str!("fixtures/sim/lrm_decisions/S47-D4_dumpvars_levels_without_scope.out"),
    );
}

#[test]
fn s47_d5_dump_controls_by_state() {
    run_decision(
        "S47-D5_dump_controls_by_state",
        include_str!("fixtures/sim/lrm_decisions/S47-D5_dump_controls_by_state.out"),
    );
}

#[test]
fn s47_d6_dumpoff_real_nan() {
    run_decision(
        "S47-D6_dumpoff_real_nan",
        include_str!("fixtures/sim/lrm_decisions/S47-D6_dumpoff_real_nan.out"),
    );
}

#[test]
fn s47_d7_dumpfile_name() {
    run_decision(
        "S47-D7_dumpfile_name",
        include_str!("fixtures/sim/lrm_decisions/S47-D7_dumpfile_name.out"),
    );
}

#[test]
fn s47_d8_dumplimit_header_whole() {
    run_decision(
        "S47-D8_dumplimit_header_whole",
        include_str!("fixtures/sim/lrm_decisions/S47-D8_dumplimit_header_whole.out"),
    );
}

#[test]
fn s47_d9_dynamic_objects_not_dumped() {
    run_decision(
        "S47-D9_dynamic_objects_not_dumped",
        include_str!("fixtures/sim/lrm_decisions/S47-D9_dynamic_objects_not_dumped.out"),
    );
}

#[test]
fn s47_d10_loop_generate_scope_names() {
    run_decision(
        "S47-D10_loop_generate_scope_names",
        include_str!("fixtures/sim/lrm_decisions/S47-D10_loop_generate_scope_names.out"),
    );
}

#[test]
fn s47_d12_value_formats() {
    run_decision(
        "S47-D12_value_formats",
        include_str!("fixtures/sim/lrm_decisions/S47-D12_value_formats.out"),
    );
}

#[test]
fn s47_d13_catalog_encoding() {
    run_decision(
        "S47-D13_catalog_encoding",
        include_str!("fixtures/sim/lrm_decisions/S47-D13_catalog_encoding.out"),
    );
}

#[test]
fn s47_d9_named_string_rejected() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/S47-D9_named_string_rejected.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "S47-D9_named_string_rejected",
        "`$dumpvars` argument `tb.label` in `tb` has no VCD/FST representation",
    );
}

#[test]
fn s47_d11_dump_open_error() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S47-D11_dump_open_error.out");
    sim_cli::run_case_checked_matrix(SUITE, "S47-D11_dump_open_error", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        let reports = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("llg: waveform: "))
            .count();
        assert_eq!(
            reports,
            1,
            "{label}: one output-error report: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("llg: waveform: cannot open `S47-D11-missing/dump.vcd`: "),
            "{label}"
        );
    });
}

#[test]
fn s44_d5_instance_event_waiter() {
    run_decision(
        "S44-D5_instance_event_waiter",
        include_str!("fixtures/sim/lrm_decisions/S44-D5_instance_event_waiter.out"),
    );
}

#[test]
fn s44_d6_waiter_starts_after_new() {
    run_decision(
        "S44-D6_waiter_starts_after_new",
        include_str!("fixtures/sim/lrm_decisions/S44-D6_waiter_starts_after_new.out"),
    );
}

#[test]
fn s44_d7_end_event_after_return() {
    run_decision(
        "S44-D7_end_event_after_return",
        include_str!("fixtures/sim/lrm_decisions/S44-D7_end_event_after_return.out"),
    );
}

#[test]
fn s44_d8_named_block_jump_out() {
    run_decision(
        "S44-D8_named_block_jump_out",
        include_str!("fixtures/sim/lrm_decisions/S44-D8_named_block_jump_out.out"),
    );
}

#[test]
fn s44_d9_strobe_block_event() {
    run_decision(
        "S44-D9_strobe_block_event",
        include_str!("fixtures/sim/lrm_decisions/S44-D9_strobe_block_event.out"),
    );
}

#[test]
fn s45_d1_fixed_bins_more_than_values() {
    run_decision(
        "S45-D1_fixed_bins_more_than_values",
        include_str!("fixtures/sim/lrm_decisions/S45-D1_fixed_bins_more_than_values.out"),
    );
}

#[test]
fn s45_d2_default_only_coverpoint() {
    run_decision(
        "S45-D2_default_only_coverpoint",
        include_str!("fixtures/sim/lrm_decisions/S45-D2_default_only_coverpoint.out"),
    );
}

/// The illegal sample ends the run; the final procedures run.
#[test]
fn s45_d3_illegal_bin_ends_run() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S45-D3_illegal_bin_ends_run",
        &[],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                include_str!("fixtures/sim/lrm_decisions/S45-D3_illegal_bin_ends_run.out"),
                "{label}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("llg: runtime error: covergroup `g` coverpoint `cp` sampled 7"),
                "{label}: {output:?}"
            );
        },
    );
}

#[test]
fn s45_d4_wildcard_range_bounds() {
    run_decision(
        "S45-D4_wildcard_range_bounds",
        include_str!("fixtures/sim/lrm_decisions/S45-D4_wildcard_range_bounds.out"),
    );
}

/// A non-positive bins array size from an argument ends the run at the
/// construction; the final procedures run.
#[test]
fn s45_d5_bins_count_not_positive() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S45-D5_bins_count_not_positive",
        &[],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                include_str!("fixtures/sim/lrm_decisions/S45-D5_bins_count_not_positive.out"),
                "{label}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(
                    "llg: runtime error: bins `b` at tb:19:12: the number of bins is not a positive value"
                ),
                "{label}: {output:?}"
            );
        },
    );
}

#[test]
fn s45_d6_value_listed_twice() {
    run_decision(
        "S45-D6_value_listed_twice",
        include_str!("fixtures/sim/lrm_decisions/S45-D6_value_listed_twice.out"),
    );
}

#[test]
fn s45_d7_unknown_values() {
    run_decision(
        "S45-D7_unknown_values",
        include_str!("fixtures/sim/lrm_decisions/S45-D7_unknown_values.out"),
    );
}

#[test]
fn s45_d8_nonconsecutive_gap() {
    run_decision(
        "S45-D8_nonconsecutive_gap",
        include_str!("fixtures/sim/lrm_decisions/S45-D8_nonconsecutive_gap.out"),
    );
}

#[test]
fn s45_d9_skipped_sample_history() {
    run_decision(
        "S45-D9_skipped_sample_history",
        include_str!("fixtures/sim/lrm_decisions/S45-D9_skipped_sample_history.out"),
    );
}

/// Each default-sequence count is an illegal-bin run-time error.
#[test]
fn s45_d10_default_sequence() {
    sim_cli::run_case_checked_matrix(SUITE, "S45-D10_default_sequence", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("fixtures/sim/lrm_decisions/S45-D10_default_sequence.out"),
            "{label}"
        );
        let errors: Vec<String> = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("llg: runtime error:"))
            .map(|line| {
                let sampled = line.split(" sampled ").nth(1).unwrap_or_default();
                let point = line.split(" coverpoint `").nth(1).unwrap_or_default();
                format!(
                    "{} {}",
                    point.split('`').next().unwrap_or_default(),
                    sampled.split(',').next().unwrap_or_default()
                )
            })
            .collect();
        assert_eq!(
            errors,
            ["v_a 7", "v_a 8", "q 9", "q 1", "q 9"],
            "{label}: {output:?}"
        );
    });
}

#[test]
fn s45_d11_length_zero_repetition() {
    sim_cli::reject_case(
        SUITE,
        "S45-D11_length_zero_repetition",
        "S45-D11_length_zero_repetition.sv:16:29: transition bins `t` of coverpoint `cp` in covergroup `g` specify a transition of length 0",
    );
}

#[test]
fn s45_d19_single_item_repetitions() {
    run_decision(
        "S45-D19_single_item_repetitions",
        include_str!("fixtures/sim/lrm_decisions/S45-D19_single_item_repetitions.out"),
    );
}

#[test]
fn s45_d12_cross_empty_bins() {
    run_decision(
        "S45-D12_cross_empty_bins",
        include_str!("fixtures/sim/lrm_decisions/S45-D12_cross_empty_bins.out"),
    );
}

#[test]
fn s45_d13_binsof_outside_cross() {
    sim_cli::reject_case(
        SUITE,
        "S45-D13_binsof_outside_cross",
        "S45-D13_binsof_outside_cross.sv:17:35: binsof in bins `sel` of cross `ab` in covergroup `g` names coverpoint `cd`, which the cross does not cross",
    );
}

#[test]
fn s45_d14_cross_needs_every_point() {
    run_decision(
        "S45-D14_cross_needs_every_point",
        include_str!("fixtures/sim/lrm_decisions/S45-D14_cross_needs_every_point.out"),
    );
}

#[test]
fn s45_d15_overlapping_cross_products() {
    run_decision(
        "S45-D15_overlapping_cross_products",
        include_str!("fixtures/sim/lrm_decisions/S45-D15_overlapping_cross_products.out"),
    );
}

#[test]
fn s45_d16_negated_bin_condition() {
    run_decision(
        "S45-D16_negated_bin_condition",
        include_str!("fixtures/sim/lrm_decisions/S45-D16_negated_bin_condition.out"),
    );
}

#[test]
fn s45_d17_detect_overlap() {
    sim_cli::run_case_checked_matrix(SUITE, "S45-D17_detect_overlap", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("fixtures/sim/lrm_decisions/S45-D17_detect_overlap.out"),
            "{label}"
        );
        let warnings: Vec<_> = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("llg: warning: covergroup"))
            .map(str::to_owned)
            .collect();
        assert_eq!(
            warnings,
            [
                "llg: warning: covergroup `g` coverpoint `c1`: bins `lo` and `mid` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
                "llg: warning: covergroup `g` coverpoint `c2`: bins `r` and `r2` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
                "llg: warning: covergroup `g` coverpoint `c3`: bins `v` and `w` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
            ],
            "{label}"
        );
    });
}

#[test]
fn s45_d18_merged_at_least() {
    run_decision(
        "S45-D18_merged_at_least",
        include_str!("fixtures/sim/lrm_decisions/S45-D18_merged_at_least.out"),
    );
}

#[test]
fn s44_d10_named_block_return() {
    run_decision(
        "S44-D10_named_block_return",
        include_str!("fixtures/sim/lrm_decisions/S44-D10_named_block_return.out"),
    );
}

#[test]
fn s46_d1_default_instance_names() {
    run_decision(
        "S46-D1_default_instance_names",
        include_str!("fixtures/sim/lrm_decisions/S46-D1_default_instance_names.out"),
    );
}

#[test]
fn s46_d2_loaded_runs_join_instances() {
    // Two runs in one directory: the first saves, the second (+load) loads.
    let expected = include_str!("fixtures/sim/lrm_decisions/S46-D2_loaded_runs_join_instances.out");
    for optimized in [false, true] {
        for (backend, controls) in sim_cli::backend_matrix() {
            let envs = controls
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect::<Vec<_>>();
            let (_directory, outputs) = sim_cli::run_case_sequence(
                SUITE,
                "S46-D2_loaded_runs_join_instances",
                optimized,
                &envs,
                &[],
                &[(&[], &[]), (&[], &["+load"])],
            );
            let label = format!("S46-D2, {backend}, optimized={optimized}");
            let mut stdout = String::new();
            for output in &outputs {
                assert!(
                    output.status.success(),
                    "{label}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                stdout.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            assert_eq!(stdout, expected, "{label}");
        }
    }
}

#[test]
fn s46_d3_failed_load_changes_nothing() {
    let expected =
        include_str!("fixtures/sim/lrm_decisions/S46-D3_failed_load_changes_nothing.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S46-D3_failed_load_changes_nothing",
        &[],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(
                    "llg: runtime error: $load_coverage_db at tb:21:5: coverage database `s46_d3_no_such_coverage_database` was not loaded: "
                ),
                "{label}: {stderr}"
            );
        },
    );
}

#[test]
fn s46_d4_assertion_covered_means_success() {
    run_decision(
        "S46-D4_assertion_covered_means_success",
        include_str!("fixtures/sim/lrm_decisions/S46-D4_assertion_covered_means_success.out"),
    );
}

#[test]
fn s46_d5_unavailable_kinds() {
    run_decision(
        "S46-D5_unavailable_kinds",
        include_str!("fixtures/sim/lrm_decisions/S46-D5_unavailable_kinds.out"),
    );
}

#[test]
fn s46_d6_string_scope_is_definition() {
    run_decision(
        "S46-D6_string_scope_is_definition",
        include_str!("fixtures/sim/lrm_decisions/S46-D6_string_scope_is_definition.out"),
    );
}

#[test]
fn s46_d8_assertion_attempts() {
    run_decision(
        "S46-D8_assertion_attempts",
        include_str!("fixtures/sim/lrm_decisions/S46-D8_assertion_attempts.out"),
    );
}

#[test]
fn s46_d9_assertion_items() {
    run_decision(
        "S46-D9_assertion_items",
        include_str!("fixtures/sim/lrm_decisions/S46-D9_assertion_items.out"),
    );
}

#[test]
fn s46_d10_vpi_coverage_control() {
    run_vpi_decision(
        "S46-D10_vpi_coverage_control",
        &["task $d10(input int phase)"],
        include_str!("fixtures/sim/lrm_decisions/S46-D10_vpi_coverage_control.out"),
    );
}

#[test]
fn s46_d11_failed_merge() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S46-D11_failed_merge.out");
    sim_cli::run_case_checked_matrix(SUITE, "S46-D11_failed_merge", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(
                "llg: runtime error: $coverage_merge at tb:18:27: coverage database `s46_d11_no_such_database` was not merged: "
            ),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn s46_d12_check_full_or_none() {
    run_decision(
        "S46-D12_check_full_or_none",
        include_str!("fixtures/sim/lrm_decisions/S46-D12_check_full_or_none.out"),
    );
}

#[test]
fn s46_d13_statement_coverage() {
    run_decision(
        "S46-D13_statement_coverage",
        include_str!("fixtures/sim/lrm_decisions/S46-D13_statement_coverage.out"),
    );
}

#[test]
fn s46_d14_toggle_coverage() {
    run_decision(
        "S46-D14_toggle_coverage",
        include_str!("fixtures/sim/lrm_decisions/S46-D14_toggle_coverage.out"),
    );
}

#[test]
fn s46_d7_merged_bin_counts() {
    run_decision(
        "S46-D7_merged_bin_counts",
        include_str!("fixtures/sim/lrm_decisions/S46-D7_merged_bin_counts.out"),
    );
}

#[test]
fn s16_d1_single_rule_weight() {
    run_decision(
        "S16-D1_single_rule_weight",
        include_str!("fixtures/sim/lrm_decisions/S16-D1_single_rule_weight.out"),
    );
}

#[test]
fn s16_d2_weight_code_block_order() {
    run_decision(
        "S16-D2_weight_code_block_order",
        include_str!("fixtures/sim/lrm_decisions/S16-D2_weight_code_block_order.out"),
    );
}

#[test]
fn s16_d3_invalid_weight_error() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S16-D3_invalid_weight_error.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S16-D3_invalid_weight_error",
        &[],
        &|label, output| {
            runtime_error(label, output, expected, "the weight of rule 1 is negative");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("unknown bits"), "{label}: {output:?}");
            assert!(
                stderr.contains("all rule weights are zero; nothing is generated"),
                "{label}: {output:?}"
            );
        },
    );
}

#[test]
fn s16_d4_invalid_repeat_count() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S16-D4_invalid_repeat_count.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S16-D4_invalid_repeat_count",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "randsequence repeat count is negative or unknown",
            );
        },
    );
}

#[test]
fn s16_d5_rand_join_steps() {
    run_decision(
        "S16-D5_rand_join_steps",
        include_str!("fixtures/sim/lrm_decisions/S16-D5_rand_join_steps.out"),
    );
}

#[test]
fn s16_d10_rand_join_rule_forms_rejected() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/S16-D10_rand_join_rule_forms_rejected.out")
            .is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "S16-D10_rand_join_rule_forms_rejected",
        "S16-D10_rand_join_rule_forms_rejected.sv:20:23: production `s1` has more than one rule",
    );
}

#[test]
fn s16_d6_rand_join_ratio() {
    run_decision(
        "S16-D6_rand_join_ratio",
        include_str!("fixtures/sim/lrm_decisions/S16-D6_rand_join_ratio.out"),
    );
}

#[test]
fn s16_d7_rand_join_ratio_range() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S16-D7_rand_join_ratio_range.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S16-D7_rand_join_ratio_range",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "rand join expression is not a real number in 0.0 to 1.0; 0.5 is used",
            );
        },
    );
}

#[test]
fn s16_d8_default_values() {
    run_decision(
        "S16-D8_default_values",
        include_str!("fixtures/sim/lrm_decisions/S16-D8_default_values.out"),
    );
}

#[test]
fn s2_d1_mintypmax_typical() {
    run_decision(
        "S2-D1_mintypmax_typical",
        include_str!("fixtures/sim/lrm_decisions/S2-D1_mintypmax_typical.out"),
    );
}

#[test]
fn s2_d2_vector_delay_rule() {
    run_decision(
        "S2-D2_vector_delay_rule",
        include_str!("fixtures/sim/lrm_decisions/S2-D2_vector_delay_rule.out"),
    );
}

#[test]
fn s2_d3_net_delay_vector() {
    run_decision(
        "S2-D3_net_delay_vector",
        include_str!("fixtures/sim/lrm_decisions/S2-D3_net_delay_vector.out"),
    );
}

#[test]
fn s2_d4_delayed_driver_initial_x() {
    run_decision(
        "S2-D4_delayed_driver_initial_x",
        include_str!("fixtures/sim/lrm_decisions/S2-D4_delayed_driver_initial_x.out"),
    );
}

#[test]
fn s2_d5_declaration_assignment_delay() {
    run_decision(
        "S2-D5_declaration_assignment_delay",
        include_str!("fixtures/sim/lrm_decisions/S2-D5_declaration_assignment_delay.out"),
    );
}

#[test]
fn s2_d6_concatenation_one_vector() {
    run_decision(
        "S2-D6_concatenation_one_vector",
        include_str!("fixtures/sim/lrm_decisions/S2-D6_concatenation_one_vector.out"),
    );
}

#[test]
fn s2_d7_pattern_leaf_delays() {
    run_decision(
        "S2-D7_pattern_leaf_delays",
        include_str!("fixtures/sim/lrm_decisions/S2-D7_pattern_leaf_delays.out"),
    );
}

#[test]
fn s2_d9_runtime_delay_evaluation() {
    run_decision(
        "S2-D9_runtime_delay_evaluation",
        include_str!("fixtures/sim/lrm_decisions/S2-D9_runtime_delay_evaluation.out"),
    );
}

#[test]
fn s2_d8_alias_first_member_delay() {
    run_decision(
        "S2-D8_alias_first_member_delay",
        include_str!("fixtures/sim/lrm_decisions/S2-D8_alias_first_member_delay.out"),
    );
}

#[test]
fn s2_d10_delayed_net_initial_x() {
    run_decision(
        "S2-D10_delayed_net_initial_x",
        include_str!("fixtures/sim/lrm_decisions/S2-D10_delayed_net_initial_x.out"),
    );
}

#[test]
fn s2_d11_driver_handover_one_update() {
    run_decision(
        "S2-D11_driver_handover_one_update",
        include_str!("fixtures/sim/lrm_decisions/S2-D11_driver_handover_one_update.out"),
    );
}

#[test]
fn s3_d1_level_rows_use_prior_state() {
    run_decision(
        "S3-D1_level_rows_use_prior_state",
        include_str!("fixtures/sim/lrm_decisions/S3-D1_level_rows_use_prior_state.out"),
    );
}

#[test]
fn s3_d2_simultaneous_inputs_in_port_order() {
    run_decision(
        "S3-D2_simultaneous_inputs_in_port_order",
        include_str!("fixtures/sim/lrm_decisions/S3-D2_simultaneous_inputs_in_port_order.out"),
    );
}

#[test]
fn s3_d3_z_x_change_is_no_transition() {
    run_decision(
        "S3-D3_z_x_change_is_no_transition",
        include_str!("fixtures/sim/lrm_decisions/S3-D3_z_x_change_is_no_transition.out"),
    );
}

#[test]
fn s3_d4_time_zero_inputs_start_x() {
    run_decision(
        "S3-D4_time_zero_inputs_start_x",
        include_str!("fixtures/sim/lrm_decisions/S3-D4_time_zero_inputs_start_x.out"),
    );
}

#[test]
fn s99_d1_procedural_queue() {
    run_decision(
        "S99-D1_procedural_queue",
        include_str!("fixtures/sim/lrm_decisions/S99-D1_procedural_queue.out"),
    );
}

#[test]
fn s99_d2_comb_sensitivity() {
    run_decision(
        "S99-D2_comb_sensitivity",
        include_str!("fixtures/sim/lrm_decisions/S99-D2_comb_sensitivity.out"),
    );
}

#[test]
fn s99_d3_assertion_controls() {
    run_decision(
        "S99-D3_assertion_controls",
        include_str!("fixtures/sim/lrm_decisions/S99-D3_assertion_controls.out"),
    );
}

#[test]
fn s99_d4_late_maturation() {
    run_decision(
        "S99-D4_late_maturation",
        include_str!("fixtures/sim/lrm_decisions/S99-D4_late_maturation.out"),
    );
}

#[test]
fn s99_d5_inferred_clock() {
    run_decision(
        "S99-D5_inferred_clock",
        include_str!("fixtures/sim/lrm_decisions/S99-D5_inferred_clock.out"),
    );
}

#[test]
fn s99_d7_vif_implicit_sensitivity() {
    run_decision(
        "S99-D7_vif_implicit_sensitivity",
        include_str!("fixtures/sim/lrm_decisions/S99-D7_vif_implicit_sensitivity.out"),
    );
}

#[test]
fn s01_d1_delay_tick_counts() {
    run_decision(
        "S01-D1_delay_tick_counts",
        include_str!("fixtures/sim/lrm_decisions/S01-D1_delay_tick_counts.out"),
    );
}

#[test]
fn s01_d2_end_of_time() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S01-D2_end_of_time.out");
    sim_cli::run_case_checked_matrix(SUITE, "S01-D2_end_of_time", &[], &|label, output| {
        runtime_error(
            label,
            output,
            expected,
            "simulation time overflow at 18446744073709551615 while scheduling a delay of 1 tick(s)",
        );
    });
}

#[test]
fn s11_d1_task_cast_class_failure() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S11-D1_task_cast_class_failure.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S11-D1_task_cast_class_failure",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "$cast failed to assign an incompatible value at tb:36:5",
            );
        },
    );
}

#[test]
fn s11_d2_task_cast_enum_failure() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S11-D2_task_cast_enum_failure.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S11-D2_task_cast_enum_failure",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "$cast failed to assign an incompatible value at tb:22:5",
            );
        },
    );
}

#[test]
fn s99_d8_null_access_stops() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S99-D8_null_access_stops.out");
    sim_cli::run_case_checked_matrix(SUITE, "S99-D8_null_access_stops", &[], &|label, output| {
        runtime_error(
            label,
            output,
            expected,
            "virtual interface access failed: virtual interface member access at tb:28:45",
        );
    });
}

#[test]
fn s99_d11_null_class_access_runs_finals() {
    let expected =
        include_str!("fixtures/sim/lrm_decisions/S99-D11_null_class_access_runs_finals.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S99-D11_null_class_access_runs_finals",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "null class handle access: property access at tb:22:",
            );
        },
    );
}

#[test]
fn s99_d9_implicit_finish_reactive() {
    run_decision(
        "S99-D9_implicit_finish_reactive",
        include_str!("fixtures/sim/lrm_decisions/S99-D9_implicit_finish_reactive.out"),
    );
}

#[test]
fn s99_d10_class_method_scope() {
    run_decision(
        "S99-D10_class_method_scope",
        include_str!("fixtures/sim/lrm_decisions/S99-D10_class_method_scope.out"),
    );
}

#[test]
fn s99_d6_builtin_override() {
    // Negative case: llg stops at startup, so the expected stdout is empty.
    assert!(include_str!("fixtures/sim/lrm_decisions/S99-D6_builtin_override.out").is_empty());
    let case = "S99-D6_builtin_override";
    let directory = crate::sim_harness::TempDir::new("lrm-s99").expect("temporary directory");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/lrm_decisions/{case}.c"));
    let archive = crate::c_compiler::static_archive(&source, directory.path(), case, &[])
        .unwrap_or_else(|error| panic!("{case}: {error}"));
    let archive = archive.to_string_lossy().into_owned();
    sim_cli::reject_case_with_args(
        SUITE,
        case,
        "a VPI application registers `$random`, which llg implements as a built-in",
        &["--dpi-lib", &archive],
    );
}

#[test]
fn cd_d1_class_block_disable() {
    run_decision(
        "CD-D1_class_block_disable",
        include_str!("fixtures/sim/lrm_decisions/CD-D1_class_block_disable.out"),
    );
}

#[test]
fn cd_d2_named_block_scope() {
    run_decision(
        "CD-D2_named_block_scope",
        include_str!("fixtures/sim/lrm_decisions/CD-D2_named_block_scope.out"),
    );
}

#[test]
fn s26_d16_strings_form_one_image() {
    run_decision(
        "S26-D16_strings_form_one_image",
        include_str!("fixtures/sim/lrm_decisions/S26-D16_strings_form_one_image.out"),
    );
}

#[test]
fn s26_d17_string_variable_data() {
    run_decision(
        "S26-D17_string_variable_data",
        include_str!("fixtures/sim/lrm_decisions/S26-D17_string_variable_data.out"),
    );
}

#[test]
fn s26_d18_start_after_finish_descends() {
    run_decision(
        "S26-D18_start_after_finish_descends",
        include_str!("fixtures/sim/lrm_decisions/S26-D18_start_after_finish_descends.out"),
    );
}

#[test]
fn s26_d19_errors_end_the_load() {
    run_decision(
        "S26-D19_errors_end_the_load",
        include_str!("fixtures/sim/lrm_decisions/S26-D19_errors_end_the_load.out"),
    );
}

#[test]
fn s27_d26_unknown_address_bits() {
    run_decision(
        "S27-D26_unknown_address_bits",
        include_str!("fixtures/sim/lrm_decisions/S27-D26_unknown_address_bits.out"),
    );
}

#[test]
fn s26_d20_comments_across_strings() {
    run_decision(
        "S26-D20_comments_across_strings",
        include_str!("fixtures/sim/lrm_decisions/S26-D20_comments_across_strings.out"),
    );
}

#[test]
fn a7_d1_path_delay_selection() {
    run_decision(
        "A7-D1_path_delay_selection",
        include_str!("fixtures/sim/lrm_decisions/A7-D1_path_delay_selection.out"),
    );
}

#[test]
fn a7_d2_path_delays_destination_net() {
    run_decision(
        "A7-D2_path_delays_destination_net",
        include_str!("fixtures/sim/lrm_decisions/A7-D2_path_delays_destination_net.out"),
    );
}

#[test]
fn a7_d3_unchanged_input_time_zero() {
    run_decision(
        "A7-D3_unchanged_input_time_zero",
        include_str!("fixtures/sim/lrm_decisions/A7-D3_unchanged_input_time_zero.out"),
    );
}

#[test]
fn a7_d4_duplicate_paths_smallest_delay() {
    run_decision(
        "A7-D4_duplicate_paths_smallest_delay",
        include_str!("fixtures/sim/lrm_decisions/A7-D4_duplicate_paths_smallest_delay.out"),
    );
}

#[test]
fn a7_d5_parameter_in_specify_rejected() {
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(
        include_str!("fixtures/sim/lrm_decisions/A7-D5_parameter_in_specify_rejected.out")
            .is_empty()
    );
    sim_cli::reject_case(
        SUITE,
        "A7-D5_parameter_in_specify_rejected",
        "parameters cannot be used in specify blocks",
    );
}

#[test]
fn a7_d6_path_destination_initial_x() {
    run_decision(
        "A7-D6_path_destination_initial_x",
        include_str!("fixtures/sim/lrm_decisions/A7-D6_path_destination_initial_x.out"),
    );
}

#[test]
fn a8_d1_conditional_path_selection() {
    run_decision(
        "A8-D1_conditional_path_selection",
        include_str!("fixtures/sim/lrm_decisions/A8-D1_conditional_path_selection.out"),
    );
}

#[test]
fn a8_d2_multibit_condition_lsb() {
    run_decision(
        "A8-D2_multibit_condition_lsb",
        include_str!("fixtures/sim/lrm_decisions/A8-D2_multibit_condition_lsb.out"),
    );
}

#[test]
fn a8_d3_ifnone_default_path() {
    run_decision(
        "A8-D3_ifnone_default_path",
        include_str!("fixtures/sim/lrm_decisions/A8-D3_ifnone_default_path.out"),
    );
}

#[test]
fn a8_d4_edge_path_last_edge() {
    run_decision(
        "A8-D4_edge_path_last_edge",
        include_str!("fixtures/sim/lrm_decisions/A8-D4_edge_path_last_edge.out"),
    );
}

#[test]
fn a8_d5_pathpulse_limits() {
    run_decision(
        "A8-D5_pathpulse_limits",
        include_str!("fixtures/sim/lrm_decisions/A8-D5_pathpulse_limits.out"),
    );
}

#[test]
fn a8_d6_showcancelled_windows() {
    run_decision(
        "A8-D6_showcancelled_windows",
        include_str!("fixtures/sim/lrm_decisions/A8-D6_showcancelled_windows.out"),
    );
}

#[test]
fn a8_d7_ondetect_filtered_pulse() {
    run_decision(
        "A8-D7_ondetect_filtered_pulse",
        include_str!("fixtures/sim/lrm_decisions/A8-D7_ondetect_filtered_pulse.out"),
    );
}

#[test]
fn a8_d8_pulse_trains() {
    run_decision(
        "A8-D8_pulse_trains",
        include_str!("fixtures/sim/lrm_decisions/A8-D8_pulse_trains.out"),
    );
}

#[test]
fn a8_d9_pulse_limits_keep_net_delay() {
    run_decision(
        "A8-D9_pulse_limits_keep_net_delay",
        include_str!("fixtures/sim/lrm_decisions/A8-D9_pulse_limits_keep_net_delay.out"),
    );
}

#[test]
fn a9_d1_simultaneous_events() {
    run_decision(
        "A9-D1_simultaneous_events",
        include_str!("fixtures/sim/lrm_decisions/A9-D1_simultaneous_events.out"),
    );
}

#[test]
fn a9_d2_removal_roles() {
    run_decision(
        "A9-D2_removal_roles",
        include_str!("fixtures/sim/lrm_decisions/A9-D2_removal_roles.out"),
    );
}

#[test]
fn a9_d3_time_zero_events() {
    run_decision(
        "A9-D3_time_zero_events",
        include_str!("fixtures/sim/lrm_decisions/A9-D3_time_zero_events.out"),
    );
}

#[test]
fn a9_d4_width_threshold() {
    run_decision(
        "A9-D4_width_threshold",
        include_str!("fixtures/sim/lrm_decisions/A9-D4_width_threshold.out"),
    );
}

#[test]
fn a9_d5_notifier_toggle_udp() {
    run_decision(
        "A9-D5_notifier_toggle_udp",
        include_str!("fixtures/sim/lrm_decisions/A9-D5_notifier_toggle_udp.out"),
    );
}

#[test]
fn a9_d6_duplicate_checks() {
    run_decision(
        "A9-D6_duplicate_checks",
        include_str!("fixtures/sim/lrm_decisions/A9-D6_duplicate_checks.out"),
    );
}

#[test]
fn a9_d7_shared_notifier() {
    run_decision(
        "A9-D7_shared_notifier",
        include_str!("fixtures/sim/lrm_decisions/A9-D7_shared_notifier.out"),
    );
}

#[test]
fn a9_d8_multibit_condition_lsb() {
    run_decision(
        "A9-D8_multibit_condition_lsb",
        include_str!("fixtures/sim/lrm_decisions/A9-D8_multibit_condition_lsb.out"),
    );
}

#[test]
fn a9_d9_condition_timing() {
    run_decision(
        "A9-D9_condition_timing",
        include_str!("fixtures/sim/lrm_decisions/A9-D9_condition_timing.out"),
    );
}

#[test]
fn a9_d10_vector_and_xz_edges() {
    run_decision(
        "A9-D10_vector_and_xz_edges",
        include_str!("fixtures/sim/lrm_decisions/A9-D10_vector_and_xz_edges.out"),
    );
}

#[test]
fn a9_d11_nochange_window() {
    run_decision(
        "A9-D11_nochange_window",
        include_str!("fixtures/sim/lrm_decisions/A9-D11_nochange_window.out"),
    );
}

#[test]
fn a9_d12_skew_repeats() {
    run_decision(
        "A9-D12_skew_repeats",
        include_str!("fixtures/sim/lrm_decisions/A9-D12_skew_repeats.out"),
    );
}

#[test]
fn a9_d13_negative_limit() {
    run_decision(
        "A9-D13_negative_limit",
        include_str!("fixtures/sim/lrm_decisions/A9-D13_negative_limit.out"),
    );
}

/// An A10 case that needs negative timing checks (31.9).
fn run_negative_timing_decision(case: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, case, expected, &["--neg-timing-checks"], &[]);
}

#[test]
fn a10_d1_combined_pairs() {
    run_decision(
        "A10-D1_combined_pairs",
        include_str!("fixtures/sim/lrm_decisions/A10-D1_combined_pairs.out"),
    );
}

#[test]
fn a10_d2_negative_option_off() {
    run_decision(
        "A10-D2_negative_option_off",
        include_str!("fixtures/sim/lrm_decisions/A10-D2_negative_option_off.out"),
    );
}

#[test]
fn a10_d3_delay_solution() {
    run_negative_timing_decision(
        "A10-D3_delay_solution",
        include_str!("fixtures/sim/lrm_decisions/A10-D3_delay_solution.out"),
    );
}

#[test]
fn a10_d4_limit_sum() {
    run_negative_timing_decision(
        "A10-D4_limit_sum",
        include_str!("fixtures/sim/lrm_decisions/A10-D4_limit_sum.out"),
    );
}

#[test]
fn a10_d5_inconsistent_limits() {
    run_negative_timing_decision(
        "A10-D5_inconsistent_limits",
        include_str!("fixtures/sim/lrm_decisions/A10-D5_inconsistent_limits.out"),
    );
}

#[test]
fn a10_d6_delayed_evaluation() {
    run_negative_timing_decision(
        "A10-D6_delayed_evaluation",
        include_str!("fixtures/sim/lrm_decisions/A10-D6_delayed_evaluation.out"),
    );
}

#[test]
fn a10_d7_nochange_negative_offsets() {
    run_decision(
        "A10-D7_nochange_negative_offsets",
        include_str!("fixtures/sim/lrm_decisions/A10-D7_nochange_negative_offsets.out"),
    );
}

#[test]
fn a10_d8_nochange_deferred() {
    run_negative_timing_decision(
        "A10-D8_nochange_deferred",
        include_str!("fixtures/sim/lrm_decisions/A10-D8_nochange_deferred.out"),
    );
}

#[test]
fn a10_d9_skew_flags() {
    run_decision(
        "A10-D9_skew_flags",
        include_str!("fixtures/sim/lrm_decisions/A10-D9_skew_flags.out"),
    );
}

#[test]
fn a10_d10_delay_lines() {
    run_negative_timing_decision(
        "A10-D10_delay_lines",
        include_str!("fixtures/sim/lrm_decisions/A10-D10_delay_lines.out"),
    );
}

#[test]
fn a13_d1_consequent_clock() {
    run_decision(
        "A13-D1_consequent_clock",
        include_str!("fixtures/sim/lrm_decisions/A13-D1_consequent_clock.out"),
    );
}

#[test]
fn a13_d2_clock_flow_scope() {
    run_decision(
        "A13-D2_clock_flow_scope",
        include_str!("fixtures/sim/lrm_decisions/A13-D2_clock_flow_scope.out"),
    );
}

#[test]
fn a13_d3_multiclock_else_start() {
    run_decision(
        "A13-D3_multiclock_else_start",
        include_str!("fixtures/sim/lrm_decisions/A13-D3_multiclock_else_start.out"),
    );
}

#[test]
fn a13_d4_async_abort_after_nonoverlap() {
    run_decision(
        "A13-D4_async_abort_after_nonoverlap",
        include_str!("fixtures/sim/lrm_decisions/A13-D4_async_abort_after_nonoverlap.out"),
    );
}

#[test]
fn a13_d5_event_list_clock_once() {
    run_decision(
        "A13-D5_event_list_clock_once",
        include_str!("fixtures/sim/lrm_decisions/A13-D5_event_list_clock_once.out"),
    );
}

#[test]
fn a13_d6_inferred_clock_flow() {
    run_decision(
        "A13-D6_inferred_clock_flow",
        include_str!("fixtures/sim/lrm_decisions/A13-D6_inferred_clock_flow.out"),
    );
}

#[test]
fn a13_d7_unbegun_strong_at_end() {
    run_decision(
        "A13-D7_unbegun_strong_at_end",
        include_str!("fixtures/sim/lrm_decisions/A13-D7_unbegun_strong_at_end.out"),
    );
}

#[test]
fn a14_d1_future_report_time() {
    run_decision(
        "A14-D1_future_report_time",
        include_str!("fixtures/sim/lrm_decisions/A14-D1_future_report_time.out"),
    );
}

#[test]
fn a14_d2_future_control_interval() {
    run_decision(
        "A14-D2_future_control_interval",
        include_str!("fixtures/sim/lrm_decisions/A14-D2_future_control_interval.out"),
    );
}

#[test]
fn a14_d3_future_end_of_simulation() {
    run_decision(
        "A14-D3_future_end_of_simulation",
        include_str!("fixtures/sim/lrm_decisions/A14-D3_future_end_of_simulation.out"),
    );
}

#[test]
fn a14_d4_future_action_controls() {
    run_decision(
        "A14-D4_future_action_controls",
        include_str!("fixtures/sim/lrm_decisions/A14-D4_future_action_controls.out"),
    );
}

#[test]
fn a15_d1_checker_reactive_sampling() {
    run_decision(
        "A15-D1_checker_reactive_sampling",
        include_str!("fixtures/sim/lrm_decisions/A15-D1_checker_reactive_sampling.out"),
    );
}

#[test]
fn a15_d2_checker_deferred_current() {
    run_decision(
        "A15-D2_checker_deferred_current",
        include_str!("fixtures/sim/lrm_decisions/A15-D2_checker_deferred_current.out"),
    );
}

#[test]
fn a15_d3_checker_unit_time_scale() {
    run_decision(
        "A15-D3_checker_unit_time_scale",
        include_str!("fixtures/sim/lrm_decisions/A15-D3_checker_unit_time_scale.out"),
    );
}

#[test]
fn a15_d4_free_initial_step() {
    run_decision(
        "A15-D4_free_initial_step",
        include_str!("fixtures/sim/lrm_decisions/A15-D4_free_initial_step.out"),
    );
}

#[test]
fn a15_d5_free_assume_solving() {
    run_decision(
        "A15-D5_free_assume_solving",
        include_str!("fixtures/sim/lrm_decisions/A15-D5_free_assume_solving.out"),
    );
}

#[test]
fn t0_d1_initializer_no_event() {
    run_decision(
        "T0-D1_initializer_no_event",
        include_str!("fixtures/sim/lrm_decisions/T0-D1_initializer_no_event.out"),
    );
}

#[test]
fn t0_d2_event_control_always_starts_first() {
    run_decision(
        "T0-D2_event_control_always_starts_first",
        include_str!("fixtures/sim/lrm_decisions/T0-D2_event_control_always_starts_first.out"),
    );
}

#[test]
fn t0_d3_driven_net_starts_x() {
    run_decision(
        "T0-D3_driven_net_starts_x",
        include_str!("fixtures/sim/lrm_decisions/T0-D3_driven_net_starts_x.out"),
    );
}

#[test]
fn t0_d3_driver_bits_start_separately() {
    run_decision(
        "T0-D3_driver_bits_start_separately",
        include_str!("fixtures/sim/lrm_decisions/T0-D3_driver_bits_start_separately.out"),
    );
}

#[test]
fn t0_d4_constant_z_driver_no_event() {
    run_decision(
        "T0-D4_constant_z_driver_no_event",
        include_str!("fixtures/sim/lrm_decisions/T0-D4_constant_z_driver_no_event.out"),
    );
}

#[test]
fn t0_d5_driven_net_time0_sample() {
    run_decision(
        "T0-D5_driven_net_time0_sample",
        include_str!("fixtures/sim/lrm_decisions/T0-D5_driven_net_time0_sample.out"),
    );
}

#[test]
fn t0_d5_assertion_net_time0_sample() {
    run_decision(
        "T0-D5_assertion_net_time0_sample",
        include_str!("fixtures/sim/lrm_decisions/T0-D5_assertion_net_time0_sample.out"),
    );
}

#[test]
fn px_d1_mixed_direction_port_expression() {
    run_decision(
        "PX-D1_mixed_direction_port_expression",
        include_str!("fixtures/sim/lrm_decisions/PX-D1_mixed_direction_port_expression.out"),
    );
}

#[test]
fn px_d2_indexed_port_reference() {
    run_decision(
        "PX-D2_indexed_port_reference",
        include_str!("fixtures/sim/lrm_decisions/PX-D2_indexed_port_reference.out"),
    );
}

#[test]
fn px_d3_explicit_ansi_port_expression() {
    run_decision(
        "PX-D3_explicit_ansi_port_expression",
        include_str!("fixtures/sim/lrm_decisions/PX-D3_explicit_ansi_port_expression.out"),
    );
}

#[test]
fn px_d4_port_expression_width_mismatch() {
    run_decision(
        "PX-D4_port_expression_width_mismatch",
        include_str!("fixtures/sim/lrm_decisions/PX-D4_port_expression_width_mismatch.out"),
    );
}

/// [`run_decision`] for an SDF annotation case: its companion files
/// (`<case>.sdf`, further SDF and configuration files) are written into the
/// run directory, where the case's `$sdf_annotate` calls name them.
fn run_sdf_decision(case: &str, expected: &str, files: &[(&str, &str)]) {
    sim_cli::run_case_backend_parity_with_files(SUITE, case, expected, &[], &[], files);
}

#[test]
fn a12_d1_leading_call_time0() {
    run_sdf_decision(
        "A12-D1_leading_call_time0",
        include_str!("fixtures/sim/lrm_decisions/A12-D1_leading_call_time0.out"),
        &[(
            "A12-D1_leading_call_time0.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D1_leading_call_time0.sdf"),
        )],
    );
}

#[test]
fn a12_d2_repeated_calls() {
    run_sdf_decision(
        "A12-D2_repeated_calls",
        include_str!("fixtures/sim/lrm_decisions/A12-D2_repeated_calls.out"),
        &[(
            "A12-D2_repeated_calls.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D2_repeated_calls.sdf"),
        )],
    );
}

#[test]
fn a12_d3_out_of_range_value() {
    run_sdf_decision(
        "A12-D3_out_of_range_value",
        include_str!("fixtures/sim/lrm_decisions/A12-D3_out_of_range_value.out"),
        &[(
            "A12-D3_out_of_range_value.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D3_out_of_range_value.sdf"),
        )],
    );
}

#[test]
fn a12_d4_scale_factors_argument() {
    run_sdf_decision(
        "A12-D4_scale_factors_argument",
        include_str!("fixtures/sim/lrm_decisions/A12-D4_scale_factors_argument.out"),
        &[(
            "A12-D4_scale_factors_argument.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D4_scale_factors_argument.sdf"),
        )],
    );
}

#[test]
fn a12_d5_file_applies_whole() {
    run_sdf_decision(
        "A12-D5_file_applies_whole",
        include_str!("fixtures/sim/lrm_decisions/A12-D5_file_applies_whole.out"),
        &[
            (
                "A12-D5_file_applies_whole.sdf",
                include_str!("fixtures/sim/lrm_decisions/A12-D5_file_applies_whole.sdf"),
            ),
            (
                "A12-D5_file_applies_whole_v.sdf",
                include_str!("fixtures/sim/lrm_decisions/A12-D5_file_applies_whole_v.sdf"),
            ),
        ],
    );
}

#[test]
fn a12_d6_config_file_per_call() {
    run_sdf_decision(
        "A12-D6_config_file_per_call",
        include_str!("fixtures/sim/lrm_decisions/A12-D6_config_file_per_call.out"),
        &[
            (
                "A12-D6_config_file_per_call.cfg",
                include_str!("fixtures/sim/lrm_decisions/A12-D6_config_file_per_call.cfg"),
            ),
            (
                "A12-D6_config_file_per_call.sdf",
                include_str!("fixtures/sim/lrm_decisions/A12-D6_config_file_per_call.sdf"),
            ),
        ],
    );
}

#[test]
fn a12_d7_iopath_after_pathpulse() {
    run_sdf_decision(
        "A12-D7_iopath_after_pathpulse",
        include_str!("fixtures/sim/lrm_decisions/A12-D7_iopath_after_pathpulse.out"),
        &[(
            "A12-D7_iopath_after_pathpulse.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D7_iopath_after_pathpulse.sdf"),
        )],
    );
}

#[test]
fn a12_d8_interconnect_unknown_source() {
    run_sdf_decision(
        "A12-D8_interconnect_unknown_source",
        include_str!("fixtures/sim/lrm_decisions/A12-D8_interconnect_unknown_source.out"),
        &[(
            "A12-D8_interconnect_unknown_source.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D8_interconnect_unknown_source.sdf"),
        )],
    );
}

#[test]
fn a12_d9_interconnect_pulse_limits() {
    run_sdf_decision(
        "A12-D9_interconnect_pulse_limits",
        include_str!("fixtures/sim/lrm_decisions/A12-D9_interconnect_pulse_limits.out"),
        &[(
            "A12-D9_interconnect_pulse_limits.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D9_interconnect_pulse_limits.sdf"),
        )],
    );
}

#[test]
fn a12_d10_multisource_load_delay() {
    run_sdf_decision(
        "A12-D10_multisource_load_delay",
        include_str!("fixtures/sim/lrm_decisions/A12-D10_multisource_load_delay.out"),
        &[(
            "A12-D10_multisource_load_delay.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D10_multisource_load_delay.sdf"),
        )],
    );
}

#[test]
fn a12_d11_device_replaces_primitive() {
    run_sdf_decision(
        "A12-D11_device_replaces_primitive",
        include_str!("fixtures/sim/lrm_decisions/A12-D11_device_replaces_primitive.out"),
        &[(
            "A12-D11_device_replaces_primitive.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D11_device_replaces_primitive.sdf"),
        )],
    );
}

#[test]
fn a12_d12_increment_floor() {
    run_sdf_decision(
        "A12-D12_increment_floor",
        include_str!("fixtures/sim/lrm_decisions/A12-D12_increment_floor.out"),
        &[(
            "A12-D12_increment_floor.sdf",
            include_str!("fixtures/sim/lrm_decisions/A12-D12_increment_floor.sdf"),
        )],
    );
}

#[test]
fn a5_d1_unknown_control_union() {
    run_decision(
        "A5-D1_unknown_control_union",
        include_str!("fixtures/sim/lrm_decisions/A5-D1_unknown_control_union.out"),
    );
}

#[test]
fn a5_d2_switch_node_blocking() {
    run_decision(
        "A5-D2_switch_node_blocking",
        include_str!("fixtures/sim/lrm_decisions/A5-D2_switch_node_blocking.out"),
    );
}

#[test]
fn a5_d3_forced_switch_node() {
    run_decision(
        "A5-D3_forced_switch_node",
        include_str!("fixtures/sim/lrm_decisions/A5-D3_forced_switch_node.out"),
    );
}

#[test]
fn a5_d4_tranif_delay_state() {
    run_decision(
        "A5-D4_tranif_delay_state",
        include_str!("fixtures/sim/lrm_decisions/A5-D4_tranif_delay_state.out"),
    );
}

#[test]
fn a5_d5_member_net_delay() {
    run_decision(
        "A5-D5_member_net_delay",
        include_str!("fixtures/sim/lrm_decisions/A5-D5_member_net_delay.out"),
    );
}

#[test]
fn a5_d6_tran_supply_reduction() {
    run_decision(
        "A5-D6_tran_supply_reduction",
        include_str!("fixtures/sim/lrm_decisions/A5-D6_tran_supply_reduction.out"),
    );
}
