//! Composed public-CLI acceptance witnesses for the synthesizable review findings.
//!
//! Each checked-in HDL fixture runs through `llg` with and without optimization,
//! and its exact output is checked independently of the generated model.

use crate::sim_cli;

const SUITE: &str = "review_bundle";

#[test]
fn recursive_type_keys_cross_function_return_array_port_and_comb_logic() {
    sim_cli::run_case_with_args(
        SUITE,
        "r13_recursive_pattern_function_port",
        "recursive pattern function/port passed: 34\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn record_deconstruction_captures_selected_nba_target_and_source() {
    sim_cli::run_case_with_args(
        SUITE,
        "r13_record_pattern_selected_nba",
        "selected record NBA scatter passed: 12/34\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
