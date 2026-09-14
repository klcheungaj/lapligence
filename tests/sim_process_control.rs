//! H16 fine-grain process handles and lifecycle control.
//!
//! The checked-in fixtures cover the process-class identity/status contract,
//! suspended event waits, terminal await behavior, recursive cancellation,
//! and ownership of delayed nonblocking assignments.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn process_handles_preserve_waits_identity_and_terminal_status() {
    sim_cli::run_case(
        "process_control",
        "control",
        "identity=1 child_status=1\ncontrol waiting=2 suspended=3 triggered=3 done=0 repeated=0 stage=1\n",
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
