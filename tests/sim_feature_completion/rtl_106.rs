//! RTL-106: source-provenance completeness.
//!
//! Frontend diagnostics (Slang, edition and lint) print the physical
//! `file:line:col` followed by the `` `line``-mapped position, counted by hand
//! from the fixture lines. The strict 2001 profile rejects later forms built
//! by macros and the variable drivers IEEE 1364-2001 lacks; each negative
//! holds one such form in otherwise legal source and compiles under 2009,
//! while `legal_2001.v` executes the nearest legal forms in both editions.

use super::sim_cli;
use llg::core::compile;
use std::path::Path;

const SUITE: &str = "feature_completion/rtl_106";
const LATER: &str = "is not available in IEEE 2001";
const ARGS_2001: [&str; 2] = ["--edition", "2001"];

/// Value-backend lanes: legacy, compact portable and compact GMP (bundled
/// unless `gmp` names an installation).
fn backend_lanes(gmp: &str) -> Vec<Vec<(&'static str, String)>> {
    vec![
        vec![
            ("LLG_DEV_VALUE_BACKEND", "legacy".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "portable".to_owned()),
        ],
        vec![
            ("LLG_DEV_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "portable".to_owned()),
        ],
        vec![
            ("LLG_DEV_VALUE_BACKEND", "compact".to_owned()),
            ("LLG_DEV_COMPACT_KERNELS", "gmp".to_owned()),
            ("GMP_ROOT", gmp.to_owned()),
        ],
    ]
}

/// Nearest legal composition: named generate loops, `parameter` ports,
/// declared genvars, ranged memories, positional and named calls, macro-built
/// named blocks and calls, UDP, gate, defparam, events, fork/join, force,
/// intra-assignment NBA and edge-list controls, on every backend lane and in
/// both editions.
#[test]
fn legal_2001_composition_executes_in_both_editions() {
    let gmp = super::sim_harness::test_gmp_root();
    for lane in backend_lanes(&gmp) {
        let envs: Vec<(&str, &str)> = lane.iter().map(|(k, v)| (*k, v.as_str())).collect();
        for edition in ["2001", "2009"] {
            sim_cli::run_case_with_inputs(
                SUITE,
                "legal_2001.v",
                &[],
                include_str!("../fixtures/sim/feature_completion/rtl_106/legal_2001.out"),
                "llg: $finish at time 10 at tb:120:5\n",
                &["--edition", edition],
                &envs,
            );
        }
    }
}

/// A form built by a macro is checked on the expanded syntax and reported at
/// the macro use site.
#[test]
fn macro_built_later_forms_reject_in_2001() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_end_label.v",
        "neg_macro_end_label.v:5:3 `end label` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_cast.v",
        "neg_macro_cast.v:4:15 `cast` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_dot_name.v",
        "neg_macro_dot_name.v:6:9 `implicit named port connection` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_size_dim.v",
        "neg_macro_size_dim.v:3:15 `unpacked dimension size` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_empty_call.v",
        "neg_macro_empty_call.v:6:11 `empty subroutine argument list` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_empty_task.v",
        "neg_macro_empty_task.v:3:13 `empty subroutine argument list` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_statement_label.v",
        "neg_macro_statement_label.v:4:5 `statement label` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_param_port.v",
        "neg_macro_param_port.v:2:12 `parameter port without the parameter keyword` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_localparam_port.v",
        "neg_macro_localparam_port.v:2:31 `localparam in a parameter port list` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_loop_genvar.v",
        "neg_macro_loop_genvar.v:5:5 `genvar declaration in a generate loop` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_edge.v",
        "neg_macro_edge.v:4:12 `edge event control` is not available in IEEE 2001",
        &ARGS_2001,
    );
}

/// Keyword-free later grammar the listed token rules did not cover.
#[test]
fn keyword_free_later_forms_reject_in_2001() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_named_argument.v",
        "neg_named_argument.v:7:19 `named subroutine argument` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_function_reg_return.v",
        "neg_function_reg_return.v:2:12 `function return type` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_param_port_keyword.v",
        "neg_param_port_keyword.v:1:14 `parameter port without the parameter keyword` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_nested_assignment.v",
        "neg_nested_assignment.v:5:10 `assignment within an expression` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_unnamed_generate_loop.v",
        "neg_unnamed_generate_loop.v:5:35 `unnamed generate loop block` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_module_lifetime.v",
        "neg_module_lifetime.v:1:8 `module lifetime` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_argument_direction.v",
        "neg_argument_direction.v:2:22 `argument without a direction` is not available in IEEE 2001",
        &ARGS_2001,
    );
}

/// IEEE 1364-2001 6.1, 7.1 and 12.3.9.2: continuous assignments, gate
/// outputs and output port connections drive nets; input ports are nets.
#[test]
fn variable_drivers_reject_in_2001() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_assign_reg.v",
        "neg_assign_reg.v:3:10 `continuous assignment to a variable` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_output_reg.v",
        "neg_output_reg.v:7:19 `output port connected to a variable` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_gate_reg.v",
        "neg_gate_reg.v:4:9 `gate output connected to a variable` is not available in IEEE 2001",
        &ARGS_2001,
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_input_reg.v",
        "neg_input_reg.v:2:9 `variable input or inout port` is not available in IEEE 2001",
        &ARGS_2001,
    );
}

/// Every negative is legal SystemVerilog-2009 and fails only under 2001.
#[test]
fn component_negatives_compile_in_2009_and_fail_in_2001() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let mut checked = 0;
    for entry in std::fs::read_dir(&directory).expect("fixture directory") {
        let path = entry.expect("fixture entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !(name.starts_with("neg_") && name.ends_with(".v")) {
            continue;
        }
        let file = path.to_string_lossy().into_owned();
        for (edition, accepted) in [
            (compile::LanguageEdition::SystemVerilog2009, true),
            (compile::LanguageEdition::Verilog2001, false),
        ] {
            let result = compile::compile_checked(&compile::CompileOpts {
                files: vec![file.clone()],
                top: Some("tb".to_owned()),
                edition,
                ..Default::default()
            });
            assert_eq!(result.is_ok(), accepted, "{name} under {edition}");
        }
        checked += 1;
    }
    assert_eq!(checked, 23);
}

/// Slang diagnostics in a `` `line`` region, in a macro body used there and
/// in a header after its own directive print the physical position first and
/// then the mapped one; a 2001 later form in a `` `line`` region likewise.
#[test]
fn frontend_diagnostics_append_the_line_mapped_position() {
    for edition in ["2001", "2009"] {
        let args = ["--edition", edition];
        if edition == "2009" {
            sim_cli::reject_case_with_args(
                SUITE,
                "line_slang_error",
                "line_slang_error.sv:5:9 (`line orig_rtl.sv:42) use of undeclared identifier 'undefined_name'",
                &args,
            );
            sim_cli::reject_case_with_args(
                SUITE,
                "line_macro_error",
                "line_macro_error.sv:6:5 (`line gen_top.sv:201) use of undeclared identifier 'missing_signal'",
                &args,
            );
        }
        sim_cli::reject_case_with_args(
            SUITE,
            "line_include_error",
            "line_include_error.svh:3:15 (`line orig_header.svh:7) use of undeclared identifier 'no_such_name'",
            &args,
        );
    }
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_macro_line.v",
        &format!("neg_macro_line.v:6:3 (`line orig_rtl.v:72) `end label` {LATER}"),
        &["--edition", "2001"],
    );
    // A diagnostic before the directive stays physical only.
    let output = sim_cli::invoke_with_env(
        SUITE,
        "line_macro_error",
        true,
        &["--edition", "2009"],
        &[],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("line_macro_error.sv:3:9 variable 'a'"),
        "{stderr}"
    );
}

/// Lint findings carry the mapped position in the text and JSON reports and
/// do not change the simulated result.
#[test]
fn lint_findings_append_the_line_mapped_position() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("line_lint.v");
    let stderr = format!(
        "{}:7:5 (`line orig_lint.v:31): [WARNING] incomplete-case: case without default in combinational process may infer a latch\n\
         lint: 0 error(s), 1 warning(s)\n\
         llg: $finish at time 1 at tb:15:5\n",
        fixture.display()
    );
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "line_lint.v",
            "y=1\n",
            &stderr,
            &[],
            &["--edition", edition, "--lint"],
        );
    }
    // `--lint-json` takes a following operand as its output path.
    let output = sim_cli::invoke_with_env(
        SUITE,
        "line_lint.v",
        true,
        &["--lint-json", "--edition", "2009"],
        &[],
        &[],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"logical_file\": \"orig_lint.v\",\n      \"logical_line\": 31,"),
        "{stdout}"
    );
}

/// The mapped position is owned by the diagnostic and survives the snapshot.
#[test]
fn component_mapped_diagnostic_positions_survive_snapshot_destruction() {
    let source = "module tb;\n`line 40 \"orig_rtl.sv\" 0\n  logic a;\n  initial a = undefined_name;\nendmodule\n";
    let error = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit("mapped.sv", source)],
        &compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("undeclared identifier");
    let diagnostics = error.into_diagnostics().expect("frontend diagnostics");
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("undefined_name"))
        .expect("undeclared identifier");
    assert_eq!((error.line, error.col), (4, 15));
    assert_eq!(
        error.logical,
        Some(compile::LogicalLine {
            file: "orig_rtl.sv".to_owned(),
            line: 41,
        })
    );
    assert_eq!(error.location(), "mapped.sv:4:15 (`line orig_rtl.sv:41)");
}
