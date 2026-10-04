//! SYN-038 typed expression operations across storage and lexical contexts.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeId, NodeKind, Operation, StmtKind},
};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/operation_context_matrix.sv");
const EXPECTED_STDOUT: &[u8] = b"types=1122,0,0,11223344,5162 task=25,0,25,2534 formal=2534 constref=34,34,34,34,5a locals=25,25,25,25,25 snapshot=00,00 constant=22,3 generate=25,25,2534 interface=25,25,2534\nunion-pattern=12,34\nfunction-types=2,56 event=1\n";
const EXPECTED_STDERR: &[u8] = b"";
const CONST_REF_EXPRESSION_DIAGNOSTIC: &str = "invalid expression for pass by reference; only variables, class properties, and members of unpacked structs and arrays are allowed";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 operation-context fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    let matches = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one source anchor `{anchor}`");
    matches[0]
}

fn source_line_after(after_anchor: &str, anchor: &str) -> u32 {
    let after_line = source_line(after_anchor);
    let matches = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .skip(after_line as usize)
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "one `{anchor}` source anchor after `{after_anchor}`"
    );
    matches[0]
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn module_variable(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
                && db.node(*id).parent() == Some(top)
        })
        .unwrap_or_else(|| panic!("module variable `{name}` is captured under tb"))
}

fn module_variable_at_line(db: &Db, top: NodeId, name: &str, line: u32) -> NodeId {
    let variable = module_variable(db, top, name);
    assert_eq!(
        db.node(variable).line(),
        line,
        "`{name}` source declaration line"
    );
    variable
}

fn direct_reference_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct reference, got {other:?}"),
    }
}

fn assignment_at_line(db: &Db, line: u32) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign { blocking: true, .. })
                )
        })
        .unwrap_or_else(|| panic!("line {line} has a blocking assignment"))
}

const CONST_REF_NEGATIVES: [(&str, &str, usize, usize, &str, &str); 4] = [
    (
        "constref_conditional_rejected.sv",
        include_str!("fixtures/sim/syn038_pairwise/constref_conditional_rejected.sv"),
        14,
        17,
        "capture(variable_actual);",
        "capture(1'b1 ? variable_actual : 8'h34);",
    ),
    (
        "constref_equality_rejected.sv",
        include_str!("fixtures/sim/syn038_pairwise/constref_equality_rejected.sv"),
        13,
        17,
        "capture(variable_actual);",
        "capture(variable_actual == 1'b1);",
    ),
    (
        "constref_cast_rejected.sv",
        include_str!("fixtures/sim/syn038_pairwise/constref_cast_rejected.sv"),
        16,
        17,
        "capture(variable_actual);",
        "capture(byte_t'(source_value));",
    ),
    (
        "constref_pattern_rejected.sv",
        include_str!("fixtures/sim/syn038_pairwise/constref_pattern_rejected.sv"),
        18,
        17,
        "capture(variable_actual);",
        "capture(pair_t'{hi: left_value, lo: right_value});",
    ),
];

#[test]
fn typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv\n"
    ));
    for anchor in [
        "typedef logic [(int'(pair_t'{hi: CONST_LEFT, lo: CONST_RIGHT}) % 4):0] pattern_width_t;",
        "packed_conditional = choose_left ? packed_left : packed_right;",
        "packed_equality = packed_left == packed_right;",
        "records_equality = records_left == records_right;",
        "records_cast = bits32_t'(records_left);",
        "record_cast = bits16_t'(record_source);",
        "union_pattern_result = '{union_pattern_left, union_pattern_right};",
        "capture_ops(choose_left ? left_value : right_value,",
        "formal_pattern = '{hi: pattern_hi, lo: pattern_lo};",
        "static bit static_event_source;",
        "@(tb.drive_static_event_source.static_event_source);",
        "function automatic color_t echo_color(input color_t value);",
        "function automatic union_t echo_union(input union_t value);",
        "capture_selected(constref_lanes[choose_left ? 1 : 0], constref_conditional);",
        "capture_selected(constref_lanes[compare_left == compare_right], constref_equality);",
        "capture_selected(constref_lanes[index_bit_t'(cast_index)], constref_cast);",
        "capture_selected(constref_lanes[index_bit_t'{pattern_index}], constref_pattern);",
        "capture_selected(constref_scalar_control, constref_scalar_observed);",
        "static_result = select_left ? static_left : static_right;",
        "automatic_result = select_left ? automatic_left : automatic_right;",
        "automatic byte_t initialized_value = choose_left ? left_value : right_value;",
        "static byte_t initialized_value = STATIC_INIT_SELECT ? STATIC_INIT_LEFT : STATIC_INIT_RIGHT;",
        "static byte_t initialized_value = static_snapshot_source;",
        "generated_conditional_value = choose_left ? left_value : right_value;",
        "generated_cast_value = two_state_t'(left_value);",
        "generated_pattern_value = '{hi: left_value, lo: right_value};",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost focal source anchor: {anchor}"
        );
    }
    for assertion in [
        "packed struct operation mismatch",
        "unpacked record operation mismatch",
        "union sources in array pattern mismatch",
        "typed function argument mismatch",
        "hierarchical static-local event mismatch",
        "task operation mismatch",
        "formal assignment-pattern operation mismatch",
        "const-ref variable actual mismatch",
        "subroutine local operation mismatch",
        "static initializer snapshot mismatch",
        "static initializer default snapshot mismatch",
        "constant operation mismatch",
        "generate operation mismatch",
        "interface operation mismatch",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost result check: {assertion}"
        );
    }

    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv");

    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("syn038-operation-context-matrix")
            .expect("CLI test directory");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(["--edition", "2009"]).arg(&fixture_path);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .unwrap_or_else(|error| {
                panic!("syn038_pairwise/operation_context_matrix, optimized={optimized}: {error}")
            });
        let label = format!("syn038_pairwise/operation_context_matrix, optimized={optimized}");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{label}: stderr={} stdout={}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, "{label}");
        assert_eq!(output.stderr.as_slice(), EXPECTED_STDERR, "{label}");

        let negative_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim/syn038_pairwise");
        for (filename, source, line, column, variable_actual, expression_actual) in
            CONST_REF_NEGATIVES
        {
            let header =
                format!("// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/{filename}\n");
            assert!(
                source.starts_with(&header),
                "{filename}: bad fixture header"
            );
            assert!(
                source.contains(variable_actual),
                "{filename}: missing variable control"
            );
            assert!(
                source.contains(expression_actual),
                "{filename}: missing expression focal"
            );

            let negative_path = negative_root.join(filename);
            let negative_directory = sim_harness::TempDir::new("syn038-constref-negative")
                .expect("negative CLI test directory");
            let mut negative_command = Command::new(env!("CARGO_BIN_EXE_llg"));
            negative_command
                .current_dir(negative_directory.path())
                .args(["--top", "tb"]);
            if !optimized {
                negative_command.arg("--no-opt");
            }
            negative_command
                .args(["--edition", "2009"])
                .arg(&negative_path);
            let negative_output =
                sim_harness::run_command(&mut negative_command, Duration::from_secs(180))
                    .unwrap_or_else(|error| panic!("{filename}, optimized={optimized}: {error}"));
            let negative_label = format!("{filename}, optimized={optimized}");
            assert_eq!(
                negative_output.status.code(),
                Some(1),
                "{negative_label}: stdout={} stderr={}",
                String::from_utf8_lossy(&negative_output.stdout),
                String::from_utf8_lossy(&negative_output.stderr)
            );
            assert!(negative_output.stdout.is_empty(), "{negative_label}");
            let expected_negative_stderr = format!(
                "Error: {}:{line}:{column} {CONST_REF_EXPRESSION_DIAGNOSTIC}\nllg: Slang reported errors; aborting\n",
                sim_harness::source_display(&negative_path)
            );
            assert_eq!(
                negative_output.stderr.as_slice(),
                expected_negative_stderr.as_bytes(),
                "{negative_label}"
            );
        }
    }
}

#[test]
fn conditional_task_actual_tracks_the_blocking_written_source_identity() {
    assert!(FIXTURE_SOURCE.contains("task_conditional !== 8'h25"));
    let db = compile_fixture();
    let top = top_module(&db);
    let choose_left = module_variable(&db, top, "choose_left");
    let left_value = module_variable_at_line(
        &db,
        top,
        "left_value",
        source_line_after("module tb;", "byte_t left_value, right_value;"),
    );
    let right_value = module_variable(&db, top, "right_value");
    let task_result = module_variable(&db, top, "task_conditional");

    let selector_write = assignment_at_line(&db, source_line("choose_left = 1'b1;"));
    let selector_target = direct_reference_target(&db, db.node(selector_write).children()[0]);
    assert_eq!(
        db.source_identity(selector_target),
        db.source_identity(choose_left)
    );

    let writer = assignment_at_line(&db, source_line("left_value = 8'h25;"));
    let writer_target = direct_reference_target(&db, db.node(writer).children()[0]);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(left_value)
    );

    let call_line = source_line("capture_ops(choose_left ? left_value : right_value,");
    let calls = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == call_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall {
                        name,
                        is_task: true,
                        ..
                    } if name == "capture_ops"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one focal capture_ops task call");
    let actuals = db.node(calls[0]).children();
    assert_eq!(actuals.len(), 4, "capture_ops retains each source actual");
    let NodeKind::Expr(ExprKind::Operation {
        op: Operation::Conditional,
        operands,
        ..
    }) = db.node_kind(actuals[0])
    else {
        panic!("first task actual retains its conditional operation")
    };
    assert_eq!(
        operands.len(),
        3,
        "conditional retains selector and both branches"
    );
    let selector = direct_reference_target(&db, operands[0]);
    let selected_source = direct_reference_target(&db, operands[1]);
    let sibling_source = direct_reference_target(&db, operands[2]);
    assert_eq!(
        db.source_identity(selector),
        db.source_identity(choose_left)
    );
    assert_eq!(
        db.source_identity(selected_source),
        db.source_identity(left_value)
    );
    assert_eq!(
        db.source_identity(sibling_source),
        db.source_identity(right_value)
    );
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(selected_source),
        "blocking writer LHS and true conditional task actual share source identity"
    );
    assert_ne!(
        db.source_identity(selected_source),
        db.source_identity(sibling_source),
        "the right branch remains a distinct sibling source"
    );
    assert_ne!(
        db.source_identity(selected_source),
        db.source_identity(task_result),
        "the call source remains distinct from its task result destination"
    );
}
