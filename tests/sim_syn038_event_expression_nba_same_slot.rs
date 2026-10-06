//! SYN-038 same-slot event-expression and NBA write coverage.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{AlwaysKind, Db, EventSpec, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv");
const EXPECTED_STDOUT: &str =
    "before=11 old=00 event=00 seen=0\nafter=11 state=22 event=22 seen=1\n";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 event-expression/NBA fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn direct_ref_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref { target }) => *target,
        _ => None,
    }
}

fn declaration(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| db.node(*id).name() == name && matches!(db.node_kind(*id), NodeKind::Var { .. }))
        .unwrap_or_else(|| panic!("module variable `{name}` is captured"))
}

fn process_ancestor(db: &Db, node: NodeId) -> Option<NodeId> {
    let mut current = db.node(node).parent();
    while let Some(parent) = current {
        if matches!(db.node_kind(parent), NodeKind::Process { .. }) {
            return Some(parent);
        }
        current = db.node(parent).parent();
    }
    None
}

#[test]
fn event_expression_and_nba_write_observe_the_same_slot_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv\n"
    ));
    for anchor in [
        "focal <= next_value;",
        "pre_nba_sample <= focal;",
        "@(focal);",
        "event_sample = focal;",
        "if (focal !== 8'h11 ||",
        "if (pre_nba_sample !== 8'h11 ||",
        "$display(\"before=%h old=%h event=%h seen=%0d\"",
        "$display(\"after=%h state=%h event=%h seen=%0d\"",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing source guard: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "event_expression_nba_same_slot",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_db_binds_event_operand_and_whole_object_nba_to_one_slot() {
    let db = compile_fixture();
    let focal = declaration(&db, "focal");

    let event_control = db
        .node_ids()
        .find_map(|id| {
            let NodeKind::Stmt(StmtKind::EventControl {
                specs, implicit, ..
            }) = db.node_kind(id)
            else {
                return None;
            };
            if *implicit {
                return None;
            }
            specs.iter().find_map(|spec| match spec {
                EventSpec::AnyChange { sig } => Some((id, *sig)),
                _ => None,
            })
        })
        .expect("the initial block has an explicit any-change event on focal");
    let event_ref = event_control.1;
    let event_target = direct_ref_target(&db, event_ref)
        .expect("the event expression is a direct reference to the focal variable");
    assert_eq!(
        db.source_identity(event_target),
        db.source_identity(focal),
        "owned event operand resolves to the focal source identity"
    );
    let event_process = process_ancestor(&db, event_control.0)
        .expect("event control is nested in an initial process");
    assert!(matches!(
        db.node_kind(event_process),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));

    let nba = db
        .node_ids()
        .find(|id| {
            matches!(
                db.node_kind(*id),
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: false,
                    op: Operation::Assignment,
                    delay: None,
                })
            ) && db
                .node(*id)
                .children()
                .first()
                .is_some_and(|lhs| direct_ref_target(&db, *lhs).is_some())
                && db
                    .node(*id)
                    .children()
                    .first()
                    .and_then(|lhs| direct_ref_target(&db, *lhs))
                    .is_some_and(|target| db.source_identity(target) == db.source_identity(focal))
        })
        .expect("focal has a whole-object nonblocking assignment");
    let nba_lhs = db.node(nba).children()[0];
    let nba_target = direct_ref_target(&db, nba_lhs).expect("NBA LHS is a direct focal reference");
    assert_eq!(
        db.source_identity(nba_target),
        db.source_identity(event_target),
        "event operand and NBA LHS share one Slang source identity"
    );
    let writer_process = process_ancestor(&db, nba).expect("NBA assignment has a process owner");
    assert!(matches!(
        db.node_kind(writer_process),
        NodeKind::Process {
            kind: ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop
            }
        }
    ));
}
