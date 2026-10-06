//! SYN-038 event expressions reading module and formal signals from subroutines.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, EventSpec, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv");
const EXPECTED_STDOUT: &str = "module=1 formal=1\n";
const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "task automatic wait_module_source();",
    "@(module_source);",
    "#1 module_source = 1'b1;",
    "task wait_formal(input bit source, input bit observer);",
    "@(source);",
    "#1 source = 1'b1;",
    "wait_module_source();",
    "wait_formal(1'b0, 1'b1)",
    "wait_formal(1'b0, 1'b0)",
];

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 subroutine event-source fixture compiles");
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

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn module_variable(db: &Db, name: &str) -> NodeId {
    let top = top_module(db);
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && db.node(*id).parent() == Some(top)
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one module variable `{name}`");
    matches[0]
}

fn task(db: &Db, name: &str) -> NodeId {
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: true, .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one task `{name}`");
    matches[0]
}

fn task_input(db: &Db, task: NodeId, name: &str) -> NodeId {
    let matches = db
        .node(task)
        .children()
        .iter()
        .copied()
        .filter(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncArg {
                        direction: Direction::Input,
                        ..
                    }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one input formal `{name}` on task");
    matches[0]
}

fn direct_ref_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected direct source reference, got {other:?}"),
    }
}

fn event_operand(db: &Db, anchor: &str) -> (NodeId, NodeId) {
    let line = source_line(anchor);
    let matches = db
        .node_ids()
        .filter_map(|id| {
            if db.node(id).line() != line {
                return None;
            }
            let NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit: false,
                ..
            }) = db.node_kind(id)
            else {
                return None;
            };
            match specs.as_slice() {
                [EventSpec::AnyChange { sig }] => Some((id, *sig)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "one explicit any-change event on line {line}"
    );
    matches[0]
}

fn blocking_writer(db: &Db, anchor: &str) -> (NodeId, NodeId) {
    let line = source_line(anchor);
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == line
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
    assert_eq!(
        matches.len(),
        1,
        "one explicit blocking assignment on line {line}"
    );
    let writer = matches[0];
    let children = db.node(writer).children();
    assert_eq!(children.len(), 2, "blocking writer retains LHS and RHS");
    (writer, children[0])
}

fn task_ancestor(db: &Db, mut node: NodeId) -> Option<NodeId> {
    loop {
        if matches!(db.node_kind(node), NodeKind::FuncTask { is_task: true, .. }) {
            return Some(node);
        }
        node = db.node(node).parent()?;
    }
}

fn process_ancestor(db: &Db, mut node: NodeId) -> Option<NodeId> {
    loop {
        if matches!(db.node_kind(node), NodeKind::Process { .. }) {
            return Some(node);
        }
        node = db.node(node).parent()?;
    }
}

fn assert_writer_and_event_share_source(
    db: &Db,
    source: NodeId,
    write_anchor: &str,
    event_anchor: &str,
) -> (NodeId, NodeId) {
    let (event, operand) = event_operand(db, event_anchor);
    let (writer, lhs) = blocking_writer(db, write_anchor);
    let event_target = direct_ref_target(db, operand);
    let write_target = direct_ref_target(db, lhs);
    let source_identity = db.source_identity(source);
    assert_eq!(
        db.source_identity(event_target),
        source_identity,
        "event operand resolves to the focal outer source"
    );
    assert_eq!(
        db.source_identity(write_target),
        source_identity,
        "explicit writer LHS resolves to the focal outer source"
    );
    assert_eq!(
        db.source_identity(write_target),
        db.source_identity(event_target),
        "event operand and explicit writer share one Slang source identity"
    );
    (writer, event)
}

#[test]
fn subroutine_event_sources_observe_transitions_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source anchor: {anchor}"
        );
    }
    for assertion in [
        "if (!module_seen)",
        "if (!formal_seen)",
        "$display(\"module=%0d formal=%0d\", module_seen, formal_seen);",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost immediate source oracle: {assertion}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "subroutine_event_signal_matrix",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_event_operands_and_blocking_writers_share_outer_source_identity() {
    let db = compile_fixture();

    let module_source = module_variable(&db, "module_source");
    let module_task = task(&db, "wait_module_source");
    assert!(matches!(
        db.node_kind(module_task),
        NodeKind::FuncTask {
            automatic: true,
            ..
        }
    ));
    let (module_writer, module_event) = assert_writer_and_event_share_source(
        &db,
        module_source,
        "#1 module_source = 1'b1;",
        "@(module_source);",
    );
    assert_eq!(task_ancestor(&db, module_event), Some(module_task));
    let module_writer_process = process_ancestor(&db, module_writer)
        .expect("module source writer is owned by an initial process");
    assert!(matches!(
        db.node_kind(module_writer_process),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));

    let formal_task = task(&db, "wait_formal");
    assert!(matches!(
        db.node_kind(formal_task),
        NodeKind::FuncTask {
            automatic: false,
            ..
        }
    ));
    let source_formal = task_input(&db, formal_task, "source");
    let (formal_writer, formal_event) =
        assert_writer_and_event_share_source(&db, source_formal, "#1 source = 1'b1;", "@(source);");
    assert_eq!(task_ancestor(&db, formal_writer), Some(formal_task));
    assert_eq!(task_ancestor(&db, formal_event), Some(formal_task));
}
