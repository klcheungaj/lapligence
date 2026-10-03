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
    ids: &SemanticIds,
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
    // A comparison of types has no runtime operands. Slang evaluates it
    // during elaboration and retains the result on the comparison node;
    // import that value while keeping its children for source navigation.
    if node.subkind == 67
        && node.constant_id.is_some()
        && edges.iter().any(|edge| {
            matches!(edge.role, SemanticEdgeRole::Left | SemanticEdgeRole::Right)
                && usize::try_from(edge.target_id)
                    .ok()
                    .and_then(|index| snapshot.semantic_nodes.get(index))
                    .is_some_and(|child| child.detail == "TypeReference")
        })
    {
        let value = node
            .constant_id
            .and_then(|id| snapshot.constants.get(id as usize))
            .map(|constant| value_data_from_slang(&constant.value))
            .ok_or_else(|| {
                DbError::InvalidSnapshot("type comparison constant is missing".into())
            })?;
        let (size, const_type) = match &value {
            ValueData::Vector { bit_width, .. } => (
                i32::try_from(*bit_width).unwrap_or(i32::MAX),
                ConstantType::Integer,
            ),
            _ => {
                return Err(DbError::InvalidSnapshot(
                    "type comparison is not integral".into(),
                ))
            }
        };
        return Ok(NodeKind::Expr(ExprKind::Constant {
            value,
            size,
            const_type,
            source: ConstantSource::NotCaptured,
            time_scale: None,
        }));
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
                .map(|id| {
                    if node.detail == "HierarchicalValue" {
                        hierarchical_reference_target(snapshot, ids, id)
                    } else {
                        canonical_reference_target(snapshot, ids, id)
                    }
                })
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
                    descriptor: projection.descriptor,
                    ty: projection.type_info,
                    two_state: projection.two_state,
                    packed_ranges: projection.packed_dimensions,
                })
            } else {
                None
            };
            let index_key = first(SemanticEdgeRole::Index)?;
            let index_value = index_key
                .and_then(|index| snapshot.semantic_nodes.get(index.index()))
                .and_then(|index| index.constant_id)
                .and_then(|constant| snapshot.constants.get(constant as usize))
                .map(|constant| value_data_from_slang(&constant.value));
            ExprKind::TaggedPattern {
                key: (!node.name.is_empty()).then(|| node.name.clone()),
                key_type,
                index_key,
                index_value,
                value: first(SemanticEdgeRole::Body)?,
            }
        }
        SEMANTIC_EXPR_TAGGED_UNION => ExprKind::TaggedUnion {
            member: node.name.clone(),
            value: first(SemanticEdgeRole::Body)?,
        },
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
        68 => {
            let predicate = predicate_from_slang(edges, ids)?;
            let (if_true, if_false) = conditional_branches_from_slang(edges, ids, true)?;
            let if_false = if_false.ok_or_else(|| {
                DbError::InvalidSnapshot("conditional false operand is missing".into())
            })?;
            if predicate.clauses.len() == 1 && !predicate.has_patterns() {
                ExprKind::Operation {
                    op: Operation::Conditional,
                    reordered: false,
                    assignment: false,
                    operands: vec![predicate.clauses[0].expression, if_true, if_false],
                }
            } else {
                ExprKind::Conditional {
                    predicate,
                    if_true,
                    if_false,
                }
            }
        }
        _ if node.operation != SemanticOperation::None => {
            let operands = match node.subkind {
                66 => edge_targets(ids, edges, SemanticEdgeRole::Operand)?,
                67 => [SemanticEdgeRole::Left, SemanticEdgeRole::Right]
                    .into_iter()
                    .map(|role| required(role, "binary operand"))
                    .collect::<Result<Vec<_>, _>>()?,
                70 => {
                    let mut values = vec![required(SemanticEdgeRole::Width, "replication count")?];
                    values.extend(edge_targets(ids, edges, SemanticEdgeRole::Operand)?);
                    values
                }
                78 if node.operation == SemanticOperation::MultiAssignmentPattern => {
                    // Replicated assignment patterns carry the count in a
                    // width edge just like ordinary concatenation
                    // replication. Keep it in operand position zero so
                    // fixed-array consumers can expand the typed pattern
                    // without losing the count or the repeated edge order.
                    let mut values = vec![required(
                        SemanticEdgeRole::Width,
                        "assignment pattern count",
                    )?];
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

// Bound scan work and indexing memory; template offsets use a bounded cache.
const SOURCE_POSITION_CHECKPOINT_BYTES: usize = 4096;
const SOURCE_POSITION_CACHE_ENTRIES: usize = 256;

#[cfg(test)]
thread_local! {
    static SOURCE_INDEX_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_QUERY_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Copy)]
struct SourceCheckpoint {
    offset: usize,
    line: usize,
    line_start: usize,
}

#[derive(Clone, Copy)]
struct CachedPosition {
    offset: usize,
    line: u32,
    column: u32,
}

struct SourceLines<'a> {
    text: &'a str,
    checkpoints: Vec<SourceCheckpoint>,
    positions: Box<[Option<CachedPosition>]>,
}

impl<'a> SourceLines<'a> {
    fn new(text: &'a str) -> Self {
        let mut checkpoints = Vec::with_capacity(text.len() / SOURCE_POSITION_CHECKPOINT_BYTES + 1);
        let mut line = 1;
        let mut line_start = 0;
        for (offset, character) in text.char_indices() {
            #[cfg(test)]
            SOURCE_INDEX_BYTES.with(|work| work.set(work.get() + character.len_utf8()));
            // Following a UTF-8 boundary is safe: earlier offsets in this bucket
            // are inside the preceding character and cannot be queried.
            while checkpoints.len() * SOURCE_POSITION_CHECKPOINT_BYTES <= offset {
                checkpoints.push(SourceCheckpoint {
                    offset,
                    line,
                    line_start,
                });
            }
            if character == '\n' {
                line += 1;
                line_start = offset + 1;
            }
        }
        while checkpoints.len() * SOURCE_POSITION_CHECKPOINT_BYTES <= text.len() {
            checkpoints.push(SourceCheckpoint {
                offset: text.len(),
                line,
                line_start,
            });
        }
        Self {
            text,
            checkpoints,
            positions: vec![None; SOURCE_POSITION_CACHE_ENTRIES].into_boxed_slice(),
        }
    }

    fn position(&mut self, offset: usize) -> Result<(u32, u32), DbError> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return Err(DbError::InvalidSnapshot(
                "semantic range is not on a source character boundary".into(),
            ));
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        offset.hash(&mut hasher);
        let slot = hasher.finish() as usize % SOURCE_POSITION_CACHE_ENTRIES;
        if let Some(position) = self.positions[slot].filter(|position| position.offset == offset) {
            return Ok((position.line, position.column));
        }
        let checkpoint = self.checkpoints[offset / SOURCE_POSITION_CHECKPOINT_BYTES];
        let prefix = &self.text[checkpoint.offset..offset];
        #[cfg(test)]
        SOURCE_QUERY_BYTES.with(|work| work.set(work.get() + prefix.len()));
        let line =
            u32::try_from(checkpoint.line + prefix.bytes().filter(|byte| *byte == b'\n').count())
                .map_err(|_| DbError::InvalidSnapshot("source line number is too large".into()))?;
        let line_start = prefix
            .rfind('\n')
            .map_or(checkpoint.line_start, |index| checkpoint.offset + index + 1);
        let column = u32::try_from(offset - line_start + 1)
            .map_err(|_| DbError::InvalidSnapshot("source column is too large".into()))?;
        self.positions[slot] = Some(CachedPosition {
            offset,
            line,
            column,
        });
        Ok((line, column))
    }
}

struct SourcePositionFile<'a> {
    file: &'a crate::ffi::slang::File,
    lines: Option<SourceLines<'a>>,
}

pub(super) struct SourcePositions<'a> {
    files: HashMap<u64, SourcePositionFile<'a>>,
}

impl<'a> SourcePositions<'a> {
    pub(super) fn new(snapshot: &'a SlangSnapshot) -> Self {
        let mut files = HashMap::with_capacity(snapshot.files.len());
        for file in &snapshot.files {
            files
                .entry(file.id)
                .or_insert(SourcePositionFile { file, lines: None });
        }
        Self { files }
    }

    pub(super) fn position(
        &mut self,
        node: &SemanticNode,
    ) -> Result<(Option<String>, u32, u32, u32, u32), DbError> {
        let Some(range) = node.range else {
            return Ok((None, 0, 0, 0, 0));
        };
        let file = self
            .files
            .get_mut(&range.file_id)
            .ok_or_else(|| DbError::InvalidSnapshot("semantic range file is missing".into()))?;
        let start = usize::try_from(range.start)
            .map_err(|_| DbError::InvalidSnapshot("semantic range start is too large".into()))?;
        let end = usize::try_from(range.end)
            .map_err(|_| DbError::InvalidSnapshot("semantic range end is too large".into()))?;
        let lines = file
            .lines
            .get_or_insert_with(|| SourceLines::new(&file.file.text));
        let (line, column) = lines.position(start)?;
        let (end_line, end_column) = lines.position(end)?;
        Ok((
            Some(file.file.name.clone()),
            line,
            column,
            end_line,
            end_column,
        ))
    }
}

pub(super) fn source_position(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
) -> Result<(Option<String>, u32, u32, u32, u32), DbError> {
    SourcePositions::new(snapshot).position(node)
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
        // The concrete loop block already carries the array name and index.
        // Its container remains addressable on its own but is not an extra
        // segment in a concrete hierarchical path (SV 27.4).
        if !node.name.is_empty()
            && (node_id == id || !matches!(node.kind(), NodeKind::GenScopeArray))
        {
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

#[cfg(test)]
mod source_position_tests {
    use super::*;

    #[test]
    fn source_position_scan_work_scales_with_source_bytes_and_queries() {
        let mut measured = Vec::new();
        for count in [
            SOURCE_POSITION_CHECKPOINT_BYTES / 64,
            SOURCE_POSITION_CHECKPOINT_BYTES / 4,
        ] {
            let text = format!("{}\n", "x".repeat(63)).repeat(count);
            let before_index = SOURCE_INDEX_BYTES.with(std::cell::Cell::get);
            let before_query = SOURCE_QUERY_BYTES.with(std::cell::Cell::get);
            let mut lines = SourceLines::new(&text);
            for index in 0..count {
                assert_eq!(lines.position(index * 64).unwrap(), (index as u32 + 1, 1));
                assert_eq!(
                    lines.position(index * 64 + 63).unwrap(),
                    (index as u32 + 1, 64)
                );
            }
            let indexed = SOURCE_INDEX_BYTES.with(std::cell::Cell::get) - before_index;
            let queried = SOURCE_QUERY_BYTES.with(std::cell::Cell::get) - before_query;
            assert_eq!(indexed, text.len());
            assert!(queried <= count * 2 * SOURCE_POSITION_CHECKPOINT_BYTES);
            measured.push(indexed + queried);
        }
        assert_eq!(measured[1], measured[0] * 16);
    }

    #[test]
    fn source_position_preserves_byte_columns_crlf_and_utf8_checkpoint_boundaries() {
        let text = "a\r\né\nlast";
        let mut lines = SourceLines::new(text);
        for (offset, expected) in [
            (0, (1, 1)),
            (1, (1, 2)),
            (2, (1, 3)),
            (3, (2, 1)),
            (5, (2, 3)),
            (6, (3, 1)),
            (10, (3, 5)),
        ] {
            assert_eq!(lines.position(offset).unwrap(), expected);
            assert_eq!(lines.position(offset).unwrap(), expected);
        }
        assert!(lines.position(4).is_err());
        assert!(lines.position(11).is_err());
        assert_eq!(SourceLines::new("").position(0).unwrap(), (1, 1));
        let text = format!(
            "{}é\nlast",
            "x".repeat(SOURCE_POSITION_CHECKPOINT_BYTES - 1)
        );
        let mut lines = SourceLines::new(&text);
        assert!(lines.position(SOURCE_POSITION_CHECKPOINT_BYTES).is_err());
        assert_eq!(
            lines
                .position(SOURCE_POSITION_CHECKPOINT_BYTES + 1)
                .unwrap(),
            (1, SOURCE_POSITION_CHECKPOINT_BYTES as u32 + 2)
        );
        assert_eq!(
            lines
                .position(SOURCE_POSITION_CHECKPOINT_BYTES + 2)
                .unwrap(),
            (2, 1)
        );
        assert_eq!(lines.position(text.len()).unwrap(), (2, 5));
    }

    #[test]
    fn owned_import_builds_each_source_index_once() {
        let source =
            "module top; for(genvar i=0;i<32;i++) begin: g logic [3:0] x=4'bxz01; end endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "indexed_source.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let before = SOURCE_INDEX_BYTES.with(std::cell::Cell::get);
        let db = Db::from_slang(&output.snapshot).unwrap();
        assert_eq!(
            SOURCE_INDEX_BYTES.with(std::cell::Cell::get) - before,
            source.len()
        );
        drop(output);
        db.validate().unwrap();
        assert_eq!(db.source_text("indexed_source.sv"), Some(source));
    }
}
