//! SYN-026: legal fixed-array iterator index queries through the public CLI.
use crate::sim_cli;

#[test]
fn fixed_array_reduction_and_ordering_indices_follow_declared_coordinates() {
    sim_cli::run_case_with_args(
        "syn026_iterator_indices",
        "indices",
        "indices=9,6,-4,28\nascending=20,10,30 descending=20,10,30\nrows=21,22;11,12\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn dynamic_dimension_preserves_formal_bounds_and_evaluates_each_map_once() {
    sim_cli::run_case_with_args(
        "syn026_iterator_indices",
        "dynamic_formal",
        "mapped=69 receiver=1 dimension=4 map=3 formal=9 width=96\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn array_iterator_query_is_available_in_systemverilog_2009() {
    sim_cli::run_case_with_args(
        "syn026_iterator_indices",
        "edition_boundary",
        "indices=9\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn array_iterator_query_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn026_iterator_indices",
        "edition_boundary",
        "use of undeclared identifier 'with'",
        &["--edition", "v2001"],
    );
}

#[test]
fn outer_iterator_rejects_an_unvisited_unpack_dimension() {
    sim_cli::reject_case_with_args(
        "syn026_iterator_indices",
        "unvisited_dimension",
        "undefined dimension 2; this lexical iterator defines dimensions 1..1",
        &["--edition", "sv2009"],
    );
}
