//! SIM-011: class field layouts and suspendable method dispatch: timed
//! virtual, super, static and hierarchical task calls, automatic method
//! activations, per-specialization statics, receiver identity, handle
//! chains and null, access, cast, abstract and const outcomes. Oracles are
//! derived by hand in the fixture readme.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_011";

#[test]
fn base_handle_calls_overridden_timed_tasks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "timed_virtual",
        include_str!("../fixtures/sim/feature_completion/sim_011/timed_virtual.out"),
        &[],
        &[],
    );
}

#[test]
fn adopted_timed_class_witness() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_timed_class",
        include_str!("../fixtures/sim/feature_completion/sim_011/witness_timed_class.out"),
        &[],
        &[],
    );
}

#[test]
fn super_static_and_hierarchical_task_calls_suspend() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "hierarchical_tasks",
        include_str!("../fixtures/sim/feature_completion/sim_011/hierarchical_tasks.out"),
        &[],
        &[],
    );
}

#[test]
fn running_method_keeps_its_receiver_after_rebinding() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "receiver_rebind",
        include_str!("../fixtures/sim/feature_completion/sim_011/receiver_rebind.out"),
        &[],
        &[],
    );
}

#[test]
fn method_fork_branches_use_the_receiver() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "method_forks",
        include_str!("../fixtures/sim/feature_completion/sim_011/method_forks.out"),
        &[],
        &[],
    );
}

#[test]
fn concurrent_and_recursive_methods_have_independent_locals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "automatic_methods",
        include_str!("../fixtures/sim/feature_completion/sim_011/automatic_methods.out"),
        &[],
        &[],
    );
}

#[test]
fn statics_are_shared_per_specialization() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "static_specializations",
        include_str!("../fixtures/sim/feature_completion/sim_011/static_specializations.out"),
        &[],
        &[],
    );
}

#[test]
fn properties_through_handle_chains_use_the_selected_object() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handle_chains",
        include_str!("../fixtures/sim/feature_completion/sim_011/handle_chains.out"),
        &[],
        &[],
    );
}

#[test]
fn subroutine_handle_locals_select_properties() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "task_local_handles",
        include_str!("../fixtures/sim/feature_completion/sim_011/task_local_handles.out"),
        &[],
        &[],
    );
}

#[test]
fn container_and_fixed_array_properties_are_per_object() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "container_properties",
        include_str!("../fixtures/sim/feature_completion/sim_011/container_properties.out"),
        &[],
        &[],
    );
}

#[test]
fn packed_and_record_properties_project_members() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "packed_properties",
        include_str!("../fixtures/sim/feature_completion/sim_011/packed_properties.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "record_properties",
        include_str!("../fixtures/sim/feature_completion/sim_011/record_properties.out"),
        &[],
        &[],
    );
}

/// The located null-handle report of `site`, then the process stops.
fn null_handle_check(stdout: &'static str, site: &'static str) -> impl Fn(&str, &Output) {
    move |label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        // Frontend lint warnings precede the run-time report.
        let runtime = stderr
            .lines()
            .filter(|line| !line.starts_with("Warning: "))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        assert_eq!(
            runtime,
            format!(
                "llg: null class handle access: {site}\nllg: severity fatal: {site}: null class handle access\n"
            ),
            "{label}"
        );
    }
}

#[test]
fn null_receivers_fail_at_their_source_site() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "null_receiver",
        &[],
        &null_handle_check("before\n", "method call at tb:15:12"),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "null_property",
        &[],
        &null_handle_check("before 3\n", "property access at tb:15:9"),
    );
}

#[test]
fn invalid_downcasts_return_zero_or_report() {
    sim_cli::run_case_checked_matrix(SUITE, "bad_downcast", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            include_str!("../fixtures/sim/feature_completion/sim_011/bad_downcast.out"),
            "{label}"
        );
        assert!(
            stderr.contains(
                "llg: runtime error: $cast failed to assign an incompatible value at tb:25:9\n"
            ),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn illegal_class_forms_are_frontend_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_local_access",
        "'secret' is a local member of 'O'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_protected_access",
        "'guarded' is a protected member of 'O'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_abstract_new",
        "cannot create instance of virtual class 'A'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_assign",
        "cannot assign to read-only variable 'k'",
    );
}

#[test]
fn unsupported_property_layouts_reject_explicitly() {
    sim_cli::reject_case(
        SUITE,
        "neg_record_container_property",
        "class property `holder_c.m` of a record type with queue, dynamic or associative members is not supported (SIM-011)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_multidim_property",
        "multidimensional fixed array class property `grid_c.g` is not supported (SIM-011)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_static_record_property",
        "static class property `holder_c.shared` of an unpacked record or union type with string, real, handle or container members is not supported (SIM-011)",
    );
}
