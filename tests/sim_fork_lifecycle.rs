//! P19 fork lifecycle coverage. Each fixture is run in optimized and
//! unoptimized modes by the shared simulator CLI harness.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn join_variants_start_and_complete_at_their_boundaries() {
    sim_cli::run_case(
        "fork_lifecycle",
        "join_lifecycle",
        "none parent before block=5 t=0\nnone child sees=5 t=0\nnone complete=2 t=2\nterminate parent t=2\nterminate child t=2\nany first=1 t=4\nany complete=2 t=5\n",
        "",
        &[],
    );
}

#[test]
fn disable_fork_cancels_nested_pending_descendants() {
    sim_cli::run_case(
        "fork_lifecycle",
        "nested_disable",
        "nested disable value=0 t=2\n",
        "",
        &[],
    );
}

#[test]
fn detached_captures_and_persistent_delayed_nba_survive_creator_exit() {
    sim_cli::run_case(
        "fork_lifecycle",
        "detached_storage",
        "capture=7 t=1\ncapture parent=9 t=1\nqueued target=5a t=3\n",
        "",
        &[],
    );
}
