//! SYN-038 selected storage and hierarchy routes through the public simulator CLI.

use crate::sim_cli;

#[test]
fn storage_and_hierarchical_routes_run_in_both_optimizer_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "storage_hier_route_remainders",
        "storage_hier_route_remainders=passed\n",
        "llg: $finish at time 2000 at tb:146:9\n",
        &[],
        &["--edition", "2009"],
    );
}
