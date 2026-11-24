//! Elaboration system tasks (IEEE 1800-2009 20.10) and lone `;` items: their
//! messages, and that the model is still generated and runs. See the fixture
//! readme.

use super::{sim_cli, sim_harness};
use std::process::Output;

const SUITE: &str = "feature_completion/ki_elab_tasks";

/// The messages of `output` with one of the frontend ids `ids`, as
/// `SEVERITY line:column text`, in the order printed.
fn messages(output: &Output, ids: &[&str]) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(llg::core::report::parse)
        .filter(|message| ids.contains(&message.id.as_str()))
        .map(|message| {
            let location = message.location.unwrap_or_default();
            let (_, at) = location
                .rsplit_once(".sv:")
                .expect("a located frontend message");
            format!("{} {at} {}", message.severity.word(), message.text)
        })
        .collect()
}

/// `output` has exactly the elaboration task messages `expected`, exit status
/// `status` and standard output `stdout`.
fn check(label: &str, output: &Output, expected: &[&str], status: i32, stdout: &str) {
    assert_eq!(messages(output, &TASKS), expected, "{label}");
    assert_eq!(output.status.code(), Some(status), "{label}: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
}

const TASKS: [&str; 4] = [
    "SLANG-InfoTask",
    "SLANG-WarningTask",
    "SLANG-ErrorTask",
    "SLANG-FatalTask",
];

const INFO_WARNING: &[&str] = &[
    "INFO 6:3 $info encountered in tb.fi: N=5",
    "INFO 9:3 $info encountered in tb.c0: checker",
    "WARNING 12:3 $warning encountered in tb.p: program (-Wwarning-task)",
    "INFO 15:3 $info encountered in tb.u",
    "INFO 15:3 $info encountered in tb.v[0]",
    "INFO 15:3 $info encountered in tb.v[1]",
    "INFO 16:3 $info encountered in tb.u",
    "INFO 16:3 $info encountered in tb.v[0]",
    "INFO 16:3 $info encountered in tb.v[1]",
    "WARNING 17:3 $warning encountered in tb.u: W=8 tb.u (-Wwarning-task)",
    "WARNING 17:3 $warning encountered in tb.v[0]: W=4 tb.v[0] (-Wwarning-task)",
    "WARNING 17:3 $warning encountered in tb.v[1]: W=4 tb.v[1] (-Wwarning-task)",
    "INFO 19:5 $info encountered in tb.u.g8: g8",
    "INFO 21:5 $info encountered in tb.v[0].other: not 8",
    "INFO 21:5 $info encountered in tb.v[1].other: not 8",
    "INFO 24:5 $info encountered in tb.u.g[0]: I=0",
    "INFO 24:5 $info encountered in tb.u.g[1]: I=1",
    "INFO 24:5 $info encountered in tb.v[0].g[0]: I=0",
    "INFO 24:5 $info encountered in tb.v[0].g[1]: I=1",
    "INFO 24:5 $info encountered in tb.v[1].g[0]: I=0",
    "INFO 24:5 $info encountered in tb.v[1].g[1]: I=1",
    "WARNING 26:7 $warning encountered in tb.u.g[1].inner: last (-Wwarning-task)",
    "WARNING 26:7 $warning encountered in tb.v[0].g[1].inner: last (-Wwarning-task)",
    "WARNING 26:7 $warning encountered in tb.v[1].g[1].inner: last (-Wwarning-task)",
    "INFO 30:19 $info encountered in tb.u.c8: case 8",
    "INFO 31:27 $info encountered in tb.v[0].cdef: case default",
    "INFO 31:27 $info encountered in tb.v[1].cdef: case default",
    "INFO 36:3 $info encountered in tb.u.b: bound",
    "INFO 36:3 $info encountered in tb.v[0].b: bound",
    "INFO 36:3 $info encountered in tb.v[1].b: bound",
    "INFO 43:3 $info encountered in tb: real 1.5, string abc",
];

#[test]
fn info_and_warning_print_once_per_call_and_the_model_runs() {
    sim_cli::run_case_checked_matrix(SUITE, "info_warning", &[], &|label, output| {
        check(
            label,
            output,
            INFO_WARNING,
            0,
            "tb.u 8\ntb.v[0] 4\ntb.v[1] 4\nrun 1\n",
        );
    });
}

#[test]
fn werror_stops_on_warning_calls_and_never_on_info_calls() {
    let expected: Vec<String> = INFO_WARNING
        .iter()
        .map(|message| message.replace("WARNING", "ERROR"))
        .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
    sim_cli::run_case_checked_matrix(SUITE, "info_warning", &["-Werror"], &|label, output| {
        check(label, output, &expected, 1, "");
    });
    sim_cli::run_case_checked_matrix(SUITE, "info_only", &["-Werror"], &|label, output| {
        check(
            label,
            output,
            &["INFO 4:3 $info encountered in tb: hi"],
            0,
            "",
        );
    });
}

#[test]
fn generate_only_prints_the_messages_and_exits_zero() {
    let out = sim_harness::TempDir::new("elab-tasks-gen").expect("output directory");
    let dir = out.path().to_string_lossy().into_owned();
    let args = ["--gen-only", "--out-dir", &dir];
    sim_cli::run_case_checked_matrix(SUITE, "info_warning", &args, &|label, output| {
        assert_eq!(messages(output, &TASKS), INFO_WARNING, "{label}");
        assert_eq!(output.status.code(), Some(0), "{label}: {output:?}");
    });
}

#[test]
fn error_and_fatal_report_every_call_once_and_stop() {
    sim_cli::run_case_checked_matrix(SUITE, "neg_error", &[], &|label, output| {
        check(
            label,
            output,
            &[
                "ERROR 6:7 $error encountered in tb.u.g[0].bad: I=0 W=1",
                "ERROR 6:7 $error encountered in tb.v.g[0].bad: I=0 W=1",
            ],
            1,
            "",
        );
    });
    sim_cli::run_case_checked_matrix(SUITE, "neg_fatal", &[], &|label, output| {
        check(
            label,
            output,
            &[
                "ERROR 4:3 $fatal encountered in tb",
                "ERROR 5:3 $fatal encountered in tb",
                "ERROR 6:3 $fatal encountered in tb: W=12",
            ],
            1,
            "",
        );
    });
}

#[test]
fn only_elaboration_tasks_with_constant_arguments_are_items() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_display",
        "SLANG-ExpectedMember",
        Some("4:3"),
        "expected member",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_run_time_argument",
        "SLANG-ConstEvalNonConstVariable",
        Some("6:16"),
        "reference to non-constant variable 'v'",
    );
}

#[test]
fn lone_semicolons_are_items_without_effect() {
    sim_cli::run_case_checked_matrix(SUITE, "empty_items", &[], &|label, output| {
        let seen = messages(output, &["SLANG-EmptyMember"]);
        assert_eq!(seen.len(), 10, "{label}: {seen:?}");
        check(label, output, &[], 0, "generate 1\n");
    });
}
