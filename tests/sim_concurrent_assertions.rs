//! File-based acceptance tests for H20/H22 concurrent assertion sampling,
//! sequence matching, and attempt scheduling. Each fixture is run through
//! both optimizer modes.

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
fn concurrent_assertions_lower_consecutive_repetition() {
    sim_cli::run_case(
        "concurrent_assertions",
        "unsupported_repetition",
        "REPETITION_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_lower_sequence_concatenation() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_concat",
        "CONCAT_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_keep_unbounded_repetition_active() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_unbounded",
        "UNBOUNDED_PASS\nUNBOUNDED_PASS\nUNBOUNDED_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_preserve_sequence_combinator_endpoints() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_combinators",
        "OR_PASS\nAND_PASS\nINTERSECT_PASS\nTHROUGHOUT_PASS\nWITHIN_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_keep_first_match_following_endpoint() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_first_match",
        "FIRST_MATCH_PASS\nFIRST_MATCH_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_allow_nonconsecutive_repetition_gaps() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_nonconsecutive",
        "NONCONSECUTIVE_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_preserve_zero_and_ranged_delays() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_ranges",
        "ZERO_PASS\nRANGED_PASS\n",
        "",
        &[],
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
