//! File-based simulator acceptance tests for procedural continuous assignment
//! priority, replacement, dependency propagation, and force layering.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn procedural_assign_priority_blocks_ordinary_writes() {
    sim_cli::run_case(
        "procedural_assign",
        "priority",
        "CHECK: blocking=1\nCHECK: nba=1\nCHECK: deassign=0\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_sites_replace_at_runtime() {
    sim_cli::run_case(
        "procedural_assign",
        "replacement",
        "CHECK: first=0\nCHECK: second=1\nCHECK: follows=0\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_converts_to_two_state_targets() {
    sim_cli::run_case(
        "procedural_assign",
        "two_state",
        "CHECK: unknown=0\nCHECK: known=1\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_tracks_function_dependencies() {
    sim_cli::run_case(
        "procedural_assign",
        "function_dependency",
        "CHECK: initial=0\nCHECK: changed=1\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_release_returns_to_live_rhs() {
    sim_cli::run_case(
        "procedural_assign",
        "force_interaction",
        "CHECK: forced=0\nCHECK: released=0\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_supports_real_targets_and_force_release() {
    sim_cli::run_case(
        "procedural_assign",
        "real",
        "CHECK: real_blocking=1.0\nCHECK: real_nba=1.0\nCHECK: real_live=2.0\nCHECK: real_forced=9.0\nCHECK: real_released=3.0\nCHECK: real_deassign=4.0\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_supports_packed_concatenation_targets() {
    sim_cli::run_case(
        "procedural_assign",
        "concat",
        "CHECK: concat_blocking=10110\nCHECK: concat_nba=10110\nCHECK: concat_live=01001\nCHECK: concat_forced=11111\nCHECK: concat_released=11011\nCHECK: concat_deassign=00000\n",
        "",
        &[],
    );
}
