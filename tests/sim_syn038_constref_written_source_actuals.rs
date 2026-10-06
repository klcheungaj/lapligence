//! SYN-038 previously written whole variables passed through const-ref actuals.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv");
const EXPECTED_STDOUT: &str =
    "constref=31,42/81,53/16/75,51,62/76,a7,b8/77,8d,9e/78\nextra=86/97\n";

#[derive(Clone, Copy, Debug)]
enum LvalueShape {
    WholeObject,
    Field,
    Element,
    RowSlice,
    Concatenation,
    PositionalPattern,
}

#[derive(Clone, Copy, Debug)]
enum WriterKind {
    Blocking,
    Nonblocking,
    ContinuousVariable,
}

struct SourceCase {
    source: &'static str,
    writer: &'static str,
    call: &'static str,
    task: &'static str,
    result: &'static str,
    shape: LvalueShape,
    writer_kind: WriterKind,
}

const CASES: [SourceCase; 8] = [
    SourceCase {
        source: "whole_source",
        writer: "whole_source = 8'h31;",
        call: "capture_whole(whole_source);",
        task: "capture_whole",
        result: "whole_result",
        shape: LvalueShape::WholeObject,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "field_source",
        writer: "field_source.selected = 8'h42;",
        call: "capture_field(field_source);",
        task: "capture_field",
        result: "field_result",
        shape: LvalueShape::Field,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "element_source",
        writer: "element_source[1] = 8'h53;",
        call: "capture_element(element_source);",
        task: "capture_element",
        result: "element_result",
        shape: LvalueShape::Element,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "row_source",
        writer: "row_source[0:1] = '{0: 8'h51, 1: 8'h62};",
        call: "capture_row(row_source);",
        task: "capture_row",
        result: "row_result",
        shape: LvalueShape::RowSlice,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "concat_source",
        writer: "{concat_source[1], concat_source[0]} = {8'hA7, 8'hB8};",
        call: "capture_concat(concat_source);",
        task: "capture_concat",
        result: "concat_result",
        shape: LvalueShape::Concatenation,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "pattern_source",
        writer: "byte_pair_t'{pattern_source[0], pattern_source[1]} = '{0: 8'h8d, 1: 8'h9e};",
        call: "capture_pattern(pattern_source);",
        task: "capture_pattern",
        result: "pattern_result",
        shape: LvalueShape::PositionalPattern,
        writer_kind: WriterKind::Blocking,
    },
    SourceCase {
        source: "nba_source",
        writer: "nba_source <= 8'h86;",
        call: "capture_nba(nba_source);",
        task: "capture_nba",
        result: "nba_result",
        shape: LvalueShape::WholeObject,
        writer_kind: WriterKind::Nonblocking,
    },
    SourceCase {
        source: "continuous_source",
        writer: "assign continuous_source = 8'h97;",
        call: "capture_continuous(continuous_source);",
        task: "capture_continuous",
        result: "continuous_result",
        shape: LvalueShape::WholeObject,
        writer_kind: WriterKind::ContinuousVariable,
    },
];

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 const-ref written-source fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    let lines = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "one source anchor `{anchor}`");
    lines[0]
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn outer_object(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && db.node(*id).parent() == Some(top)
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Array { .. } | NodeKind::Net { .. } | NodeKind::Var { .. }
                )
        })
        .unwrap_or_else(|| panic!("outer object `{name}` is captured under tb"))
}

fn assignment_at_line(db: &Db, line: u32, writer_kind: WriterKind) -> NodeId {
    db.node_ids()
        .find(|id| {
            if db.node(*id).line() != line {
                return false;
            }
            match writer_kind {
                WriterKind::Blocking => matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: true,
                        op: Operation::Assignment,
                        delay: None,
                    })
                ),
                WriterKind::Nonblocking => matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: false,
                        op: Operation::Assignment,
                        delay: None,
                    })
                ),
                WriterKind::ContinuousVariable => {
                    matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
                }
            }
        })
        .unwrap_or_else(|| panic!("line {line} has the expected {writer_kind:?} writer"))
}

fn references(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut targets = Vec::new();
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        match db.node_kind(id) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => targets.push(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                targets.extend(refs.iter().flatten().copied());
            }
            _ => {}
        }
        pending.extend(db.node(id).children().iter().copied());
    }
    targets
}

fn refers_to_identity(db: &Db, root: NodeId, identity: NodeId) -> bool {
    references(db, root)
        .into_iter()
        .any(|target| db.source_identity(target) == identity)
}

fn assert_writer_shape(db: &Db, lhs: NodeId, source: NodeId, shape: LvalueShape) {
    let identity = db.source_identity(source);
    match (shape, db.node_kind(lhs)) {
        (LvalueShape::WholeObject, NodeKind::Expr(ExprKind::Ref { target })) => {
            assert_eq!(
                db.source_identity(target.expect("whole writer binds its source")),
                identity
            );
        }
        (LvalueShape::Field, NodeKind::Expr(ExprKind::HierPath { parts, refs })) => {
            assert_eq!(parts, &["field_source", "selected"]);
            assert!(refs
                .iter()
                .flatten()
                .any(|target| db.source_identity(*target) == identity));
        }
        (LvalueShape::Element, NodeKind::Expr(ExprKind::ArraySelect { base, indices })) => {
            assert_eq!(indices.len(), 1);
            assert_eq!(db.source_identity(*base), identity);
        }
        (LvalueShape::RowSlice, NodeKind::Expr(ExprKind::PartSelect { base, .. })) => {
            assert!(refers_to_identity(db, *base, identity));
        }
        (
            LvalueShape::Concatenation,
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }),
        ) => {
            assert_eq!(operands.len(), 2);
            assert!(operands
                .iter()
                .all(|operand| refers_to_identity(db, *operand, identity)));
        }
        (
            LvalueShape::PositionalPattern,
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                operands,
                ..
            }),
        ) => {
            assert_eq!(operands.len(), 2);
            assert!(operands
                .iter()
                .all(|operand| refers_to_identity(db, *operand, identity)));
        }
        (shape, kind) => panic!("expected {shape:?} writer LHS, got {kind:?}"),
    }
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

fn assert_case(db: &Db, top: NodeId, case: &SourceCase) {
    let source = outer_object(db, top, case.source);
    let result = outer_object(db, top, case.result);
    let sibling = outer_object(db, top, "sibling_control");
    let identity = db.source_identity(source);
    assert!(matches!(
        db.node_kind(source),
        NodeKind::Array { .. } | NodeKind::Var { .. }
    ));
    let writer_line = source_line(case.writer);
    let call_line = source_line(case.call);
    assert!(
        writer_line < call_line,
        "the selected writer precedes its whole-variable const-ref call"
    );
    if matches!(case.writer_kind, WriterKind::ContinuousVariable) {
        assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
    }

    let writer = assignment_at_line(db, writer_line, case.writer_kind);
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer retains LHS and RHS");
    assert_writer_shape(db, writer_children[0], source, case.shape);
    assert!(refers_to_identity(db, writer_children[0], identity));

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
                    } if name == case.task
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one `{}` call", case.task);
    let call = calls[0];
    let actuals = db.node(call).children();
    assert_eq!(actuals.len(), 1, "one whole-variable actual");
    let NodeKind::Expr(ExprKind::Ref {
        target: Some(actual_target),
    }) = db.node_kind(actuals[0])
    else {
        panic!("const-ref actual is a direct whole-variable reference")
    };
    assert_eq!(db.source_identity(*actual_target), identity);
    assert_eq!(
        db.source_identity(
            references(db, writer_children[0])
                .into_iter()
                .find(|target| db.source_identity(*target) == identity)
                .expect("writer LHS refers to the focal source")
        ),
        db.source_identity(*actual_target),
        "writer LHS and const-ref actual share one owned source identity"
    );

    let NodeKind::FuncCall {
        callee: Some(callee),
        ..
    } = db.node_kind(call)
    else {
        panic!("const-ref task call retains its resolved callee")
    };
    let formals = db
        .node(*callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), 1, "task has one formal");
    let formal = formals[0];
    assert!(matches!(
        db.node_kind(formal),
        NodeKind::FuncArg {
            direction: Direction::Ref,
            const_ref: true,
            ..
        }
    ));
    assert!(matches!(
        db.node_kind(process_ancestor(db, call)),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));
    let formal_identity = db.source_identity(formal);
    let result_identity = db.source_identity(result);
    let sibling_identity = db.source_identity(sibling);
    assert_ne!(formal_identity, identity);
    assert_ne!(result_identity, identity);
    assert_ne!(sibling_identity, identity);
    assert_ne!(formal_identity, result_identity);
    assert_ne!(formal_identity, sibling_identity);
    assert_ne!(result_identity, sibling_identity);
}

#[test]
fn prior_writes_reach_whole_const_ref_actuals_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv\n"
    ));
    for case in &CASES {
        assert!(
            FIXTURE_SOURCE.contains(case.writer),
            "missing `{}`",
            case.writer
        );
        assert!(
            FIXTURE_SOURCE.contains(case.call),
            "missing `{}`",
            case.call
        );
    }
    for oracle in [
        "whole const-ref source mismatch",
        "field const-ref source mismatch",
        "element const-ref source mismatch",
        "row-slice const-ref source mismatch",
        "concatenation const-ref source mismatch",
        "positional-pattern const-ref source mismatch",
        "NBA const-ref source mismatch",
        "continuous-variable const-ref source mismatch",
        "independent sibling source changed",
    ] {
        assert!(FIXTURE_SOURCE.contains(oracle), "missing oracle `{oracle}`");
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "constref_written_source_actuals",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_prior_writes_and_const_ref_actuals_share_source_identity() {
    let db = compile_fixture();
    let top = top_module(&db);
    for case in &CASES {
        assert_case(&db, top, case);
    }
}
