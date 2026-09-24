//! SYN-038 public-CLI witnesses for record-valued conditional writes in latches.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn child_record_and_selected_record_array_latches_match_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "latch_record",
        concat!(
            "closed record=x,x array=x,x:x,x\n",
            "open record=3,4 array=x,x:7,8\n",
            "held record=3,4 array=x,x:7,8\n",
            "selected record=9,a array=9,a:7,8\n",
            "updated record=d,e array=d,e:7,8\n",
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}
