//! Same-declaration event reads after selected lvalue writes.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, EventSpec, ExprKind, NodeId, NodeKind, Operation, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv";
const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv");
const EXPECTED_STDOUT: &str =
    "events=111111 field=31,80 unpacked=35,80 element=70,42 row=51,52/73 concat=a6c2 pattern=5c\n";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 event/lvalue fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn descendants(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        pending.extend(db.node(id).children().iter().copied());
    }
    visited
}

fn line_of(anchor: &str) -> u32 {
    let line = FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture lost source anchor {anchor:?}"))
        + 1;
    u32::try_from(line).expect("fixture line fits in u32")
}

fn declaration(db: &Db, name: &str) -> NodeId {
    let assignment = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Array { .. } | NodeKind::Var { .. } | NodeKind::Net { .. }
                )
        })
        .unwrap_or_else(|| panic!("Slang DB did not capture declaration `{name}`"));
    assignment
}

fn assignment_lhs(db: &Db, line: u32) -> NodeId {
    let assignment = db
        .node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign { blocking: true, .. })
                )
        })
        .unwrap_or_else(|| panic!("line {line} has the expected blocking assignment"));
    let children = db.node(assignment).children();
    assert_eq!(children.len(), 2, "line {line} has one LHS and one RHS");
    children[0]
}

fn event_operand(db: &Db, line: u32) -> NodeId {
    let control = db
        .node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::EventControl { .. })
                )
        })
        .unwrap_or_else(|| panic!("line {line} has an explicit event control"));
    let NodeKind::Stmt(StmtKind::EventControl { specs, .. }) = db.node_kind(control) else {
        unreachable!("event-control lookup returns an event-control statement")
    };
    let [EventSpec::AnyChange { sig }] = specs.as_slice() else {
        panic!("line {line} retains one any-change operand: {specs:?}");
    };
    *sig
}

fn has_reference_to(db: &Db, root: NodeId, declaration: NodeId) -> bool {
    let identity = db.source_identity(declaration);
    descendants(db, root).into_iter().any(|id| {
        matches!(
            db.node_kind(id),
            NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                if db.source_identity(*target) == identity
        )
    })
}

fn assert_same_decl_event_path(db: &Db, name: &str, write_line: u32, event_line: u32) -> NodeId {
    let declaration = declaration(db, name);
    let lhs = assignment_lhs(db, write_line);
    let event = event_operand(db, event_line);
    assert!(
        has_reference_to(db, lhs, declaration),
        "line {write_line} LHS binds to outer declaration `{name}`"
    );
    assert!(
        has_reference_to(db, event, declaration),
        "line {event_line} event operand reads the same outer declaration `{name}`"
    );
    lhs
}

fn assert_lvalue_forms_bind_to_event_slots(db: &Db) {
    let field_lhs = assert_same_decl_event_path(
        db,
        "field_slot",
        line_of("field_slot.value = 8'h31;"),
        line_of("@(field_slot.value);"),
    );
    assert!(matches!(
        db.node_kind(field_lhs),
        NodeKind::Expr(ExprKind::HierPath { parts, refs })
            if parts == &["field_slot", "value"]
                && refs.iter().flatten().any(|id| {
                    db.source_identity(*id) == db.source_identity(declaration(db, "field_slot"))
                })
    ));
    let unpacked_field_lhs = assert_same_decl_event_path(
        db,
        "unpacked_field_slot",
        line_of("unpacked_field_slot.value = 8'h35;"),
        line_of("@(unpacked_field_slot.value);"),
    );
    assert!(matches!(
        db.node_kind(unpacked_field_lhs),
        NodeKind::Expr(ExprKind::HierPath { parts, refs })
            if parts == &["unpacked_field_slot", "value"]
                && refs.iter().flatten().any(|id| {
                    db.source_identity(*id)
                        == db.source_identity(declaration(db, "unpacked_field_slot"))
                })
    ));
    let guard_control = declaration(db, "guard_control");
    let guard_write = assignment_lhs(db, line_of("guard_control.guard = 8'h33;"));
    let guard_event = event_operand(db, line_of("@(guard_control.value);"));
    assert!(has_reference_to(db, guard_write, guard_control));
    assert!(matches!(
        db.node_kind(guard_event),
        NodeKind::Expr(ExprKind::HierPath { parts, refs })
            if parts == &["guard_control", "value"]
                && refs.iter().flatten().any(|id| {
                    db.source_identity(*id) == db.source_identity(guard_control)
                })
    ));

    let element_lhs = assert_same_decl_event_path(
        db,
        "element_slot",
        line_of("element_slot[1] = 8'h42;"),
        line_of("@(element_slot[1]);"),
    );
    assert!(matches!(
        db.node_kind(element_lhs),
        NodeKind::Expr(ExprKind::ArraySelect { base, indices })
            if indices.len() == 1
                && db.source_identity(*base)
                    == db.source_identity(declaration(db, "element_slot"))
    ));

    let row_lhs = assert_same_decl_event_path(
        db,
        "row_slot",
        line_of("row_slot[0:1] = '{0:8'h51, 1:8'h52};"),
        line_of("@(row_slot[0]);"),
    );
    assert!(matches!(
        db.node_kind(row_lhs),
        NodeKind::Expr(ExprKind::PartSelect { base, .. })
            if has_reference_to(db, *base, declaration(db, "row_slot"))
    ));

    let concat_lhs = assert_same_decl_event_path(
        db,
        "concat_slot",
        line_of("{concat_slot[11:8], concat_slot[3:0]} = 8'h62;"),
        line_of("@(concat_slot);"),
    );
    assert!(matches!(
        db.node_kind(concat_lhs),
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::Concat,
            operands,
            ..
        }) if operands.len() == 2
            && operands.iter().all(|operand| {
                has_reference_to(db, *operand, declaration(db, "concat_slot"))
            })
    ));

    let pattern_lhs = assert_same_decl_event_path(
        db,
        "pattern_slot",
        line_of("bits2_t'{pattern_slot[3], pattern_slot[1]} = 2'b10;"),
        line_of("@(pattern_slot);"),
    );
    assert!(matches!(
        db.node_kind(pattern_lhs),
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::AssignmentPattern,
            operands,
            ..
        }) if operands.len() == 2
            && operands.iter().all(|operand| {
                has_reference_to(db, *operand, declaration(db, "pattern_slot"))
            })
    ));
}

#[test]
fn event_expression_lvalues_run_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv\n"
    ));
    for anchor in [
        "@(field_slot.value);",
        "@(guard_control.value);",
        "@(unpacked_field_slot.value);",
        "@(element_slot[1]);",
        "@(row_slot[0]);",
        "@(concat_slot);",
        "@(pattern_slot);",
        "field_slot.value = 8'h31;",
        "guard_control.guard = 8'h33;",
        "unpacked_field_slot.value = 8'h35;",
        "element_slot[1] = 8'h42;",
        "row_slot[0:1] = '{0:8'h51, 1:8'h52};",
        "{concat_slot[11:8], concat_slot[3:0]} = 8'h62;",
        "bits2_t'{pattern_slot[3], pattern_slot[1]} = 2'b10;",
        "guard_control.value !== 8'h11 || guard_control.guard !== 8'h33 ||",
        "unpacked_field_slot.value !== 8'h35 || unpacked_field_slot.guard !== 8'h80 ||",
        "event lvalue readback or neighboring guard mismatch",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost anchor {anchor:?}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "event_lvalue_matrix",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn event_operands_and_lvalue_forms_share_each_owned_declaration_identity() {
    let db = compile_fixture();
    assert_lvalue_forms_bind_to_event_slots(&db);
}
