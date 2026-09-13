use super::run_case_with_stderr;

#[test]
fn clocking_input_skews_sample_preponed_observed_and_history_values() {
    run_case_with_stderr(
        "clocking_h13",
        "t=6 raw=1 step=0 zero=1 two=0\nt=16 raw=1 step=1 zero=1 two=1\n",
        "llg: $finish at time 18 at tb:23:9\n",
    );
}

#[test]
fn clocking_inputs_resolve_defaults_and_aliases_through_interfaces() {
    run_case_with_stderr(
        "clocking_h13_interface",
        "zero t=6000 raw=0 sample=0\ndefault t=6000 raw=1 sample=0\ndefault t=16000 raw=1 sample=1\nzero t=16000 raw=1 sample=1\n",
        "llg: $finish at time 18000 at tb:28:8\n",
    );
}

#[test]
fn clocking_inputs_resolve_through_static_virtual_interfaces() {
    run_case_with_stderr(
        "clocking_h13_virtual",
        "t=6000 data=1 sampled=0\n",
        "llg: $finish at time 15000 at tb:18:9\n",
    );
}

#[test]
fn clocking_outputs_and_cycle_delays_follow_clock_events_and_skews() {
    run_case_with_stderr(
        "clocking_h14",
        "drive1 t=2 zero=0 two=0\ndrive2 t=9 zero=1 two=0\nskew t=12 zero=1 two=1\ninout t=23 raw=0 sample=0\n",
        "llg: $finish at time 26 at tb:29:8\n",
    );
}

#[test]
fn clocking_output_selected_targets_capture_values() {
    run_case_with_stderr(
        "clocking_h14_targets",
        "targets t=7 a=1 b=2\n",
        "llg: $finish at time 8 at tb:18:8\n",
    );
}

#[test]
fn clocking_drives_preserve_nba_order_and_inout_resolution() {
    run_case_with_stderr(
        "clocking_h14_collisions",
        "collision t=3 out=1\nconflict t=7 bus=x sample=0\n",
        "llg: $finish at time 8 at tb:25:8\n",
    );
}

#[test]
fn asynchronous_clocking_drive_waits_for_next_event_before_skew() {
    run_case_with_stderr(
        "clocking_h14_async",
        "pending t=2 out=0 data=1\nedge t=3 out=0 data=1\ndrive t=6 out=1 data=1\n",
        "llg: $finish at time 7 at tb:19:8\n",
    );
}

#[test]
fn asynchronous_clocking_inout_drive_uses_its_net_driver() {
    run_case_with_stderr(
        "clocking_h14_async_inout",
        "edge t=2 bus=z sample=x\ndrive t=4 bus=1 sample=z\n",
        "llg: $finish at time 5 at tb:18:8\n",
    );
}

#[test]
fn zero_cycle_delays_distinguish_current_and_next_clocking_events() {
    run_case_with_stderr(
        "clocking_h14_zero",
        "same t=2 out=0\nfirst t=3 out=1\nsecond t=7 out=0\nsettled t=10 out=0\n",
        "llg: $finish at time 11 at tb:19:8\n",
    );
}

#[test]
fn clocking_drives_commit_after_design_nba_in_re_nba() {
    run_case_with_stderr(
        "clocking_h14_regions",
        "change t=1 out=0\nchange t=1 out=1\n",
        "llg: $finish at time 2 at tb:14:8\n",
    );
}

#[test]
fn clocking_output_edge_qualifiers_select_the_drive_event() {
    run_case_with_stderr(
        "clocking_h14_edges",
        "edges t=4 neg=1 pos=1\n",
        "llg: $finish at time 5 at tb:18:8\n",
    );
}

#[test]
fn clocking_cycle_delay_statement_executes_its_body_after_the_wait() {
    run_case_with_stderr(
        "clocking_h14_cycle_body",
        "body t=2\n",
        "llg: $finish at time 3 at tb:12:8\n",
    );
}

#[test]
fn clocking_intra_assignment_cycle_delay_captures_rhs_before_wait() {
    run_case_with_stderr(
        "clocking_h14_intra",
        "issued t=9 out=0 data=0\nafter t=11 out=1 data=0\n",
        "llg: $finish at time 12 at tb:22:8\n",
    );
}

#[test]
fn clocking_intra_assignment_cycle_delay_captures_selectors_before_wait() {
    run_case_with_stderr(
        "clocking_h14_intra_select",
        "selected issued t=9 out=00 sel=1\nselected after t=10 out=01 sel=1\n",
        "llg: $finish at time 11 at tb:23:8\n",
    );
}
