//! SYN-022: primitive conditional patterns through the public simulator.
use crate::sim_cli;

#[test]
fn primitive_patterns_keep_order_scope_and_four_state_equality() {
    sim_cli::run_case_with_args(
        "syn022_primitive_patterns",
        "primitive_runtime",
        "primitive_patterns=pass checks=10 calls=12\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn primitive_pattern_bindings_end_at_the_true_arm() {
    for fixture in ["bad_binding_after_if", "bad_binding_else"] {
        sim_cli::reject_case_with_args(
            "syn022_primitive_patterns",
            fixture,
            "undeclared identifier",
            &["--edition", "sv2009"],
        );
    }
}

#[test]
fn duplicate_bindings_in_one_pattern_are_rejected() {
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "bad_duplicate_binding",
        "redefinition of 'same'",
        &["--edition", "sv2009"],
    );
}

#[test]
fn unsupported_whole_value_patterns_have_specific_diagnostics() {
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_dynamic_binding",
        "conditional whole-value pattern requires a supported fixed value",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_dynamic_wildcard",
        "conditional whole-value pattern requires a supported fixed value",
        &["--edition", "sv2009"],
    );
}

#[test]
fn real_and_string_whole_value_bindings_run() {
    sim_cli::run_case_with_args(
        "syn022_primitive_patterns",
        "real_binding",
        "real binding=1.50 twice=3.00\nfilter rejected\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "syn022_primitive_patterns",
        "string_binding",
        "native binding=abc joined=abc!\nfilter rejected\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn primitive_patterns_are_rejected_in_verilog_2001() {
    sim_cli::run_case_with_args(
        "syn022_primitive_patterns",
        "edition_boundary",
        "edition_pattern=pass\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "edition_boundary",
        "undeclared identifier 'matches'",
        &["--edition", "v2001"],
    );
}
