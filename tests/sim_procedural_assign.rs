//! File-based simulator acceptance tests for procedural continuous assignment
//! priority, replacement, dependency propagation, and force layering.

use crate::sim_cli;

#[test]
fn procedural_assign_batches_preserve_selections_loop_owners_and_real_force_layers() {
    sim_cli::run_case(
        "procedural_assign",
        "batch_selections",
        "CHECK: selects=9 6 c 3 bits=0101\nCHECK: real=1.25 2.50 -3.50 0.25\nCHECK: forced=0 9.00\nCHECK: released=1 4.50\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_batches_preserve_issue_order_casts_and_deassign() {
    sim_cli::run_case(
        "procedural_assign",
        "batch_order",
        "CHECK: ordered=12 81 12 34\nCHECK: replaced=34 34 81 12\nCHECK: casts=ff81 0012 0034 0056 two=00 81 12 34\nCHECK: settled=56 34 81 12\nCHECK: live=56 43 7e 22 casts=007e 0022 0043 0056\n",
        "",
        &[],
    );
}

#[test]
fn procedural_assign_batches_work_in_shared_bodies_with_net_sources() {
    sim_cli::run_case(
        "procedural_assign",
        "batch_shared",
        "CHECK: shared=5a 5a 5a 5a\nCHECK: shared=5a 5a 5a 5a\nCHECK: shared=5a 5a 5a 5a\nCHECK: shared=5a 5a 5a 5a\nCHECK: freed=a5\nCHECK: freed=a5\nCHECK: freed=a5\nCHECK: freed=a5\n",
        "",
        &[],
    );
}

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
