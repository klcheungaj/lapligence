//! SYN-038 interface-member actuals connected to nested child-interface ports.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, Direction, ExprKind, NodeKind},
};

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/interface_member_child_port.sv");

#[test]
fn mutable_interface_member_drives_nested_child_port_readback_in_both_modes() {
    for anchor in [
        "interface child_if(input logic [7:0] member);",
        "interface parent_if;",
        "logic [7:0] member;",
        "child_if nested(member);",
        "assign readback = member;",
        "bus.member = 8'h12;",
        "bus.member = 8'ha5;",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source anchor: {anchor}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "interface_member_child_port",
        "interface-child-port=a5,a5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn slang_binds_the_nested_child_port_to_the_parent_interface_member() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/interface_member_child_port.sv");
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal nested interface port connection compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("capture owned semantic database");

    let parent_interface = db
        .node_ids()
        .find(|id| {
            db.node(*id).name() == "bus"
                && matches!(
                    db.node_kind(*id),
                    NodeKind::ModuleInst {
                        def_name,
                        is_interface: true,
                        ..
                    } if def_name == "parent_if"
                )
        })
        .expect("parent interface instance is captured");
    let member = db
        .node(parent_interface)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name() == "member"
                && matches!(db.node_kind(*child), NodeKind::Var { .. })
        })
        .expect("mutable parent interface member is captured");
    let nested = db
        .node(parent_interface)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name() == "nested"
                && matches!(
                    db.node_kind(*child),
                    NodeKind::ModuleInst {
                        def_name,
                        is_interface: true,
                        ..
                    } if def_name == "child_if"
                )
        })
        .expect("nested child interface instance is captured");
    let input = db
        .node(nested)
        .children()
        .iter()
        .copied()
        .find(|child| {
            db.node(*child).name() == "member"
                && matches!(db.node_kind(*child), NodeKind::Port { .. })
        })
        .expect("nested child interface input port is captured");

    let NodeKind::Port {
        direction,
        high,
        high_expr,
        high_present,
        high_open,
        ..
    } = db.node_kind(input)
    else {
        unreachable!("filtered to the nested interface input port")
    };
    assert_eq!(*direction, Direction::Input);
    assert!(*high_present, "nested port has an actual connection");
    assert!(!*high_open, "nested port actual is not open");
    assert_eq!(
        *high,
        Some(member),
        "Slang resolves the child port actual to the mutable parent member"
    );
    let high_expr = high_expr.expect("nested port retains its actual expression");
    let NodeKind::Expr(ExprKind::Ref { target }) = db.node_kind(high_expr) else {
        panic!(
            "nested child port actual should remain a direct reference: {:?}",
            db.node_kind(high_expr)
        );
    };
    assert_eq!(
        *target,
        Some(member),
        "the owned actual expression binds to the same parent member"
    );
}
