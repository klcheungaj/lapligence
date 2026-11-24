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
    for (case, location, name) in [
        ("neg_sformatf", "8:33", "$sformatf"),
        ("neg_method", "8:33", "toupper"),
        ("neg_function", "8:33", "f"),
        ("neg_nested", "8:37", "f"),
        ("neg_queue", "8:33", "q"),
    ] {
        sim_cli::reject_case_with_id(
            SUITE,
            case,
            "CODEGEN-002",
            Some(location),
            &format!("$bits cannot enclose `{name}`, which returns a dynamically sized type"),
        );
    }
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
