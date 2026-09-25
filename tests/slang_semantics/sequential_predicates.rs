//! R06: snapshot-owned clause order and branch roles survive native teardown.
use llg::core::{
    compile,
    db::{ConditionalPatternKind, Db, ExprKind, NodeKind, StmtKind},
};
use llg::sim::{codegen, opt::OptConfig};
use std::collections::HashSet;

fn capture(name: &str, source: &str) -> Db {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(name, source)],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("legal sequential predicate source");
    Db::from_slang(&compiled.snapshot).expect("owned sequential predicate import")
}

#[test]
fn sequential_predicate_import_keeps_all_clauses_and_role_resolved_branches() {
    let db = capture(
        "truth-table.sv",
        include_str!("../fixtures/sim/sequential_predicates/truth_table.sv"),
    );
    db.validate().unwrap();
    let mut statements = 0;
    let mut expressions = 0;
    for node in db.nodes() {
        let predicate = match &node.kind {
            NodeKind::Stmt(StmtKind::IfElse {
                predicate,
                if_true,
                if_false,
                ..
            }) if predicate.clauses.len() == 3 => {
                statements += 1;
                assert!(matches!(db.node_kind(*if_true), NodeKind::Stmt(_)));
                assert!(matches!(
                    db.node_kind(if_false.expect("else branch")),
                    NodeKind::Stmt(_)
                ));
                predicate
            }
            NodeKind::Expr(ExprKind::Conditional {
                predicate,
                if_true,
                if_false,
            }) => {
                expressions += 1;
                assert!(matches!(db.node_kind(*if_true), NodeKind::Expr(_)));
                assert!(matches!(db.node_kind(*if_false), NodeKind::Expr(_)));
                predicate
            }
            _ => continue,
        };
        assert_eq!(predicate.clauses.len(), 3);
        assert!(!predicate.has_patterns());
        let names: Vec<_> = predicate
            .clauses
            .iter()
            .map(|clause| {
                let NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) = db.node_kind(clause.expression)
                else {
                    panic!("expected one source reference per clause");
                };
                db.node(*target).name.as_str()
            })
            .collect();
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
        (
            "branch_roles.sv",
            include_str!("../fixtures/sim/sequential_predicates/branch_roles.sv"),
        ),
        (
            "effects.sv",
            include_str!("../fixtures/sim/sequential_predicates/effects.sv"),
        ),
        (
            "constant_prefix.sv",
            include_str!("../fixtures/sim/sequential_predicates/constant_prefix.sv"),
        ),
        (
            "wide_truth.sv",
            include_str!("../fixtures/sim/sequential_predicates/wide_truth.sv"),
        ),
        (
            "real_result.sv",
            include_str!("../fixtures/sim/sequential_predicates/real_result.sv"),
        ),
        (
            "array_results.sv",
            include_str!("../fixtures/sim/sequential_predicates/array_results.sv"),
        ),
        (
            "clock_sensitivity.sv",
            include_str!("../fixtures/sim/sequential_predicates/clock_sensitivity.sv"),
        ),
        (
            "nested_qualifiers.sv",
            include_str!("../fixtures/sim/sequential_predicates/nested_qualifiers.sv"),
        ),
        (
            "reduction_context.sv",
            include_str!("../fixtures/sim/sequential_predicates/reduction_context.sv"),
        ),
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
fn syn_023_import_retains_recursive_structure_pattern_roles() {
    let db = capture(
        "syn_023_structure_patterns.sv",
        include_str!("../fixtures/sim/sequential_predicates/syn_023_structure_patterns.sv"),
    );
    db.validate().unwrap();
    let mut structures = 0;
    let mut nested_structures = 0;
    let mut bindings = HashSet::new();
    for id in db.node_ids() {
        let Some(info) = db.conditional_pattern(id) else {
            continue;
        };
        if info.kind != ConditionalPatternKind::Structure {
            continue;
        }
        structures += 1;
        let fields = db
            .conditional_pattern_fields(id)
            .expect("structure pattern field roles");
        assert!(!fields.is_empty());
        let mut names = HashSet::new();
        for field in fields {
            assert!(names.insert(db.node(field.field).name.clone()));
            assert!(db.type_descriptor(field.field).is_some());
            let nested = db
                .conditional_pattern(field.pattern)
                .expect("nested pattern metadata");
            if nested.kind == ConditionalPatternKind::Structure {
                nested_structures += 1;
            }
            if let Some(target) = nested.binding {
                bindings.insert(target);
            }
        }
    }
    assert!(structures >= 5, "structure patterns were not retained");
    assert!(
        nested_structures >= 1,
        "nested structure roles were not retained"
    );
    assert!(
        bindings.len() >= 5,
        "nested binding identities were not retained"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&db, &options).unwrap();
    }
}

#[test]
fn syn_024_import_retains_tagged_members_and_payload_patterns() {
    let db = capture(
        "syn_024_tagged_patterns.sv",
        include_str!("../fixtures/sim/sequential_predicates/syn_024_tagged_patterns.sv"),
    );
    db.validate().expect("tagged pattern owned graph validates");
    let mut tagged = 0;
    let mut payload_patterns = 0;
    for id in db.node_ids() {
        let Some(info) = db.conditional_pattern(id) else {
            continue;
        };
        if info.kind != ConditionalPatternKind::Tagged {
            continue;
        }
        tagged += 1;
        let member = info.tagged_member.expect("tagged member identity");
        assert!(!db.node(member).name.is_empty());
        assert!(db.type_descriptor(member).is_some());
        if let Some(payload) = info.value_pattern {
            payload_patterns += 1;
            assert!(db.conditional_pattern(payload).is_some());
        }
    }
    assert!(tagged >= 8, "tagged pattern identities were not retained");
    assert!(
        payload_patterns >= 5,
        "tagged payload pattern identities were not retained"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        let model = codegen::generate_from_db_with_opts(&db, &options)
            .expect("tagged patterns lower from owned metadata");
        assert!(model.model_c.contains("sv4_case_eq("));
    }
}

#[test]
fn syn_025_pattern_case_is_owned_and_generates_in_both_modes() {
    let db = capture(
        "syn_025_pattern_case.sv",
        include_str!("../fixtures/sim/sequential_predicates/syn_025_pattern_case.sv"),
    );
    db.validate().expect("pattern case owned graph validates");
    let mut pattern_cases = 0;
    let mut filtered_items = 0;
    let mut defaults = 0;
    let mut case_types = HashSet::new();
    for id in db.node_ids() {
        if let NodeKind::Stmt(StmtKind::PatternCase {
            case_type,
            selector,
            items,
            default,
            ..
        }) = db.node_kind(id)
        {
            pattern_cases += 1;
            case_types.insert(*case_type);
            assert!(db.node(*selector).parent().is_some());
            filtered_items += items.iter().filter(|item| item.filter.is_some()).count();
            defaults += usize::from(default.is_some());
        }
    }
    assert!(pattern_cases >= 7, "all pattern-case forms remain distinct");
    assert!(filtered_items >= 5, "item filters remain owned");
    assert!(defaults >= 7, "default arms remain owned");
    assert!(case_types.contains(&llg::core::db::CaseKind::Exact));
    assert!(case_types.contains(&llg::core::db::CaseKind::X));
    assert!(case_types.contains(&llg::core::db::CaseKind::Z));
    for options in [OptConfig::none(), OptConfig::default()] {
        let model = codegen::generate_from_db_with_opts(&db, &options)
            .expect("pattern case lowers from owned metadata");
        assert!(model.model_c.contains("sv4_case_eq("));
    }
}

#[test]
fn whole_fixed_bindings_and_tag_modes_lower_after_native_teardown() {
    for (name, source) in [
        (
            "n08_whole_patterns.sv",
            include_str!("../fixtures/sim/review_bundle/n08_whole_patterns.sv"),
        ),
        (
            "n09_tagged_case_modes.sv",
            include_str!("../fixtures/sim/review_bundle/n09_tagged_case_modes.sv"),
        ),
    ] {
        let db = capture(name, source);
        db.validate().unwrap();
        let mut bindings = 0;
        for id in db.node_ids() {
            if let Some(info) = db.conditional_pattern(id) {
                if info.kind == ConditionalPatternKind::Binding {
                    let target = info.binding.expect("binding retains declaration identity");
                    assert!(db.type_descriptor(target).is_some());
                    bindings += 1;
                }
            }
        }
        assert!(bindings > 0, "no binding captured from {name}");
        for options in [OptConfig::none(), OptConfig::default()] {
            let model = codegen::generate_from_db_with_opts(&db, &options)
                .expect("whole fixed patterns and match modes lower from owned data");
            assert!(model.model_c.contains("sv4_clone("));
            if name.starts_with("n09") {
                assert!(model.model_c.contains("sv4_casex_eq("));
                assert!(model.model_c.contains("sv4_casez_eq("));
            }
        }
    }
}
