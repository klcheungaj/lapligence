//! SYN-038 continuously driven net and variable sources connected to child inputs.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NetType, NodeId, NodeKind},
};
use std::path::Path;

const WIRE_FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv";
const WIRE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv");
const LOGIC_FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/port_driven_logic_child_input.sv";
const LOGIC_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/port_driven_logic_child_input.sv");

fn compile_fixture(fixture: &str) -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 child-input fixture compiles");
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

fn reference_target(db: &Db, expression: NodeId) -> NodeId {
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

fn assert_child_input_source(
    db: &Db,
    source_text: &str,
    source_name: &str,
    seed_name: &str,
    variable: bool,
) {
    let top = top_module(db);
    let source = outer_declaration(db, top, source_name);
    if variable {
        assert!(matches!(db.node_kind(source), NodeKind::Var { .. }));
    } else {
        assert!(matches!(
            db.node_kind(source),
            NodeKind::Net {
                net_type: NetType::Wire,
                ..
            }
        ));
    }

    let driver_line = source_line(source_text, "assign source = seed;");
    let drivers = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == driver_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ContAssign {
                        net_decl: false,
                        ..
                    }
                )
                && db
                    .node(*id)
                    .children()
                    .first()
                    .is_some_and(|lhs| reference_target(db, *lhs) == source)
        })
        .collect::<Vec<_>>();
    assert_eq!(drivers.len(), 1, "one whole-object continuous driver");
    let driver = drivers[0];
    assert!(
        !has_process_ancestor(db, driver),
        "continuous driver has no process"
    );
    let driver_children = db.node(driver).children();
    assert_eq!(driver_children.len(), 2, "driver retains LHS and RHS");
    let lhs_target = reference_target(db, driver_children[0]);
    let seed = outer_declaration(db, top, seed_name);
    let rhs_target = reference_target(db, driver_children[1]);
    assert_eq!(db.source_identity(rhs_target), db.source_identity(seed));

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
    let port = db
        .node(child)
        .children()
        .iter()
        .copied()
        .find(|id| {
            db.node(*id).name() == "value" && matches!(db.node_kind(*id), NodeKind::Port { .. })
        })
        .expect("child input formal is captured");
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
    let actual_declaration = high.expect("direct actual resolves to an outer declaration");
    let actual_expression = high_expr.expect("child input retains its actual expression");
    let actual_target = reference_target(db, actual_expression);

    let source_identity = db.source_identity(source);
    assert_eq!(db.source_identity(lhs_target), source_identity);
    assert_eq!(db.source_identity(actual_declaration), source_identity);
    assert_eq!(db.source_identity(actual_target), source_identity);
    assert_eq!(
        db.source_identity(lhs_target),
        db.source_identity(actual_target)
    );
}

#[test]
fn continuously_driven_wire_child_input_matches_both_cli_modes() {
    assert!(WIRE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv\n"
    ));
    for anchor in [
        "assign source = seed;",
        "leaf child(.value(source), .echoed());",
        "source !== 8'h00 || child.value !== 8'h00",
        "source !== 8'h5a || child.value !== 8'h5a",
    ] {
        assert!(
            WIRE_SOURCE.contains(anchor),
            "missing wire fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "port_driven_wire_child_input",
        "port_source=5a/5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_wire_driver_and_child_input_actual_share_outer_source_identity() {
    let db = compile_fixture(WIRE_FIXTURE);
    assert_child_input_source(&db, WIRE_SOURCE, "source", "seed", false);
}

#[test]
fn continuously_driven_logic_child_input_matches_both_cli_modes() {
    assert!(LOGIC_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/port_driven_logic_child_input.sv\n"
    ));
    for anchor in [
        "assign source = seed;",
        "leaf child(.value(source));",
        "source !== 8'h00 || child.value !== 8'h00",
        "source !== 8'h5a || child.value !== 8'h5a",
    ] {
        assert!(
            LOGIC_SOURCE.contains(anchor),
            "missing logic fixture anchor: {anchor}"
        );
    }
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "port_driven_logic_child_input",
        "port_logic=5a/5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_logic_driver_and_child_input_actual_share_outer_source_identity() {
    let db = compile_fixture(LOGIC_FIXTURE);
    assert_child_input_source(&db, LOGIC_SOURCE, "source", "seed", true);
}
