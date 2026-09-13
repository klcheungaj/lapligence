use super::run_case;

#[test]
fn event_callbacks_reject_function_side_effects_before_emission() {
    for fixture in ["event_effectful_expression", "event_effectful_condition"] {
        super::reject_case(fixture, "function calls in evaluated event controls");
    }
}

#[test]
fn vector_edges_use_only_the_least_significant_bit() {
    run_case("event_vector_lsb", "3 3 1\n");
}

#[test]
fn constant_false_wait_suspends_without_blocking_time_advance() {
    run_case("wait_constant_false", "ready 0\nlater 3000\n");
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
