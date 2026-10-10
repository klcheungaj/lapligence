//! SIM-026: scanning, line and binary input destinations.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_026";

const SELECTED_INPUT: (&str, &str) = (
    "selected_destinations.txt",
    include_str!("../fixtures/sim/feature_completion/sim_026/selected_destinations.txt"),
);
const BINARY_INPUT: (&str, &str) = (
    "binary_input.dat",
    include_str!("../fixtures/sim/feature_completion/sim_026/binary_input.dat"),
);
const LINE_INPUT: (&str, &str) = (
    "line_input.txt",
    include_str!("../fixtures/sim/feature_completion/sim_026/line_input.txt"),
);

// A01: legal selected, formal and container destinations; byte counts, bit
// order, partial updates and the unread suffix.
#[test]
fn fscanf_into_selects_elements_members_and_properties() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_026/selected_destinations.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "selected_destinations",
        expected,
        &[],
        &[],
        &[SELECTED_INPUT],
    );
}

#[test]
fn sscanf_into_task_and_function_formals() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_026/formal_destinations.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "formal_destinations",
        expected,
        &[],
        &[],
        &[],
    );
}

#[test]
fn sscanf_into_container_elements_with_entry_snapshot() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_026/container_destinations.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "container_destinations",
        expected,
        &[],
        &[],
        &[],
    );
}

#[test]
fn every_scan_conversion_and_source_kind() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_026/scan_conversions.out");
    sim_cli::run_case_backend_parity_with_files(SUITE, "scan_conversions", expected, &[], &[], &[]);
}

#[test]
fn mixed_text_and_binary_input() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_026/binary_input.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "binary_input",
        expected,
        &[],
        &[],
        &[BINARY_INPUT],
    );
}

#[test]
fn line_input_ungetc_and_positioning() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_026/line_input.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "line_input",
        expected,
        &[],
        &[],
        &[LINE_INPUT],
    );
}

// A02: end of file, short reads, malformed tokens, overlong fields,
// zero-length reads and invalid descriptors.
#[test]
fn input_failures_have_independent_results() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_026/input_failures.out");
    sim_cli::run_case_backend_parity_with_files(SUITE, "input_failures", expected, &[], &[], &[]);
}

#[test]
fn invalid_descriptors_fail_without_side_effects() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_026/invalid_descriptors.out");
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "invalid_descriptors",
        expected,
        &[],
        &[],
        &[],
    );
}

// A03: illegal destinations are rejected before a model is built.
#[test]
fn neg_unpacked_aggregate_destinations() {
    let unpacked = "is illegal: scan conversions assign integral, real or string variables, or an unpacked array of bytes (SV 21.3.4.3)";
    sim_cli::reject_case(SUITE, "neg_unpacked_array", unpacked);
    sim_cli::reject_case(SUITE, "neg_string_into_int_array", unpacked);
    sim_cli::reject_case(SUITE, "neg_unpacked_struct", unpacked);
}

#[test]
fn neg_non_storage_destinations() {
    sim_cli::reject_case(
        SUITE,
        "neg_class_handle",
        "file input destination must be packed, real, or string storage in `tb`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_string_character",
        "a character of a string as a file input destination in `tb` is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_destination",
        "constant 'P' is not modifiable",
    );
}

#[test]
fn neg_fread_and_fgets_destination_types() {
    sim_cli::reject_case(
        SUITE,
        "neg_fread_real_memory",
        "$fread destination array must contain packed elements in `tb`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fread_associative",
        "$fread destination container must be a dynamic array or queue of packed elements in `tb`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fread_string",
        "invalid argument type 'string' to system function",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fgets_real",
        "value of type 'real' is not a valid string argument",
    );
}
