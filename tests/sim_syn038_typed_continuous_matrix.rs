//! Typed continuous assignments across whole values, lvalues, and operations.

use crate::sim_cli;

#[test]
fn typed_continuous_assignments_match_with_and_without_optimization() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_continuous_matrix",
        "operation=03,03,03,03,03,05,05\nlvalue=1234,1234,b2,b3,c6\ncont=01,01,01,1234,1234,a5c3,a5c3,56,78,11,44\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
