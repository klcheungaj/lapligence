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
        "INFO: storage_hier_route_remainders.sv:146:9: [SIM-001] (tb @ 2 ns) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}
