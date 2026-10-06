//! SIM-015: process handles in collections, formals, members and copies.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_015";

#[test]
fn collection_handles_are_controlled_independently() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "collections",
        include_str!("../fixtures/sim/feature_completion/sim_015/collections.out"),
        &[],
        &[],
    );
}

#[test]
fn process_formals_of_every_direction_and_results() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "formals",
        include_str!("../fixtures/sim/feature_completion/sim_015/formals.out"),
        &[],
        &[],
    );
}

#[test]
fn class_properties_and_record_members_share_identity() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "members",
        include_str!("../fixtures/sim/feature_completion/sim_015/members.out"),
        &[],
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "members",
        include_str!("../fixtures/sim/feature_completion/sim_015/members.out"),
    );
}

#[test]
fn control_is_independent_of_the_wait_kind() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "wait_kinds",
        include_str!("../fixtures/sim/feature_completion/sim_015/wait_kinds.out"),
        &[],
        &[],
    );
}

#[test]
fn self_kill_repeated_control_and_terminal_await() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "terminal_cases",
        include_str!("../fixtures/sim/feature_completion/sim_015/terminal_cases.out"),
        &[],
        &[],
    );
}

#[test]
fn task_process_variables_and_formals_are_shared_with_forks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "shared_handles",
        include_str!("../fixtures/sim/feature_completion/sim_015/shared_handles.out"),
        &[],
        &[],
    );
}

#[test]
fn random_state_methods_act_on_the_named_process() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "randstate",
        include_str!("../fixtures/sim/feature_completion/sim_015/randstate.out"),
        &[],
        &[],
    );
}

#[test]
fn retained_handles_survive_model_teardown() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "teardown",
        include_str!("../fixtures/sim/feature_completion/sim_015/teardown.out"),
        &[],
        &[],
    );
}

#[test]
fn adopted_randstate_witness() {
    sim_cli::run_case_backend_parity(SUITE, "process_randstate", "1\n", &[], &[]);
}

#[test]
fn kill_during_wake_delivery_has_permitted_outcomes() {
    sim_cli::run_case_checked_matrix(SUITE, "wake_kill", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        // SV 4.7 lets the triggering process be interleaved after `->go`;
        // either the victim never runs or it finishes before the kill.
        let victim = ["victim KILLED\n", "victim ran\nvictim FINISHED\n"];
        let pair = [
            "a woke\npair FINISHED KILLED\n",
            "b woke\npair KILLED FINISHED\n",
        ];
        assert!(
            victim.iter().any(|first| {
                pair.iter()
                    .any(|second| stdout == format!("{first}{second}"))
            }),
            "{label}: {stdout}"
        );
    });
}

#[test]
fn null_handle_method_ends_the_simulation() {
    sim_cli::run_case_checked_matrix(SUITE, "neg_null_await", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "before\n",
            "{label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with("llg: null process handle access: process::await\n"),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn self_await_is_an_error() {
    sim_cli::run_case_checked_matrix(SUITE, "neg_self_await", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "before\n",
            "{label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with("llg: process::await called on the current process\n"),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn random_state_of_a_terminated_process_is_reported() {
    sim_cli::run_case_checked_matrix(SUITE, "neg_randstate_terminated", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "status FINISHED\n",
            "{label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with("llg: random runtime: process::srandom on a terminated process\n"),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn ref_formal_bound_to_plain_handle_storage_is_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_property",
        "ref actual of process formal `p` in `tb` must be a process variable (SIM-015)",
    );
}

#[test]
fn output_formal_in_an_expression_call_is_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_expression_output",
        "output or inout process formal `p` bound to a process variable in a call inside an expression in `tb` is not supported; call the subroutine as a statement (SIM-015)",
    );
}
