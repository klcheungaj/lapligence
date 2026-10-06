//! Selected typed source and target paths for the SYN-038 remainder matrix.

use crate::sim_cli;

#[test]
fn typed_context_remainders_keep_exact_values_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_context_remainders",
        "const=2,5,6,4,2 op=1234,0,2143,11223344,556677,1 init=5a6b,21,11,55,77,64 target=20,60,81,92 events=1,1 row=3344 override=62,82,43 extra=1,0,1,c0de\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
