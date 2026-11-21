//! KI timescale option (decision F2-D3): `--timescale <unit>/<precision>`
//! and `compile.timescale` set the time scale of the design elements that
//! declare none (built-in default 1ns/1ps; IEEE 1800-2009 3.14.2.3 leaves it
//! to the tool). Delays, `$time`/`$realtime`, `%t`, the `$timeformat`
//! defaults, `$printtimescale`, the VPI time properties and the waveform
//! header all follow it, and the `missing-timescale` lint rule reports the
//! elements that take the built-in default. Every expected value is derived
//! by hand in the fixture readme. Positive cases run in both optimizer modes
//! and on every value backend.
use super::{c_compiler, sim_cli, sim_harness, vcd};
use std::path::Path;
use std::process::Output;

const SUITE: &str = "feature_completion/ki_timescale_option";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/ki_timescale_option/",
            $name
        ))
    };
}

/// A config file that sets the default time scale.
const CONFIG_10US: &str = "schema_version = 1\n\n[compile]\ntimescale = \"10us/1ns\"\n";

/// The `missing-timescale` findings of `stderr` as `file:line:col text`.
fn timescale_findings(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(llg::core::report::parse)
        .filter(|message| message.id == "LINT-026")
        .map(|message| {
            format!(
                "{} {}",
                message.severity.word(),
                sim_harness::located_text(&message)
            )
        })
        .collect()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Delays, `$time`, `$realtime`, `%t`, `$timeformat` and `$printtimescale`
/// of modules and a package without a time scale, under the built-in default
/// and under `--timescale` (also written with the blanks a `` `timescale``
/// directive allows) or `compile.timescale`.
#[test]
fn consumers_follow_the_default_time_scale() {
    sim_cli::run_case_backend_parity(SUITE, "consumers", fixture!("consumers.out"), &[], &[]);
    for value in ["10us/1ns", "10 us / 1 ns"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "consumers",
            fixture!("consumers_10us.out"),
            &["--timescale", value],
            &[],
        );
    }
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "consumers",
        fixture!("consumers_10us.out"),
        &["--config", "llg.toml"],
        &[],
        &[("llg.toml", CONFIG_10US)],
    );
    // The command line replaces the configured value.
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "consumers",
        fixture!("consumers.out"),
        &["--config", "llg.toml", "--timescale", "1ns/1ps"],
        &[],
        &[("llg.toml", CONFIG_10US)],
    );
}

/// A design whose elements all declare their time scale is unaffected by
/// `--timescale`; in a mixed design, which needs it (decision F2-D4), only
/// the elements without one change.
#[test]
fn declared_time_scales_win_over_the_default() {
    for args in [&[][..], &["--timescale", "10us/1ns"][..]] {
        sim_cli::run_case_backend_parity(SUITE, "declared", fixture!("declared.out"), args, &[]);
    }
    sim_cli::run_case_backend_parity(
        SUITE,
        "mixed",
        fixture!("mixed.out"),
        &["--timescale", "1ns/1ps"],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "mixed",
        fixture!("mixed_1us.out"),
        &["--timescale", "1us/1ns"],
        &[],
    );
}

/// Decision F2-D4: when only some design elements have a time scale
/// (IEEE 1800-2009 3.14.2.3), the run stops before lint with one located
/// error at the first element without one, naming an element with one; a
/// default time scale from `--timescale` or `compile.timescale` makes the
/// design legal without a `missing-timescale` warning.
#[test]
fn mixed_time_scales_need_a_default() {
    sim_cli::reject_case_with_id(
        SUITE,
        "mixed",
        "ELAB-006",
        Some("2:9"),
        "package `pkg` has no `timescale, timeunit or timeprecision, but module `tb` at ",
    );
    let (_directory, output) = sim_cli::run_case_keeping_dir(SUITE, "mixed", true, &[], &[]);
    let message = stderr(&output)
        .lines()
        .filter_map(llg::core::report::parse)
        .find(|message| message.id == "ELAB-006")
        .expect("ELAB-006 error");
    assert!(
        message.text.ends_with(
            "mixed.sv:11:8 has a time scale; a design in which only some elements have one \
             needs a default time scale (`--timescale`) (IEEE 1800-2009 3.14.2.3)"
        ),
        "{}",
        message.text
    );
    assert!(
        timescale_findings(&stderr(&output)).is_empty(),
        "lint did not run"
    );

    let (_directory, output) =
        sim_cli::run_case_keeping_dir(SUITE, "mixed", true, &["--timescale", "1ns/1ps"], &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), fixture!("mixed.out"));
    assert!(!stderr(&output).contains("ELAB-006"), "{}", stderr(&output));
    assert!(timescale_findings(&stderr(&output)).is_empty());

    let config = "schema_version = 1\n\n[compile]\ntimescale = \"1us/1ns\"\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "mixed",
        fixture!("mixed_1us.out"),
        &["--config", "llg.toml"],
        &[],
        &[("llg.toml", config)],
    );
}

/// The VCD `$timescale` is the global precision and change times count in
/// it: the default's 1ps, or the 1ns precision of `--timescale 10us/1ns`.
#[test]
fn waveform_header_and_times_follow_the_default_time_scale() {
    for (extra, timescale, change) in [
        (&[][..], "1ps", 1000),
        (&["--timescale", "10us/1ns"][..], "1ns", 10_000),
    ] {
        for optimized in [false, true] {
            let mut args = vec!["--wave", "dump.vcd"];
            args.extend_from_slice(extra);
            let label = format!("wave {extra:?}, optimized={optimized}");
            let (directory, output) =
                sim_cli::run_case_keeping_dir(SUITE, "wave", optimized, &args, &[]);
            assert!(output.status.success(), "{label}: {}", stderr(&output));
            let path = directory.path().join("dump.vcd");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{label}: read {}: {error}", path.display()));
            let vcd = vcd::parse(&text).unwrap_or_else(|error| panic!("{label}: {error}\n{text}"));
            assert_eq!(vcd.timescale.as_deref(), Some(timescale), "{label}");
            let rises: Vec<u64> = vcd
                .changes
                .iter()
                .filter(|change| change.value == "1")
                .map(|change| change.time)
                .collect();
            assert_eq!(rises, [change], "{label}\n{text}");
        }
    }
}

/// `vpiTimeUnit` and `vpiTimePrecision` of a module and of the simulation,
/// and the simulation time in ticks, from a VPI application.
#[test]
fn vpi_time_properties_follow_the_default_time_scale() {
    let directory = sim_harness::TempDir::new("ki-timescale-vpi").expect("temporary directory");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/ki_timescale_option/vpi_time.c");
    let archive = c_compiler::static_archive(&source, directory.path(), "vpi_time", &[])
        .unwrap_or_else(|error| panic!("vpi_time.c: {error}"));
    let archive = archive.to_string_lossy().into_owned();
    let base = [
        "--define-system-task",
        "task $ts_report()",
        "--dpi-lib",
        archive.as_str(),
    ];
    sim_cli::run_case_backend_parity(SUITE, "vpi_time", fixture!("vpi_time.out"), &base, &[]);
    let mut scaled = base.to_vec();
    scaled.extend(["--timescale", "10us/1ns"]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "vpi_time",
        fixture!("vpi_time_10us.out"),
        &scaled,
        &[],
    );
}

/// One `missing-timescale` warning per run, at the first element that takes
/// the built-in default, counting the others; none when every element
/// declares a time scale or when `--timescale`/`compile.timescale` chose the
/// default.
#[test]
fn missing_timescale_warns_once_at_the_first_element() {
    let cases: [(&str, &[&str], &[&str]); 5] = [
        (
            "consumers",
            &[],
            &[
                "WARNING consumers.sv:3:9 package `p` and 2 other design elements have no \
               `timescale, timeunit or timeprecision and use the default time scale 1ns/1ps \
               (missing-timescale)",
            ],
        ),
        ("declared", &[], &[]),
        ("consumers", &["--timescale", "1ns/1ps"], &[]),
        ("mixed", &["--timescale", "1us/1ns"], &[]),
        (
            "wave",
            &[],
            &[
                "WARNING wave.sv:2:8 module `tb` has no `timescale, timeunit or timeprecision and \
             uses the default time scale 1ns/1ps (missing-timescale)",
            ],
        ),
    ];
    for (fixture, args, expected) in cases {
        let (_directory, output) = sim_cli::run_case_keeping_dir(SUITE, fixture, true, args, &[]);
        let label = format!("{fixture} {args:?}");
        assert!(output.status.success(), "{label}: {}", stderr(&output));
        assert_eq!(timescale_findings(&stderr(&output)), expected, "{label}");
    }
}

/// `-Werror` reports the warning as an error, which stops the run before a
/// model is built; disabling the rule in the config removes it.
#[test]
fn missing_timescale_follows_the_lint_policy() {
    let (_directory, output) =
        sim_cli::run_case_keeping_dir(SUITE, "consumers", true, &["-Werror"], &[]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(
        timescale_findings(&stderr(&output)),
        [
            "ERROR consumers.sv:3:9 package `p` and 2 other design elements have no `timescale, \
             timeunit or timeprecision and use the default time scale 1ns/1ps \
             (missing-timescale)"
        ]
    );
    assert_eq!(stdout(&output), "", "no model ran");

    let (_directory, output) = sim_cli::run_case_keeping_dir(
        SUITE,
        "consumers",
        true,
        &["-Werror", "--timescale", "1ns/1ps"],
        &[],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), fixture!("consumers.out"));
    assert_eq!(timescale_findings(&stderr(&output)), Vec::<String>::new());

    let disabled = "schema_version = 1\n\n[lint.rules.missing-timescale]\nenabled = false\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "consumers",
        fixture!("consumers.out"),
        &["--config", "llg.toml", "-Werror"],
        &[],
        &[("llg.toml", disabled)],
    );
}

/// Malformed `--timescale` values and a precision coarser than the unit are
/// usage errors naming the option; a bad `compile.timescale` names its key.
#[test]
fn invalid_time_scales_are_located_errors() {
    for (value, text) in [
        (
            "1ns",
            "--timescale: invalid time scale `1ns` (expected <unit>/<precision>, each 1, 10 or \
             100 followed by s, ms, us, ns, ps or fs, e.g. 1ns/1ps)",
        ),
        (
            "2ns/1ps",
            "--timescale: invalid time scale `2ns/1ps` (expected <unit>/<precision>, each 1, 10 \
             or 100 followed by s, ms, us, ns, ps or fs, e.g. 1ns/1ps)",
        ),
        (
            "1ns/1xs",
            "--timescale: invalid time scale `1ns/1xs` (expected <unit>/<precision>, each 1, 10 \
             or 100 followed by s, ms, us, ns, ps or fs, e.g. 1ns/1ps)",
        ),
        (
            "1ps/1ns",
            "--timescale: invalid time scale `1ps/1ns`: the precision 1ns is coarser than the \
             unit 1ps",
        ),
    ] {
        let (_directory, output) =
            sim_cli::run_case_keeping_dir(SUITE, "wave", true, &["--timescale", value], &[]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{value}: {}",
            stderr(&output)
        );
        assert_eq!(
            stderr(&output),
            format!("ERROR: [CLI-003] {text}\n"),
            "{value}"
        );
    }
}
