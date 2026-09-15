//! Expression import.

use super::*;

fn source_spelling(snapshot: &SlangSnapshot, node: &SemanticNode) -> Option<String> {
    // The bridge exports the expanded token for time literals. Never guess a
    // macro's replacement by scanning raw source: inactive branches, undef,
    // includes, and function-like macros make that semantically incorrect.
    if node.kind == SemanticKind::Expression && node.subkind == 85 && !node.name.is_empty() {
        return Some(node.name.clone());
    }
    let range = node.range?;
    let file = snapshot
        .files
        .iter()
        .find(|file| file.id == range.file_id)?;
    let start = usize::try_from(range.start).ok()?;
    let end = usize::try_from(range.end).ok()?;
    let spelling = file.text.get(start..end)?;
    (!spelling.is_empty()).then(|| spelling.to_owned())
}

pub(super) fn expression_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
    ty: TypeInfo,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    if node.detail == "ArbitrarySymbol" {
        return Ok(NodeKind::Expr(ExprKind::ScopeRef {
            target: required(SemanticEdgeRole::Reference, "scope reference target")?,
        }));
    }
    if node.detail == "DataType" {
        return Ok(NodeKind::Expr(ExprKind::DataType));
    }
    if node.detail == "UnboundedLiteral" {
        return Ok(NodeKind::Expr(ExprKind::Unbounded));
    }
    Ok(NodeKind::Expr(match node.subkind {
        64 | 85 => {
            let value = node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .map(|constant| value_data_from_slang(&constant.value))
                .unwrap_or(ValueData::None);
            let (size, const_type) = match &value {
                ValueData::Vector { bit_width, .. } => (
                    i32::try_from(*bit_width).unwrap_or(i32::MAX),
                    ConstantType::Integer,
                ),
                ValueData::Real(_) if node.subkind == 85 => (64, ConstantType::Time),
                ValueData::Real(_) => (64, ConstantType::Real),
                ValueData::Bytes(_) | ValueData::Str(_) => (-1, ConstantType::String),
                _ => (-1, ConstantType::Null),
            };
            ExprKind::Constant {
                value,
                size,
                const_type,
                source: source_spelling(snapshot, node)
                    .map(ConstantSource::Exact)
                    .unwrap_or(ConstantSource::Unavailable),
                time_scale: (node.subkind == 85)
                    .then(|| time_literal_scale(node.time_scale))
                    .flatten(),
            }
        }
        65 => ExprKind::Ref {
            target: node
                .target_id
                .map(|id| canonical_reference_target(snapshot, ids, id))
                .transpose()?,
        },
        72 => ExprKind::Cast {
            operand: required(SemanticEdgeRole::Operand, "conversion operand")?,
            ty: ty.clone(),
            size_cast: false,
            size_cast_expr: None,
            cast_kind_known: true,
            propagated: node.is_propagated_conversion,
            two_state: snapshot
                .types
                .iter()
                .find(|candidate| Some(candidate.id) == node.type_id)
                .is_some_and(|candidate| !candidate.is_four_state),
        },
        73 => {
            let index = required(SemanticEdgeRole::Index, "element select index")?;
            if let Some((base, indices)) =
                array_select_from_slang(snapshot, type_projector, ids, node, 0)?
            {
                ExprKind::ArraySelect { base, indices }
            } else {
                ExprKind::BitSelect {
                    base: required(SemanticEdgeRole::Base, "element select base")?,
                    index,
                }
            }
        }
        74 if node.is_indexed_up || node.is_indexed_down => ExprKind::IndexedPartSelect {
            base: required(SemanticEdgeRole::Base, "indexed select base")?,
            base_expr: required(SemanticEdgeRole::Left, "indexed select base expression")?,
            width_expr: required(SemanticEdgeRole::Right, "indexed select width")?,
            neg: node.is_indexed_down,
        },
        74 => ExprKind::PartSelect {
            base: required(SemanticEdgeRole::Base, "range select base")?,
            left: required(SemanticEdgeRole::Left, "range select left bound")?,
            right: required(SemanticEdgeRole::Right, "range select right bound")?,
        },
        75 => match member_path_from_slang(snapshot, type_projector, ids, node, 0)? {
            Some((parts, refs)) => ExprKind::HierPath { parts, refs },
            None => ExprKind::Other,
        },
        86 => ExprKind::NewArray {
            size: required(SemanticEdgeRole::Width, "dynamic-array size")?,
            initializer: first(SemanticEdgeRole::Initializer)?,
        },
        87 => ExprKind::NewClass {
            class_name: ty.type_name.clone(),
            class_type: node.type_id.map(TypeId),
            constructor: first(SemanticEdgeRole::Initializer)?,
            is_super_class: node.auxiliary & crate::ffi::slang::NEW_CLASS_SUPER != 0,
        },
        90 => {
            let target = node
                .target_id
                .map(|id| semantic_id(ids, id))
                .transpose()?
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("assertion instance has no target".into())
                })?;
            let body = required(SemanticEdgeRole::Body, "assertion instance body")?;
            let mut formals = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::AssertionFormal)
                .collect::<Vec<_>>();
            let mut actuals = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::AssertionActual)
                .collect::<Vec<_>>();
            formals.sort_by_key(|edge| edge.index);
            actuals.sort_by_key(|edge| edge.index);
            if formals.len() != actuals.len()
                || formals
                    .iter()
                    .zip(&actuals)
                    .any(|(formal, actual)| formal.index != actual.index)
            {
                return Err(DbError::InvalidSnapshot(
                    "assertion instance formal/actual bindings are not paired".into(),
                ));
            }
            let bindings = formals
                .into_iter()
                .zip(actuals)
                .map(|(formal, actual)| {
                    Ok(AssertionBinding {
                        formal: semantic_id(ids, formal.target_id)?,
                        actual: semantic_id(ids, actual.target_id)?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            ExprKind::AssertionInstance {
                target,
                body,
                bindings,
            }
        }
        SEMANTIC_EXPR_CLOCKING_EVENT => {
            let control = required(SemanticEdgeRole::Operand, "clocking event control")?;
            let timing = snapshot
                .semantic_nodes
                .get(control.index())
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("clocking event control is missing".into())
                })?;
            if timing.kind != SemanticKind::TimingControl
                || timing.subkind != 113
                || (!timing.is_posedge && !timing.is_negedge)
            {
                ExprKind::Other
            } else {
                let timing_edges = semantic_edges(snapshot, timing)?;
                let signal =
                    edge_target(ids, timing_edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking event has no signal".into())
                    })?;
                if is_named_event_expression(snapshot, ids, signal)? {
                    ExprKind::Other
                } else {
                    let gate = edge_target(ids, timing_edges, SemanticEdgeRole::Condition)?;
                    ExprKind::ClockingEvent {
                        signal,
                        posedge: timing.is_posedge,
                        gate,
                    }
                }
            }
        }
        81..=84 => {
            let key_type = if node.subkind == 82 {
                let type_id = node.type_id.ok_or_else(|| {
                    DbError::InvalidSnapshot("assignment pattern type key has no type".into())
                })?;
                let projection = type_projector.project(type_id)?;
                Some(AssignmentPatternKeyType {
                    type_id: projection.descriptor.id,
                    ty: projection.type_info,
                    two_state: projection.two_state,
                    packed_ranges: projection.packed_dimensions,
                })
            } else {
                None
            };
            ExprKind::TaggedPattern {
                key: (!node.name.is_empty()).then(|| node.name.clone()),
                key_type,
                value: first(SemanticEdgeRole::Body)?,
            }
        }
        69 if matches!(
            node.operation,
            SemanticOperation::StreamLeft | SemanticOperation::StreamRight
        ) =>
        {
            let mut operand_edges = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect::<Vec<_>>();
            operand_edges.sort_by_key(|edge| edge.index);
            let operand_indices = operand_edges
                .iter()
                .map(|edge| edge.index)
                .collect::<HashSet<_>>();
            let streams = operand_edges
                .into_iter()
                .map(|operand| {
                    let value = semantic_id(ids, operand.target_id)?;
                    let with_expr = edges
                        .iter()
                        .find(|edge| {
                            edge.role == SemanticEdgeRole::Index && edge.index == operand.index
                        })
                        .map(|edge| semantic_id(ids, edge.target_id))
                        .transpose()?;
                    Ok(StreamOperand { value, with_expr })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            if edges.iter().any(|edge| {
                edge.role == SemanticEdgeRole::Index && !operand_indices.contains(&edge.index)
            }) {
                return Err(DbError::InvalidSnapshot(
                    "streaming selector has no matching operand".into(),
                ));
            }
            ExprKind::Streaming {
                direction: if node.operation == SemanticOperation::StreamLeft {
                    StreamingDirection::LeftToRight
                } else {
                    StreamingDirection::RightToLeft
                },
                slice_size: node.auxiliary,
                streams,
            }
        }
        _ if node.operation != SemanticOperation::None => {
            let operands = match node.subkind {
                66 => edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
                67 => [SemanticEdgeRole::Left, SemanticEdgeRole::Right]
                    .into_iter()
                    .map(|role| required(role, "binary operand"))
                    .collect::<Result<Vec<_>, _>>()?,
                68 => {
                    let mut values = edge_targets(ids, edges, SemanticEdgeRole::Condition)?;
                    values.push(required(
                        SemanticEdgeRole::Then,
                        "conditional true operand",
                    )?);
                    values.push(required(
                        SemanticEdgeRole::Else,
                        "conditional false operand",
                    )?);
                    values
                }
                70 => {
                    let mut values = vec![required(SemanticEdgeRole::Width, "replication count")?];
                    values.extend(edge_targets(ids, edges, SemanticEdgeRole::Operand)?);
                    values
                }
                71 => [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| required(role, "assignment operand"))
                    .collect::<Result<Vec<_>, _>>()?,
                77 => {
                    let mut values = vec![required(SemanticEdgeRole::Lhs, "inside selector")?];
                    values.extend(edge_targets(ids, edges, SemanticEdgeRole::Operand)?);
                    values
                }
                89 => [SemanticEdgeRole::Left, SemanticEdgeRole::Right]
                    .into_iter()
                    .map(|role| required(role, "value range bound"))
                    .collect::<Result<Vec<_>, _>>()?,
                _ => edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
            };
            ExprKind::Operation {
                op: operation_from_slang(node.operation, node.subkind == 66),
                reordered: false,
                assignment: node.subkind == 71,
                operands,
            }
        }
        _ => ExprKind::Other,
    }))
}

pub(super) fn source_position(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
) -> Result<(Option<String>, u32, u32, u32, u32), DbError> {
    let Some(range) = node.range else {
        return Ok((None, 0, 0, 0, 0));
    };
    let file = snapshot
        .files
        .iter()
        .find(|file| file.id == range.file_id)
        .ok_or_else(|| DbError::InvalidSnapshot("semantic range file is missing".into()))?;
    let start = usize::try_from(range.start)
        .map_err(|_| DbError::InvalidSnapshot("semantic range start is too large".into()))?;
    let end = usize::try_from(range.end)
        .map_err(|_| DbError::InvalidSnapshot("semantic range end is too large".into()))?;
    let (line, col) = line_column(&file.text, start)?;
    let (end_line, end_col) = line_column(&file.text, end)?;
    Ok((Some(file.name.clone()), line, col, end_line, end_col))
}

fn line_column(text: &str, offset: usize) -> Result<(u32, u32), DbError> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err(DbError::InvalidSnapshot(
            "semantic range is not on a source character boundary".into(),
        ));
    }
    let prefix = &text[..offset];
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)
        .map_err(|_| DbError::InvalidSnapshot("source line number is too large".into()))?;
    let column = u32::try_from(offset - line_start + 1)
        .map_err(|_| DbError::InvalidSnapshot("source column is too large".into()))?;
    Ok((line, column))
}

pub(super) fn semantic_full_name(nodes: &[Node], id: NodeId) -> Result<String, DbError> {
    let mut parts = Vec::new();
    let mut current = Some(id);
    let mut visited = HashSet::new();
    while let Some(node_id) = current {
        if !visited.insert(node_id) {
            return Err(DbError::InvalidSnapshot(
                "Slang semantic parent links contain a cycle".into(),
            ));
        }
        let node = nodes.get(node_id.index()).ok_or_else(|| {
            DbError::InvalidSnapshot("Slang semantic parent is outside the node arena".into())
        })?;
        if !node.name.is_empty() {
            parts.push(node.name.as_str());
        }
        current = node.parent;
    }
    parts.reverse();
    Ok(parts.join("."))
}

pub(super) fn enclosing_scope_name(nodes: &[Node], id: NodeId) -> Option<String> {
    let parent = nodes.get(id.index())?.parent?;
    let full_name = &nodes.get(parent.index())?.full_name;
    (!full_name.is_empty()).then(|| full_name.clone())
}
