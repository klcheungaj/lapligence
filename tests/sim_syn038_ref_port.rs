//! SYN-038 whole-variable module `ref` port aliasing through the public CLI.

use crate::sim_cli;

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
