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
        assert!(model.model_c.contains("_p30_value_"));
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
    ] {
        let database = capture(name, source);
        database.validate().expect("valid captured array graph");
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }
}
