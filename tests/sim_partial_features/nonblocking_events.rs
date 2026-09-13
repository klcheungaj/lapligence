use super::run_case;

#[test]
fn nonblocking_named_event_triggers_preserve_direct_and_delay_semantics() {
    run_case(
        "nonblocking_event_direct_delay",
        "CHECK: direct stage=1\nCHECK: delayed time=2\n",
    );
}

#[test]
fn nonblocking_event_controls_register_at_issue_time() {
    run_case(
        "nonblocking_event_triggers",
        "CHECK: controlled caller=running\nCHECK: repeated caller=running\nCHECK: deferred stage=1\nCHECK: immediate woke=0\nCHECK: controlled time=1000\nCHECK: delayed time=2000\nCHECK: mixed wakes=1\nCHECK: repeated time=3000\n",
    );
}

#[test]
fn nonblocking_event_repeat_registers_dynamic_count() {
    run_case(
        "nonblocking_event_repeat_dynamic",
        "CHECK: repeated count time=2000\n",
    );
}

#[test]
fn zero_repeat_nonblocking_event_still_schedules_the_target() {
    run_case(
        "nonblocking_event_repeat_zero",
        "CHECK: zero repeat time=0\nCHECK: negative repeat time=0\nCHECK: unknown repeat time=0\n",
    );
}

#[test]
fn nonblocking_event_triggers_preserve_nba_order_and_same_slot_wakes() {
    run_case(
        "nonblocking_event_order",
        "CHECK: direct marker=2\nCHECK: first marker=2\nCHECK: second marker=2\nCHECK: mixed once=2\nCHECK: qualified once=2\n",
    );
}

#[test]
fn nonblocking_event_trigger_keeps_hierarchical_source_identity() {
    run_case(
        "nonblocking_event_hierarchy",
        "CHECK: caller=running\nCHECK: hierarchy time=1000\n",
    );
}
