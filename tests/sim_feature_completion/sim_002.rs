//! SIM-002: time-reporting scope and formatting controls.
//!
//! Every expectation is computed by hand from IEEE 1800-2009 §§3.14, 20.3,
//! 20.4 and 21.2.1.3 and IEEE 1364-2001 §§17.3 and 17.7, never captured from
//! `llg`. Executed fixtures run through the public CLI in both optimizer modes
//! on the legacy and compact value backends.

use std::path::Path;

use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_002";
const V2001: &[&str] = &["--edition", "v2001"];

#[test]
fn printtimescale_reports_the_named_or_current_scope() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_002/printtimescale_scope.out");
    sim_cli::run_case(SUITE, "printtimescale_scope", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "printtimescale_scope", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "printtimescale_scope", expected);

    let lrm = include_str!("../fixtures/sim/feature_completion/sim_002/printtimescale_v2001.out");
    sim_cli::run_case_with_args(SUITE, "printtimescale_v2001.v", lrm, "", &[], V2001);
    sim_cli::run_case_backend_parity(SUITE, "printtimescale_v2001.v", lrm, V2001, &[]);

    // Adopted FND-002 witness: a caller at 1us/1ns printing a 1ns/1ps child.
    let witness =
        include_str!("../fixtures/sim/feature_completion/sim_002/printtimescale_child_scope.out");
    sim_cli::run_case(SUITE, "printtimescale_child_scope.v", witness, "", &[]);
    sim_cli::run_case_with_args(
        SUITE,
        "printtimescale_child_scope.v",
        witness,
        "",
        &[],
        V2001,
    );
}

#[test]
fn time_functions_scale_by_the_declaring_scope() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_002/time_scopes.out");
    sim_cli::run_case(SUITE, "time_scopes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "time_scopes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "time_scopes", expected);

    let verilog = include_str!("../fixtures/sim/feature_completion/sim_002/time_scopes_v2001.out");
    sim_cli::run_case_with_args(SUITE, "time_scopes_v2001.v", verilog, "", &[], V2001);
    sim_cli::run_case_backend_parity(SUITE, "time_scopes_v2001.v", verilog, V2001, &[]);
}

#[test]
fn timeformat_controls_are_design_wide() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_controls.out");
    sim_cli::run_case(SUITE, "timeformat_controls", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "timeformat_controls", expected, &[], &[]);

    // Adopted FND-002 witnesses for the documented zero- and four-argument
    // forms and the zero-argument reset; their clauses cover both editions.
    let four = include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_four_args.out");
    sim_cli::run_case(SUITE, "timeformat_four_args.v", four, "", &[]);
    sim_cli::run_case_with_args(SUITE, "timeformat_four_args.v", four, "", &[], V2001);
    let zero = include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_zero_args.out");
    sim_cli::run_case(SUITE, "timeformat_zero_args.v", zero, "", &[]);
    sim_cli::run_case_with_args(SUITE, "timeformat_zero_args.v", zero, "", &[], V2001);
    let reset =
        include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_reset_defaults.out");
    sim_cli::run_case(SUITE, "timeformat_reset_defaults.v", reset, "", &[]);
    sim_cli::run_case_with_args(SUITE, "timeformat_reset_defaults.v", reset, "", &[], V2001);
}

#[test]
fn time_reports_are_exact_at_boundary_times() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_boundary.out");
    sim_cli::run_case(SUITE, "timeformat_boundary", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "timeformat_boundary", expected, &[], &[]);
}

#[test]
fn timeformat_never_moves_scheduled_events() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_002/timeformat_schedule.out");
    sim_cli::run_case(SUITE, "timeformat_schedule", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "timeformat_schedule", expected, &[], &[]);
}

#[test]
fn illegal_arity_and_types_remain_diagnostics() {
    // Adopted FND-002 extension probes: only the zero- and four-argument
    // forms are documented syntax.
    let arity = "$timeformat requires either zero or four arguments";
    sim_cli::reject_case(SUITE, "neg_timeformat_one_arg.v", arity);
    sim_cli::reject_case(SUITE, "neg_timeformat_two_args.v", arity);
    sim_cli::reject_case(SUITE, "neg_timeformat_three_args.v", arity);
    sim_cli::reject_case(
        SUITE,
        "neg_timeformat_five_args.v",
        "too many arguments for '$timeformat'; expected 4 but 5 were provided",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_timeformat_suffix_type",
        "no implicit conversion from 'int' to 'string'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_timeformat_units_string",
        "no implicit conversion from 'string' to 'int'",
    );
    sim_cli::reject_case(SUITE, "neg_printtimescale_generate", "expected module name");
    sim_cli::reject_case(
        SUITE,
        "neg_printtimescale_expression",
        "expected module name",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_printtimescale_two_args",
        "too many arguments for '$printtimescale'; expected 1 but 2 were provided",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_printtimescale_root",
        "unsupported executable node `ArbitrarySymbol`",
    );
    let stdout =
        include_str!("../fixtures/sim/feature_completion/sim_002/neg_timeformat_units_range.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_timeformat_units_range",
        &[],
        &|label, output| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{label}: unexpected success");
            assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
            assert!(
                stderr.contains("$timeformat units must be between -15 and 0"),
                "{label}: {stderr}"
            );
        },
    );
}

/// Package, `$unit` and class scales survive into the owned Db after the
/// frontend snapshot is dropped; classes inherit their declaring scope.
#[test]
fn component_declaration_time_scales_are_owned() {
    use llg::core::{
        compile,
        db::{Db, NodeKind},
    };

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/sim_002/time_scopes.sv");
    let database = sim_harness::with_frontend_temp_cwd("sim-002-db", |_| {
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))
    })
    .expect("checked compilation");
    let scale_of = |name: &str, kind: fn(&NodeKind) -> bool| {
        let id = database
            .node_ids()
            .find(|id| database.node(*id).name == name && kind(database.node_kind(*id)))
            .unwrap_or_else(|| panic!("missing {name}"));
        database.declaration_time_scale(id)
    };
    assert_eq!(
        scale_of("pkg", |kind| matches!(kind, NodeKind::Package)),
        Some((-5, -6))
    );
    assert_eq!(
        scale_of("Clock", |kind| matches!(kind, NodeKind::ClassDef)),
        Some((-5, -6))
    );
    assert_eq!(
        scale_of("Local", |kind| matches!(kind, NodeKind::ClassDef)),
        Some((-6, -12))
    );
    let unit = database
        .node_ids()
        .find(|id| database.node(*id).name == "unit_report")
        .and_then(|function| database.node(function).parent())
        .expect("compilation unit of unit_report");
    assert_eq!(database.semantic_detail(unit), Some("CompilationUnit"));
    assert_eq!(database.declaration_time_scale(unit), Some((-10, -11)));
}
