//! SYN-038 runtime packed-array indexing over packed localparam members.

use crate::sim_cli;

#[test]
fn localparam_packed_member_runtime_indices_preserve_four_state_selection() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "localparam_runtime_index",
        "valid=a5/c3 asc=a5/c3 signed=c3 unsigned=xx z=z3 unknown=xx out=xx/xx/xx\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
