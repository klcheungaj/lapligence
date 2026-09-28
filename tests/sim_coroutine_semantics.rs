//! Phase-0 semantic oracles for the stackless-coroutine migration.
//!
//! These fixtures run unchanged on the current stackful backend. The shared
//! harness invokes every positive fixture through the public CLI with and
//! without optimizer passes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn event_waits_named_events_and_wait_order_are_one_shot() {
    sim_cli::run_case(
        "coroutine_semantics",
        "event_waits",
        "PASS event_waits wakes=2 triggered=3 success=1 failure=1\n",
        "",
        &[],
    );
}

#[test]
fn intra_assignment_delay_captures_rhs_and_applies_lhs_at_the_defined_stage() {
    sim_cli::run_case(
        "coroutine_semantics",
        "intra_assignment_delay_capture",
        "PASS intra_assignment_delay_capture blocking=08 nba=01 source=0 index=3\n",
        "",
        &[],
    );
}

#[test]
fn nested_control_flow_preserves_loop_state_across_waits() {
    sim_cli::run_case(
        "coroutine_semantics",
        "nested_control_flow",
        "PASS nested_control_flow sum=133 i=3 w=2 r=3 f=2\n",
        "",
        &[],
    );
}

#[test]
fn nested_timing_calls_preserve_locals_outputs_and_call_shapes() {
    sim_cli::run_case(
        "coroutine_semantics",
        "nested_timing_calls",
        "PASS nested_timing_calls delay=44 event=44 direct=2\n",
        "",
        &[],
    );
}

#[test]
fn timed_output_and_inout_copy_back_once_and_skip_cancelled_calls() {
    sim_cli::run_case(
        "coroutine_semantics",
        "copyback_once_cancel",
        "PASS copyback_once_cancel value=12 changes=1 cancelled=0\n",
        "",
        &[],
    );
}

#[test]
fn join_none_children_wait_for_a_real_blocking_boundary() {
    sim_cli::run_case(
        "coroutine_semantics",
        "stackless_join_none_real_block",
        concat!(
            "READY boundary child=0 ready=2\n",
            "PASS stackless_join_none_real_block child=1\n",
        ),
        "",
        &[],
    );
}

#[test]
fn deep_kill_and_named_disable_do_not_resume_cancelled_code() {
    sim_cli::run_case(
        "coroutine_semantics",
        "deep_cancellation",
        "PASS deep_cancellation killed=4 kill_after=0 disable_after=1 escaped=0 survivor=1\n",
        "",
        &[],
    );
}

#[test]
fn self_suspend_and_resume_preserve_the_pending_continuation() {
    sim_cli::run_case(
        "coroutine_semantics",
        "self_suspend",
        "PASS self_suspend suspended=3 before=1 after=2 self_resume=1\n",
        "",
        &[],
    );
}

#[test]
fn stop_and_finish_propagate_through_nested_timing_calls() {
    sim_cli::run_case(
        "coroutine_semantics",
        "termination_depth",
        concat!(
            "stop before\n",
            "stop resumed\n",
            "finish before\n",
            "final count=1 stop_after=1 finish_after=0\n",
        ),
        "",
        &[],
    );
}

#[test]
fn plain_function_loop_exhausts_the_process_step_budget() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "coroutine_semantics",
            "function_step_budget",
            optimized,
            &[],
            &[("LLG_PROCESS_STEP_LIMIT", "8")],
            &["LLG_ZERO_LOOP_LIMIT", "LLG_NONCONVERGENCE_LIMIT"],
        );
        let label = format!("function_step_budget, optimized={optimized}");
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert!(output.stdout.is_empty(), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "llg: nonconvergent zero-time execution in process `fn_tb_spin` at time 0 (process step limit 8)\n",
            "{label}"
        );
    }
}
