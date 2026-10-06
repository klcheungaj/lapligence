//! SYN-038 generated child output actuals to packed-struct fields.

use crate::sim_cli;

#[test]
fn generated_child_output_propagates_to_a_packed_struct_field() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "generate_output_field",
        "hi=5a\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
