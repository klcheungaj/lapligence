use super::{reject_case, run_case_with_stderr};

#[test]
fn severity_tasks_format_once_and_keep_stable_counts() {
    run_case_with_stderr(
        "severity_nonfatal",
        "stdout evals=3\n",
        concat!(
            "llg: severity info: tb:13:5: scope=tb info=1 literal={args}\n",
            "llg: severity warning: tb:14:5: warning=2\n",
            "llg: severity error: tb:15:5: error=3\n",
            "llg: $finish at time 0 at tb:17:5\n",
            "llg: simulation statistics: processes=1\n",
            "llg: severity counts: info=1 warning=1 error=1 fatal=0\n",
        ),
    );
}

#[test]
fn nonfatal_severity_tasks_allow_empty_messages() {
    run_case_with_stderr(
        "severity_no_args",
        "stdout after\n",
        concat!(
            "llg: severity info: tb:4:5: \n",
            "llg: severity warning: tb:5:5: \n",
            "llg: severity error: tb:6:5: \n",
        ),
    );
}

#[test]
fn fatal_stops_the_current_process_and_runs_final_once() {
    run_case_with_stderr(
        "severity_fatal",
        "stdout before\nstdout final evals=1\n",
        "llg: severity fatal: tb:14:5: fatal=7\n",
    );
}

#[test]
fn fatal_level_two_reuses_finish_statistics_and_counts() {
    run_case_with_stderr(
        "severity_fatal_level2",
        "stdout final\n",
        concat!(
            "llg: severity fatal: tb:4:5: fatal level 2\n",
            "llg: $finish at time 0 at tb:4:5\n",
            "llg: simulation statistics: processes=1\n",
            "llg: severity counts: info=0 warning=0 error=0 fatal=1\n",
        ),
    );
}

#[test]
fn fatal_message_without_finish_number_uses_level_one() {
    run_case_with_stderr(
        "severity_fatal_default",
        "stdout final\n",
        concat!(
            "llg: severity fatal: tb:4:5: fatal default\n",
            "llg: $finish at time 0 at tb:4:5\n",
        ),
    );
}

#[test]
fn fatal_real_first_message_uses_default_finish_level() {
    run_case_with_stderr(
        "severity_fatal_real",
        "stdout final\n",
        concat!(
            "llg: severity fatal: tb:6:5: 1.250000\n",
            "llg: $finish at time 0 at tb:6:5\n",
        ),
    );
}

#[test]
fn fatal_string_first_message_uses_default_finish_level() {
    run_case_with_stderr(
        "severity_fatal_string",
        "stdout final\n",
        concat!(
            "llg: severity fatal: tb:7:5: dynamic fatal\n",
            "llg: $finish at time 0 at tb:7:5\n",
        ),
    );
}

#[test]
fn fatal_after_an_error_ends_the_run_with_a_failure_status() {
    // The harness expects exit status 1 because a fatal report is expected;
    // the `$error` alone ends with status 0 (`severity_nonfatal`).
    run_case_with_stderr(
        "severity_error_then_fatal",
        "stdout between\n",
        concat!(
            "llg: severity error: tb:4:5: e1\n",
            "llg: severity fatal: tb:6:5: boom\n",
            "llg: $finish at time 0 at tb:6:5\n",
        ),
    );
}

#[test]
fn fatal_in_a_forked_task_ends_the_run_with_a_failure_status() {
    run_case_with_stderr(
        "severity_fatal_in_fork",
        "stdout final\n",
        concat!(
            "llg: severity fatal: tb:4:8: late fatal\n",
            "llg: $finish at time 1000 at tb:4:8\n",
        ),
    );
}

#[test]
fn the_model_binary_exits_with_the_status_of_its_severity_tasks() {
    // The generated executable itself, without `llg` in between: `$fatal`
    // ends the run "with an error code" (SV 20.9), while `$error` and a
    // `$finish` that follows it leave the status 0.
    let run = |source: &str, tag: &str| {
        crate::sim_harness::run_generated_sim_allow_failure(source, "tb", tag)
            .unwrap_or_else(|error| panic!("{tag}: {error}"))
    };
    let fatal = run(
        "module tb; initial begin $error(\"e1\"); $fatal(1, \"boom\"); end endmodule",
        "severity-exit-fatal",
    );
    assert_eq!(fatal.status.code(), Some(1), "{}", fatal.stderr);
    let nonfatal = run(
        "module tb; initial begin $error(\"e1\"); #1 $finish; end endmodule",
        "severity-exit-error",
    );
    assert_eq!(nonfatal.status.code(), Some(0), "{}", nonfatal.stderr);
}

#[test]
fn fatal_finish_number_must_be_a_known_constant_in_range() {
    reject_case("severity_invalid_argument", "$fatal finish number at");
    reject_case("severity_runtime_argument", "$fatal finish number at");
}

#[test]
fn long_runtime_reports_keep_every_byte_on_one_line() {
    // Longer than the runtime reporter's inline buffer, so the message
    // grows on the heap before its single write.
    let digits = "0123456789".repeat(100);
    let stderr = format!(
        "llg: severity warning: tb:12:9: {digits}|{digits}\n\
         llg: assertion assert failed: tb:13:9 (long_label_for_the_failing_immediate_assertion)\n\
         llg: $finish at time 0 at tb:14:9\n"
    );
    run_case_with_stderr("severity_long_messages", "", &stderr);
}
