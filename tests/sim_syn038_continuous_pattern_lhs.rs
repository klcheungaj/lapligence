//! SYN-038 positional assignment-pattern continuous LHSs through the public CLI.

use crate::sim_cli;

#[test]
fn continuous_pattern_lvalues_preserve_targets_and_rhs_sensitivity() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "continuous_pattern_lhs",
        "rhs_eval\nrhs_eval\nnet=01 selected_net=01 pure_variable=01 counted_variable=01\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
