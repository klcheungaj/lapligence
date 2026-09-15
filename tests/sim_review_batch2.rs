//! Source-review regressions. Added without executing builds or tests.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn program_exit_uses_dynamic_origin_and_instance_identity() {
    sim_cli::run_case(
        "review_batch2",
        "program_instance_lifecycle",
        "detached=1 finished=1 unreachable=0 outside=1\n",
        "",
        &[],
    );
}

#[test]
fn last_program_initial_cancels_only_its_detached_descendants() {
    sim_cli::run_case(
        "review_batch2",
        "program_multiple_initials",
        "before=1 after=0 initial=1\n",
        "",
        &[],
    );
}

#[test]
fn program_completion_does_not_drain_pending_renba() {
    sim_cli::run_case(
        "review_batch2",
        "program_immediate_finish",
        "pending=0\n",
        "",
        &[],
    );
}

#[test]
fn postponed_alias_reads_are_side_effect_free() {
    sim_cli::run_case(
        "review_batch2",
        "alias_postponed",
        "strobe=11\nmonitor=00\n",
        "",
        &[],
    );
}

#[test]
fn alias_waiters_wake_at_net_propagation_commit() {
    sim_cli::run_case(
        "review_batch2",
        "alias_net_delay",
        "edge=1@6 level=1@6\n",
        "",
        &[],
    );
}

#[test]
fn mailbox_scalar_mismatches_preserve_values_and_messages() {
    sim_cli::run_case(
        "review_batch2",
        "mailbox_mismatch",
        concat!(
            "empty=0 value=99\n",
            "text=-1 value=unchanged n=1\n",
            "width=-1 value=-2 n=1\n",
            "state=-1 value=-1 n=1\n",
            "sign=-1 value=17 n=1\n",
            "get=1 value=42 n=0\n",
            "real_kind=-1 n=1\n",
            "real=1.25 n=0\n",
        ),
        "",
        &[],
    );
}

#[test]
fn blocking_mailbox_mismatch_fails_instead_of_hanging_or_skipping_fifo() {
    for fixture in ["mailbox_blocking_mismatch", "mailbox_fifo_mismatch"] {
        for optimized in [false, true] {
            let output =
                sim_cli::invoke_with_env("review_batch2", fixture, optimized, &[], &[], &[]);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{fixture}: {stderr}");
            assert!(
                stderr.contains("llg: mailbox retrieval type mismatch"),
                "{fixture}: expected runtime type error, got {stderr}"
            );
            assert!(output.stdout.is_empty(), "{fixture}: {:?}", output.stdout);
        }
    }
}

#[test]
fn concurrent_failures_do_not_duplicate_explicit_actions() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "review_batch2",
            "assertion_failure_actions",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "handled\n");
        assert_eq!(
            stderr.matches("llg: assertion assert failed:").count(),
            1,
            "{stderr}"
        );
        assert_eq!(stderr.matches("EXPLICIT_ERROR").count(), 1, "{stderr}");
        assert!(
            stderr.contains("severity counts: info=0 warning=0 error=2 fatal=0"),
            "{stderr}"
        );
        assert!(
            stderr.contains("assertion counts: assert_failed=4 assume_failed=0 cover=0"),
            "{stderr}"
        );
    }
}

#[test]
fn fork_creation_uses_the_next_parent_draw() {
    sim_cli::run_case(
        "review_batch2",
        "rng_creation",
        "child seeding ok\n",
        "",
        &[],
    );
}
