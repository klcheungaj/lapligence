//! SYN-038 packed union function results, child ports, and field NBAs.

use crate::sim_cli;

#[test]
fn packed_union_function_and_child_port_paths_keep_four_state_fields() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "union_function_port",
        "echo=a5z3 halves=a5,z3 captured=xx\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
