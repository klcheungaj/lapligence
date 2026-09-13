//! File-based simulator regressions for fixed-array and resizable-container
//! sensitivity. The CLI harness runs every fixture with and without
//! optimization.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn fixed_array_readers_wake_on_element_and_index_changes() {
    sim_cli::run_case(
        "array_sensitivity",
        "fixed_array",
        "fixed0=a5 xx a5 a5 a5 a5\nfixed1=5a xx 5a 5a 5a 5a\nfixed2=3c 3c 3c 3c 3c 3c\nfixed_wait=c3 c3 c3 c3 c3 c3\n",
        "llg: $finish at time 4000 at tb:54:9\n",
        &[],
    );
}

#[test]
fn fixed_array_nba_elements_wake_combinational_readers() {
    sim_cli::run_case(
        "array_sensitivity",
        "nba_array",
        "nba0=00\nnba1=5a\nnba2=a5\n",
        "llg: $finish at time 4000 at tb:34:9\n",
        &[],
    );
}

#[test]
fn resizable_container_readers_wake_on_resize_push_and_delete() {
    sim_cli::run_case(
        "array_sensitivity",
        "containers",
        "container1=11 1 11 1\nwait_resize=2\ncontainer2=11 2 11 2\nwait_delete=0\ncontainer3=xx 0 xx 0\n",
        "llg: $finish at time 3000 at tb:43:9\n",
        &[],
    );
}

#[test]
fn array_and_container_identity_survives_alias_copy_and_unchanged_writes() {
    sim_cli::run_case(
        "array_sensitivity",
        "edge_paths",
        "unchanged=0\nchanged=1\nalias=5a\ncopy=22\npush=2\nlhs_only=3\ndelete=0\n",
        "llg: $finish at time 8000 at tb:62:9\n",
        &[],
    );
}
