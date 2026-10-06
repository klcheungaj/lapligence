//! SYN-038 blocking-written sources passed to task and function input formals.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const TASK_FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/blocking_written_task_input.sv";
const TASK_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/blocking_written_task_input.sv");
const FUNCTION_FIXTURE: &str =
    "tests/fixtures/sim/syn038_pairwise/blocking_written_function_input.sv";
const FUNCTION_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/blocking_written_function_input.sv");

fn compile_fixture(fixture: &str) -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 written-input fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn outer_variable(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
                && db.node(*id).parent() == Some(top)
        })
        .unwrap_or_else(|| panic!("outer variable `{name}` is captured under tb"))
}

fn reference_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct reference, got {other:?}"),
    }
}

fn source_line(source: &str, anchor: &str) -> u32 {
    source
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture source is missing `{anchor}`")) as u32
        + 1
}

fn source_line_after(source: &str, after_line: u32, anchor: &str) -> u32 {
    source
        .lines()
        .enumerate()
        .skip(after_line as usize)
        .find_map(|(index, line)| (line.trim() == anchor).then_some(index as u32 + 1))
        .unwrap_or_else(|| panic!("fixture source is missing `{anchor}` after line {after_line}"))
}

fn process_ancestor(db: &Db, mut node: NodeId) -> NodeId {
    loop {
        if matches!(db.node_kind(node), NodeKind::Process { .. }) {
            return node;
        }
        node = db
            .node(node)
            .parent()
            .unwrap_or_else(|| panic!("node {node:?} has no process ancestor"));
    }
}

fn assert_written_input_call(
    db: &Db,
    source_text: &str,
    call_anchor: &str,
    expected_call_name: &str,
    expected_task: bool,
) {
    let top = top_module(db);
    let source = outer_variable(db, top, "source");
    let writer_line = source_line(source_text, "source = 8'h5a;");
    let writers = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == writer_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: true,
                        op: Operation::Assignment,
                        delay: None,
                    })
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(writers.len(), 1, "one whole-object blocking write");
    let writer = writers[0];
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer retains LHS and RHS");
    let writer_target = reference_target(db, writer_children[0]);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(source)
    );

    let call_line = source_line_after(source_text, writer_line, call_anchor);
    assert!(
        writer_line < call_line,
        "blocking write precedes input call"
    );
    let calls = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == call_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall { name, is_task, .. }
                        if name == expected_call_name && *is_task == expected_task
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one matching task/function call");
    let call = calls[0];
    let NodeKind::FuncCall { callee, .. } = db.node_kind(call) else {
        unreachable!("filtered to a subroutine call")
    };
    let callee = callee.expect("resolved call retains its task or function");
    let NodeKind::FuncTask { is_task, .. } = db.node_kind(callee) else {
        panic!("call resolves to a subroutine declaration")
    };
    assert_eq!(*is_task, expected_task, "callee kind matches call kind");
    let formals = db
        .node(callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), 1, "callee has one explicit input formal");
    let formal = formals[0];
    let NodeKind::FuncArg { direction, .. } = db.node_kind(formal) else {
        unreachable!("filtered to a subroutine argument")
    };
    assert_eq!(*direction, Direction::Input);
    assert_eq!(db.node(formal).name(), "value");

    let actual_targets = db
        .node(call)
        .children()
        .iter()
        .copied()
        .filter_map(|actual| match db.node_kind(actual) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_targets.len(),
        1,
        "call retains one direct input actual"
    );
    let actual_target = actual_targets[0];
    assert_eq!(
        db.source_identity(actual_target),
        db.source_identity(source)
    );
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(actual_target),
        "blocking LHS and actual expression share the outer source identity"
    );

    let writer_process = process_ancestor(db, writer);
    let call_process = process_ancestor(db, call);
    assert_eq!(
        writer_process, call_process,
        "write and call share a process"
    );
    assert!(matches!(
        db.node_kind(writer_process),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));
}

#[test]
fn blocking_written_task_input_matches_both_cli_modes() {
    assert!(TASK_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/blocking_written_task_input.sv\n"
    ));
    for anchor in [
        "task automatic capture(input logic [7:0] value);",
        "capture(source);",
        "source = 8'h5a;",
        "observed !== 8'h5a",
    ] {
        assert!(
            TASK_SOURCE.contains(anchor),
            "missing task fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "blocking_written_task_input",
        "call_source=5a/5a\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_blocking_lhs_and_task_actual_share_outer_source_identity() {
    let db = compile_fixture(TASK_FIXTURE);
    assert_written_input_call(&db, TASK_SOURCE, "capture(source);", "capture", true);
}

#[test]
fn blocking_written_function_input_matches_both_cli_modes() {
    assert!(FUNCTION_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/blocking_written_function_input.sv\n"
    ));
    for anchor in [
        "function automatic logic [7:0] capture(input logic [7:0] value);",
        "observed = capture(source);",
        "source = 8'h5a;",
        "observed !== 8'h5a",
    ] {
        assert!(
            FUNCTION_SOURCE.contains(anchor),
            "missing function fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "blocking_written_function_input",
        "call_function=5a/5a\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_blocking_lhs_and_function_actual_share_outer_source_identity() {
    let db = compile_fixture(FUNCTION_FIXTURE);
    assert_written_input_call(
        &db,
        FUNCTION_SOURCE,
        "observed = capture(source);",
        "capture",
        false,
    );
}
