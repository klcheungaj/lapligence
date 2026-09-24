//! SYN-038 event-expression witnesses for interface record and reduction sources.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn interface_record_reduction_and_field_events_count_one_source_change() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_record_events",
        "events=1,1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
