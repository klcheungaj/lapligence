//! H16 fine-grain process handles and lifecycle control.
//!
//! The checked-in fixtures cover the process-class identity/status contract,
//! suspended event waits, terminal await behavior, recursive cancellation
//! (also of descendants whose parent has finished), and ownership of delayed
//! nonblocking assignments.

use crate::sim_cli;

#[test]
fn process_handles_preserve_waits_identity_and_terminal_status() {
    sim_cli::run_case(
        "process_control",
        "control",
        "identity=1 child_status=1\ncontrol waiting=2 suspended=3 triggered=3 resumed=2 done=0 repeated=0 stage=1\n",
        "",
        &[],
    );
}

#[test]
fn killing_a_process_tree_drops_only_descendant_nbas() {
    sim_cli::run_case(
        "process_control",
        "kill_tree",
        "kill owner=4 killed=00 independent=3c\n",
        "",
        &[],
    );
}

#[test]
fn killing_a_child_updates_parent_fork_accounting() {
    sim_cli::run_case(
        "process_control",
        "kill_join",
        "kill join status=4 waited=1\n",
        "",
        &[],
    );
}

#[test]
fn static_block_handles_keep_their_identity_until_completion() {
    sim_cli::run_case(
        "process_control",
        "static_handle",
        "static identity=1 status=0\n",
        "",
        &[],
    );
}

#[test]
fn a_child_killing_its_ancestor_never_returns_to_released_locals() {
    sim_cli::run_case(
        "process_control",
        "kill_ancestor",
        "ancestor=4 caller=4 escaped=0 owner_after=0 descendant=0\n",
        "",
        &[],
    );
}

// Kill and disable fork reach processes whose parent has finished (SV 9.6.3,
// 9.7). Expected outputs match an independent industry-standard simulator
// (fixture readme).

/// Leaves two to four generations below finished processes end with their root.
#[test]
fn killing_a_root_ends_leaves_below_finished_processes() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_depth",
        include_str!("fixtures/sim/process_control/kill_orphan_depth.out"),
        "",
        &[],
    );
}

/// Leaves outliving a finished `join_any` parent and a finished joined branch
/// end with their root.
#[test]
fn killing_a_root_ends_leaves_of_finished_join_branches() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_joins",
        include_str!("fixtures/sim/process_control/kill_orphan_joins.out"),
        "",
        &[],
    );
}

/// Killed leaves blocked on an event, a delay, a semaphore, a mailbox, a wait
/// condition and an edge never resume and take nothing.
#[test]
fn killed_orphan_waiters_take_no_keys_or_messages() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_waits",
        include_str!("fixtures/sim/process_control/kill_orphan_waits.out"),
        "",
        &[],
    );
}

/// A root killing itself ends its orphaned leaf; an orphaned leaf killing its
/// root ends itself and its sibling.
#[test]
fn orphans_end_with_a_root_that_kills_itself_or_is_killed_by_them() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_self",
        include_str!("fixtures/sim/process_control/kill_orphan_self.out"),
        "",
        &[],
    );
}

/// `await()` of an orphaned leaf returns at its root's kill; the finished
/// middle process stays FINISHED.
#[test]
fn await_of_a_killed_orphan_returns_at_the_kill() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_await",
        include_str!("fixtures/sim/process_control/kill_orphan_await.out"),
        "",
        &[],
    );
}

/// After a root's kill its live descendants are KILLED and its finished ones
/// stay FINISHED.
#[test]
fn finished_descendants_stay_finished_after_a_kill() {
    sim_cli::run_case(
        "process_control",
        "kill_orphan_status",
        include_str!("fixtures/sim/process_control/kill_orphan_status.out"),
        "",
        &[],
    );
}

/// `kill()` of a finished process ends the child it left running; the process
/// stays FINISHED.
#[test]
fn killing_a_finished_process_ends_its_live_children() {
    sim_cli::run_case(
        "process_control",
        "kill_finished_process",
        include_str!("fixtures/sim/process_control/kill_finished_process.out"),
        "",
        &[],
    );
}

/// `wait fork` waits for the immediate child only; `disable fork` then ends the
/// grandchild it left.
#[test]
fn wait_fork_ignores_orphans_and_disable_fork_ends_them() {
    sim_cli::run_case(
        "process_control",
        "wait_fork_orphans",
        include_str!("fixtures/sim/process_control/wait_fork_orphans.out"),
        "",
        &[],
    );
}

/// `disable fork` ends leaves below finished processes and leaves another
/// initial's tree alone.
#[test]
fn disable_fork_ends_leaves_below_finished_processes() {
    sim_cli::run_case(
        "process_control",
        "disable_fork_orphans",
        include_str!("fixtures/sim/process_control/disable_fork_orphans.out"),
        "",
        &[],
    );
}

/// The records kept for finished processes with live descendants are
/// reclaimed with their trees, whichever way the tree ends: 2000 trees leave
/// one registered process.
#[test]
fn records_kept_for_finished_subtrees_are_reclaimed() {
    sim_cli::run_case(
        "process_control",
        "orphan_records_reclaimed",
        include_str!("fixtures/sim/process_control/orphan_records_reclaimed.out"),
        concat!(
            "INFO: orphan_records_reclaimed.sv:44:5: [SIM-001] (tb @ 40100 ns) $finish called\n",
            "INFO: [SIM-008] simulation statistics: processes=1\n",
        ),
        &[],
    );
}
