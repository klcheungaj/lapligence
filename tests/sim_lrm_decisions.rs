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
