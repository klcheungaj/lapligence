//! Portable LRM decision cases (`tests/fixtures/sim/lrm_decisions/`): one
//! plain IEEE 1800-2009 source per decision, with a header that quotes the
//! clause and states the decision, and the exact expected stdout. The same
//! sources are meant to be rerun on other simulators; here each runs through
//! the public CLI in both optimizer modes on the legacy and compact value
//! backends.
use crate::sim_cli;

const SUITE: &str = "lrm_decisions";

fn check(case: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, case, expected, &[], &[]);
}

#[test]
fn b1_string_property_wait() {
    check(
        "B1_string_property_wait",
        include_str!("fixtures/sim/lrm_decisions/B1_string_property_wait.out"),
    );
}

#[test]
fn b2_function_member_wait() {
    check(
        "B2_function_member_wait",
        include_str!("fixtures/sim/lrm_decisions/B2_function_member_wait.out"),
    );
}

#[test]
fn b3_resume_resensitize() {
    check(
        "B3_resume_resensitize",
        include_str!("fixtures/sim/lrm_decisions/B3_resume_resensitize.out"),
    );
}
