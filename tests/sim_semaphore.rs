//! H17 semaphore acceptance tests.
//!
//! The checked-in fixtures exercise the IEEE 1800-2009 semaphore constructor,
//! key-count operations, strict FIFO contention, and removal of a blocked get
//! when its process is killed.  The shared CLI harness runs each case with and
//! without optimizer passes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn semaphore_zero_key_and_try_get_results_are_exact() {
    sim_cli::run_case(
        "semaphore",
        "basic",
        "zero=0 ztry=1\nfirst=1\nempty=0\nafter_put=1\nzero_put=1\n",
        "llg: $finish at time 0 at tb:22:9\n",
        &[],
    );
}

#[test]
fn semaphore_contention_keeps_differing_requests_in_fifo_order() {
    sim_cli::run_case(
        "semaphore",
        "fifo",
        "zero_try=1\nwide\nnarrow\n",
        "llg: $finish at time 0 at tb:29:9\n",
        &[],
    );
}

#[test]
fn killing_a_blocked_semaphore_waiter_does_not_consume_a_later_put() {
    sim_cli::run_case(
        "semaphore",
        "kill",
        "recovered=1\n",
        "llg: $finish at time 0 at tb:24:9\n",
        &[],
    );
}

#[test]
fn semaphore_task_arguments_survive_a_blocking_get() {
    sim_cli::run_case(
        "semaphore",
        "task_arg",
        "task_arg=1\n",
        "llg: $finish at time 0 at tb:25:9\n",
        &[],
    );
}

#[test]
fn semaphore_automatic_locals_survive_a_captured_blocking_get() {
    sim_cli::run_case(
        "semaphore",
        "local",
        "local=1\n",
        "llg: $finish at time 0 at tb:23:9\n",
        &[],
    );
}

#[test]
fn semaphore_static_procedural_initializers_run_once() {
    sim_cli::run_case(
        "semaphore",
        "static",
        "static_try=0\n",
        "llg: $finish at time 1 at tb:18:13\n",
        &[],
    );
}

#[test]
fn semaphore_wake_waits_for_a_suspended_process_to_resume() {
    sim_cli::run_case(
        "semaphore",
        "suspend",
        "waiting=2 suspended=3 woken=3 acquired=1 done=0\n",
        "",
        &[],
    );
}

#[test]
fn semaphore_locals_can_be_assigned_after_declaration() {
    sim_cli::run_case("semaphore", "assign", "assigned=1\n", "", &[]);
}

#[test]
fn semaphore_function_returns_can_be_used_as_handles() {
    sim_cli::run_case("semaphore", "function_return", "returned=1\n", "", &[]);
}

#[test]
fn semaphore_rejects_a_negative_key_count_at_runtime() {
    sim_cli::reject_case(
        "semaphore",
        "invalid_count",
        "semaphore key count must be a known nonnegative integral value",
    );
}

#[test]
fn semaphore_automatic_task_locals_are_constructed_and_released() {
    sim_cli::run_case("semaphore", "task_local", "task_local=1\n", "", &[]);
}

#[test]
fn semaphore_null_initializers_preserve_a_null_handle() {
    sim_cli::run_case("semaphore", "null", "null=1\n", "", &[]);
}

#[test]
fn cancelling_the_head_grants_existing_keys_without_another_put() {
    sim_cli::run_case(
        "semaphore",
        "cancel_head",
        "acquired=1 cancelled_after=0 remaining=0\n",
        "",
        &[],
    );
}

#[test]
fn tree_cancellation_does_not_grant_keys_to_another_cancelled_child() {
    sim_cli::run_case(
        "semaphore",
        "cancel_tree",
        "survivor=1 escaped=0 remaining=0\n",
        "",
        &[],
    );
}

#[test]
fn named_disable_services_the_next_semaphore_waiter() {
    sim_cli::run_case(
        "semaphore",
        "cancel_named",
        "named acquired=1 continued=1 escaped=0 remaining=0\n",
        "",
        &[],
    );
}

#[test]
fn disable_fork_services_unrelated_semaphore_waiters() {
    sim_cli::run_case(
        "semaphore",
        "cancel_fork",
        "fork acquired=1 escaped=0 remaining=0\n",
        "",
        &[],
    );
}
