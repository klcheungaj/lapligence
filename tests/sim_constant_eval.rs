//! G1-28 let expressions and constant evaluation boundaries through the CLI.
//!
//! A `let` expands at its use site with free names bound in the declaration
//! scope; constant functions and generate conditions are evaluated during
//! elaboration; a constant context that reads runtime state is rejected before
//! a model is built.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn let_scope_shadowing_resolves_free_names_in_declaration_scope() {
    sim_cli::run_case(
        "feature_completion/g1_28",
        "let_scope_shadowing",
        "o=11\n",
        "",
        &[],
    );
}

#[test]
fn constant_function_generate_selects_and_sizes_from_elaboration() {
    sim_cli::run_case(
        "feature_completion/g1_28",
        "constant_function_generate",
        "ADDR_BITS=4 addr=a\n",
        "",
        &[],
    );
}

#[test]
fn recursive_let_is_rejected_with_its_declaration_location() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "feature_completion/g1_28",
            "let_recursive",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "optimized={optimized}: recursive let produced stdout: {output:?}"
        );
        assert!(stderr.contains("let_recursive.sv:6:17"), "{stderr}");
        assert!(stderr.contains("is recursive"), "{stderr}");
    }
}

#[test]
fn constant_context_runtime_read_is_rejected_before_model_building() {
    sim_cli::reject_case(
        "feature_completion/g1_28",
        "constant_context_runtime_read",
        "not allowed in a constant expression",
    );
}
