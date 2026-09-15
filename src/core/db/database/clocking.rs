//! Clocking.

use super::*;

pub(super) fn virtual_interface_instance_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    variable: NodeId,
) -> Result<Option<NodeId>, DbError> {
    let variable_node = snapshot
        .semantic_nodes
        .get(variable.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface variable is missing".into()))?;
    if variable_node.kind != SemanticKind::Variable {
        return Ok(None);
    }
    let initializer = semantic_edges(snapshot, variable_node)?
        .iter()
        .find(|edge| edge.role == SemanticEdgeRole::Initializer)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()?;
    let Some(initializer) = initializer else {
        return Ok(None);
    };
    let Some(target) = expression_reference_target(
        snapshot,
        ids,
        snapshot
            .semantic_nodes
            .get(initializer.index())
            .ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface initializer is missing".into())
            })?,
    )?
    else {
        return Ok(None);
    };
    let target_node = snapshot
        .semantic_nodes
        .get(target.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface target is missing".into()))?;
    if target_node.kind != SemanticKind::Instance {
        return Ok(None);
    }
    let Some(definition) = target_node.target_id else {
        return Ok(None);
    };
    let definition = snapshot
        .semantic_nodes
        .get(definition as usize)
        .ok_or_else(|| {
            DbError::InvalidSnapshot("virtual interface definition is missing".into())
        })?;
    if definition.kind == SemanticKind::Definition
        && definition.definition_kind == Some(crate::ffi::slang::SemanticDefinitionKind::Interface)
    {
        return Ok(Some(target));
    }
    Ok(None)
}

fn find_clocking_member_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    owner: NodeId,
    name: &str,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "virtual interface member path contains a cycle".into(),
        ));
    }
    let owner_node = snapshot
        .semantic_nodes
        .get(owner.index())
        .ok_or_else(|| DbError::InvalidSnapshot("virtual interface owner is missing".into()))?;
    for edge in semantic_edges(snapshot, owner_node)?
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Child)
    {
        let child = semantic_id(ids, edge.target_id)?;
        let child_node = snapshot.semantic_nodes.get(child.index()).ok_or_else(|| {
            DbError::InvalidSnapshot("virtual interface member is missing".into())
        })?;
        if child_node.name == name
            && ((child_node.kind == SemanticKind::Scope
                && child_node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK)
                || (child_node.kind == SemanticKind::Variable
                    && child_node.subkind == SEMANTIC_VARIABLE_CLOCKING))
        {
            return Ok(Some(child));
        }
        if matches!(
            child_node.kind,
            SemanticKind::Instance | SemanticKind::Scope
        ) {
            if let Some(found) =
                find_clocking_member_from_slang(snapshot, ids, child, name, depth + 1)?
            {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

pub(super) fn clocking_block_from_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "clocking expression contains a cycle".into(),
        ));
    }
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking expression is missing".into()))?;
    if node.kind == SemanticKind::Scope && node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK {
        return Ok(Some(expression));
    }
    if node.kind != SemanticKind::Expression {
        return Ok(None);
    }
    if let Some(target) = expression_reference_target(snapshot, ids, node)? {
        let target_node = snapshot
            .semantic_nodes
            .get(target.index())
            .ok_or_else(|| DbError::InvalidSnapshot("clocking target is missing".into()))?;
        if target_node.kind == SemanticKind::Scope
            && target_node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
        {
            return Ok(Some(target));
        }
    }
    if node.subkind == 75 {
        let base = edge_target(ids, semantic_edges(snapshot, node)?, SemanticEdgeRole::Base)?;
        if let Some(base) = base {
            let base_node = snapshot.semantic_nodes.get(base.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface base is missing".into())
            })?;
            if let Some(variable) = expression_reference_target(snapshot, ids, base_node)? {
                if let Some(interface) =
                    virtual_interface_instance_from_slang(snapshot, ids, variable)?
                {
                    return find_clocking_member_from_slang(
                        snapshot,
                        ids,
                        interface,
                        &node.name,
                        depth + 1,
                    );
                }
            }
        }
    }
    let edges = semantic_edges(snapshot, node)?;
    let role = match node.subkind {
        72 => SemanticEdgeRole::Operand,
        73..=75 => SemanticEdgeRole::Base,
        _ => return Ok(None),
    };
    edge_target(ids, edges, role)?
        .map(|base| clocking_block_from_expression(snapshot, ids, base, depth + 1))
        .transpose()
        .map(|value| value.flatten())
}

pub(super) fn clocking_source_from_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
    depth: usize,
) -> Result<Option<NodeId>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "clocking source expression contains a cycle".into(),
        ));
    }
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking source expression is missing".into()))?;
    if let Some(target) = expression_reference_target(snapshot, ids, node)? {
        let target_node = snapshot
            .semantic_nodes
            .get(target.index())
            .ok_or_else(|| DbError::InvalidSnapshot("clocking source target is missing".into()))?;
        if matches!(
            target_node.kind,
            SemanticKind::Net
                | SemanticKind::Variable
                | SemanticKind::Port
                | SemanticKind::Array
                | SemanticKind::NamedEvent
        ) {
            return Ok(Some(target));
        }
    }
    if node.kind != SemanticKind::Expression {
        return Ok(None);
    }
    if node.subkind == 75 {
        let base = edge_target(ids, semantic_edges(snapshot, node)?, SemanticEdgeRole::Base)?;
        if let Some(base) = base {
            let base_node = snapshot.semantic_nodes.get(base.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("virtual interface base is missing".into())
            })?;
            if let Some(variable) = expression_reference_target(snapshot, ids, base_node)? {
                if let Some(interface) =
                    virtual_interface_instance_from_slang(snapshot, ids, variable)?
                {
                    return find_clocking_member_from_slang(
                        snapshot,
                        ids,
                        interface,
                        &node.name,
                        depth + 1,
                    );
                }
            }
        }
    }
    let edges = semantic_edges(snapshot, node)?;
    let role = match node.subkind {
        72 => SemanticEdgeRole::Operand,
        73..=75 => SemanticEdgeRole::Base,
        _ => return Ok(None),
    };
    edge_target(ids, edges, role)?
        .map(|base| clocking_source_from_expression(snapshot, ids, base, depth + 1))
        .transpose()
        .map(|value| value.flatten())
}

fn clocking_edge(code: u64) -> Result<ClockingEdge, DbError> {
    match code & CLOCKING_EDGE_MASK {
        0 => Ok(ClockingEdge::None),
        1 => Ok(ClockingEdge::Posedge),
        2 => Ok(ClockingEdge::Negedge),
        3 => Ok(ClockingEdge::BothEdges),
        _ => unreachable!("clocking edge mask has four values"),
    }
}

fn clocking_delay_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    delay: Option<NodeId>,
) -> Result<Option<NodeId>, DbError> {
    let Some(delay) = delay else {
        return Ok(None);
    };
    let timing = snapshot
        .semantic_nodes
        .get(delay.index())
        .ok_or_else(|| DbError::InvalidSnapshot("clocking skew timing is missing".into()))?;
    if timing.subkind == 112 {
        return edge_target_at(
            ids,
            semantic_edges(snapshot, timing)?,
            SemanticEdgeRole::Delay,
            0,
        );
    }
    if timing.subkind == SEMANTIC_TIMING_ONE_STEP_DELAY {
        return Ok(None);
    }
    Err(DbError::InvalidSnapshot(format!(
        "unsupported clocking skew timing subkind {}",
        timing.subkind
    )))
}

pub(super) fn clocking_skew_from_slang(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    delay: Option<NodeId>,
    edge: u64,
) -> Result<ClockingSkew, DbError> {
    Ok(ClockingSkew {
        edge: clocking_edge(edge)?,
        delay,
        delay_expression: clocking_delay_expression(snapshot, ids, delay)?,
    })
}
