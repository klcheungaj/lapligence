//! SYN-038 blocking writes consumed through whole-variable ref and inout task actuals.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv");
const EXPECTED_STDOUT: &str =
    "prior-ref=10>ef,21>de,32>cd,54>ab,a6>59,5c>a3\nprior-inout=11>ee,22>dd,42>bd,64>9b,b6>49,6c>93\n";

#[derive(Clone, Copy, Debug)]
enum LvalueShape {
    Whole,
    Field,
    Element,
    RowSlice,
    Concatenation,
    PositionalPattern,
}

#[derive(Clone, Copy)]
struct SourceCase {
    source: &'static str,
    writer: &'static str,
    task: &'static str,
    call: &'static str,
    shape: LvalueShape,
    direction: Direction,
}

const CASES: [SourceCase; 12] = [
    SourceCase {
        source: "whole_ref_source",
        writer: "whole_ref_source = 8'h10;",
        task: "mutate_scalar_ref",
        call: "mutate_scalar_ref(whole_ref_source);",
        shape: LvalueShape::Whole,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "field_ref_source",
        writer: "field_ref_source.selected = 8'h21;",
        task: "mutate_field_ref",
        call: "mutate_field_ref(field_ref_source);",
        shape: LvalueShape::Field,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "element_ref_source",
        writer: "element_ref_source[1] = 8'h32;",
        task: "mutate_array_ref",
        call: "mutate_array_ref(element_ref_source, 1, seen_element_ref);",
        shape: LvalueShape::Element,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "row_ref_source",
        writer: "row_ref_source[0:1] = '{0: 8'h43, 1: 8'h54};",
        task: "mutate_array_ref",
        call: "mutate_array_ref(row_ref_source, 1, seen_row_ref);",
        shape: LvalueShape::RowSlice,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "concat_ref_source",
        writer: "{concat_ref_source[1], concat_ref_source[0]} = {8'hA6, 8'hC2};",
        task: "mutate_array_ref",
        call: "mutate_array_ref(concat_ref_source, 1, seen_concat_ref);",
        shape: LvalueShape::Concatenation,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "pattern_ref_source",
        writer: "byte_pair_t'{pattern_ref_source[0], pattern_ref_source[1]} = '{0: 8'h5c, 1: 8'h7d};",
        task: "mutate_array_ref",
        call: "mutate_array_ref(pattern_ref_source, 0, seen_pattern_ref);",
        shape: LvalueShape::PositionalPattern,
        direction: Direction::Ref,
    },
    SourceCase {
        source: "whole_inout_source",
        writer: "whole_inout_source = 8'h11;",
        task: "mutate_scalar_inout",
        call: "mutate_scalar_inout(whole_inout_source);",
        shape: LvalueShape::Whole,
        direction: Direction::Inout,
    },
    SourceCase {
        source: "field_inout_source",
        writer: "field_inout_source.selected = 8'h22;",
        task: "mutate_field_inout",
        call: "mutate_field_inout(field_inout_source);",
        shape: LvalueShape::Field,
        direction: Direction::Inout,
    },
    SourceCase {
        source: "element_inout_source",
        writer: "element_inout_source[1] = 8'h42;",
        task: "mutate_array_inout",
        call: "mutate_array_inout(element_inout_source, 1, seen_element_inout);",
        shape: LvalueShape::Element,
        direction: Direction::Inout,
    },
    SourceCase {
        source: "row_inout_source",
        writer: "row_inout_source[0:1] = '{0: 8'h53, 1: 8'h64};",
        task: "mutate_array_inout",
        call: "mutate_array_inout(row_inout_source, 1, seen_row_inout);",
        shape: LvalueShape::RowSlice,
        direction: Direction::Inout,
    },
    SourceCase {
        source: "concat_inout_source",
        writer: "{concat_inout_source[1], concat_inout_source[0]} = {8'hB6, 8'hD2};",
        task: "mutate_array_inout",
        call: "mutate_array_inout(concat_inout_source, 1, seen_concat_inout);",
        shape: LvalueShape::Concatenation,
        direction: Direction::Inout,
    },
    SourceCase {
        source: "pattern_inout_source",
        writer: "byte_pair_t'{pattern_inout_source[0], pattern_inout_source[1]} = '{0: 8'h6c, 1: 8'h8d};",
        task: "mutate_array_inout",
        call: "mutate_array_inout(pattern_inout_source, 0, seen_pattern_inout);",
        shape: LvalueShape::PositionalPattern,
        direction: Direction::Inout,
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
    .expect("legal SV2009 prior-write ref/inout fixture compiles");
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
        (LvalueShape::Whole, NodeKind::Expr(ExprKind::Ref { target })) => {
            let target = target.expect("whole-variable writer binds its target");
            assert_eq!(db.source_identity(target), source_identity);
        }
        (LvalueShape::Field, NodeKind::Expr(ExprKind::HierPath { parts, refs })) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[1], "selected");
            assert!(refs
                .iter()
                .flatten()
                .any(|target| db.source_identity(*target) == source_identity));
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

fn assert_written_ref_inout_case(db: &Db, top: NodeId, case: &SourceCase) {
    let source = outer_object(db, top, case.source);
    let source_identity = db.source_identity(source);
    let writer_line = source_line(case.writer);
    let writer = assignment_at_line(db, writer_line);
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer retains LHS and RHS");
    let writer_lhs = writer_children[0];
    assert_lvalue_shape(db, writer_lhs, source, case.shape);
    assert!(refers_to_identity(db, writer_lhs, source_identity));

    let call_line = source_line(case.call);
    assert!(writer_line < call_line, "blocking write precedes task call");
    let calls = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == call_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall { name, is_task: true, .. }
                        if name == case.task
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one `{}` task call", case.task);
    let call = calls[0];
    let actuals = db.node(call).children();
    let expected_actual_count = if matches!(
        case.shape,
        LvalueShape::Element
            | LvalueShape::RowSlice
            | LvalueShape::Concatenation
            | LvalueShape::PositionalPattern
    ) {
        3
    } else {
        1
    };
    assert_eq!(actuals.len(), expected_actual_count);
    let actual = actuals[0];
    let actual_references = match db.node_kind(actual) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => vec![*target],
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::Assignment,
            operands,
            ..
        }) => {
            let direct_source_refs = operands
                .iter()
                .filter_map(|operand| match db.node_kind(*operand) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) if db.source_identity(*target) == source_identity => Some(*target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                direct_source_refs.len(),
                1,
                "inout copy-in wrapper for `{}` keeps one direct source reference",
                case.source
            );
            direct_source_refs
        }
        other => panic!(
            "call actual for `{}` should directly identify the whole variable, got {other:?}",
            case.source
        ),
    };
    assert_eq!(
        actual_references.len(),
        1,
        "call actual for `{}` contains one direct source reference",
        case.source
    );
    assert_eq!(
        db.source_identity(actual_references[0]),
        source_identity,
        "later direct actual names the writer's whole outer variable"
    );

    let NodeKind::FuncCall {
        callee: Some(callee),
        ..
    } = db.node_kind(call)
    else {
        panic!("task call retains its resolved callee")
    };
    let NodeKind::FuncTask { is_task: true, .. } = db.node_kind(*callee) else {
        panic!("call resolves to a task declaration")
    };
    let formals = db
        .node(*callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), expected_actual_count);
    assert!(matches!(
        db.node_kind(formals[0]),
        NodeKind::FuncArg { direction, .. } if *direction == case.direction
    ));
    if expected_actual_count == 3 {
        assert!(matches!(
            db.node_kind(formals[1]),
            NodeKind::FuncArg {
                direction: Direction::Input,
                ..
            }
        ));
        assert!(matches!(
            db.node_kind(formals[2]),
            NodeKind::FuncArg {
                direction: Direction::Output,
                ..
            }
        ));
    }

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
fn blocking_written_whole_variables_reach_ref_and_inout_tasks_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv\n"
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
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_ref_inout_actuals",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_blocking_writes_and_whole_ref_inout_actuals_share_source_identity() {
    let db = compile_fixture();
    let top = top_module(&db);
    for case in &CASES {
        assert_written_ref_inout_case(&db, top, case);
    }
}
