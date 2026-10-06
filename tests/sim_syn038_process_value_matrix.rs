//! SYN-038 process/value coverage for always, always_comb, always_latch, and always_ff.

use crate::sim_cli;

#[test]
fn process_values_match_across_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "process_value_matrix",
        "process=12,abcd,55,a,b2,01 c3,01,12/34,11/22/33/44,a5,1,25 d4,01,01,55/66,1,25,a5 e,01,12/34,1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
