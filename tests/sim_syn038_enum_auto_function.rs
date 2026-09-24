//! SYN-038 automatic enum initialization, explicit function return, and
//! separate clocked enum NBA destination through the public CLI.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn automatic_enum_pattern_function_and_clocked_nba_match_oracle() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "enum_auto_function",
        "initial=00000101 state=00000111\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
