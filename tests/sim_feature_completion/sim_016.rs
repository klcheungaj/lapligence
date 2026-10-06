//! SIM-016: semaphore collections and shared-lifetime synchronization.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_016";

#[test]
fn task_handle_variables_are_shared_with_fork_branches() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_016/shared_handles.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_handles", expected, &[], &[]);
}

#[test]
fn handle_outputs_copy_out_into_container_elements() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_016/element_outputs.out");
    sim_cli::run_case_backend_parity(SUITE, "element_outputs", expected, &[], &[]);
}

#[test]
fn ref_handle_element_actuals_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_element_handle",
        "ref actual of handle formal `s` in `tb` is a container element; element references of handles are not supported (SIM-016)",
    );
}

#[test]
fn array_semaphores_keep_pools_and_aliases_share_them() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_016/pools_and_aliases.out");
    sim_cli::run_case_backend_parity(SUITE, "pools_and_aliases", expected, &[], &[]);
}

#[test]
fn killed_waiters_never_consume_granted_keys() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_016/grant_cancellation.out");
    sim_cli::run_case_backend_parity(SUITE, "grant_cancellation", expected, &[], &[]);
}
