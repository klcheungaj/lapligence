//! SYN-038 written module sources passed directly to task input formals.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{
        AlwaysKind, Db, Direction, ExprKind, NetType, NodeId, NodeKind, Operation, ProcessKind,
        StmtKind,
    },
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv");
const EXPECTED_STDOUT: &str = "nba_call=5a/5a\ncontinuous_call=5a/5a\n";

#[derive(Clone, Copy)]
enum WriteForm {
    Nonblocking,
    ContinuousNet,
    ContinuousVariable,
}

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 written-source call fixture compiles");
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

fn outer_declaration(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Var { .. } | NodeKind::Net { .. }
                )
                && db.node(*id).parent() == Some(top)
        })
        .unwrap_or_else(|| panic!("outer module declaration `{name}` is captured"))
}

fn direct_ref_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref { target }) => *target,
        _ => None,
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

fn source_line(anchor: &str) -> u32 {
    source_lines(anchor)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("fixture is missing source anchor `{anchor}`"))
}

fn source_lines(anchor: &str) -> Vec<u32> {
    FIXTURE_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect()
}

fn assert_written_source_call(
    db: &Db,
    source_name: &str,
    source_kind: WriteForm,
    write_anchor: &str,
    rhs_name: &str,
    task_name: &str,
    call_anchor: &str,
) {
    let top = top_module(db);
    let source = outer_declaration(db, top, source_name);
    match source_kind {
        WriteForm::Nonblocking | WriteForm::ContinuousVariable => {
            assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
        }
        WriteForm::ContinuousNet => {
            assert!(matches!(
                db.node_kind(source),
                NodeKind::Net {
                    net_type: NetType::Wire,
                    ..
                }
            ));
        }
    }

    let write_line = source_line(write_anchor);
    let writer =
        db.node_ids()
            .find(|id| {
                if db.node(*id).line() != write_line {
                    return false;
                }
                let lhs = db.node(*id).children().first().copied();
                match source_kind {
                    WriteForm::Nonblocking => {
                        matches!(
                            db.node_kind(*id),
                            NodeKind::Stmt(StmtKind::Assign {
                                blocking: false,
                                op: Operation::Assignment,
                                delay: None,
                            })
                        ) && lhs
                            .and_then(|lhs| direct_ref_target(db, lhs))
                            .is_some_and(|target| {
                                db.source_identity(target) == db.source_identity(source)
                            })
                    }
                    WriteForm::ContinuousNet | WriteForm::ContinuousVariable => {
                        matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
                            && lhs.and_then(|lhs| direct_ref_target(db, lhs)).is_some_and(
                                |target| db.source_identity(target) == db.source_identity(source),
                            )
                    }
                }
            })
            .unwrap_or_else(|| panic!("source `{source_name}` has the expected explicit writer"));
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer retains LHS and RHS");
    let write_target = direct_ref_target(db, writer_children[0])
        .expect("whole-object writer LHS is a direct declaration reference");
    assert_eq!(
        db.source_identity(write_target),
        db.source_identity(source),
        "the explicit write targets the focal source declaration"
    );

    let rhs = writer_children[1];
    let rhs_target = direct_ref_target(db, rhs).expect("writer RHS is a source reference");
    assert_eq!(db.node(rhs_target).name(), rhs_name);
    if let WriteForm::Nonblocking = source_kind {
        let process = process_ancestor(db, writer);
        assert!(matches!(
            db.node_kind(process),
            NodeKind::Process {
                kind: ProcessKind::Always {
                    always_type: AlwaysKind::FlipFlop
                }
            }
        ));
    }

    let call_lines = source_lines(call_anchor);
    assert!(!call_lines.is_empty(), "call anchor exists in the fixture");
    let calls = db
        .node_ids()
        .filter(|id| {
            call_lines.contains(&db.node(*id).line())
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall {
                        name,
                        is_task: true,
                        ..
                    } if name == task_name
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        call_lines.len(),
        "each source phase has one matching task-input call"
    );
    let task = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == task_name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: true, .. })
        })
        .unwrap_or_else(|| panic!("task `{task_name}` is captured"));

    for call in calls {
        let NodeKind::FuncCall {
            callee: Some(callee),
            ..
        } = db.node_kind(call)
        else {
            panic!("task call has a resolved callee")
        };
        assert!(matches!(
            db.node_kind(*callee),
            NodeKind::FuncTask { is_task: true, .. }
        ));
        assert_eq!(
            db.source_identity(*callee),
            db.source_identity(task),
            "each call resolves to the expected task definition"
        );
        let formals = db
            .node(*callee)
            .children()
            .iter()
            .copied()
            .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
            .collect::<Vec<_>>();
        assert_eq!(formals.len(), 1, "task receives one explicit formal");
        let NodeKind::FuncArg { direction, .. } = db.node_kind(formals[0]) else {
            unreachable!("filtered to task formal")
        };
        assert_eq!(*direction, Direction::Input, "task formal is an input");

        let actual_targets = db
            .node(call)
            .children()
            .iter()
            .copied()
            .filter_map(|actual| direct_ref_target(db, actual))
            .collect::<Vec<_>>();
        assert_eq!(
            actual_targets.len(),
            1,
            "task call has one direct source actual"
        );
        let actual_target = actual_targets[0];
        assert_eq!(
            db.source_identity(actual_target),
            db.source_identity(source),
            "task input actual resolves to the exact written source declaration"
        );
        assert_eq!(
            db.source_identity(write_target),
            db.source_identity(actual_target),
            "writer LHS and call actual share one Slang source identity"
        );

        let call_process = process_ancestor(db, call);
        assert!(matches!(
            db.node_kind(call_process),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ));
    }
}

#[test]
fn written_module_sources_reach_task_inputs_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv\n"
    ));
    for anchor in [
        "nba_source <= nba_next;",
        "assign net_source = net_seed;",
        "assign variable_source = variable_seed;",
        "task automatic capture_nba(input logic [7:0] value);",
        "task automatic capture_net(input logic [7:0] value);",
        "task automatic capture_variable(input logic [7:0] value);",
        "capture_nba(nba_source);",
        "capture_net(net_source);",
        "capture_variable(variable_source);",
        "if (nba_source !== 8'h11 ||",
        "if (nba_source !== 8'h5a ||",
        "if (net_source !== 8'h11 ||",
        "if (net_source !== 8'h5a ||",
        "$display(\"nba_call=%02h/%02h\"",
        "$display(\"continuous_call=%02h/%02h\"",
        "$finish(0);",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing source/phase guard: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_sources_call_arguments",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_writers_and_task_actuals_share_each_focal_source_identity() {
    let db = compile_fixture();
    for (source, kind, write, rhs, task, call) in [
        (
            "nba_source",
            WriteForm::Nonblocking,
            "nba_source <= nba_next;",
            "nba_next",
            "capture_nba",
            "capture_nba(nba_source);",
        ),
        (
            "net_source",
            WriteForm::ContinuousNet,
            "assign net_source = net_seed;",
            "net_seed",
            "capture_net",
            "capture_net(net_source);",
        ),
        (
            "variable_source",
            WriteForm::ContinuousVariable,
            "assign variable_source = variable_seed;",
            "variable_seed",
            "capture_variable",
            "capture_variable(variable_source);",
        ),
    ] {
        assert_written_source_call(&db, source, kind, write, rhs, task, call);
    }
}
