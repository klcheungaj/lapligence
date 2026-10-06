//! SYN-038 event-expression witnesses for interface record and reduction sources.

use crate::sim_cli;

#[test]
fn interface_record_reduction_and_field_events_count_one_source_change() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_record_events",
        "events=1,1\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
