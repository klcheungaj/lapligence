//! SYN-038 child-parameter reads in module variable declaration initializers.
//!
//! The fixture checks an unoverridden child localparam, separate elaborated
//! values in two overridden sibling instances, and a procedural-read control.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn child_parameter_initializers_keep_instance_identity_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "child_param_decl_init",
        "decl=05 siblings=05/0a procedural=05\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
