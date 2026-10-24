//! ADV-010: combined and negative timing checks (`$setuphold`, `$recrem`,
//! delayed signals and the negative timing check option), `$timeskew` and
//! `$fullskew` with their flags, and negative `$nochange` offsets. Oracles
//! are hand-derived from IEEE 1364-2001 15 and IEEE 1800-2009 31, as quoted
//! in the fixture readme. Every positive case runs in both editions, both
//! optimizer modes and on every value backend.
use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_010";
const BOTH: &[&str] = &["v2001", "sv2009"];
const NEGATIVE: &str = "--neg-timing-checks";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_010/",
            $name
        ))
    };
}

/// The absolute fixture path, as reports name the source: the resolved
/// native spelling, since a joined path keeps `SUITE`'s `/` on Windows.
fn source_path(fixture: &str) -> String {
    sim_harness::source_display(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(fixture),
    )
}

/// The `llg: warning: timing violation` lines of a run, sorted.
fn violations(output: &Output) -> Vec<String> {
    let mut lines = String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: timing violation at "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    lines.sort();
    lines
}

/// The other `llg: warning:` lines of a run (model generation warnings).
fn generation_warnings(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: "))
        .filter(|line| !line.starts_with("timing violation at "))
        .map(str::to_owned)
        .collect()
}

/// Non-negative `$setuphold` and `$recrem` limits are `$setup`/`$hold` and
/// `$removal`/`$recovery` pairs, with or without negative timing checks.
#[test]
fn combined_checks_with_non_negative_limits() {
    for edition in BOTH {
        for options in [
            &["--edition", edition][..],
            &["--edition", edition, NEGATIVE],
        ] {
            sim_cli::run_case_backend_parity(
                SUITE,
                "combined.v",
                fixture!("combined.out"),
                options,
                &[],
            );
        }
    }
}

/// Negative limits delay the reference and data signals and shift the
/// windows (31.9); without the option they count as 0 and the delayed
/// signals are copies (31.9.4).
#[test]
fn negative_limits_delay_the_checked_signals() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "negative.v",
            fixture!("negative.out"),
            &["--edition", edition, NEGATIVE],
            &[],
        );
        sim_cli::run_case_backend_parity(
            SUITE,
            "negative.v",
            fixture!("negative_off.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// With timing checks off the explicit delayed signals stay copies of their
/// signals (31.9.4) and no notifier changes.
#[test]
fn delayed_signals_are_copies_without_timing_checks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "negative.v",
        fixture!("negative_disabled.out"),
        &[NEGATIVE, "--no-timing-checks"],
        &[],
    );
}

/// The `[simulator]` key enables negative timing checks; the command line
/// wins.
#[test]
fn llg_toml_enables_negative_timing_checks() {
    let on = "schema_version = 1\n\n[simulator]\nneg_timing_checks = true\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "negative.v",
        fixture!("negative.out"),
        &["--config", "llg.toml"],
        &[],
        &[("llg.toml", on)],
    );
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "negative.v",
        fixture!("negative_off.out"),
        &["--config", "llg.toml", "--no-neg-timing-checks"],
        &[],
        &[("llg.toml", on)],
    );
}

/// Timestamp and timecheck conditions gate the stamping and the checking
/// event of each part (31.9.2) and are read at the delayed events.
#[test]
fn timestamp_and_timecheck_conditions() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "conditions.v",
            fixture!("conditions.out"),
            &["--edition", edition, NEGATIVE],
            &[],
        );
        sim_cli::run_case_backend_parity(
            SUITE,
            "conditions.v",
            fixture!("conditions_off.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// `$timeskew` and `$fullskew` in timer-based and event-based mode, with and
/// without remain_active, including the window boundaries and windows a new
/// timestamp replaces (no stale expiry reports).
#[test]
fn timeskew_and_fullskew_flags() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "skew.v",
            fixture!("skew.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// Negative `$nochange` offsets shrink the window whatever the option; a
/// window that can end before its delayed trailing edge is decided there.
#[test]
fn nochange_negative_offsets_and_delayed_reference() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "nochange.v",
            fixture!("nochange.out"),
            &["--edition", edition, NEGATIVE],
            &[],
        );
        sim_cli::run_case_backend_parity(
            SUITE,
            "nochange.v",
            fixture!("nochange_off.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// Inconsistent negative limits are set to 0 from the smallest one with a
/// warning until the delays have a solution (31.9.1).
#[test]
fn inconsistent_negative_limits_are_zeroed_with_a_warning() {
    let path = source_path("inconsistent.v");
    for options in [&[NEGATIVE][..], &[]] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "inconsistent.v",
            fixture!("inconsistent.out"),
            options,
            &[],
        );
    }
    sim_cli::run_case_checked_matrix(SUITE, "inconsistent.v", &[NEGATIVE], &|label, output| {
        let warning = |limit: i32, line: u32| {
            format!(
                "the negative timing check limits of a module have no consistent delayed signal \
                 delays; the limit {limit} of `$setuphold` at {path}:{line}:5 counts as 0 (IEEE \
                 1800-2009 31.9.1)"
            )
        };
        assert_eq!(
            generation_warnings(output),
            vec![warning(-6, 15), warning(-5, 14)],
            "{label}"
        );
        assert_eq!(
            violations(output),
            vec![format!(
                "{path}:14:5 in tb.u at time 106 (1ns): $hold(posedge A:100, B:106, 10)"
            )],
            "{label}"
        );
    });
}

/// Reports name the component check while the limits are non-negative and
/// the combined check with its limits once one is negative, with the
/// undelayed event times; timer-based skew reports give the expiry time.
#[test]
fn violation_reports_name_the_check_and_undelayed_times() {
    let path = source_path("combined.v");
    sim_cli::run_case_checked_matrix(SUITE, "combined.v", &[], &|label, output| {
        let reports = violations(output);
        for expected in [
            "32:5 in tb.a2 at time 100 (1ns): $setup(d:91, posedge r:100, 10)",
            "32:5 in tb.a6 at time 101 (1ns): $hold(posedge r:100, d:101, 5)",
            "59:5 in tb.e2 at time 100 (1ns): $removal(posedge r:100, posedge d:96, 5)",
            "59:5 in tb.e7 at time 109 (1ns): $recovery(posedge r:100, posedge d:109, 10)",
        ] {
            let expected = format!("{path}:{expected}");
            assert!(
                reports.contains(&expected),
                "{label}: {expected}\n{reports:?}"
            );
        }
        assert_eq!(reports.len(), 18, "{label}: {reports:?}");
    });
    let path = source_path("negative.v");
    sim_cli::run_case_checked_matrix(SUITE, "negative.v", &[NEGATIVE], &|label, output| {
        let reports = violations(output);
        for expected in [
            "34:5 in tb.a2 at time 104 (1ns): $setuphold(posedge r:100, d:104, -3, 8)",
            "44:5 in tb.b2 at time 100 (1ns): $setuphold(posedge r:100, d:93, 8, -3)",
            "63:5 in tb.e2 at time 107 (1ns): $recrem(posedge r:100, posedge d:107, 8, -3)",
            "72:5 in tb.f1 at time 103 (1ns): $hold(posedge r:100, d:103, 4)",
        ] {
            let expected = format!("{path}:{expected}");
            assert!(
                reports.contains(&expected),
                "{label}: {expected}\n{reports:?}"
            );
        }
    });
    let path = source_path("skew.v");
    sim_cli::run_case_checked_matrix(SUITE, "skew.v", &[], &|label, output| {
        let reports = violations(output);
        for expected in [
            "11:5 in tb.i1a at time 150 (1ns): $timeskew(posedge CP &&& ...:100, negedge \
             CPN:150, 50)",
            "47:5 in tb.f1a at time 270 (1ns): $fullskew(posedge CP &&& ...:270, negedge \
             CPN:200, 50, 70)",
            "65:5 in tb.f1c at time 300 (1ns): $fullskew(posedge CP &&& ...:300, negedge \
             CPN:200, 50, 70)",
        ] {
            let expected = format!("{path}:{expected}");
            assert!(
                reports.contains(&expected),
                "{label}: {expected}\n{reports:?}"
            );
        }
    });
    let path = source_path("nochange.v");
    sim_cli::run_case_checked_matrix(SUITE, "nochange.v", &[NEGATIVE], &|label, output| {
        let reports = violations(output);
        for expected in [
            "15:5 in tb.a0 at time 120 (1ns): $nochange(negedge c:100, d:105, -2, -3)",
            "15:5 in tb.a0 at time 120 (1ns): $nochange(negedge c:100, d:110, -2, -3)",
            "24:5 in tb.b1 at time 120 (1ns): $nochange(negedge c:100, d:116, 2, -3)",
            "33:5 in tb.e2 at time 122 (1ns): $nochange(negedge c:100, d:122, -2, 3)",
            "44:5 in tb.g0 at time 226 (1ns): $nochange(posedge C:200, E:203, 0, 0)",
        ] {
            let expected = format!("{path}:{expected}");
            assert!(
                reports.contains(&expected),
                "{label}: {expected}\n{reports:?}"
            );
        }
    });
}

/// Negative limits are warned about once per check: without the option
/// (they count as 0), with a negative sum (set to 0), with a sum of at
/// most one precision unit (no violation possible) and when the delays of
/// the delayed signals adjust another check's limit to 0 or less.
#[test]
fn negative_limit_settlements_are_warned_once_per_check() {
    let path = source_path("negative.v");
    sim_cli::run_case_checked_matrix(SUITE, "negative.v", &[], &|label, output| {
        let warnings = generation_warnings(output);
        let off = |line: u32| {
            format!(
                "`$setuphold` at {path}:{line}:5: negative limits count as 0 unless negative \
                 timing checks are enabled (--neg-timing-checks; IEEE 1800-2009 31.9.4)"
            )
        };
        assert!(warnings.contains(&off(34)), "{label}: {warnings:?}");
        assert_eq!(
            warnings.iter().filter(|line| **line == off(34)).count(),
            1,
            "{label}: {warnings:?}"
        );
    });
    sim_cli::run_case_checked_matrix(SUITE, "negative.v", &[NEGATIVE], &|label, output| {
        assert_eq!(
            generation_warnings(output),
            vec![
                format!(
                    "`$setuphold` at {path}:72:5: the sum of its limits is negative, so its \
                     negative limits count as 0 (IEEE 1800-2009 31.3.3)"
                ),
                format!(
                    "`$setuphold` at {path}:81:5: the sum of its limits is not above one \
                     precision unit, so it cannot report violations (IEEE 1800-2009 31.3.3, \
                     31.9.1)"
                ),
            ],
            "{label}"
        );
    });
    let path = source_path("adjusted_limit.v");
    sim_cli::run_case_checked_matrix(SUITE, "adjusted_limit.v", &[NEGATIVE], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "hold 0@0\n",
            "{label}"
        );
        assert_eq!(
            generation_warnings(output),
            vec![format!(
                "`$hold` at {path}:14:5: its limit 2 adjusted by the delays of its delayed \
                 signals is -2, so it counts as 0 (IEEE 1800-2009 31.9.1)"
            )],
            "{label}"
        );
    });
}

/// Delayed signal and flag configurations the checks cannot use are
/// rejected with their location before any model runs.
#[test]
fn invalid_configurations_are_rejected_with_their_location() {
    for edition in BOTH {
        for option in [NEGATIVE, "--no-neg-timing-checks"] {
            let args = ["--edition", edition, option];
            sim_cli::reject_case_with_args(
                SUITE,
                "neg_delayed_width.v",
                "neg_delayed_width.v:8:5 in `tb.u`: delayed signal `tb.u.dd` has 2 bits but its \
                 signal has 1 (IEEE 1800-2009 31.9.1)",
                &args,
            );
            sim_cli::reject_case_with_args(
                SUITE,
                "neg_delayed_self.v",
                "neg_delayed_self.v:7:5 in `tb.u`: delayed signal `tb.u.c` cannot be its own \
                 signal",
                &args,
            );
            sim_cli::reject_case_with_args(
                SUITE,
                "neg_delayed_two_names.v",
                "neg_delayed_two_names.v:9:5 in `tb.u`: one signal has two delayed signals, \
                 `tb.u.dc1` and `tb.u.dc2`",
                &args,
            );
            sim_cli::reject_case_with_args(
                SUITE,
                "neg_delayed_shared.v",
                "neg_delayed_shared.v:8:5 in `tb.u`: delayed signal `tb.u.dd` is named for two \
                 different signals",
                &args,
            );
            sim_cli::reject_case_with_args(
                SUITE,
                "neg_skew_flag.v",
                "neg_skew_flag.v:6:5: an event_based or remain_active flag must be a known value",
                &args,
            );
        }
    }
}
