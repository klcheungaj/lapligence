//! R04: the module assignment reaches typed array lowering without a wrapper call.
use llg::core::compile;
use llg::core::db::{Db, ExprKind, NodeKind, Operation, StmtKind, TypeShape};
use llg::sim::{codegen, opt::OptConfig};

fn capture(name: &str, source: &str) -> Db {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(name, source)],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("valid array conditional assignment source");
    Db::from_slang(&compiled.snapshot).expect("owned array conditional assignment import")
}

#[test]
fn array_conditional_assignment_import_keeps_direct_module_rhs() {
    let database = capture(
        "array-conditional-comb.sv",
        include_str!("../fixtures/sim/array_conditional_assignments/comb.sv"),
    );
    database.validate().expect("valid imported database");
    let mut direct = 0;
    for node in database.nodes() {
        if !matches!(&node.kind, NodeKind::Stmt(StmtKind::Assign { .. })) {
            continue;
        }
        let Some(rhs) = node.children.get(1).copied() else {
            continue;
        };
        if !matches!(
            database.node_kind(rhs),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                ..
            })
        ) {
            continue;
        }
        let descriptor = database
            .type_descriptor(rhs)
            .expect("owned conditional result type");
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            panic!("conditional lost its unpacked result type");
        };
        assert_eq!(dimensions.as_slice(), &[(0, 1)]);
        assert_eq!(element.info.width, Some(8));
        direct += 1;
    }
    assert_eq!(
        direct, 1,
        "a cast or function wrapper must not hide the original R04 gate"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        let model = codegen::generate_from_db_with_opts(&database, &options)
            .expect("direct module array conditional lowers after snapshot drop");
        assert!(model.model_c.contains("sv4_array_conditional_merge_to("));
        // The owned emitter replaces IR-local names with scope slots. Exact
        // snapshot / one-evaluation behavior is covered by the CLI effects,
        // overlap and NBA cases, not by spelling a lowerer-private local name.
        assert!(model.model_c.contains("sv4_part_select_to("));
    }
}

#[test]
fn array_conditional_assignment_contexts_lower_after_snapshot_drop() {
    for (name, source) in [
        (
            "effects.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/effects.sv"),
        ),
        (
            "overlap.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/overlap.sv"),
        ),
        (
            "nba.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/nba.sv"),
        ),
        (
            "clocked.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/clocked.sv"),
        ),
        (
            "views.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/views.sv"),
        ),
        (
            "shapes.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/shapes.sv"),
        ),
        (
            "expressions.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/expressions.sv"),
        ),
        (
            "typed_defaults.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/typed_defaults.sv"),
        ),
        (
            "deep_defaults.sv",
            include_str!("../fixtures/sim/array_conditional_assignments/deep_defaults.sv"),
        ),
    ] {
        let database = capture(name, source);
        database.validate().expect("valid captured array graph");
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }
}

#[test]
fn array_conditional_assignment_nested_defaults_export_only_bound_values() {
    let database = capture(
        "nested-defaults.sv",
        include_str!("../fixtures/sim/array_conditional_assignments/nested_defaults.sv"),
    );
    database.validate().expect("valid nested default graph");
    let mut patterns = 0;
    let mut shared_rows = 0;
    for id in database.node_ids() {
        let node = database.node(id);
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::AssignmentPattern,
            operands,
            ..
        }) = &node.kind
        else {
            continue;
        };
        let Some(descriptor) = database.type_descriptor(id) else {
            continue;
        };
        let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
            continue;
        };
        if dimensions.len() == 1
            && database.semantic_detail(id) == Some("SimpleAssignmentPattern")
            && operands
                .first()
                .is_some_and(|first| operands.iter().all(|operand| operand == first))
        {
            let (left, right) = dimensions[0];
            let count = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
            assert_eq!(
                operands.len() as u64,
                count,
                "a synthesized row must retain every positional occurrence"
            );
            assert_eq!(operands.len(), 2, "this fixture has two bytes per row");
            assert_eq!(
                operands[0], operands[1],
                "shared expression identities must not collapse operand slots"
            );
            shared_rows += 1;
        }
        if dimensions.len() != 2 {
            continue;
        }
        patterns += 1;
        assert_eq!(operands.len(), 3, "each outer row needs a bound operand");
        for operand in operands {
            assert!(
                matches!(
                    database.node_kind(*operand),
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::AssignmentPattern,
                        ..
                    })
                ),
                "untyped default placeholders must not be executable operands"
            );
        }
    }
    assert_eq!(patterns, 2);
    assert_eq!(
        shared_rows, 2,
        "each default generates a shared two-byte row"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("bound nested defaults lower after snapshot destruction");
    }
}

#[test]
fn array_conditional_assignment_shared_default_operands_keep_every_slot() {
    use llg::ffi::slang::{SemanticEdgeRole, SemanticOperation};

    for extent in [1, 2, 3, 17] {
        let source = format!(
            "// llg-test-fixture: tests/slang_semantics/array_conditional_assignments.rs/shared-default-slots\n\
             module tb; logic [7:0] rows [1:0][-1:{}];\n\
             initial begin rows = '{{default: '{{default: 1'b0}}}}; $finish(0); end\n\
             endmodule\n",
            extent - 2,
        );
        let compiled = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "shared-default-slots.sv",
                source,
            )],
            &compile::CompileOpts {
                top: Some("tb".into()),
                ..Default::default()
            },
        )
        .expect("valid nested-default source");
        let snapshot = &compiled.snapshot;
        let mut rows = 0;
        for node in &snapshot.semantic_nodes {
            if node.detail != "SimpleAssignmentPattern" {
                continue;
            }
            assert_eq!(node.operation, SemanticOperation::AssignmentPattern);
            let edges = super::edges(snapshot, node);
            let operands: Vec<_> = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect();
            assert_eq!(operands.len(), extent as usize, "extent={extent}");
            for (index, edge) in operands.iter().enumerate() {
                assert_eq!(
                    edge.index, index as u32,
                    "each occurrence needs its own index"
                );
                assert_eq!(
                    edge.target_id, operands[0].target_id,
                    "Slang shares the value"
                );
            }
            assert!(edges
                .iter()
                .all(|edge| edge.role != SemanticEdgeRole::Child));
            rows += 1;
        }
        assert_eq!(rows, 1, "one shared synthesized row is sufficient");
        let database =
            Db::from_slang(snapshot).expect("shared operands import without deduplication");
        drop(compiled);
        database.validate().expect("owned shared-row graph");
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("all operand occurrences lower after snapshot destruction");
        }
    }
}

#[test]
fn assignment_pattern_index_key_keeps_owned_expression_and_constant_value() {
    let database = capture(
        "pattern-index-key.sv",
        "module tb;\n\
         localparam int BASE = 0;\n\
         int values [0:1]; int seed;\n\
         initial values = '{(BASE + 1): seed, default: 0};\n\
         endmodule\n",
    );
    database
        .validate()
        .expect("valid assignment-pattern key graph");

    let (key_expression, key_value) = database
        .node_ids()
        .find_map(|id| match database.node_kind(id) {
            NodeKind::Expr(ExprKind::TaggedPattern {
                index_key: Some(key_expression),
                index_value: Some(key_value),
                ..
            }) => Some((*key_expression, key_value.to_i128())),
            _ => None,
        })
        .expect("constant array index key is retained as owned semantic data");
    assert_eq!(key_value, Some(1));
    assert!(matches!(
        database.node_kind(key_expression),
        NodeKind::Expr(ExprKind::Operation { .. })
            | NodeKind::Expr(ExprKind::Cast { .. })
            | NodeKind::Expr(ExprKind::Constant { .. })
    ));
}

#[test]
fn replicated_assignment_pattern_import_keeps_count_and_element_order() {
    use llg::ffi::slang::{SemanticEdgeRole, SemanticOperation};

    let source = "// llg-test-fixture: tests/slang_semantics/array_conditional_assignments.rs/replicated-pattern-edges\n\
                  module tb;\n\
                  logic [7:0] y; logic [7:0] values [0:3];\n\
                  initial values = '{2{y, 8'h12}};\n\
                  endmodule\n";
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "replicated-pattern-edges.sv",
            source,
        )],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("valid replicated assignment pattern source");
    let snapshot = &compiled.snapshot;
    let pattern = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.operation == SemanticOperation::MultiAssignmentPattern)
        .expect("replicated assignment pattern node");
    let pattern_edges = super::edges(snapshot, pattern);
    let widths: Vec<_> = pattern_edges
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Width)
        .collect();
    assert_eq!(widths.len(), 1, "the repetition count is one width edge");
    let operands: Vec<_> = pattern_edges
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Operand)
        .collect();
    assert_eq!(operands.len(), 2, "the syntactic element list is retained");
    assert_eq!(
        operands.iter().map(|edge| edge.index).collect::<Vec<_>>(),
        [0, 1],
        "replicated pattern elements retain source order"
    );
    let database = Db::from_slang(snapshot).expect("replicated pattern import");
    let pattern_id = database
        .node_ids()
        .find(|id| {
            matches!(
                database.node_kind(*id),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::MultiAssignmentPattern,
                    ..
                })
            )
        })
        .expect("owned replicated assignment pattern node");
    let NodeKind::Expr(ExprKind::Operation { operands, .. }) = database.node_kind(pattern_id)
    else {
        unreachable!();
    };
    assert_eq!(
        operands.len(),
        3,
        "owned operands retain count plus elements"
    );
    assert!(matches!(
        database.node_kind(operands[1]),
        NodeKind::Expr(ExprKind::Ref { .. })
    ));
    assert_ne!(
        operands[1], operands[2],
        "distinct syntactic elements must retain distinct operand slots"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("replicated pattern edges lower after snapshot drop");
    }
}

#[test]
fn mixed_index_type_patterns_export_bound_operands_in_declaration_order() {
    let database = capture(
        "mixed-index-types.sv",
        r#"module tb;
            typedef struct { int x; int y; } record_t;
            record_t values[2:0];
            record_t special;
            int seed;
            initial values = '{1:special, int:seed};
        endmodule"#,
    );
    let mut found = 0;
    for id in database.node_ids() {
        if database.semantic_detail(id) != Some("StructuredAssignmentPattern") {
            continue;
        }
        let Some(descriptor) = database.type_descriptor(id) else {
            continue;
        };
        let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
            continue;
        };
        if dimensions.as_slice() != [(2, 0)] {
            continue;
        }
        let NodeKind::Expr(ExprKind::Operation { operands, .. }) = database.node_kind(id) else {
            panic!("pattern operation was lost")
        };
        assert_eq!(operands.len(), 3);
        assert_eq!(
            operands[0], operands[2],
            "shared recursive values keep both positions"
        );
        let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = database.node_kind(operands[1])
        else {
            panic!("middle declared element must retain its explicit override")
        };
        assert_eq!(database.node(*target).name, "special");
        for operand in [operands[0], operands[2]] {
            let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) =
                database.node_kind(operand)
            else {
                panic!("recursive record must be a resolved pattern value")
            };
            assert_eq!(*op, Operation::AssignmentPattern);
            assert_eq!(operands.len(), 2);
            assert_eq!(operands[0], operands[1]);
        }
        found += 1;
    }
    assert_eq!(
        found, 1,
        "mixed root must not remain an unresolved setter list"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("mixed type/index values lower after snapshot destruction");
    }
}
