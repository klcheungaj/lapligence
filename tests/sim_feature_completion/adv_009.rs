//! ADV-009: system timing checks (`$setup`, `$hold`, `$recovery`,
//! `$removal`, `$skew`, `$period`, `$width`, `$nochange`), `&&&`
//! conditions, edge-control specifiers and notifiers. Oracles are
//! hand-derived from IEEE 1364-2001 15 and IEEE 1800-2009 31, as quoted in
//! the fixture readme. Every positive case runs in both editions, both
//! optimizer modes and on every value backend.
use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_009";
const BOTH: &[&str] = &["v2001", "sv2009"];

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_009/",
            $name
        ))
    };
}

/// The absolute fixture path, as violation reports name the source: the resolved
/// native spelling, since a joined path keeps `SUITE`'s `/` on Windows.
fn source_path(fixture: &str) -> String {
    sim_harness::source_display(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(fixture),
    )
}

/// The `llg: warning: timing violation` lines of a run, in order.
fn violations(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: timing violation at "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn setup_hold_recovery_and_removal_windows() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "windows.v",
            fixture!("windows.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn skew_period_and_width_limits() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "skew_period_width.v",
            fixture!("skew_period_width.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn nochange_windows_and_offsets() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "nochange.v",
            fixture!("nochange.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn edge_control_x_z_transitions_and_vector_terminals() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "edges.v",
            fixture!("edges.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn conditions_enable_and_disable_events() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "conditions.v",
            fixture!("conditions.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn notifiers_toggle_and_feed_sequential_udps() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "notifiers.v",
            fixture!("notifiers.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// A violation reports the check's source, the instance, the time and both
/// event times on stderr, in the time step of the violation.
#[test]
fn violation_reports_name_source_scope_and_times() {
    let path = source_path("windows.v");
    sim_cli::run_case_checked_matrix(SUITE, "windows.v", &[], &|label, output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            sim_cli::observed_output(output)
        );
        let mut reports = violations(output);
        reports.sort();
        let at = |position: &str, rest: &str| format!("{path}:{position} in {rest}");
        let mut expected = vec![
            at(
                "5:45",
                "tb.s2 at time 100 (1ns): $setup(d:91, posedge c:100, 10)",
            ),
            at(
                "5:45",
                "tb.s3 at time 100 (1ns): $setup(d:99, posedge c:100, 10)",
            ),
            at(
                "6:45",
                "tb.h0 at time 100 (1ns): $hold(posedge c:100, d:100, 10)",
            ),
            at(
                "6:45",
                "tb.h1 at time 100 (1ns): $hold(posedge c:100, d:100, 10)",
            ),
            at(
                "6:45",
                "tb.h5 at time 100 (1ns): $hold(posedge c:100, d:100, 10)",
            ),
            at(
                "6:45",
                "tb.h6 at time 100 (1ns): $hold(posedge c:100, d:100, 10)",
            ),
            at(
                "6:45",
                "tb.h2 at time 101 (1ns): $hold(posedge c:100, d:101, 10)",
            ),
            at(
                "6:45",
                "tb.h3 at time 109 (1ns): $hold(posedge c:100, d:109, 10)",
            ),
            at(
                "7:45",
                "tb.r0 at time 100 (1ns): $recovery(posedge r:100, posedge c:100, 10)",
            ),
            at(
                "7:45",
                "tb.r1 at time 109 (1ns): $recovery(posedge r:100, posedge c:109, 10)",
            ),
            at(
                "8:45",
                "tb.m1 at time 101 (1ns): $removal(posedge r:101, posedge c:100, 10)",
            ),
            at(
                "8:45",
                "tb.m2 at time 109 (1ns): $removal(posedge r:109, posedge c:100, 10)",
            ),
        ];
        expected.sort();
        assert_eq!(reports, expected, "{label}");
    });
    let path = source_path("skew_period_width.v");
    sim_cli::run_case_checked_matrix(SUITE, "skew_period_width.v", &[], &|label, output| {
        let reports = violations(output);
        assert!(
            reports.contains(&format!(
                "{path}:7:36 in tb.w2 at time 103 (1ns): $width(posedge c:100, negedge c:103, 10, 2)"
            )),
            "{label}: {reports:?}"
        );
        assert!(
            reports.contains(&format!(
                "{path}:6:36 in tb.p2 at time 115 (1ns): $period(posedge c:111, posedge c:115, 10)"
            )),
            "{label}: {reports:?}"
        );
    });
}

/// Run-time controls (the `+notimingcheck`, `+no_notifier` and
/// `+no_tchk_msg` conventions): no checks at all, reports without notifier
/// changes, notifier changes without reports.
#[test]
fn invocation_options_turn_off_checks_notifiers_or_reports() {
    for (option, expected, reported) in [
        (
            "--no-timing-checks",
            fixture!("notifiers_unchanged.out"),
            false,
        ),
        ("--no-notifiers", fixture!("notifiers_unchanged.out"), true),
        (
            "--no-timing-check-messages",
            fixture!("notifiers.out"),
            false,
        ),
    ] {
        for edition in BOTH {
            sim_cli::run_case_backend_parity(
                SUITE,
                "notifiers.v",
                expected,
                &["--edition", edition, option],
                &[],
            );
        }
        sim_cli::run_case_checked_matrix(SUITE, "notifiers.v", &[option], &|label, output| {
            assert!(output.status.success(), "{label}");
            assert_eq!(
                !violations(output).is_empty(),
                reported,
                "{label}: {option}"
            );
        });
    }
}

/// The `[simulator]` keys give the same controls; the command line wins.
#[test]
fn llg_toml_sets_the_timing_check_controls() {
    let off = "schema_version = 1\n\n[simulator]\ntiming_checks = false\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "notifiers.v",
        fixture!("notifiers_unchanged.out"),
        &["--config", "llg.toml"],
        &[],
        &[("llg.toml", off)],
    );
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "notifiers.v",
        fixture!("notifiers.out"),
        &["--config", "llg.toml", "--timing-checks"],
        &[],
        &[("llg.toml", off)],
    );
    let quiet =
        "schema_version = 1\n\n[simulator]\nnotifiers = false\ntiming_check_messages = false\n";
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "notifiers.v",
        &["--config", "llg.toml"],
        &[("llg.toml", quiet)],
        &|label, output| {
            assert!(output.status.success(), "{label}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                fixture!("notifiers_unchanged.out"),
                "{label}"
            );
            assert!(violations(output).is_empty(), "{label}");
        },
    );
}

/// Of duplicate checks only the one with the larger limit is simulated
/// (decision A9-D6); the other is reported when the model is generated.
#[test]
fn duplicate_checks_keep_the_larger_limit_with_a_warning() {
    let path = source_path("notifiers.v");
    sim_cli::run_case_checked_matrix(SUITE, "notifiers.v", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for (dropped, kept, scope) in [("34:42", "34:11", "tb.k"), ("37:11", "37:40", "tb.r")] {
            let warning = format!(
                "llg: warning: duplicate timing check `$setup` at {path}:{dropped} in `{scope}` \
                 is disabled; the one at {path}:{kept} with the larger limit is simulated"
            );
            assert!(stderr.contains(&warning), "{label}: {warning}\n{stderr}");
        }
    });
}

/// The model runs with the owned database already dropped.
#[test]
fn nochange_runs_after_the_database_is_dropped() {
    let path = source_path("nochange.v");
    let first = format!(
        "timing violation at {path}:6:45 in tb.x1 at time 100 (1ns): \
         $nochange(posedge c:100, d:99, 2, 3)"
    );
    let second = format!(
        "timing violation at {path}:5:45 in tb.q2 at time 101 (1ns): \
         $nochange(posedge c:100, d:101, 0, 0)"
    );
    let third = format!(
        "timing violation at {path}:6:45 in tb.x2 at time 121 (1ns): \
         $nochange(posedge c:100, d:121, 2, 3)"
    );
    sim_cli::run_compile_opts_after_db_drop_with_warnings(
        SUITE,
        "nochange.v",
        llg::core::compile::CompileOpts {
            files: vec![path.clone()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        fixture!("nochange.out"),
        "llg: $finish at time 132 at tb:27:9\n",
        &[&first, &second, &third],
    );
}

/// Forms the simulator cannot run are rejected with their location before
/// any model runs: no check of the design is partially configured.
#[test]
fn invalid_checks_are_rejected_with_their_location() {
    for edition in BOTH {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_notifier_vector.v",
            "neg_notifier_vector.v:9:5: notifier `tb.u.n` must be a scalar variable (IEEE \
             1800-2009 31.6)",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_width_notifier.v",
            "neg_width_notifier.v:7:5: the threshold argument must be present when the notifier \
             is given",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_period_edges.v",
            "timing check `$period` at ",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_period_edges.v",
            "needs only rising or only falling transitions",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_signature.v",
            "neg_signature.v:5:5 too few arguments for '$setup'",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_limit.v",
            "reference to non-constant variable 'lim' is not allowed in a constant expression",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_width_edge.v",
            "'$width' event argument requires an edge trigger",
            &args,
        );
    }
}
