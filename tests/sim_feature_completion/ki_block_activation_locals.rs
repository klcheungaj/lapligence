//! Automatic block strings, handles, events and records captured by a fork
//! that runs again ([readme](../fixtures/sim/feature_completion/ki_block_activation_locals/readme.md)).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_block_activation_locals";

#[test]
fn each_block_activation_shares_its_strings_handles_and_events_with_its_forks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "forks",
        include_str!("../fixtures/sim/feature_completion/ki_block_activation_locals/forks.out"),
        &[],
        &[],
    );
}

#[test]
fn each_block_activation_shares_its_records_with_its_forks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "records",
        include_str!("../fixtures/sim/feature_completion/ki_block_activation_locals/records.out"),
        &[],
        &[],
    );
}
