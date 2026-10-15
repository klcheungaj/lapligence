//! SIM-044: covergroup declarations, instances and sampling (IEEE 1800-2009
//! §§19.3-19.5, 19.7-19.9, 19.11). Every expected percentage is derived by
//! hand in the fixture readme; each positive case runs in both optimizer
//! modes on the legacy, compact/portable and compact/GMP value backends.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_044";

fn assert_stdout(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
}

/// The lowering warnings of a run, without the source path prefix.
fn lowering_warnings(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: "))
        .map(|warning| {
            warning
                .split_once(".sv:")
                .map_or(warning, |(_, rest)| rest)
                .to_owned()
        })
        .collect()
}

#[test]
fn instances_collect_independently_and_queries_follow_19_11() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/instances.out");
    sim_cli::run_case_checked_matrix(SUITE, "instances", &[], &|label, output| {
        assert_stdout(label, output, expected);
        assert!(lowering_warnings(output).is_empty(), "{label}");
    });
}

#[test]
fn event_function_and_block_sampling_see_the_prescribed_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/sampling.out");
    sim_cli::run_case_checked_matrix(SUITE, "sampling", &[], &|label, output| {
        assert_stdout(label, output, expected);
    });
}

#[test]
fn embedded_groups_and_held_arguments_outlive_their_last_reference() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/embedded.out");
    // A collection at every allocation proves the owner and the handle
    // argument stay reachable from the covergroup objects.
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "embedded",
        &[],
        &[("LLG_GC_STRESS", "1")],
        &|label, output| assert_stdout(label, output, expected),
    );
}

#[test]
fn bins_values_resolve_against_the_coverpoint_type() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/bins.out");
    sim_cli::run_case_checked_matrix(SUITE, "bins", &[], &|label, output| {
        assert_stdout(label, output, expected);
        // One warning per resolved element of the 19.5.6 examples, plus the
        // out-of-domain `never` value.
        let limited =
            "range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)";
        let ignored = "value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)";
        let mut warnings = lowering_warnings(output);
        warnings.sort();
        let mut expected = vec![
            format!("11:28: bins `b1` {limited}"),
            format!("12:18: bins `b2` {ignored}"),
            format!("12:22: bins `b2` {limited}"),
            format!("12:30: bins `b2` {ignored}"),
            format!("15:21: bins `b3` {limited}"),
            format!("15:28: bins `b3` {ignored}"),
            format!("16:22: bins `b4` {limited}"),
            format!("16:30: bins `b4` {ignored}"),
            format!("23:62: bins `never` {ignored}"),
        ];
        expected.sort();
        assert_eq!(warnings, expected, "{label}");
    });
}

#[test]
fn ref_arguments_follow_their_bound_variable() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/ref_formals.out");
    sim_cli::run_case_checked_matrix(SUITE, "ref_formals", &[], &|label, output| {
        assert_stdout(label, output, expected);
    });
}

#[test]
fn coverpoint_only_reads_keep_their_storage() {
    sim_cli::run_case_checked_matrix(SUITE, "storage", &[], &|label, output| {
        assert_stdout(label, output, "point=25.00 sample=6.25\n");
    });
}

#[test]
fn instance_events_wait_per_instance_independently_of_their_creator() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/instance_events.out");
    sim_cli::run_case_checked_matrix(SUITE, "instance_events", &[], &|label, output| {
        assert_stdout(label, output, expected);
    });
}

#[test]
fn dropped_instances_keep_their_waiters() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/dropped_waiters.out");
    // A collection at every allocation proves the waiter's capture keeps
    // the instance and its owner reachable.
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "dropped_waiters",
        &[],
        &[("LLG_GC_STRESS", "1")],
        &|label, output| assert_stdout(label, output, expected),
    );
}

#[test]
fn strobe_samples_once_per_slot_in_the_postponed_region() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/strobe.out");
    sim_cli::run_case_checked_matrix(SUITE, "strobe", &[], &|label, output| {
        assert_stdout(label, output, expected);
    });
}

#[test]
fn end_events_follow_returns_and_named_blocks() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_044/block_returns.out");
    sim_cli::run_case_checked_matrix(SUITE, "block_returns", &[], &|label, output| {
        assert_stdout(label, output, expected);
    });
}

#[test]
fn null_covergroup_handle_is_a_runtime_error() {
    sim_cli::run_case_checked_matrix(SUITE, "null_sample", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "before\n",
            "{label}"
        );
        assert!(
            stderr.contains("null class handle access: covergroup method call at tb:11:5"),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn unsupported_coverage_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_option_access",
        "neg_option_access.sv:10:5: covergroup options can only be set in the covergroup declaration",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_two_arg_query",
        "neg_two_arg_query.sv:12:9: `get_inst_coverage` with covered and total bin arguments is not supported yet (SIM-046)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_strobe",
        "neg_strobe.sv:8:19 no member named 'strobe'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wide_point",
        "neg_wide_point.sv:6:16: coverpoint `a` of covergroup `g` is 80 bits wide",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_import_event",
        "neg_import_event.sv:5:31: the covergroup block event on `c_value` is not supported for this subroutine",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_embedded_outside_new",
        "neg_embedded_outside_new.sv:12:9 cannot assign to read-only variable 'g'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_sample_arguments",
        "neg_sample_arguments.sv:9:5 too few arguments for 'sample'; expected 1 but 0 were provided",
    );
}
