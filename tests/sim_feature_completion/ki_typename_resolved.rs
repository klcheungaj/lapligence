//! `$typename` of user-defined types (see the fixture readme).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_typename_resolved";

#[test]
fn user_defined_types_are_named_by_their_declarations() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/ki_typename_resolved/typename_resolved.out"
    );
    sim_cli::run_case_backend_parity(SUITE, "typename_resolved", expected, &[], &[]);
}
