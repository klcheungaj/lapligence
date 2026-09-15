//! Capture.

use super::*;

impl Db {
    /// Build the owned semantic database from a validated Slang snapshot.
    ///
    /// This conversion never reads source files and retains no native owner.
    pub fn from_slang(snapshot: &SlangSnapshot) -> Result<Self, DbError> {
        let type_projector = SlangTypeProjector::new(snapshot)?;
        let ids = snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, NodeId::from_index(index)))
            .collect::<HashMap<_, _>>();
        if ids.len() != snapshot.semantic_nodes.len() {
            return Err(DbError::InvalidSnapshot(
                "duplicate Slang semantic node id".to_owned(),
            ));
        }
        if snapshot
            .semantic_nodes
            .iter()
            .enumerate()
            .any(|(index, node)| node.id != index as u64)
        {
            return Err(DbError::InvalidSnapshot(
                "Slang semantic node ids are not contiguous arena indices".to_owned(),
            ));
        }
        let mut source_identities = HashMap::new();
        for semantic in &snapshot.semantic_nodes {
            let id = ids[&semantic.id];
            for edge in semantic_edges(snapshot, semantic)? {
                if edge.role == SemanticEdgeRole::SourceIdentity {
                    source_identities.insert(id, semantic_id(&ids, edge.target_id)?);
                }
            }
        }

        let mut nodes = Vec::with_capacity(snapshot.semantic_nodes.len());
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
        let semantic_details = snapshot
            .semantic_nodes
            .iter()
            .map(|semantic| semantic.detail.clone())
            .collect();
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
        let mut arrays = HashMap::new();
        let mut event_arrays = HashMap::new();
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
            .map(|semantic| ids[&semantic.id])
            .collect();
        let assertion_formal_directions = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                (semantic.kind == SemanticKind::Argument
                    || (semantic.kind == SemanticKind::Variable
                        && semantic.subkind
                            == crate::ffi::slang::SEMANTIC_VARIABLE_ASSERTION_LOCAL))
                    && (semantic.is_input
                        || semantic.is_output
                        || semantic.is_inout
                        || semantic.is_ref)
            })
            .map(|semantic| (ids[&semantic.id], direction_from_slang(semantic)))
            .collect();
        let mut implicit_nets = HashSet::new();
        let mut implicit_conversions = HashSet::new();
        let mut var_lifetimes = HashMap::new();
        let mut var_lifetime_qualifiers = HashMap::new();
        let mut method_calls_with_clause = HashSet::new();
        let mut method_call_iterators = HashMap::new();
        let mut packed_members = HashMap::new();
        let mut aggregate_layouts = HashMap::new();
        let mut type_descriptors = HashMap::new();
        let mut enum_types = HashMap::new();
        let mut packed_dimensions = HashMap::new();
        let mut clocking_blocks = HashMap::new();
        let mut clocking_vars = HashMap::new();
        let dpi_imports = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                semantic.kind == SemanticKind::Subroutine
                    && semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_IMPORT != 0
            })
            .map(|semantic| {
                let id = ids[&semantic.id];
                let c_name = if semantic.definition_name.is_empty() {
                    semantic.name.clone()
                } else {
                    semantic.definition_name.clone()
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
        for semantic in &snapshot.semantic_nodes {
            let id = ids[&semantic.id];
            let edges = semantic_edges(snapshot, semantic)?;
            let mut children = Vec::new();
            for edge in edges {
                let child_semantic = snapshot
                    .semantic_nodes
                    .get(edge.target_id as usize)
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("semantic child target is missing".into())
                    })?;
                if child_semantic.is_uninstantiated
                    || child_semantic.kind == SemanticKind::Definition
                {
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
            if matches!(
                semantic.kind,
                SemanticKind::Instance | SemanticKind::Scope | SemanticKind::GenerateScope
            ) && semantic.subkind != 194
            {
                let mut flattened = Vec::new();
                let mut expanded = HashSet::new();
                let mut pending = children;
                pending.reverse();
                while let Some(child) = pending.pop() {
                    let child_semantic = &snapshot.semantic_nodes[child.index()];
                    let instance_body =
                        child_semantic.kind == SemanticKind::Scope && child_semantic.subkind == 194;
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
                                        edge.role != SemanticEdgeRole::Reference
                                    }
                                })
                                .filter(|edge| {
                                    snapshot
                                        .semantic_nodes
                                        .get(edge.target_id as usize)
                                        .is_some_and(|node| {
                                            !node.is_uninstantiated
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
            } else if semantic.kind == SemanticKind::Scope && semantic.subkind == 194 {
                children.clear();
            }
            let mut seen_children = HashSet::new();
            children.retain(|child| seen_children.insert(*child));
            let projection = semantic
                .type_id
                .map(|type_id| type_projector.project(type_id))
                .transpose()?;
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
            if semantic.kind == SemanticKind::Net && semantic.is_implicit {
                implicit_nets.insert(id);
            }
            if semantic.is_implicit_conversion {
                implicit_conversions.insert(id);
            }
            if semantic.kind == SemanticKind::MethodCall && semantic.method_with_clause {
                method_calls_with_clause.insert(id);
                if let Some(iterator) = semantic.target_id {
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
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind == SEMANTIC_VARIABLE_CLOCKING
            {
                let initializer = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot(
                            "clocking variable has no source expression".into(),
                        )
                    })?;
                let source = clocking_source_from_expression(snapshot, &ids, initializer, 0)?
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("clocking variable source is unresolved".into())
                    })?;
                let parent_raw = semantic
                    .parent_id
                    .and_then(|parent| snapshot.semantic_nodes.get(parent as usize));
                let block = semantic
                    .parent_id
                    .and_then(|parent| ids.get(&parent).copied())
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
                let lifetime = if semantic.is_automatic {
                    VariableLifetimeQualifier::Automatic
                } else if semantic.is_static {
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
                type_descriptors.insert(id, projection.descriptor.clone());
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
                    semantic.type_id,
                    semantic
                        .constant_id
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
                                name: semantic.name.clone(),
                                value: Val::Bits(value),
                            });
                    }
                }
            }
            let mut parent = if semantic.is_top
                || matches!(
                    semantic.kind,
                    SemanticKind::Definition | SemanticKind::Package | SemanticKind::Class
                ) {
                None
            } else {
                semantic
                    .parent_id
                    .map(|parent| semantic_id(&ids, parent))
                    .transpose()?
            };
            if let Some(parent_id) = parent {
                let parent_semantic = &snapshot.semantic_nodes[parent_id.index()];
                if parent_semantic.kind == SemanticKind::Scope && parent_semantic.subkind == 194 {
                    parent = parent_semantic
                        .parent_id
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                } else if parent_semantic.kind == SemanticKind::Instance
                    && parent_semantic.subkind == 193
                {
                    parent = parent_semantic
                        .parent_id
                        .map(|id| semantic_id(&ids, id))
                        .transpose()?;
                }
            }
            let (file, line, col, end_line, end_col) = source_position(snapshot, semantic)?;
            let mut kind =
                node_kind_from_slang(snapshot, &type_projector, semantic, edges, &ids, type_info)?;
            if semantic.kind == SemanticKind::Net {
                if let Some(delay) = driver_delay(snapshot, &ids, edges)? {
                    net_delays.insert(id, delay);
                }
            }
            if semantic.kind == SemanticKind::Expression && semantic.subkind == 73 {
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
            }
            let target_name = || {
                semantic
                    .target_id
                    .and_then(|target| ids.get(&target))
                    .and_then(|target| snapshot.semantic_nodes.get(target.index()))
                    .map(|target| target.name.clone())
                    .unwrap_or_default()
            };
            let name = if semantic.name.is_empty()
                && ((semantic.kind == SemanticKind::Statement
                    && matches!(semantic.subkind, 32 | 56..=58))
                    || (semantic.kind == SemanticKind::Expression && semantic.subkind == 65))
            {
                target_name()
            } else {
                semantic.name.clone()
            };
            nodes.push(Node {
                kind,
                children,
                parent,
                name,
                full_name: String::new(),
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
            let id = ids[&semantic.id];
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
        let mut virtual_interface_targets = HashMap::new();
        for &variable in vars_init.keys() {
            if let Some(instance) = virtual_interface_instance_from_slang(snapshot, &ids, variable)?
            {
                virtual_interface_targets.insert(variable, instance);
            }
        }

        for index in 0..nodes.len() {
            let full_name = semantic_full_name(&nodes, NodeId::from_index(index))?;
            nodes[index].full_name = full_name;
        }
        let mut elaborated_type_ranges = Vec::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let id = NodeId::from_index(index);
            let Some(type_id) = semantic.type_id else {
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
            let mut ancestor = semantic.parent_id;
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
                ancestor = parent.parent_id;
            }
            if !has_runtime_instance {
                continue;
            }
            let instance = enclosing_scope_name(&nodes, id)
                .unwrap_or_else(|| nodes[id.index()].full_name.clone());
            if !instance.is_empty() {
                elaborated_type_ranges.push(type_projector.elaborated_ranges(
                    id,
                    instance,
                    semantic.name.clone(),
                    type_id,
                )?);
            }
        }
        let tops: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Instance && node.is_top)
            .map(|node| ids[&node.id])
            .collect();
        let flat_modules: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Definition)
            .map(|node| ids[&node.id])
            .collect();
        let packages: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Package)
            .map(|node| ids[&node.id])
            .collect();
        let classes: Vec<NodeId> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| ids[&node.id])
            .collect();
        let class_metadata = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Class)
            .map(|node| -> Result<(NodeId, ClassMetadata), DbError> {
                let id = ids[&node.id];
                Ok((
                    id,
                    ClassMetadata {
                        type_id: node.type_id.map(TypeId),
                        base: node.target_id.and_then(|target| ids.get(&target).copied()),
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
            program_instances,
            tops,
            flat_modules,
            packages,
            classes,
            class_metadata,
            design_name,
            arrays,
            event_arrays,
            array_select_paths,
            vars_init,
            net_delays,
            var_lifetimes,
            var_lifetime_qualifiers,
            method_calls_with_clause,
            method_call_iterators,
            packed_members,
            aggregate_layouts,
            type_descriptors,
            enum_types,
            packed_dimensions,
            two_state_types,
            assertion_local_vars,
            assertion_formal_directions,
            source_identities,
            clocking_blocks,
            clocking_vars,
            modport_directions,
            virtual_interface_targets,
            dpi_imports,
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
