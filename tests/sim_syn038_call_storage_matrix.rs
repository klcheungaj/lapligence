//! SYN-038 task/function actuals across storage lifetimes and access routes.

use crate::sim_cli;

#[test]
fn call_actual_storage_and_routes_match_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "call_storage_matrix",
        "calls=12,43,13,44,25,2d,55 iface=11,41,13,2c,42,12 hier=33,6f\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
