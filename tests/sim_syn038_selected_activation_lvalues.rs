//! SYN-038 selected writes preserve activation and local storage roots.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn selected_formal_local_and_return_lvalues_match_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "selected_activation_lvalues",
        "local=b6,b7 formal=b4 return=b5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
