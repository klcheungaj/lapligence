//! SYN-038 packed-struct cast function returns in an always_ff NBA RHS.

use crate::sim_cli;

#[test]
fn packed_cast_function_return_is_captured_by_always_ff() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "packed_return_ff",
        "12 34\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
