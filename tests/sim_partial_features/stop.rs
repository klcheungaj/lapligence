use super::{reject_case, run_case, run_case_with_stderr, sim_cli};

#[test]
fn stop_resumes_nested_call_and_preserves_future_work_and_finals() {
    run_case(
        "stop_resume",
        concat!(
            "CHECK: before\n",
            "CHECK: nested before\n",
            "CHECK: nested after t=0\n",
            "CHECK: resumed t=0\n",
            "CHECK: pending value=90 t=2\n",
            "CHECK: final value=5a t=3\n",
        ),
    );
}

#[test]
fn stop_exit_policy_returns_without_draining_pending_work_or_finals() {
    sim_cli::run_case_with_args(
        "partial_features",
        "stop_resume",
        "CHECK: before\nCHECK: nested before\n",
        "",
        &[],
        &["--stop-policy", "exit"],
    );
}

#[test]
fn stop_default_and_level_two_report_diagnostics_without_affecting_exit_status() {
    run_case_with_stderr(
        "stop_verbose_default",
        "",
        "llg: $stop at time 0 at tb:5:9\n",
    );
    run_case_with_stderr(
        "stop_verbose_2",
        "",
        "llg: $stop at time 0 at tb:5:9\nllg: simulation statistics: processes=1\n",
    );
}

#[test]
fn stop_rejects_an_out_of_range_verbosity_with_source_context() {
    reject_case("stop_invalid_argument", "$stop argument at");
}
