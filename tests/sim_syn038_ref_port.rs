//! SYN-038 whole-variable module `ref` port aliasing through the public CLI.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn module_ref_port_tracks_parent_and_child_updates_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "ref_port",
        "refport=c3\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
