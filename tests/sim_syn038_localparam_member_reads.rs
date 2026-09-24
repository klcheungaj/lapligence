//! SYN-038 packed localparam member projections used at runtime.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn packed_localparam_members_lower_as_constant_runtime_reads() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "localparam_member_reads",
        "struct=12\nunion=a5c3\noctet=a5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
