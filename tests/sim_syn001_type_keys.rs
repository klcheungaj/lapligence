//! SYN-001 fixed-array assignment-pattern type-key coverage through `llg`.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn fixed_array_type_keys_and_precedence() {
    sim_cli::run_case(
        "syn001_type_keys",
        "syn_001_type_keys",
        "PASS syn_001_type_keys\n",
        "llg: $finish at time 1000 at tb:70:9\n",
        &[],
    );
}

#[test]
fn packed_structure_type_keys_remain_a_neighbor_control() {
    sim_cli::run_case(
        "data_types_completion",
        "packed_struct_assignment_patterns",
        "PASS packed_struct_assignment_patterns\n",
        "llg: $finish at time 0 at tb:117:9\n",
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
fn incompatible_type_key_is_rejected() {
    sim_cli::reject_case(
        "syn001_type_keys",
        "syn_001_incompatible_type_key",
        "has no matching index or type",
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
