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
        let udp_tables = udp_tables_from_slang(snapshot, &mut source_positions)?;
        let specify = specify_tables_from_slang(snapshot, &ids)?;
        let mut source_identities = HashMap::new();
        // IEEE 1800-2009 37.23: a typedef reference names a declaration with
        // detail `TypeAlias` and a name; any other target (a typedef of a
        // scope the capture did not visit) leaves the type unnamed.
        let typedef_target = |target: u64| -> Result<Option<NodeId>, DbError> {
            let named = usize::try_from(target)
                .ok()
                .and_then(|index| snapshot.semantic_nodes.get(index))
                .is_some_and(|node| {
                    node.detail.as_str() == "TypeAlias" && !node.name.as_str().is_empty()
                });
            Ok(if named {
                Some(semantic_id(&ids, target)?)
            } else {
                None
            })
        };
        let mut typedefs = TypedefNames::default();
        for semantic in &snapshot.semantic_nodes {
            let id = NodeId::from_index(semantic.id as usize);
            if semantic.detail.as_str() == "TypeAlias"
                && semantic.auxiliary & crate::ffi::slang::TYPEDEF_BUILTIN != 0
            {
                typedefs.builtin.insert(id);
            }
            for edge in semantic_edges(snapshot, semantic)? {
                if edge.role == SemanticEdgeRole::SourceIdentity {
                    source_identities.insert(id, semantic_id(&ids, edge.target_id)?);
                }
                if edge.role == SemanticEdgeRole::Typedef {
                    if let Some(typedef) = typedef_target(edge.target_id)? {
                        typedefs.declared.insert(
                            id,
                            TypedefRef {
                                typedef,
                                depth: edge.index,
                            },
                        );
                    }
                }
            }
        }
        for ty in &snapshot.types {
            let members = usize::try_from(ty.member_start)
                .ok()
                .zip(usize::try_from(ty.member_count).ok())
                .and_then(|(start, count)| {
                    snapshot.type_members.get(start..start.checked_add(count)?)
                })
                .ok_or_else(|| {
                    DbError::InvalidSnapshot(format!("type {} member window is invalid", ty.id))
                })?;
            for (index, member) in members.iter().enumerate() {
                let Some((target, depth)) = member.typedef else {
                    continue;
                };
                if let Some(typedef) = typedef_target(target)? {
                    typedefs
                        .members
                        .insert((TypeId(ty.id), index as u32), TypedefRef { typedef, depth });
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
        let mut checker_instances = HashMap::new();
        let mut checker_actuals = HashMap::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            if semantic.kind != SemanticKind::Instance
                || semantic.subkind != crate::ffi::slang::SEMANTIC_INSTANCE_CHECKER
            {
                continue;
            }
            let instance = NodeId::from_index(index);
            checker_instances.insert(
                instance,
                CheckerInstanceInfo {
                    procedural: semantic.auxiliary & crate::ffi::slang::CHECKER_PROCEDURAL != 0,
                    bound: semantic.auxiliary & crate::ffi::slang::CHECKER_BOUND != 0,
                    nested: semantic.auxiliary & crate::ffi::slang::CHECKER_NESTED != 0,
                    unit_time_scale: semantic.auxiliary
                        & crate::ffi::slang::CHECKER_UNIT_TIME_SCALE
                        != 0,
                    default_disable: edge_target(
                        &ids,
                        semantic_edges(snapshot, semantic)?,
                        SemanticEdgeRole::DefaultDisable,
                    )?,
                },
            );
            for edge in semantic_edges(snapshot, semantic)? {
                if edge.role != SemanticEdgeRole::CheckerActual {
                    continue;
                }
                let expression = semantic_id(&ids, edge.target_id)?;
                if !matches!(
                    snapshot.semantic_nodes.get(expression.index()),
                    Some(node) if matches!(
                        node.kind,
                        SemanticKind::Expression
                            | SemanticKind::SystemCall
                            | SemanticKind::MethodCall
                            | SemanticKind::FunctionCall
                    )
                ) {
                    return Err(DbError::InvalidSnapshot(
                        "checker actual does not name an expression".into(),
                    ));
                }
                let actual = CheckerActual {
                    instance,
                    default: edge.index & crate::ffi::slang::CHECKER_ACTUAL_DEFAULT != 0,
                };
                if checker_actuals.insert(expression, actual).is_some() {
                    return Err(DbError::InvalidSnapshot(
                        "checker actual names an expression twice".into(),
                    ));
                }
            }
        }
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
        let instance_configs = snapshot
            .source_libraries
            .iter()
            .filter_map(|binding| Some((binding.semantic_id, binding.config.clone()?)))
            .collect::<HashMap<_, _>>();
        let mut declaration_time_scales = HashMap::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            let namespace = |node: &crate::ffi::slang::SemanticNode| {
                node.kind == SemanticKind::Package || node.detail == "CompilationUnit"
            };
            // A class takes the scale of the nearest enclosing module,
            // package or compilation unit (Slang's `Scope::getTimeScale`).
            // A compilation unit without a time unit declaration reports
            // none, and neither do its classes.
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
        // Class nodes have no owned parent, so the scope that declares each
        // class is kept apart for names such as `%m` (SV 21.2.1.6): the
        // nearest enclosing class, package, compilation unit, instance,
        // generate block or definition. A specialization's parent is its
        // generic class definition, which is passed over: the specialization
        // is declared where the generic class is.
        let mut class_declaring_scopes = HashMap::new();
        for (index, semantic) in snapshot.semantic_nodes.iter().enumerate() {
            if semantic.kind != SemanticKind::Class {
                continue;
            }
            let mut ancestor = semantic.parent_id();
            while let Some(parent) = ancestor {
                let Some(node) = snapshot.semantic_nodes.get(parent as usize) else {
                    return Err(DbError::InvalidSnapshot(format!(
                        "class node {index} has unknown ancestor {parent}"
                    )));
                };
                let generic_class =
                    node.kind == SemanticKind::Class && node.detail == "GenericClassDef";
                if node.detail == "CompilationUnit"
                    || (!generic_class && node.kind == SemanticKind::Class)
                    || matches!(
                        node.kind,
                        SemanticKind::Package
                            | SemanticKind::Instance
                            | SemanticKind::GenerateScope
                            | SemanticKind::Definition
                    )
                {
                    class_declaring_scopes
                        .insert(NodeId::from_index(index), semantic_id(&ids, parent)?);
                    break;
                }
                ancestor = node.parent_id();
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
        let instance_definitions =
            instance_definitions(snapshot, &ids, &mut source_positions, &instance_configs)?;
        let attributes = attribute_instances(snapshot, &ids, &mut source_positions)?;
        let primitive_array_positions = primitive_array_positions(snapshot);
        let instance_array_ranges = instance_array_ranges(snapshot);
        let mut keyword_lines = statement_keyword_lines(snapshot, &mut source_positions)?;
        keyword_lines.extend(port_header_lines(snapshot, &mut source_positions)?);
        let mut arrays = HashMap::new();
        let mut event_arrays = HashMap::new();
        let mut subroutine_arrays = HashMap::new();
        let mut array_select_paths = HashMap::new();
        let mut vars_init = HashMap::new();
        let mut param_inits = HashMap::new();
        let mut assertion_local_initializers = HashMap::new();
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
        let mut implicit_declarations = HashSet::new();
        let mut declaration_qualifiers = HashMap::new();
        let mut implicit_conversions = HashSet::new();
        let mut var_lifetimes = HashMap::new();
        let mut var_lifetime_qualifiers = HashMap::new();
        let mut const_variables = HashSet::new();
        let mut rand_qualifiers = HashMap::new();
        let mut member_visibilities = HashMap::new();
        let mut virtual_prototypes = HashSet::new();
        let mut randomize_calls = HashMap::new();
        let mut method_calls_with_clause = HashSet::new();
        let mut const_casts = HashSet::new();
        let mut parenthesized_assertions = HashSet::new();
        let mut method_call_iterators = HashMap::new();
        let mut packed_members = HashMap::new();
        let mut aggregate_layouts = HashMap::new();
        let mut dpi_open_types = HashMap::new();
        let mut type_descriptors = DenseSharedBuilder::new(snapshot.semantic_nodes.len());
        // A projection depends only on its frontend type record, so each
        // distinct type is projected once and shared by every typed node.
        let mut projections: HashMap<u64, std::rc::Rc<_>> = HashMap::new();
        let mut packed_pattern_elements = HashMap::new();
        let mailbox_elements = type_projector
            .mailbox_elements()
            .map_err(DbError::InvalidSnapshot)?;
        let packed_array_elements = type_projector
            .packed_array_elements()
            .map_err(DbError::InvalidSnapshot)?;
        let mut enum_types = HashMap::new();
        let mut packed_dimensions = HashMap::new();
        let mut clocking_blocks = HashMap::new();
        let mut coverage = CoverageImport::default();
        let mut clocking_vars = HashMap::new();
        let mut default_clockings = HashMap::new();
        let mut instance_default_disables = HashMap::new();
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
        // The export's C identifier travels in the linkage-name field; an
        // empty one falls back to the subroutine name (SV 35.4).
        let dpi_exports = snapshot
            .semantic_nodes
            .iter()
            .filter(|semantic| {
                semantic.kind == SemanticKind::Subroutine
                    && semantic.auxiliary & crate::ffi::slang::SUBROUTINE_DPI_EXPORT != 0
            })
            .map(|semantic| {
                let c_name = if semantic.definition_name.is_empty() {
                    semantic.name.to_string()
                } else {
                    semantic.definition_name.to_string()
                };
                (NodeId::from_index(semantic.id as usize), c_name)
            })
            .collect::<HashMap<_, _>>();
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
                // A checker instance's instantiation-context disable
                // condition is metadata (`checker_instances`), not a member.
                if semantic.kind == SemanticKind::Instance
                    && edge.role == SemanticEdgeRole::DefaultDisable
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
                if child_semantic.kind == SemanticKind::Production
                    && child_semantic.subkind == crate::ffi::slang::SEMANTIC_PRODUCTION_DECLARATION
                    && semantic.kind != SemanticKind::Statement
                {
                    // A randsequence production is a member of the statement's
                    // block scope, and that scope is also a child of the
                    // enclosing statement. Productions own executable code, so
                    // only the randsequence statement lists them as children;
                    // a second path would make tree walks visit each code
                    // block more than once.
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
                        | SemanticEdgeRole::CheckerActual
                        | SemanticEdgeRole::ProceduralChecker
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
                                        // child; the default disable
                                        // expression is instance metadata.
                                        edge.role != SemanticEdgeRole::Reference
                                            && edge.role != SemanticEdgeRole::Clocking
                                            && edge.role != SemanticEdgeRole::DefaultDisable
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
            if semantic.kind == SemanticKind::Coverage {
                coverage.import(snapshot, &ids, id, semantic, edges, &type_info)?;
            }
            if matches!(
                semantic.kind,
                SemanticKind::Net | SemanticKind::GenerateScope
            ) && semantic.is_implicit()
            {
                implicit_declarations.insert(id);
            }
            let qualifiers = match semantic.kind {
                SemanticKind::Net => super::DeclarationQualifiers {
                    vectored: semantic.auxiliary & crate::ffi::slang::NET_VECTORED != 0,
                    scalared: semantic.auxiliary & crate::ffi::slang::NET_SCALARED != 0,
                    ..Default::default()
                },
                SemanticKind::Port => super::DeclarationQualifiers {
                    explicit_port_name: semantic.auxiliary & crate::ffi::slang::PORT_EXPLICIT_NAME
                        != 0,
                    connected_by_name: semantic.auxiliary
                        & crate::ffi::slang::PORT_CONNECTED_BY_NAME
                        != 0,
                    ..Default::default()
                },
                _ => super::DeclarationQualifiers::default(),
            };
            if qualifiers != super::DeclarationQualifiers::default() {
                declaration_qualifiers.insert(id, qualifiers);
            }
            if semantic.is_implicit_conversion() {
                implicit_conversions.insert(id);
            }
            if matches!(
                semantic.kind,
                SemanticKind::SystemCall | SemanticKind::MethodCall
            ) {
                let form = if semantic.auxiliary & crate::ffi::slang::CALL_CLASS_RANDOMIZE != 0 {
                    Some(RandomizeForm::Class)
                } else if semantic.auxiliary & crate::ffi::slang::CALL_SCOPE_RANDOMIZE != 0 {
                    Some(RandomizeForm::Scope)
                } else {
                    None
                };
                if let Some(form) = form {
                    randomize_calls.insert(
                        id,
                        RandomizeCall {
                            form,
                            inline: edge_target(&ids, edges, SemanticEdgeRole::Body)?,
                        },
                    );
                }
            }
            if semantic.kind == SemanticKind::Expression
                && semantic.subkind == 72
                && semantic.auxiliary & crate::ffi::slang::CONVERSION_CONST != 0
            {
                const_casts.insert(id);
            }
            if semantic.kind == SemanticKind::AssertionExpr
                && semantic.auxiliary & crate::ffi::slang::SEMANTIC_ASSERTION_PARENTHESIZED != 0
            {
                parenthesized_assertions.insert(id);
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
                    if let Some(disable) = edge_target(
                        &ids,
                        semantic_edges(snapshot, body)?,
                        SemanticEdgeRole::DefaultDisable,
                    )? {
                        instance_default_disables.insert(id, disable);
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
            if semantic.kind == SemanticKind::Subroutine {
                if semantic.auxiliary & crate::ffi::slang::SUBROUTINE_PROTOTYPE_VIRTUAL != 0 {
                    virtual_prototypes.insert(id);
                }
                if semantic.auxiliary & crate::ffi::slang::SUBROUTINE_LOCAL != 0 {
                    member_visibilities.insert(id, MemberVisibility::Local);
                } else if semantic.auxiliary & crate::ffi::slang::SUBROUTINE_PROTECTED != 0 {
                    member_visibilities.insert(id, MemberVisibility::Protected);
                }
            }
            if matches!(
                semantic.kind,
                SemanticKind::Variable | SemanticKind::NamedEvent
            ) && semantic.subkind != 229
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
            {
                if semantic.auxiliary & crate::ffi::slang::VARIABLE_CONST != 0 {
                    const_variables.insert(id);
                }
                if semantic.auxiliary & crate::ffi::slang::VARIABLE_LOCAL != 0 {
                    member_visibilities.insert(id, MemberVisibility::Local);
                } else if semantic.auxiliary & crate::ffi::slang::VARIABLE_PROTECTED != 0 {
                    member_visibilities.insert(id, MemberVisibility::Protected);
                }
                if semantic.auxiliary & crate::ffi::slang::VARIABLE_RAND != 0 {
                    rand_qualifiers.insert(id, RandQualifier::Rand);
                } else if semantic.auxiliary & crate::ffi::slang::VARIABLE_RANDC != 0 {
                    rand_qualifiers.insert(id, RandQualifier::RandC);
                }
                let resolved_lifetime =
                    match semantic.auxiliary & crate::ffi::slang::VARIABLE_LIFETIME_MASK {
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
            // The default expression of a value parameter (IEEE 1800-2009
            // 37.26 detail 3), as the frontend binds it.
            if semantic.kind == SemanticKind::Parameter {
                if let Some(initializer) = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                {
                    param_inits.insert(id, initializer);
                }
            }
            if semantic.kind == SemanticKind::Variable
                && semantic.subkind != SEMANTIC_VARIABLE_CLOCKING
                && !is_array
            {
                if let Some(initializer) = edge_target(&ids, edges, SemanticEdgeRole::Initializer)?
                {
                    if semantic.subkind == crate::ffi::slang::SEMANTIC_VARIABLE_ASSERTION_LOCAL {
                        assertion_local_initializers.insert(id, initializer);
                    } else {
                        vars_init.insert(id, initializer);
                    }
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
                if let Some(open) = &projection.dpi_open {
                    dpi_open_types.insert(id, open.clone());
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
            let mut kind = node_kind_from_slang(
                snapshot,
                &type_projector,
                semantic,
                edges,
                &ids,
                type_info,
                &udp_tables,
            )?;
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
                if assignment_node.kind == SemanticKind::Expression
                    && assignment_node.detail == "Invalid"
                {
                    // Slang replaces an assignment it has already diagnosed
                    // (an undeclared name, or a reference into an unknown
                    // module's black box) with an operand-less invalid
                    // expression. Record the assignment as an unsupported
                    // `Other` node so the rest of the design stays
                    // importable for editor analysis; the simulator never
                    // gets here because it stops on the frontend error.
                    kind = NodeKind::Other;
                    children = vec![assignment];
                } else {
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
                }
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

        // An output variable port declaration may carry an initializer
        // (IEEE 1800-2009 A.2.1.2 `[= constant_expression]`; IEEE 1364-2001
        // 12.3.3). Slang binds it on the port, not on the internal variable,
        // so record it as that variable's declaration initializer: the
        // variable then initializes like any other static variable (6.8).
        for semantic in &snapshot.semantic_nodes {
            if semantic.kind != SemanticKind::Port || !semantic.is_output() {
                continue;
            }
            let edges = semantic_edges(snapshot, semantic)?;
            let Some(initializer) = edge_target(&ids, edges, SemanticEdgeRole::Initializer)? else {
                continue;
            };
            let Some(internal) = edge_target(&ids, edges, SemanticEdgeRole::LowConnection)? else {
                continue;
            };
            let Some(variable) = port_internal_variable(snapshot, &ids, internal)? else {
                continue;
            };
            let target = &snapshot.semantic_nodes[variable.index()];
            if let Some(array) = arrays.get_mut(&variable) {
                array.init.get_or_insert(initializer);
            } else if target.kind == SemanticKind::Variable
                && target.subkind != SEMANTIC_VARIABLE_CLOCKING
                && target.subkind != crate::ffi::slang::SEMANTIC_VARIABLE_ASSERTION_LOCAL
            {
                vars_init.entry(variable).or_insert(initializer);
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
        // A port whose port expression is not a whole internal symbol (a
        // select or an explicit ANSI expression, SV 23.2.2.1-23.2.2.2) keeps
        // that expression as its low connection, and a multi-port lists its
        // component ports there, most significant first. Neither edge is a
        // child, so record them beside the port.
        let mut port_expressions = HashMap::new();
        let mut multi_ports = HashMap::new();
        for semantic in &snapshot.semantic_nodes {
            if semantic.kind != SemanticKind::Port {
                continue;
            }
            let port = NodeId::from_index(semantic.id as usize);
            let edges = semantic_edges(snapshot, semantic)?;
            if semantic.detail == "MultiPort" {
                let mut components = edges
                    .iter()
                    .filter(|edge| edge.role == SemanticEdgeRole::LowConnection)
                    .map(|edge| Ok((edge.index, semantic_id(&ids, edge.target_id)?)))
                    .collect::<Result<Vec<_>, DbError>>()?;
                components.sort_by_key(|(index, _)| *index);
                multi_ports.insert(
                    port,
                    components
                        .into_iter()
                        .map(|(_, component)| component)
                        .collect(),
                );
                continue;
            }
            if let Some(internal) = port_internal_expression(snapshot, &ids, edges)? {
                port_expressions.insert(port, internal);
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
        hoist_procedural_checkers(&mut nodes, &checker_instances);
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
        let coverage = coverage.finish(&nodes)?;
        let fsm_tables = fsm::FsmTables::from_slang(snapshot, &nodes, &mut source_positions)?;
        let db = Self {
            nodes,
            edition: snapshot.edition(),
            delay_selection: snapshot.delay_selection,
            overridden_parameters,
            semantic_kinds,
            semantic_details,
            conditional_patterns,
            conditional_pattern_fields,
            program_instances,
            checker_instances,
            checker_actuals,
            unconnected_drives,
            instance_definitions,
            attributes,
            primitive_array_positions,
            instance_array_ranges,
            keyword_lines,
            source_libraries,
            declaration_time_scales,
            class_declaring_scopes,
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
            param_inits,
            net_delays,
            var_lifetimes,
            var_lifetime_qualifiers,
            const_variables,
            rand_qualifiers,
            member_visibilities,
            virtual_prototypes,
            randomize_calls,
            method_calls_with_clause,
            const_casts,
            parenthesized_assertions,
            method_call_iterators,
            packed_members,
            aggregate_layouts,
            dpi_open_types,
            type_descriptors: type_descriptors.finish(),
            packed_pattern_elements,
            mailbox_elements,
            enum_types,
            typedefs,
            packed_array_elements,
            packed_dimensions,
            two_state_types,
            assertion_local_vars,
            assertion_formal_directions,
            assertion_local_initializers,
            source_identities,
            clocking_blocks,
            coverage,
            clocking_vars,
            default_clockings,
            instance_default_disables,
            modport_directions,
            modport_expressions,
            port_expressions,
            multi_ports,
            virtual_interface_targets,
            dpi_imports,
            dpi_exports,
            implicit_declarations,
            declaration_qualifiers,
            implicit_conversions,
            source_files: snapshot
                .files
                .iter()
                .map(|file| (file.name.clone(), file.text.clone()))
                .collect(),
            elaborated_type_ranges,
            specify,
            fsms: fsm_tables.fsms,
            fsm_pragma_errors: fsm_tables.errors,
        };
        db.validate().map_err(DbError::InvalidDatabase)?;
        Ok(db)
    }
}

/// The variable a port's internal connection names: the internal symbol
/// itself, or the variable a non-ANSI port expression references through
/// selects. Nets and anything else yield `None`.
fn port_internal_variable(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    internal: NodeId,
) -> Result<Option<NodeId>, DbError> {
    let mut current = internal;
    for _ in 0..=snapshot.semantic_nodes.len() {
        let node = &snapshot.semantic_nodes[current.index()];
        match node.kind {
            SemanticKind::Variable | SemanticKind::Array => return Ok(Some(current)),
            SemanticKind::Expression if node.subkind == 65 => {
                let Some(target) = expression_reference_target(snapshot, ids, node)? else {
                    return Ok(None);
                };
                current = target;
            }
            SemanticKind::Expression => {
                let edges = semantic_edges(snapshot, node)?;
                let Some(base) = edge_target(ids, edges, SemanticEdgeRole::Base)? else {
                    return Ok(None);
                };
                current = base;
            }
            _ => return Ok(None),
        }
    }
    Err(DbError::InvalidSnapshot(
        "port connection chain contains a cycle".into(),
    ))
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

/// The definition facts of every module, interface and program instance:
/// its definition's location (the node its target names), `` `celldefine``,
/// default lifetime and `` `default_nettype`` from the instance auxiliary,
/// and the configuration controlling its binding.
fn instance_definitions(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    positions: &mut SourcePositions<'_>,
    configs: &HashMap<u64, String>,
) -> Result<HashMap<NodeId, InstanceDefinition>, DbError> {
    use crate::ffi::slang::{
        INSTANCE_AUTOMATIC, INSTANCE_CELL_DEFINE, INSTANCE_NET_TYPE_MASK, INSTANCE_NET_TYPE_SHIFT,
    };
    let mut definitions = HashMap::new();
    for semantic in snapshot.semantic_nodes.iter() {
        // A package is its own definition (IEEE 1800-2009 37.10).
        let definition = match semantic.kind {
            SemanticKind::Package => Some(semantic),
            SemanticKind::Instance
                if semantic.subkind == crate::ffi::slang::SEMANTIC_INSTANCE_SINGLE =>
            {
                semantic
                    .target_id()
                    .and_then(|target| snapshot.semantic_nodes.get(target as usize))
            }
            _ => None,
        };
        let Some(definition) = definition else {
            continue;
        };
        let (file, line, _, _, _) = positions.position(definition)?;
        let net_code =
            u32::try_from((semantic.auxiliary >> INSTANCE_NET_TYPE_SHIFT) & INSTANCE_NET_TYPE_MASK)
                .unwrap_or(0);
        definitions.insert(
            semantic_id(ids, semantic.id)?,
            InstanceDefinition {
                file,
                line,
                cell_define: semantic.auxiliary & INSTANCE_CELL_DEFINE != 0,
                automatic: semantic.auxiliary & INSTANCE_AUTOMATIC != 0,
                default_net_type: if net_code == 0 {
                    NetType::None
                } else {
                    net_type_from_subkind(net_code)
                },
                config: configs.get(&semantic.id).cloned(),
            },
        );
    }
    Ok(definitions)
}

/// The attribute instances of every owner. A single instance lists its own
/// attributes, then those of the definition it instantiates.
fn attribute_instances(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    positions: &mut SourcePositions<'_>,
) -> Result<HashMap<NodeId, Vec<super::AttributeInstance>>, DbError> {
    if snapshot.attributes.is_empty() {
        return Ok(HashMap::new());
    }
    let mut own: HashMap<u64, Vec<super::AttributeInstance>> = HashMap::new();
    for attribute in &snapshot.attributes {
        let constant = snapshot
            .constants
            .get(attribute.constant_id as usize)
            .ok_or_else(|| DbError::InvalidSnapshot("attribute value is missing".into()))?;
        let value = match (&constant.value, attribute.string) {
            (
                crate::ffi::slang::ConstantValue::Integer {
                    bit_width,
                    value_words,
                    ..
                },
                true,
            ) => string_literal_val(*bit_width, value_words),
            (value, _) => val_from_slang(value),
        }
        .ok_or_else(|| DbError::InvalidSnapshot("attribute value is not a constant".into()))?;
        let (file, line) = match attribute.range {
            Some(range) => {
                let (file, line, _, _, _) = positions.range_position(range)?;
                (file, line)
            }
            None => (None, 0),
        };
        own.entry(attribute.owner_id)
            .or_default()
            .push(super::AttributeInstance {
                name: attribute.name.clone(),
                value,
                definition: false,
                file,
                line,
            });
    }
    let mut attributes = HashMap::new();
    for (&owner, list) in &own {
        let node = snapshot
            .semantic_nodes
            .get(owner as usize)
            .ok_or_else(|| DbError::InvalidSnapshot("attribute owner is missing".into()))?;
        if node.kind == SemanticKind::Definition {
            continue;
        }
        attributes.insert(semantic_id(ids, owner)?, list.clone());
    }
    for semantic in &snapshot.semantic_nodes {
        if semantic.kind != SemanticKind::Instance
            || semantic.subkind != crate::ffi::slang::SEMANTIC_INSTANCE_SINGLE
        {
            continue;
        }
        let Some(definition) = semantic.target_id().and_then(|target| own.get(&target)) else {
            continue;
        };
        attributes
            .entry(semantic_id(ids, semantic.id)?)
            .or_insert_with(Vec::new)
            .extend(definition.iter().map(|attribute| super::AttributeInstance {
                definition: true,
                ..attribute.clone()
            }));
    }
    Ok(attributes)
}

/// The text of a string literal held as packed bits, eight per character
/// with the first character most significant (IEEE 1800-2009 5.9); the
/// zero bytes of padding are dropped.
fn string_literal_val(bit_width: u64, words: &[u64]) -> Option<Val> {
    let bytes = (0..bit_width.div_ceil(8))
        .rev()
        .map(|byte| {
            let word = words
                .get(usize::try_from(byte / 8).ok()?)
                .copied()
                .unwrap_or(0);
            u8::try_from((word >> ((byte % 8) * 8)) & 0xff).ok()
        })
        .collect::<Option<Vec<u8>>>()?;
    let text: Vec<u8> = bytes.into_iter().filter(|byte| *byte != 0).collect();
    Val::from_string_bytes(&text)
}

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

/// The slice of every primitive instance array element (IEEE 1800-2009
/// 28.3.6), as the frontend exported it.
fn primitive_array_positions(
    snapshot: &SlangSnapshot,
) -> HashMap<NodeId, super::PrimitiveArrayPosition> {
    snapshot
        .semantic_nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.kind == SemanticKind::Primitive
                && node.is_primitive_instance()
                && node.auxiliary != 0
        })
        .filter_map(|(index, node)| {
            let count = u32::try_from(node.auxiliary >> 32).ok()?;
            let slice = u32::try_from(node.auxiliary & u64::from(u32::MAX)).ok()?;
            (slice < count).then(|| {
                (
                    NodeId::from_index(index),
                    super::PrimitiveArrayPosition { slice, count },
                )
            })
        })
        .collect()
}

/// The declared range of every instance array level, as the frontend
/// exported it (left bound in the low half of `auxiliary`).
fn instance_array_ranges(snapshot: &SlangSnapshot) -> HashMap<NodeId, (i32, i32)> {
    snapshot
        .semantic_nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == SemanticKind::Instance && node.subkind == 193)
        .map(|(index, node)| {
            let left = (node.auxiliary & u64::from(u32::MAX)) as u32 as i32;
            let right = (node.auxiliary >> 32) as u32 as i32;
            (NodeId::from_index(index), (left, right))
        })
        .collect()
}

/// The keyword lines blocks and subroutine body lists carry in
/// `auxiliary` (a byte offset + 1): a block's `begin`/`fork` line when its
/// range starts on another line, and the line where a body list opens.
fn statement_keyword_lines(
    snapshot: &SlangSnapshot,
    positions: &mut SourcePositions<'_>,
) -> Result<HashMap<NodeId, u32>, DbError> {
    let mut line_at = |file_id: u64, offset: u64| -> Result<u32, DbError> {
        let at = crate::ffi::slang::SourceRange {
            file_id,
            start: offset,
            end: offset,
        };
        positions.range_position(at).map(|(_, line, _, _, _)| line)
    };
    let mut lines = HashMap::new();
    for (index, node) in snapshot.semantic_nodes.iter().enumerate() {
        if node.kind != SemanticKind::Statement || node.auxiliary == 0 {
            continue;
        }
        let offset = node.auxiliary - 1;
        match node.subkind {
            crate::ffi::slang::SEMANTIC_STMT_BLOCK | 56..=58 => {
                let Some(range) = node.range else { continue };
                let keyword = line_at(range.file_id, offset)?;
                if keyword != line_at(range.file_id, range.start)? {
                    lines.insert(NodeId::from_index(index), keyword);
                }
            }
            crate::ffi::slang::SEMANTIC_STMT_LIST => {
                let parent = node
                    .parent_id()
                    .and_then(|parent| usize::try_from(parent).ok())
                    .and_then(|parent| snapshot.semantic_nodes.get(parent))
                    .and_then(|parent| parent.range);
                let Some(range) = parent else { continue };
                lines.insert(NodeId::from_index(index), line_at(range.file_id, offset)?);
            }
            _ => {}
        }
    }
    Ok(lines)
}

/// The module header line of each non-ANSI port whose io declaration, its
/// node range, is elsewhere (IEEE 1800-2009 23.2.2.1).
fn port_header_lines(
    snapshot: &SlangSnapshot,
    positions: &mut SourcePositions<'_>,
) -> Result<HashMap<NodeId, u32>, DbError> {
    let mut lines = HashMap::new();
    for (index, node) in snapshot.semantic_nodes.iter().enumerate() {
        let offset = node.auxiliary >> crate::ffi::slang::PORT_HEADER_OFFSET_SHIFT;
        if node.kind != SemanticKind::Port || offset == 0 {
            continue;
        }
        let Some(range) = node.range else { continue };
        let at = crate::ffi::slang::SourceRange {
            file_id: range.file_id,
            start: offset - 1,
            end: offset - 1,
        };
        let (_, line, _, _, _) = positions.range_position(at)?;
        lines.insert(NodeId::from_index(index), line);
    }
    Ok(lines)
}

/// Make each procedural checker instance a child of the instance or generate
/// scope that encloses its instantiation statement. The frontend keeps it as
/// a member of the statement block around the statement, but all its
/// contents other than static assertions "are considered to exist during
/// every time step, regardless of whether the checker is static or
/// procedural" (IEEE 1800-2009 17.3.1), as in a static instance. Runs after
/// full names are assigned, so the instance keeps its hierarchical name
/// through named blocks.
fn hoist_procedural_checkers(nodes: &mut [Node], checkers: &HashMap<NodeId, CheckerInstanceInfo>) {
    let mut moves = Vec::new();
    for (&instance, info) in checkers {
        if !info.procedural {
            continue;
        }
        let Some(block) = nodes[instance.index()].parent else {
            continue;
        };
        let mut owner = Some(block);
        while let Some(scope) = owner {
            if !matches!(nodes[scope.index()].kind, NodeKind::Stmt(StmtKind::Begin)) {
                break;
            }
            owner = nodes[scope.index()].parent;
        }
        let Some(owner) = owner.filter(|owner| {
            *owner != block
                && matches!(
                    nodes[owner.index()].kind,
                    NodeKind::ModuleInst { .. } | NodeKind::GenScope
                )
        }) else {
            continue;
        };
        moves.push((instance, block, owner));
    }
    // Instances keep their capture order among the owner's children.
    moves.sort_unstable_by_key(|(instance, _, _)| instance.index());
    for (instance, block, owner) in moves {
        nodes[block.index()]
            .children
            .retain(|child| *child != instance);
        nodes[owner.index()].children.push(instance);
        nodes[instance.index()].parent = Some(owner);
    }
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

    /// A continuous assignment Slang already rejected (here a reference into
    /// an unknown module's black box, then an undeclared name) imports as an
    /// unsupported `Other` node instead of failing the whole database, so the
    /// editor keeps the rest of the design.
    #[test]
    fn diagnosed_invalid_continuous_assignments_import_as_other() {
        let source = "module top(input logic c, output logic o, output logic p, output logic q);\n\
            missing_mod u_m(.a(c));\n\
            assign o = u_m.sig;\n\
            assign p = nope;\n\
            assign q = c;\n\
            endmodule\n";
        let output = crate::core::compile::compile(&crate::core::compile::CompileOpts {
            sources: vec![crate::core::compile::OwnedSource::compilation_unit(
                "invalid_assign.sv",
                source,
            )],
            ..Default::default()
        })
        .unwrap();
        assert!(
            !output.ok(),
            "Slang reports the unknown module and the name"
        );
        let db = Db::from_slang(&output.snapshot).unwrap();
        let assignments = |want_other: bool| {
            db.node_ids()
                .filter(|id| db.semantic_detail(*id) == Some("ContinuousAssign"))
                .filter(|id| matches!(db.node_kind(*id), NodeKind::Other) == want_other)
                .count()
        };
        assert_eq!(assignments(true), 2, "both rejected assignments are Other");
        assert_eq!(assignments(false), 1, "the valid assignment stays typed");
        assert!(db
            .node_ids()
            .any(|id| matches!(db.node_kind(id), NodeKind::ContAssign { .. })));
        // A valid design never takes the tolerant path.
        let valid = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "valid_assign.sv",
                "module top(input logic c, output logic q); assign q = c; endmodule",
            )],
            &crate::core::compile::CompileOpts::default(),
        )
        .unwrap();
        let db = Db::from_slang(&valid.snapshot).unwrap();
        assert!(db
            .node_ids()
            .filter(|id| db.semantic_detail(*id) == Some("ContinuousAssign"))
            .all(|id| matches!(db.node_kind(id), NodeKind::ContAssign { .. })));
    }

    #[test]
    fn output_variable_port_initializers_become_declaration_initializers() {
        let source = "module child(output logic o = 1'b1, output logic [3:0] ua [2] = '{1, 2}, \
            output wire w, input logic i = 1'b0); endmodule \
            module legacy(q); output logic [3:0] q = 4'h6; endmodule \
            module top; wire o, w; logic [3:0] ua [2]; wire [3:0] q; \
            child c(.o(o), .ua(ua), .w(w)); legacy l(.q(q)); endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "port_initializers.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let storage = |path: &str| {
            db.node_ids()
                .find(|id| {
                    db.node(*id).full_name() == path
                        && matches!(
                            db.node_kind(*id),
                            NodeKind::Var { .. } | NodeKind::Array { .. }
                        )
                })
                .unwrap_or_else(|| panic!("missing {path}"))
        };
        for path in ["top.c.o", "top.l.q"] {
            assert!(
                db.var_initializer(storage(path)).is_some(),
                "{path} keeps its port initializer"
            );
        }
        assert!(db
            .array_meta(storage("top.c.ua"))
            .and_then(|meta| meta.init)
            .is_some());
        // An input port's value is a default for an unconnected port
        // (23.2.2.4), never a variable initializer.
        assert!(db.var_initializer(storage("top.c.i")).is_none());
    }

    #[test]
    fn instance_arrays_keep_their_declared_ranges() {
        // IEEE 1800-2009 37.11: every array level reports its own bounds.
        let source = "module leaf(input logic a); endmodule \
            module top; logic [3:0] x; wire [3:0] y; \
            leaf up [3:1] (.a(x[0])); leaf grid [0:1][5:4] (.a(x[1])); \
            nand ng [0:3] (y, x, x); endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "arrays.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let mut ranges = db
            .node_ids()
            .filter(|id| matches!(db.node_kind(*id), NodeKind::InstanceArray))
            .map(|id| (db.node(id).name.clone(), db.instance_array_range(id)))
            .collect::<Vec<_>>();
        ranges.sort();
        assert_eq!(
            ranges,
            vec![
                (String::new(), Some((5, 4))),
                (String::new(), Some((5, 4))),
                ("grid".to_owned(), Some((0, 1))),
                ("ng".to_owned(), Some((0, 3))),
                ("up".to_owned(), Some((3, 1))),
            ]
        );
        let names = db
            .node_ids()
            .filter(|id| matches!(db.node_kind(*id), NodeKind::ModuleInst { .. }))
            .map(|id| db.node(id).name.clone())
            .filter(|name| name.starts_with("grid["))
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"grid[1][4]".to_owned()));
    }

    #[test]
    fn typedef_references_name_declared_types_aliases_and_members() {
        // IEEE 1800-2009 37.23 details 1, 6 and 8.
        let source = "package p; typedef logic [3:0] nib_t; endpackage \
            module top; \
            typedef enum bit [0:2] {red, yellow, blue} primary_colors; \
            typedef primary_colors colors; \
            typedef time my_time; \
            typedef struct packed { int i1; bit bvec; } [1:3] parrtype; \
            typedef parrtype [2:1] parrtype2; \
            typedef parrtype2 unparrtype [6:4]; \
            typedef struct { p::nib_t n; p::nib_t pair [2]; int plain; } rec_t; \
            colors c; my_time t; unparrtype arr [3:0]; rec_t r; logic [7:0] v; \
            initial $display(\"%0d %0d %0d %0d\", c, t, arr[0][4], r.plain, v); endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "typedefs.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let declaration = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(
                        db.node_kind(*id),
                        NodeKind::Var { .. } | NodeKind::Array { .. }
                    ) && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("variable `{name}`"))
        };
        let named = |reference: Option<TypedefRef>| {
            reference.map(|reference| (db.node(reference.typedef).name.clone(), reference.depth))
        };
        let typedef_of = |name: &str| named(db.declared_typedef(declaration(name)));
        assert_eq!(typedef_of("c"), Some(("colors".to_owned(), 0)));
        assert_eq!(typedef_of("t"), Some(("my_time".to_owned(), 0)));
        assert_eq!(typedef_of("arr"), Some(("unparrtype".to_owned(), 1)));
        assert_eq!(typedef_of("r"), Some(("rec_t".to_owned(), 0)));
        assert_eq!(typedef_of("v"), None);
        // Each typedef names the typedef it aliases, at its own depth.
        let chain = |reference: TypedefRef| db.declared_typedef(reference.typedef);
        let colors = db.declared_typedef(declaration("c")).unwrap();
        assert_eq!(named(chain(colors)), Some(("primary_colors".to_owned(), 0)));
        let primary = chain(colors).unwrap();
        assert_eq!(chain(primary), None);
        assert!(!db.typedef_renames_builtin(primary.typedef));
        let my_time = db.declared_typedef(declaration("t")).unwrap();
        assert!(db.typedef_renames_builtin(my_time.typedef));
        assert_eq!(chain(my_time), None);
        let unparrtype = db.declared_typedef(declaration("arr")).unwrap();
        assert_eq!(named(chain(unparrtype)), Some(("parrtype2".to_owned(), 1)));
        let parrtype2 = chain(unparrtype).unwrap();
        assert_eq!(named(chain(parrtype2)), Some(("parrtype".to_owned(), 1)));
        assert_eq!(chain(chain(parrtype2).unwrap()), None);
        // Aggregate members keep their typedef by member position.
        let layout = db.aggregate_layout(declaration("r")).unwrap();
        let aggregate = layout.type_id.unwrap();
        assert_eq!(
            named(db.member_typedef(aggregate, 0)),
            Some(("nib_t".to_owned(), 0))
        );
        assert_eq!(
            named(db.member_typedef(aggregate, 1)),
            Some(("nib_t".to_owned(), 1))
        );
        assert_eq!(db.member_typedef(aggregate, 2), None);
        // Packed arrays of structures keep their element structure.
        let packed = db.type_descriptor(parrtype2.typedef).unwrap();
        let element = db.packed_array_element(packed.id).unwrap();
        assert!(matches!(element.shape, TypeShape::PackedAtom { ref ranges } if ranges.len() == 1));
        let base = db.packed_array_element(element.id).unwrap();
        assert!(matches!(base.shape, TypeShape::Aggregate(_)));
    }

    #[test]
    fn const_qualifier_is_captured_for_variables_and_class_properties() {
        let source = "module top; class C; const int k = 1; int v; endclass \
            const int cq[$] = '{1}; const string cs = \"a\"; int plain = 2; \
            const C ch = new; \
            function automatic int f(); const int local_c = 3; int local_v = 4; \
            return local_c + local_v; endfunction \
            initial $display(\"%0d %0d %s %0d\", cq.size(), plain, cs, f()); endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "const_variables.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let named = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(
                        db.node_kind(*id),
                        NodeKind::Var { .. } | NodeKind::Array { .. }
                    ) && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("variable `{name}`"))
        };
        for name in ["k", "cq", "cs", "ch", "local_c"] {
            assert!(db.is_const_variable(named(name)), "`{name}` is const");
        }
        for name in ["v", "plain", "local_v"] {
            assert!(!db.is_const_variable(named(name)), "`{name}` is not const");
        }
        // The qualifier bit leaves the resolved lifetime intact.
        assert_eq!(db.variable_lifetime(named("cq")), VariableLifetime::Static);
        assert_eq!(
            db.variable_lifetime(named("local_c")),
            VariableLifetime::Automatic
        );
    }

    #[test]
    fn class_member_visibility_is_captured() {
        let source = "module top; class C; local int l; protected rand int p; int q; \
            local function void lf(); endfunction protected task pt(); endtask \
            function void f(); endfunction endclass endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "visibility.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let node = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(
                        db.node_kind(*id),
                        NodeKind::Var { .. } | NodeKind::FuncTask { .. }
                    ) && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("member `{name}`"))
        };
        assert_eq!(db.member_visibility(node("l")), MemberVisibility::Local);
        assert_eq!(db.member_visibility(node("p")), MemberVisibility::Protected);
        assert_eq!(db.rand_qualifier(node("p")), RandQualifier::Rand);
        assert_eq!(db.member_visibility(node("q")), MemberVisibility::Public);
        assert_eq!(db.member_visibility(node("lf")), MemberVisibility::Local);
        assert_eq!(
            db.member_visibility(node("pt")),
            MemberVisibility::Protected
        );
        assert_eq!(db.member_visibility(node("f")), MemberVisibility::Public);
    }

    #[test]
    fn rand_qualifiers_constraint_blocks_and_randomize_calls_are_typed() {
        let source = "module top; class B; rand bit [7:0] a; randc bit [3:0] c; int s; \
            static rand int sr; \
            constraint ca { a < 10; if (s > 0) a != 3; else a != 4; (a == 1) -> c != 2; } \
            constraint proto; static constraint sc { sr > 0; } \
            function int go(); return randomize(); endfunction endclass \
            constraint B::proto { a > 1; } \
            class D extends B; constraint ca { a < 20; } endclass \
            int x, y, ok; initial begin static D d = new; \
            ok = d.randomize() with { a > x; }; ok = std::randomize(x, y) with { x < y; }; \
            ok = d.randomize(null); end endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "randomization.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let var = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(db.node_kind(*id), NodeKind::Var { .. }) && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("variable `{name}`"))
        };
        assert_eq!(db.rand_qualifier(var("a")), RandQualifier::Rand);
        assert_eq!(db.rand_qualifier(var("c")), RandQualifier::RandC);
        assert_eq!(db.rand_qualifier(var("sr")), RandQualifier::Rand);
        assert_eq!(db.rand_qualifier(var("s")), RandQualifier::None);
        assert_eq!(db.rand_qualifier(var("x")), RandQualifier::None);

        let blocks = db
            .node_ids()
            .filter_map(|id| match db.node_kind(id) {
                NodeKind::ConstraintBlock(info) => Some((db.node(id).name.clone(), *info)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let block = |name: &str, nth: usize| {
            blocks
                .iter()
                .filter(|(block, _)| block == name)
                .nth(nth)
                .map(|(_, info)| *info)
                .unwrap_or_else(|| panic!("constraint block `{name}`"))
        };
        assert_eq!(blocks.iter().filter(|(name, _)| name == "ca").count(), 2);
        let ca = block("ca", 0);
        assert!(!ca.is_static && !ca.is_extern && !ca.is_pure);
        let proto = block("proto", 0);
        assert!(proto.is_extern && !proto.explicit_extern);
        assert!(block("sc", 0).is_static);
        // The external body is bound to the prototype.
        let NodeKind::Constraint(ConstraintKind::List { items }) =
            db.node_kind(proto.body.expect("external body"))
        else {
            panic!("prototype body is a constraint list");
        };
        assert_eq!(items.len(), 1);
        let NodeKind::Constraint(ConstraintKind::List { items }) =
            db.node_kind(ca.body.expect("ca body"))
        else {
            panic!("ca body is a constraint list");
        };
        assert!(matches!(
            db.node_kind(items[0]),
            NodeKind::Constraint(ConstraintKind::Expr { soft: false, .. })
        ));
        let NodeKind::Constraint(ConstraintKind::Conditional {
            predicate,
            else_body: Some(_),
            ..
        }) = db.node_kind(items[1])
        else {
            panic!("if/else constraint");
        };
        assert!(matches!(db.node_kind(*predicate), NodeKind::Expr(_)));
        assert!(matches!(
            db.node_kind(items[2]),
            NodeKind::Constraint(ConstraintKind::Implication { .. })
        ));

        let calls = db
            .node_ids()
            .filter_map(|id| db.randomize_call(id).map(|call| (id, *call)))
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 4);
        let forms = calls.iter().map(|(_, call)| call.form).collect::<Vec<_>>();
        assert_eq!(
            forms
                .iter()
                .filter(|form| **form == RandomizeForm::Scope)
                .count(),
            1
        );
        let with_inline = calls
            .iter()
            .filter(|(_, call)| call.inline.is_some())
            .count();
        assert_eq!(with_inline, 2);
        // The method's own randomize() is the class form without a receiver.
        assert!(calls.iter().any(|(id, call)| {
            call.form == RandomizeForm::Class
                && matches!(db.node_kind(*id), NodeKind::SysCall { .. })
        }));
    }

    #[test]
    fn dist_items_and_foreach_constraint_loop_variables_are_typed() {
        let source = "module top; class C; rand bit [7:0] x; rand byte a[4]; int w; \
            constraint d { x dist { 0 := 5, [1:9] :/ w, 10 }; } \
            constraint f { foreach (a[i]) a[i] < i; } endclass endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "dist.sv", source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let dist = db
            .node_ids()
            .find_map(|id| match db.node_kind(id) {
                NodeKind::Expr(ExprKind::Dist {
                    left,
                    items,
                    default_weight,
                }) => Some((*left, items.clone(), *default_weight)),
                _ => None,
            })
            .expect("dist expression");
        let (left, items, default_weight) = dist;
        assert!(!default_weight);
        assert!(matches!(db.node_kind(left), NodeKind::Expr(_)));
        assert_eq!(items.len(), 3);
        assert!(items[0].weight.is_some() && !items[0].per_range);
        assert!(items[1].weight.is_some() && items[1].per_range);
        assert!(items[2].weight.is_none());
        let (vars, body) = db
            .node_ids()
            .find_map(|id| match db.node_kind(id) {
                NodeKind::Constraint(ConstraintKind::Foreach { vars, body, .. }) => {
                    Some((vars.clone(), *body))
                }
                _ => None,
            })
            .expect("foreach constraint");
        assert_eq!(vars.len(), 1);
        let loop_var = vars[0].expect("loop variable");
        assert_eq!(db.node(loop_var).name, "i");
        assert!(matches!(db.node_kind(body), NodeKind::Constraint(_)));
    }

    #[test]
    fn port_expressions_capture_components_and_internal_expressions() {
        let source = "module m(.p({a, b}), .q(c[2:1]), d);\n\
            input [1:0] a; input b; input [3:0] c; input d;\n\
            endmodule\n\
            module top; logic [2:0] x; logic [1:0] y; m u(.p(x), .q(y), .d(1'b0)); endmodule\n";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "port_expressions.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        db.validate().unwrap();
        let port = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(db.node_kind(*id), NodeKind::Port { .. }) && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("port `{name}`"))
        };
        let (p, a, b, q, d) = (port("p"), port("a"), port("b"), port("q"), port("d"));
        // The concatenation's components, MSB first, each knowing its group.
        assert_eq!(db.multi_port_components(p), [a, b]);
        assert_eq!(db.multi_port_of(a), Some(p));
        assert_eq!(db.multi_port_of(b), Some(p));
        assert_eq!(db.multi_port_of(q), None);
        assert!(db.multi_port_components(q).is_empty());
        // A select port keeps its expression instead of a whole child signal.
        let NodeKind::Port { low, .. } = db.node_kind(q) else {
            unreachable!()
        };
        assert_eq!(*low, None);
        let internal = db
            .port_internal_expression(q)
            .expect("select port expression");
        assert!(matches!(db.node_kind(internal), NodeKind::Expr(_)));
        assert_eq!(db.semantic_detail(internal), Some("RangeSelect"));
        // Simple ports and components keep their whole internal signal.
        for simple in [a, b, d] {
            let NodeKind::Port { low, .. } = db.node_kind(simple) else {
                unreachable!()
            };
            assert!(low.is_some(), "{}", db.node(simple).name);
            assert_eq!(db.port_internal_expression(simple), None);
        }
        assert_eq!(db.port_internal_expression(p), None);
    }

    #[test]
    fn instances_keep_the_default_disable_they_declare() {
        let source = "module leaf(input logic c); endmodule \
            module top; logic clk, rst, q; default disable iff rst; \
            leaf l(clk); a1: assert property (@(posedge clk) q); endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "disable.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let instance = |name: &str| {
            db.node_ids()
                .find(|id| {
                    matches!(db.node_kind(*id), NodeKind::ModuleInst { .. })
                        && db.node(*id).name == name
                })
                .unwrap_or_else(|| panic!("instance `{name}`"))
        };
        let top = instance("top");
        let disable = db.instance_default_disable(top).expect("default disable");
        assert!(matches!(
            db.node_kind(disable),
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) if db.node(*t).name == "rst"
        ));
        // The expression is metadata, not a member of the instance.
        assert!(!db.node(top).children.contains(&disable));
        // An enclosing scope's default disable is not the child's own.
        assert_eq!(db.instance_default_disable(instance("l")), None);
    }

    #[test]
    fn checker_instances_actuals_and_default_disable_are_recorded() {
        let source = "checker inner(logic x, event ck = $inferred_clock); \
            bit h; always @(ck) h <= x; endchecker \
            checker outer(logic a, event clk, logic r = $inferred_disable); \
            default clocking @clk; endclocking \
            bit n; always @(clk) n <= a; inner i1(a); \
            a1: assert property (disable iff (r) a); endchecker \
            module dut(input logic c, input logic d); endmodule \
            checker bound(logic v); endchecker \
            module top; logic clk, a, rst; default disable iff rst; \
            outer u(a, posedge clk); dut d(clk, a); endmodule \
            bind dut bound b(d);";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "checkers.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        assert!(db.has_checker_instances());
        let named = |name: &str| {
            db.checker_instances()
                .into_iter()
                .find(|id| db.node(*id).name == name)
                .unwrap_or_else(|| panic!("checker instance `{name}`"))
        };
        let (u, i1, b) = (named("u"), named("i1"), named("b"));
        assert!(matches!(
            db.node_kind(u),
            NodeKind::ModuleInst { def_name, .. } if def_name == "outer"
        ));
        let info = db.checker_instance(u).unwrap();
        assert!(!info.procedural && !info.bound && !info.nested);
        assert!(info.unit_time_scale);
        // `default disable iff rst` of the instantiating module.
        let disable = info.default_disable.expect("default disable");
        assert!(matches!(
            db.node_kind(disable),
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) if db.node(*t).name == "rst"
        ));
        assert_eq!(db.node(i1).parent, Some(u));
        assert!(db.checker_instance(i1).unwrap().nested);
        assert!(db.checker_instance(b).unwrap().bound);
        // Formal references in the bodies name their instance.
        let actuals = db
            .node_ids()
            .filter_map(|id| db.checker_actual(id).map(|actual| actual.instance))
            .collect::<Vec<_>>();
        assert!(actuals.contains(&u));
        assert!(actuals.contains(&i1));
        assert!(actuals.iter().all(|instance| [u, i1, b].contains(instance)));
        // A module without checkers keeps the tables empty.
        let plain = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "plain.sv",
                "module top; logic a; endmodule",
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let plain = Db::from_slang(&plain.snapshot).unwrap();
        assert!(!plain.has_checker_instances());
        assert!(plain.checker_instances().is_empty());
    }
    #[test]
    fn procedural_checker_instances_are_hoisted_and_named_by_their_statement() {
        let source = "checker pc(logic a, event clk); a1: assert property (@clk a); endchecker \
            module top; logic clk, a; \
            always @(posedge clk) begin : blk if (a) begin pc u(a, posedge clk); end end \
            endmodule";
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "procedural.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let u = db
            .checker_instances()
            .into_iter()
            .find(|id| db.node(*id).name == "u")
            .expect("procedural checker instance");
        assert!(db.checker_instance(u).unwrap().procedural);
        // A child of the module, keeping its name through the named block.
        let top = db.node(u).parent.expect("hoisted parent");
        assert!(matches!(db.node_kind(top), NodeKind::ModuleInst { .. }));
        assert!(db.node(top).children.contains(&u));
        assert_eq!(&*db.node(u).full_name, "top.blk.u");
        assert_eq!(
            db.node_ids()
                .filter(|id| db.node(*id).children.contains(&u))
                .count(),
            1
        );
        let statements = db
            .node_ids()
            .filter_map(|id| match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::ProceduralChecker { instances }) => {
                    Some(instances.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(statements, vec![vec![u]]);
    }
}
