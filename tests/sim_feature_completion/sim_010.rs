//! SIM-010: fork lifetime, named disable and capture cleanup. Oracles are
//! derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_010";

#[test]
fn fork_block_declarations_are_created_per_fork_execution() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_010/fork_declarations.out");
    sim_cli::run_case_backend_parity(SUITE, "fork_declarations", expected, &[], &[]);
}
