//! SIM-017: mailbox aggregate messages, handle messages, selected and
//! automatic destinations, cancellation and re-entry. Oracles are derived by
//! hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_017";

#[test]
fn record_messages_copy_values_and_share_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "records",
        include_str!("../fixtures/sim/feature_completion/sim_017/records.out"),
        &[],
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "records",
        include_str!("../fixtures/sim/feature_completion/sim_017/records.out"),
    );
}

#[test]
fn queue_and_dynamic_array_messages_are_deep_copies() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "containers",
        include_str!("../fixtures/sim/feature_completion/sim_017/containers.out"),
        &[],
        &[],
    );
}

#[test]
fn unpacked_array_messages_from_patterns() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "array_patterns",
        include_str!("../fixtures/sim/feature_completion/sim_017/array_patterns.out"),
        &[],
        &[],
    );
}

#[test]
fn handle_messages_keep_object_identity() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "handles",
        include_str!("../fixtures/sim/feature_completion/sim_017/handles.out"),
        &[],
        &[],
    );
}

#[test]
fn try_variants_preserve_message_and_destination() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "try_variants",
        include_str!("../fixtures/sim/feature_completion/sim_017/try_variants.out"),
        &[],
        &[],
    );
}

#[test]
fn selected_destinations_freeze_selectors_at_the_call() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "selected",
        include_str!("../fixtures/sim/feature_completion/sim_017/selected.out"),
        &[],
        &[],
    );
}

#[test]
fn mailbox_handles_in_members_arrays_and_formals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "members",
        include_str!("../fixtures/sim/feature_completion/sim_017/members.out"),
        &[],
        &[],
    );
}

#[test]
fn killed_receivers_and_senders_neither_lose_nor_duplicate_messages() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cancellation",
        include_str!("../fixtures/sim/feature_completion/sim_017/cancellation.out"),
        &[],
        &[],
    );
}

/// llg policy, not a conformance oracle: IEEE 1800-2009 9.7 L12632-12634
/// terminates a process that is not blocked "at some unspecified time in the
/// current time step", so a woken receiver may or may not take its message
/// before the kill. llg returns the message to the head (see the readme).
#[test]
fn llg_policy_woken_receiver_kill_returns_its_message() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "policy_woken_kill",
        include_str!("../fixtures/sim/feature_completion/sim_017/policy_woken_kill.out"),
        &[],
        &[],
    );
}

#[test]
fn woken_processes_reenter_the_mailbox() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "reentry",
        include_str!("../fixtures/sim/feature_completion/sim_017/reentry.out"),
        &[],
        &[],
    );
}

#[test]
fn adopted_mailbox_witnesses() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "mailbox_aggregate",
        include_str!("../fixtures/sim/feature_completion/sim_017/mailbox_aggregate.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "mailbox_auto_capture",
        include_str!("../fixtures/sim/feature_completion/sim_017/mailbox_auto_capture.out"),
        &[],
        &[],
    );
}

#[test]
fn blocking_get_type_mismatch_ends_the_simulation() {
    sim_cli::run_case_checked_matrix(SUITE, "neg_blocking_mismatch", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "before\n",
            "{label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("llg: mailbox retrieval type mismatch\n"),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn associative_array_messages_are_explicit_rejections() {
    sim_cli::reject_case(
        SUITE,
        "neg_assoc_message",
        "mailbox message of type `int$[string]` in `tb` is not supported: the message is an associative array, which has no nested value form (SIM-017)",
    );
}

#[test]
fn whole_array_variable_messages_are_explicit_rejections() {
    sim_cli::reject_case(
        SUITE,
        "neg_array_variable",
        "a whole array variable operand is not supported (SIM-017)",
    );
}

#[test]
fn record_try_get_in_a_loop_condition_is_an_explicit_rejection() {
    sim_cli::reject_case(
        SUITE,
        "neg_loop_condition",
        "mailbox `try_get` with an aggregate or selected operand in `tb` is supported only as a statement, in an assignment, a system-task argument or an if condition (SIM-017)",
    );
}
