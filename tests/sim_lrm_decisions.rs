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
