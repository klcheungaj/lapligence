//! SYN-038 public-CLI matrix for subroutine formal modes across process families.

use crate::sim_cli;

#[test]
fn all_formal_modes_execute_from_each_process_family() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "formal_process_matrix",
        concat!(
            "always=8,9,3,14,5\n",
            "comb=8,9,3,24,5\n",
            "latch=8,9,3,34,5\n",
            "ff=8,9,3,44,5\n",
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}
