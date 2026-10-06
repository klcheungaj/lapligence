//! SYN-038 fixed-array reductions observe element writes on the same receiver.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{
        AlwaysKind, ArrayKind, Db, ExprKind, NetType, NodeId, NodeKind, Operation, ProcessKind,
        StmtKind,
    },
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv");
const EXPECTED_STDOUT: &str = "net-reduction=6\nvar-reduction=6\nnba-reduction=5\n";

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("the SV2009 written-reduction fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    FIXTURE_SOURCE
        .lines()
        .position(|line| line.trim() == anchor)
        .unwrap_or_else(|| panic!("fixture is missing source anchor `{anchor}`")) as u32
        + 1
}

fn source_lines(anchor: &str) -> Vec<u32> {
    FIXTURE_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect()
}

fn top_module(db: &Db) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == "tb"
                && matches!(db.node_kind(*id), NodeKind::ModuleInst { is_top: true, .. })
        })
        .expect("top module instance is captured")
}

fn outer_array(db: &Db, top: NodeId, name: &str, anchor: &str) -> NodeId {
    let line = source_line(anchor);
    let found = db
        .node_ids()
        .filter(|id| {
            db.node(*id).name() == name
                && db.node(*id).line() == line
                && db.node(*id).parent() == Some(top)
                && matches!(db.node_kind(*id), NodeKind::Array { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "one outer fixed array `{name}` at line {line}"
    );
    found[0]
}

fn direct_ref_target(db: &Db, expression: NodeId) -> Option<NodeId> {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref { target }) => *target,
        _ => None,
    }
}

fn required_direct_ref_target(db: &Db, expression: NodeId) -> NodeId {
    direct_ref_target(db, expression).unwrap_or_else(|| {
        panic!(
            "expected a direct bound reference, got {:?}",
            db.node_kind(expression)
        )
    })
}

fn bound_array_source(db: &Db, expression: NodeId) -> NodeId {
    direct_ref_target(db, expression).unwrap_or_else(|| match db.node_kind(expression) {
        NodeKind::Array { .. } => expression,
        other => panic!("expected a bound array source, got {other:?}"),
    })
}

fn selected_root(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
            assert_eq!(indices.len(), 1, "focal write selects one array element");
            bound_array_source(db, *base)
        }
        other => panic!("expected an array-element lvalue, got {other:?}"),
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

fn node_contains(db: &Db, root: NodeId, needle: NodeId) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node == needle {
            return true;
        }
        pending.extend(db.node(node).children().iter().copied());
    }
    false
}

fn assert_static_array(db: &Db, array: NodeId, expected_net_type: Option<NetType>) {
    let metadata = db
        .array_meta(array)
        .expect("outer fixed array has captured array metadata");
    assert_eq!(metadata.kind, ArrayKind::Static);
    assert_eq!(metadata.net_type, expected_net_type);
}

fn assert_continuous_element_writer(
    db: &Db,
    array: NodeId,
    write_anchor: &str,
    seed_name: Option<&str>,
) {
    let line = source_line(write_anchor);
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
    assert_eq!(
        matches.len(),
        1,
        "one static continuous element writer at line {line}"
    );
    let writer = matches[0];
    let children = db.node(writer).children();
    assert_eq!(children.len(), 2, "continuous writer keeps LHS and RHS");
    let source = selected_root(db, children[0]);
    assert_eq!(
        db.source_identity(source),
        db.source_identity(array),
        "continuous element LHS binds to the focal outer array"
    );
    if let Some(seed_name) = seed_name {
        let seed = required_direct_ref_target(db, children[1]);
        assert_eq!(db.node(seed).name(), seed_name);
        assert_ne!(
            db.source_identity(seed),
            db.source_identity(array),
            "the source seed is distinct from the focal array"
        );
    }
}

fn assert_nba_element_writer(db: &Db, array: NodeId, write_anchor: &str) {
    let line = source_line(write_anchor);
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
    assert_eq!(matches.len(), 1, "one NBA element writer at line {line}");
    let writer = matches[0];
    let lhs = *db
        .node(writer)
        .children()
        .first()
        .expect("NBA assignment keeps its selected LHS");
    let source = selected_root(db, lhs);
    assert_eq!(
        db.source_identity(source),
        db.source_identity(array),
        "NBA element LHS binds to the focal outer array"
    );
    assert!(matches!(
        db.node_kind(process_ancestor(db, writer)),
        NodeKind::Process {
            kind: ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop
            }
        }
    ));
}

fn assert_reductions_share_writer_source(
    db: &Db,
    array: NodeId,
    reduction_anchor: &str,
    sum_name: &str,
    expected_count: usize,
) {
    let lines = source_lines(reduction_anchor);
    assert_eq!(
        lines.len(),
        expected_count,
        "fixture has expected reduction anchors"
    );
    let reductions = db
        .node_ids()
        .filter(|id| {
            lines.contains(&db.node(*id).line())
                && matches!(
                    db.node_kind(*id),
                    NodeKind::MethodCall {
                        name,
                        receiver: Some(_),
                        ..
                    } if name == "sum"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reductions.len(),
        expected_count,
        "one reduction call per anchor"
    );

    for reduction in reductions {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } = db.node_kind(reduction)
        else {
            unreachable!("filtered to a reduction method")
        };
        let receiver_target = bound_array_source(db, *receiver);
        assert_eq!(
            db.source_identity(receiver_target),
            db.source_identity(array),
            "fixed-array reduction receiver binds to the same outer array as its writer"
        );

        let assignment = db
            .node_ids()
            .find(|id| {
                db.node(*id).line() == db.node(reduction).line()
                    && matches!(
                        db.node_kind(*id),
                        NodeKind::Stmt(StmtKind::Assign {
                            blocking: true,
                            op: Operation::Assignment,
                            delay: None,
                        })
                    )
                    && db
                        .node(*id)
                        .children()
                        .get(1)
                        .is_some_and(|rhs| node_contains(db, *rhs, reduction))
            })
            .expect("reduction result is the RHS of a blocking assignment");
        let children = db.node(assignment).children();
        let result_slot = required_direct_ref_target(db, children[0]);
        assert_eq!(db.node(result_slot).name(), sum_name);
        assert_ne!(
            db.source_identity(result_slot),
            db.source_identity(array),
            "result target is distinct from the fixed-array reduction receiver"
        );
        assert!(matches!(
            db.node_kind(process_ancestor(db, reduction)),
            NodeKind::Process {
                kind: ProcessKind::Initial
            }
        ));
    }
}

#[test]
fn fixed_array_reductions_observe_continuous_and_nba_writes_in_both_cli_modes() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv\n"
    ));
    for anchor in [
        "assign net_lanes[0] = net_seed;",
        "assign net_lanes[1] = 8'd2;",
        "assign variable_lanes[0] = variable_seed;",
        "assign variable_lanes[1] = 8'd2;",
        "nba_lanes[0] <= 8'd3;",
        "nba_lanes[1] <= 8'd2;",
        "net_sum = net_lanes.sum();",
        "variable_sum = variable_lanes.sum();",
        "nba_sum = nba_lanes.sum();",
        "#0;",
        "$finish(0);",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "missing write/use anchor: {anchor}"
        );
    }
    for vector in [
        "CP=fixed_array_reduction, CT=none, IN=none, WK=continuous_net",
        "CP=fixed_array_reduction, CT=none, IN=none, WK=continuous_variable",
        "CP=fixed_array_reduction, CT=none, IN=none, WK=procedural_nba",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(vector),
            "missing focal vector: {vector}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "written_reduction_continuous_and_nba",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_reduction_receivers_match_continuous_and_nba_element_lhss() {
    let db = compile_fixture();
    let top = top_module(&db);
    let net_lanes = outer_array(&db, top, "net_lanes", "wire [7:0] net_lanes [0:1];");
    let variable_lanes = outer_array(
        &db,
        top,
        "variable_lanes",
        "logic [7:0] variable_lanes [0:1];",
    );
    let nba_lanes = outer_array(&db, top, "nba_lanes", "bit [7:0] nba_lanes [0:1];");

    assert_static_array(&db, net_lanes, Some(NetType::Wire));
    assert_static_array(&db, variable_lanes, None);
    assert_static_array(&db, nba_lanes, None);

    assert_continuous_element_writer(
        &db,
        net_lanes,
        "assign net_lanes[0] = net_seed;",
        Some("net_seed"),
    );
    assert_continuous_element_writer(&db, net_lanes, "assign net_lanes[1] = 8'd2;", None);
    assert_continuous_element_writer(
        &db,
        variable_lanes,
        "assign variable_lanes[0] = variable_seed;",
        Some("variable_seed"),
    );
    assert_continuous_element_writer(
        &db,
        variable_lanes,
        "assign variable_lanes[1] = 8'd2;",
        None,
    );
    assert_nba_element_writer(&db, nba_lanes, "nba_lanes[0] <= 8'd3;");
    assert_nba_element_writer(&db, nba_lanes, "nba_lanes[1] <= 8'd2;");

    assert_reductions_share_writer_source(
        &db,
        net_lanes,
        "net_sum = net_lanes.sum();",
        "net_sum",
        2,
    );
    assert_reductions_share_writer_source(
        &db,
        variable_lanes,
        "variable_sum = variable_lanes.sum();",
        "variable_sum",
        2,
    );
    assert_reductions_share_writer_source(
        &db,
        nba_lanes,
        "nba_sum = nba_lanes.sum();",
        "nba_sum",
        2,
    );
}
