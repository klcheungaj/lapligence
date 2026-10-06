use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/rtl_008";

#[test]
fn package_initializers_follow_their_static_dependencies() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/package_order.out");
    sim_cli::run_case(SUITE, "package_order", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "package_order", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "package_order", expected);
}

#[test]
fn call_initializers_keep_declaration_order() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/call_order.out");
    sim_cli::run_case(SUITE, "call_order", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "call_order", expected, &[], &[]);
}

#[test]
fn explicit_static_locals_initialize_once_before_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/static_locals.out");
    sim_cli::run_case(SUITE, "static_locals", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "static_locals", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "static_locals", expected);
}

#[test]
fn nested_member_and_argument_defaults_initialize_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/member_defaults.out");
    sim_cli::run_case(SUITE, "member_defaults", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_defaults", expected, &[], &[]);
}

#[test]
fn let_templates_bind_declaration_scope_and_return_fixed_aggregates() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/aggregate_lets.out");
    sim_cli::run_case(SUITE, "aggregate_lets", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "aggregate_lets", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "aggregate_lets", expected);
}

#[test]
fn descriptor_initializers_run_in_the_static_schedule() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_008/descriptor_initializers.out");
    sim_cli::run_case(SUITE, "descriptor_initializers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_initializers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_initializers", expected);
}

#[test]
fn package_and_unit_constants_reach_module_initializers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/package_constants.out");
    for units in ["separate", "merged"] {
        sim_cli::run_case_with_args(
            SUITE,
            "package_constants",
            expected,
            "",
            &[],
            &["--compilation-units", units],
        );
    }
    sim_cli::run_case_backend_parity(SUITE, "package_constants", expected, &[], &[]);
}

#[test]
fn package_state_is_shared_across_compilation_unit_modes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/shared_top.out");
    for units in ["separate", "merged"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "shared_top",
            &["shared_pkg", "shared_user"],
            expected,
            "",
            &[],
            &["--compilation-units", units],
        );
    }
}

#[test]
fn merged_unit_declarations_and_reexports_are_shared() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/unit_merged_top.out");
    sim_cli::run_case_with_source_prefix(
        SUITE,
        "unit_merged_top",
        &["unit_decls"],
        expected,
        "",
        &[],
        &["--compilation-units", "merged"],
    );
}

#[test]
fn separate_units_keep_distinct_unit_scopes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_008/unit_separate_top.out");
    sim_cli::run_case_with_source_prefix(
        SUITE,
        "unit_separate_top",
        &["unit_separate_helper"],
        expected,
        "",
        &[],
        &["--compilation-units", "separate"],
    );
}

/// Generate the C model of one fixture without building it and report
/// whether declaration initialization was emitted as an active-region
/// process (`<design>.declaration_init.<n>`).
fn declaration_init_processes(fixture: &str, edition: &str) -> bool {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(fixture);
    let directory = sim_harness::TempDir::new("rtl008-edition").expect("CLI test directory");
    let output = sim_harness::run_command(
        std::process::Command::new(env!("CARGO_BIN_EXE_llg"))
            .current_dir(directory.path())
            .args([
                "--top",
                "tb",
                "--gen-only",
                "--edition",
                edition,
                "--out-dir",
            ])
            .arg(directory.path().join("out"))
            .arg(&source),
        std::time::Duration::from_secs(180),
    )
    .expect("generate model");
    assert!(
        output.status.success(),
        "{fixture} --edition {edition}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let model = std::fs::read_to_string(directory.path().join("out/sim/tb/model.c"))
        .expect("generated model source");
    model.contains("\"tb.declaration_init.")
}

#[test]
fn verilog_declaration_races_are_not_converted_to_static_initialization() {
    // SystemVerilog: static initialization precedes every process and is not
    // an event, so the `always` never observes it.
    sim_cli::run_case_with_args(
        SUITE,
        "edition_race.v",
        "1 0 6\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    // Verilog-2001: the declaration assignment is an initial-time assignment
    // racing with the `always`; both outcomes are legal.
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            SUITE,
            "edition_race.v",
            optimized,
            &["--edition", "v2001"],
            &[],
            &[],
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "optimized={optimized}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            matches!(stdout.as_ref(), "1 0 6\n" | "1 1 6\n"),
            "optimized={optimized}: {stdout:?}"
        );
    }
    assert!(declaration_init_processes("edition_race.v", "v2001"));
    assert!(!declaration_init_processes("edition_race.v", "sv2009"));
}

#[test]
fn package_string_call_witness() {
    // Adopted FND-002 witness for L-F03-03-02 (SV2009 26, 13.5).
    sim_cli::run_case_with_args(
        SUITE,
        "package_string_call_witness",
        "ok\n",
        "llg: $finish at time 0 at tb:4:49\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn timed_function_witness() {
    // Adopted FND-002 witness for L-F02-05-01 (SV2009 13.4.4).
    sim_cli::reject_case(
        SUITE,
        "timed_function_witness",
        "statements that pass time are not allowed in this context",
    );
}

#[test]
fn let_recursive_witness() {
    // Adopted FND-002 witness for L-F07-16-01 (SV2009 11.13).
    sim_cli::reject_case(
        SUITE,
        "let_recursive_witness",
        "let declaration 'f' is recursive",
    );
}

#[test]
fn hierarchical_constant_witness() {
    // Adopted FND-002 witness for L-F03-03-04 (SV2009 11.2.1).
    sim_cli::reject_case(
        SUITE,
        "hierarchical_constant_witness",
        "reference to 'X' by hierarchical name is not allowed in a constant expression",
    );
}

#[test]
fn soft_constraint_witness() {
    // Adopted FND-002 witness for L-F03-03-03 (SV2009 18.5, Annex A.1.9).
    sim_cli::reject_case(
        SUITE,
        "soft_constraint_witness",
        "use of undeclared identifier 'soft'",
    );
}

#[test]
fn neg_timed_initializer() {
    sim_cli::reject_case(
        SUITE,
        "neg_timed_initializer",
        "statements that pass time are not allowed in this context",
    );
}

#[test]
fn neg_event_initializer() {
    sim_cli::reject_case(
        SUITE,
        "neg_event_initializer",
        "statements that pass time are not allowed in this context",
    );
}

#[test]
fn neg_let_verilog() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_let_verilog.v",
        "unknown module 'let'",
        &["--edition", "v2001"],
    );
}

#[test]
fn neg_nonconstant_extent() {
    sim_cli::reject_case(
        SUITE,
        "neg_nonconstant_extent",
        "reference to non-constant variable 'n' is not allowed in a constant expression",
    );
}

#[test]
fn neg_const_write() {
    sim_cli::reject_case(
        SUITE,
        "neg_const_write",
        "cannot assign to read-only variable 'c'",
    );
}

#[test]
fn neg_const_in_parameter() {
    sim_cli::reject_case(
        SUITE,
        "neg_const_in_parameter",
        "reference to non-constant variable 'c' is not allowed in a constant expression",
    );
}

#[test]
fn neg_package_variable_parameter() {
    sim_cli::reject_case(
        SUITE,
        "neg_package_variable_parameter",
        "reference to non-constant variable 'v' is not allowed in a constant expression",
    );
}

#[test]
fn neg_unit_redefinition() {
    sim_cli::reject_case(
        SUITE,
        "neg_unit_redefinition",
        "redefinition of `unit_k` in the compilation-unit scope",
    );
}

#[test]
fn neg_package_redefinition() {
    sim_cli::reject_case(
        SUITE,
        "neg_package_redefinition",
        "redefinition of `v` in package `p`",
    );
}
