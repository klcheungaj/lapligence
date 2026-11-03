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
fn container_properties_and_mailbox_counts_wake_waits() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/observed_storage.out");
    sim_cli::run_case_backend_parity(SUITE, "observed_storage", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "observed_storage", expected);
}

#[test]
fn callee_locals_interface_members_and_containers_wake_waits() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/callee_dataflow.out");
    sim_cli::run_case_backend_parity(SUITE, "callee_dataflow", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "callee_dataflow", expected);
}

#[test]
fn neg_callee_alias() {
    // Implementation restriction: a callee local that holds one of two
    // handles cannot be re-evaluated when the wait arms; rejected instead of
    // never waking.
    sim_cli::reject_case(
        SUITE,
        "neg_callee_alias",
        "class property `x` read by a called subroutine through a handle other than",
    );
}

#[test]
fn neg_container_property() {
    // Implementation restriction: the queue a callee reads through a local
    // holding one of two handles cannot be named when the wait arms.
    sim_cli::reject_case(
        SUITE,
        "neg_container_property",
        "container class property `q` read by a called subroutine through a handle other than",
    );
}

#[test]
fn neg_mailbox_query() {
    // Implementation restriction: only a mailbox num() publishes a change;
    // a semaphore try_get() does not.
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

#[test]
fn every_handle_writer_publishes_its_change() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/handle_writers.out");
    sim_cli::run_case_backend_parity(SUITE, "handle_writers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "handle_writers", expected);
}

#[test]
fn suspended_waiters_stay_queue_candidates() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/ki_lrm_audit_b/suspended_queue_waiters.out"
    );
    sim_cli::run_case_backend_parity(SUITE, "suspended_queue_waiters", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "suspended_queue_waiters", expected);
}

#[test]
fn disabling_a_whole_branch_block_kills_the_branch() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_lrm_audit_b/disable_branch_status.out");
    sim_cli::run_case_backend_parity(SUITE, "disable_branch_status", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "disable_branch_status", expected);
}
