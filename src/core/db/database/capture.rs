//! Capture.

use super::*;

impl Db {
    /// Build the owned semantic database from a validated Slang snapshot.
    ///
    /// This conversion never reads source files and retains no native owner.
    pub fn from_slang(snapshot: &SlangSnapshot) -> Result<Self, DbError> {
        let type_projector = SlangTypeProjector::new(snapshot)?;
        let ids = SemanticIds::new(&snapshot.semantic_nodes)?;
        let mut source_positions = SourcePositions::new(snapshot);
        let mut source_identities = HashMap::new();
        for semantic in &snapshot.semantic_nodes {
            let id = NodeId::from_index(semantic.id as usize);
            for edge in semantic_edges(snapshot, semantic)? {
                if edge.role == SemanticEdgeRole::SourceIdentity {
                    source_identities.insert(id, semantic_id(&ids, edge.target_id)?);
                }
            }
        }

        let mut nodes = Vec::with_capacity(snapshot.semantic_nodes.len());
        // Placeholder until hierarchical names are assigned after the node
        // loop; one shared value avoids a temporary allocation per node.
        let unnamed: std::sync::Arc<str> = std::sync::Arc::from("");
        let overridden_parameters = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                semantic.kind == SemanticKind::Parameter && semantic.auxiliary == 1
            })
            .map(|(index, _)| NodeId::from_index(index))
            .collect();
        let semantic_kinds = snapshot
            .semantic_nodes
            .iter()
            .map(|semantic| semantic.kind.into())
            .collect();
        let mut semantic_details = DenseSharedBuilder::new(snapshot.semantic_nodes.len());
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            semantic_details.assign(NodeId::from_index(index), semantic.detail.as_str(), || {
                Box::from(semantic.detail.as_str())
            })?;
        }
        let semantic_details = semantic_details.finish();
        let mut conditional_patterns = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter_map(|(index, semantic)| {
                if semantic.kind != SemanticKind::Unsupported {
                    return None;
                }
                let kind = match semantic.subkind {
                    crate::ffi::slang::SEMANTIC_PATTERN_INVALID => ConditionalPatternKind::Invalid,
                    crate::ffi::slang::SEMANTIC_PATTERN_WILDCARD => {
                        ConditionalPatternKind::Wildcard
                    }
                    crate::ffi::slang::SEMANTIC_PATTERN_CONSTANT => {
                        ConditionalPatternKind::Constant
                    }
                    crate::ffi::slang::SEMANTIC_PATTERN_VARIABLE => ConditionalPatternKind::Binding,
                    crate::ffi::slang::SEMANTIC_PATTERN_TAGGED => ConditionalPatternKind::Tagged,
                    crate::ffi::slang::SEMANTIC_PATTERN_STRUCTURE => {
                        ConditionalPatternKind::Structure
                    }
                    _ => ConditionalPatternKind::Unsupported,
                };
                Some((
                    NodeId::from_index(index),
                    ConditionalPatternInfo {
                        kind,
                        binding: semantic
                            .target_id()
                            .and_then(|target| ids.get(&target))
                            .filter(|_| kind == ConditionalPatternKind::Binding),
                        tagged_member: semantic
                            .target_id()
                            .and_then(|target| ids.get(&target))
                            .filter(|_| kind == ConditionalPatternKind::Tagged),
                        value_pattern: None,
                    },
                ))
            })
            .collect::<HashMap<_, _>>();
        for (pattern, info) in &mut conditional_patterns {
            if info.kind != ConditionalPatternKind::Tagged {
                continue;
            }
            let semantic = snapshot
                .semantic_nodes
                .get(pattern.index())
                .ok_or_else(|| DbError::InvalidSnapshot("tagged pattern node is missing".into()))?;
            let edges = semantic_edges(snapshot, semantic)?;
            let operands = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect::<Vec<_>>();
            if operands.len() > 1 || operands.first().is_some_and(|edge| edge.index != 0) {
                return Err(DbError::InvalidSnapshot(
                    "tagged pattern has an invalid payload pattern edge".into(),
                ));
            }
            info.value_pattern = operands
                .first()
                .map(|edge| semantic_id(&ids, edge.target_id))
                .transpose()?;
        }
        let mut conditional_pattern_fields = HashMap::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            if semantic.kind != SemanticKind::Unsupported
                || semantic.subkind != crate::ffi::slang::SEMANTIC_PATTERN_STRUCTURE
            {
                continue;
            }
            let edges = semantic_edges(snapshot, semantic)?;
            let mut operands = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Operand)
                .collect::<Vec<_>>();
            operands.sort_by_key(|edge| edge.index);
            let mut declarations = HashMap::new();
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
            {
                if declarations.insert(edge.index, edge.target_id).is_some() {
                    return Err(DbError::InvalidSnapshot(
                        "duplicate structure pattern field index".into(),
                    ));
                }
            }
            let mut fields = Vec::with_capacity(operands.len());
            for (expected, operand) in operands.into_iter().enumerate() {
                if usize::try_from(operand.index).ok() != Some(expected) {
                    return Err(DbError::InvalidSnapshot(
                        "structure pattern field indices must be dense and unique".into(),
                    ));
                }
                let field = declarations.remove(&operand.index).ok_or_else(|| {
                    DbError::InvalidSnapshot(
                        "structure pattern field has no resolved declaration".into(),
                    )
                })?;
                fields.push(ConditionalPatternField {
                    field: semantic_id(&ids, field)?,
                    pattern: semantic_id(&ids, operand.target_id)?,
                });
            }
            if !declarations.is_empty() {
                return Err(DbError::InvalidSnapshot(
                    "structure pattern has an unpaired field declaration".into(),
                ));
            }
            conditional_pattern_fields.insert(NodeId::from_index(index), fields);
        }
        let program_instances = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                matches!(
                    semantic.kind,
                    SemanticKind::Instance | SemanticKind::Definition
                ) && semantic.definition_kind == Some(SemanticDefinitionKind::Program)
            })
            .map(|(index, _)| NodeId::from_index(index))
            .collect();
        let source_libraries = snapshot
            .source_libraries
            .iter()
            .map(|binding| {
                Ok((
                    semantic_id(&ids, binding.semantic_id)?,
                    binding.library.clone(),
                ))
            })
            .collect::<Result<HashMap<_, _>, DbError>>()?;
        let mut declaration_time_scales = HashMap::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let namespace = |node: &crate::ffi::slang::SemanticNode| {
                node.kind == SemanticKind::Package || node.detail == "CompilationUnit"
            };
            // A class takes the scale of the nearest enclosing module,
            // package or compilation unit (Slang's `Scope::getTimeScale`).
            let scale = if namespace(semantic) {
                semantic.time_scale()
            } else if semantic.kind == SemanticKind::Class {
                let mut ancestor = semantic.parent_id();
                let mut scale = None;
                while let Some(parent) =
                    ancestor.and_then(|id| snapshot.semantic_nodes.get(id as usize))
                {
                    if namespace(parent)
                        || matches!(
                            parent.kind,
                            SemanticKind::Instance | SemanticKind::Definition
                        )
                    {
                        scale = parent.time_scale();
                        break;
                    }
                    ancestor = parent.parent_id();
                }
                scale
            } else {
                continue;
            };
            if scale.is_some() {
                declaration_time_scales.insert(
                    NodeId::from_index(index),
                    (time_exponent(scale, false)?, time_exponent(scale, true)?),
                );
            }
        }
        let source_map = super::super::SourceMap::from_slang(snapshot)?;
        let legacy_directives = legacy_directives(snapshot, &mut source_positions)?;
        let unconnected_drives = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                matches!(
                    semantic.kind,
                    SemanticKind::Instance | SemanticKind::Definition
                )
            })
            .filter_map(|(index, semantic)| {
                let drive = if semantic.auxiliary & crate::ffi::slang::UNCONNECTED_PULL0 != 0 {
                    UnconnectedDrive::Pull0
                } else if semantic.auxiliary & crate::ffi::slang::UNCONNECTED_PULL1 != 0 {
                    UnconnectedDrive::Pull1
                } else {
                    return None;
                };
                Some((NodeId::from_index(index), drive))
            })
            .collect();
        let mut arrays = HashMap::new();
        let mut event_arrays = HashMap::new();
        let mut subroutine_arrays = HashMap::new();
        let mut array_select_paths = HashMap::new();
        let mut vars_init = HashMap::new();
        let mut net_delays = HashMap::new();
        let mut two_state_types = HashSet::new();
        let assertion_local_vars = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                semantic.kind == SemanticKind::Variable
                    && semantic.subkind == crate::ffi::slang::SEMANTIC_VARIABLE_ASSERTION_LOCAL
            })
            .map(|semantic| NodeId::from_index(semantic.id as usize))
            .collect();
        let assertion_formal_directions = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                (semantic.kind == SemanticKind::Argument
                    || (semantic.kind == SemanticKind::Variable
                        && semantic.subkind
                            == crate::ffi::slang::SEMANTIC_VARIABLE_ASSERTION_LOCAL))
                    && (semantic.is_input()
                        || semantic.is_output()
                        || semantic.is_inout()
                        || semantic.is_ref())
            })
            .map(|semantic| {
                (
                    NodeId::from_index(semantic.id as usize),
                    direction_from_slang(semantic),
                )
            })
            .collect();
        let mut implicit_nets = HashSet::new();
        let mut implicit_conversions = HashSet::new();
        let mut var_lifetimes = HashMap::new();
        let mut var_lifetime_qualifiers = HashMap::new();
        let mut method_calls_with_clause = HashSet::new();
        let mut method_call_iterators = HashMap::new();
        let mut packed_members = HashMap::new();
        let mut aggregate_layouts = HashMap::new();
        let mut type_descriptors = DenseSharedBuilder::new(snapshot.semantic_nodes.len());
        // A projection depends only on its frontend type record, so each
        // distinct type is projected once and shared by every typed node.
        let mut projections: HashMap<u64, std::rc::Rc<_>> = HashMap::new();
        let mut packed_pattern_elements = HashMap::new();
        let mailbox_elements = type_projector
            .mailbox_elements()
            .map_err(DbError::InvalidSnapshot)?;
        let mut enum_types = HashMap::new();
        let mut packed_dimensions = HashMap::new();
        let mut clocking_blocks = HashMap::new();
        let mut clocking_vars = HashMap::new();
        let mut default_clockings = HashMap::new();
        let dpi_imports = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                semantic.kind == SemanticKind::Subroutine
                    && semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_IMPORT != 0
            })
            .map(|semantic| {
                let id = NodeId::from_index(semantic.id as usize);
                let c_name = if semantic.definition_name.is_empty() {
                    semantic.name.to_string()
                } else {
                    semantic.definition_name.to_string()
                };
                (
                    id,
                    DpiImportInfo {
                        c_name,
                        context: semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_CONTEXT
                            != 0,
                        pure: semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_PURE != 0,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let has_dpi_exports = snapshot.semantic_nodes.iter().any(|semantic| {
            semantic.kind == SemanticKind::Subroutine
                && semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_EXPORT != 0
        });
        for semantic in &snapshot.semantic_nodes {
            let id = NodeId::from_index(semantic.id as usize);
            let edges = semantic_edges(snapshot, semantic)?;
            let mut children = Vec::new();
            for edge in edges {
                let child_semantic = snapshot
                    .semantic_nodes
                    .get(edge.target_id as usize)
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("semantic child target is missing".into())
                    })?;
                if child_semantic.is_uninstantiated()
                    || child_semantic.kind == SemanticKind::Definition
                {
                    continue;
                }
                if semantic.kind == SemanticKind::Unsupported
                    && semantic.subkind == crate::ffi::slang::SEMANTIC_PATTERN_STRUCTURE
                    && edge.role == SemanticEdgeRole::Declaration
                {
                    // Structure-pattern field declarations are metadata, not
                    // executable children. Their resolved identities live in
                    // `conditional_pattern_fields` below.
                    continue;
                }
                if !matches!(
                    edge.role,
                    SemanticEdgeRole::Reference
                        | SemanticEdgeRole::Callee
                        | SemanticEdgeRole::Actual
                        | SemanticEdgeRole::HighConnection
                        | SemanticEdgeRole::LowConnection
                        | SemanticEdgeRole::SourceIdentity
                        | SemanticEdgeRole::ReturnOwner
                ) {
                    children.push(semantic_id(&ids, edge.target_id)?);
                }
            }
            if semantic.kind == SemanticKind::Port {
                if let Some(high_expression) =
                    edge_target(&ids, edges, SemanticEdgeRole::HighConnection)?
                {
                    children.push(high_expression);
                }
            }
            let anonymous_program = |node: &crate::ffi::slang::SemanticNode| {
                node.kind == SemanticKind::Scope
                    && node.subkind == crate::ffi::slang::SEMANTIC_SCOPE_ANONYMOUS_PROGRAM
            };
            if matches!(
                semantic.kind,
                SemanticKind::Instance
                    | SemanticKind::Scope
                    | SemanticKind::GenerateScope
                    | SemanticKind::Package
            ) && semantic.subkind != 194
                && !anonymous_program(semantic)
            {
                let mut flattened = Vec::new();
                let mut expanded = HashSet::new();
                let mut pending = children;
                pending.reverse();
                while let Some(child) = pending.pop() {
                    let child_semantic = &snapshot.semantic_nodes[child.index()];
                    // An anonymous program's members belong to the
                    // enclosing package or `$unit` namespace (SV 24.6).
                    let instance_body = child_semantic.kind == SemanticKind::Scope
                        && (child_semantic.subkind == 194 || anonymous_program(child_semantic));
                    let instance_array = child_semantic.kind == SemanticKind::Instance
                        && child_semantic.subkind == 193;
                    if instance_body || instance_array {
                        if !expanded.insert(child) {
                            return Err(DbError::InvalidSnapshot(
                                "cyclic or repeated instance container".to_owned(),
                            ));
                        }
                        // Arrays can occur inside the implicit instance body
                        // or another array dimension. Expand every container
                        // before exposing concrete children to consumers.
                        pending.extend(
                            semantic_edges(snapshot, child_semantic)?
                                .iter()
                                .rev()
                                .filter(|edge| {
                                    if instance_array {
                                        edge.role == SemanticEdgeRole::Child
                                    } else {
                                        // The default clocking edge names a
                                        // block owned elsewhere or already a
                                        // child.
                                        edge.role != SemanticEdgeRole::Reference
                                            && edge.role != SemanticEdgeRole::Clocking
                                    }
                                })
                                .filter(|edge| {
                                    snapshot
                                        .semantic_nodes
                                        .get(edge.target_id as usize)
                                        .is_some_and(|node| {
                                            !node.is_uninstantiated()
                                                && node.kind != SemanticKind::Definition
                                        })
                                })
                                .map(|edge| semantic_id(&ids, edge.target_id))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    } else {
                        flattened.push(child);
                    }
                }
                children = flattened;
            } else if semantic.kind == SemanticKind::Scope
                && (semantic.subkind == 194 || anonymous_program(semantic))
            {
                children.clear();
            }
            let mut seen_children = HashSet::new();
            children.retain(|child| seen_children.insert(*child));
            let projection = match semantic.type_id() {
                Some(type_id) => Some(match projections.get(&type_id) {
                    Some(projection) => std::rc::Rc::clone(projection),
                    None => {
                        let projection = std::rc::Rc::new(type_projector.project(type_id)?);
                        projections.insert(type_id, std::rc::Rc::clone(&projection));
                        projection
                    }
                }),
                None => None,
            };
            let type_info = projection
                .as_ref()
                .map(|projection| projection.type_info.clone())
                .unwrap_or_default();
            if projection
                .as_ref()
                .is_some_and(|projection| projection.two_state)
            {
                two_state_types.insert(id);
            }
            if semantic.kind == SemanticKind::Net && semantic.is_implicit() {
                implicit_nets.insert(id);
            }
            if semantic.is_implicit_conversion() {
                implicit_conversions.insert(id);
            }
            if semantic.kind == SemanticKind::MethodCall && semantic.method_with_clause() {
                method_calls_with_clause.insert(id);
                if let Some(iterator) = semantic.target_id() {
                    method_call_iterators.insert(id, semantic_id(&ids, iterator)?);
                }
            }
            if semantic.kind == SemanticKind::Scope
                && semantic.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
            {
                let event =
                    edge_target(&ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking block has no event control".into())
                    })?;
                let event_node = snapshot.semantic_nodes.get(event.index()).ok_or_else(|| {
                    DbError::InvalidSnapshot("clocking block event is missing".into())
                })?;
                let (event_specs, event_implicit) = event_specs(snapshot, event_node, &ids)?;
                let input_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 0)?;
                let output_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 1)?;
                let default_input = clocking_skew_from_slang(
                    snapshot,
                    &ids,
                    input_delay,
                    semantic.auxiliary >> CLOCKING_INPUT_EDGE_SHIFT,
                )?;
                let default_output = clocking_skew_from_slang(
                    snapshot,
                    &ids,
                    output_delay,
                    semantic.auxiliary >> CLOCKING_OUTPUT_EDGE_SHIFT,
                )?;
                clocking_blocks.insert(
                    id,
                    ClockingBlockInfo {
                        event,
                        event_specs,
                        event_implicit,
                        is_default: semantic.auxiliary & CLOCKING_BLOCK_DEFAULT != 0,
                        is_global: semantic.auxiliary & CLOCKING_BLOCK_GLOBAL != 0,
                        default_input,
                        default_output,
                    },
                );
            }
            if semantic.kind == SemanticKind::Instance && semantic.subkind != 193 {
                // The instance body carries the frontend's resolved default
                // clocking as an extra clocking edge.
                for edge in edges
                    .iter()
                    .filter(|edge| edge.role == SemanticEdgeRole::Child)
                {
                    let body = snapshot
                        .semantic_nodes
                        .get(edge.target_id as usize)
                        .ok_or_else(|| {
                            DbError::InvalidSnapshot("instance body is missing".into())
                        })?;
                    if body.kind != SemanticKind::Scope || body.subkind != 194 {
                        continue;
                    }
                    let Some(block) = edge_target(
                        &ids,
                        semantic_edges(snapshot, body)?,
                        SemanticEdgeRole::Clocking,
                    )?
                    else {
                        continue;
                    };
                    let block_node = snapshot.semantic_nodes.get(block.index());
                    if !block_node.is_some_and(|node| {
                        node.kind == SemanticKind::Scope
                            && node.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
                    }) {
                        return Err(DbError::InvalidSnapshot(
                            "default clocking does not name a clocking block".into(),
                        ));
                    }
                    default_clockings.insert(id, block);
                }
            }
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind == SEMANTIC_VARIABLE_CLOCKING
            {
                let initializer = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "clocking variable has no source expression".into(),
                        )
                    })?;
                let source = clocking_source_from_expression(snapshot, &ids, initializer, 0)?;
                let parent_raw = semantic
                    .parent_id()
                    .and_then(|parent| snapshot.semantic_nodes.get(parent as usize));
                let block = semantic
                    .parent_id()
                    .and_then(|parent| ids.get(&parent))
                    .filter(|_| {
                        parent_raw.is_some_and(|parent| {
                            parent.kind == SemanticKind::Scope
                                && parent.subkind == SEMANTIC_SCOPE_CLOCKING_BLOCK
                        })
                    })
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "clocking variable is not owned by a clocking block".into(),
                        )
                    })?;
                let input_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 0)?;
                let output_delay = edge_target_at(&ids, edges, SemanticEdgeRole::Delay, 1)?;
                clocking_vars.insert(
                    id,
                    ClockingVarInfo {
                        block,
                        source,
                        expression: initializer,
                        direction: direction_from_slang(semantic),
                        input: clocking_skew_from_slang(
                            snapshot,
                            &ids,
                            input_delay,
                            semantic.auxiliary,
                        )?,
                        output: clocking_skew_from_slang(
                            snapshot,
                            &ids,
                            output_delay,
                            semantic.auxiliary >> CLOCKING_VAR_OUTPUT_EDGE_SHIFT,
                        )?,
                    },
                );
            }
            if matches!(
                semantic.kind,
                SemanticKind::Variable | SemanticKind::NamedEvent
            ) && semantic.subkind != 229
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
            {
                let resolved_lifetime = match semantic.auxiliary {
                    0 => VariableLifetime::Unavailable,
                    1 => VariableLifetime::Static,
                    2 => VariableLifetime::Automatic,
                    _ => {
                        return Err(DbError::InvalidSnapshot(
                            "variable has an unknown resolved lifetime".into(),
                        ));
                    }
                };
                var_lifetimes.insert(id, resolved_lifetime);
                let lifetime = if semantic.is_automatic() {
                    VariableLifetimeQualifier::Automatic
                } else if semantic.is_static() {
                    VariableLifetimeQualifier::Static
                } else {
                    VariableLifetimeQualifier::None
                };
                var_lifetime_qualifiers.insert(id, lifetime);
            }
            let is_event_array = semantic.kind == SemanticKind::NamedEvent
                && projection
                    .as_ref()
                    .is_some_and(|projection| projection.array.is_some());
            let is_array = matches!(semantic.kind, SemanticKind::Variable | SemanticKind::Net)
                && semantic.subkind != 229
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
                && projection
                    .as_ref()
                    .is_some_and(|projection| projection.array.is_some())
                || semantic.kind == SemanticKind::Array;
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
                && !is_array
            {
                if let Some(initializer) = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                {
                    vars_init.insert(id, initializer);
                }
            }
            if is_array {
                let array = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref());
                arrays.insert(
                    id,
                    ArrayMeta {
                        kind: array
                            .map(|array| array.kind.clone())
                            .unwrap_or(ArrayKind::Static),
                        dims: array
                            .map(|array| array.dimensions.clone())
                            .unwrap_or_default(),
                        init: edge_target(&ids, edges, SemanticEdgeRole::Initializer)?,
                        net_type: (semantic.kind == SemanticKind::Net)
                            .then(|| net_type_from_subkind(semantic.subkind)),
                    },
                );
            }
            if matches!(
                semantic.kind,
                SemanticKind::Argument | SemanticKind::Subroutine
            ) {
                if let Some(array) = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref())
                    .filter(|array| !matches!(array.kind, ArrayKind::Static))
                {
                    subroutine_arrays.insert(
                        id,
                        ArrayMeta {
                            kind: array.kind.clone(),
                            dims: array.dimensions.clone(),
                            init: None,
                            net_type: None,
                        },
                    );
                }
            }
            if is_event_array {
                let array = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref());
                event_arrays.insert(
                    id,
                    ArrayMeta {
                        kind: array
                            .map(|array| array.kind.clone())
                            .unwrap_or(ArrayKind::Static),
                        dims: array
                            .map(|array| array.dimensions.clone())
                            .unwrap_or_default(),
                        init: edge_target(&ids, edges, SemanticEdgeRole::Initializer)?,
                        net_type: None,
                    },
                );
            }
            if let Some(projection) = &projection {
                if let Some(type_id) = semantic.type_id() {
                    type_descriptors.assign(id, type_id, || projection.descriptor.clone())?;
                }
                if !projection.packed_dimensions.is_empty() {
                    packed_dimensions.insert(id, projection.packed_dimensions.clone());
                }
                if let Some(members) = &projection.packed_members {
                    packed_members.insert(id, members.clone());
                }
                if let Some(layout) = &projection.aggregate_layout {
                    aggregate_layouts.insert(id, layout.clone());
                }
            }
            if semantic.kind == SemanticKind::EnumConstant {
                if let (Some(type_id), Some(Val::Bits(value)), Some(enum_type)) = (
                    semantic.type_id(),
                    semantic
                        .constant_id()
                        .and_then(|constant_id| snapshot.constants.get(constant_id as usize))
                        .and_then(|constant| val_from_slang(&constant.value)),
                    projection.as_ref(),
                ) {
                    if enum_type.type_info.kind == "enum" {
                        let width = enum_type.type_info.width.ok_or_else(|| {
                            DbError::InvalidSnapshot(format!(
                                "enum type {type_id} has no resolved width"
                            ))
                        })?;
                        enum_types
                            .entry(TypeId(type_id))
                            .or_insert_with(|| EnumTypeMetadata {
                                width,
                                signed: enum_type.type_info.signed,
                                two_state: enum_type.two_state,
                                members: Vec::new(),
                            })
                            .members
                            .push(EnumMember {
                                name: semantic.name.to_string(),
                                value: Val::Bits(value),
                            });
                    }
                }
            }
            let mut parent = if semantic.is_top()
                || matches!(
                    semantic.kind,
                    SemanticKind::Definition | SemanticKind::Package | SemanticKind::Class
                ) {
                None
            } else {
                semantic
                    .parent_id()
                    .map(|parent| semantic_id(&ids, parent))
                    .transpose()?
            };
            if let Some(parent_id) = parent {
                let parent_semantic = &snapshot.semantic_nodes[parent_id.index()];
                if parent_semantic.kind == SemanticKind::Scope && parent_semantic.subkind == 194 {
                    parent = parent_semantic
                        .parent_id()
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                } else if parent_semantic.kind == SemanticKind::Instance
                    && parent_semantic.subkind == 193
                {
                    parent = parent_semantic
                        .parent_id()
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                }
            }
            let (file, line, col, end_line, end_col) = source_positions.position(semantic)?;
            let mut kind =
                node_kind_from_slang(snapshot, &type_projector, semantic, edges, &ids, type_info)?;
            if matches!(
                &kind,
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                    ..
                })
            ) {
                if let Some(type_id) = semantic.type_id() {
                    if let Some(element) = type_projector.packed_pattern_element(type_id)? {
                        packed_pattern_elements.insert(id, element);
                    }
                }
            }
            if semantic.kind == SemanticKind::Net {
                if let Some(delay) = driver_delay(snapshot, &ids, edges)? {
                    net_delays.insert(id, delay);
                }
            }
            if semantic.kind == SemanticKind::Expression && semantic.subkind == 73 {
                if let Some(mut base) = edge_target(&ids, edges, SemanticEdgeRole::Base)? {
                    // ArraySelect flattens successive element indices. Its
                    // aggregate owner path must come from the original member
                    // access, not from an intervening element-select node.
                    let mut depth = 0;
                    while snapshot.semantic_nodes[base.index()].kind == SemanticKind::Expression
                        && snapshot.semantic_nodes[base.index()].subkind == 73
                    {
                        if depth >= snapshot.semantic_nodes.len() {
                            return Err(DbError::InvalidSnapshot(
                                "array select chain contains a cycle".into(),
                            ));
                        }
                        depth += 1;
                        let base_edges =
                            semantic_edges(snapshot, &snapshot.semantic_nodes[base.index()])?;
                        let Some(parent) = edge_target(&ids, base_edges, SemanticEdgeRole::Base)?
                        else {
                            break;
                        };
                        base = parent;
                    }
                    if let Some((parts, refs)) = member_path_from_slang(
                        snapshot,
                        &type_projector,
                        &ids,
                        &snapshot.semantic_nodes[base.index()],
                        0,
                    )? {
                        if parts.len() > 1 {
                            if let Some(owner) = refs.into_iter().flatten().next() {
                                array_select_paths
                                    .insert(id, (owner, parts.into_iter().skip(1).collect()));
                            }
                        }
                    }
                }
            }
            if semantic.kind == SemanticKind::Statement && semantic.subkind == 59 {
                // `foreach (r.q[i])` resolves its target to the member's
                // declaration; keep the owning record and member path so a
                // container member of a record instance can be iterated.
                if let Some(base) = edge_target(&ids, edges, SemanticEdgeRole::Base)? {
                    if let Some((parts, refs)) = member_path_from_slang(
                        snapshot,
                        &type_projector,
                        &ids,
                        &snapshot.semantic_nodes[base.index()],
                        0,
                    )? {
                        if parts.len() > 1 {
                            if let Some(owner) = refs.into_iter().flatten().next() {
                                array_select_paths
                                    .insert(id, (owner, parts.into_iter().skip(1).collect()));
                            }
                        }
                    }
                }
            }
            if is_array {
                let element_type = projection
                    .as_ref()
                    .and_then(|projection| projection.array.as_ref())
                    .map(|array| array.element_type.clone())
                    .unwrap_or_default();
                kind = NodeKind::Array { ty: element_type };
            }
            if matches!(kind, NodeKind::Stmt(StmtKind::Assign { .. })) {
                let expression =
                    edge_target(&ids, edges, SemanticEdgeRole::Body)?.ok_or_else(|| {
                        DbError::InvalidSnapshot("assignment statement has no expression".into())
                    })?;
                let expression_node = &snapshot.semantic_nodes[expression.index()];
                let expression_edges = semantic_edges(snapshot, expression_node)?;
                children = [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| {
                        edge_target(&ids, expression_edges, role)?.ok_or_else(|| {
                            DbError::InvalidSnapshot("assignment expression has no operand".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            } else if semantic.kind == SemanticKind::ContinuousAssign {
                let assignment =
                    edge_target(&ids, edges, SemanticEdgeRole::Body)?.ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "continuous assignment has no assignment expression".into(),
                        )
                    })?;
                let assignment_node = &snapshot.semantic_nodes[assignment.index()];
                let assignment_edges = semantic_edges(snapshot, assignment_node)?;
                children = [SemanticEdgeRole::Lhs, SemanticEdgeRole::Rhs]
                    .into_iter()
                    .map(|role| {
                        edge_target(&ids, assignment_edges, role)?.ok_or_else(|| {
                            DbError::InvalidSnapshot(
                                "continuous assignment expression has no operand".into(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(delay) = edge_target(&ids, edges, SemanticEdgeRole::Delay)? {
                    children.push(delay);
                }
            } else if semantic.kind == SemanticKind::Statement && semantic.subkind == 40 {
                children = match &kind {
                    NodeKind::Stmt(StmtKind::EventControl { specs, body, .. }) => {
                        let mut values = Vec::new();
                        for spec in specs {
                            spec.referenced_nodes(&mut values);
                        }
                        values.extend(*body);
                        values
                    }
                    _ => edge_target(&ids, edges, SemanticEdgeRole::Body)?
                        .into_iter()
                        .collect(),
                };
            } else if semantic.kind == SemanticKind::Statement
                && semantic.subkind == 42
                && semantic.auxiliary == 1
            {
                children = match &kind {
                    NodeKind::Stmt(StmtKind::WaitOrder {
                        events,
                        if_true,
                        if_false,
                    }) => {
                        let mut values = events.clone();
                        values.extend(if_true.iter().chain(if_false.iter()).copied());
                        values
                    }
                    _ => Vec::new(),
                };
            } else if semantic.kind == SemanticKind::Statement
                && semantic.subkind == crate::ffi::slang::SEMANTIC_STMT_PATTERN_CASE
            {
                // Pattern-case roles are explicit because a pattern item has
                // a pattern, optional filter, and body.  Keep those in source
                // order so downstream traversal cannot confuse a filter or
                // binding scope with an ordinary case expression.
                children = match &kind {
                    NodeKind::Stmt(StmtKind::PatternCase {
                        selector,
                        items,
                        default,
                        ..
                    }) => {
                        let mut values = vec![*selector];
                        for item in items {
                            values.push(item.pattern);
                            values.extend(item.filter);
                            values.push(item.body);
                        }
                        values.extend(*default);
                        values
                    }
                    _ => Vec::new(),
                };
            }
            let target_name = || {
                semantic
                    .target_id()
                    .and_then(|target| ids.get(&target))
                    .and_then(|target| snapshot.semantic_nodes.get(target.index()))
                    .map(|target| target.name.to_string())
                    .unwrap_or_default()
            };
            let name = if semantic.name.is_empty()
                && ((semantic.kind == SemanticKind::Statement
                    && matches!(semantic.subkind, 32 | 56..=58))
                    || (semantic.kind == SemanticKind::Expression && semantic.subkind == 65))
            {
                target_name()
            } else {
                semantic.name.to_string()
            };
            nodes.push(Node {
                kind,
                children,
                parent,
                name,
                full_name: std::sync::Arc::clone(&unnamed),
                file,
                line,
                col,
                end_line,
                end_col,
            });
        }

        // Member-access expressions through a virtual interface can bind the
        // final clocking variable to a detached semantic node. Slang retains
        // the declaration's `ClockVar` detail and source range on that node,
        // but not the declaration subkind used above. Reuse the declaration
        // metadata by range/name so lowering can select sampled storage for
        // both static and dynamically-held virtual interfaces.
        let clocking_declarations = clocking_vars
            .iter()
            .filter_map(|(id, info)| {
                let semantic = snapshot.semantic_nodes.get(id.index())?;
                Some((*id, semantic.range?, semantic.name.clone(), info.clone()))
            })
            .collect::<Vec<_>>();
        for semantic in &snapshot.semantic_nodes {
            if semantic.detail != "ClockVar" {
                continue;
            }
            let id = NodeId::from_index(semantic.id as usize);
            if clocking_vars.contains_key(&id) {
                continue;
            }
            let Some(range) = semantic.range else {
                continue;
            };
            if let Some((_, _, _, info)) =
                clocking_declarations
                    .iter()
                    .find(|(_, declaration_range, name, _)| {
                        *declaration_range == range && name == &semantic.name
                    })
            {
                clocking_vars.insert(id, info.clone());
            }
        }

        // A virtual interface handle is an elaboration-time alias to a
        // concrete interface instance.  Capture that static binding while
        // the frontend identities are still available; lowering can then
        // resolve clocking members without retaining native Slang objects.
        let modport_directions = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .filter(|(_, semantic)| {
                semantic.kind == SemanticKind::Modport && semantic.detail == "ModportPort"
            })
            .map(|(index, semantic)| (NodeId::from_index(index), direction_from_slang(semantic)))
            .collect();
        // The port expression is a low connection edge, which the child
        // projection above excludes: it is not executable on its own, only
        // through references to the port.
        let mut modport_expressions = HashMap::new();
        for semantic in &snapshot.semantic_nodes {
            if semantic.kind != SemanticKind::Modport || semantic.detail != "ModportPort" {
                continue;
            }
            let edges = semantic_edges(snapshot, semantic)?;
            if let Some(expression) = edge_target(&ids, edges, SemanticEdgeRole::LowConnection)? {
                modport_expressions.insert(NodeId::from_index(semantic.id as usize), expression);
            }
        }
        let mut virtual_interface_targets = HashMap::new();
        for &variable in vars_init.keys() {
            if let Some(instance) = virtual_interface_instance_from_slang(snapshot, &ids, variable)?
            {
                virtual_interface_targets.insert(variable, instance);
            }
        }

        assign_semantic_full_names(&mut nodes)?;
        let mut elaborated_type_ranges = Vec::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let id = NodeId::from_index(index);
            let Some(type_id) = semantic.type_id() else {
                continue;
            };
            if semantic.name.is_empty()
                || !matches!(
                    semantic.kind,
                    SemanticKind::Net | SemanticKind::Variable | SemanticKind::Array
                )
                || !packed_dimensions.contains_key(&id)
            {
                continue;
            }
            let mut ancestor = semantic.parent_id();
            let mut has_runtime_instance = false;
            for _ in 0..snapshot.semantic_nodes.len() {
                let Some(parent) = ancestor.and_then(|id| snapshot.semantic_nodes.get(id as usize))
                else {
                    break;
                };
                if parent.kind == SemanticKind::Instance && parent.subkind == 192 {
                    has_runtime_instance = true;
                    break;
                }
                ancestor = parent.parent_id();
            }
            if !has_runtime_instance {
                continue;
            }
            let instance = enclosing_scope_name(&nodes, id)
                .unwrap_or_else(|| nodes[id.index()].full_name.to_string());
            if !instance.is_empty() {
                elaborated_type_ranges.push(type_projector.elaborated_ranges(
                    id,
                    instance,
                    semantic.name.to_string(),
                    type_id,
                )?);
            }
        }
        let tops: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Instance && node.is_top())
            .map(|node| NodeId::from_index(node.id as usize))
            .collect();
        let flat_modules: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Definition)
            .map(|node| NodeId::from_index(node.id as usize))
            .collect();
        let packages: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Package)
            .map(|node| NodeId::from_index(node.id as usize))
            .collect();
        let classes: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| NodeId::from_index(node.id as usize))
            .collect();
        let class_metadata = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| -> Result<(NodeId, ClassMetadata), DbError> {
                let id = NodeId::from_index(node.id as usize);
                Ok((
                    id,
                    ClassMetadata {
                        type_id: node.type_id().map(TypeId),
                        base: node.target_id().and_then(|target| ids.get(&target)),
                        base_constructor: edge_target(
                            &ids,
                            semantic_edges(snapshot, node)?,
                            SemanticEdgeRole::BaseConstructor,
                        )?,
                        is_abstract: node.auxiliary & crate::ffi::slang::CLASS_ABSTRACT != 0,
                        is_final: node.auxiliary & crate::ffi::slang::CLASS_FINAL != 0,
                        is_interface: node.auxiliary & crate::ffi::slang::CLASS_INTERFACE != 0,
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        let design_name = tops
            .first()
            .map(|id| nodes[id.index()].name.clone())
            .unwrap_or_else(|| "design".to_owned());
        let db = Self {
            nodes,
            edition: snapshot.edition(),
            overridden_parameters,
            semantic_kinds,
            semantic_details,
            conditional_patterns,
            conditional_pattern_fields,
            program_instances,
            unconnected_drives,
            source_libraries,
            declaration_time_scales,
            source_map,
            legacy_directives,
            tops,
            flat_modules,
            packages,
            classes,
            class_metadata,
            design_name,
            arrays,
            event_arrays,
            subroutine_arrays,
            array_select_paths,
            vars_init,
            net_delays,
            var_lifetimes,
            var_lifetime_qualifiers,
            method_calls_with_clause,
            method_call_iterators,
            packed_members,
            aggregate_layouts,
            type_descriptors: type_descriptors.finish(),
            packed_pattern_elements,
            mailbox_elements,
            enum_types,
            packed_dimensions,
            two_state_types,
            assertion_local_vars,
            assertion_formal_directions,
            source_identities,
            clocking_blocks,
            clocking_vars,
            default_clockings,
            modport_directions,
            modport_expressions,
            virtual_interface_targets,
            dpi_imports,
            has_dpi_exports,
            implicit_nets,
            implicit_conversions,
            source_files: snapshot
                .files
                .iter()
                .map(|file| (file.name.clone(), file.text.clone()))
                .collect(),
            elaborated_type_ranges,
        };
        db.validate().map_err(DbError::InvalidDatabase)?;
        Ok(db)
    }
}

/// Optional charge and delay-mode directives (IEEE 1364-2001 Annex D, IEEE
/// 1800-2009 Annex E). Slang consumes them without effect, so their tokens
/// are the only record that the source used them.
const LEGACY_DIRECTIVES: [&str; 6] = [
    "default_decay_time",
    "default_trireg_strength",
    "delay_mode_distributed",
    "delay_mode_path",
    "delay_mode_unit",
    "delay_mode_zero",
];

fn legacy_directives(
    snapshot: &SlangSnapshot,
    positions: &mut SourcePositions<'_>,
) -> Result<Vec<LegacyDirective>, DbError> {
    let mut directives = Vec::new();
    for token in &snapshot.lexical_tokens {
        if !token.is_directive || token.is_skipped || token.is_missing {
            continue;
        }
        let Some(name) = token
            .text
            .strip_prefix('`')
            .filter(|name| LEGACY_DIRECTIVES.contains(name))
        else {
            continue;
        };
        let Some(range) = token.range else { continue };
        let (file, line, column, _, _) = positions.range_position(range)?;
        directives.push(LegacyDirective {
            name: name.to_owned(),
            file: file.map(|file| file.to_string()).unwrap_or_default(),
            line,
            column,
        });
    }
    Ok(directives)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_import_shares_type_descriptors_and_details_per_distinct_value() {
        let source = "module top; for (genvar i = 0; i < 32; i++) begin : g \
            logic [3:0] x = 4'bxz01; logic [3:0] y; assign y = x ^ 4'b0110; end endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "shared_metadata.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let snapshot = &output.snapshot;
        let db = Db::from_slang(snapshot).unwrap();
        let projector = SlangTypeProjector::new(snapshot).unwrap();
        let mut by_type: HashMap<u64, *const TypeDescriptor> = HashMap::new();
        let mut typed = 0;
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let id = NodeId::from_index(index);
            assert_eq!(db.semantic_detail(id), Some(semantic.detail.as_str()));
            let Some(type_id) = semantic.type_id() else {
                assert!(db.type_descriptor(id).is_none());
                continue;
            };
            typed += 1;
            let descriptor = db.type_descriptor(id).unwrap();
            assert_eq!(*descriptor, projector.project(type_id).unwrap().descriptor);
            let shared = *by_type.entry(type_id).or_insert(descriptor);
            assert!(std::ptr::eq(shared, descriptor));
        }
        assert_eq!(db.type_descriptors.value_count(), by_type.len());
        assert!(by_type.len() < typed);
        assert!(db.semantic_details.value_count() < snapshot.semantic_nodes.len());
        assert_eq!(
            db.semantic_details.slot_count(),
            snapshot.semantic_nodes.len()
        );
    }
}
