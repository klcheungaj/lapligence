//! SYN-038 packed integral assignment-pattern function returns in always_comb.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn packed_pattern_function_return_runs_in_always_comb() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "packed_pattern_return_comb",
        "result=a5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
