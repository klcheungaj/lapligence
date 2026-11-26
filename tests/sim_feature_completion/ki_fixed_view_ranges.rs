//! Fixed arrays of containers whose declared range does not start at 0
//! ([readme](../fixtures/sim/feature_completion/ki_fixed_view_ranges/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_fixed_view_ranges";

#[test]
fn every_access_maps_the_index_by_the_declared_range() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "containers",
        include_str!("../fixtures/sim/feature_completion/ki_fixed_view_ranges/containers.out"),
        &[],
        &[],
    );
}

#[test]
fn module_and_subroutine_arrays_of_containers_are_views() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "declarations",
        include_str!("../fixtures/sim/feature_completion/ki_fixed_view_ranges/declarations.out"),
        &[],
        &[],
    );
}

#[test]
fn real_elements_of_arrays_of_containers_read_as_reals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_elements",
        include_str!("../fixtures/sim/feature_completion/ki_fixed_view_ranges/real_elements.out"),
        &[],
        &[],
    );
}
