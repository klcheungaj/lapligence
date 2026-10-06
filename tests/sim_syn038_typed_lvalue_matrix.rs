//! SYN-038 selected lvalue paths over enums, packed structs, and packed unions.

use crate::sim_cli;

#[test]
fn typed_selected_lvalue_matrix_matches_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_lvalue_matrix",
        "enum=01/a0/a5 struct=0100/a000/b5c2 union=0100/a000/d3a4\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
