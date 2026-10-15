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
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = crate::sim_harness::TempDir::new("lrm-b8").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/lrm_decisions/B8_context_import_strobe.c");
    let library = directory.path().join("libb8_context.so");
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let output = crate::sim_harness::run_command(
        std::process::Command::new(compiler)
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&library),
        std::time::Duration::from_secs(60),
    )
    .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let library = library.to_string_lossy();
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

#[test]
fn s14_d6_wait_order_error_continues() {
    run_decision(
        "S14-D6_wait_order_error_continues",
        include_str!("fixtures/sim/lrm_decisions/S14-D6_wait_order_error_continues.out"),
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
fn s36_d3_receiver_at_issue() {
    run_decision(
        "S36-D3_receiver_at_issue",
        include_str!("fixtures/sim/lrm_decisions/S36-D3_receiver_at_issue.out"),
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
/// `directory`; `None` (with a SKIP note) on hosts without shared DPI fixtures.
fn dpi_companion(directory: &crate::sim_harness::TempDir, case: &str) -> Option<String> {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return None;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/lrm_decisions/{case}.c"));
    let library = directory.path().join(format!("lib{case}.so"));
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let mut command = std::process::Command::new(compiler);
    command
        .args(["-shared", "-fPIC", "-I"])
        .arg(root.join("src/sim/rt"));
    if cfg!(target_os = "macos") {
        command.args(["-undefined", "dynamic_lookup"]);
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command.arg(&source).arg("-o").arg(&library);
    let output = crate::sim_harness::run_command(&mut command, std::time::Duration::from_secs(60))
        .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(library.to_string_lossy().into_owned())
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
    run_dpi_fatal_decision(
        "S41-D8_task_from_import_function",
        include_str!("fixtures/sim/lrm_decisions/S41-D8_task_from_import_function.out"),
        "llg: DPI-C export task `d8_task` was called from an imported function",
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

#[test]
fn s31_d4_negative_weight_unsigned() {
    run_decision(
        "S31-D4_negative_weight_unsigned",
        include_str!("fixtures/sim/lrm_decisions/S31-D4_negative_weight_unsigned.out"),
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
fn s31_d7_draw_is_urandom_range() {
    run_decision(
        "S31-D7_draw_is_urandom_range",
        include_str!("fixtures/sim/lrm_decisions/S31-D7_draw_is_urandom_range.out"),
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
fn s27_d22_associative_rows_whole() {
    run_decision(
        "S27-D22_associative_rows_whole",
        include_str!("fixtures/sim/lrm_decisions/S27-D22_associative_rows_whole.out"),
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
fn s29_d1_unknown_state_fails() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S29-D1_unknown_state_fails.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S29-D1_unknown_state_fails",
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
fn s29_d6_failed_call_consumes_draws() {
    run_decision(
        "S29-D6_failed_call_consumes_draws",
        include_str!("fixtures/sim/lrm_decisions/S29-D6_failed_call_consumes_draws.out"),
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
    run_decision(
        "S30-D6_empty_reductions",
        include_str!("fixtures/sim/lrm_decisions/S30-D6_empty_reductions.out"),
    );
}

#[test]
fn s30_d7_hook_order() {
    run_decision(
        "S30-D7_hook_order",
        include_str!("fixtures/sim/lrm_decisions/S30-D7_hook_order.out"),
    );
}

#[test]
fn s30_d8_negative_weight_error() {
    let expected = include_str!("fixtures/sim/lrm_decisions/S30-D8_negative_weight_error.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S30-D8_negative_weight_error",
        &[],
        &|label, output| {
            runtime_error(
                label,
                output,
                expected,
                "negative or unevaluated dist weight",
            );
        },
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
    // Negative case: the expected stdout is empty because compilation fails.
    assert!(include_str!("fixtures/sim/lrm_decisions/S30-D10_dist_random_weight.out").is_empty());
    sim_cli::reject_case(
        SUITE,
        "S30-D10_dist_random_weight",
        "a dist item value, range bound or weight that reads a random variable is not supported",
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
    sim_cli::run_case_backend_parity(SUITE, case, expected, &args, &[]);
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
fn s42_d5_delayed_put_nba() {
    run_vpi_decision(
        "S42-D5_delayed_put_nba",
        &["task $d5()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D5_delayed_put_nba.out"),
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
fn s42_d8_size_of_real_undefined() {
    run_vpi_decision(
        "S42-D8_size_of_real_undefined",
        &["task $d8()"],
        include_str!("fixtures/sim/lrm_decisions/S42-D8_size_of_real_undefined.out"),
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
fn s43_d1_read_write_synch_after_nba() {
    run_vpi_decision(
        "S43-D1_read_write_synch_after_nba",
        &[],
        include_str!("fixtures/sim/lrm_decisions/S43-D1_read_write_synch_after_nba.out"),
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
fn s43_d9_postponed_systf_read_only() {
    run_vpi_decision(
        "S43-D9_postponed_systf_read_only",
        &["function int $bump(input int v)"],
        include_str!("fixtures/sim/lrm_decisions/S43-D9_postponed_systf_read_only.out"),
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

/// The illegal sample is a run-time error: exact stdout, exit status 1.
#[test]
fn s45_d3_illegal_bin_continues() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "S45-D3_illegal_bin_continues",
        &[],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                include_str!("fixtures/sim/lrm_decisions/S45-D3_illegal_bin_continues.out"),
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

/// A non-positive bins array size from an argument is a run-time error.
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
                    "llg: runtime error: bins `b` at tb:18:12: the number of bins is not a positive value"
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
