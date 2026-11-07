use super::{reject_case, run_case_with_stderr};

#[test]
fn severity_tasks_format_once_and_keep_stable_counts() {
    run_case_with_stderr(
        "severity_nonfatal",
        "stdout evals=3\n",
        "INFO: severity_nonfatal.sv:13:5: [SIM-003] (tb @ 0 ps) scope=tb info=1 literal={args}\nWARNING: severity_nonfatal.sv:14:5: [SIM-004] (tb @ 0 ps) warning=2\nERROR: severity_nonfatal.sv:15:5: [SIM-005] (tb @ 0 ps) error=3\nINFO: severity_nonfatal.sv:17:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-008] simulation statistics: processes=1\nINFO: [SIM-008] severity counts: info=1 warning=1 error=1 fatal=0\nINFO: [SIM-000] 1 error(s), 1 warning(s), 4 info(s)\n",
    );
}

#[test]
fn nonfatal_severity_tasks_allow_empty_messages() {
    run_case_with_stderr(
        "severity_no_args",
        "stdout after\n",
        "INFO: severity_no_args.sv:4:5: [SIM-003] (tb @ 0 ps) \nWARNING: severity_no_args.sv:5:5: [SIM-004] (tb @ 0 ps) \nERROR: severity_no_args.sv:6:5: [SIM-005] (tb @ 0 ps) \nINFO: [SIM-000] 1 error(s), 1 warning(s), 1 info(s)\n",
    );
}

#[test]
fn fatal_stops_the_current_process_and_runs_final_once() {
    run_case_with_stderr(
        "severity_fatal",
        "stdout before\nstdout final evals=1\n",
        "FATAL: severity_fatal.sv:14:5: [SIM-006] (tb @ 0 ps) fatal=7\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s)\n",
    );
}

#[test]
fn fatal_level_two_reuses_finish_statistics_and_counts() {
    run_case_with_stderr(
        "severity_fatal_level2",
        "stdout final\n",
        "FATAL: severity_fatal_level2.sv:4:5: [SIM-006] (tb @ 0 ps) fatal level 2\nINFO: severity_fatal_level2.sv:4:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-008] simulation statistics: processes=1\nINFO: [SIM-008] severity counts: info=0 warning=0 error=0 fatal=1\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 3 info(s)\n",
    );
}

#[test]
fn fatal_message_without_finish_number_uses_level_one() {
    run_case_with_stderr(
        "severity_fatal_default",
        "stdout final\n",
        "FATAL: severity_fatal_default.sv:4:5: [SIM-006] (tb @ 0 ps) fatal default\nINFO: severity_fatal_default.sv:4:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 1 info(s)\n",
    );
}

#[test]
fn fatal_real_first_message_uses_default_finish_level() {
    run_case_with_stderr(
        "severity_fatal_real",
        "stdout final\n",
        "FATAL: severity_fatal_real.sv:6:5: [SIM-006] (tb @ 0 ps) 1.250000\nINFO: severity_fatal_real.sv:6:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 1 info(s)\n",
    );
}

#[test]
fn fatal_string_first_message_uses_default_finish_level() {
    run_case_with_stderr(
        "severity_fatal_string",
        "stdout final\n",
        "FATAL: severity_fatal_string.sv:7:5: [SIM-006] (tb @ 0 ps) dynamic fatal\nINFO: severity_fatal_string.sv:7:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 1 info(s)\n",
    );
}

#[test]
fn fatal_after_an_error_ends_the_run_with_a_failure_status() {
    // The harness expects exit status 1 because a fatal report is expected;
    // the `$error` alone ends with status 0 (`severity_nonfatal`).
    run_case_with_stderr(
        "severity_error_then_fatal",
        "stdout between\n",
        "ERROR: severity_error_then_fatal.sv:4:5: [SIM-005] (tb @ 0 ps) e1\nFATAL: severity_error_then_fatal.sv:6:5: [SIM-006] (tb @ 0 ps) boom\nINFO: severity_error_then_fatal.sv:6:5: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 1 fatal, 1 error(s), 0 warning(s), 1 info(s)\n",
    );
}

#[test]
fn fatal_in_a_forked_task_ends_the_run_with_a_failure_status() {
    run_case_with_stderr(
        "severity_fatal_in_fork",
        "stdout final\n",
        "FATAL: severity_fatal_in_fork.sv:4:8: [SIM-006] (tb @ 1 ns) late fatal\nINFO: severity_fatal_in_fork.sv:4:8: [SIM-001] (tb @ 1 ns) $finish called\nINFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 1 info(s)\n",
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
        "WARNING: severity_long_messages.sv:12:9: [SIM-004] (tb @ 0 ps) {digits}|{digits}\n\
         ERROR: severity_long_messages.sv:13:9: [SIM-007] (tb @ 0 ps) assert failed \
         (long_label_for_the_failing_immediate_assertion)\n\
         INFO: severity_long_messages.sv:14:9: [SIM-001] (tb @ 0 ps) $finish called\n\
         INFO: [SIM-000] 1 error(s), 1 warning(s), 1 info(s)\n"
    );
    run_case_with_stderr("severity_long_messages", "", &stderr);
}
