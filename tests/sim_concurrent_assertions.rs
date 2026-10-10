//! File-based acceptance tests for H20/H22/H23/H24/H25/H26 concurrent
//! assertion sampling, sequence/property composition, clock/control flow,
//! assertion controls, and attempt
//! scheduling. Each fixture is run through both optimizer modes.

use crate::sim_cli;

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
    // The completed pass-action process is reclaimed before `$finish`, leaving
    // the clock and initial processes registered.
    sim_cli::run_case(
        "concurrent_assertions",
        "vacuity",
        "VACUOUS_PASS\n",
        "llg: $finish at time 2000 at tb:15:12\nllg: simulation statistics: processes=2\nllg: assertion vacuous=1\n",
        &[],
    );
}

/// Run `action_retention` for `cycles` posedges on every value backend and
/// return the `$finish(2)` process statistic per backend/optimizer label.
fn action_retention_processes(cycles: u64) -> std::collections::BTreeMap<String, u64> {
    let define = format!("CYCLES={cycles}");
    // Hand-derived: the sampled count at posedge k is k-1, so every attempt
    // passes (three vacuously) and `$rose(count[0])` matches at even k.
    let stdout = format!("passes={cycles} fails=0 covers={}\n", cycles / 2);
    let finish = format!("llg: $finish at time {} at tb:31:9\n", 2000 * cycles);
    let tail = format!(
        "llg: assertion counts: assert_failed=0 assume_failed=0 cover={}\nllg: assertion vacuous=3\n",
        cycles / 2
    );
    let counts = std::cell::RefCell::new(std::collections::BTreeMap::new());
    sim_cli::run_case_checked_matrix(
        "concurrent_assertions",
        "action_retention",
        &["--define", &define],
        &|label, output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{label}: {stderr}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
            let rest = stderr
                .strip_prefix(finish.as_str())
                .and_then(|rest| rest.strip_prefix("llg: simulation statistics: processes="))
                .unwrap_or_else(|| panic!("{label}: unexpected stderr {stderr:?}"));
            let (count, rest) = rest.split_once('\n').expect("statistics line");
            assert_eq!(rest, tail, "{label}");
            counts
                .borrow_mut()
                .insert(label.to_owned(), count.parse().expect("process count"));
        },
    );
    counts.into_inner()
}

#[test]
fn concurrent_assertions_release_completed_actions_and_history() {
    // 4x the cycles runs 4x the action processes; reclaimed processes leave
    // the registry exactly as large, and only the three static processes live.
    let short = action_retention_processes(50);
    let long = action_retention_processes(200);
    assert!(!short.is_empty());
    assert_eq!(short, long);
    assert!(short.values().all(|count| *count <= 3), "{short:?}");
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
    // At the second edge the first attempt matches, and the new attempt
    // succeeds vacuously because `first` is false. Both run the pass action.
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_concat",
        "CONCAT_PASS\nCONCAT_PASS\n",
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
    // One nonvacuous completion and false antecedents at the second and
    // third edges each execute the assertion's pass action.
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_first_match",
        "FIRST_MATCH_PASS\nFIRST_MATCH_PASS\nFIRST_MATCH_PASS\n",
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
    // The first edge matches `zero`. The second matches the pending ranged
    // attempt and starts one vacuous success for each assertion.
    sim_cli::run_case(
        "concurrent_assertions",
        "sequence_ranges",
        "ZERO_PASS\nZERO_PASS\nRANGED_PASS\nZERO_PASS\nRANGED_PASS\nRANGED_PASS\n",
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
fn concurrent_assertions_expect_runs_one_attempt_from_an_expect_only_block() {
    sim_cli::run_case(
        "concurrent_assertions",
        "expect_single_attempt",
        "PREDICATE_FAIL 2\nSEQUENCE_FAIL 3\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_gate_their_clock_by_iff() {
    sim_cli::run_case(
        "concurrent_assertions",
        "clock_iff",
        "LATE 20000\nEXPLICIT 30000\nLATE 30000\nSAMPLED 30000\n",
        "",
        &[],
    );
}

#[test]
fn concurrent_assertions_reject_nested_iff_clocks() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "neg_nested_clock_iff",
        "an `iff`-qualified clock inside a concurrent assertion sequence is not supported unless it is the leading clocking event",
    );
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
        "H25_MULTICLOCK_ZERO_PASS\nH25_MULTICLOCK_PASS\n",
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
fn concurrent_assertions_reject_post_2009_control_builtin() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_assertion_argument",
        "is not available in IEEE 2009",
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
