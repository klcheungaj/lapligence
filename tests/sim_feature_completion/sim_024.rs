//! SIM-024: typed formatting of native and aggregate values.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_024";

#[test]
fn integral_sizes_radixes_and_field_flags() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/integral_sizes.out");
    sim_cli::run_case(SUITE, "integral_sizes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "integral_sizes", expected, &[], &[]);
}

#[test]
fn strings_reals_and_cross_type_conversions() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/strings_reals.out");
    sim_cli::run_case(SUITE, "strings_reals", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "strings_reals", expected, &[], &[]);
}

#[test]
fn aggregate_patterns_nest_and_keep_declared_bounds() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/aggregates.out");
    sim_cli::run_case(SUITE, "aggregates", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "aggregates", expected, &[], &[]);
}

#[test]
fn class_patterns_stop_at_cycles_and_the_depth_bound() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/classes.out");
    sim_cli::run_case(SUITE, "classes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "classes", expected, &[], &[]);
}

#[test]
fn opaque_handles_events_and_interfaces() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/handles.out");
    sim_cli::run_case(SUITE, "handles", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "handles", expected, &[], &[]);
}

#[test]
fn hierarchy_names_follow_the_calling_scope() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/hierarchy.out");
    sim_cli::run_case(SUITE, "hierarchy", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "hierarchy", expected, &[], &[]);
}

#[test]
fn monitor_and_strobe_patterns() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/deferred.out");
    sim_cli::run_case(SUITE, "deferred", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "deferred", expected, &[], &[]);
}

#[test]
fn file_and_string_outputs_share_the_formatter() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/outputs.out");
    sim_cli::run_case(SUITE, "outputs", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "outputs", expected, &[], &[]);
}

#[test]
fn run_time_formats_report_missing_arguments_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_024/dynamic_missing.out");
    sim_cli::run_case(
        SUITE,
        "dynamic_missing",
        expected,
        "",
        &["format specification `%0d` has no argument; printed as written"],
    );
    sim_cli::run_case_backend_parity(SUITE, "dynamic_missing", expected, &[], &[]);
}

#[test]
fn neg_argument_count_type_and_specifier_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_missing_argument",
        "no argument provided for '%d' format specifier",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unknown_specifier",
        "unknown format specifier '%q'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_decimal",
        "value of type 'int$[2]' is invalid for '%d' format specifier",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unformatted_queue",
        "cannot format values of type 'int$[$]' without a specification string",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_class_decimal",
        "$display format `%d` cannot format a handle of type `C` in `tb`; only `%p` formats it",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_width_on_hierarchy",
        "field width not allowed on '%m' format specifiers",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_tagged_union_pattern",
        "is not supported: tagged unions have no pattern form yet",
    );
}
