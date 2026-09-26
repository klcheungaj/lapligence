//! SYN-022: primitive conditional patterns through the public simulator.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn primitive_patterns_keep_order_scope_and_four_state_equality() {
    sim_cli::run_case_with_args(
        "syn022_primitive_patterns",
        "primitive_runtime",
        "primitive_patterns=pass checks=10 calls=12\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn primitive_pattern_bindings_end_at_the_true_arm() {
    for fixture in ["bad_binding_after_if", "bad_binding_else"] {
        sim_cli::reject_case_with_args(
            "syn022_primitive_patterns",
            fixture,
            "undeclared identifier",
            &["--edition", "2009"],
        );
    }
}

#[test]
fn duplicate_bindings_in_one_pattern_are_rejected() {
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "bad_duplicate_binding",
        "redefinition of 'same'",
        &["--edition", "2009"],
    );
}

#[test]
fn unsupported_whole_value_patterns_have_specific_diagnostics() {
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_dynamic_binding",
        "conditional whole-value pattern requires a supported fixed value",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_dynamic_wildcard",
        "conditional whole-value pattern requires a supported fixed value",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_real_binding",
        "conditional whole-value pattern requires a supported fixed value",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "unsupported_native_binding",
        "string/class signals are not supported",
        &["--edition", "2009"],
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
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn022_primitive_patterns",
        "edition_boundary",
        "undeclared identifier 'matches'",
        &["--edition", "2001"],
    );
}
