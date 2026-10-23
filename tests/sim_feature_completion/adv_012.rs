//! ADV-012: SDF back-annotation (`$sdf_annotate`, `--sdf`; IEEE 1800-2009
//! 32, IEEE 1364-2001 16). The SDF files and cell library of the fixtures
//! were written for these tests; every expected timestamp, value and
//! notifier is derived by hand from the clause text quoted in the fixture
//! readme. Positive cases run in both optimizer modes and on every value
//! backend.
use super::sim_cli;
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_012";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_012/",
            $name
        ))
    };
}

/// The absolute fixture path, as reports name the source.
fn source_path(fixture: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(fixture)
        .to_string_lossy()
        .into_owned()
}

/// The `llg: warning:` lines of a run, sorted (reports at one time come in
/// no defined order).
fn warnings(output: &Output) -> Vec<String> {
    let mut lines = String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    lines.sort();
    lines
}

/// Run `fixture` with input `files` on every backend and optimizer mode and
/// require `expected` standard output and exactly the `expected_warnings`
/// (`{path}` stands for the fixture's absolute path).
fn run_with_warnings(
    fixture: &str,
    expected: &str,
    args: &[&str],
    files: &[(&str, &str)],
    expected_warnings: &[&str],
) {
    let path = source_path(fixture);
    let mut expected_warnings = expected_warnings
        .iter()
        .map(|line| line.replace("{path}", &path))
        .collect::<Vec<_>>();
    expected_warnings.sort();
    sim_cli::run_case_checked_matrix_with_files(SUITE, fixture, args, files, &|label, output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{label}: output"
        );
        assert_eq!(warnings(output), expected_warnings, "{label}: warnings");
    });
}

/// IOPATH delays, delval pulse limits, PATHPULSE and PATHPULSEPERCENT
/// (32.7), and INCREMENT annotations applied while simulating: an event
/// already scheduled keeps its time, later events use the new delays, and
/// an incremented delay stops at 0.
#[test]
fn path_delays_and_pulse_limits() {
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "paths.v",
        fixture!("paths.out"),
        &[],
        &[],
        &[
            ("paths.sdf", fixture!("paths.sdf")),
            ("inc.sdf", fixture!("inc.sdf")),
        ],
    );
}

/// Annotated limits of every check family (Table 32-2): each check reports
/// against its new limit, and the report names that limit.
#[test]
fn timing_check_limits() {
    run_with_warnings(
        "checks.v",
        fixture!("checks.out"),
        &[],
        &[("checks.sdf", fixture!("checks.sdf"))],
        &[
            "timing violation at {path}:12:5 in tb.su at time 103000 (1ps): \
             $setup(d:100000, posedge clk:103000, 6000)",
            "timing violation at {path}:19:5 in tb.ho at time 204000 (1ps): \
             $hold(posedge clk:200000, d:204000, 6000)",
            "timing violation at {path}:26:5 in tb.sh at time 303000 (1ps): \
             $setup(d:300000, posedge clk:303000, 6000)",
            "timing violation at {path}:26:5 in tb.sh at time 307000 (1ps): \
             $hold(posedge clk:303000, d:307000, 6000)",
            "timing violation at {path}:33:5 in tb.rc at time 405000 (1ps): \
             $recovery(posedge rst:400000, posedge clk:405000, 7000)",
            "timing violation at {path}:40:5 in tb.rm at time 505000 (1ps): \
             $removal(posedge rst:505000, posedge clk:500000, 8000)",
            "timing violation at {path}:47:5 in tb.rr at time 605000 (1ps): \
             $recovery(posedge rst:600000, posedge clk:605000, 7000)",
            "timing violation at {path}:47:5 in tb.rr at time 645000 (1ps): \
             $removal(posedge rst:645000, posedge clk:640000, 8000)",
            "timing violation at {path}:54:5 in tb.sk at time 724000 (1ps): \
             $skew(posedge clk:720000, posedge d:724000, 3000)",
            "timing violation at {path}:61:5 in tb.wi at time 807000 (1ps): \
             $width(posedge clk:800000, negedge clk:807000, 12000, 0)",
            "timing violation at {path}:68:5 in tb.pe at time 930000 (1ps): \
             $period(posedge clk:900000, posedge clk:930000, 40000)",
            "timing violation at {path}:75:5 in tb.nc at time 1012000 (1ps): \
             $nochange(posedge clk:1000000, d:1012000, 2000, 3000)",
        ],
    );
}

/// With negative timing checks, annotated `$setuphold` limits settle and
/// solve the delayed signal delays again exactly as the same limits declared
/// in the source do (31.9.1), and a window too small to report stays silent.
#[test]
fn negative_setuphold_limits() {
    run_with_warnings(
        "negative.v",
        fixture!("negative.out"),
        &["--neg-timing-checks"],
        &[("negative.sdf", fixture!("negative.sdf"))],
        &[
            "timing violation at {path}:14:5 in tb.h at time 104000 (1ps): \
             $setuphold(posedge clk:100000, d:104000, -2000, 5000)",
            "timing violation at {path}:23:5 in tb.a at time 104000 (1ps): \
             $setuphold(posedge clk:100000, d:104000, -2000, 5000)",
        ],
    );
}

/// The member each call selects (mtm_spec, TOOL_CONTROL following
/// `--delay-select`), scale factors and scale types (Tables 32-5 and 32-6),
/// a configuration file and the call arguments that override it, a missing
/// triple member, and the log file both calls append to.
#[test]
fn mtm_selection_scaling_and_log() {
    let files = [
        ("mtm.sdf", fixture!("mtm.sdf")),
        ("partial.sdf", fixture!("partial.sdf")),
        ("mtm.cfg", fixture!("mtm.cfg")),
    ];
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "mtm.v",
        fixture!("mtm.out"),
        &[],
        &[],
        &files,
    );
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "mtm.v",
        fixture!("mtm_min.out"),
        &["--delay-select", "min"],
        &[],
        &files,
    );
}

/// DEVICE entries (Table 32-1): primitives driving the outputs of a cell
/// without paths, a UDP, three-state gates with one, two and three delays
/// and the TURNOFF_DELAY configuration, every path of a cell, the paths to
/// one output, one named primitive, and an INCREMENT on a gate applied while
/// simulating that stops at 0.
#[test]
fn device_delays() {
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "device.v",
        fixture!("device.out"),
        &[],
        &[],
        &[
            ("device.sdf", fixture!("device.sdf")),
            ("tri.sdf", fixture!("tri.sdf")),
            ("dinc.sdf", fixture!("dinc.sdf")),
            ("turnoff.cfg", fixture!("turnoff.cfg")),
        ],
    );
}
