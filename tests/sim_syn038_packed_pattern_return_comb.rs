//! SYN-038 packed integral assignment-pattern function returns in always_comb.

use crate::sim_cli;

#[test]
fn packed_pattern_function_return_runs_in_always_comb() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "packed_pattern_return_comb",
        "result=a5\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
