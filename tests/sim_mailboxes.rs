//! End-to-end coverage for the bounded mailbox synchronization subset.
//!
//! Every fixture is lowered and executed with and without optimization. The
//! fixtures intentionally exercise the runtime's FIFO value/handle ownership
//! and coroutine waiter paths instead of depending on host-side mocks.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn typed_and_untyped_mailboxes_preserve_fifo_values_and_handle_identity() {
    sim_cli::run_case(
        "mailboxes",
        "basic",
        concat!(
            "start=0\n",
            "put1=1 n=1\n",
            "put2=1 n=2\n",
            "full=0 n=2\n",
            "peek=10 n=2\n",
            "fifo=10,20 n=0\n",
            "empty=0\n",
            "strput=1\n",
            "str=mailbox n=0\n",
            "objput=1\n",
            "objget=1 same=1 id=77 n=0\n",
            "mismatch=-1 n=1\n",
            "preserved=99 n=0\n",
        ),
        "llg: $finish at time 0 at tb:48:9\n",
        &[],
    );
}

#[test]
fn blocking_mailbox_waiters_handoff_in_fifo_order_and_peek_is_nonconsuming() {
    sim_cli::run_case(
        "mailboxes",
        "blocking",
        concat!(
            "consumer1=1 n=1\n",
            "producer2 n=1\n",
            "consumer2=2 n=1\n",
            "producer3 n=1\n",
            "consumer3=3 n=0\n",
            "peek1=8 n=0\n",
            "peek2=8 n=0\n",
            "get=8 n=0\n",
            "observed_final=0\n",
        ),
        "llg: $finish at time 4000 at tb:53:9\n",
        &[],
    );
}

#[test]
fn killed_mailbox_waiters_release_pending_storage_and_keep_queue_state() {
    sim_cli::run_case(
        "mailboxes",
        "cancellation",
        concat!(
            "get_killed n=0\n",
            "after_get_kill n=1\n",
            "put_killed n=1 try=0\n",
            "retained=5 n=0\n",
        ),
        "llg: $finish at time 2000 at tb:30:9\n",
        &[],
    );
}

#[test]
fn mailbox_element_descriptors_preserve_four_state_real_and_typedef_values() {
    sim_cli::run_case(
        "mailboxes",
        "typed_values",
        concat!(
            "byte_try=1\n",
            "byte=x5 n=1\n",
            "byte_get=1\n",
            "state=1 n=0\n",
            "real=1.250000 n=1\n",
            "short=1.234568 n=0\n",
        ),
        "llg: $finish at time 0 at tb:34:9\n",
        &[],
    );
}

#[test]
fn nested_and_automatic_mailbox_locals_have_fresh_storage() {
    sim_cli::run_case(
        "mailboxes",
        "locals",
        concat!("nested=12 n=0\n", "task=21 n=0\n", "task=34 n=0\n",),
        "llg: $finish at time 0 at tb:23:9\n",
        &[],
    );
}
