//! SYN-038 hierarchical child value reads, event observation, and selected writes.
//!
//! The checked-in fixture exercises typed enum, packed-struct, unpacked-record,
//! and fixed-array values through the public CLI in both optimizer modes under
//! IEEE 1800-2009.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn hierarchical_child_values_and_selected_writes_execute() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "hierarchical_value_matrix",
        "hier=01,1,12,56,21,33,56,12,53 event=1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
