use super::run_case;

#[test]
fn future_nba_advances_time_without_live_processes() {
    super::run_case_with_stderr(
        "nba_only_future",
        "final 7000 42\n",
        "llg: simulation ended without $finish (no processes remain) at time 7000\n",
    );
}

#[test]
fn delayed_nba_converts_real_and_two_state_values_when_issued() {
    run_case(
        "nba_conversions",
        "issued 0\n2.5 16777216.0 17ffffffeffffffff\n",
    );
}

#[test]
fn selected_nbas_preserve_four_state_bits_and_ignore_invalid_indices() {
    run_case("nba_four_state", "1xz01 z10x ba 00\n");
}

#[test]
fn delayed_nba_captures_values_and_continues_without_suspending() {
    run_case(
        "delayed_nba",
        "issued 0 0 9\npending 2000 0\ncommitted 5000 7\n",
    );
}

#[test]
fn delayed_nba_outlives_its_process_and_commits_after_active_events() {
    run_case("delayed_nba_process", "active 0\npostponed 11\nlater 11\n");
}

#[test]
fn delayed_nbas_keep_issue_order_and_capture_selected_targets() {
    run_case("delayed_nba_select", "4000 c1bd 42 00\n");
}

#[test]
fn zero_delay_and_plain_selected_nbas_preserve_disjoint_updates() {
    run_case("nba_disjoint", "inactive d000\npostponed dcba\n");
}
