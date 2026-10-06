//! SYN-038 read-only `ref` actual for a continuously driven logic variable.

use crate::sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeId, NodeKind, ProcessKind, StmtKind},
};
use std::{path::Path, process::Command, time::Duration};

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv");
const EXPECTED_STDOUT: &str = "ref-read=5a\n";
const WARNING: &str = "cannot mix continuous and procedural assignments to variable 'source'";

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn compile_fixture() -> Db {
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![fixture_path().to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 read-only ref fixture compiles");
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

fn module_variable(db: &Db, top: NodeId, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && db.node(*id).parent() == Some(top)
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
        })
        .unwrap_or_else(|| panic!("module variable `{name}` is captured under tb"))
}

fn direct_reference_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct variable reference, got {other:?}"),
    }
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
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = db.node_kind(id)
        {
            targets.push(*target);
        }
        pending.extend(db.node(id).children().iter().copied());
    }
    targets
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

fn assert_task_body_only_reads_formal(db: &Db, body: NodeId, formal: NodeId, result: NodeId) {
    let formal_identity = db.source_identity(formal);
    let mut pending = vec![body];
    let mut visited = Vec::new();
    let mut assignments = Vec::new();
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        match db.node_kind(id) {
            NodeKind::Stmt(StmtKind::Assign { .. }) => assignments.push(id),
            NodeKind::FuncCall { .. } => panic!("read-only ref body has no nested calls"),
            _ => {}
        }
        pending.extend(db.node(id).children().iter().copied());
    }
    assert_eq!(assignments.len(), 1, "task body has one result assignment");
    let assignment_children = db.node(assignments[0]).children();
    assert_eq!(
        assignment_children.len(),
        2,
        "body assignment has LHS and RHS"
    );
    let lhs = direct_reference_target(db, assignment_children[0]);
    let rhs = direct_reference_target(db, assignment_children[1]);
    assert_eq!(db.source_identity(lhs), db.source_identity(result));
    assert_eq!(db.source_identity(rhs), formal_identity);
    assert!(
        assignments.iter().all(|assignment| {
            !references(db, db.node(*assignment).children()[0])
                .into_iter()
                .any(|target| db.source_identity(target) == formal_identity)
        }),
        "no task assignment targets the ref formal"
    );
}

fn assert_owned_driver_and_read_only_call(db: &Db) {
    let top = top_module(db);
    let source = module_variable(db, top, "source");
    let observed = module_variable(db, top, "observed");

    let driver_line = source_line("assign source = 8'h5a;");
    let drivers = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == driver_line
                && matches!(db.node_kind(*id), NodeKind::ContAssign { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(drivers.len(), 1, "continuous driver is captured");
    let driver_children = db.node(drivers[0]).children();
    assert_eq!(driver_children.len(), 2, "driver retains LHS and RHS");
    let writer_target = direct_reference_target(db, driver_children[0]);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(source)
    );

    let call_line = source_line("capture_ref(source);");
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
                    } if name == "capture_ref"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one ref task call is captured");
    let call = calls[0];
    let actuals = db.node(call).children();
    assert_eq!(actuals.len(), 1, "ref task receives one actual");
    let actual_target = direct_reference_target(db, actuals[0]);
    assert_eq!(
        db.source_identity(actual_target),
        db.source_identity(source)
    );
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(actual_target),
        "continuous driver LHS and ref actual share one owned source identity"
    );

    let NodeKind::FuncCall {
        callee: Some(callee),
        ..
    } = db.node_kind(call)
    else {
        panic!("ref task call retains its resolved callee")
    };
    let formals = db
        .node(*callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), 1, "task has one formal");
    let NodeKind::FuncArg {
        direction,
        const_ref,
        ref_static,
        ..
    } = db.node_kind(formals[0])
    else {
        unreachable!("filtered to formal arguments")
    };
    assert_eq!(*direction, Direction::Ref);
    assert!(!const_ref, "this witness is `ref`, not `const ref`");
    assert!(!ref_static);
    assert!(matches!(
        db.node_kind(process_ancestor(db, call)),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));
    assert_ne!(db.source_identity(formals[0]), db.source_identity(source));
    assert_ne!(db.source_identity(observed), db.source_identity(source));

    let NodeKind::FuncTask {
        body: Some(body), ..
    } = db.node_kind(*callee)
    else {
        panic!("ref task retains an owned body")
    };
    assert_task_body_only_reads_formal(db, *body, formals[0], observed);
}

fn assert_public_cli_output(optimized: bool) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let source = fixture_path();
    let working_directory =
        sim_harness::TempDir::new("ref_read_continuous_variable").expect("CLI test directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .current_dir(working_directory.path())
        .args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.args(["--edition", "2009"]).arg(&source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("optimized={optimized}: {error}"));
    assert!(
        output.status.success(),
        "optimized={optimized}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, EXPECTED_STDOUT.as_bytes());
    let expected_stderr = format!(
        "Warning: {}:{}:21 {WARNING}\n",
        sim_harness::source_display(&source),
        source_line("capture_ref(source);")
    );
    assert_eq!(
        output.stderr,
        expected_stderr.as_bytes(),
        "exact conservative ref warning, optimized={optimized}"
    );
}

#[test]
fn read_only_ref_of_continuously_driven_logic_runs_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv\n"
    ));
    assert!(FIXTURE_SOURCE.contains("assign source = 8'h5a;"));
    assert!(FIXTURE_SOURCE.contains("task automatic capture_ref(ref logic [7:0] value);"));
    assert!(FIXTURE_SOURCE.contains("observed = value;"));
    assert!(FIXTURE_SOURCE.contains("capture_ref(source);"));
    assert!(FIXTURE_SOURCE.contains("$display(\"ref-read=%02h\", observed);"));
    for optimized in [false, true] {
        assert_public_cli_output(optimized);
    }
}

#[test]
fn owned_continuous_driver_and_ref_actual_share_source_identity() {
    let db = compile_fixture();
    assert_owned_driver_and_read_only_call(&db);
}
