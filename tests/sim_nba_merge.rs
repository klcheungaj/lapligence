//! FIN-001 O2: overridden nonblocking assignments that lowering merges
//! (`src/sim/codegen/lowering/statements/nba_merge.rs`) keep every output.
//! The expected outputs are the industry-standard results for the same
//! sources (fixture readme); both run in both optimizer modes.

use crate::sim_cli;

const SUITE: &str = "nba_merge";

#[test]
fn overridden_updates_queue_the_last_value() {
    sim_cli::run_case(
        SUITE,
        "overridden_updates",
        include_str!("fixtures/sim/nba_merge/overridden_updates.out"),
        "",
        &[],
    );
}

#[test]
fn observed_updates_report_the_last_value() {
    sim_cli::run_case(
        SUITE,
        "observed_updates",
        include_str!("fixtures/sim/nba_merge/observed_updates.out"),
        "",
        &[],
    );
}
