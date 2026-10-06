//! SYN-038 automatic enum initialization, explicit function return, and
//! separate clocked enum NBA destination through the public CLI.

use crate::sim_cli;

#[test]
fn automatic_enum_pattern_function_and_clocked_nba_match_oracle() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "enum_auto_function",
        "initial=00000101 state=00000111\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
