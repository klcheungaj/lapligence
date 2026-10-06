//! SYN-027 public CLI qualification of fixed-array reverse.

use crate::sim_cli;

#[test]
fn fixed_lengths_bounds_and_local_reverse_run_through_cli() {
    sim_cli::run_case_with_args(
        "data_types_completion",
        "syn_027_fixed_reverse",
        "PASS syn_027_fixed_reverse\n",
        "llg: $finish at time 0 at tb:122:9\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn selected_rows_and_records_publish_reversed_values() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "notifications",
        "PASS syn027_reverse_notifications\n",
        "llg: $finish at time 3 at tb:47:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn automatic_formals_keep_selection_and_copy_semantics() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "formal_publication",
        "PASS syn027_reverse_formals\n",
        "llg: $finish at time 2 at tb:56:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn reverse_is_available_in_systemverilog_2009() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "edition_boundary",
        "PASS syn027_reverse_edition\n",
        "llg: $finish at time 0 at tb:13:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn reverse_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn027_fixed_reverse",
        "edition_boundary",
        "`whole unpacked array value` is not available in IEEE 2001",
        &["--edition", "2001"],
    );
}

#[test]
fn real_elements_reverse_numerically() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "real_elements",
        "-2.25 -0.00 1.50 8000000000000000\n",
        "llg: $finish at time 0 at tb:12:5\n",
        &[],
        &["--edition", "2009"],
    );
}
