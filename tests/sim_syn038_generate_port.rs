//! SYN-038 function-result aggregate actuals on generated child input ports.
//!
//! The child instances check every field of their fixed array-of-record input.
//! This checked-in IEEE 1800-2009 witness runs through the public CLI in both
//! optimizer modes.

use crate::sim_cli;

#[test]
fn automatic_fixed_array_record_results_reach_generated_child_ports() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "generate_port",
        "generated=11\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
