//! SYN-038 written sources consumed by static child inputs and function inputs.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{
        AlwaysKind, Db, Direction, ExprKind, NetType, NodeId, NodeKind, Operation, ProcessKind,
        StmtKind,
    },
};
use std::path::Path;

const BLOCKING_PORT_FIXTURE: &str =
    "tests/fixtures/sim/syn038_pairwise/blocking_written_child_input.sv";
const BLOCKING_PORT_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/blocking_written_child_input.sv");
const NBA_PORT_FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/nba_written_child_input.sv";
const NBA_PORT_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/nba_written_child_input.sv");
const FUNCTION_FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv";
const FUNCTION_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv");

fn compile_fixture(fixture: &str) -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 written-consumer fixture compiles");
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
                    NodeKind::Net { .. } | NodeKind::Var { .. }
                )
                && db.node(*id).parent() == Some(top)
        })
        .unwrap_or_else(|| panic!("outer declaration `{name}` is captured under tb"))
}

fn direct_reference_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct reference, got {other:?}"),
    }
}

fn source_line(source: &str, anchor: &str) -> u32 {
    source
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture source is missing `{anchor}`")) as u32
        + 1
}

fn source_line_after(source: &str, after_line: u32, anchor: &str) -> u32 {
    source
        .lines()
        .enumerate()
        .skip(after_line as usize)
        .find_map(|(index, line)| (line.trim() == anchor).then_some(index as u32 + 1))
        .unwrap_or_else(|| panic!("fixture source is missing `{anchor}` after line {after_line}"))
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

fn has_process_ancestor(db: &Db, mut node: NodeId) -> bool {
    loop {
        if matches!(db.node_kind(node), NodeKind::Process { .. }) {
            return true;
        }
        let Some(parent) = db.node(node).parent() else {
            return false;
        };
        node = parent;
    }
}

fn child_input_port(db: &Db, top: NodeId) -> NodeId {
    let child = db
        .node(top)
        .children()
        .iter()
        .copied()
        .find(|id| {
            db.node(*id).name() == "child"
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ModuleInst {
                        def_name,
                        is_interface: false,
                        ..
                    } if def_name == "leaf"
                )
        })
        .expect("child module instance is routed from tb");
    assert_eq!(db.node(child).parent(), Some(top));
    db.node(child)
        .children()
        .iter()
        .copied()
        .find(|id| {
            db.node(*id).name() == "value" && matches!(db.node_kind(*id), NodeKind::Port { .. })
        })
        .expect("child input formal is captured")
}

fn assert_child_input_binding(db: &Db, source_text: &str, nonblocking: bool) {
    let top = top_module(db);
    let source = outer_declaration(db, top, "source");
    assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
    let (anchor, blocking) = if nonblocking {
        ("source <= 8'h5a;", false)
    } else {
        ("source = 8'h5a;", true)
    };
    let writer_line = source_line(source_text, anchor);
    let writers = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == writer_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign {
                        blocking: is_blocking,
                        op: Operation::Assignment,
                        delay: None,
                    }) if *is_blocking == blocking
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(writers.len(), 1, "one whole-object source write");
    let writer = writers[0];
    let children = db.node(writer).children();
    assert_eq!(children.len(), 2, "write retains LHS and RHS");
    let writer_target = direct_reference_target(db, children[0]);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(source)
    );
    let process = process_ancestor(db, writer);
    if nonblocking {
        assert!(matches!(
            db.node_kind(process),
            NodeKind::Process {
                kind: ProcessKind::Always {
                    always_type: AlwaysKind::FlipFlop
                }
            }
        ));
    } else {
        assert!(matches!(
            db.node_kind(process),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ));
    }

    let port = child_input_port(db, top);
    let NodeKind::Port {
        direction,
        high,
        high_expr,
        high_present,
        high_open,
        ..
    } = db.node_kind(port)
    else {
        unreachable!("filtered to a child port")
    };
    assert_eq!(*direction, Direction::Input);
    assert!(*high_present, "child input has an actual connection");
    assert!(!*high_open, "child input actual is not open");
    let actual_declaration = high.expect("actual resolves to the outer declaration");
    let actual_expression = high_expr.expect("port retains its actual expression");
    let actual_target = direct_reference_target(db, actual_expression);
    let source_identity = db.source_identity(source);
    assert_eq!(db.source_identity(actual_declaration), source_identity);
    assert_eq!(db.source_identity(actual_target), source_identity);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(actual_target)
    );
    assert!(
        !has_process_ancestor(db, port),
        "static child port consumption has no process context"
    );
}

fn continuous_driver(db: &Db, source_text: &str, source_name: &str) -> NodeId {
    let line = source_line(
        source_text,
        match source_name {
            "net_source" => "assign net_source = seed;",
            "variable_source" => "assign variable_source = seed;",
            _ => panic!("unsupported continuous source {source_name}"),
        },
    );
    let matches = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ContAssign {
                        net_decl: false,
                        ..
                    }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one continuous source driver");
    matches[0]
}

fn assert_function_input_call(
    db: &Db,
    source_text: &str,
    source_name: &str,
    after_anchor: &str,
    call_anchor: &str,
    writer: NodeId,
    expected_net: bool,
) {
    let top = top_module(db);
    let source = outer_declaration(db, top, source_name);
    if expected_net {
        assert!(matches!(
            db.node_kind(source),
            NodeKind::Net {
                net_type: NetType::Wire,
                ..
            }
        ));
    } else {
        assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
    }

    let writer_lhs = db
        .node(writer)
        .children()
        .first()
        .copied()
        .expect("source writer retains its LHS");
    let writer_target = direct_reference_target(db, writer_lhs);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(source)
    );
    let writer_line = db.node(writer).line();

    let after_line = source_line(source_text, after_anchor);
    let call_line = source_line_after(source_text, after_line, call_anchor);
    assert!(
        writer_line < call_line,
        "source write precedes function input"
    );
    let calls = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == call_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall {
                        name,
                        is_task: false,
                        ..
                    } if name == "echo"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one function input call for this source");
    let call = calls[0];
    let NodeKind::FuncCall { callee, .. } = db.node_kind(call) else {
        unreachable!("filtered to a function call")
    };
    let callee = callee.expect("resolved function retains its callee");
    let NodeKind::FuncTask { is_task, .. } = db.node_kind(callee) else {
        panic!("function call resolves to a subroutine declaration")
    };
    assert!(!*is_task, "callee is a function");
    assert_eq!(db.node(callee).name(), "echo");
    assert_eq!(
        db.node(callee).parent(),
        Some(top),
        "function route is module local"
    );
    let formals = db
        .node(callee)
        .children()
        .iter()
        .copied()
        .filter(|id| matches!(db.node_kind(*id), NodeKind::FuncArg { .. }))
        .collect::<Vec<_>>();
    assert_eq!(formals.len(), 1, "function has one input formal");
    let NodeKind::FuncArg { direction, .. } = db.node_kind(formals[0]) else {
        unreachable!("filtered to a function argument")
    };
    assert_eq!(*direction, Direction::Input);
    assert_eq!(db.node(formals[0]).name(), "value");

    let actuals = db
        .node(call)
        .children()
        .iter()
        .copied()
        .filter_map(|actual| match db.node_kind(actual) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actuals.len(), 1, "call retains one direct source actual");
    let actual_target = actuals[0];
    assert_eq!(
        db.source_identity(actual_target),
        db.source_identity(source)
    );
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(actual_target)
    );
    assert!(matches!(
        db.node_kind(process_ancestor(db, call)),
        NodeKind::Process {
            kind: ProcessKind::Initial
        }
    ));
}

#[test]
fn blocking_written_child_input_matches_both_cli_modes() {
    assert!(BLOCKING_PORT_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/blocking_written_child_input.sv\n"
    ));
    for anchor in [
        "source = 8'h5a;",
        "leaf child(.value(source));",
        "child.echoed !== 8'h11",
        "child.echoed !== 8'h5a",
    ] {
        assert!(
            BLOCKING_PORT_SOURCE.contains(anchor),
            "missing blocking-port fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "blocking_written_child_input",
        "blocking_port=5a/5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_blocking_write_binds_to_static_child_input_actual() {
    let db = compile_fixture(BLOCKING_PORT_FIXTURE);
    assert_child_input_binding(&db, BLOCKING_PORT_SOURCE, false);
}

#[test]
fn nba_written_child_input_matches_both_cli_modes() {
    assert!(NBA_PORT_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/nba_written_child_input.sv\n"
    ));
    for anchor in [
        "source <= 8'h5a;",
        "leaf child(.value(source));",
        "child.echoed !== 8'h11",
        "child.echoed !== 8'h5a",
    ] {
        assert!(
            NBA_PORT_SOURCE.contains(anchor),
            "missing NBA-port fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "nba_written_child_input",
        "nba_port=5a/5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_nba_write_binds_to_static_child_input_actual() {
    let db = compile_fixture(NBA_PORT_FIXTURE);
    assert_child_input_binding(&db, NBA_PORT_SOURCE, true);
}

#[test]
fn driven_sources_passed_to_function_inputs_match_both_cli_modes() {
    assert!(FUNCTION_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv\n"
    ));
    for anchor in [
        "assign net_source = seed;",
        "assign variable_source = seed;",
        "nba_source <= 8'h5a;",
        "nba_observed = echo(nba_source);",
        "net_observed = echo(net_source);",
        "variable_observed = echo(variable_source);",
        "function-input baseline mismatch",
        "function-input update mismatch",
    ] {
        assert!(
            FUNCTION_SOURCE.contains(anchor),
            "missing function-input fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "function_inputs_from_drivers",
        "function_inputs=5a/5a/5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_function_input_actuals_share_each_written_source_identity() {
    let db = compile_fixture(FUNCTION_FIXTURE);
    let net_driver = continuous_driver(&db, FUNCTION_SOURCE, "net_source");
    let variable_driver = continuous_driver(&db, FUNCTION_SOURCE, "variable_source");
    let nba_line = source_line(FUNCTION_SOURCE, "nba_source <= 8'h5a;");
    let nba_writers = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == nba_line
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
    assert_eq!(nba_writers.len(), 1, "one whole-object NBA source write");
    let nba_writer = nba_writers[0];
    assert!(matches!(
        db.node_kind(process_ancestor(&db, nba_writer)),
        NodeKind::Process {
            kind: ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop
            }
        }
    ));

    for (source_name, after_anchor, call_anchor, writer, is_net) in [
        (
            "net_source",
            "seed = 8'h5a;",
            "net_observed = echo(net_source);",
            net_driver,
            true,
        ),
        (
            "variable_source",
            "seed = 8'h5a;",
            "variable_observed = echo(variable_source);",
            variable_driver,
            false,
        ),
        (
            "nba_source",
            "clk = 1'b1;",
            "nba_observed = echo(nba_source);",
            nba_writer,
            false,
        ),
    ] {
        assert_function_input_call(
            &db,
            FUNCTION_SOURCE,
            source_name,
            after_anchor,
            call_anchor,
            writer,
            is_net,
        );
    }
}
