//! SYN-038 typed actuals through output, inout, ref, and const-ref task formals.

use crate::sim_cli;

#[test]
fn typed_actuals_keep_formal_directions_distinct_from_storage() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_formal_matrix",
        "enum=01,01,01 pair=5aa5 union=5aa5 records=11/a1,22/b2 inout=c3/44 ref=31/c7\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
