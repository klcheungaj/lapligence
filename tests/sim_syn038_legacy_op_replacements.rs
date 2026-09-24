//! Checked-in replacement evidence for SYN-038 legacy OP/write pairings.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{AlwaysKind, Db, ExprKind, NodeId, NodeKind, Operation, ProcessKind},
};

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/legacy_op_replacements.sv";
const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/legacy_op_replacements.sv");
const EXPECTED_STDOUT: &str = "conditional=a5,a5 equality=b6,b6 cast=c7,c7 pattern=d8,d8 net=05 union=5678,1234 source=11,22\n";
const EXPECTED_STDERR: &str = "";

const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "conditional_fields[choose ? 1 : 0].value <= 8'ha5;",
    "equality_fields[choose == 1'b1].value <= 8'hb6;",
    "cast_fields[slot_index_t'(choose)].value <= 8'hc7;",
    "pattern_fields[slot_index_t'{1'b0, choose}].value <= 8'hd8;",
    "assign conditional_net[0] = net_select ? conditional_net[2] : conditional_net[1];",
    "process_observed <= process_select ? process_source_right : process_source_left;",
    "union_observed = read_union(\n            union_select ? make_left_union() : make_right_union());",
];

fn named_decl(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Array { .. } | NodeKind::Var { .. } | NodeKind::Net { .. }
                )
        })
        .unwrap_or_else(|| panic!("Slang DB did not capture declaration {name}"))
}

fn named_function(db: &Db, name: &str) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && matches!(db.node_kind(*id), NodeKind::FuncTask { is_task: false, .. })
        })
        .unwrap_or_else(|| panic!("Slang DB did not capture function {name}"))
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

fn has_bound_reference(db: &Db, root: NodeId, target: NodeId) -> bool {
    descendants(db, root).into_iter().any(|id| {
        matches!(
            db.node_kind(id),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(reference)
            }) if db.source_identity(*reference) == db.source_identity(target)
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

fn selected_array_index(db: &Db, array: NodeId, expected: Operation) -> NodeId {
    let matches: Vec<_> = db
        .node_ids()
        .filter_map(|id| match db.node_kind(id) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if db.source_identity(*base) == db.source_identity(array)
                    && indices.len() == 1
                    && has_operation(db, indices[0], expected) =>
            {
                Some(indices[0])
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one {expected:?} index expression on {}",
        db.node(array).name()
    );
    matches[0]
}

fn selected_array_cast_index(db: &Db, array: NodeId) -> NodeId {
    let matches: Vec<_> = db
        .node_ids()
        .filter_map(|id| match db.node_kind(id) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if db.source_identity(*base) == db.source_identity(array)
                    && indices.len() == 1
                    && matches!(
                        db.node_kind(indices[0]),
                        NodeKind::Expr(ExprKind::Cast { .. })
                    ) =>
            {
                Some(indices[0])
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one cast index expression on {}",
        db.node(array).name()
    );
    matches[0]
}

fn selected_array_pattern_index(db: &Db, array: NodeId) -> NodeId {
    let matches: Vec<_> = db
        .node_ids()
        .filter_map(|id| match db.node_kind(id) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                if db.source_identity(*base) == db.source_identity(array)
                    && indices.len() == 1
                    && has_operation(db, indices[0], Operation::AssignmentPattern) =>
            {
                Some(indices[0])
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one typed assignment-pattern index on {}",
        db.node(array).name()
    );
    matches[0]
}

fn enclosing_flip_flop(db: &Db, root: NodeId) -> bool {
    let mut current = Some(root);
    while let Some(id) = current {
        if matches!(
            db.node_kind(id),
            NodeKind::Process {
                kind: ProcessKind::Always {
                    always_type: AlwaysKind::FlipFlop
                }
            }
        ) {
            return true;
        }
        current = db.node(id).parent();
    }
    false
}

#[test]
fn selected_write_addresses_and_source_only_paths_have_independent_oracles() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/legacy_op_replacements.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost focal anchor: {anchor}"
        );
    }
    for oracle in [
        "selected write initial values",
        "continuous conditional selector update",
        "continuous conditional source update",
        "selected write low-address NBA readback",
        "pre-second-edge source and target readback",
        "selected write high-address NBA readback",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(oracle),
            "fixture lost readback: {oracle}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "legacy_op_replacements",
        EXPECTED_STDOUT,
        EXPECTED_STDERR,
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn slang_bindings_keep_operations_on_the_focal_address_or_source() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 focal-operation fixture compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("capture owned semantic database");

    let choose = named_decl(&db, "choose");
    for (name, operation) in [
        ("conditional_fields", Operation::Conditional),
        ("equality_fields", Operation::Equal),
    ] {
        let array = named_decl(&db, name);
        let index = selected_array_index(&db, array, operation);
        assert!(
            has_bound_reference(&db, index, choose),
            "{name} selector must bind to the declared choose variable"
        );
    }

    let cast_fields = named_decl(&db, "cast_fields");
    let cast_index = selected_array_cast_index(&db, cast_fields);
    assert!(
        has_bound_reference(&db, cast_index, choose),
        "cast selector must bind to the declared choose variable"
    );

    let pattern_fields = named_decl(&db, "pattern_fields");
    let pattern_index = selected_array_pattern_index(&db, pattern_fields);
    assert!(
        has_bound_reference(&db, pattern_index, choose),
        "typed assignment-pattern selector must bind to the declared choose variable"
    );

    let left_function = named_function(&db, "make_left_union");
    let right_function = named_function(&db, "make_right_union");
    for (name, expected) in [
        ("make_left_union", left_function),
        ("make_right_union", right_function),
    ] {
        assert!(
            db.node_ids().any(|id| matches!(
                db.node_kind(id),
                NodeKind::FuncCall { name: call_name, callee: Some(callee), is_task: false, .. }
                    if call_name == name
                        && db.source_identity(*callee) == db.source_identity(expected)
            )),
            "union source call {name} must bind to its function declaration"
        );
    }

    let process_select = named_decl(&db, "process_select");
    let process_source_left = named_decl(&db, "process_source_left");
    let process_source_right = named_decl(&db, "process_source_right");
    assert!(
        db.node_ids().any(|id| {
            matches!(
                db.node_kind(id),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Conditional,
                    ..
                })
            ) && has_bound_reference(&db, id, process_select)
                && has_bound_reference(&db, id, process_source_left)
                && has_bound_reference(&db, id, process_source_right)
                && enclosing_flip_flop(&db, id)
        }),
        "source-only conditional must bind its selector and branches inside always_ff"
    );

    let continuous_net = named_decl(&db, "conditional_net");
    let net_bit_selects = db
        .node_ids()
        .filter(|id| {
            matches!(db.node_kind(*id), NodeKind::Expr(ExprKind::BitSelect { base, .. })
                if has_bound_reference(&db, *base, continuous_net))
        })
        .count();
    assert!(
        net_bit_selects >= 3,
        "conditional net source and target selects must all bind to one outer net"
    );
}
