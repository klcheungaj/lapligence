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
        assert!(model.model_c.contains("sv4_array_conditional_merge("));
        // The owned emitter replaces IR-local names with scope slots. Exact
        // snapshot / one-evaluation behavior is covered by the CLI effects,
        // overlap and NBA cases, not by spelling a lowerer-private local name.
        assert!(model.model_c.contains("sv4_part_select("));
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
        }) = &node.kind else {
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
            assert!(matches!(
                database.node_kind(*operand),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::AssignmentPattern,
                    ..
                })
            ), "untyped default placeholders must not be executable operands");
        }
    }
    assert_eq!(patterns, 2);
    assert_eq!(shared_rows, 2, "each default generates a shared two-byte row");
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
            &[compile::OwnedSource::compilation_unit("shared-default-slots.sv", source)],
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
            assert!(edges.iter().all(|edge| edge.role != SemanticEdgeRole::Child));
            rows += 1;
        }
        assert_eq!(rows, 1, "one shared synthesized row is sufficient");
        let database = Db::from_slang(snapshot).expect("shared operands import without deduplication");
        drop(compiled);
        database.validate().expect("owned shared-row graph");
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("all operand occurrences lower after snapshot destruction");
        }
    }
}
