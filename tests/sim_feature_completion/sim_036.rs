//! SIM-036: deferred immediate assertions in subroutine, class, final and
//! module contexts; report queues, flush points, disable, assertion control
//! and the single-call action contract. Oracles are derived by hand in the
//! fixture readme from IEEE 1800-2009 §§16.4, 20.11, 9.2.3 and 4.4.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_036";

/// Reports of different processes in one time step execute in the Reactive
/// region, whose order is arbitrary (SV 4.4.2.6): compare the line sets.
fn same_lines(label: &str, output: &std::process::Output, expected: &str) {
    assert!(output.status.success(), "{label}: {output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut got: Vec<&str> = stdout.lines().collect();
    let mut want: Vec<&str> = expected.lines().collect();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want, "{label}: {stdout}");
}

// ── A01: one function, several processes; issue-time values ───────────────

#[test]
fn function_called_by_two_processes_keeps_independent_queues() {
    sim_cli::run_case_checked_matrix(SUITE, "function_two_processes", &[], &|label, output| {
        same_lines(
            label,
            output,
            include_str!("../fixtures/sim/feature_completion/sim_036/function_two_processes.out"),
        )
    });
}

#[test]
fn tasks_methods_and_forked_callers_report_issue_time_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "subroutine_contexts",
        include_str!("../fixtures/sim/feature_completion/sim_036/subroutine_contexts.out"),
        &[],
        &[],
    );
}

// ── A02: flush points, disable, assertion control, native arguments ────────

#[test]
fn flush_points_clear_only_the_resuming_process_queue() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "flush_points",
        include_str!("../fixtures/sim/feature_completion/sim_036/flush_points.out"),
        &[],
        &[],
    );
}

#[test]
fn disable_cancels_an_assertion_and_flushes_an_outermost_scope() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "disable_flush",
        include_str!("../fixtures/sim/feature_completion/sim_036/disable_flush.out"),
        &[],
        &[],
    );
}

#[test]
fn assertoff_keeps_queued_reports_and_assertkill_flushes_pending_ones() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "assert_control",
        include_str!("../fixtures/sim/feature_completion/sim_036/assert_control.out"),
        &[],
        &[],
    );
}

#[test]
fn native_and_packed_arguments_use_issue_time_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "native_arguments",
        include_str!("../fixtures/sim/feature_completion/sim_036/native_arguments.out"),
        &[],
        &[],
    );
}

// ── A03: control actions, final context and negative boundaries ───────────

#[test]
fn control_actions_execute_once_in_the_reactive_region() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "control_actions",
        include_str!("../fixtures/sim/feature_completion/sim_036/control_actions.out"),
        &[],
        &[],
    );
}

#[test]
fn finish_action_ends_the_step_after_the_issuing_process() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "finish_action",
        include_str!("../fixtures/sim/feature_completion/sim_036/finish_action.out"),
        &[],
        &[],
    );
}

#[test]
fn fatal_action_ends_the_run() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fatal_action",
        include_str!("../fixtures/sim/feature_completion/sim_036/fatal_action.out"),
        &[],
        &[],
    );
}

#[test]
fn final_procedure_reports_when_it_returns() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "final_block",
        include_str!("../fixtures/sim/feature_completion/sim_036/final_block.out"),
        &[],
        &[],
    );
}

#[test]
fn begin_end_action_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_begin_action",
        "deferred assertion action must be a subroutine call",
    );
}

#[test]
fn output_and_inout_action_formals_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_formal",
        "cannot invoke a subroutine with 'output' or 'inout' arguments from a deferred assertion action",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_inout_formal",
        "cannot invoke a subroutine with 'output' or 'inout' arguments from a deferred assertion action",
    );
}

#[test]
fn automatic_and_dynamic_ref_actuals_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_ref",
        "cannot pass automatic variable 'local_value' to 'ref' argument 'x' in a deferred assertion action",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_class_property_ref",
        "cannot pass automatic variable 'v' to 'ref' argument 'x' in a deferred assertion action",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_array_ref",
        "deferred immediate assertion action cannot pass a dynamic variable to ref argument `x` at tb:11:27",
    );
}

#[test]
fn assert_final_is_not_a_2009_form() {
    sim_cli::reject_case(
        SUITE,
        "neg_assert_final",
        "`assert final` is not available in IEEE 2009",
    );
}
