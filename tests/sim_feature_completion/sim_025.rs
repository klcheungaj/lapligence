//! SIM-025: postponed output and legal monitor dependencies.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_025";

// A01: the final settled values of a slot, with string and nested arguments.
#[test]
fn monitor_reports_final_values_once_per_slot() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/monitor_final.out");
    sim_cli::run_case(SUITE, "monitor_final", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "monitor_final", expected, &[], &[]);
}

#[test]
fn strobe_reads_container_actuals_at_the_end_of_the_slot() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/strobe_container_actuals.out");
    sim_cli::run_case_backend_parity(SUITE, "strobe_container_actuals", expected, &[], &[]);
}

#[test]
fn strobe_reports_settled_values_last_issued_first() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/strobe_final.out");
    sim_cli::run_case(SUITE, "strobe_final", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "strobe_final", expected, &[], &[]);
}

#[test]
fn reports_build_their_actuals_when_they_print() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/strobe_built_actuals.out");
    sim_cli::run_case(SUITE, "strobe_built_actuals", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "strobe_built_actuals", expected, &[], &[]);
}

#[test]
fn time_arguments_print_but_never_trigger() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/monitor_time_args.out");
    sim_cli::run_case(SUITE, "monitor_time_args", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "monitor_time_args", expected, &[], &[]);
}

#[test]
fn scope_and_time_in_monitored_lists() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/scope_time.out");
    sim_cli::run_case(SUITE, "scope_time", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "scope_time", expected, &[], &[]);
}

#[test]
fn strobe_and_monitor_in_static_subroutines() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/static_subroutines.out");
    sim_cli::run_case(SUITE, "static_subroutines", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "static_subroutines", expected, &[], &[]);
}

#[test]
fn monitor_follows_handle_selected_storage() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/dynamic_members.out");
    sim_cli::run_case(SUITE, "dynamic_members", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "dynamic_members", expected, &[], &[]);
}

#[test]
fn monitor_follows_handles_and_properties_exactly() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/class_property_reports.out");
    sim_cli::run_case(SUITE, "class_property_reports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "class_property_reports", expected, &[], &[]);
}

#[test]
fn monitor_over_a_property_path_evaluates_only_on_change() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/exact_class_triggers.out");
    sim_cli::run_case(SUITE, "exact_class_triggers", expected, "", &[]);
}

#[test]
fn reports_in_class_methods_keep_their_receiver() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/class_method_reports.out");
    sim_cli::run_case(SUITE, "class_method_reports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "class_method_reports", expected, &[], &[]);
}

#[test]
fn reports_read_static_string_and_handle_variables() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/static_handle_formals.out");
    sim_cli::run_case(SUITE, "static_handle_formals", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "static_handle_formals", expected, &[], &[]);
}

// A02: replacement, enable/disable and file outputs.
#[test]
fn monitor_replacement_and_enable_flag() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/monitor_control.out");
    sim_cli::run_case(SUITE, "monitor_control", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "monitor_control", expected, &[], &[]);
}

#[test]
fn fmonitor_lists_are_independent_and_cancelled_by_fclose() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/file_monitors.out");
    sim_cli::run_case(SUITE, "file_monitors", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "file_monitors", expected, &[], &[]);
}

#[test]
fn fstrobe_is_cancelled_by_fclose_per_channel() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_025/file_strobes.out");
    sim_cli::run_case(SUITE, "file_strobes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "file_strobes", expected, &[], &[]);
}

// A03: invalid descriptors, read-only Postponed region.
#[test]
fn invalid_descriptors_register_nothing() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/invalid_descriptors.out");
    sim_cli::run_case(SUITE, "invalid_descriptors", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "invalid_descriptors", expected, &[], &[]);
}

#[test]
fn postponed_reports_leave_design_state_unchanged() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_025/postponed_readonly.out");
    sim_cli::run_case(SUITE, "postponed_readonly", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "postponed_readonly", expected, &[], &[]);
}

#[test]
fn neg_automatic_storage_stays_illegal() {
    sim_cli::reject_case(
        SUITE,
        "neg_auto_task_monitor",
        "automatic variable 'v' cannot be traced",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_auto_local_strobe",
        "automatic variable 'i' cannot be traced",
    );
}

#[test]
fn neg_strobe_container_conversion_names_the_formal() {
    sim_cli::reject_case(
        SUITE,
        "neg_strobe_container_conversion",
        "a `$strobe` or `$monitor` argument for container formal `x` in `tb` must be a container variable of the formal's type",
    );
}

#[test]
fn neg_postponed_helpers_cannot_write_design_state() {
    sim_cli::reject_case(
        SUITE,
        "neg_strobe_helper_write",
        "a helper in `tb` writes storage it does not own: `$monitor`/`$strobe` arguments are evaluated in the read-only Postponed region",
    );
}

#[test]
fn neg_class_storage_limits_are_diagnosed() {
    sim_cli::reject_case(
        SUITE,
        "neg_monitor_record_property",
        "has no dependency that sees it change",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_method_formal_strobe",
        "automatic variable 'a' cannot be traced",
    );
}

#[test]
fn null_handle_in_a_report_stops_the_run() {
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "null_report",
        &[],
        include_str!("../fixtures/sim/feature_completion/sim_025/null_report.out"),
        "null class handle access: property access at tb:19:",
    );
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "null_monitor",
        &[],
        include_str!("../fixtures/sim/feature_completion/sim_025/null_monitor.out"),
        "null class handle access: property access at tb:11:",
    );
}
