//! Hierarchy.

use super::*;

pub(super) fn graph_cycle_root<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    component: &[usize],
    elaborated_lookup: &SourceElaborationLookup<'_>,
    budget: &mut InstanceBudget,
) -> Option<ExplorerInstance>
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let definition_index = *component.first()?;
    let definition = &catalog.definitions[definition_index];
    let hierarchy = format!("<cycle-root>.{}", clean_name(&definition.name));
    let mut active = Vec::new();
    let mut root = graph_instance_node(
        root_id,
        catalog,
        source_map,
        hierarchy,
        definition.name.clone(),
        clean_name(&definition.name).to_owned(),
        definition.file.clone(),
        definition.line,
        definition.col,
        definition.end_line,
        definition.end_col,
        GraphResolution::Unique(definition_index),
        None,
        elaborated_lookup,
        &mut active,
        budget,
        0,
        true,
    )?;
    root.is_cycle_root = true;
    Some(root)
}

pub(super) fn graph_definition_children(
    definition: &ModuleGraphDefinition,
    visit: &mut impl FnMut(&ModuleGraphInstance),
) {
    for child in &definition.children {
        visit(child);
    }

    // This is a root-discovery pre-pass, not hierarchy expansion. Keep its
    // source-order semantics while avoiding call-stack growth for malformed
    // or deeply nested generated scopes.
    let mut pending = definition.generated_scopes.iter().rev().collect::<Vec<_>>();
    while let Some(scope) = pending.pop() {
        for child in &scope.children {
            visit(child);
        }
        pending.extend(scope.nested.iter().rev());
    }
}

// Hierarchy expansion keeps source identity, elaborated identity, cycle state,
// and response-budget state explicit because each has distinct semantics.
#[allow(clippy::too_many_arguments)]
pub(super) fn graph_instance_node<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    hierarchy: String,
    instance_name: String,
    module_type: String,
    file: Option<String>,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
    resolution: GraphResolution,
    elaborated: Option<&InstanceModel>,
    elaborated_lookup: &SourceElaborationLookup<'_>,
    active: &mut Vec<usize>,
    budget: &mut InstanceBudget,
    depth: usize,
    root: bool,
) -> Option<ExplorerInstance>
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let definition_id = match resolution {
        GraphResolution::Unique(index) => Some(catalog.ids[index].clone()),
        GraphResolution::Ambiguous | GraphResolution::Missing => None,
    };
    let source_definition = match resolution {
        GraphResolution::Unique(index) => Some(&catalog.definitions[index]),
        GraphResolution::Ambiguous | GraphResolution::Missing => None,
    };
    let id = graph_instance_id(
        root_id,
        definition_id.as_deref(),
        &module_type,
        &hierarchy,
        file.as_deref(),
        line,
        col,
        source_map,
    );
    let mut node = ExplorerInstance {
        id: id.clone(),
        instance_name,
        module_type: clean_name(&module_type).to_owned(),
        definition_id,
        is_budget_truncated: false,
        is_cycle_truncated: false,
        is_ambiguous: matches!(resolution, GraphResolution::Ambiguous),
        is_cycle_root: false,
        content_source: Some(
            if elaborated.is_some() {
                "elaborated"
            } else {
                "declaration"
            }
            .to_owned(),
        ),
        uri: source_uri(file.as_deref()),
        range: source_range(line, col, end_line, end_col),
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        generated_scopes: Vec::new(),
        children: Vec::new(),
    };

    // An ambiguous module name is intentionally a visible but unexpanded
    // leaf. There is no safe definition identity to attach to it. A missing
    // definition may still use an elaborated instance's children, but the
    // instance itself consumes a budget slot just like every other node.
    if let GraphResolution::Unique(definition_index) = resolution {
        // Check the active definition path before asking for an ordinary
        // budget slot. A cycle at the budget boundary is still a cycle.
        if active.contains(&definition_index) {
            node.is_cycle_truncated = true;
            return budget.take_terminal().then_some(node);
        }
    }
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        return budget.take_budget_marker().then(|| {
            node.is_budget_truncated = true;
            node
        });
    }

    let budget_slot = if root {
        budget.take_root()
    } else {
        budget.take_regular()
    };
    if !budget_slot {
        return budget.take_budget_marker().then(|| {
            node.is_budget_truncated = true;
            node
        });
    }

    let content_hierarchy = elaborated
        .map(|instance| graph_hierarchy_name(&instance.full_name, &instance.name))
        .unwrap_or_else(|| hierarchy.clone());
    let mut content_truncated = false;
    match elaborated {
        Some(instance) => {
            node.ports = collect_budgeted(
                instance.ports.iter(),
                budget,
                &mut content_truncated,
                |item| {
                    port_with_source(
                        item,
                        source_definition
                            .and_then(|definition| definition_port(definition, &item.name)),
                        catalog.packed_ranges(&content_hierarchy, &item.name),
                    )
                },
            );
            node.params = collect_budgeted(
                instance.params.iter(),
                budget,
                &mut content_truncated,
                |item| {
                    parameter_with_source(
                        item,
                        source_definition
                            .and_then(|definition| definition_parameter(definition, &item.name)),
                        catalog.packed_ranges(&content_hierarchy, &item.name),
                    )
                },
            );
            node.signals = instance_signals_with_source(
                instance,
                source_definition,
                catalog,
                &content_hierarchy,
                budget,
                &mut content_truncated,
            );
        }
        None => match resolution {
            GraphResolution::Unique(index) => {
                node.ports = collect_budgeted(
                    catalog.definitions[index].ports.iter(),
                    budget,
                    &mut content_truncated,
                    graph_port,
                );
                node.params = collect_budgeted(
                    catalog.definitions[index].params.iter(),
                    budget,
                    &mut content_truncated,
                    graph_parameter,
                );
                node.signals =
                    graph_signals(&catalog.definitions[index], budget, &mut content_truncated);
            }
            GraphResolution::Ambiguous | GraphResolution::Missing => {}
        },
    }
    node.is_budget_truncated = content_truncated;
    if budget.should_stop() {
        let omitted_descendants = elaborated.is_some_and(|instance| {
            !instance.children.is_empty() || !instance.gen_scopes.is_empty()
        }) || source_definition.is_some_and(|definition| {
            !definition.children.is_empty() || !definition.generated_scopes.is_empty()
        });
        node.is_budget_truncated |= omitted_descendants;
        return Some(node);
    }

    let Some(definition_index) = (match resolution {
        GraphResolution::Unique(index) => Some(index),
        GraphResolution::Ambiguous | GraphResolution::Missing => None,
    }) else {
        if matches!(resolution, GraphResolution::Missing) {
            if let Some(instance) = elaborated {
                for child in &instance.children {
                    if budget.should_stop() {
                        break;
                    }
                    let child_type = clean_name(&child.def_name).to_owned();
                    let child_resolution = catalog.resolve(&child_type);
                    let Some(child_node) = graph_instance_node(
                        root_id,
                        catalog,
                        source_map,
                        graph_child_hierarchy(&hierarchy, &child.name),
                        child.name.clone(),
                        child_type,
                        child.file.clone(),
                        child.line,
                        child.col,
                        child.line,
                        child
                            .col
                            .saturating_add(clean_name(&child.def_name).chars().count() as u32),
                        child_resolution,
                        Some(child),
                        elaborated_lookup,
                        active,
                        budget,
                        depth.saturating_add(1),
                        false,
                    ) else {
                        node.is_budget_truncated = true;
                        break;
                    };
                    node.children.push(child_node);
                    if budget.should_stop() {
                        break;
                    }
                }
            }
        }
        return Some(node);
    };

    active.push(definition_index);

    let source_definition = &catalog.definitions[definition_index];
    if let Some(instance) = elaborated {
        for child in &instance.children {
            if budget.should_stop() {
                break;
            }
            let child_type = clean_name(&child.def_name).to_owned();
            let child_type_width = child_type.chars().count() as u32;
            let child_resolution = catalog.resolve(&child_type);
            let source_occurrence = source_definition.children.iter().find(|source_child| {
                same_name(&source_child.name, &child.name)
                    && same_name(&source_child.module_type, &child_type)
            });
            let child_file = source_occurrence
                .and_then(|source_child| source_child.file.clone())
                .or_else(|| child.file.clone());
            let child_line = source_occurrence.map_or(child.line, |source_child| source_child.line);
            let child_col = source_occurrence.map_or(child.col, |source_child| source_child.col);
            let Some(child_node) = graph_instance_node(
                root_id,
                catalog,
                source_map,
                graph_child_hierarchy(&hierarchy, &child.name),
                child.name.clone(),
                child_type,
                child_file,
                child_line,
                child_col,
                child_line,
                child_col.saturating_add(child_type_width),
                child_resolution,
                Some(child),
                elaborated_lookup,
                active,
                budget,
                depth.saturating_add(1),
                false,
            ) else {
                node.is_budget_truncated = true;
                break;
            };
            node.children.push(child_node);
            if budget.should_stop() {
                break;
            }
        }
        if !budget.should_stop() {
            for scope in &instance.gen_scopes {
                match graph_elaborated_scope(
                    root_id,
                    catalog,
                    source_map,
                    &hierarchy,
                    &id,
                    scope,
                    elaborated_lookup,
                    active,
                    budget,
                    depth,
                ) {
                    Some(generated) => node.generated_scopes.push(generated),
                    None => {
                        node.is_budget_truncated = true;
                        append_graph_budget_marker(
                            root_id,
                            &hierarchy,
                            source_map,
                            &mut node.children,
                            budget,
                        );
                        break;
                    }
                }
                if budget.should_stop() {
                    break;
                }
            }
        }
        // Add source edges that elaboration omitted.  A name/type match is
        // sufficient here because source declarations are unique edges within
        // a definition; exact source positions remain part of the child ID.
        if !budget.should_stop() {
            for child in &source_definition.children {
                if instance.children.iter().any(|elaborated_child| {
                    same_name(&elaborated_child.name, &child.name)
                        && same_name(&elaborated_child.def_name, &child.module_type)
                }) {
                    continue;
                }
                let Some(child_node) = graph_source_child(
                    root_id,
                    catalog,
                    source_map,
                    &hierarchy,
                    definition_index,
                    child,
                    elaborated_lookup,
                    active,
                    budget,
                    depth,
                ) else {
                    node.is_budget_truncated = true;
                    break;
                };
                node.children.push(child_node);
                if budget.should_stop() {
                    break;
                }
            }
        }
        if !budget.should_stop() {
            for scope in &source_definition.generated_scopes {
                let matching_indices = node
                    .generated_scopes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, generated)| {
                        generated_scope_matches(generated, scope).then_some(index)
                    })
                    .collect::<Vec<_>>();
                if matching_indices.is_empty() {
                    match graph_source_scope(
                        root_id,
                        catalog,
                        source_map,
                        &hierarchy,
                        &id,
                        definition_index,
                        scope,
                        elaborated_lookup,
                        active,
                        budget,
                        depth,
                    ) {
                        Some(generated) => node.generated_scopes.push(generated),
                        None => {
                            node.is_budget_truncated = true;
                            append_graph_budget_marker(
                                root_id,
                                &hierarchy,
                                source_map,
                                &mut node.children,
                                budget,
                            );
                            break;
                        }
                    }
                } else {
                    // Elaboration is authoritative for concrete scope contents.
                    // Merge only source edges that elaboration omitted and retain
                    // nested source wrappers, rather than appending a duplicate
                    // declaration-only scope beside each elaborated occurrence.
                    for index in matching_indices {
                        merge_source_scope(
                            root_id,
                            catalog,
                            source_map,
                            &hierarchy,
                            definition_index,
                            &mut node.generated_scopes[index],
                            scope,
                            elaborated_lookup,
                            active,
                            budget,
                            depth,
                        );
                        if budget.should_stop() {
                            break;
                        }
                    }
                }
                if budget.should_stop() {
                    break;
                }
            }
        }
    } else {
        for child in &source_definition.children {
            if budget.should_stop() {
                break;
            }
            let Some(child_node) = graph_source_child(
                root_id,
                catalog,
                source_map,
                &hierarchy,
                definition_index,
                child,
                elaborated_lookup,
                active,
                budget,
                depth,
            ) else {
                node.is_budget_truncated = true;
                break;
            };
            node.children.push(child_node);
            if budget.should_stop() {
                break;
            }
        }
        if !budget.should_stop() {
            for scope in &source_definition.generated_scopes {
                match graph_source_scope(
                    root_id,
                    catalog,
                    source_map,
                    &hierarchy,
                    &id,
                    definition_index,
                    scope,
                    elaborated_lookup,
                    active,
                    budget,
                    depth,
                ) {
                    Some(generated) => node.generated_scopes.push(generated),
                    None => {
                        node.is_budget_truncated = true;
                        append_graph_budget_marker(
                            root_id,
                            &hierarchy,
                            source_map,
                            &mut node.children,
                            budget,
                        );
                        break;
                    }
                }
                if budget.should_stop() {
                    break;
                }
            }
        }
    }
    active.pop();
    node.children.sort_by(|left, right| left.id.cmp(&right.id));
    node.generated_scopes
        .sort_by(|left, right| left.id.cmp(&right.id));
    Some(node)
}

#[allow(clippy::too_many_arguments)]
fn graph_source_child<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    parent_hierarchy: &str,
    owner_definition: usize,
    child: &ModuleGraphInstance,
    elaborated_lookup: &SourceElaborationLookup<'_>,
    active: &mut Vec<usize>,
    budget: &mut InstanceBudget,
    depth: usize,
) -> Option<ExplorerInstance>
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let module_type = clean_name(&child.module_type).to_owned();
    let resolution = catalog.resolve(&module_type);
    graph_instance_node(
        root_id,
        catalog,
        source_map,
        graph_child_hierarchy(parent_hierarchy, &child.name),
        child.name.clone(),
        module_type,
        child.file.clone(),
        child.line,
        child.col,
        child.line,
        child.col.saturating_add(child.name.chars().count() as u32),
        resolution,
        elaborated_lookup.get(owner_definition, child),
        elaborated_lookup,
        active,
        budget,
        depth.saturating_add(1),
        false,
    )
}

fn append_graph_budget_marker<F>(
    root_id: &str,
    parent_hierarchy: &str,
    source_map: &F,
    children: &mut Vec<ExplorerInstance>,
    budget: &mut InstanceBudget,
) where
    F: Fn(&Path) -> Option<PathBuf>,
{
    if !budget.take_budget_marker() {
        return;
    }
    let hierarchy = graph_child_hierarchy(parent_hierarchy, "<budget-truncated>");
    children.push(ExplorerInstance {
        id: graph_instance_id(
            root_id,
            None,
            "<budget-truncated>",
            &hierarchy,
            None,
            0,
            0,
            source_map,
        ),
        instance_name: "<budget-truncated>".to_owned(),
        module_type: "<budget-truncated>".to_owned(),
        definition_id: None,
        is_budget_truncated: true,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: None,
        range: None,
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        generated_scopes: Vec::new(),
        children: Vec::new(),
    });
}

fn mark_graph_scope_truncated<F>(
    root_id: &str,
    hierarchy: &str,
    source_map: &F,
    children: &mut Vec<ExplorerInstance>,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
    omitted: bool,
) where
    F: Fn(&Path) -> Option<PathBuf>,
{
    if !omitted {
        return;
    }
    *truncated = true;
    // The helper owns marker admission, so a tight response can still expose
    // the truncation flag without serializing an unreserved marker node.
    append_graph_budget_marker(root_id, hierarchy, source_map, children, budget);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn graph_elaborated_scope<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    parent_hierarchy: &str,
    parent_id: &str,
    scope: &GenScopeModel,
    elaborated_lookup: &SourceElaborationLookup<'_>,
    active: &mut Vec<usize>,
    budget: &mut InstanceBudget,
    depth: usize,
) -> Option<ExplorerGenerateScope>
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    if !budget.take_regular() {
        return None;
    }
    let id = graph_scope_id(
        root_id,
        parent_id,
        &scope.name,
        scope.full_name.as_str(),
        0,
        0,
    );
    let hierarchy = graph_child_hierarchy(parent_hierarchy, &scope.name);
    let mut children = Vec::new();
    let mut is_budget_truncated = false;
    let params = collect_budgeted(
        scope.params.iter(),
        budget,
        &mut is_budget_truncated,
        parameter,
    );
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        is_budget_truncated = true;
        append_graph_budget_marker(root_id, &hierarchy, source_map, &mut children, budget);
        return Some(ExplorerGenerateScope {
            id,
            name: scope.name.clone(),
            is_budget_truncated,
            params,
            children,
            nested_scopes: Vec::new(),
        });
    }
    for child in &scope.children {
        if budget.should_stop() {
            break;
        }
        let child_type = clean_name(&child.def_name).to_owned();
        let Some(child_node) = graph_instance_node(
            root_id,
            catalog,
            source_map,
            graph_child_hierarchy(&hierarchy, &child.name),
            child.name.clone(),
            child_type.clone(),
            child.file.clone(),
            child.line,
            child.col,
            child.line,
            child
                .col
                .saturating_add(clean_name(&child.def_name).chars().count() as u32),
            catalog.resolve(&child_type),
            Some(child),
            elaborated_lookup,
            active,
            budget,
            depth.saturating_add(1),
            false,
        ) else {
            is_budget_truncated = true;
            break;
        };
        children.push(child_node);
        if budget.should_stop() {
            break;
        }
    }
    let omitted_children = children.len() < scope.children.len();
    mark_graph_scope_truncated(
        root_id,
        &hierarchy,
        source_map,
        &mut children,
        budget,
        &mut is_budget_truncated,
        omitted_children,
    );
    children.sort_by(|left, right| left.id.cmp(&right.id));
    Some(ExplorerGenerateScope {
        id,
        name: scope.name.clone(),
        is_budget_truncated,
        params,
        children,
        nested_scopes: Vec::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn graph_source_scope<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    parent_hierarchy: &str,
    parent_id: &str,
    owner_definition: usize,
    scope: &ModuleGraphGenerateScope,
    elaborated_lookup: &SourceElaborationLookup<'_>,
    active: &mut Vec<usize>,
    budget: &mut InstanceBudget,
    depth: usize,
) -> Option<ExplorerGenerateScope>
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    if !budget.take_regular() {
        return None;
    }
    let id = graph_scope_id(root_id, parent_id, &scope.name, "", scope.line, scope.col);
    let hierarchy = graph_child_hierarchy(parent_hierarchy, &scope.name);
    let mut children = Vec::new();
    let mut is_budget_truncated = false;
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        append_graph_budget_marker(root_id, &hierarchy, source_map, &mut children, budget);
        return Some(ExplorerGenerateScope {
            id,
            name: scope.name.clone(),
            is_budget_truncated: true,
            params: Vec::new(),
            children,
            nested_scopes: Vec::new(),
        });
    }
    for child in &scope.children {
        if budget.should_stop() {
            break;
        }
        let Some(child_node) = graph_source_child(
            root_id,
            catalog,
            source_map,
            &hierarchy,
            owner_definition,
            child,
            elaborated_lookup,
            active,
            budget,
            depth,
        ) else {
            is_budget_truncated = true;
            break;
        };
        children.push(child_node);
        if budget.should_stop() {
            break;
        }
    }
    let omitted_children = children.len() < scope.children.len();
    mark_graph_scope_truncated(
        root_id,
        &hierarchy,
        source_map,
        &mut children,
        budget,
        &mut is_budget_truncated,
        omitted_children,
    );
    children.sort_by(|left, right| left.id.cmp(&right.id));
    let mut nested_scopes = Vec::new();
    if !budget.should_stop() {
        for nested in &scope.nested {
            match graph_source_scope(
                root_id,
                catalog,
                source_map,
                &hierarchy,
                &id,
                owner_definition,
                nested,
                elaborated_lookup,
                active,
                budget,
                depth.saturating_add(1),
            ) {
                Some(generated) => nested_scopes.push(generated),
                None => {
                    is_budget_truncated = true;
                    append_graph_budget_marker(
                        root_id,
                        &hierarchy,
                        source_map,
                        &mut children,
                        budget,
                    );
                    break;
                }
            }
            if budget.should_stop() {
                break;
            }
        }
    }
    let omitted_nested = nested_scopes.len() < scope.nested.len();
    mark_graph_scope_truncated(
        root_id,
        &hierarchy,
        source_map,
        &mut children,
        budget,
        &mut is_budget_truncated,
        omitted_nested,
    );
    nested_scopes.sort_by(|left, right| left.id.cmp(&right.id));
    Some(ExplorerGenerateScope {
        id,
        name: scope.name.clone(),
        is_budget_truncated,
        params: Vec::new(),
        children,
        nested_scopes,
    })
}

fn generated_scope_base_name(name: &str) -> &str {
    name.split_once('[').map(|(base, _)| base).unwrap_or(name)
}

fn generated_scope_matches(
    elaborated: &ExplorerGenerateScope,
    source: &ModuleGraphGenerateScope,
) -> bool {
    if same_name(
        generated_scope_base_name(&elaborated.name),
        generated_scope_base_name(&source.name),
    ) {
        return true;
    }
    // Slang names an unlabeled source block `genblkN`, so its display name
    // cannot always be matched to the source fallback name. A direct child
    // name/type pair is a safe second identity because the source graph keeps
    // each instance declaration position in the node ID.
    source.children.iter().any(|source_child| {
        elaborated.children.iter().any(|elaborated_child| {
            same_name(&elaborated_child.instance_name, &source_child.name)
                && same_name(&elaborated_child.module_type, &source_child.module_type)
        })
    })
}

fn source_child_is_present(
    children: &[ExplorerInstance],
    source_child: &ModuleGraphInstance,
) -> bool {
    children.iter().any(|child| {
        same_name(&child.instance_name, &source_child.name)
            && same_name(&child.module_type, &source_child.module_type)
    })
}

fn source_scope_has_unrepresented_child(
    elaborated: &ExplorerGenerateScope,
    source: &ModuleGraphGenerateScope,
) -> bool {
    source
        .children
        .iter()
        .any(|source_child| !source_child_is_present(&elaborated.children, source_child))
}

fn source_scope_has_unrepresented_nested(
    elaborated: &ExplorerGenerateScope,
    source: &ModuleGraphGenerateScope,
) -> bool {
    source.nested.iter().any(|source_nested| {
        source_scope_is_unrepresented(&elaborated.nested_scopes, source_nested)
    })
}

fn source_scope_is_unrepresented(
    scopes: &[ExplorerGenerateScope],
    source: &ModuleGraphGenerateScope,
) -> bool {
    match scopes
        .iter()
        .find(|candidate| generated_scope_matches(candidate, source))
    {
        Some(existing) => existing.is_budget_truncated,
        None => true,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn merge_source_scope<F>(
    root_id: &str,
    catalog: &GraphCatalog<'_>,
    source_map: &F,
    parent_hierarchy: &str,
    owner_definition: usize,
    elaborated: &mut ExplorerGenerateScope,
    source: &ModuleGraphGenerateScope,
    elaborated_lookup: &SourceElaborationLookup<'_>,
    active: &mut Vec<usize>,
    budget: &mut InstanceBudget,
    depth: usize,
) where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let hierarchy = graph_child_hierarchy(parent_hierarchy, &elaborated.name);
    let parent_id = elaborated.id.clone();
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        elaborated.is_budget_truncated = true;
        append_graph_budget_marker(
            root_id,
            &hierarchy,
            source_map,
            &mut elaborated.children,
            budget,
        );
        return;
    }
    for source_child in &source.children {
        if budget.should_stop() {
            break;
        }
        if elaborated.children.iter().any(|elaborated_child| {
            same_name(&elaborated_child.instance_name, &source_child.name)
                && same_name(&elaborated_child.module_type, &source_child.module_type)
        }) {
            continue;
        }
        let Some(child_node) = graph_source_child(
            root_id,
            catalog,
            source_map,
            &hierarchy,
            owner_definition,
            source_child,
            elaborated_lookup,
            active,
            budget,
            depth,
        ) else {
            elaborated.is_budget_truncated = true;
            break;
        };
        elaborated.children.push(child_node);
        if budget.should_stop() {
            break;
        }
    }
    let omitted_children = source_scope_has_unrepresented_child(elaborated, source);
    mark_graph_scope_truncated(
        root_id,
        &hierarchy,
        source_map,
        &mut elaborated.children,
        budget,
        &mut elaborated.is_budget_truncated,
        omitted_children,
    );
    if !budget.should_stop() {
        for source_nested in &source.nested {
            if let Some(existing_nested) = elaborated
                .nested_scopes
                .iter_mut()
                .find(|candidate| generated_scope_matches(candidate, source_nested))
            {
                merge_source_scope(
                    root_id,
                    catalog,
                    source_map,
                    &hierarchy,
                    owner_definition,
                    existing_nested,
                    source_nested,
                    elaborated_lookup,
                    active,
                    budget,
                    depth.saturating_add(1),
                );
            } else {
                match graph_source_scope(
                    root_id,
                    catalog,
                    source_map,
                    &hierarchy,
                    &parent_id,
                    owner_definition,
                    source_nested,
                    elaborated_lookup,
                    active,
                    budget,
                    depth.saturating_add(1),
                ) {
                    Some(generated) => elaborated.nested_scopes.push(generated),
                    None => {
                        elaborated.is_budget_truncated = true;
                        append_graph_budget_marker(
                            root_id,
                            &hierarchy,
                            source_map,
                            &mut elaborated.children,
                            budget,
                        );
                        break;
                    }
                }
            }
            if budget.should_stop() {
                break;
            }
        }
    }
    let omitted_nested = source_scope_has_unrepresented_nested(elaborated, source);
    mark_graph_scope_truncated(
        root_id,
        &hierarchy,
        source_map,
        &mut elaborated.children,
        budget,
        &mut elaborated.is_budget_truncated,
        omitted_nested,
    );
    elaborated
        .children
        .sort_by(|left, right| left.id.cmp(&right.id));
    elaborated
        .nested_scopes
        .sort_by(|left, right| left.id.cmp(&right.id));
}

#[allow(clippy::too_many_arguments)]
fn graph_instance_id<F>(
    root_id: &str,
    definition_id: Option<&str>,
    module_type: &str,
    hierarchy: &str,
    file: Option<&str>,
    line: u32,
    col: u32,
    source_map: &F,
) -> String
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    format!(
        "instance:{root_id}:{}:{}:{}:{}:{}:{}",
        definition_id.unwrap_or_else(|| clean_name(module_type)),
        hierarchy,
        mapped_source_identity(file, source_map),
        line,
        col,
        clean_name(module_type),
    )
}

fn graph_scope_id(
    root_id: &str,
    parent_id: &str,
    name: &str,
    full_name: &str,
    line: u32,
    col: u32,
) -> String {
    let identity = if full_name.is_empty() {
        format!("{name}:{line}:{col}")
    } else {
        format!("{}:{line}:{col}", clean_name(full_name))
    };
    format!("generate:{root_id}:{parent_id}:{identity}")
}

pub(super) fn graph_hierarchy_name(full_name: &str, fallback: &str) -> String {
    let full_name = clean_name(full_name);
    if full_name.is_empty() {
        fallback.to_owned()
    } else {
        full_name.to_owned()
    }
}

fn graph_child_hierarchy(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}.{child}")
    }
}
