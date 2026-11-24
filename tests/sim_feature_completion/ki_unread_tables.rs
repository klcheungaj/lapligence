//! Objects that nothing reads (`tests/fixtures/sim/feature_completion/ki_unread_tables/readme.md`).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_unread_tables";

#[test]
fn objects_only_typename_names_run_without_unread_tables() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_unread_tables/unread_tables.out");
    sim_cli::run_case_backend_parity(SUITE, "unread_tables", expected, &[], &[]);
}
