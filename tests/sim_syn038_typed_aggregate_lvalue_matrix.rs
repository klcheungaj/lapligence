//! Typed aggregate lvalues retain selected values and adjacent sentinels.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn typed_aggregate_lvalue_matrix_matches_exact_oracle_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_aggregate_lvalue_matrix",
        "records=20,21,22,23 payload=70,71,61 enum=1,2\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
