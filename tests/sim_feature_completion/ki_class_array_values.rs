//! Whole uses of fixed arrays that are class properties
//! ([readme](../fixtures/sim/feature_completion/ki_class_array_values/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_class_array_values";

#[test]
fn integral_arrays_are_reduced_ordered_copied_and_compared() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "integral",
        include_str!("../fixtures/sim/feature_completion/ki_class_array_values/integral.out"),
        &[],
        &[],
    );
}

#[test]
fn real_and_event_arrays_are_used_through_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_events",
        include_str!("../fixtures/sim/feature_completion/ki_class_array_values/real_events.out"),
        &[],
        &[],
    );
}
