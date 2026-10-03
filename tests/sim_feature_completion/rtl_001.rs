use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_001";

#[test]
fn mixed_member_state_adopted_witness() {
    sim_cli::run_case(
        SUITE,
        "q01_member_conversion",
        include_str!("../fixtures/sim/feature_completion/rtl_001/q01_member_conversion.out"),
        "",
        &[],
    );
}

#[test]
fn nested_records_copies_indices_and_slices() {
    sim_cli::run_case(
        SUITE,
        "nested_records",
        include_str!("../fixtures/sim/feature_completion/rtl_001/nested_records.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "nested_records",
        include_str!("../fixtures/sim/feature_completion/rtl_001/nested_records.out"),
    );
}

#[test]
fn equal_and_unequal_union_views() {
    sim_cli::run_case(
        SUITE,
        "union_views",
        include_str!("../fixtures/sim/feature_completion/rtl_001/union_views.out"),
        "",
        &[],
    );
}

#[test]
fn record_net_member_contributions_and_defaults() {
    sim_cli::run_case(
        SUITE,
        "record_nets",
        include_str!("../fixtures/sim/feature_completion/rtl_001/record_nets.out"),
        "",
        &[],
    );
}

#[test]
fn arrays_of_record_net_members() {
    sim_cli::run_case(
        SUITE,
        "record_net_arrays",
        include_str!("../fixtures/sim/feature_completion/rtl_001/record_net_arrays.out"),
        "",
        &[],
    );
}

#[test]
fn illegal_packed_union_width() {
    sim_cli::reject_case(
        SUITE,
        "neg_unequal_packed_union",
        "all members of a packed union must have the same width",
    );
}

#[test]
fn illegal_packed_native_member() {
    sim_cli::reject_case(
        SUITE,
        "neg_packed_native",
        "packed members must be of integral type",
    );
}

#[test]
fn illegal_net_member_state() {
    sim_cli::reject_case(
        SUITE,
        "neg_record_net_state",
        "is not a valid type for a net",
    );
}

#[test]
fn illegal_dynamic_untagged_union() {
    sim_cli::reject_case(SUITE, "neg_untagged_dynamic", "union");
}

#[test]
fn illegal_verilog_whole_array_copy() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_whole_array_2001.v",
        "whole unpacked array value",
        &["--edition", "2001"],
    );
}

#[test]
fn packed_member_limb_boundaries_and_reversed_indices() {
    sim_cli::run_case(
        SUITE,
        "packed_boundaries",
        include_str!("../fixtures/sim/feature_completion/rtl_001/packed_boundaries.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "packed_boundaries",
        include_str!("../fixtures/sim/feature_completion/rtl_001/packed_boundaries.out"),
    );
}

#[test]
fn record_net_ports_preserve_member_names_and_signedness() {
    sim_cli::run_case(
        SUITE,
        "record_port_composition",
        include_str!("../fixtures/sim/feature_completion/rtl_001/record_port_composition.out"),
        "",
        &[],
    );
}

#[test]
fn verilog_memory_elements_remain_legal() {
    sim_cli::run_case_with_args(
        SUITE,
        "memory_elements_2001.v",
        include_str!("../fixtures/sim/feature_completion/rtl_001/memory_elements_2001.out"),
        "",
        &[],
        &["--edition", "2001"],
    );
}

#[test]
fn illegal_unpacked_union_nested_in_net_array() {
    sim_cli::reject_case(SUITE, "neg_unpacked_union_net", "not a valid net type");
}

#[test]
fn packed_record_net_member_contributions() {
    sim_cli::run_case(
        SUITE,
        "packed_record_nets",
        include_str!("../fixtures/sim/feature_completion/rtl_001/packed_record_nets.out"),
        "",
        &[],
    );
}
