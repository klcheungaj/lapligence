//! Catalog.

use super::*;

pub(super) struct GraphCatalog<'a> {
    pub(super) definitions: &'a [ModuleGraphDefinition],
    pub(super) elaborated_types: &'a [ModuleGraphElaboratedType],
    pub(super) ids: Vec<String>,
    pub(super) by_name: std::collections::HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphResolution {
    Unique(usize),
    Ambiguous,
    Missing,
}

impl<'a> GraphCatalog<'a> {
    pub(super) fn new<F>(root_id: &str, graph: &'a crate::features::ModuleGraph, source_map: &F) -> Self
    where
        F: Fn(&Path) -> Option<PathBuf>,
    {
        let mut ids = Vec::with_capacity(graph.definitions.len());
        let mut by_name: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (index, definition) in graph.definitions.iter().enumerate() {
            ids.push(graph_definition_id(root_id, definition, source_map));
            by_name
                .entry(clean_name(&definition.name).to_owned())
                .or_default()
                .push(index);
        }
        for indices in by_name.values_mut() {
            indices.sort_by(|left, right| ids[*left].cmp(&ids[*right]));
        }
        Self {
            definitions: &graph.definitions,
            elaborated_types: &graph.elaborated_types,
            ids,
            by_name,
        }
    }

    pub(super) fn resolve(&self, name: &str) -> GraphResolution {
        match self.by_name.get(clean_name(name)) {
            Some(indices) if indices.len() == 1 => GraphResolution::Unique(indices[0]),
            Some(indices) if !indices.is_empty() => GraphResolution::Ambiguous,
            _ => GraphResolution::Missing,
        }
    }

    pub(super) fn packed_ranges(
        &self,
        instance: &str,
        name: &str,
    ) -> Option<&[Option<ModuleGraphPackedRange>]> {
        let instance = clean_name(instance);
        self.elaborated_types
            .iter()
            .find(|item| clean_name(&item.instance) == instance && item.name == name)
            .map(|item| item.packed_ranges.as_slice())
    }
}

fn graph_definition_id<F>(
    root_id: &str,
    definition: &ModuleGraphDefinition,
    source_map: &F,
) -> String
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    format!(
        "module:{root_id}:{}:{}:{}:{}:{}:{}",
        clean_name(&definition.name),
        definition.line,
        definition.col,
        definition.end_line,
        definition.end_col,
        mapped_source_identity(definition.file.as_deref(), source_map),
    )
}

pub(super) fn mapped_source_identity<F>(file: Option<&str>, source_map: &F) -> String
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let Some(file) = file else {
        return "<unknown>".to_owned();
    };
    source_map(Path::new(file))
        .unwrap_or_else(|| PathBuf::from(file))
        .to_string_lossy()
        .into_owned()
}

pub(super) struct GraphTopology {
    pub(super) roots: Vec<usize>,
    pub(super) cycle_components: Vec<Vec<usize>>,
}

/// Build the source graph topology once for root selection and cycle recovery.
/// Incoming edges retain the old name-based behavior for ordinary roots,
/// including duplicate-name ambiguity.  SCC discovery uses only uniquely
/// resolved edges, because an ambiguous module name cannot safely establish a
/// cycle or a definition identity.
pub(super) fn graph_topology(catalog: &GraphCatalog<'_>) -> GraphTopology {
    let definition_count = catalog.definitions.len();
    let mut incoming = vec![false; definition_count];
    let mut adjacency = vec![Vec::new(); definition_count];
    for (definition_index, definition) in catalog.definitions.iter().enumerate() {
        graph_definition_children(definition, &mut |child| {
            if let Some(indices) = catalog.by_name.get(clean_name(&child.module_type)) {
                for &index in indices {
                    incoming[index] = true;
                }
            }
            if let GraphResolution::Unique(index) = catalog.resolve(&child.module_type) {
                adjacency[definition_index].push(index);
            }
        });
        adjacency[definition_index]
            .sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));
        adjacency[definition_index].dedup();
    }

    let mut roots = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, has_incoming)| (!has_incoming).then_some(index))
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));

    let mut reachable = vec![false; definition_count];
    let mut pending = roots.clone();
    while let Some(index) = pending.pop() {
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        pending.extend(adjacency[index].iter().copied());
    }

    let mut reverse = vec![Vec::new(); definition_count];
    for (from, children) in adjacency.iter().enumerate() {
        for &to in children {
            reverse[to].push(from);
        }
    }
    for predecessors in &mut reverse {
        predecessors.sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));
        predecessors.dedup();
    }

    // Iterative Kosaraju traversal keeps malformed/deep source graphs off the
    // native call stack while preserving a stable ID-driven order.
    let mut order = Vec::with_capacity(definition_count);
    let mut visited = vec![false; definition_count];
    let mut starts = (0..definition_count).collect::<Vec<_>>();
    starts.sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));
    for start in starts {
        if visited[start] {
            continue;
        }
        let mut stack = vec![(start, false)];
        while let Some((index, expanded)) = stack.pop() {
            if expanded {
                order.push(index);
                continue;
            }
            if !visited[index] {
                visited[index] = true;
                stack.push((index, true));
                for &child in adjacency[index].iter().rev() {
                    if !visited[child] {
                        stack.push((child, false));
                    }
                }
            }
        }
    }

    let mut components = Vec::new();
    visited.fill(false);
    for &start in order.iter().rev() {
        if visited[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(index) = stack.pop() {
            component.push(index);
            for &predecessor in reverse[index].iter().rev() {
                if !visited[predecessor] {
                    visited[predecessor] = true;
                    stack.push(predecessor);
                }
            }
        }
        component.sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));
        components.push(component);
    }

    let mut component_of = vec![0; definition_count];
    let mut cyclic = vec![false; components.len()];
    for (component_index, component) in components.iter().enumerate() {
        for &member in component {
            component_of[member] = component_index;
        }
        cyclic[component_index] = component.len() > 1
            || adjacency[component[0]]
                .iter()
                .any(|child| *child == component[0]);
    }

    let mut cycle_components = Vec::new();
    for (component_index, component) in components.into_iter().enumerate() {
        if !cyclic[component_index] || component.iter().any(|member| reachable[*member]) {
            continue;
        }
        // If two unreachable cyclic SCCs are connected, expose only the
        // upstream one. Its bounded expansion reaches the downstream SCC and
        // avoids duplicate synthetic roots for one disconnected hierarchy.
        let has_unreachable_cyclic_predecessor = component.iter().any(|member| {
            reverse[*member].iter().any(|predecessor| {
                let predecessor_component = component_of[*predecessor];
                predecessor_component != component_index
                    && cyclic[predecessor_component]
                    && !reachable[*predecessor]
            })
        });
        if !has_unreachable_cyclic_predecessor {
            cycle_components.push(component);
        }
    }
    cycle_components.sort_by(|left, right| catalog.ids[left[0]].cmp(&catalog.ids[right[0]]));

    GraphTopology {
        roots,
        cycle_components,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SourceOccurrenceKey {
    pub(super) owner_definition: usize,
    pub(super) name: String,
    pub(super) module_type: String,
    pub(super) file: Option<String>,
    pub(super) line: u32,
    pub(super) col: u32,
}

impl SourceOccurrenceKey {
    pub(super) fn new(owner_definition: usize, child: &ModuleGraphInstance) -> Self {
        Self {
            owner_definition,
            name: child.name.clone(),
            module_type: clean_name(&child.module_type).to_owned(),
            file: child.file.clone(),
            line: child.line,
            col: child.col,
        }
    }
}

/// Maps a source edge to at most one retained elaborated top instance.  Exact
/// source location/name matches win.  The only looser fallback is allowed
/// when one uniquely resolved module type has exactly one source occurrence,
/// which is the safe case for a configured top nested under a source parent.
pub(super) struct SourceElaborationLookup<'a> {
    pub(super) top_instances: &'a [InstanceModel],
    pub(super) by_occurrence: HashMap<SourceOccurrenceKey, usize>,
}

impl<'a> SourceElaborationLookup<'a> {
    pub(super) fn new(
        catalog: &GraphCatalog<'_>,
        top_instances: &'a [InstanceModel],
        reserved_top_instances: &HashSet<usize>,
    ) -> Self {
        let mut occurrences = Vec::new();
        for (owner_definition, definition) in catalog.definitions.iter().enumerate() {
            graph_definition_children(definition, &mut |child| {
                occurrences.push(SourceOccurrenceKey::new(owner_definition, child));
            });
        }

        let unique_type = |occurrence: &SourceOccurrenceKey, module_type: &str| {
            same_name(&occurrence.module_type, module_type)
                && matches!(
                    catalog.resolve(&occurrence.module_type),
                    GraphResolution::Unique(_)
                )
        };

        // Reserve exact source location/name matches before considering any
        // type-only fallback.  Otherwise an earlier retained instance with
        // the same unique type can consume the only occurrence and make a
        // later, stronger match lose its elaborated contents.
        let mut exact_candidates_by_top = vec![None; top_instances.len()];
        let mut exact_claimants = vec![Vec::new(); occurrences.len()];
        for (top_index, instance) in top_instances.iter().enumerate() {
            if reserved_top_instances.contains(&top_index) {
                continue;
            }
            let candidates = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| {
                    unique_type(occurrence, clean_name(&instance.def_name))
                        && source_location_matches(instance, occurrence)
                        && source_name_matches(instance, occurrence)
                })
                .map(|(occurrence_index, _)| occurrence_index)
                .collect::<Vec<_>>();
            if candidates.len() == 1 {
                exact_claimants[candidates[0]].push(top_index);
            }
            if !candidates.is_empty() {
                exact_candidates_by_top[top_index] = Some(candidates);
            }
        }

        let mut exact_assignments = vec![None; top_instances.len()];
        let mut reserved_occurrences = HashSet::new();
        let mut blocked_exact_occurrences = HashSet::new();
        for (occurrence_index, claimants) in exact_claimants.iter().enumerate() {
            match claimants.as_slice() {
                [top_index] => {
                    exact_assignments[*top_index] = Some(occurrence_index);
                    reserved_occurrences.insert(occurrence_index);
                }
                [] => {}
                _ => {
                    // Multiple exact retained instances claim the same
                    // source edge; leave it declaration-only rather than
                    // selecting one by input order.
                    blocked_exact_occurrences.insert(occurrence_index);
                }
            }
        }

        let mut by_occurrence = HashMap::new();
        for (top_index, _) in top_instances.iter().enumerate() {
            if reserved_top_instances.contains(&top_index) {
                continue;
            }
            let Some(occurrence_index) = exact_assignments[top_index] else {
                continue;
            };
            by_occurrence.insert(occurrences[occurrence_index].clone(), top_index);
        }

        // Resolve all weaker candidates as a separate pass.  A candidate is
        // accepted only when its complete tier was unique before any other
        // assignment was removed, and only one retained instance claims it.
        // This preserves the no-guessing behavior for duplicate occurrences
        // and makes the fallback independent of top-instance order too.
        let mut loose_candidates = vec![None; top_instances.len()];
        let mut loose_claimants = vec![Vec::new(); occurrences.len()];
        for (top_index, instance) in top_instances.iter().enumerate() {
            if reserved_top_instances.contains(&top_index)
                || exact_candidates_by_top[top_index].is_some()
            {
                continue;
            }
            let module_type = clean_name(&instance.def_name);
            let type_only_candidates = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| unique_type(occurrence, module_type))
                .map(|(occurrence_index, _)| occurrence_index)
                .collect::<Vec<_>>();

            let mut candidates = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| {
                    unique_type(occurrence, module_type)
                        && source_location_matches(instance, occurrence)
                        && source_name_matches(instance, occurrence)
                })
                .map(|(occurrence_index, _)| occurrence_index)
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                candidates = occurrences
                    .iter()
                    .enumerate()
                    .filter(|(_, occurrence)| {
                        unique_type(occurrence, module_type)
                            && source_location_matches(instance, occurrence)
                    })
                    .map(|(occurrence_index, _)| occurrence_index)
                    .collect();
            }
            if candidates.is_empty() {
                candidates = occurrences
                    .iter()
                    .enumerate()
                    .filter(|(_, occurrence)| {
                        unique_type(occurrence, module_type)
                            && source_name_matches(instance, occurrence)
                    })
                    .map(|(occurrence_index, _)| occurrence_index)
                    .collect();
            }
            if candidates.is_empty() && type_only_candidates.len() == 1 {
                // The type-only fallback is safe only when the complete
                // pre-filter candidate set contained one source occurrence.
                candidates = type_only_candidates;
            }

            if candidates.len() == 1 {
                let occurrence_index = candidates[0];
                loose_candidates[top_index] = Some(occurrence_index);
                loose_claimants[occurrence_index].push(top_index);
            }
        }

        for (top_index, occurrence_index) in loose_candidates.into_iter().enumerate() {
            let Some(occurrence_index) = occurrence_index else {
                continue;
            };
            if loose_claimants[occurrence_index].len() != 1
                || reserved_occurrences.contains(&occurrence_index)
                || blocked_exact_occurrences.contains(&occurrence_index)
            {
                continue;
            }
            by_occurrence.insert(occurrences[occurrence_index].clone(), top_index);
        }

        Self {
            top_instances,
            by_occurrence,
        }
    }

    pub(super) fn get(
        &self,
        owner_definition: usize,
        child: &ModuleGraphInstance,
    ) -> Option<&'a InstanceModel> {
        let key = SourceOccurrenceKey::new(owner_definition, child);
        self.by_occurrence
            .get(&key)
            .and_then(|index| self.top_instances.get(*index))
    }
}

fn source_location_matches(instance: &InstanceModel, occurrence: &SourceOccurrenceKey) -> bool {
    instance.line != 0
        && occurrence.line != 0
        && instance.line == occurrence.line
        && instance.col == occurrence.col
        && instance.file.as_deref() == occurrence.file.as_deref()
}

fn source_name_matches(instance: &InstanceModel, occurrence: &SourceOccurrenceKey) -> bool {
    same_name(&instance.name, &occurrence.name)
        || clean_name(&instance.full_name)
            .rsplit('.')
            .next()
            .is_some_and(|leaf| same_name(leaf, &occurrence.name))
}
