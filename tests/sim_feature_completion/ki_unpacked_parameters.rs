//! Parameters of unpacked array and structure types (IEEE 1800-2009 6.20.2,
//! 7.4.6).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_unpacked_parameters";

#[test]
fn multidimensional_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "multidimensional",
        include_str!(
            "../fixtures/sim/feature_completion/ki_unpacked_parameters/multidimensional.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn string_and_real_array_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "strings_reals",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/strings_reals.out"),
        &[],
        &[],
    );
}

#[test]
fn structure_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "structures",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/structures.out"),
        &[],
        &[],
    );
}

#[test]
fn overridden_unpacked_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "overrides",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/overrides.out"),
        &[],
        &[],
    );
}

#[test]
fn element_types_of_array_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "elements",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/elements.out"),
        &[],
        &[],
    );
}

#[test]
fn contexts_that_read_array_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "contexts",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/contexts.out"),
        &[],
        &[],
    );
}

#[test]
fn time_array_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "times",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/times.out"),
        &[],
        &[],
    );
}

#[test]
fn nonexistent_elements_of_array_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "nonexistent",
        include_str!("../fixtures/sim/feature_completion/ki_unpacked_parameters/nonexistent.out"),
        &[],
        &[],
    );
}

#[test]
fn strings_and_characters_of_unpacked_parameters() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_selects",
        include_str!(
            "../fixtures/sim/feature_completion/ki_unpacked_parameters/string_selects.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn unsupported_reads_of_unpacked_parameters_are_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_copy",
        "neg_string_copy.sv:5:9 resizable container assignment in `tb` requires a compatible array",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_copy",
        "neg_record_copy.sv:6:5 unpacked aggregate used as a scalar assignment LHS in `tb`",
    );
}
