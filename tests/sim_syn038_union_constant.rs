//! Packed-union constant elaboration and direct member projection through the public CLI.

use crate::sim_cli;

#[test]
fn packed_union_projection_drives_constant_dimension_and_runtime_reads() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "union_constant",
        "base=a5 low=5 elaborated_width=10\nruntime=3c low=c\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn packed_union_constant_form_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn038_pairwise",
        "union_constant",
        "`$bits` is not available in IEEE 2001",
        &["--edition", "2001"],
    );
}
