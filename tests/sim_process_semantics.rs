//! File-backed process-family semantic regressions through the public CLI.
//!
//! The harness runs each successful fixture with and without optimization and
//! leaves lint disabled for the rejection cases, proving these are simulator
//! semantic diagnostics rather than optional lint findings.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn always_comb_time_zero_and_written_local_exclusion() {
    sim_cli::run_case(
        "process_semantics",
        "always_comb_time_zero",
        "zero y=0\none y=1\n",
        "llg: $finish at time 0 at tb:24:9\n",
        &[],
    );
}

#[test]
fn at_star_does_not_follow_called_function_reads() {
    sim_cli::run_case(
        "process_semantics",
        "at_star_distinct",
        "t=1 at=x comb=0\nt=2 at=x comb=1\n",
        "llg: $finish at time 2000 at tb:21:9\n",
        &[],
    );
}

#[test]
fn legal_latch_and_edge_flip_flop_run_without_false_errors() {
    sim_cli::run_case(
        "process_semantics",
        "legal_latch_ff",
        "start latched=x q=x\nhold latched=x q=x\nopen latched=1 q=x\nedge latched=1 q=1\ndata latched=0 q=1\n",
        "llg: $finish at time 5000 at tb:27:9\n",
        &[],
    );
}

#[test]
fn legal_function_side_effect_is_not_rejected() {
    sim_cli::run_case(
        "process_semantics",
        "function_side_effect",
        "t=1 helper=0 y=0\nt=2 helper=1 y=1\n",
        "llg: $finish at time 2000 at tb:21:9\n",
        &[],
    );
}

#[test]
fn multiple_writers_reject_without_lint() {
    sim_cli::reject_case("process_semantics", "multiple_writer", "multiple writers");
}

#[test]
fn timing_controls_reject_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "timing_control",
        "statements that pass time",
    );
}

#[test]
fn always_ff_blocking_data_assignment_is_legal() {
    sim_cli::run_case("process_semantics", "ff_blocking", "q=1\n", "", &[]);
}

#[test]
fn legal_level_flip_flop_event_does_not_false_error() {
    sim_cli::run_case(
        "process_semantics",
        "ff_level_event",
        "q=1\n",
        "llg: $finish at time 2000 at tb:14:9\n",
        &[],
    );
}

#[test]
fn static_array_writers_and_called_function_reads_are_precise() {
    sim_cli::run_case(
        "process_semantics",
        "static_prefix",
        "initial selected=11 result=01 m0=11 m1=22 helper=1\nswitch selected=22 result=01 m0=11 m1=22 helper=1\nchange selected=22 result=01 m0=33 m1=22 helper=1\n",
        "",
        &[],
    );
}

#[test]
fn unpacked_member_writers_and_function_reads_are_precise() {
    sim_cli::run_case(
        "process_semantics",
        "member_prefix",
        "initial observed=a hi=a lo=3\nlo observed=a hi=a lo=5\nhi observed=c hi=c lo=5\n",
        "",
        &[],
    );
}

#[test]
fn delayed_nonblocking_assignment_is_legal_in_always_ff() {
    sim_cli::run_case(
        "process_semantics",
        "delayed_nba_ff",
        "edge q=x\ndelayed q=1\n",
        "",
        &[],
    );
}

#[test]
fn always_ff_fork_is_rejected_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "ff_forbidden_fork",
        "cannot contain a fork",
    );
}

#[test]
fn always_ff_extra_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "ff_extra_event",
        "one and only one event control",
    );
}

#[test]
fn always_latch_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "latch_event",
        "statements that pass time",
    );
}

#[test]
fn called_function_timing_is_rejected_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "function_timing",
        "cannot contain blocking timing controls",
    );
}

#[test]
fn called_function_writer_conflict_is_rejected_without_lint() {
    sim_cli::reject_case(
        "process_semantics",
        "function_writer_conflict",
        "multiple writers",
    );
}
