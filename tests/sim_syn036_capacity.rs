//! SYN-036 separates packed value capacity from fixed-array cell storage.

use crate::sim_cli;

const SUITE: &str = "syn036_capacity";

#[test]
#[ignore = "resource lane: million-bit generated C in both editions and optimizer modes"]
fn packed_value_one_below_and_at_the_limit_executes_in_both_editions() {
    for edition in ["2001", "2009"] {
        for (fixture, expected) in [
            ("packed_width_below", "PASS syn036 packed below\n"),
            ("packed_width_at", "PASS syn036 packed at\n"),
        ] {
            sim_cli::run_case_with_args(SUITE, fixture, expected, "", &[], &["--edition", edition]);
        }
    }
}

#[test]
fn packed_value_at_the_exclusive_limit_is_a_resource_diagnostic_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "packed_width_above",
            "1048576",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "packed_width_above",
            "maximum supported width",
            &["--edition", edition],
        );
    }
}

#[test]
#[ignore = "resource lane: 65,536 17-bit cells in generated C"]
fn direct_fixed_array_reduction_reads_cells_at_the_selected_limit() {
    sim_cli::run_case(
        SUITE,
        "cellwise_reduction",
        "PASS syn036 cellwise reduction\n",
        "",
        &[],
    );
}

#[test]
#[ignore = "resource lane: 65,535/65,536-cell generated models"]
fn fixed_array_storage_accepts_the_cell_below_and_at_the_limit_in_both_editions() {
    for edition in ["2001", "2009"] {
        for (fixture, expected) in [
            ("cell_limit_below", "PASS syn036 cell limit below\n"),
            ("cell_limit_at", "PASS syn036 cell limit at\n"),
        ] {
            sim_cli::run_case_with_args(SUITE, fixture, expected, "", &[], &["--edition", edition]);
        }
    }
}

#[test]
fn fixed_array_storage_accepts_the_former_65537_boundary() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(SUITE, "cell_limit", "", "", &[], &["--edition", edition]);
        sim_cli::run_case_with_args(
            "review_bundle",
            "r11_array_capacity_65537",
            "PASS r11_array_capacity_65537\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn fixed_value_formals_use_nonflattened_transport_above_packed_capacity() {
    sim_cli::run_case(SUITE, "aggregate_value_limit", "", "", &[]);
}

#[test]
#[ignore = "resource lane: million-bit fixed value formals and return values"]
fn flattened_fixed_values_below_and_at_payload_limit_execute() {
    for (fixture, expected) in [
        ("flat_value_below", "PASS syn036 flat below\n"),
        ("flat_value_at", "PASS syn036 flat at\n"),
    ] {
        sim_cli::run_case_with_args(SUITE, fixture, expected, "", &[], &["--edition", "2009"]);
    }
}

#[test]
fn nested_fixed_value_copy_keeps_immediate_elements_distinct() {
    sim_cli::run_case_with_args(
        SUITE,
        "nested_fixed_value",
        "PASS syn036 nested value\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn recursive_calls_reach_the_guard_only_above_256_activations_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "recursion_boundary",
            "PASS syn036 recursion below and at\n",
            "",
            &[],
            &["--edition", edition],
        );
        sim_cli::run_case_with_args(
            SUITE,
            "recursion_guard",
            "",
            "llg: recursion limit exceeded\n",
            &[],
            &["--edition", edition],
        );
    }
}
