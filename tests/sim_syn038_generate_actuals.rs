//! SYN-038 generated-scope inout, ref, and const-ref actual paths.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn generated_scope_actuals_reach_ports_and_subroutine_formals() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "generate_actuals",
        "pads=3c,a5 ref=12,24 const=55,c3\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
