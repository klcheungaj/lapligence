//! SYN-038 typed constant, initializer, return, and event source paths.

use crate::sim_cli;

#[test]
fn typed_constant_event_paths_keep_exact_values_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "typed_constant_event_matrix",
        "widths=3,5,5,4 constfunc=5,6,7 const=4/12,3/0d runtime=5/17,4/0b static=6/1a,2/09 return=5aa5 overrides=2/1a,7/03,4/12 nested=51,62,73,84,31,42 events=1,1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
