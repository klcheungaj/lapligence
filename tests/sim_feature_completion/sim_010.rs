//! SIM-010: fork lifetime, named disable and capture cleanup. Oracles are
//! derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_010";

#[test]
fn fork_block_declarations_are_created_per_fork_execution() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/fork_declarations.out");
    sim_cli::run_case_backend_parity(SUITE, "fork_declarations", expected, &[], &[]);
}

#[test]
fn detached_branches_share_enclosing_automatics() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/shared_automatics.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_automatics", expected, &[], &[]);
}

#[test]
fn detached_branches_share_enclosing_automatic_strings() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/shared_strings.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_strings", expected, &[], &[]);
}

#[test]
fn fork_branches_share_task_containers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/shared_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_containers", expected, &[], &[]);
}

#[test]
fn joined_branches_share_automatic_strings() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/joined_strings.out");
    sim_cli::run_case_backend_parity(SUITE, "joined_strings", expected, &[], &[]);
}

#[test]
fn event_controls_wake_on_writes_by_other_fork_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/fork_wakeups.out");
    sim_cli::run_case_backend_parity(SUITE, "fork_wakeups", expected, &[], &[]);
}

#[test]
fn detached_branches_share_input_formals() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/shared_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_formals", expected, &[], &[]);
}

#[test]
fn fork_branches_share_task_records() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/shared_records.out");
    sim_cli::run_case_backend_parity(SUITE, "shared_records", expected, &[], &[]);
}

#[test]
fn waits_on_shared_containers_wake_on_changes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/container_waits.out");
    sim_cli::run_case_backend_parity(SUITE, "container_waits", expected, &[], &[]);
}

#[test]
fn disable_fork_and_task_disable_reach_their_targets_only() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/disable_scope.out");
    sim_cli::run_case_backend_parity(SUITE, "disable_scope", expected, &[], &[]);
}

#[test]
fn disabled_branches_never_resume_from_any_suspension() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/kill_suspensions.out");
    sim_cli::run_case_backend_parity(SUITE, "kill_suspensions", expected, &[], &[]);
}

#[test]
fn fork_branches_share_container_formals() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/container_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "container_formals", expected, &[], &[]);
}

#[test]
fn recursive_task_children_outlive_returns_and_sibling_disables() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/recursive_lifetimes.out");
    sim_cli::run_case_backend_parity(SUITE, "recursive_lifetimes", expected, &[], &[]);
}
