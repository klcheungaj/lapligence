//! SYN-023: recursive fixed structure patterns through the public simulator.
use crate::sim_cli;

#[test]
fn recursive_fixed_structure_patterns_preserve_values_and_scope() {
    sim_cli::run_case_with_args(
        "syn023_structure_patterns",
        "recursive_runtime",
        "structure_patterns=pass checks=11 calls=3\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn mixed_state_structure_constants_read_each_member_in_its_state_domain() {
    sim_cli::run_case_with_args(
        "syn023_structure_patterns",
        "mixed_state_constants",
        "mixed_state_patterns=pass checks=4\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn structure_patterns_require_systemverilog() {
    sim_cli::run_case_with_args(
        "syn023_structure_patterns",
        "edition_boundary",
        "structure_edition=pass\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn023_structure_patterns",
        "edition_boundary",
        "undeclared identifier 'matches'",
        &["--edition", "v2001"],
    );
}

#[test]
fn malformed_or_out_of_scope_structure_patterns_are_rejected() {
    for (fixture, diagnostic) in [
        ("bad_duplicate_nested_binding", "redefinition of 'same'"),
        ("bad_positional_shape", "too few"),
        ("bad_nested_member_type", "not a struct"),
        ("bad_binding_else", "undeclared identifier"),
    ] {
        sim_cli::reject_case_with_args(
            "syn023_structure_patterns",
            fixture,
            diagnostic,
            &["--edition", "sv2009"],
        );
    }
}
