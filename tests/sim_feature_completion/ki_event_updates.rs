//! Nonblocking assignments under an event control
//! ([readme](../fixtures/sim/feature_completion/ki_event_updates/readme.md)).

use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_event_updates";

#[test]
fn a_clocking_event_queues_updates() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "clocking",
        include_str!("../fixtures/sim/feature_completion/ki_event_updates/clocking.out"),
        &[],
        &[],
    );
}

#[test]
fn real_arrays_take_timed_updates_and_controls_read_triggered() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_arrays",
        include_str!("../fixtures/sim/feature_completion/ki_event_updates/real_arrays.out"),
        &[],
        &[],
    );
}

#[test]
fn a_captured_loop_variable_ends_with_its_last_write() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "loop_variable",
        include_str!("../fixtures/sim/feature_completion/ki_event_updates/loop_variable.out"),
        &[],
        &[],
    );
}

#[test]
fn a_wait_holds_the_event_it_started_on() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_objects",
        include_str!("../fixtures/sim/feature_completion/ki_event_updates/event_objects.out"),
        &[],
        &[],
    );
}

#[test]
fn a_null_event_in_a_control_ends_the_run_where_the_statement_is() {
    for form in ["FORM=0", "FORM=1", "FORM=2", "FORM=3"] {
        sim_cli::run_case_checked_matrix(SUITE, "null_event", &["-D", form], &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{form} {label}: {output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                include_str!("../fixtures/sim/feature_completion/ki_event_updates/null_event.out"),
                "{form} {label}"
            );
            assert!(
                sim_harness::mentions(
                    &String::from_utf8_lossy(&output.stderr),
                    "null event handle access"
                ),
                "{form} {label}: {output:?}"
            );
        });
    }
}
