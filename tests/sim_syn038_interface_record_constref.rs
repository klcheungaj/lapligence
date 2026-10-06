//! SYN-038 public-CLI evidence for interface record input through const ref.

use crate::sim_cli;

#[test]
fn interface_record_const_ref_function_returns_the_source_key() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_record_constref",
        "read=3c\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
