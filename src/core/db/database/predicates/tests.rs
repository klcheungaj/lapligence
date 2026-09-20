//! Predicate edges must not be mistaken for statement branches.
use super::*;

fn edge(role: SemanticEdgeRole, index: u32, target_id: u64) -> SemanticEdge {
    SemanticEdge { role, index, target_id, sequence_delay: None }
}

fn ids() -> HashMap<u64, NodeId> {
    (0..6).map(|index| (10 + u64::from(index), NodeId(index))).collect()
}

#[test]
fn sequential_predicate_import_orders_clauses_by_role_index() {
    let edges = [
        edge(SemanticEdgeRole::Condition, 2, 12),
        edge(SemanticEdgeRole::Else, 0, 15),
        edge(SemanticEdgeRole::ConditionPattern, 1, 13),
        edge(SemanticEdgeRole::Condition, 0, 10),
        edge(SemanticEdgeRole::Then, 0, 14),
        edge(SemanticEdgeRole::Condition, 1, 11),
    ];
    let predicate = predicate_from_slang(&edges, &ids()).unwrap();
    assert_eq!(predicate.clauses.iter().map(|clause| clause.expression).collect::<Vec<_>>(),
        vec![NodeId(0), NodeId(1), NodeId(2)]);
    assert_eq!(predicate.clauses[1].pattern, Some(NodeId(3)));
    assert!(predicate.has_patterns());
    assert_eq!(conditional_branches_from_slang(&edges, &ids(), true).unwrap(),
        (NodeId(4), Some(NodeId(5))));
}

#[test]
fn sequential_predicate_import_rejects_empty_duplicate_and_gapped_clauses() {
    for indices in [vec![], vec![1], vec![0, 0], vec![0, 2], vec![0, u32::MAX]] {
        let edges = indices.into_iter().map(|index|
            edge(SemanticEdgeRole::Condition, index, 10)).collect::<Vec<_>>();
        assert!(predicate_from_slang(&edges, &ids()).is_err());
    }
}

#[test]
fn sequential_predicate_import_rejects_orphan_duplicate_and_dangling_patterns() {
    for patterns in [
        vec![edge(SemanticEdgeRole::ConditionPattern, 1, 11)],
        vec![edge(SemanticEdgeRole::ConditionPattern, 0, 11),
             edge(SemanticEdgeRole::ConditionPattern, 0, 12)],
        vec![edge(SemanticEdgeRole::ConditionPattern, 0, 99)],
    ] {
        let mut edges = vec![edge(SemanticEdgeRole::Condition, 0, 10)];
        edges.extend(patterns);
        assert!(predicate_from_slang(&edges, &ids()).is_err());
    }
    assert!(predicate_from_slang(&[edge(SemanticEdgeRole::Condition, 0, 99)], &ids()).is_err());
}

#[test]
fn sequential_predicate_branches_ignore_positional_children_and_allow_missing_else() {
    let mut edges = vec![
        edge(SemanticEdgeRole::Child, 0, 10),
        edge(SemanticEdgeRole::Child, 1, 11),
        edge(SemanticEdgeRole::Condition, 0, 10),
        edge(SemanticEdgeRole::Condition, 1, 11),
        edge(SemanticEdgeRole::Then, 0, 14),
    ];
    assert_eq!(conditional_branches_from_slang(&edges, &ids(), false).unwrap(),
        (NodeId(4), None));
    assert!(conditional_branches_from_slang(&edges, &ids(), true).is_err());
    edges.push(edge(SemanticEdgeRole::Else, 0, 15));
    assert_eq!(conditional_branches_from_slang(&edges, &ids(), true).unwrap(),
        (NodeId(4), Some(NodeId(5))));
}

#[test]
fn sequential_predicate_import_rejects_invalid_branch_roles() {
    let branch = edge(SemanticEdgeRole::Then, 0, 14);
    for edges in [
        vec![],
        vec![edge(SemanticEdgeRole::Then, 1, 14)],
        vec![branch.clone(), branch.clone()],
        vec![edge(SemanticEdgeRole::Then, 0, 99)],
        vec![branch.clone(), edge(SemanticEdgeRole::Else, 1, 15)],
        vec![branch.clone(), edge(SemanticEdgeRole::Else, 0, 15),
             edge(SemanticEdgeRole::Else, 0, 15)],
        vec![branch, edge(SemanticEdgeRole::Else, 0, 99)],
    ] {
        assert!(conditional_branches_from_slang(&edges, &ids(), false).is_err());
    }
}

#[test]
fn sequential_predicate_references_include_patterns_and_every_clause() {
    let predicate = ConditionalPredicate { clauses: vec![
        PredicateClause { expression: NodeId(1), pattern: Some(NodeId(2)) },
        PredicateClause { expression: NodeId(3), pattern: None },
    ] };
    for kind in [
        NodeKind::Stmt(StmtKind::IfElse {
            predicate: predicate.clone(), if_true: NodeId(4), if_false: Some(NodeId(5)),
            check: UniquePriorityCheck::None,
        }),
        NodeKind::Expr(ExprKind::Conditional {
            predicate, if_true: NodeId(4), if_false: NodeId(5),
        }),
    ] {
        let mut refs = Vec::new();
        kind.append_references(&mut refs);
        assert_eq!(refs, vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5)]);
    }
}
