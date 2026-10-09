//! LRM audit part B fixes: evaluated-event dependencies of string and handle
//! class properties and of class storage read by called functions and
//! methods. Oracles are derived by hand in the fixture readme from IEEE
//! 1800-2009 §§9.4.2 and 9.4.3.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_lrm_audit_b";

#[test]
fn string_and_handle_properties_wake_waits_and_events() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/string_props.out");
    sim_cli::run_case_backend_parity(SUITE, "string_props", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "string_props", expected);
}

#[test]
fn class_storage_read_by_callees_wakes_waits_and_events() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/callee_reads.out");
    sim_cli::run_case_backend_parity(SUITE, "callee_reads", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "callee_reads", expected);
}

#[test]
fn neg_callee_alias() {
    // Implementation restriction: a handle copied into a callee local cannot
    // be re-evaluated when the wait arms; rejected instead of never waking.
    sim_cli::reject_case(
        SUITE,
        "neg_callee_alias",
        "class property `x` read by a called subroutine through a handle other than",
    );
}

#[test]
fn neg_container_property() {
    // Implementation restriction: per-object containers publish no change.
    sim_cli::reject_case(
        SUITE,
        "neg_container_property",
        "container class property `q`",
    );
}

#[test]
fn neg_mailbox_query() {
    // Implementation restriction: mailbox state publishes no change.
    sim_cli::reject_case(
        SUITE,
        "neg_mailbox_query",
        "a mailbox or semaphore method call",
    );
}

#[test]
fn neg_virtual_method() {
    // Implementation restriction: the dispatched implementation's reads are
    // unknown when the wait arms.
    sim_cli::reject_case(
        SUITE,
        "neg_virtual_method",
        "the storage a virtual method reads",
    );
}

#[test]
fn resume_resensitizes_event_controls_and_completes_satisfied_waits() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/resume_resensitize.out");
    sim_cli::run_case_backend_parity(SUITE, "resume_resensitize", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "resume_resensitize", expected);
}
