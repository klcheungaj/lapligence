//! SYN-027 public CLI qualification of fixed-array reverse.

use crate::sim_cli;

#[test]
fn fixed_lengths_bounds_and_local_reverse_run_through_cli() {
    sim_cli::run_case_with_args(
        "data_types_completion",
        "syn_027_fixed_reverse",
        "PASS syn_027_fixed_reverse\n",
        "INFO: syn_027_fixed_reverse.sv:122:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn selected_rows_and_records_publish_reversed_values() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "notifications",
        "PASS syn027_reverse_notifications\n",
        "INFO: notifications.sv:47:5: [SIM-001] (tb @ 3 ns) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn automatic_formals_keep_selection_and_copy_semantics() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "formal_publication",
        "PASS syn027_reverse_formals\n",
        "INFO: formal_publication.sv:56:5: [SIM-001] (tb @ 2 ns) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn reverse_is_available_in_systemverilog_2009() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "edition_boundary",
        "PASS syn027_reverse_edition\n",
        "INFO: edition_boundary.sv:13:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn reverse_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "syn027_fixed_reverse",
        "edition_boundary",
        "`whole unpacked array value` is not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

#[test]
fn real_elements_reverse_numerically() {
    sim_cli::run_case_with_args(
        "syn027_fixed_reverse",
        "real_elements",
        "-2.25 -0.00 1.50 8000000000000000\n",
        "INFO: real_elements.sv:12:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}
