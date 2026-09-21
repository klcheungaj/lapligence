//! SYN-014 public-pipeline evidence for aggregate sensitivity and always-family contracts.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn aggregate_sensitivity_and_always_family_contracts_match_in_both_modes() {
    sim_cli::run_case(
        "syn014_process_contexts",
        "process_contexts",
        concat!(
            "aggregate arm=1 at=2 feedback=c linked=07 split=c1 latch=x ff=00 delta=1\n",
            "enabled arm=5 at=2 feedback=c linked=15 split=c5 latch=5 ff=00\n",
            "triggered arm=c at=c feedback=c linked=15 split=c5 latch=5 ff=00\n",
            "array arm=c at=c feedback=c linked=31 split=c5 latch=5 ff=00\n",
            "held ff=00\n",
            "edge ff=2a\n",
            "reset ff=00\n",
        ),
        "",
        &[],
    );
}

#[test]
fn overlapping_packed_writers_are_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "overlapping_writers",
        "multiple writers",
    );
}

#[test]
fn extra_always_ff_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "extra_event",
        "one and only one event control",
    );
}

#[test]
fn explicit_always_latch_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "latch_event",
        "statements that pass time",
    );
}
