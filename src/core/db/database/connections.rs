//! Connections.

use super::*;

pub(super) fn peel_gate_terminal(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    mut expression: NodeId,
    direction: Direction,
) -> Result<NodeId, DbError> {
    let mut visited = HashSet::new();
    while visited.insert(expression) {
        let semantic = snapshot
            .semantic_nodes
            .get(expression.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("primitive terminal expression is missing".into())
            })?;
        if semantic.kind != SemanticKind::Expression {
            return Ok(expression);
        }
        let edges = semantic_edges(snapshot, semantic)?;
        if semantic.subkind == 72 && semantic.is_implicit_conversion {
            expression = edge_target(ids, edges, SemanticEdgeRole::Operand)?.ok_or_else(|| {
                DbError::InvalidSnapshot(
                    "implicit primitive terminal conversion has no operand".into(),
                )
            })?;
        } else if semantic.subkind == 71
            && matches!(direction, Direction::Output | Direction::Inout)
        {
            expression = edge_target(ids, edges, SemanticEdgeRole::Lhs)?.ok_or_else(|| {
                DbError::InvalidSnapshot("primitive output terminal has no lvalue".into())
            })?;
        } else {
            return Ok(expression);
        }
    }
    Err(DbError::InvalidSnapshot(
        "primitive terminal conversions contain a cycle".into(),
    ))
}

pub(super) fn connection_source_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    mut expression: NodeId,
    direction: Direction,
) -> Result<NodeId, DbError> {
    let mut visited = HashSet::new();
    while visited.insert(expression) {
        let semantic = snapshot
            .semantic_nodes
            .get(expression.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("port connection expression is missing".into())
            })?;
        if semantic.kind != SemanticKind::Expression {
            return Ok(expression);
        }
        let edges = semantic_edges(snapshot, semantic)?;
        if semantic.subkind == 71 && matches!(direction, Direction::Output | Direction::Inout) {
            expression = edge_target(ids, edges, SemanticEdgeRole::Lhs)?.ok_or_else(|| {
                DbError::InvalidSnapshot("output port assignment wrapper has no lhs".into())
            })?;
            continue;
        }
        if semantic.subkind == 72 && semantic.is_implicit_conversion {
            expression = edge_target(ids, edges, SemanticEdgeRole::Operand)?.ok_or_else(|| {
                DbError::InvalidSnapshot("implicit port conversion has no operand".into())
            })?;
            continue;
        }
        return Ok(expression);
    }
    Err(DbError::InvalidSnapshot(
        "implicit port conversions contain a cycle".into(),
    ))
}

pub(super) fn direction_from_slang(node: &SemanticNode) -> Direction {
    if node.is_input {
        Direction::Input
    } else if node.is_output {
        Direction::Output
    } else if node.is_inout {
        Direction::Inout
    } else if node.is_ref {
        Direction::Ref
    } else {
        Direction::None
    }
}

pub(super) fn driver_delay(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
) -> Result<Option<DriverDelay>, DbError> {
    let Some(delay) = edge_target(ids, edges, SemanticEdgeRole::Delay)? else {
        return Ok(None);
    };
    let semantic = snapshot
        .semantic_nodes
        .get(delay.index())
        .ok_or_else(|| DbError::InvalidSnapshot("delay semantic node is missing".into()))?;
    if semantic.kind != SemanticKind::TimingControl {
        return Ok(Some(DriverDelay::Single(delay)));
    }
    let expressions = edge_targets(
        ids,
        semantic_edges(snapshot, semantic)?,
        SemanticEdgeRole::Delay,
    )?;
    Ok(Some(match expressions.as_slice() {
        [delay] => DriverDelay::Single(*delay),
        [rise, fall] => DriverDelay::RiseFall(*rise, *fall),
        [rise, fall, turn_off] => DriverDelay::RiseFallTurnOff(*rise, *fall, *turn_off),
        _ => {
            let location = source_position(snapshot, semantic)
                .ok()
                .and_then(|(file, line, col, _, _)| {
                    file.map(|file| format!(" at {file}:{line}:{col}"))
                })
                .unwrap_or_default();
            return Err(DbError::InvalidSnapshot(format!(
                "driver timing control{location} requires one to three delay expressions"
            )));
        }
    }))
}
