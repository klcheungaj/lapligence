//! File-based acceptance tests for H20 concurrent assertion sampling and
//! attempt scheduling.  Each fixture is run through both optimizer modes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn concurrent_assertions_sample_before_nba_updates() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sampling_nba",
        "SAMPLED_PREPONED\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_disable_aborts_pending_attempts() {
    sim_cli::run_case(
        "concurrent_assertions",
        "disable_async",
        "DISABLE_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_preserve_overlapping_attempt_order() {
    sim_cli::run_case(
        "concurrent_assertions",
        "overlap_order",
        "OVERLAP_PASS\nOVERLAP_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_drop_pending_attempts_at_end_of_simulation() {
    sim_cli::run_case("concurrent_assertions", "end_pending", "", "", &[]);
}

#[test]
fn concurrent_assertions_account_for_vacuous_successes() {
    sim_cli::run_case(
        "concurrent_assertions",
        "vacuity",
        "VACUOUS_PASS\n",
        "llg: $finish at time 2000 at tb:15:12\nllg: simulation statistics: processes=3\nllg: assertion vacuous=1\n",
        &[],
    );
}

#[test]
fn concurrent_assertions_reject_unsupported_repetition() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_repetition",
        "sequence repetition is not supported",
    );
}

#[test]
fn concurrent_assertions_reject_named_property_instances_fail_closed() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_instance",
        "assertion instances with formal bindings are not supported",
    );
}
