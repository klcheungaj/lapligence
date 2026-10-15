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

#[test]
fn transition_bins_follow_19_5_1() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/transitions.out");
    sim_cli::run_case_checked_matrix(SUITE, "transitions", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(llg_messages(output), Vec::<String>::new(), "{label}");
    });
}

#[test]
fn illegal_transitions_are_run_time_errors() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_045/transition_illegal.out");
    let rest = "llg: runtime error: covergroup `g` coverpoint `cp` sampled 5, ending a transition no other bin holds, which is in illegal_bins `rest` at tb:13:20 (IEEE 1800-2009 19.5.5)";
    let nine = rest.replace("sampled 5", "sampled 9");
    sim_cli::run_case_checked_matrix(SUITE, "transition_illegal", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(
            llg_messages(output),
            [
                "llg: runtime error: covergroup `g` coverpoint `cp` sampled 3, completing a transition of illegal_bins `bad` at tb:12:20 (IEEE 1800-2009 19.5.5)",
                rest,
                nine.as_str(),
                nine.as_str(),
            ],
            "{label}"
        );
    });
}

#[test]
fn invalid_transition_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_trans_length0",
        "neg_trans_length0.sv:6:29: transition bins `t` of coverpoint `cp` in covergroup `g` specify a transition of length 0: a single range list whose repetition can be 1 (IEEE 1800-2009 19.5.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_array_goto",
        "neg_trans_array_goto.sv:6:29: transition bins `t` of coverpoint `cp` in covergroup `g` with `[]` cannot use goto or nonconsecutive repetition",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_ignore_goto",
        "neg_trans_ignore_goto.sv:6:55: ignore_bins and illegal_bins `skip` of coverpoint `cp` in covergroup `g` cannot use goto or nonconsecutive repetition",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_argument",
        "neg_trans_argument.sv:6:39: transition bins `t` of coverpoint `cp` in covergroup `g` read a covergroup argument; llg supports only constant transition values (llg limitation)",
    );
}

#[test]
fn crosses_follow_19_6_and_19_11_2() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/crosses.out");
    sim_cli::run_case_checked_matrix(SUITE, "crosses", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(llg_messages(output), Vec::<String>::new(), "{label}");
    });
}

#[test]
fn illegal_cross_products_are_run_time_errors() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/cross_illegal.out");
    sim_cli::run_case_checked_matrix(SUITE, "cross_illegal", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(
            llg_messages(output),
            ["llg: runtime error: covergroup `zz` cross `xy` sampled a product of illegal_bins `illegal` at tb:14:20 (IEEE 1800-2009 19.6.3)"],
            "{label}"
        );
    });
}

#[test]
fn invalid_crosses_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_cross_of_cross",
        "neg_cross_of_cross.sv:9:5: cross `abc` in covergroup `g` crosses another cross, which IEEE 1800-2009 does not define (19.6)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_cross_transition",
        "neg_cross_transition.sv:8:5: cross `ab` in covergroup `g` crosses coverpoint `ca`, which has transition bins; llg crosses only value bins (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_cross_set",
        "neg_cross_set.sv:8:24 `covergroup bins with or matches clause` is not available in IEEE 2009",
    );
}

#[test]
fn options_follow_19_7_and_19_11() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/options.out");
    sim_cli::run_case_checked_matrix(SUITE, "options", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(
            llg_messages(output),
            [
                "llg: warning: covergroup `go` coverpoint `cx`: bins `lo` and `mid` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
                "llg: warning: covergroup `go` coverpoint `tx`: bins `t1` and `t2` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
            ],
            "{label}"
        );
    });
}

#[test]
fn merged_crosses_need_the_same_cross_bins() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/merge_cross.out");
    sim_cli::run_case_checked_matrix(SUITE, "merge_cross", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(
            llg_messages(output),
            ["llg: runtime error: covergroup `g` cross `ab`: merged type coverage needs every instance to have the same cross bins (llg limitation)"],
            "{label}"
        );
    });
}

#[test]
fn invalid_options_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_type_weight",
        "neg_type_weight.sv:6:24: type_option.weight in covergroup `g` must be a non-negative integral value (IEEE 1800-2009 Table 19-3)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_merge_argument",
        "neg_merge_argument.sv:6:35 cannot refer to automatic variable 'm' from static initializer",
    );
}
