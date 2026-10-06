//! Procedural row-slice, concatenation, and positional-pattern writes in generate scope.

use crate::sim_cli;

#[test]
fn generated_scope_lvalues_write_the_selected_objects() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "lvalue_generate_scope_remainders",
        "generated_row=31,32 generated_concat=4142 generated_pattern=01\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
