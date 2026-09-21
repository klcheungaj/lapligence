//! R06: snapshot-owned clause order and branch roles survive native teardown.
use llg::core::{
    compile,
    db::{ConditionalPatternKind, Db, ExprKind, NodeKind, StmtKind},
};
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
fn syn_022_import_retains_primitive_pattern_ownership() {
    let db = capture(
        "syn_022_basic_patterns.sv",
        include_str!("../fixtures/sim/sequential_predicates/syn_022_basic_patterns.sv"),
    );
    db.validate().unwrap();
    let mut constants = 0;
    let mut wildcards = 0;
    let mut bindings = 0;
    let pattern_ids = db
        .nodes()
        .iter()
        .flat_map(|node| match &node.kind {
            NodeKind::Stmt(StmtKind::IfElse { predicate, .. })
            | NodeKind::Expr(ExprKind::Conditional { predicate, .. }) => predicate
                .clauses
                .iter()
                .filter_map(|clause| clause.pattern)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    for id in pattern_ids {
        let info = db.conditional_pattern(id).expect("owned pattern metadata");
        assert!(matches!(db.node_kind(id), NodeKind::Other));
        match info.kind {
            ConditionalPatternKind::Constant => constants += 1,
            ConditionalPatternKind::Wildcard => wildcards += 1,
            ConditionalPatternKind::Binding => {
                bindings += 1;
                let target = info.binding.expect("binding pattern target");
                assert!(matches!(db.node_kind(target), NodeKind::Var { .. }));
                assert!(db.is_conditional_pattern_binding(target));
                assert_eq!(
                    db.variable_lifetime(target),
                    llg::core::db::VariableLifetime::Automatic
                );
            }
            other => panic!("unexpected primitive fixture pattern: {other:?}"),
        }
    }
    assert!(constants >= 3, "constant patterns were not retained");
    assert!(wildcards >= 1, "wildcard patterns were not retained");
    assert!(bindings >= 8, "binding identities were not retained");
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&db, &options).unwrap();
    }
}

#[test]
fn sequential_predicate_unsupported_patterns_are_retained_and_rejected_before_optimization() {
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
