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
fn fatal_finish_number_must_be_a_known_constant_in_range() {
    reject_case("severity_invalid_argument", "$fatal finish number at");
    reject_case("severity_runtime_argument", "$fatal finish number at");
}
