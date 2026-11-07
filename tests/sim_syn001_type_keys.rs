//! SYN-001 fixed-array assignment-pattern type-key coverage through `llg`.

use crate::sim_cli;

#[test]
fn fixed_array_type_keys_and_precedence() {
    sim_cli::run_case(
        "syn001_type_keys",
        "syn_001_type_keys",
        "PASS syn_001_type_keys\n",
        "INFO: syn_001_type_keys.sv:70:9: [SIM-001] (tb @ 1 ns) $finish called\n",
        &[],
    );
}

#[test]
fn packed_structure_type_keys_remain_a_neighbor_control() {
    sim_cli::run_case(
        "data_types_completion",
        "packed_struct_assignment_patterns",
        "PASS packed_struct_assignment_patterns\n",
        "INFO: packed_struct_assignment_patterns.sv:117:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}

#[test]
fn duplicate_explicit_index_is_rejected() {
    sim_cli::reject_case(
        "syn001_type_keys",
        "syn_001_duplicate_index",
        "multiple keys",
    );
}

#[test]
fn uncovered_array_with_unmatched_type_key_is_rejected() {
    sim_cli::reject_case(
        "syn001_type_keys",
        "syn_001_incompatible_type_key",
        "not all elements",
    );
}

#[test]
fn missing_fixed_array_coverage_is_rejected() {
    sim_cli::reject_case(
        "syn001_type_keys",
        "syn_001_missing_coverage",
        "not all elements",
    );
}
