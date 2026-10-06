//! SYN-028 public CLI qualification of fixed-array sort and rsort.

use crate::sim_cli;

#[test]
fn signed_unsigned_enum_and_packed_record_ordering() {
    sim_cli::run_case_with_args(
        "data_types_completion",
        "syn_028_fixed_sort",
        "PASS syn_028_fixed_sort\n",
        "llg: $finish at time 0 at tb:134:9\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn fixed_sort_is_rejected_in_verilog_2001() {
    sim_cli::reject_case_with_args(
        "data_types_completion",
        "syn028_edition_boundary",
        "not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

#[test]
fn fixed_sort_and_rsort_are_available_in_systemverilog_2009() {
    sim_cli::run_case_with_args(
        "data_types_completion",
        "syn028_edition_boundary",
        "PASS syn028_edition_boundary\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn unpacked_records_keep_key_payload_pairs_across_sort_directions() {
    sim_cli::run_case_with_args(
        "data_types_completion",
        "syn028_unpacked_record_maps",
        "PASS syn028_unpacked_record_maps\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn incompatible_map_and_const_ref_receiver_are_rejected() {
    sim_cli::reject_case_with_args(
        "data_types_completion",
        "syn_028_sort_bad_map",
        "integral",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "data_types_completion",
        "syn_028_sort_const_ref",
        "const",
        &["--edition", "sv2009"],
    );
}
