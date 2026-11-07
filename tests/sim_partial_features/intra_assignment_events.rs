use super::run_case_with_stderr;

#[test]
fn intra_assignment_event_controls_capture_values_and_indices() {
    run_case_with_stderr(
        "intra_assignment_events",
        "nba caller t=0 index=0 deferred=00\nzero t=1 zero=a0 negative=b0 unknown=c0\nblocking t=3 value=08 b=22 index=3\nrepeat t=7 repeated=11\nfinal t=9 blocking=08 repeated=11 deferred=01 index=3\n",
        "INFO: intra_assignment_events.sv:55:12: [SIM-001] (tb @ 10 ns) $finish called\n",
    );
}
