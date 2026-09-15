//! Assertion import.

use super::*;

fn assertion_unary_from_slang(operation: SemanticOperation) -> Result<AssertionUnaryOp, DbError> {
    Ok(match operation {
        SemanticOperation::AssertionNot => AssertionUnaryOp::Not,
        SemanticOperation::AssertionNextTime => AssertionUnaryOp::NextTime,
        SemanticOperation::AssertionSNextTime => AssertionUnaryOp::SNextTime,
        SemanticOperation::AssertionAlways => AssertionUnaryOp::Always,
        SemanticOperation::AssertionSAlways => AssertionUnaryOp::SAlways,
        SemanticOperation::AssertionEventually => AssertionUnaryOp::Eventually,
        SemanticOperation::AssertionSEventually => AssertionUnaryOp::SEventually,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion unary node has a non-unary operation".into(),
            ))
        }
    })
}

fn assertion_binary_from_slang(operation: SemanticOperation) -> Result<AssertionBinaryOp, DbError> {
    Ok(match operation {
        SemanticOperation::AssertionAnd => AssertionBinaryOp::And,
        SemanticOperation::AssertionOr => AssertionBinaryOp::Or,
        SemanticOperation::AssertionIntersect => AssertionBinaryOp::Intersect,
        SemanticOperation::AssertionThroughout => AssertionBinaryOp::Throughout,
        SemanticOperation::AssertionWithin => AssertionBinaryOp::Within,
        SemanticOperation::AssertionIff => AssertionBinaryOp::Iff,
        SemanticOperation::AssertionUntil => AssertionBinaryOp::Until,
        SemanticOperation::AssertionSUntil => AssertionBinaryOp::SUntil,
        SemanticOperation::AssertionUntilWith => AssertionBinaryOp::UntilWith,
        SemanticOperation::AssertionSUntilWith => AssertionBinaryOp::SUntilWith,
        SemanticOperation::AssertionImplies => AssertionBinaryOp::Implies,
        SemanticOperation::AssertionOverlappedImplies => AssertionBinaryOp::OverlappedImplication,
        SemanticOperation::AssertionNonOverlappedImplies => {
            AssertionBinaryOp::NonOverlappedImplication
        }
        SemanticOperation::AssertionOverlappedFollowedBy => AssertionBinaryOp::OverlappedFollowedBy,
        SemanticOperation::AssertionNonOverlappedFollowedBy => {
            AssertionBinaryOp::NonOverlappedFollowedBy
        }
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion binary node has a non-binary operation".into(),
            ))
        }
    })
}

fn assertion_repetition(node: &SemanticNode) -> Result<Option<AssertionRepetition>, DbError> {
    if node.auxiliary & SEMANTIC_ASSERTION_REPETITION == 0 {
        return Ok(None);
    }
    let kind = match node.assertion_repetition_kind {
        crate::ffi::slang::SEMANTIC_ASSERTION_REPEAT_CONSECUTIVE => {
            AssertionRepetitionKind::Consecutive
        }
        crate::ffi::slang::SEMANTIC_ASSERTION_REPEAT_NONCONSECUTIVE => {
            AssertionRepetitionKind::Nonconsecutive
        }
        crate::ffi::slang::SEMANTIC_ASSERTION_REPEAT_GOTO => AssertionRepetitionKind::GoTo,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion repetition kind is missing or invalid".into(),
            ))
        }
    };
    Ok(Some(AssertionRepetition {
        kind,
        range: AssertionRange {
            min: node.assertion_range_min,
            max: node.assertion_range_max,
        },
    }))
}

pub(super) fn assertion_expr_from_slang(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    let kind = match node.subkind {
        1 => AssertionExprKind::Invalid {
            child: first(SemanticEdgeRole::Body)?,
        },
        2 => AssertionExprKind::Simple {
            expr: required(SemanticEdgeRole::Operand, "simple assertion operand")?,
            repeated: node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0,
            repetition: assertion_repetition(node)?,
        },
        3 => {
            let mut operand_edges = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect::<Vec<_>>();
            operand_edges.sort_by_key(|edge| edge.index);
            let elements = operand_edges
                .iter()
                .map(|edge| semantic_id(ids, edge.target_id))
                .collect::<Result<Vec<_>, _>>()?;
            let delays = operand_edges
                .iter()
                .map(|edge| {
                    edge.sequence_delay
                        .map(|range| AssertionRange {
                            min: range.min,
                            max: range.max,
                        })
                        .unwrap_or(AssertionRange {
                            min: 0,
                            max: Some(0),
                        })
                })
                .collect();
            AssertionExprKind::SequenceConcat { elements, delays }
        }
        4 => AssertionExprKind::SequenceWithMatch {
            expr: required(SemanticEdgeRole::Body, "sequence match body")?,
            match_items: edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
            repeated: node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0,
            repetition: assertion_repetition(node)?,
        },
        5 => AssertionExprKind::Unary {
            op: assertion_unary_from_slang(node.operation)?,
            expr: required(SemanticEdgeRole::Body, "unary assertion body")?,
            ranged: node.auxiliary & SEMANTIC_ASSERTION_RANGE != 0,
            range: (node.auxiliary & SEMANTIC_ASSERTION_RANGE != 0).then_some(AssertionRange {
                min: node.assertion_range_min,
                max: node.assertion_range_max,
            }),
        },
        6 => AssertionExprKind::Binary {
            op: assertion_binary_from_slang(node.operation)?,
            left: required(SemanticEdgeRole::Left, "assertion binary left")?,
            right: required(SemanticEdgeRole::Right, "assertion binary right")?,
        },
        7 => AssertionExprKind::FirstMatch {
            sequence: required(SemanticEdgeRole::Body, "first_match sequence")?,
            match_items: edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
        },
        8 => {
            let control = required(SemanticEdgeRole::Clocking, "assertion clocking")?;
            let timing = snapshot
                .semantic_nodes
                .get(control.index())
                .ok_or_else(|| DbError::InvalidSnapshot("assertion clocking is missing".into()))?;
            let timing_edges = semantic_edges(snapshot, timing)?;
            let signal = edge_target(ids, timing_edges, SemanticEdgeRole::Event)?
                .ok_or_else(|| DbError::InvalidSnapshot("assertion clock has no signal".into()))?;
            AssertionExprKind::Clocking {
                control,
                signal,
                posedge: timing.is_posedge,
                expr: required(SemanticEdgeRole::Body, "clocked assertion body")?,
            }
        }
        9 => AssertionExprKind::StrongWeak {
            expr: required(SemanticEdgeRole::Body, "strong/weak assertion body")?,
            strong: node.auxiliary & SEMANTIC_ASSERTION_STRONG != 0,
        },
        10 => AssertionExprKind::Abort {
            condition: required(SemanticEdgeRole::Condition, "abort condition")?,
            expr: required(SemanticEdgeRole::Body, "abort assertion body")?,
            reject: node.auxiliary & SEMANTIC_ASSERTION_ABORT_REJECT != 0,
            sync: node.auxiliary & SEMANTIC_ASSERTION_ABORT_SYNC != 0,
        },
        11 => AssertionExprKind::Conditional {
            condition: required(SemanticEdgeRole::Condition, "assertion conditional")?,
            if_expr: required(SemanticEdgeRole::Then, "assertion conditional then")?,
            else_expr: first(SemanticEdgeRole::Else)?,
        },
        12 => {
            let expr = required(
                SemanticEdgeRole::CaseExpression,
                "assertion case expression",
            )?;
            let branch_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Branch)
                .map(|edge| edge.index)
                .max()
                .map_or(0, |index| index.saturating_add(1));
            let mut items = Vec::with_capacity(branch_count as usize);
            for item_index in 0..branch_count {
                let expressions = edges
                    .iter()
                    .filter(|edge| {
                        edge.role == SemanticEdgeRole::CaseItem && edge.index >> 16 == item_index
                    })
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?;
                let body = edges
                    .iter()
                    .find(|edge| edge.role == SemanticEdgeRole::Branch && edge.index == item_index)
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .transpose()?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("assertion case body is missing".into())
                    })?;
                items.push(AssertionCaseItem { expressions, body });
            }
            AssertionExprKind::Case {
                expr,
                items,
                default_case: first(SemanticEdgeRole::Else)?,
            }
        }
        13 => AssertionExprKind::DisableIff {
            condition: required(SemanticEdgeRole::Condition, "disable iff condition")?,
            expr: required(SemanticEdgeRole::Body, "disable iff assertion body")?,
        },
        _ => {
            return Err(DbError::InvalidSnapshot(
                "assertion expression has an unknown subkind".into(),
            ))
        }
    };
    Ok(NodeKind::AssertionExpr(kind))
}
