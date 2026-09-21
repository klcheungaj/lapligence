//! SYN-003 owned-graph coverage for positional assignment-pattern lvalues.

use llg::core::compile;
use llg::core::db::{Db, ExprKind, NodeKind, Operation};
use llg::sim::{codegen, opt::OptConfig};

fn is_empty_argument(db: &Db, mut node: llg::core::db::NodeId) -> bool {
    while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = db.node_kind(node) {
        node = *operand;
    }
    matches!(db.node_kind(node), NodeKind::Expr(ExprKind::Other))
        && db.semantic_detail(node) == Some("EmptyArgument")
}

fn is_pattern_target(db: &Db, node: llg::core::db::NodeId) -> bool {
    matches!(
        db.node_kind(node),
        NodeKind::Expr(
            ExprKind::Ref { .. }
                | ExprKind::BitSelect { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. }
                | ExprKind::ArraySelect { .. }
                | ExprKind::HierPath { .. }
                | ExprKind::Operation {
                    op: Operation::AssignmentPattern,
                    ..
                }
        )
    )
}

#[test]
fn positional_lvalue_operands_survive_snapshot_drop() {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "syn_003_pattern_lvalues.sv",
            include_str!("fixtures/sim/syn003_pattern_lvalues/syn_003_pattern_lvalues.sv"),
        )],
        &compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("SYN-003 legal fixture compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("SYN-003 owned database");
    database.validate().expect("SYN-003 database validates");

    let lvalue_patterns = database
        .node_ids()
        .filter(|id| {
            let NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                operands,
                ..
            }) = database.node_kind(*id)
            else {
                return false;
            };
            !operands.is_empty()
                && operands.iter().all(|operand| {
                    let NodeKind::Expr(ExprKind::Operation {
                        op: Operation::Assignment,
                        operands,
                        ..
                    }) = database.node_kind(*operand)
                    else {
                        return false;
                    };
                    operands.len() == 2
                        && is_empty_argument(&database, operands[1])
                        && is_pattern_target(&database, operands[0])
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(lvalue_patterns.len(), 10);
    for pattern in lvalue_patterns {
        let NodeKind::Expr(ExprKind::Operation { operands, .. }) = database.node_kind(pattern)
        else {
            unreachable!();
        };
        for operand in operands {
            let NodeKind::Expr(ExprKind::Operation { operands, .. }) = database.node_kind(*operand)
            else {
                unreachable!();
            };
            assert_eq!(operands.len(), 2);
            assert!(is_empty_argument(&database, operands[1]));
        }
    }
    drop(compiled);
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("positional assignment-pattern lvalues lower after snapshot drop");
    }
}
