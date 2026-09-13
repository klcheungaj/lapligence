use super::{reject_case, reject_case_with_args, run_case, run_case_with_stderr};

#[test]
fn finish_does_not_return_to_the_current_process() {
    run_case("finish_does_not_return", "CHECK: before\nCHECK: final\n");
}

#[test]
fn finish_does_not_return_through_function_and_task_calls() {
    run_case(
        "finish_nested",
        "CHECK: task before\nCHECK: function before\nCHECK: final\n",
    );
}

#[test]
fn finish_does_not_return_from_a_forked_coroutine() {
    run_case("finish_fork", "CHECK: branch before\nCHECK: final\n");
}

#[test]
fn finish_discards_pending_nba_and_timed_work_before_finals() {
    run_case(
        "finish_pending",
        "CHECK: nba queued\nCHECK: finish\nCHECK: final value=0\n",
    );
}

#[test]
fn finish_runs_only_the_first_final_and_stops_that_final() {
    run_case("finish_final_boundary", "CHECK: final one\n");
}

#[test]
fn finish_level_zero_is_quiet_and_default_reports_level_one() {
    run_case("finish_verbose_0", "CHECK: level 0\n");
    run_case_with_stderr(
        "finish_verbose_default",
        "CHECK: default\n",
        "llg: $finish at time 0 at tb:4:5\n",
    );
}

#[test]
fn finish_levels_report_time_and_statistics_without_changing_exit_status() {
    run_case_with_stderr(
        "finish_verbose_1",
        "CHECK: level 1\n",
        "llg: $finish at time 0 at tb:4:5\n",
    );
    run_case_with_stderr(
        "finish_verbose_2",
        "CHECK: level 2\n",
        "llg: $finish at time 0 at tb:4:5\nllg: simulation statistics: processes=1\n",
    );
}

#[test]
fn finish_rejects_an_out_of_range_verbosity_with_source_context() {
    reject_case("finish_invalid_argument", "$finish argument at");
}

#[test]
fn finish_number_is_constant_in_both_target_editions() {
    reject_case_with_args("finish_runtime_argument", "$finish argument at", &[]);
    reject_case_with_args(
        "finish_runtime_argument",
        "$finish argument at",
        &["--edition", "2001"],
    );
}
