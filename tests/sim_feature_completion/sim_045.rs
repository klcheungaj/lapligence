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

/// The listed warnings of a run's stderr, from the line on.
fn lowering_warnings(output: &Output) -> Vec<String> {
    super::sim_harness::listed_warnings(&String::from_utf8_lossy(&output.stderr))
        .iter()
        .map(|warning| {
            warning
                .split_once(".sv:")
                .map_or(warning.as_str(), |(_, rest)| rest)
                .to_owned()
        })
        .collect()
}

/// The run-time messages of a run's stderr ([`super::sim_harness::runtime_messages`]).
fn llg_messages(output: &Output) -> Vec<String> {
    super::sim_harness::runtime_messages(&String::from_utf8_lossy(&output.stderr))
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
                format!("WARNING: [SIM-011] tb:59:20: bins `good` {limited}"),
                format!("WARNING: [SIM-011] tb:60:24: bins `split` {limited}"),
            ],
            "{label}"
        );
    });
}

/// The run ended with exit status 1 after exactly `expected` on stdout and
/// the given llg messages on stderr.
fn ends_run(label: &str, output: &Output, expected: &str, messages: &[&str]) {
    assert_eq!(output.status.code(), Some(1), "{label}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
    assert_eq!(llg_messages(output), messages, "{label}");
}

/// An illegal value ends the run; the final procedure runs (S45-D3).
#[test]
fn illegal_values_end_the_run() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/illegal.out");
    sim_cli::run_case_checked_matrix(SUITE, "illegal", &[], &|label, output| {
        ends_run(
            label,
            output,
            expected,
            &[
                "ERROR: [SIM-010] covergroup `g` coverpoint `cp` sampled 5, a value of illegal_bins `bad` at tb:12:20 (IEEE 1800-2009 19.5.5)",
                "FATAL: [SIM-009] (tb:12:20 @ 0 ps) illegal_bins value sampled",
            ],
        );
    });
}

/// A value of an illegal default bin ends the run (S45-D3).
#[test]
fn illegal_default_values_end_the_run() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/illegal_default.out");
    sim_cli::run_case_checked_matrix(SUITE, "illegal_default", &[], &|label, output| {
        ends_run(
            label,
            output,
            expected,
            &[
                "ERROR: [SIM-010] covergroup `g` coverpoint `cp` sampled 12, a value of illegal_bins `rest` at tb:10:20 (IEEE 1800-2009 19.5.5)",
                "FATAL: [SIM-009] (tb:10:20 @ 0 ps) illegal_bins value sampled",
            ],
        );
    });
}

/// A bins array size argument that is not positive ends the run when the
/// instance is constructed (S45-D5).
#[test]
fn bins_counts_that_are_not_positive_end_the_run() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_045/bins_count_runtime.out");
    sim_cli::run_case_checked_matrix(SUITE, "bins_count_runtime", &[], &|label, output| {
        ends_run(
            label,
            output,
            expected,
            &[
                "ERROR: [SIM-010] bins `b` at tb:9:29: the number of bins is not a positive value",
                "FATAL: [SIM-009] (tb:9:29 @ 0 ps) the number of bins is not a positive value",
            ],
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
    let nine = "ERROR: [SIM-010] covergroup `g` coverpoint `cp` sampled 9, ending a transition no other bin holds, which is in illegal_bins `rest` at tb:15:20 (IEEE 1800-2009 19.5.5)";
    let one = nine.replace("sampled 9", "sampled 1");
    sim_cli::run_case_checked_matrix(SUITE, "transition_illegal", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert_eq!(
            llg_messages(output),
            [
                nine,
                nine,
                one.as_str(),
                "ERROR: [SIM-010] covergroup `g` coverpoint `cp` sampled 3, completing a transition of illegal_bins `bad` at tb:14:20 (IEEE 1800-2009 19.5.5)",
                "FATAL: [SIM-009] (tb:14:20 @ 0 ps) illegal_bins transition sampled",
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
        "neg_trans_length0.sv:7:29: transition bins `t` of coverpoint `cp` in covergroup `g` specify a transition of length 0: a single range list, alone or with a consecutive repetition that evaluates to 1 (IEEE 1800-2009 19.5.1)",
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
}

/// Transition values read from covergroup arguments resolve per instance
/// when it is constructed (19.5, 19.5.6), as value bins do.
#[test]
fn transition_values_read_covergroup_arguments() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_045/transition_arguments.out");
    let ignored = "value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)";
    let limited = "range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)";
    sim_cli::run_case_checked_matrix(SUITE, "transition_arguments", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(
            llg_messages(output),
            [
                format!("WARNING: [SIM-011] tb:13:39: bins `out` {ignored}"),
                format!("WARNING: [SIM-011] tb:14:41: bins `part` {limited}"),
                format!("WARNING: [SIM-011] tb:13:39: bins `out` {ignored}"),
                format!("WARNING: [SIM-011] tb:14:41: bins `part` {limited}"),
                format!("WARNING: [SIM-011] tb:21:53: bins `o` {ignored}"),
            ],
            "{label}"
        );
    });
}

/// The transition forms whose counts need compile-time searches keep
/// constant values (llg limitations).
#[test]
fn transition_argument_limits_are_located_errors() {
    let describe = "transition bins `t` of coverpoint `cp` in covergroup `g`";
    sim_cli::reject_case(
        SUITE,
        "neg_trans_argument_array",
        &format!("neg_trans_argument_array.sv:7:29: {describe} with `[]` read a covergroup argument; llg creates one bin per sequence only for constant transition values (llg limitation)"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_argument_removal",
        &format!("neg_trans_argument_removal.sv:7:29: {describe} read a covergroup argument in a coverpoint with ignore_bins or illegal_bins transitions; llg supports this only for constant transition values (llg limitation)"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_argument_overlap",
        &format!("neg_trans_argument_overlap.sv:8:29: {describe} read a covergroup argument in a coverpoint whose option.detect_overlap may be set; llg supports this only for constant transition values (llg limitation)"),
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
            [
                "ERROR: [SIM-010] covergroup `zz` cross `xy` sampled a product of illegal_bins `illegal` at tb:14:20 (IEEE 1800-2009 19.6.3)",
                "FATAL: [SIM-009] (tb:14:20 @ 0 ps) illegal_bins product sampled",
            ],
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
                "WARNING: [SIM-011] covergroup `go` coverpoint `cx`: bins `lo` and `mid` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
                "WARNING: [SIM-011] covergroup `go` coverpoint `tx`: bins `t1` and `t2` overlap (option.detect_overlap, IEEE 1800-2009 19.7)",
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
            ["ERROR: [SIM-010] covergroup `g` cross `ab`: merged type coverage needs every instance to have the same cross bins (llg limitation)"],
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

/// The compile-time budgets of transition bins (llg limitations): automaton
/// states, `[* m:n]` expansion of `[]` bins and of removal bins, start
/// entries and the value class sequences that count `[]` bins.
#[test]
fn transition_budgets_are_located_errors() {
    let describe = "transition bins `t` of coverpoint `cp`";
    sim_cli::reject_case(
        SUITE,
        "neg_trans_states",
        &format!("neg_trans_states.sv:7:29: {describe} needs more than 65536 transition states in its coverpoint (llg limitation)"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_sequences",
        &format!("neg_trans_sequences.sv:7:29: {describe} expands to more than 4096 transition sequences (llg limitation)"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_ignore_sequences",
        "neg_trans_ignore_sequences.sv:7:56: transition bins `skip` of coverpoint `cp` expands to more than 4096 transition sequences (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_starts",
        &format!("neg_trans_starts.sv:83:12: {describe} needs more than 1048576 transition start entries in its coverpoint (llg limitation)"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_trans_class_sequences",
        &format!("neg_trans_class_sequences.sv:83:12: {describe} needs more than 1048576 value class sequences to count its sequences (llg limitation)"),
    );
}

/// A cross may name only bins of the coverpoints it crosses, so a `binsof`
/// of a transition bin meets the rejection of the cross first.
#[test]
fn binsof_a_transition_bin_is_a_located_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_binsof_transition",
        "neg_binsof_transition.sv:10:5: cross `ab` in covergroup `g` crosses coverpoint `ca`, which has transition bins; llg crosses only value bins (llg limitation)",
    );
}

/// Select expressions expand to at most 1024 terms, through `&&` and `||`.
#[test]
fn select_term_budget_is_a_located_error() {
    let message = "the select expression of bins `big` of cross `ab` in covergroup `g` expands to more than 1024 terms (llg limitation)";
    sim_cli::reject_case(
        SUITE,
        "neg_cross_select_product",
        &format!("neg_cross_select_product.sv:10:18: {message}"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_cross_select_union",
        &format!("neg_cross_select_union.sv:10:18: {message}"),
    );
}

/// Selects that split the crossed coverpoints into more than 2^20 value
/// class combinations: a located run-time error at construction; the run
/// goes on and the cross counts nothing.
#[test]
fn cross_value_class_budget_is_a_run_time_error() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/cross_classes.out");
    sim_cli::run_case_checked_matrix(SUITE, "cross_classes", &[], &|label, output| {
        ends_run(
            label,
            output,
            expected,
            &["ERROR: [SIM-010] tb:16:12: covergroup `g` cross `abcd` has select expressions that split its coverpoints into more than 2^20 value class combinations; its bins are not counted (llg limitation)"],
        );
    });
}

/// The detect_overlap search of transition bins has a budget; reaching it
/// is a located compile-time warning.
#[test]
fn overlap_budget_is_a_located_warning() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/overlap_budget.out");
    sim_cli::run_case_checked_matrix(SUITE, "overlap_budget", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(llg_messages(output), Vec::<String>::new(), "{label}");
        assert_eq!(
            lowering_warnings(output),
            ["11:5: option.detect_overlap stops checking the transition bins of coverpoint `cp` in covergroup `g` at bins `a` and `b`, after 1048576 explored sequence steps; overlaps of these and later bins are not reported (llg limitation)"],
            "{label}"
        );
    });
}

/// Coverpoints of 65 to 128 bits count every bins form at full width
/// (industry-standard behaviour, probed).
#[test]
fn wide_coverpoints_compare_at_full_width() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/wide_points.out");
    sim_cli::run_case_checked_matrix(SUITE, "wide_points", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(llg_messages(output), Vec::<String>::new(), "{label}");
    });
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/wide_arguments.out");
    sim_cli::run_case_checked_matrix(SUITE, "wide_arguments", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert_eq!(llg_messages(output), Vec::<String>::new(), "{label}");
    });
}

/// An illegal value of a wide coverpoint ends the run and is spelled in
/// full (S45-D3).
#[test]
fn wide_illegal_values_end_the_run() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_045/wide_illegal.out");
    sim_cli::run_case_checked_matrix(SUITE, "wide_illegal", &[], &|label, output| {
        ends_run(
            label,
            output,
            expected,
            &[
                "ERROR: [SIM-010] covergroup `cg` coverpoint `cp` sampled -170141183460469231731687303715884105728, a value of illegal_bins `bad` at tb:8:56 (IEEE 1800-2009 19.5.5)",
                "FATAL: [SIM-009] (tb:8:56 @ 0 ps) illegal_bins value sampled",
            ],
        );
    });
}

/// The wide forms llg does not serve are located errors.
#[test]
fn wide_coverpoint_limits_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_wide_auto_ignore",
        "neg_wide_auto_ignore.sv:7:36: ignore_bins and illegal_bins of coverpoint `cp` in covergroup `g`, which is 71 bits wide and has automatic bins: llg removes values from automatic bins only for coverpoints of at most 64 bits (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_exact",
        "neg_wide_exact.sv:7:37: bins `b` of a coverpoint wider than 64 bits has an item of more than 2^32 values, each a bin or counted in a fixed number of bins; llg supports this only for coverpoints of at most 64 bits (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_point_argument",
        "neg_wide_point_argument.sv:7:34: bins `b` of a coverpoint wider than 64 bits reads a covergroup argument; llg reads bins values from arguments only for coverpoints of at most 64 bits (llg limitation)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_intersect_auto",
        "neg_wide_intersect_auto.sv:10:34: binsof in bins `lo` of cross `ab` in covergroup `g` intersects coverpoint `ca`, which is wider than 64 bits and has automatic bins (llg limitation)",
    );
}
