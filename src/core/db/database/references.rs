//! References.

use super::*;

pub(super) fn semantic_edges<'a>(
    snapshot: &'a SlangSnapshot,
    node: &SemanticNode,
) -> Result<&'a [crate::ffi::slang::SemanticEdge], DbError> {
    let start = usize::try_from(node.edge_start)
        .map_err(|_| DbError::InvalidSnapshot("semantic edge start is too large".to_owned()))?;
    let count = usize::try_from(node.edge_count)
        .map_err(|_| DbError::InvalidSnapshot("semantic edge count is too large".to_owned()))?;
    let end = start
        .checked_add(count)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic edge window overflowed".to_owned()))?;
    snapshot
        .semantic_edges
        .get(start..end)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic edge window is invalid".to_owned()))
}

pub(super) fn semantic_id(ids: &HashMap<u64, NodeId>, id: u64) -> Result<NodeId, DbError> {
    ids.get(&id)
        .copied()
        .ok_or_else(|| DbError::InvalidSnapshot(format!("unknown semantic node id {id}")))
}

pub(super) fn canonical_reference_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    target_id: u64,
) -> Result<NodeId, DbError> {
    let target = semantic_id(ids, target_id)?;
    let semantic = snapshot
        .semantic_nodes
        .get(target.index())
        .ok_or_else(|| DbError::InvalidSnapshot("semantic reference target is missing".into()))?;
    let edges = semantic_edges(snapshot, semantic)?;
    if let Some(owner) = edge_target(ids, edges, SemanticEdgeRole::ReturnOwner)? {
        return Ok(owner);
    }
    if semantic.kind == SemanticKind::Modport {
        if let Some(internal) = semantic
            .target_id
            .map(|id| semantic_id(ids, id))
            .transpose()?
        {
            return Ok(internal);
        }
    }
    Ok(target)
}

pub(super) fn edge_target(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Option<NodeId>, DbError> {
    edges
        .iter()
        .find(|edge| edge.role == role)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()
}

pub(super) fn edge_target_at(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
    index: u32,
) -> Result<Option<NodeId>, DbError> {
    edges
        .iter()
        .find(|edge| edge.role == role && edge.index == index)
        .map(|edge| semantic_id(ids, edge.target_id))
        .transpose()
}

pub(super) fn edge_targets(
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Vec<NodeId>, DbError> {
    edges
        .iter()
        .filter(|edge| edge.role == role)
        .map(|edge| semantic_id(ids, edge.target_id))
        .collect()
}

pub(super) fn resolved_edge_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Option<NodeId>, DbError> {
    let Some(edge) = edges.iter().find(|edge| edge.role == role) else {
        return Ok(None);
    };
    let semantic =
        snapshot
            .semantic_nodes
            .get(usize::try_from(edge.target_id).map_err(|_| {
                DbError::InvalidSnapshot("semantic edge target is too large".into())
            })?)
            .ok_or_else(|| DbError::InvalidSnapshot("semantic edge target is missing".into()))?;
    semantic
        .target_id
        .map(|id| semantic_id(ids, id))
        .transpose()?
        .map_or_else(
            || {
                matches!(
                    semantic.kind,
                    SemanticKind::Instance
                        | SemanticKind::Port
                        | SemanticKind::Net
                        | SemanticKind::Variable
                        | SemanticKind::Array
                        | SemanticKind::NamedEvent
                        | SemanticKind::Parameter
                        | SemanticKind::Subroutine
                        | SemanticKind::EnumConstant
                        | SemanticKind::InterfaceConnection
                )
                .then(|| semantic_id(ids, semantic.id))
                .transpose()
            },
            |target| Ok(Some(target)),
        )
}

fn is_array_semantic(snapshot: &SlangSnapshot, id: NodeId) -> bool {
    let Some(type_id) = snapshot
        .semantic_nodes
        .get(id.index())
        .and_then(|node| node.type_id)
    else {
        return false;
    };
    snapshot
        .types
        .iter()
        .find(|ty| ty.id == type_id)
        .is_some_and(|ty| {
            matches!(
                ty.kind,
                crate::ffi::slang::TypeKind::FixedUnpackedArray
                    | crate::ffi::slang::TypeKind::DynamicArray
                    | crate::ffi::slang::TypeKind::AssociativeArray
                    | crate::ffi::slang::TypeKind::Queue
            )
        })
}

pub(super) fn array_select_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
    depth: usize,
) -> Result<Option<(NodeId, Vec<NodeId>)>, DbError> {
    if depth > snapshot.semantic_nodes.len() || node.subkind != 73 {
        return Ok(None);
    }
    let edges = semantic_edges(snapshot, node)?;
    let index = edge_target(ids, edges, SemanticEdgeRole::Index)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select index is missing".into()))?;
    let raw_base = edge_target(ids, edges, SemanticEdgeRole::Base)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select base is missing".into()))?;
    let base_semantic = &snapshot.semantic_nodes[raw_base.index()];
    if let Some((base, mut indices)) =
        array_select_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
    {
        indices.push(index);
        return Ok(Some((base, indices)));
    }
    let base = resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::Base)?
        .ok_or_else(|| DbError::InvalidSnapshot("element select base is missing".into()))?;
    if is_array_semantic(snapshot, base) {
        return Ok(Some((base, vec![index])));
    }
    let multidimensional_packed = base_semantic
        .type_id
        .map(|type_id| type_projector.project(type_id))
        .transpose()?
        .is_some_and(|projection| projection.packed_dimensions.len() > 1);
    Ok(multidimensional_packed.then(|| (raw_base, vec![index])))
}

pub(super) type SemanticMemberPath = (Vec<String>, Vec<Option<NodeId>>);

pub(super) fn member_path_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
    depth: usize,
) -> Result<Option<SemanticMemberPath>, DbError> {
    if depth > snapshot.semantic_nodes.len() {
        return Err(DbError::InvalidSnapshot(
            "member access chain contains a cycle".into(),
        ));
    }
    let edges = semantic_edges(snapshot, node)?;
    let Some(base) = edge_target(ids, edges, SemanticEdgeRole::Base)? else {
        return Ok(None);
    };
    let base_semantic = &snapshot.semantic_nodes[base.index()];
    let (mut parts, mut refs) =
        if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 75 {
            let Some(path) =
                member_path_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
            else {
                return Ok(None);
            };
            path
        } else if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 65 {
            let Some(target) = expression_reference_target(snapshot, ids, base_semantic)? else {
                return Ok(None);
            };
            let target_semantic = &snapshot.semantic_nodes[target.index()];
            (vec![target_semantic.name.clone()], vec![Some(target)])
        } else if base_semantic.kind == SemanticKind::Expression && base_semantic.subkind == 73 {
            // A member access on an unpacked virtual-interface array is captured
            // as `ArraySelect` followed by `MemberAccess`. Keep the element-select
            // node as the base reference so lowering can retrieve the runtime
            // handle from the container instead of collapsing it to the array
            // declaration.
            let Some((array, _indices)) =
                array_select_from_slang(snapshot, type_projector, ids, base_semantic, depth + 1)?
            else {
                return Ok(None);
            };
            let Some(name) = snapshot
                .semantic_nodes
                .get(array.index())
                .map(|array| array.name.clone())
                .filter(|name| !name.is_empty())
            else {
                return Ok(None);
            };
            (vec![name], vec![Some(semantic_id(ids, base_semantic.id)?)])
        } else {
            return Ok(None);
        };
    let member = node
        .target_id
        .map(|id| canonical_reference_target(snapshot, ids, id))
        .transpose()?;
    let member_name = member
        .and_then(|id| snapshot.semantic_nodes.get(id.index()))
        .map(|member| member.name.clone())
        .filter(|name| !name.is_empty())
        .or_else(|| (!node.name.is_empty()).then(|| node.name.clone()));
    let Some(member_name) = member_name else {
        return Ok(None);
    };
    parts.push(member_name);
    refs.push(member);
    Ok(Some((parts, refs)))
}

pub(super) fn expression_reference_target(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    node: &SemanticNode,
) -> Result<Option<NodeId>, DbError> {
    if let Some(target) = node.target_id {
        return canonical_reference_target(snapshot, ids, target).map(Some);
    }
    let edges = semantic_edges(snapshot, node)?;
    edge_target(ids, edges, SemanticEdgeRole::Reference)
}
