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

/// The check of one run of `fixture`: success, `expected` standard output
/// and exactly the `expected_warnings` (`{path}` stands for the fixture's
/// absolute path, `{any}` for text that differs between platforms, such as
/// an operating system error).
fn warnings_check(
    fixture: &str,
    expected: &'static str,
    expected_warnings: &[&str],
) -> impl Fn(&str, &Output) {
    let path = source_path(fixture);
    let mut expected_warnings = expected_warnings
        .iter()
        .map(|line| line.replace("{path}", &path))
        .collect::<Vec<_>>();
    expected_warnings.sort();
    move |label: &str, output: &Output| {
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
        let actual = warnings(output);
        let matches = actual.len() == expected_warnings.len()
            && actual
                .iter()
                .zip(&expected_warnings)
                .all(|(actual, expected)| match expected.split_once("{any}") {
                    Some((before, after)) => {
                        actual.starts_with(before)
                            && actual.ends_with(after)
                            && actual.len() >= before.len() + after.len()
                    }
                    None => actual == expected,
                });
        assert!(
            matches,
            "{label}: warnings\n  actual: {actual:#?}\n  expected: {expected_warnings:#?}"
        );
    }
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
    let check = warnings_check(
        "checks.v",
        fixture!("checks.out"),
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
    let files = [("checks.sdf", fixture!("checks.sdf"))];
    sim_cli::run_case_checked_matrix_with_files(SUITE, "checks.v", &[], &files, &check);
}

/// With negative timing checks, annotated `$setuphold` limits settle and
/// solve the delayed signal delays again exactly as the same limits declared
/// in the source do (31.9.1), and data at the end of a window does not
/// violate.
#[test]
fn negative_setuphold_limits() {
    let check = warnings_check(
        "negative.v",
        fixture!("negative.out"),
        &[
            "timing violation at {path}:14:5 in tb.h at time 104000 (1ps): \
             $setuphold(posedge clk:100000, d:104000, -2000, 5000)",
            "timing violation at {path}:23:5 in tb.a at time 104000 (1ps): \
             $setuphold(posedge clk:100000, d:104000, -2000, 5000)",
        ],
    );
    let files = [("negative.sdf", fixture!("negative.sdf"))];
    sim_cli::run_case_checked_matrix_with_files(
        SUITE,
        "negative.v",
        &["--neg-timing-checks"],
        &files,
        &check,
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

/// Interconnect delays (32.4.4, Table 32-3): PORT, INTERCONNECT from one
/// source, two sources combined by each INTERCONNECT_MIPD policy, NETDELAY
/// to a port, to every load port of a net and to the load ports of one bit
/// of a vector net (SDF 2.1), and an INTERCONNECT whose source does not
/// exist, which applies from every source of the load with a warning.
#[test]
fn interconnect_delays() {
    let check = warnings_check(
        "interconnect.v",
        fixture!("interconnect.out"),
        &[
            "interconnect.sdf:9:5: the INTERCONNECT source was not found; the delay applies \
             from every source of the load (IEEE 1800-2009 32.4.4)",
            "interconnect.sdf:9:18: instance `nosuch` of the INTERCONNECT source `nosuch.y` \
             not found in `tb`",
        ],
    );
    let files = [
        ("interconnect.sdf", fixture!("interconnect.sdf")),
        ("netdelay.sdf", fixture!("netdelay.sdf")),
        ("ms.sdf", fixture!("ms.sdf")),
        ("min.cfg", fixture!("min.cfg")),
        ("average.cfg", fixture!("average.cfg")),
        ("last.cfg", fixture!("last.cfg")),
    ];
    sim_cli::run_case_checked_matrix_with_files(SUITE, "interconnect.v", &[], &files, &check);
}

/// `--sdf` files are annotated in command-line order before simulation
/// starts: the time-0 transitions use them, a scope and a min:typ:max
/// member may be named, and a later INCREMENT file adds to an earlier one.
#[test]
fn command_line_annotation() {
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "startup.v",
        fixture!("startup.out"),
        &[
            "--sdf",
            "startup.sdf",
            "--sdf",
            "max:tb.u1=cell.sdf",
            "--sdf",
            "min:tb.u2=cell.sdf",
        ],
        &[],
        &[
            ("startup.sdf", fixture!("startup.sdf")),
            ("cell.sdf", fixture!("cell.sdf")),
        ],
    );
}

/// Diagnostics (32.2, 32.9): a missing or malformed file annotates nothing
/// (the valid CELL before the syntax error included), invalid mtm_spec,
/// scale_factors and scale_type arguments and an unknown configuration
/// statement skip the call, and unmatched instances, cell types, paths,
/// checks and wildcard cell types, a missing triple member, an empty value,
/// an out-of-range value and a list of more than 12 delays are reported
/// while the rest of the file applies.
#[test]
fn diagnostics() {
    let call = "$sdf_annotate at {path}";
    let check = warnings_check(
        "diagnostics.v",
        fixture!("diagnostics.out"),
        &[
            &format!(
                "{call}:32:5: cannot open the SDF file `missing.sdf`: {{any}}; the call \
                 annotates nothing"
            ),
            &format!(
                "{call}:33:5: malformed.sdf:5:76: expected `)` after the IOPATH delays, found \
                 `nine`; the call annotates nothing"
            ),
            "unmatched.sdf:5:2: instance `nosuch` not found in `tb`; the CELL is not annotated",
            "unmatched.sdf:6:2: `tb.u0` is an instance of `BUF`, not cell type \"FF\"; the \
             CELL is not annotated",
            "unmatched.sdf:8:21: no specify path from `b` to `y` in `tb.u0`; the IOPATH is not \
             annotated",
            "unmatched.sdf:10:17: no timing check in `tb.f0` matches (HOLD d (posedge clk)); it \
             is not annotated",
            "unmatched.sdf:11:2: no instance of cell type \"NONE\" in `tb`; the CELL is not \
             annotated",
            "values.sdf:5:69: single values and min:typ:max triples are mixed in one file; a \
             single value applies to every data set",
            "values.sdf:7:83: IOPATH has more than 12 delay values; it is not annotated",
            "values.sdf:4:57: the value 1e999 is out of the simulation time range; it is \
             annotated as one precision unit",
            &format!(
                "{call}:36:5: the mtm_spec `NOMINAL` is not MAXIMUM, MINIMUM, TYPICAL or \
                 TOOL_CONTROL (IEEE 1800-2009 Table 32-5); the call annotates nothing"
            ),
            &format!(
                "{call}:37:5: the scale_factors `1:x:2` are not three non-negative numbers \
                 `min:typ:max` (IEEE 1800-2009 32.9); the call annotates nothing"
            ),
            &format!(
                "{call}:38:5: the scale_type `FROM_ELSEWHERE` is not FROM_MINIMUM, \
                 FROM_TYPICAL, FROM_MAXIMUM or FROM_MTM (IEEE 1800-2009 Table 32-6); the call \
                 annotates nothing"
            ),
            &format!(
                "{call}:39:5: configuration file `unknown.cfg`:2: unknown statement \
                 `PULSE_DELAY`; llg reads MTM, SCALE_FACTORS, SCALE_TYPE, INTERCONNECT_MIPD and \
                 TURNOFF_DELAY; the call annotates nothing"
            ),
            "timing violation at {path}:18:5 in tb.f0 at time 60000 (1ps): $setup(d:57000, \
             posedge clk:60000, 4000)",
        ],
    );
    let files = [
        ("malformed.sdf", fixture!("malformed.sdf")),
        ("unmatched.sdf", fixture!("unmatched.sdf")),
        ("values.sdf", fixture!("values.sdf")),
        ("override.sdf", fixture!("override.sdf")),
        ("unknown.cfg", fixture!("unknown.cfg")),
    ];
    sim_cli::run_case_checked_matrix_with_files(SUITE, "diagnostics.v", &[], &files, &check);
}
