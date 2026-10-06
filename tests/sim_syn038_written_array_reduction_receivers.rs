//! SYN-038 fixed-array reductions read receivers after selected blocking writes.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv");
const EXPECTED_STDOUT: &str = "written-reduction=0a,29,37,30,1e\n";

#[derive(Clone, Copy)]
enum WriteForm {
    WholeObject,
    Element,
    RowSlice,
    Concatenation,
    PositionalPattern,
}

impl WriteForm {
    fn ref_count(self) -> usize {
        match self {
            Self::WholeObject | Self::Element | Self::RowSlice => 1,
            Self::Concatenation => 2,
            Self::PositionalPattern => 4,
        }
    }
}

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 written-array reduction fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture is missing source anchor `{anchor}`")) as u32
        + 1
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
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

fn ref_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a direct declaration reference, got {other:?}"),
    }
}

fn declaration(db: &Db, name: &str, anchor: &str, parent: NodeId) -> NodeId {
    let line = source_line(anchor);
    let found = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && db.node(*id).line() == line
                && db.node(*id).parent() == Some(parent)
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "one module variable `{name}` at line {line}"
    );
    found[0]
}

fn assignment_at(db: &Db, anchor: &str) -> NodeId {
    let line = source_line(anchor);
    let found = db
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
    assert_eq!(found.len(), 1, "one blocking assignment at line {line}");
    found[0]
}

fn source_refs(db: &Db, root: NodeId, source_identity: NodeId) -> Vec<NodeId> {
    let mut pending = vec![root];
    let mut matches = Vec::new();
    while let Some(node) = pending.pop() {
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = db.node_kind(node)
        {
            if db.source_identity(*target) == source_identity {
                matches.push(*target);
            }
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    matches
}

fn reduction_receiver(db: &Db, anchor: &str, source: NodeId) -> (NodeId, NodeId) {
    let line = source_line(anchor);
    let reduction = db
        .node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::MethodCall {
                        name,
                        receiver: Some(_),
                        ..
                    } if name == "sum"
                )
        })
        .unwrap_or_else(|| panic!("reduction receiver for `{anchor}` is captured"));
    let NodeKind::MethodCall {
        receiver: Some(receiver),
        ..
    } = db.node_kind(reduction)
    else {
        unreachable!("filtered to a reduction method call")
    };
    let target = ref_target(db, *receiver);
    assert_eq!(
        db.source_identity(target),
        db.source_identity(source),
        "reduction receiver binds to the outer fixed-array declaration"
    );
    (reduction, target)
}

#[test]
fn written_fixed_array_reductions_match_both_optimizer_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv\n"
    ));
    for anchor in [
        "lanes = '{8'h01, 8'h02, 8'h03, 8'h04};",
        "whole_sum = lanes.sum();",
        "lanes[1] = 8'h21;",
        "element_sum = lanes.sum();",
        "lanes[0:1] = '{0:8'h10, 1:8'h20};",
        "row_slice_sum = lanes.sum();",
        "{lanes[0], lanes[2]} = {8'h05, 8'h07};",
        "concatenation_sum = lanes.sum();",
        "byte_array_t'{lanes[0], lanes[1], lanes[2], lanes[3]} = '{8'h03, 8'h06, 8'h09, 8'h0c};",
        "positional_pattern_sum = lanes.sum();",
        "$finish(0);",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing write/use anchor: {anchor}"
        );
    }
    for vector in [
        "LV=whole_object, CP=fixed_array_reduction, WK=procedural_blocking",
        "LV=element, CP=fixed_array_reduction, WK=procedural_blocking",
        "LV=row_slice, CP=fixed_array_reduction, WK=procedural_blocking",
        "LV=concatenation, CP=fixed_array_reduction, WK=procedural_blocking",
        "LV=positional_pattern, CP=fixed_array_reduction, WK=procedural_blocking",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(vector),
            "missing focal vector: {vector}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_array_reduction_receivers",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_reduction_receivers_share_identity_with_each_selected_writer() {
    let db = compile_fixture();
    let top = top_module(&db);
    let lanes = declaration(&db, "lanes", "byte_array_t lanes;", top);
    let initial = db
        .node_ids()
        .find(|id| {
            db.node(*id).line() == source_line("initial begin : check")
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Process {
                        kind: ProcessKind::Initial
                    }
                )
        })
        .expect("initial process is captured");

    for (write, reduction, form) in [
        (
            "lanes = '{8'h01, 8'h02, 8'h03, 8'h04};",
            "whole_sum = lanes.sum();",
            WriteForm::WholeObject,
        ),
        (
            "lanes[1] = 8'h21;",
            "element_sum = lanes.sum();",
            WriteForm::Element,
        ),
        (
            "lanes[0:1] = '{0:8'h10, 1:8'h20};",
            "row_slice_sum = lanes.sum();",
            WriteForm::RowSlice,
        ),
        (
            "{lanes[0], lanes[2]} = {8'h05, 8'h07};",
            "concatenation_sum = lanes.sum();",
            WriteForm::Concatenation,
        ),
        (
            "byte_array_t'{lanes[0], lanes[1], lanes[2], lanes[3]} = '{8'h03, 8'h06, 8'h09, 8'h0c};",
            "positional_pattern_sum = lanes.sum();",
            WriteForm::PositionalPattern,
        ),
    ] {
        let writer = assignment_at(&db, write);
        let writer_children = db.node(writer).children();
        assert_eq!(writer_children.len(), 2, "writer has an LHS and RHS");
        let lvalue_targets = source_refs(&db, writer_children[0], db.source_identity(lanes));
        assert_eq!(
            lvalue_targets.len(),
            form.ref_count(),
            "selected LHS form retains all references to its focal array"
        );
        assert!(lvalue_targets.iter().all(|target| {
            db.source_identity(*target) == db.source_identity(lanes)
        }));
        let (method, receiver) = reduction_receiver(&db, reduction, lanes);
        assert_eq!(
            db.source_identity(lvalue_targets[0]),
            db.source_identity(receiver),
            "write LHS and later reduction receiver share one outer Slang source identity"
        );
        assert_eq!(process_ancestor(&db, writer), initial);
        assert_eq!(process_ancestor(&db, method), initial);
        assert!(db.node(writer).line() < db.node(method).line());
    }
}
