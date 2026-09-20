//! R06: snapshot-owned clause order and branch roles survive native teardown.
use llg::core::{compile, db::{Db, ExprKind, NodeKind, StmtKind}};
use llg::sim::{codegen, opt::OptConfig};

fn capture(name: &str, source: &str) -> Db {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(name, source)],
        &compile::CompileOpts { top: Some("tb".into()), ..Default::default() },
    ).expect("legal sequential predicate source");
    Db::from_slang(&compiled.snapshot).expect("owned sequential predicate import")
}

#[test]
fn sequential_predicate_import_keeps_all_clauses_and_role_resolved_branches() {
    let db = capture("truth-table.sv",
        include_str!("../fixtures/sim/sequential_predicates/truth_table.sv"));
    db.validate().unwrap();
    let mut statements = 0;
    let mut expressions = 0;
    for node in db.nodes() {
        let predicate = match &node.kind {
            NodeKind::Stmt(StmtKind::IfElse { predicate, if_true, if_false, .. })
                if predicate.clauses.len() == 3 =>
            {
                statements += 1;
                assert!(matches!(db.node_kind(*if_true), NodeKind::Stmt(_)));
                assert!(matches!(db.node_kind(if_false.expect("else branch")), NodeKind::Stmt(_)));
                predicate
            }
            NodeKind::Expr(ExprKind::Conditional { predicate, if_true, if_false }) => {
                expressions += 1;
                assert!(matches!(db.node_kind(*if_true), NodeKind::Expr(_)));
                assert!(matches!(db.node_kind(*if_false), NodeKind::Expr(_)));
                predicate
            }
            _ => continue,
        };
        assert_eq!(predicate.clauses.len(), 3);
        assert!(!predicate.has_patterns());
        let names: Vec<_> = predicate.clauses.iter().map(|clause| {
            let NodeKind::Expr(ExprKind::Ref { target: Some(target) }) = db.node_kind(clause.expression)
                else { panic!("expected one source reference per clause"); };
            db.node(*target).name.as_str()
        }).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
    assert_eq!((statements, expressions), (1, 1));
    for options in [OptConfig::none(), OptConfig::default()] {
        let model = codegen::generate_from_db_with_opts(&db, &options).unwrap();
        assert!(model.model_c.contains("sv4_reduce_or("));
    }
}

#[test]
fn sequential_predicate_contexts_generate_from_owned_database() {
    for (name, source) in [
        ("branch_roles.sv", include_str!("../fixtures/sim/sequential_predicates/branch_roles.sv")),
        ("effects.sv", include_str!("../fixtures/sim/sequential_predicates/effects.sv")),
        ("constant_prefix.sv", include_str!("../fixtures/sim/sequential_predicates/constant_prefix.sv")),
        ("wide_truth.sv", include_str!("../fixtures/sim/sequential_predicates/wide_truth.sv")),
        ("real_result.sv", include_str!("../fixtures/sim/sequential_predicates/real_result.sv")),
        ("array_results.sv", include_str!("../fixtures/sim/sequential_predicates/array_results.sv")),
        ("clock_sensitivity.sv", include_str!("../fixtures/sim/sequential_predicates/clock_sensitivity.sv")),
        ("nested_qualifiers.sv", include_str!("../fixtures/sim/sequential_predicates/nested_qualifiers.sv")),
        ("reduction_context.sv", include_str!("../fixtures/sim/sequential_predicates/reduction_context.sv")),
    ] {
        let db = capture(name, source);
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&db, &options)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }
}

#[test]
fn sequential_predicate_patterns_are_retained_and_rejected_before_optimization() {
    for (name, source) in [
        ("pattern-if.sv", include_str!("../fixtures/sim/sequential_predicates/bad_matches_if.sv")),
        ("pattern-conditional.sv", include_str!("../fixtures/sim/sequential_predicates/bad_matches_conditional.sv")),
    ] {
        let db = capture(name, source);
        assert!(db.nodes().iter().any(|node| match &node.kind {
            NodeKind::Stmt(StmtKind::IfElse { predicate, .. })
            | NodeKind::Expr(ExprKind::Conditional { predicate, .. }) => predicate.has_patterns(),
            _ => false,
        }));
        for options in [OptConfig::none(), OptConfig::default()] {
            let error = match codegen::generate_from_db_with_opts(&db, &options) {
                Ok(_) => panic!("{name}: pattern was silently erased"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("pattern"), "{name}: {error}");
        }
    }
}
