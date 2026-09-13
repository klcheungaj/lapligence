//! File-based acceptance tests for H20/H22/H23/H24/H25/H26 concurrent
//! assertion sampling, sequence/property composition, clock/control flow,
//! assertion controls, and attempt
//! scheduling. Each fixture is run through both optimizer modes.

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
fn concurrent_assertions_expand_named_sequence_and_property_instances() {
    sim_cli::run_case(
        "concurrent_assertions",
        "property_instances",
        "PROPERTY_INSTANCE_PASS\nDEFAULT_ARGUMENT_PASS\nDISABLE_ARGUMENT_PASS\nPROPERTY_NOT_PASS\nPROPERTY_AND_PASS\nNAMED_COMPOSITION_PASS\nNAMED_OR_PASS\nNAMED_NOT_PASS\nPROPERTY_IFF_PASS\nPROPERTY_IMPLIES_PASS\nSEQUENCE_INSTANCE_PASS\nSEQUENCE_NAMED_ARGS_PASS\nPROPERTY_INSTANCE_PASS\nDEFAULT_ARGUMENT_PASS\nDISABLE_ARGUMENT_PASS\nPROPERTY_NOT_PASS\nPROPERTY_AND_PASS\nNAMED_COMPOSITION_PASS\nNAMED_OR_PASS\nNAMED_NOT_PASS\nPROPERTY_IFF_PASS\nPROPERTY_IMPLIES_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_isolate_local_match_item_state() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h24_local_match",
        "H24_CALL_PASS\nH24_LOCAL_PASS\nH24_LOCAL_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_keep_branch_local_match_state_isolated() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h24_branch_locals",
        "H24_BRANCH_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_block_expect_until_its_endpoint() {
    sim_cli::run_case("concurrent_assertions", "expect", "EXPECT_PASS\n", "", &[]);
}

#[test]
fn concurrent_assertions_expose_sequence_matched_endpoint() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_matched",
        "MATCHED_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_control_named_instances_and_kill_attempts() {
    sim_cli::run_case(
        "concurrent_assertions",
        "assertion_control",
        "ON_PASS\nOFF_REENABLED\nON_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_control_hierarchy_selectors() {
    sim_cli::run_case(
        "concurrent_assertions",
        "assertion_control_hierarchy",
        "HIERARCHY_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_capture_local_formal_defaults() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h24_formal_default",
        "H24_DEFAULT_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_reject_unsupported_named_property_temporal_forms() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_instance",
        "assertion binary operator Until is not supported",
    );
}

#[test]
fn concurrent_assertions_reject_conflicting_named_property_clocks() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_clock_instance",
        "multiple clocks in concurrent assertion",
    );
}

#[test]
fn concurrent_assertions_apply_bounded_abort_and_conditional_controls() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h25_controls",
        "H25_CONDITIONAL_PASS\nH25_ABORT_ACCEPT\nH25_ABORT_ACCEPT\nH25_CONDITIONAL_PASS\nH25_ABORT_ACCEPT\nH25_CONDITIONAL_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_inherit_default_clocking() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h25_default_clock",
        "H25_DEFAULT_CLOCK_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_advance_legal_multiclock_sequence_boundaries() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h25_multiclock",
        "H25_MULTICLOCK_PASS\nH25_MULTICLOCK_ZERO_PASS\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_resolve_abort_control_variants() {
    sim_cli::run_case(
        "concurrent_assertions",
        "h25_abort_variants",
        "H25_SYNC_ACCEPT\nH25_SYNC_ACCEPT\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_execute_reject_control_variants() {
    sim_cli::run_case("concurrent_assertions", "h25_reject_controls", "", "", &[]);
}

#[test]
fn concurrent_assertions_reject_unbounded_cross_clock_delay() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "h25_unsupported_multiclock",
        "multiclocked sequence",
    );
}

#[test]
fn concurrent_assertions_reject_unimplemented_action_controls() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_assertion_control",
        "outside the bounded simulator subset",
    );
}

#[test]
fn concurrent_assertions_reject_unresolved_control_scopes() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_assertion_scope",
        "expected scope or assertion name",
    );
}

#[test]
fn concurrent_assertions_reject_unsupported_control_arguments() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_assertion_argument",
        "bounded $assertcontrol supports only ON, OFF, and KILL",
    );
}

#[test]
fn concurrent_assertions_reject_unsupported_control_levels() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_assertion_level",
        "bounded assertion control supports only level 0",
    );
}
