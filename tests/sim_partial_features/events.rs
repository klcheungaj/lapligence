use super::run_case;

#[test]
fn effectful_event_expressions_and_qualifiers_are_process_evaluated() {
    // A constant result never triggers even though the helper keeps writing.
    run_case(
        "event_effectful_expression",
        "result unchanged evaluated=1 clk=x\n",
    );
    // A qualifier runs once, when its edge is detected.
    run_case("event_effectful_condition", "qualified calls=1 clk=0\n");
}

#[test]
fn effectful_event_helpers_publish_their_visible_writes() {
    super::sim_cli::run_case(
        "feature_completion/g1_06",
        "effects_impure_helper",
        "unchanged evaluated=1\n",
        "",
        &[],
    );
}

#[test]
fn vector_edges_use_only_the_least_significant_bit() {
    run_case("event_vector_lsb", "3 3 1\n");
}

#[test]
fn constant_false_wait_suspends_without_blocking_time_advance() {
    for edition in ["2001", "2009"] {
        super::sim_cli::run_case_with_args(
            "partial_features",
            "wait_constant_false",
            "ready 0\nlater 3000\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn selected_expression_edges_follow_four_state_transitions() {
    run_case("event_selected_xz", "2 2 5\n");
}

#[test]
fn duplicate_named_event_clauses_and_disabled_waits_release_registrations() {
    run_case("event_cleanup", "3\n");
}

#[test]
fn edge_iff_is_sampled_at_the_edge_not_when_the_waiter_runs() {
    run_case("event_iff", "rejected 0\naccepted 1\nunknown 1\n");
}

#[test]
fn event_expression_changes_only_when_its_value_changes() {
    run_case(
        "event_expression",
        "same 0 0\nrise 1 1\nfall 2 1\nsame 2 1\n",
    );
}

#[test]
fn mixed_qualified_events_remain_one_atomic_wait() {
    run_case(
        "mixed_iff_events",
        "filtered 0\nclock 1\nevent 2\nfiltered 2\n",
    );
}

#[test]
fn event_handles_triggered_state_and_wait_order_follow_identity() {
    run_case(
        "event_h15",
        "aliases=1/1 null=0 stale=1/1 queued=1 order=1/1 triggered=1/1/1 ordinary=0\n",
    );
}
