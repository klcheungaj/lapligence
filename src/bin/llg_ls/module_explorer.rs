//! Read-only module hierarchy snapshots for the HDL Modules explorer.
//!
//! The snapshot is deliberately built from a committed [`DesignModel`].  It
//! does not parse, touch the filesystem, or inspect a live Slang handle, so
//! requests are cheap and cannot race an analysis job.  The model contains
//! elaborated instances (including concrete parameterized types), while the
//! analysis also retains a source definition/instance graph for definitions
//! omitted by configured elaboration.  Source-only nodes carry declaration
//! contents and remain useful even when no elaborated instance exists.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tower_lsp::lsp_types::Url;

use crate::features::{
    Analysis, ModuleGraphDefinition, ModuleGraphElaboratedType, ModuleGraphGenerateScope,
    ModuleGraphInstance, ModuleGraphLocation, ModuleGraphPackedRange, ModuleGraphTypeShape,
};
use llg::core::elab::Val;
use llg::core::model::{
    DesignModel, Direction, GenScopeModel, InstanceModel, ModuleDef, ParamModel, PortModel,
    SignalModel, TypeInfo,
};

mod catalog;
use catalog::{
    graph_topology, mapped_source_identity, GraphCatalog, GraphResolution, SourceElaborationLookup,
};
mod budget;
#[cfg(test)]
use budget::GUARANTEED_HIERARCHY_ROOT_SLOTS;
use budget::{
    collect_budgeted, compatibility_module, take_module_record, COMPATIBILITY_TERMINAL_SLOTS,
    GRAPH_TERMINAL_SLOTS, MAX_INSTANCE_NODES, MAX_SAFE_HIERARCHY_DEPTH,
};
pub(crate) use budget::{new_response_budget, InstanceBudget};
mod content;
use content::{
    definition_parameter, definition_port, graph_module, graph_parameter, graph_port,
    graph_signals, instance_signals, instance_signals_with_source,
};
mod hierarchy;
use hierarchy::{
    graph_cycle_root, graph_definition_children, graph_hierarchy_name, graph_instance_node,
};
#[cfg(test)]
use hierarchy::{graph_elaborated_scope, merge_source_scope};
mod compatibility;
use compatibility::{collect_instances_bounded, instance_node};
mod presentation;
use presentation::{remap_content_uris, remap_instance_uris, remap_uri};
mod types;
use types::{
    bracket_spans, clean_name, direction, explorer_location, explorer_type_with_context,
    find_definition_id, generate_scope_id, instance_id, instance_range, module_id, parameter,
    parameter_with_source, port, port_with_source, same_name, signal, signal_with_source,
    source_range, source_uri,
};
#[cfg(test)]
use types::{normalize_symbolic_expression, normalize_type_display, resolved_type_display};

fn is_false(value: &bool) -> bool {
    !value
}

/// A source range in the LSP's zero-based coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Declaration location for a module-content entry.  Unlike an instance's
/// `uri`/`range`, this is always the identifier range of the declaration
/// itself (for example `payload` in `logic [WIDTH-1:0] payload`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerLocation {
    pub uri: String,
    pub range: ExplorerRange,
}

/// A serializable HDL type.  Keeping width and signedness separate from the
/// display kind lets clients make useful decisions without parsing a detail
/// string such as `logic [7:0]`.  `displayType` is an optional, sanitized
/// source/type rendering: for an elaborated instance it uses the committed
/// `TypeInfo` rendering for known packed widths (for example `logic [7:0]`),
/// while an unresolved dimension remains normalized symbolic text (for
/// example `logic [WIDTH-1:0]`). It is additive; clients must continue to use
/// the structured fields when they only need kind, width, or signedness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerType {
    pub kind: String,
    pub width: Option<u32>,
    pub signed: bool,
    pub type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_type: Option<String>,
}

/// One typed module port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerPort {
    pub name: String,
    pub direction: String,
    #[serde(rename = "type")]
    pub ty: ExplorerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ExplorerLocation>,
}

/// One typed net, variable, or unpacked array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerSignal {
    pub name: String,
    pub kind: String,
    #[serde(rename = "type")]
    pub ty: ExplorerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ExplorerLocation>,
}

/// One resolved parameter/localparam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ExplorerType,
    pub value: Option<String>,
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ExplorerLocation>,
}

/// A module definition and its typed declaration contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerModule {
    /// Stable within a server session and independent of result ordering.
    pub id: String,
    pub name: String,
    pub uri: Option<String>,
    pub range: Option<ExplorerRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_source: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub is_budget_truncated: bool,
    pub ports: Vec<ExplorerPort>,
    pub params: Vec<ExplorerParameter>,
    pub signals: Vec<ExplorerSignal>,
}

/// A generated scope and the elaborated instances directly inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerGenerateScope {
    /// The concrete scope identity, for example `top.g[0]`.
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub is_budget_truncated: bool,
    pub params: Vec<ExplorerParameter>,
    pub children: Vec<ExplorerInstance>,
    /// Nested named generate blocks remain wrapper nodes instead of being
    /// flattened into the nearest instance list. This is additive to the
    /// original wire shape so older clients can continue to consume `children`.
    pub nested_scopes: Vec<ExplorerGenerateScope>,
}

/// One node in the recursive, multi-top instance snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerInstance {
    /// Derived from the elaborated full hierarchy name, not a vector index.
    pub id: String,
    pub instance_name: String,
    pub module_type: String,
    pub definition_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub is_budget_truncated: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub is_cycle_truncated: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub is_ambiguous: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub is_cycle_root: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_source: Option<String>,
    pub uri: Option<String>,
    pub range: Option<ExplorerRange>,
    pub ports: Vec<ExplorerPort>,
    pub params: Vec<ExplorerParameter>,
    pub signals: Vec<ExplorerSignal>,
    pub generated_scopes: Vec<ExplorerGenerateScope>,
    pub children: Vec<ExplorerInstance>,
}

/// The result of `llg/moduleExplorer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplorerSnapshot {
    pub modules: Vec<ExplorerModule>,
    pub roots: Vec<ExplorerInstance>,
}

/// Build one root-scoped snapshot from an already committed model.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn snapshot(root_id: &str, model: &DesignModel) -> ExplorerSnapshot {
    let mut budget = InstanceBudget::new(COMPATIBILITY_TERMINAL_SLOTS);
    snapshot_with_budget(root_id, model, &mut budget)
}

fn snapshot_with_budget(
    root_id: &str,
    model: &DesignModel,
    budget: &mut InstanceBudget,
) -> ExplorerSnapshot {
    let instances = collect_instances_bounded(&model.top_instances);

    let mut definition_ids: Vec<(String, String)> = model
        .modules
        .iter()
        .map(|module| {
            (
                clean_name(&module.name).to_owned(),
                module_id(root_id, module),
            )
        })
        .collect();
    // Duplicate module names are legal across source files.  Pick the same
    // definition for an instance regardless of how the model happened to
    // order those declarations.
    definition_ids.sort();
    let mut roots: Vec<ExplorerInstance> = Vec::new();
    for instance in &model.top_instances {
        if !budget.can_take_root() {
            if let Some(last) = roots.last_mut() {
                last.is_budget_truncated = true;
            }
            break;
        }
        let Some(root) = instance_node(root_id, instance, &definition_ids, budget, 0, true) else {
            if let Some(last) = roots.last_mut() {
                last.is_budget_truncated = true;
            }
            break;
        };
        roots.push(root);
    }

    let mut module_indices = (0..model.modules.len()).collect::<Vec<_>>();
    module_indices.sort_by(|left, right| {
        module_id(root_id, &model.modules[*left]).cmp(&module_id(root_id, &model.modules[*right]))
    });
    let mut modules = Vec::new();
    let module_count = module_indices.len();
    for (module_position, module_index) in module_indices.into_iter().enumerate() {
        if !take_module_record(
            root_id,
            &mut modules,
            budget,
            module_position + 1 < module_count,
        ) {
            break;
        }
        let module = &model.modules[module_index];
        let representative = instances
            .iter()
            .copied()
            .filter(|instance| same_name(&instance.def_name, &module.name))
            .min_by(|left, right| {
                clean_name(&left.full_name)
                    .cmp(clean_name(&right.full_name))
                    .then(left.name.cmp(&right.name))
            });
        modules.push(compatibility_module(
            root_id,
            module,
            representative,
            budget,
        ));
    }

    modules.sort_by(|left, right| left.id.cmp(&right.id));
    roots.sort_by(|left, right| left.id.cmp(&right.id));
    ExplorerSnapshot { modules, roots }
}

/// Build the production snapshot from one committed analysis.  The source
/// graph was captured during that analysis; this function only joins it with
/// retained elaborated instances and never parses or reads a project file.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn snapshot_analysis<F>(
    root_id: &str,
    analysis: &Analysis,
    source_map: F,
) -> ExplorerSnapshot
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    let mut budget = InstanceBudget::new(GRAPH_TERMINAL_SLOTS);
    snapshot_analysis_with_budget(root_id, analysis, source_map, &mut budget)
}

/// Build a snapshot while consuming a caller-owned response budget.  The LSP
/// request uses one instance of this budget for every analysis root, so a
/// multi-workspace response cannot exceed `MAX_INSTANCE_NODES` merely because
/// each root was individually below its own limit.
pub(crate) fn snapshot_analysis_with_budget<F>(
    root_id: &str,
    analysis: &Analysis,
    source_map: F,
    budget: &mut InstanceBudget,
) -> ExplorerSnapshot
where
    F: Fn(&Path) -> Option<PathBuf>,
{
    if analysis.module_graph.definitions.is_empty() {
        return snapshot_with_budget(root_id, &analysis.model, budget);
    }

    let catalog = GraphCatalog::new(root_id, &analysis.module_graph, &source_map);
    let instances = collect_instances_bounded(&analysis.model.top_instances);

    let topology = graph_topology(&catalog);
    let root_definitions = topology.roots.clone();
    let mut occurrences: Vec<(Option<usize>, Option<&InstanceModel>)> = Vec::new();
    let mut represented_top_instances = HashSet::new();

    // Every source root is represented, using an elaborated top instance when
    // one is available for that exact definition.  A definition can have
    // multiple elaborated top occurrences, so retain each occurrence.
    for definition_index in root_definitions {
        let matching = analysis
            .model
            .top_instances
            .iter()
            .enumerate()
            .filter(|(_, instance)| {
                matches!(catalog.resolve(&instance.def_name), GraphResolution::Unique(i) if i == definition_index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            occurrences.push((Some(definition_index), None));
        } else {
            for (instance_index, instance) in matching {
                represented_top_instances.insert(instance_index);
                occurrences.push((Some(definition_index), Some(instance)));
            }
        }
    }

    // Preserve only elaborated occurrences that have no unique source
    // definition to classify them. A uniquely resolved occurrence whose
    // definition has an incoming source edge is already represented below its
    // source root (or deliberately omitted when there is no source root).
    for (instance_index, instance) in analysis.model.top_instances.iter().enumerate() {
        if represented_top_instances.contains(&instance_index) {
            continue;
        }
        match catalog.resolve(&instance.def_name) {
            GraphResolution::Ambiguous | GraphResolution::Missing => {
                occurrences.push((None, Some(instance)));
            }
            GraphResolution::Unique(_) => {}
        }
    }

    let elaborated_lookup = SourceElaborationLookup::new(
        &catalog,
        &analysis.model.top_instances,
        &represented_top_instances,
    );

    let mut roots: Vec<ExplorerInstance> = Vec::new();
    for (definition_index, elaborated) in occurrences {
        if !budget.can_take_root() {
            if let Some(last) = roots.last_mut() {
                last.is_budget_truncated = true;
            }
            break;
        }
        let (instance_name, module_type, hierarchy, file, line, col, end_line, end_col) =
            match (definition_index, elaborated) {
                (Some(_index), Some(instance)) => (
                    instance.name.clone(),
                    clean_name(&instance.def_name).to_owned(),
                    graph_hierarchy_name(&instance.full_name, &instance.name),
                    instance.file.clone(),
                    instance.line,
                    instance.col,
                    instance.line,
                    instance
                        .col
                        .saturating_add(clean_name(&instance.def_name).chars().count() as u32),
                ),
                (Some(index), None) => {
                    let definition = &catalog.definitions[index];
                    (
                        definition.name.clone(),
                        definition.name.clone(),
                        definition.name.clone(),
                        definition.file.clone(),
                        definition.line,
                        definition.col,
                        definition.end_line,
                        definition.end_col,
                    )
                }
                (None, Some(instance)) => (
                    instance.name.clone(),
                    clean_name(&instance.def_name).to_owned(),
                    graph_hierarchy_name(&instance.full_name, &instance.name),
                    instance.file.clone(),
                    instance.line,
                    instance.col,
                    instance.line,
                    instance
                        .col
                        .saturating_add(clean_name(&instance.def_name).chars().count() as u32),
                ),
                (None, None) => continue,
            };
        let resolution = catalog.resolve(&module_type);
        let mut active = Vec::new();
        let Some(root) = graph_instance_node(
            root_id,
            &catalog,
            &source_map,
            hierarchy,
            instance_name,
            module_type,
            file,
            line,
            col,
            end_line,
            end_col,
            resolution,
            elaborated,
            &elaborated_lookup,
            &mut active,
            budget,
            0,
            true,
        ) else {
            if let Some(last) = roots.last_mut() {
                last.is_budget_truncated = true;
            }
            break;
        };
        roots.push(root);
    }

    for component in topology.cycle_components {
        if !budget.can_take_root() {
            if let Some(last) = roots.last_mut() {
                last.is_budget_truncated = true;
            }
            break;
        }
        if let Some(cycle_root) = graph_cycle_root(
            root_id,
            &catalog,
            &source_map,
            &component,
            &elaborated_lookup,
            budget,
        ) {
            roots.push(cycle_root);
        }
    }
    roots.sort_by(|left, right| left.id.cmp(&right.id));

    let mut module_indices = (0..catalog.definitions.len()).collect::<Vec<_>>();
    module_indices.sort_by(|left, right| catalog.ids[*left].cmp(&catalog.ids[*right]));
    let mut modules = Vec::new();
    let module_count = module_indices.len();
    for (module_position, index) in module_indices.into_iter().enumerate() {
        if !take_module_record(
            root_id,
            &mut modules,
            budget,
            module_position + 1 < module_count,
        ) {
            break;
        }
        let definition = &catalog.definitions[index];
        let representative = instances
            .iter()
            .copied()
            .filter(|instance| {
                matches!(catalog.resolve(&instance.def_name), GraphResolution::Unique(i) if i == index)
            })
            .min_by(|left, right| {
                clean_name(&left.full_name)
                    .cmp(clean_name(&right.full_name))
                    .then(left.name.cmp(&right.name))
            });
        modules.push(graph_module(
            root_id,
            definition,
            catalog.ids[index].clone(),
            representative,
            &catalog,
            budget,
        ));
    }
    modules.sort_by(|left, right| left.id.cmp(&right.id));
    ExplorerSnapshot { modules, roots }
}

/// Merge snapshots from independent analysis roots while preserving root
/// identity in instance and definition IDs.
pub(crate) fn merge(snapshots: impl IntoIterator<Item = ExplorerSnapshot>) -> ExplorerSnapshot {
    let mut modules = Vec::new();
    let mut roots = Vec::new();
    for snapshot in snapshots {
        modules.extend(snapshot.modules);
        roots.extend(snapshot.roots);
    }
    modules.sort_by(|left, right| left.id.cmp(&right.id));
    roots.sort_by(|left, right| left.id.cmp(&right.id));
    ExplorerSnapshot { modules, roots }
}

/// Remap file URIs in a snapshot from a root's private shadow tree to the
/// real project paths.  IDs intentionally remain untouched: they are based on
/// the committed model's hierarchy identity and remain stable across updates.
pub(crate) fn remap_uris(snapshot: &mut ExplorerSnapshot, map: impl Fn(&Path) -> Option<PathBuf>) {
    for module in &mut snapshot.modules {
        remap_uri(&mut module.uri, &map);
        remap_content_uris(
            &mut module.ports,
            &mut module.params,
            &mut module.signals,
            &map,
        );
    }
    for root in &mut snapshot.roots {
        remap_instance_uris(root, &map);
    }
}

fn remove_bracket_dimensions(text: &str) -> String {
    let spans = bracket_spans(text);
    if spans.is_empty() {
        return text.to_owned();
    }
    let mut result = String::new();
    let mut cursor = 0;
    for (start, end, _) in spans {
        result.push_str(&text[cursor..start]);
        cursor = end;
    }
    result.push_str(&text[cursor..]);
    result
}

#[cfg(test)]
mod tests;
