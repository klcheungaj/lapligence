//! `$bits` of string expressions (IEEE 1800-2009 20.6.2).

use super::sim_cli;

const SUITE: &str = "feature_completion/ki_bits_of_strings";

#[test]
fn sizes_of_string_expressions() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "sizes",
        include_str!("../fixtures/sim/feature_completion/ki_bits_of_strings/sizes.out"),
        &[],
        &[],
    );
}

#[test]
fn functions_of_dynamically_sized_types_are_rejected() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_sformatf",
        "CODEGEN-002",
        Some("8:33"),
        "$bits cannot enclose `$sformatf`, which returns a dynamically sized type",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_method",
        "CODEGEN-002",
        Some("8:33"),
        "$bits cannot enclose `toupper`, which returns a dynamically sized type",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_function",
        "CODEGEN-002",
        Some("8:33"),
        "$bits cannot enclose `f`, which returns a dynamically sized type",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_nested",
        "CODEGEN-002",
        Some("8:37"),
        "$bits cannot enclose `f`, which returns a dynamically sized type",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_queue",
        "CODEGEN-002",
        Some("8:33"),
        "$bits cannot enclose `q`, which returns a dynamically sized type",
    );
}

#[test]
fn string_expressions_with_side_effects_are_rejected() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_side_effect",
        "CODEGEN-001",
        Some("8:33"),
        "$bits of a string expression with a call or side effect",
    );
}
