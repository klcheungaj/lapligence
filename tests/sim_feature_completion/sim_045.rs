//! SIM-045: coverpoint bins, transitions, crosses and options (IEEE 1800-2009
//! §§19.5-19.7, 19.11). Every expected percentage is derived by hand in the
//! fixture readme; each positive case runs in both optimizer modes on the
//! legacy, compact/portable and compact/GMP value backends.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_045";

fn assert_stdout(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
}

/// The `llg:` lines of a run's stderr other than the `$finish` report.
fn llg_messages(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("llg: ") && !line.starts_with("llg: $finish"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn value_bins_follow_19_5_and_19_11_1() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/bins.out");
    sim_cli::run_case_checked_matrix(SUITE, "bins", &[], &|label, output| {
        assert_stdout(label, output, expected);
        // The second instance's arguments exceed the 4-bit coverpoint; the
        // ranges are resolved by 19.5.6 when it is constructed.
        let limited =
            "range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)";
        assert_eq!(
            llg_messages(output),
            [
                format!("llg: warning: tb:59:20: bins `good` {limited}"),
                format!("llg: warning: tb:60:24: bins `split` {limited}"),
            ],
            "{label}"
        );
    });
}

#[test]
fn illegal_values_are_run_time_errors_and_the_run_continues() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/illegal.out");
    sim_cli::run_case_checked_matrix(SUITE, "illegal", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(
            llg_messages(output),
            [
                "llg: runtime error: covergroup `g` coverpoint `cp` sampled 5, a value of illegal_bins `bad` at tb:11:20 (IEEE 1800-2009 19.5.5)",
                "llg: runtime error: covergroup `g` coverpoint `cp` sampled 12, a value of illegal_bins `rest` at tb:12:20 (IEEE 1800-2009 19.5.5)",
                "llg: runtime error: bins `b` at tb:16:29: the number of bins is not a positive value, so the declaration has no bins",
            ],
            "{label}"
        );
    });
}

#[test]
fn invalid_bins_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_ignore_default",
        "neg_ignore_default.sv:8:26 default coverage bins cannot be ignored",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_wide",
        "neg_wildcard_wide.sv:7:45: wildcard bins `odd` value matches more than 4096 separate value ranges",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_bins_count",
        "neg_bins_count.sv:6:31: the number of bins of `b` of coverpoint `cp` in covergroup `g` must be a positive value",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_bins_with",
        "neg_bins_with.sv:6:24 `covergroup bins with or matches clause` is not available in IEEE 2009",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_argument",
        "neg_wide_argument.sv:6:35: bins values that read covergroup arguments must be integral values of at most 64 bits",
    );
}
