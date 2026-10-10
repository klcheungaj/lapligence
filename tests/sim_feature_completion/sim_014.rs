//! SIM-014: repeated waits, intra-assignment timing and event triggers.
//! Oracles are derived by hand in the fixture readme from V2001 §§9.6,
//! 9.7.7 and IEEE 1800-2009 §§4.9.3, 4.9.4, 6.12.2, 6.21, 9.4.5, 10.4,
//! 12.7.2, 13.3.2, 13.4 and 15.5.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_014";

/// Run a fixture after the frontend snapshot and owned Db are destroyed; the
/// runtime stderr is the `$finish` report at the fixture's own location.
fn after_db_drop(fixture: &str, expected: &str, finish: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        fixture,
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        finish,
    );
}

#[test]
fn standalone_repeat_counts_follow_one_conversion() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_counts",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_counts.out"),
        &[],
        &[],
    );
}

#[test]
fn intra_assignment_repeat_counts_and_issue_time_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "intra_counts",
        include_str!("../fixtures/sim/feature_completion/sim_014/intra_counts.out"),
        &[],
        &[],
    );
}

#[test]
fn blocking_destinations_resolve_at_update_and_nba_destinations_at_issue() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/destinations.out");
    sim_cli::run_case_backend_parity(SUITE, "destinations", expected, &[], &[]);
    after_db_drop(
        "destinations",
        expected,
        "llg: $finish at time 81 at tb:116:5\n",
    );
}

#[test]
fn blocking_container_element_destinations_resolve_at_update() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "container_elements",
        include_str!("../fixtures/sim/feature_completion/sim_014/container_elements.out"),
        &[],
        &[],
    );
}

#[test]
fn nonblocking_triggers_keep_the_event_named_at_issue() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/nb_triggers.out");
    sim_cli::run_case_backend_parity(SUITE, "nb_triggers", expected, &[], &[]);
    after_db_drop(
        "nb_triggers",
        expected,
        "llg: $finish at time 37 at tb:60:8\n",
    );
}

#[test]
fn mixed_lists_and_event_formals_of_tasks_functions_and_methods() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_formals",
        include_str!("../fixtures/sim/feature_completion/sim_014/event_formals.out"),
        &[],
        &[],
    );
}

#[test]
fn triggered_state_identity_and_wait_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_values",
        include_str!("../fixtures/sim/feature_completion/sim_014/event_values.out"),
        &[],
        &[],
    );
}

#[test]
fn failed_wait_order_without_else_reports_a_runtime_error() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/wait_order_error.out");
    sim_cli::run_case_checked_matrix(SUITE, "wait_order_error", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert!(
            stderr
                .contains("llg: severity error: tb:9:9: wait_order: events triggered out of order"),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn cancelled_repeated_waits_leave_no_pending_work() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cancellation",
        include_str!("../fixtures/sim/feature_completion/sim_014/cancellation.out"),
        &[],
        &[],
    );
}

#[test]
fn intra_assignment_event_controls_in_task_bodies() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "task_timing",
        include_str!("../fixtures/sim/feature_completion/sim_014/task_timing.out"),
        &[],
        &[],
    );
}

#[test]
fn verilog_2001_repeated_event_controls() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "v2001_forms.v",
        include_str!("../fixtures/sim/feature_completion/sim_014/v2001_forms.out"),
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn repeat_real_witness() {
    // FND-002 L-F06-05-03: an integral-valued real count.
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_real_witness.v",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_real_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn persistent_repeat_nba_witness() {
    // FND-002 L-F06-04-04: a repeated-event string NBA.
    sim_cli::run_case_backend_parity(
        SUITE,
        "persistent_repeat_nba_witness",
        include_str!(
            "../fixtures/sim/feature_completion/sim_014/persistent_repeat_nba_witness.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn repeat_nested_timing_witness() {
    // FND-002 L-F06-04-01 / L-F06-05-02: a repeat loop around a repeated
    // intra-assignment control.
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_nested_timing_witness",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_nested_timing_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn trigger_repeat_witness() {
    // FND-002: `->> repeat (2) @(posedge clk) e`.
    sim_cli::run_case_backend_parity(
        SUITE,
        "trigger_repeat_witness",
        include_str!("../fixtures/sim/feature_completion/sim_014/trigger_repeat_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn neg_class_property_nba() {
    // Language rule (SV 6.21), reported by llg with its location.
    sim_cli::reject_case(
        SUITE,
        "neg_class_property_nba",
        "neg_class_property_nba.sv:10:14 in `tb` is illegal (IEEE 1800-2009 6.21",
    );
}

#[test]
fn neg_automatic_event_nba() {
    // Language rule (SV 6.21, 10.4.2), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_event_nba",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
}

#[test]
fn neg_automatic_nba_witness() {
    // FND-002 L-F06-04-02: a queued automatic real write.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_nba_witness",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
}

#[test]
fn neg_automatic_in_nba_control() {
    // Language rule (SV 13.3.2), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_in_nba_control",
        "cannot refer to automatic variable 'w' from intra-assignment event control of nonblocking assignment",
    );
}

#[test]
fn neg_dynamic_element_nba() {
    // Language rule (SV 6.21), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_element_nba",
        "nonblocking assignments to elements of dynamically sized arrays are not allowed",
    );
}

#[test]
fn neg_function_event_control() {
    // Language rule (SV 13.4), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_function_event_control",
        "statements that pass time are not allowed in this context",
    );
}
