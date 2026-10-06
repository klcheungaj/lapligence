//! SYN-038 prior committed NBA values consumed through ref and inout task actuals.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{AlwaysKind, Db, Direction, ExprKind, NodeId, NodeKind, Operation, ProcessKind, StmtKind},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv");
const EXPECTED_STDOUT: &str = "nba-ref-inout=5a>a5,5a>a5 final=a5/a5\n";

#[derive(Clone, Copy)]
struct SourceCase {
    source: &'static str,
    previous_write: &'static str,
    call: &'static str,
    current_write: &'static str,
    task: &'static str,
    direction: Direction,
}

const CASES: [SourceCase; 2] = [
    SourceCase {
        source: "ref_source",
        previous_write: "ref_source <= 8'h5a;",
        call: "capture_ref(ref_source);",
        current_write: "ref_source <= 8'ha5;",
        task: "capture_ref",
        direction: Direction::Ref,
    },
    SourceCase {
        source: "inout_source",
        previous_write: "inout_source <= 8'h5a;",
        call: "capture_inout(inout_source);",
        current_write: "inout_source <= 8'ha5;",
        task: "capture_inout",
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
    .expect("legal SV2009 prior-NBA ref/inout fixture compiles");
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

fn outer_variable(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
                && db.node(*id).parent() == Some(top)
        })
        .unwrap_or_else(|| panic!("outer variable `{name}` is captured under tb"))
}

fn direct_ref_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref { target }) => *target,
        _ => None,
    }
}

fn assignment_at_line(db: &Db, line: u32) -> NodeId {
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: false,
                        op: Operation::Assignment,
                        delay: None,
                    })
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "line {line} has one nonblocking assignment"
    );
    matches[0]
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

fn assert_actual_source(db: &Db, actual: NodeId, source: NodeId, direction: Direction) {
    let source_identity = db.source_identity(source);
    match (direction, db.node_kind(actual)) {
        (Direction::Ref, NodeKind::Expr(ExprKind::Ref { target })) => {
            let target = target.expect("ref actual is a direct bound reference");
            assert_eq!(db.source_identity(target), source_identity);
        }
        (
            Direction::Inout,
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }),
        ) => {
            let source_refs = operands
                .iter()
                .filter_map(|operand| direct_ref_target(db, *operand))
                .filter(|target| db.source_identity(*target) == source_identity)
                .collect::<Vec<_>>();
            assert_eq!(
                source_refs.len(),
                1,
                "inout copy-in has one direct source ref"
            );
        }
        (direction, kind) => panic!("unexpected {direction:?} actual representation: {kind:?}"),
    }
}

fn assert_prior_nba_case(db: &Db, top: NodeId, case: &SourceCase) {
    let source = outer_variable(db, top, case.source);
    let source_identity = db.source_identity(source);
    let previous_line = source_line(case.previous_write);
    let call_line = source_line(case.call);
    let current_line = source_line(case.current_write);
    assert!(
        previous_line < call_line,
        "prior NBA source precedes its later consumer"
    );
    assert!(
        call_line < current_line,
        "current-edge NBA follows the task read"
    );

    let previous = assignment_at_line(db, previous_line);
    let current = assignment_at_line(db, current_line);
    for (label, assignment) in [("previous", previous), ("current", current)] {
        let children = db.node(assignment).children();
        assert_eq!(children.len(), 2, "{label} NBA retains LHS and RHS");
        let target = direct_ref_target(db, children[0])
            .unwrap_or_else(|| panic!("{label} NBA uses a whole-variable LHS"));
        assert_eq!(db.source_identity(target), source_identity);
    }

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
    assert_eq!(actuals.len(), 1, "task has one actual");
    assert_actual_source(db, actuals[0], source, case.direction);

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
    assert_eq!(formals.len(), 1, "task has one formal");
    assert!(matches!(
        db.node_kind(formals[0]),
        NodeKind::FuncArg { direction, .. } if *direction == case.direction
    ));

    let process = process_ancestor(db, call);
    assert_eq!(process, process_ancestor(db, previous));
    assert_eq!(process, process_ancestor(db, current));
    assert!(matches!(
        db.node_kind(process),
        NodeKind::Process {
            kind: ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop
            }
        }
    ));
}

#[test]
fn prior_nba_values_reach_whole_ref_and_inout_tasks_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv\n"
    ));
    for case in &CASES {
        assert!(FIXTURE_SOURCE.contains(case.previous_write));
        assert!(FIXTURE_SOURCE.contains(case.call));
        assert!(FIXTURE_SOURCE.contains(case.current_write));
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_nba_ref_inout_actuals",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_prior_nba_writes_and_ref_inout_actuals_share_source_identity() {
    let db = compile_fixture();
    let top = top_module(&db);
    for case in &CASES {
        assert_prior_nba_case(&db, top, case);
    }
}
