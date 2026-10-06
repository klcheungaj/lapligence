//! Same-declaration reads and writes at assignment RHS use sites for SYN-038.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv");
const EXPECTED_STDOUT: &[u8] = b"same_root=11,21,69,01,31\n";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 same-root assignments compile");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn declaration(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Var { .. } | NodeKind::Net { .. }
                )
        })
        .unwrap_or_else(|| panic!("Slang DB did not capture declaration {name}"))
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

fn has_bound_reference(db: &Db, root: NodeId, declaration: NodeId) -> bool {
    descendants(db, root).into_iter().any(|id| {
        matches!(
            db.node_kind(id),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target)
            }) if db.source_identity(*target) == db.source_identity(declaration)
        )
    })
}

fn has_operation(db: &Db, root: NodeId, expected: Operation) -> bool {
    descendants(db, root).into_iter().any(|id| {
        matches!(
            db.node_kind(id),
            NodeKind::Expr(ExprKind::Operation { op, .. }) if *op == expected
        )
    })
}

fn assignment_at_line(db: &Db, line: u32, blocking: bool) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: assignment_blocking,
                        ..
                    }) if *assignment_blocking == blocking
                )
        })
        .unwrap_or_else(|| panic!("line {line} has the expected assignment kind"))
}

fn runs_in_initial(db: &Db, statement: NodeId) -> bool {
    let mut current = Some(statement);
    while let Some(id) = current {
        if matches!(
            db.node_kind(id),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ) {
            return true;
        }
        current = db.node(id).parent();
    }
    false
}

fn assert_same_root_assignment(
    db: &Db,
    line: u32,
    blocking: bool,
    root_name: &str,
    lhs_operation: Option<Operation>,
    rhs_operation: Option<Operation>,
) {
    let root = declaration(db, root_name);
    let statement = assignment_at_line(db, line, blocking);
    assert!(
        runs_in_initial(db, statement),
        "line {line} executes in initial"
    );

    let children = db.node(statement).children();
    assert_eq!(children.len(), 2, "line {line} has one LHS and one RHS");
    let lhs = children[0];
    let rhs = children[1];
    assert!(
        has_bound_reference(db, lhs, root),
        "line {line} LHS binds to outer declaration `{root_name}`"
    );
    assert!(
        has_bound_reference(db, rhs, root),
        "line {line} RHS reads outer declaration `{root_name}`"
    );
    if let Some(operation) = lhs_operation {
        assert!(
            has_operation(db, lhs, operation),
            "line {line} LHS retains {operation:?}"
        );
    }
    if let Some(operation) = rhs_operation {
        assert!(
            has_operation(db, rhs, operation),
            "line {line} RHS retains {operation:?}"
        );
    }
}

#[test]
fn same_root_assignment_rhs_witnesses_match_in_both_optimizer_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv\n"
    ));
    for anchor in [
        "whole = whole + 8'h01;",
        "record.value = record.value + 8'h01;",
        "{concat_value[3:0], concat_value[7:4]} = concat_value;",
        "bits_t'{pattern_value[1], pattern_value[0]} = {pattern_value[0], pattern_value[1]};",
        "nba_value <= nba_value + 8'h01;",
        "same-root initial readback",
        "same-root NBA pre-commit readback",
        "same-root NBA post-commit readback",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost anchor: {anchor}"
        );
    }

    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "co_same_root_assignment_rhs",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("co_same_root_assignment_rhs, optimized={optimized}");
        assert!(output.status.success(), "{label}");
        assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, "{label}");
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            "",
            "{label}"
        );
    }
}

#[test]
fn slang_binds_each_assignment_rhs_to_its_own_destination_declaration() {
    assert!(FIXTURE_SOURCE.contains("record.value = record.value + 8'h01;"));
    let db = compile_fixture();

    assert_same_root_assignment(&db, 22, true, "whole", None, Some(Operation::Add));
    assert_same_root_assignment(&db, 26, true, "record", None, Some(Operation::Add));
    assert_same_root_assignment(&db, 30, true, "concat_value", Some(Operation::Concat), None);
    assert_same_root_assignment(
        &db,
        34,
        true,
        "pattern_value",
        Some(Operation::AssignmentPattern),
        Some(Operation::Concat),
    );
    assert_same_root_assignment(&db, 38, false, "nba_value", None, Some(Operation::Add));
}
