//! SYN-038 selected blocking writes consumed through task input actuals.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeId, NodeKind, Operation, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv");
const EXPECTED_STDOUT: &str = "selected-inputs=31/80,42/17,51,52/73,a6/74,5c,7d/75\n";

#[derive(Clone, Copy, Debug)]
enum LvalueShape {
    Field,
    Element,
    RowSlice,
    Concatenation,
    PositionalPattern,
}

#[derive(Clone, Copy, Debug)]
enum InputActualShape {
    Field,
    Element,
    RowSlice,
    WholeObject,
}

struct SourceCase {
    source: &'static str,
    sibling: &'static str,
    writer: &'static str,
    task: &'static str,
    call: &'static str,
    result: &'static str,
    writer_shape: LvalueShape,
    actual_shape: InputActualShape,
}

const CASES: [SourceCase; 5] = [
    SourceCase {
        source: "field_source",
        sibling: "field_sibling",
        writer: "field_source.selected = 8'h31;",
        task: "capture_field",
        call: "capture_field(field_source.selected);",
        result: "field_result",
        writer_shape: LvalueShape::Field,
        actual_shape: InputActualShape::Field,
    },
    SourceCase {
        source: "element_source",
        sibling: "element_sibling",
        writer: "element_source[1] = 8'h42;",
        task: "capture_element",
        call: "capture_element(element_source[1]);",
        result: "element_result",
        writer_shape: LvalueShape::Element,
        actual_shape: InputActualShape::Element,
    },
    SourceCase {
        source: "row_source",
        sibling: "row_sibling",
        writer: "row_source[0:1] = '{0: 8'h51, 1: 8'h52};",
        task: "capture_row",
        call: "capture_row(row_source[0:1]);",
        result: "row_result",
        writer_shape: LvalueShape::RowSlice,
        actual_shape: InputActualShape::RowSlice,
    },
    SourceCase {
        source: "concat_source",
        sibling: "concat_sibling",
        writer: "{concat_source[1], concat_source[0]} = {8'hA6, 8'hC2};",
        task: "capture_concat",
        call: "capture_concat(concat_source[1]);",
        result: "concat_result",
        writer_shape: LvalueShape::Concatenation,
        actual_shape: InputActualShape::Element,
    },
    SourceCase {
        source: "pattern_source",
        sibling: "pattern_sibling",
        writer: "byte_pair_t'{pattern_source[0], pattern_source[1]} = '{0: 8'h5c, 1: 8'h7d};",
        task: "capture_pattern",
        call: "capture_pattern(pattern_source);",
        result: "pattern_result",
        writer_shape: LvalueShape::PositionalPattern,
        actual_shape: InputActualShape::WholeObject,
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
    .expect("legal SV2009 selected-source input fixture compiles");
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

fn assignment_at_line(db: &Db, line: u32) -> NodeId {
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
    assert_eq!(matches.len(), 1, "line {line} has one blocking assignment");
    matches[0]
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

fn assert_lvalue_shape(db: &Db, lhs: NodeId, source: NodeId, shape: LvalueShape) {
    let source_identity = db.source_identity(source);
    match (shape, db.node_kind(lhs)) {
        (LvalueShape::Field, NodeKind::Expr(ExprKind::HierPath { parts, refs })) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0], "field_source");
            assert_eq!(parts[1], "selected");
            assert!(refs
                .iter()
                .flatten()
                .any(|target| { db.source_identity(*target) == source_identity }));
        }
        (LvalueShape::Element, NodeKind::Expr(ExprKind::ArraySelect { base, indices })) => {
            assert_eq!(indices.len(), 1);
            assert_eq!(db.source_identity(*base), source_identity);
        }
        (LvalueShape::RowSlice, NodeKind::Expr(ExprKind::PartSelect { base, .. })) => {
            assert!(refers_to_identity(db, *base, source_identity));
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
            assert!(operands.iter().all(|operand| refers_to_identity(
                db,
                *operand,
                source_identity
            )));
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
            assert!(operands.iter().all(|operand| refers_to_identity(
                db,
                *operand,
                source_identity
            )));
        }
        (shape, kind) => panic!("expected {shape:?} lvalue, got {kind:?}"),
    }
}

fn assert_input_actual_shape(db: &Db, actual: NodeId, source: NodeId, shape: InputActualShape) {
    let source_identity = db.source_identity(source);
    match (shape, db.node_kind(actual)) {
        (InputActualShape::Field, NodeKind::Expr(ExprKind::HierPath { parts, refs })) => {
            assert_eq!(parts, &["field_source", "selected"]);
            assert!(refs
                .iter()
                .flatten()
                .any(|target| db.source_identity(*target) == source_identity));
        }
        (InputActualShape::Element, NodeKind::Expr(ExprKind::ArraySelect { base, indices })) => {
            assert_eq!(indices.len(), 1);
            assert_eq!(db.source_identity(*base), source_identity);
        }
        (InputActualShape::RowSlice, NodeKind::Expr(ExprKind::PartSelect { base, .. })) => {
            assert!(refers_to_identity(db, *base, source_identity));
        }
        (InputActualShape::WholeObject, NodeKind::Expr(ExprKind::Ref { target })) => {
            let target = target.expect("whole-array input actual binds its source");
            assert_eq!(db.source_identity(target), source_identity);
        }
        (shape, kind) => panic!("expected {shape:?} input value actual, got {kind:?}"),
    }
}

fn assert_written_input_case(db: &Db, top: NodeId, case: &SourceCase) {
    let source = outer_object(db, top, case.source);
    let sibling = outer_object(db, top, case.sibling);
    let result = outer_object(db, top, case.result);
    let source_identity = db.source_identity(source);
    let writer = assignment_at_line(db, source_line(case.writer));
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer keeps LHS and RHS");
    let writer_lhs = writer_children[0];
    assert_lvalue_shape(db, writer_lhs, source, case.writer_shape);
    assert!(refers_to_identity(db, writer_lhs, source_identity));

    let call_line = source_line(case.call);
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
    assert_eq!(calls.len(), 1, "one `{}` task call", case.task);
    let call = calls[0];
    let actuals = db.node(call).children();
    assert_eq!(actuals.len(), 1, "one selected input actual");
    let actual = actuals[0];
    // These input formals consume value expressions, not address/ref actuals.
    assert_input_actual_shape(db, actual, source, case.actual_shape);
    assert!(
        refers_to_identity(db, actual, source_identity),
        "input actual reads the focal outer source `{}`",
        case.source
    );

    let NodeKind::FuncCall {
        callee: Some(callee),
        ..
    } = db.node_kind(call)
    else {
        panic!("task call retains its resolved callee")
    };
    let formals = db
        .node(*callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), 1, "task has one input formal");
    let formal = formals[0];
    assert!(matches!(
        db.node_kind(formal),
        NodeKind::FuncArg {
            direction: Direction::Input,
            ..
        }
    ));

    let actual_source = references(db, actual)
        .into_iter()
        .find(|target| db.source_identity(*target) == source_identity)
        .expect("actual contains a reference to the focal source");
    let writer_source = references(db, writer_lhs)
        .into_iter()
        .find(|target| db.source_identity(*target) == source_identity)
        .expect("writer LHS contains a reference to the focal source");
    assert_eq!(
        db.source_identity(writer_source),
        db.source_identity(actual_source),
        "writer LHS and input actual refer to the same owned source identity"
    );
    assert_ne!(db.source_identity(formal), source_identity);
    assert_ne!(db.source_identity(result), source_identity);
    // Keep a separate sibling declaration available for an independent identity check.
    assert_ne!(
        db.source_identity(sibling),
        source_identity,
        "the independent sibling source has a distinct owned identity"
    );
    assert_ne!(
        db.source_identity(formal),
        db.source_identity(result),
        "input formal and result storage are separate from the focal source"
    );
}

#[test]
fn written_selected_sources_reach_task_inputs_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv\n"
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
        "field input or neighbor mismatch",
        "element input or neighbor mismatch",
        "row-slice input or neighbor mismatch",
        "concatenation input or neighbor mismatch",
        "positional-pattern input or neighbor mismatch",
    ] {
        assert!(FIXTURE_SOURCE.contains(oracle), "missing oracle `{oracle}`");
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_selected_input_actuals",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_selected_writes_and_input_actuals_share_source_identity() {
    let db = compile_fixture();
    let top = top_module(&db);
    for case in &CASES {
        assert_written_input_case(&db, top, case);
    }
}
