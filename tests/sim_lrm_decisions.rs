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
