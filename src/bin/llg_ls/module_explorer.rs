//! Read-only module hierarchy snapshots for the HDL Modules explorer.
//!
//! The snapshot is deliberately built from a committed [`DesignModel`].  It
//! does not parse, touch the filesystem, or inspect a live Surelog handle, so
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
        roots.push(instance_node(
            root_id,
            instance,
            &definition_ids,
            budget,
            0,
            true,
        ));
    }

    let mut module_indices = (0..model.modules.len()).collect::<Vec<_>>();
    module_indices.sort_by(|left, right| {
        module_id(root_id, &model.modules[*left]).cmp(&module_id(root_id, &model.modules[*right]))
    });
    let mut modules = Vec::new();
    for module_index in module_indices {
        if !take_module_record(root_id, &mut modules, budget) {
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
    for index in module_indices {
        if !take_module_record(root_id, &mut modules, budget) {
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

struct GraphCatalog<'a> {
    definitions: &'a [ModuleGraphDefinition],
    elaborated_types: &'a [ModuleGraphElaboratedType],
    ids: Vec<String>,
    by_name: std::collections::HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphResolution {
    Unique(usize),
    Ambiguous,
    Missing,
}

impl<'a> GraphCatalog<'a> {
    fn new<F>(root_id: &str, graph: &'a crate::features::ModuleGraph, source_map: &F) -> Self
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

    fn resolve(&self, name: &str) -> GraphResolution {
        match self.by_name.get(clean_name(name)) {
            Some(indices) if indices.len() == 1 => GraphResolution::Unique(indices[0]),
            Some(indices) if !indices.is_empty() => GraphResolution::Ambiguous,
            _ => GraphResolution::Missing,
        }
    }

    fn packed_ranges(
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

fn mapped_source_identity<F>(file: Option<&str>, source_map: &F) -> String
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

/// Consume one shared response-budget slot for each serialized item in a
/// declaration/content array.  Arrays do not have a separate marker shape,
/// so the containing record is marked and the single global budget marker
/// stops all subsequent traversal when the prefix is exhausted.
fn collect_budgeted<I, U, F>(
    items: I,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
    map: F,
) -> Vec<U>
where
    I: IntoIterator,
    F: Fn(I::Item) -> U,
{
    let mut collected = Vec::new();
    for item in items {
        if !budget.take_regular() {
            *truncated = true;
            budget.take_budget_marker();
            break;
        }
        collected.push(map(item));
    }
    collected
}

fn take_module_record(
    root_id: &str,
    modules: &mut Vec<ExplorerModule>,
    budget: &mut InstanceBudget,
) -> bool {
    if budget.take_module() {
        return true;
    }
    if budget.take_module_marker() {
        modules.push(budget_module_marker(root_id));
    }
    false
}

fn budget_module_marker(root_id: &str) -> ExplorerModule {
    ExplorerModule {
        id: format!("module:{root_id}:<budget-truncated>"),
        name: "<budget-truncated>".to_owned(),
        uri: None,
        range: None,
        content_source: Some("budget-truncated".to_owned()),
        is_budget_truncated: true,
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
    }
}

fn compatibility_module(
    root_id: &str,
    module: &ModuleDef,
    representative: Option<&InstanceModel>,
    budget: &mut InstanceBudget,
) -> ExplorerModule {
    let mut is_budget_truncated = false;
    let (content_source, ports, params, signals) = if let Some(instance) = representative {
        (
            Some("elaborated".to_owned()),
            collect_budgeted(
                instance.ports.iter(),
                budget,
                &mut is_budget_truncated,
                port,
            ),
            collect_budgeted(
                instance.params.iter(),
                budget,
                &mut is_budget_truncated,
                parameter,
            ),
            instance_signals(instance, budget, &mut is_budget_truncated),
        )
    } else {
        (None, Vec::new(), Vec::new(), Vec::new())
    };
    ExplorerModule {
        id: module_id(root_id, module),
        name: clean_name(&module.name).to_owned(),
        uri: source_uri(module.file.as_deref()),
        range: source_range(module.line, module.col, module.end_line, module.end_col),
        content_source,
        is_budget_truncated,
        ports,
        params,
        signals,
    }
}

fn graph_module(
    _root_id: &str,
    definition: &ModuleGraphDefinition,
    id: String,
    representative: Option<&InstanceModel>,
    catalog: &GraphCatalog<'_>,
    budget: &mut InstanceBudget,
) -> ExplorerModule {
    if let Some(instance) = representative {
        let hierarchy = graph_hierarchy_name(&instance.full_name, &instance.name);
        let mut is_budget_truncated = false;
        let ports = collect_budgeted(
            instance.ports.iter(),
            budget,
            &mut is_budget_truncated,
            |item| {
                port_with_source(
                    item,
                    definition_port(definition, &item.name),
                    catalog.packed_ranges(&hierarchy, &item.name),
                )
            },
        );
        let params = collect_budgeted(
            instance.params.iter(),
            budget,
            &mut is_budget_truncated,
            |item| {
                parameter_with_source(
                    item,
                    definition_parameter(definition, &item.name),
                    catalog.packed_ranges(&hierarchy, &item.name),
                )
            },
        );
        let signals = instance_signals_with_source(
            instance,
            Some(definition),
            catalog,
            &hierarchy,
            budget,
            &mut is_budget_truncated,
        );
        return ExplorerModule {
            id,
            name: clean_name(&definition.name).to_owned(),
            uri: source_uri(definition.file.as_deref()),
            range: source_range(
                definition.line,
                definition.col,
                definition.end_line,
                definition.end_col,
            ),
            content_source: Some("elaborated".to_owned()),
            is_budget_truncated,
            ports,
            params,
            signals,
        };
    }
    let mut is_budget_truncated = false;
    let ports = collect_budgeted(
        definition.ports.iter(),
        budget,
        &mut is_budget_truncated,
        graph_port,
    );
    let params = collect_budgeted(
        definition.params.iter(),
        budget,
        &mut is_budget_truncated,
        graph_parameter,
    );
    let signals = graph_signals(definition, budget, &mut is_budget_truncated);
    ExplorerModule {
        id,
        name: clean_name(&definition.name).to_owned(),
        uri: source_uri(definition.file.as_deref()),
        range: source_range(
            definition.line,
            definition.col,
            definition.end_line,
            definition.end_col,
        ),
        content_source: Some("declaration".to_owned()),
        is_budget_truncated,
        ports,
        params,
        signals,
    }
}

struct GraphTopology {
    roots: Vec<usize>,
    cycle_components: Vec<Vec<usize>>,
}

/// Build the source graph topology once for root selection and cycle recovery.
/// Incoming edges retain the old name-based behavior for ordinary roots,
/// including duplicate-name ambiguity.  SCC discovery uses only uniquely
/// resolved edges, because an ambiguous module name cannot safely establish a
/// cycle or a definition identity.
fn graph_topology(catalog: &GraphCatalog<'_>) -> GraphTopology {
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
struct SourceOccurrenceKey {
    owner_definition: usize,
    name: String,
    module_type: String,
    file: Option<String>,
    line: u32,
    col: u32,
}

impl SourceOccurrenceKey {
    fn new(owner_definition: usize, child: &ModuleGraphInstance) -> Self {
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
struct SourceElaborationLookup<'a> {
    top_instances: &'a [InstanceModel],
    by_occurrence: HashMap<SourceOccurrenceKey, usize>,
}

impl<'a> SourceElaborationLookup<'a> {
    fn new(
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

    fn get(
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

fn graph_cycle_root<F>(
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

fn graph_definition_children(
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

fn graph_instance_node<F>(
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

fn graph_elaborated_scope<F>(
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
    // Surelog names an unlabeled source block `genblkN`, so its display name
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

fn merge_source_scope<F>(
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
    elaborated
        .children
        .sort_by(|left, right| left.id.cmp(&right.id));
    elaborated
        .nested_scopes
        .sort_by(|left, right| left.id.cmp(&right.id));
}

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

fn graph_hierarchy_name(full_name: &str, fallback: &str) -> String {
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

fn graph_port(port: &crate::features::ModuleGraphPort) -> ExplorerPort {
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type_with_context(
            &port.ty,
            port.display_type.as_deref(),
            Some(port.display_shape),
            None,
        ),
        detail: port.detail.clone(),
        location: port.location.as_ref().and_then(explorer_location),
    }
}

fn graph_signal(signal: &crate::features::ModuleGraphSignal) -> ExplorerSignal {
    ExplorerSignal {
        name: signal.name.clone(),
        kind: signal.kind.clone(),
        ty: explorer_type_with_context(
            &signal.ty,
            signal.display_type.as_deref(),
            Some(signal.display_shape),
            None,
        ),
        detail: signal.detail.clone(),
        location: signal.location.as_ref().and_then(explorer_location),
    }
}

fn graph_parameter(parameter: &crate::features::ModuleGraphParameter) -> ExplorerParameter {
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type_with_context(
            &parameter.ty,
            parameter.display_type.as_deref(),
            Some(parameter.display_shape),
            None,
        ),
        value: None,
        local: parameter.local,
        detail: parameter.detail.clone(),
        location: parameter.location.as_ref().and_then(explorer_location),
    }
}

fn instance_signals(
    instance: &InstanceModel,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = instance
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        instance
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        signal,
    )
}

fn instance_signals_with_source(
    instance: &InstanceModel,
    definition: Option<&ModuleGraphDefinition>,
    catalog: &GraphCatalog<'_>,
    hierarchy: &str,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = instance
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        instance
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        |signal| {
            signal_with_source(
                signal,
                definition.and_then(|definition| definition_signal(definition, &signal.name)),
                catalog.packed_ranges(hierarchy, &signal.name),
            )
        },
    )
}

fn graph_signals(
    definition: &ModuleGraphDefinition,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = definition
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        definition
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        graph_signal,
    )
}

fn definition_port<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphPort> {
    definition.ports.iter().find(|port| port.name == name)
}

fn definition_signal<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphSignal> {
    definition.signals.iter().find(|signal| signal.name == name)
}

fn definition_parameter<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphParameter> {
    definition
        .params
        .iter()
        .find(|parameter| parameter.name == name)
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

const MAX_INSTANCE_NODES: usize = 10_000;
/// Keep one hierarchy slot out of the ordinary catalog budget. Additional
/// roots consume ordinary slots when available, but this floor lets the first
/// root survive a catalog/content truncation in a shared response.
const GUARANTEED_HIERARCHY_ROOT_SLOTS: usize = 1;
/// The complete workspace response can contain several analysis roots. Keep
/// a small bounded pool for their first hierarchy records without taking a
/// material share of the module catalog budget.
const RESPONSE_HIERARCHY_ROOT_SLOTS: usize = 64;
/// Keep enough definition records for ordinary editor workspaces even when a
/// very large hierarchy consumes every ordinary response slot. One reserved
/// slot is retained for an explicit module-catalog truncation marker.
const RESPONSE_MODULE_CATALOG_SLOTS: usize = 256;
/// Independent from the serialized-node budget, this guard bounds native
/// call-stack use while walking malformed source/elaboration trees. Normal
/// hierarchies rarely approach it; a deeper hierarchy is represented by the
/// single shared truncation marker even when node budget remains.
const MAX_SAFE_HIERARCHY_DEPTH: usize = 64;
#[cfg_attr(not(test), allow(dead_code))]
const COMPATIBILITY_TERMINAL_SLOTS: usize = 1;
const GRAPH_TERMINAL_SLOTS: usize = 2;

/// Create the budget used by the complete `llg/moduleExplorer` response.
/// Keeping construction here prevents the LSP layer from depending on the
/// accounting details while still allowing it to share one budget across
/// independent analysis roots.
pub(crate) fn new_response_budget() -> InstanceBudget {
    InstanceBudget::with_reserved_slots(
        GRAPH_TERMINAL_SLOTS,
        RESPONSE_HIERARCHY_ROOT_SLOTS,
        RESPONSE_MODULE_CATALOG_SLOTS,
    )
}

fn is_false(value: &bool) -> bool {
    !value
}

/// The serialized instance budget is shared by one complete snapshot.  Keep
/// terminal slots available so a cycle can still be represented when the
/// ordinary expansion slots have been consumed; a budget marker uses one of
/// the same slots and then stops all remaining sibling traversal.
pub(crate) struct InstanceBudget {
    remaining: usize,
    terminal_slots: usize,
    hierarchy_root_slots: usize,
    future_hierarchy_root_slots: usize,
    module_catalog_slots: usize,
    stopped: bool,
    budget_marker_emitted: bool,
    module_marker_emitted: bool,
}

impl InstanceBudget {
    fn new(terminal_slots: usize) -> Self {
        Self::with_reserved_slots(terminal_slots, GUARANTEED_HIERARCHY_ROOT_SLOTS, 2)
    }

    fn with_reserved_slots(
        terminal_slots: usize,
        hierarchy_root_slots: usize,
        module_catalog_slots: usize,
    ) -> Self {
        let terminal_slots = terminal_slots.min(MAX_INSTANCE_NODES);
        let hierarchy_root_slots =
            hierarchy_root_slots.min(MAX_INSTANCE_NODES.saturating_sub(terminal_slots));
        Self {
            remaining: MAX_INSTANCE_NODES,
            terminal_slots,
            hierarchy_root_slots,
            future_hierarchy_root_slots: 0,
            module_catalog_slots: module_catalog_slots.min(
                MAX_INSTANCE_NODES
                    .saturating_sub(terminal_slots)
                    .saturating_sub(hierarchy_root_slots),
            ),
            stopped: false,
            budget_marker_emitted: false,
            module_marker_emitted: false,
        }
    }

    fn should_stop(&self) -> bool {
        self.stopped || self.remaining == 0
    }

    fn reserved_root_slots(&self) -> usize {
        self.hierarchy_root_slots
            .saturating_add(self.future_hierarchy_root_slots)
    }

    /// Divide the root reserve across a known set of workspace snapshots.
    /// Call [`Self::begin_workspace`] before serializing each one.
    pub(crate) fn prepare_workspaces(&mut self) {
        self.future_hierarchy_root_slots = self
            .future_hierarchy_root_slots
            .saturating_add(self.hierarchy_root_slots);
        self.hierarchy_root_slots = 0;
    }

    /// Assign a fair share of the still-reserved hierarchy roots to the next
    /// workspace. Unused capacity from the previous workspace is returned to
    /// the pool before the split.
    pub(crate) fn begin_workspace(&mut self, remaining_workspaces: usize) {
        self.future_hierarchy_root_slots = self
            .future_hierarchy_root_slots
            .saturating_add(self.hierarchy_root_slots);
        self.hierarchy_root_slots = 0;
        if remaining_workspaces == 0 || self.future_hierarchy_root_slots == 0 {
            return;
        }
        let quota = self
            .future_hierarchy_root_slots
            .div_ceil(remaining_workspaces);
        self.future_hierarchy_root_slots -= quota;
        self.hierarchy_root_slots = quota;
    }

    fn take_regular(&mut self) -> bool {
        if self.stopped
            || self.remaining
                <= self
                    .terminal_slots
                    .saturating_add(self.reserved_root_slots())
                    .saturating_add(self.module_catalog_slots)
        {
            return false;
        }
        self.remaining -= 1;
        true
    }

    fn can_take_root(&self) -> bool {
        (self.hierarchy_root_slots > 0
            && self.remaining
                > self
                    .future_hierarchy_root_slots
                    .saturating_add(self.module_catalog_slots))
            || (!self.stopped
                && self.remaining
                    > self
                        .terminal_slots
                        .saturating_add(self.future_hierarchy_root_slots)
                        .saturating_add(self.module_catalog_slots))
    }

    fn take_root(&mut self) -> bool {
        if !self.stopped
            && self.remaining
                > self
                    .terminal_slots
                    .saturating_add(self.reserved_root_slots())
                    .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            return true;
        }
        if self.hierarchy_root_slots > 0
            && self.remaining
                > self
                    .future_hierarchy_root_slots
                    .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            self.hierarchy_root_slots -= 1;
            return true;
        }
        false
    }

    fn take_module(&mut self) -> bool {
        if !self.stopped
            && self.remaining
                > self
                    .terminal_slots
                    .saturating_add(self.reserved_root_slots())
                    .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            return true;
        }
        // Keep the last catalog slot for an explicit marker if there are more
        // definitions than the reserved prefix can represent.
        if self.module_catalog_slots > 1 && self.remaining > self.reserved_root_slots() {
            self.remaining -= 1;
            self.module_catalog_slots -= 1;
            return true;
        }
        false
    }

    fn take_terminal(&mut self) -> bool {
        if self.stopped
            || self.remaining
                <= self
                    .reserved_root_slots()
                    .saturating_add(self.module_catalog_slots)
        {
            self.stopped = true;
            return false;
        }
        self.remaining -= 1;
        self.terminal_slots = self.terminal_slots.saturating_sub(1);
        if self.remaining == 0 {
            self.stopped = true;
        }
        true
    }

    fn take_budget_marker(&mut self) -> bool {
        if self.stopped || self.budget_marker_emitted || self.remaining == 0 {
            self.stopped = true;
            return false;
        }
        // A truncation marker is itself a serialized hierarchy node. It may
        // consume an ordinary slot while there is room, or one of the
        // reserved terminal slots at the normal budget boundary. Do not
        // consume the final hierarchy slot: a later root must remain
        // representable even when this marker stops ordinary traversal.
        let ordinary_slots = self.remaining.saturating_sub(
            self.terminal_slots
                .saturating_add(self.reserved_root_slots())
                .saturating_add(self.module_catalog_slots),
        );
        if ordinary_slots > 0 {
            self.remaining -= 1;
        } else if self.terminal_slots > 0 {
            self.remaining -= 1;
            self.terminal_slots = self.terminal_slots.saturating_sub(1);
        } else if self.hierarchy_root_slots > 1 {
            // A cycle may have consumed every terminal slot before ordinary
            // traversal reaches its boundary. Spend one extra root slot for
            // the marker, but retain a root floor for the response.
            self.remaining -= 1;
            self.hierarchy_root_slots -= 1;
        } else {
            self.stopped = true;
            return false;
        }
        self.budget_marker_emitted = true;
        self.stopped = true;
        true
    }

    fn take_module_marker(&mut self) -> bool {
        if self.module_marker_emitted
            || self.module_catalog_slots == 0
            || self.remaining <= self.reserved_root_slots()
        {
            return false;
        }
        self.remaining -= 1;
        self.module_catalog_slots -= 1;
        self.module_marker_emitted = true;
        true
    }
}

struct InstanceWalkFrame<'a> {
    instance: &'a InstanceModel,
    entered: bool,
    next_child: usize,
    next_scope: usize,
    next_scope_child: usize,
}

impl<'a> InstanceWalkFrame<'a> {
    fn new(instance: &'a InstanceModel) -> Self {
        Self {
            instance,
            entered: false,
            next_child: 0,
            next_scope: 0,
            next_scope_child: 0,
        }
    }
}

/// Collect representative instances without recursive calls or an
/// unbounded pending list.  The traversal order matches the old pre-order:
/// direct children come before generated-scope children.
fn collect_instances_bounded<'a>(roots: &'a [InstanceModel]) -> Vec<&'a InstanceModel> {
    let mut out = Vec::new();
    let mut root_index = 0;
    let mut stack = Vec::new();

    while out.len() < MAX_INSTANCE_NODES {
        if stack.is_empty() {
            let Some(root) = roots.get(root_index) else {
                break;
            };
            root_index += 1;
            stack.push(InstanceWalkFrame::new(root));
        }

        let next_instance = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            if frame.entered {
                None
            } else {
                frame.entered = true;
                Some(frame.instance)
            }
        };
        if let Some(instance) = next_instance {
            out.push(instance);
            continue;
        }

        let next_child = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            if frame.next_child < frame.instance.children.len() {
                let child = &frame.instance.children[frame.next_child];
                frame.next_child += 1;
                Some(child)
            } else {
                None
            }
        };
        if let Some(child) = next_child {
            stack.push(InstanceWalkFrame::new(child));
            continue;
        }

        let next_generated_child = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            let mut next = None;
            while frame.next_scope < frame.instance.gen_scopes.len() {
                let scope = &frame.instance.gen_scopes[frame.next_scope];
                if frame.next_scope_child < scope.children.len() {
                    let child = &scope.children[frame.next_scope_child];
                    frame.next_scope_child += 1;
                    next = Some(child);
                    break;
                }
                frame.next_scope += 1;
                frame.next_scope_child = 0;
            }
            next
        };
        if let Some(child) = next_generated_child {
            stack.push(InstanceWalkFrame::new(child));
            continue;
        }

        stack.pop();
    }

    out
}

fn instance_node(
    root_id: &str,
    instance: &InstanceModel,
    definition_ids: &[(String, String)],
    budget: &mut InstanceBudget,
    depth: usize,
    root: bool,
) -> ExplorerInstance {
    let id = instance_id(root_id, instance);
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        let node = compatibility_budget_marker(root_id, instance, definition_ids);
        budget.take_budget_marker();
        return node;
    }
    let budget_slot = if root {
        budget.take_root()
    } else {
        budget.take_regular()
    };
    if !budget_slot {
        let node = compatibility_budget_marker(root_id, instance, definition_ids);
        budget.take_budget_marker();
        return node;
    }

    let mut children = Vec::new();
    let omitted_descendants =
        budget.should_stop() && (!instance.children.is_empty() || !instance.gen_scopes.is_empty());
    for child in &instance.children {
        if budget.should_stop() {
            break;
        }
        children.push(instance_node(
            root_id,
            child,
            definition_ids,
            budget,
            depth.saturating_add(1),
            false,
        ));
        if budget.should_stop() {
            break;
        }
    }
    let mut generated_scopes = Vec::new();
    if !budget.should_stop() {
        for scope in &instance.gen_scopes {
            match generate_scope(root_id, &id, scope, definition_ids, budget, depth) {
                Some(generated) => generated_scopes.push(generated),
                None => {
                    append_compatibility_budget_marker(&id, scope, &mut children, budget);
                    break;
                }
            }
            if budget.should_stop() {
                break;
            }
        }
    }
    children.sort_by(|left, right| left.id.cmp(&right.id));
    generated_scopes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut content_truncated = false;
    let ports = collect_budgeted(instance.ports.iter(), budget, &mut content_truncated, port);
    let params = collect_budgeted(
        instance.params.iter(),
        budget,
        &mut content_truncated,
        parameter,
    );
    let signals = instance_signals(instance, budget, &mut content_truncated);

    ExplorerInstance {
        id,
        instance_name: instance.name.clone(),
        module_type: clean_name(&instance.def_name).to_owned(),
        definition_id: find_definition_id(definition_ids, &instance.def_name),
        is_budget_truncated: content_truncated || omitted_descendants,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: source_uri(instance.file.as_deref()),
        range: instance_range(instance),
        ports,
        params,
        signals,
        generated_scopes,
        children,
    }
}

fn generate_scope(
    root_id: &str,
    parent_id: &str,
    scope: &GenScopeModel,
    definition_ids: &[(String, String)],
    budget: &mut InstanceBudget,
    depth: usize,
) -> Option<ExplorerGenerateScope> {
    if !budget.take_regular() {
        return None;
    }
    let id = generate_scope_id(root_id, parent_id, scope);
    let mut children = Vec::new();
    let mut is_budget_truncated = false;
    let params = collect_budgeted(
        scope.params.iter(),
        budget,
        &mut is_budget_truncated,
        parameter,
    );
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        append_compatibility_budget_marker(parent_id, scope, &mut children, budget);
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
        children.push(instance_node(
            root_id,
            child,
            definition_ids,
            budget,
            depth.saturating_add(1),
            false,
        ));
        if budget.should_stop() {
            break;
        }
    }
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

fn append_compatibility_budget_marker(
    parent_id: &str,
    scope: &GenScopeModel,
    children: &mut Vec<ExplorerInstance>,
    budget: &mut InstanceBudget,
) {
    if !budget.take_budget_marker() {
        return;
    }
    children.push(ExplorerInstance {
        id: format!("{parent_id}:generate:{}:budget-truncated", scope.name),
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

fn compatibility_budget_marker(
    root_id: &str,
    instance: &InstanceModel,
    definition_ids: &[(String, String)],
) -> ExplorerInstance {
    ExplorerInstance {
        id: instance_id(root_id, instance),
        instance_name: instance.name.clone(),
        module_type: clean_name(&instance.def_name).to_owned(),
        definition_id: find_definition_id(definition_ids, &instance.def_name),
        is_budget_truncated: true,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: source_uri(instance.file.as_deref()),
        range: instance_range(instance),
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        generated_scopes: Vec::new(),
        children: Vec::new(),
    }
}

fn remap_instance_uris(instance: &mut ExplorerInstance, map: &impl Fn(&Path) -> Option<PathBuf>) {
    remap_uri(&mut instance.uri, map);
    remap_content_uris(
        &mut instance.ports,
        &mut instance.params,
        &mut instance.signals,
        map,
    );
    for generated in &mut instance.generated_scopes {
        remap_scope_uris(generated, map);
    }
    for child in &mut instance.children {
        remap_instance_uris(child, map);
    }
}

fn remap_scope_uris(scope: &mut ExplorerGenerateScope, map: &impl Fn(&Path) -> Option<PathBuf>) {
    for parameter in &mut scope.params {
        remap_location(&mut parameter.location, map);
    }
    for child in &mut scope.children {
        remap_instance_uris(child, map);
    }
    for nested in &mut scope.nested_scopes {
        remap_scope_uris(nested, map);
    }
}

fn remap_content_uris(
    ports: &mut [ExplorerPort],
    params: &mut [ExplorerParameter],
    signals: &mut [ExplorerSignal],
    map: &impl Fn(&Path) -> Option<PathBuf>,
) {
    for port in ports {
        remap_location(&mut port.location, map);
    }
    for parameter in params {
        remap_location(&mut parameter.location, map);
    }
    for signal in signals {
        remap_location(&mut signal.location, map);
    }
}

fn remap_location(
    location: &mut Option<ExplorerLocation>,
    map: &impl Fn(&Path) -> Option<PathBuf>,
) {
    let Some(location) = location else {
        return;
    };
    let Ok(parsed) = Url::parse(&location.uri) else {
        return;
    };
    let Ok(path) = parsed.to_file_path() else {
        return;
    };
    let Some(mapped) = map(&path) else {
        return;
    };
    if let Ok(uri) = Url::from_file_path(mapped) {
        location.uri = uri.to_string();
    }
}

fn remap_uri(uri: &mut Option<String>, map: &impl Fn(&Path) -> Option<PathBuf>) {
    let Some(value) = uri.as_deref() else {
        return;
    };
    let Ok(parsed) = Url::parse(value) else {
        return;
    };
    let Ok(path) = parsed.to_file_path() else {
        return;
    };
    if let Some(mapped) = map(&path).and_then(|path| Url::from_file_path(path).ok()) {
        *uri = Some(mapped.to_string());
    }
}

fn module_id(root_id: &str, module: &ModuleDef) -> String {
    format!(
        "module:{root_id}:{}:{}:{}:{}:{}:{}",
        clean_name(&module.name),
        module.line,
        module.col,
        module.end_line,
        module.end_col,
        source_identity(module.file.as_deref()),
    )
}

fn instance_id(root_id: &str, instance: &InstanceModel) -> String {
    let hierarchy = clean_name(&instance.full_name);
    let hierarchy = if hierarchy.is_empty() {
        instance.name.as_str()
    } else {
        hierarchy
    };
    format!("instance:{root_id}:{hierarchy}")
}

fn generate_scope_id(root_id: &str, parent_id: &str, scope: &GenScopeModel) -> String {
    let hierarchy = clean_name(&scope.full_name);
    if hierarchy.is_empty() {
        format!("{parent_id}:generate:{}", scope.name)
    } else {
        format!("generate:{root_id}:{hierarchy}")
    }
}

fn find_definition_id(definitions: &[(String, String)], name: &str) -> Option<String> {
    let name = clean_name(name);
    definitions
        .iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, id)| id.clone())
}

fn same_name(left: &str, right: &str) -> bool {
    clean_name(left) == clean_name(right)
}

fn clean_name(name: &str) -> &str {
    name.split_once('@').map(|(_, rest)| rest).unwrap_or(name)
}

fn source_identity(file: Option<&str>) -> String {
    file.unwrap_or("<unknown>").to_owned()
}

fn source_uri(file: Option<&str>) -> Option<String> {
    let file = file?;
    Url::from_file_path(file)
        .ok()
        .map(|uri| uri.to_string())
        .or_else(|| Some(file.to_owned()))
}

fn source_range(line: u32, col: u32, end_line: u32, end_col: u32) -> Option<ExplorerRange> {
    if line == 0 || col == 0 {
        return None;
    }
    Some(ExplorerRange {
        start_line: line.saturating_sub(1),
        start_character: col.saturating_sub(1),
        end_line: end_line.max(line).saturating_sub(1),
        end_character: end_col.max(col).saturating_sub(1),
    })
}

fn instance_range(instance: &InstanceModel) -> Option<ExplorerRange> {
    let displayed_type = if instance.def_name.is_empty() {
        instance.name.as_str()
    } else {
        clean_name(&instance.def_name)
    };
    let end_col = instance
        .col
        .saturating_add(displayed_type.chars().count() as u32);
    source_range(instance.line, instance.col, instance.line, end_col)
}

fn explorer_type(ty: &TypeInfo) -> ExplorerType {
    explorer_type_with_context(ty, None, None, None)
}

fn explorer_type_with_context(
    ty: &TypeInfo,
    symbolic_display: Option<&str>,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerType {
    ExplorerType {
        kind: ty.kind.clone(),
        width: ty.width,
        signed: ty.signed,
        type_name: ty.type_name.clone(),
        display_type: resolved_type_display(ty, symbolic_display, display_shape, concrete_ranges),
    }
}

fn direction(direction: Direction) -> String {
    match direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "none",
    }
    .to_owned()
}

fn port(port: &PortModel) -> ExplorerPort {
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type(&port.ty),
        detail: None,
        location: None,
    }
}

fn signal(signal: &SignalModel) -> ExplorerSignal {
    ExplorerSignal {
        name: signal.name.clone(),
        kind: signal.kind.clone(),
        ty: explorer_type(&signal.ty),
        detail: None,
        location: None,
    }
}

fn parameter(parameter: &ParamModel) -> ExplorerParameter {
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type(&parameter.ty),
        value: parameter.value.as_ref().map(Val::format_verilog),
        local: parameter.local,
        detail: None,
        location: None,
    }
}

fn port_with_source(
    port: &PortModel,
    source: Option<&crate::features::ModuleGraphPort>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerPort {
    let ty = if port.ty.kind == "other" {
        source.map_or(&port.ty, |source| &source.ty)
    } else {
        &port.ty
    };
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        // Elaborated contents deliberately keep their source text in
        // `displayType`; the legacy detail field stays declaration-fallback
        // text so older clients can distinguish the two sources.
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

fn signal_with_source(
    signal: &SignalModel,
    source: Option<&crate::features::ModuleGraphSignal>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerSignal {
    let kind = if signal.kind == "net" {
        source
            .map(|source| source.kind.as_str())
            .unwrap_or(signal.kind.as_str())
    } else {
        signal.kind.as_str()
    };
    let ty = if signal.ty.kind == "other" {
        source.map_or(&signal.ty, |source| &source.ty)
    } else {
        &signal.ty
    };
    ExplorerSignal {
        name: signal.name.clone(),
        kind: kind.to_owned(),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

fn parameter_with_source(
    parameter: &ParamModel,
    source: Option<&crate::features::ModuleGraphParameter>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerParameter {
    let ty = if parameter.ty.kind == "other" {
        source.map_or(&parameter.ty, |source| &source.ty)
    } else {
        &parameter.ty
    };
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        value: parameter.value.as_ref().map(Val::format_verilog),
        local: parameter.local,
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

fn explorer_location(location: &ModuleGraphLocation) -> Option<ExplorerLocation> {
    Some(ExplorerLocation {
        uri: source_uri(Some(location.file.as_str()))?,
        range: source_range(
            location.line,
            location.col,
            location.end_line,
            location.end_col,
        )?,
    })
}

fn resolved_type_display(
    ty: &TypeInfo,
    symbolic_display: Option<&str>,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> Option<String> {
    let fallback = symbolic_display.map(normalize_type_display);
    if let Some(source) = fallback {
        if source.contains('[') {
            return Some(render_concrete_type_display(
                &source,
                display_shape,
                concrete_ranges,
            ));
        }
        // A scalar source spelling is authoritative for named typedefs and
        // source qualifiers such as `wire`.  Do not replace it with the
        // underlying elaborated TypeInfo (`logic`, for example).
        return Some(source);
    }
    (ty.kind != "other").then(|| ty.render())
}

/// Render canonical captured packed ranges while retaining source unpacked
/// dimensions. The source string is only a normalized spelling fallback; no
/// request-time expression evaluation is performed here.
fn render_concrete_type_display(
    source: &str,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> String {
    let spans = bracket_spans(source);
    if spans.is_empty() {
        return source.to_owned();
    }

    let (packed_count, unpacked_count) = display_shape
        .filter(|shape| {
            shape
                .packed_dimensions
                .saturating_add(shape.unpacked_dimensions)
                > 0
        })
        .map_or_else(
            || {
                // Before shape metadata was added, graph entries only had a
                // flat list of packed dimensions. Treat all source brackets
                // as packed for that legacy representation.
                (spans.len(), 0)
            },
            |shape| {
                let packed = shape.packed_dimensions.min(spans.len());
                let remaining = spans.len().saturating_sub(packed);
                (packed, shape.unpacked_dimensions.min(remaining))
            },
        );
    let source_base = remove_bracket_dimensions(source).trim().to_owned();
    let mut display = source_base;

    for (index, (start, end, expression)) in spans.iter().take(packed_count).enumerate() {
        let concrete = concrete_ranges
            .and_then(|ranges| ranges.get(index))
            .and_then(|range| range.as_ref())
            .map(|range| format!("[{}:{}]", range.left, range.right));
        let dimension =
            concrete.unwrap_or_else(|| format!("[{}]", normalize_symbolic_expression(expression)));
        // `start`/`end` are intentionally used only to document that the
        // source span is retained per dimension; the normalized expression is
        // already owned by `bracket_spans` and is UTF-8 safe.
        let _ = (start, end);
        append_display_part(&mut display, &dimension);
    }

    let first_unpacked = packed_count;
    let last_unpacked = first_unpacked
        .saturating_add(unpacked_count)
        .min(spans.len());
    for (start, end, _) in &spans[first_unpacked..last_unpacked] {
        if let Some(dimension) = source.get(*start..*end) {
            append_display_part(&mut display, &normalize_type_display(dimension));
        }
    }
    display
}

fn append_display_part(display: &mut String, part: &str) {
    if !display.is_empty() {
        display.push(' ');
    }
    display.push_str(part);
}

fn normalize_type_display(text: &str) -> String {
    if contains_comment(text) {
        return text.trim().to_owned();
    }

    let spans = bracket_spans(text);
    if spans.is_empty() {
        return text.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    let mut normalized = String::new();
    let mut cursor = 0;
    for (start, end, expression) in spans {
        let prefix = text[cursor..start]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !prefix.is_empty() {
            append_display_part(&mut normalized, &prefix);
        }
        append_display_part(
            &mut normalized,
            &format!("[{}]", normalize_symbolic_expression(&expression)),
        );
        cursor = end;
    }
    let suffix = text[cursor..]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !suffix.is_empty() {
        append_display_part(&mut normalized, &suffix);
    }
    normalized
}

fn normalize_symbolic_expression(expression: &str) -> String {
    if contains_comment(expression) {
        return expression.trim().to_owned();
    }

    let chars = expression.chars().collect::<Vec<_>>();
    let mut normalized = String::new();
    let mut pending_space = false;
    let mut pending_after_escaped_identifier = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in chars.iter().copied().enumerate() {
        if in_string {
            normalized.push(character);
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        // SystemVerilog escaped identifiers start with `\\` and end at the
        // first whitespace. Keep that terminating separator even when the
        // following token is punctuation.
        if escaped_identifier {
            if character.is_whitespace() {
                pending_space = true;
                pending_after_escaped_identifier = true;
                escaped_identifier = false;
            } else {
                normalized.push(character);
            }
            continue;
        }

        if character.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space {
            if should_retain_symbolic_separator(
                &normalized,
                &chars,
                index,
                pending_after_escaped_identifier,
            ) && !normalized.ends_with(' ')
            {
                normalized.push(' ');
            }
            pending_space = false;
            pending_after_escaped_identifier = false;
        }

        normalized.push(character);
        if character == '\\' {
            escaped_identifier = true;
        } else if character == '"' {
            in_string = true;
            escaped_string_character = false;
        }
    }

    if pending_space && pending_after_escaped_identifier && !normalized.ends_with(' ') {
        normalized.push(' ');
    }
    normalized
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn should_retain_symbolic_separator(
    normalized: &str,
    chars: &[char],
    next_index: usize,
    after_escaped_identifier: bool,
) -> bool {
    if after_escaped_identifier {
        return true;
    }

    let Some(previous) = normalized.chars().last() else {
        return false;
    };
    let Some(next) = chars.get(next_index).copied() else {
        return false;
    };

    if is_word_character(previous) && is_word_character(next) {
        return true;
    }

    let previous_word = normalized
        .rsplit(|character: char| !is_word_character(character))
        .next()
        .filter(|word| !word.is_empty());
    let next_word = next_symbolic_word(chars, next_index);
    if previous_word.is_some_and(is_dimension_keyword)
        || next_word.as_deref().is_some_and(is_dimension_keyword)
    {
        return true;
    }

    // A backslash starts a new escaped identifier. Keep a separator before
    // it as well; otherwise a preceding identifier could join the escaped
    // token.
    if next == '\\' {
        return true;
    }

    // Do not merge two operators into a different token (`+ +` → `++`,
    // `/ *` → `/*`, `: :` → `::`, and so on).
    if operator_pair_requires_separator(previous, next) {
        return true;
    }

    // Compact the known punctuation alphabet, while conservatively retaining
    // a separator around an unfamiliar character.
    !is_known_symbolic_character(previous) || !is_known_symbolic_character(next)
}

fn next_symbolic_word(chars: &[char], start: usize) -> Option<String> {
    let mut index = start;
    while chars
        .get(index)
        .is_some_and(|character| character.is_whitespace())
    {
        index += 1;
    }
    let word_start = index;
    while chars
        .get(index)
        .is_some_and(|character| is_word_character(*character))
    {
        index += 1;
    }
    (index > word_start).then(|| chars[word_start..index].iter().collect())
}

fn is_dimension_keyword(word: &str) -> bool {
    word.eq_ignore_ascii_case("inside")
}

fn is_known_symbolic_character(character: char) -> bool {
    is_word_character(character)
        || matches!(
            character,
            '"' | '\''
                | '['
                | ']'
                | '{'
                | '}'
                | '('
                | ')'
                | ','
                | ':'
                | ';'
                | '.'
                | '?'
                | '+'
                | '-'
                | '*'
                | '/'
                | '%'
                | '&'
                | '|'
                | '^'
                | '~'
                | '!'
                | '='
                | '<'
                | '>'
        )
}

fn operator_pair_requires_separator(previous: char, next: char) -> bool {
    matches!(
        (previous, next),
        ('+', '+' | '=')
            | ('-', '-' | '=' | '>' | ':')
            | ('*', '*' | '=' | '/' | '>')
            | ('/', '/' | '*' | '=')
            | ('%', '=')
            | ('&', '&' | '=')
            | ('|', '|' | '=' | '-' | '>')
            | ('^', '^' | '=' | '~')
            | ('~', '^' | '=')
            | ('!', '!' | '=')
            | ('=', '=' | '<' | '>')
            | ('<', '<' | '=' | '>')
            | ('>', '>' | '=')
            | ('?', '?')
            | (':', ':' | '=' | '+' | '-')
            | ('.', '.' | '*')
    )
}

fn contains_comment(text: &str) -> bool {
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in text.char_indices() {
        if in_string {
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if escaped_identifier {
            if character.is_whitespace() {
                escaped_identifier = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            escaped_string_character = false;
            continue;
        }
        if character == '\\' {
            escaped_identifier = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("//") {
            return true;
        }
        if character == '/' && text[index..].starts_with("/*") {
            return true;
        }
    }
    false
}

fn bracket_spans(text: &str) -> Vec<(usize, usize, String)> {
    let mut spans = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    for (index, character) in text.char_indices() {
        if in_line_comment {
            if matches!(character, '\n' | '\r') {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && text[index..].starts_with("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if escaped_identifier {
            if character.is_whitespace() {
                escaped_identifier = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            escaped_string_character = false;
            continue;
        }
        if character == '\\' {
            escaped_identifier = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("//") {
            in_line_comment = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("/*") {
            in_block_comment = true;
            continue;
        }
        match character {
            '[' if depth == 0 => {
                start = Some(index);
                depth = 1;
            }
            '[' if depth > 0 => depth += 1,
            ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = start.take() {
                        let expression = &text[start + 1..index];
                        spans.push((start, index + character.len_utf8(), expression.to_owned()));
                    }
                }
            }
            _ => {}
        }
    }
    spans
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
mod tests {
    use super::*;
    use crate::features::{
        Analysis, ModuleGraph, ModuleGraphDefinition, ModuleGraphElaboratedType,
        ModuleGraphGenerateScope, ModuleGraphInstance, ModuleGraphLocation, ModuleGraphPackedRange,
        ModuleGraphParameter, ModuleGraphPort, ModuleGraphSignal,
    };
    use llg::core::elab::Value;

    fn ty(kind: &str, width: Option<u32>) -> TypeInfo {
        TypeInfo {
            kind: kind.to_owned(),
            width,
            signed: false,
            type_name: None,
        }
    }

    fn instance(name: &str, full_name: &str, def_name: &str) -> InstanceModel {
        InstanceModel {
            name: name.to_owned(),
            def_name: def_name.to_owned(),
            full_name: full_name.to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        }
    }

    fn source_instance(name: &str, module_type: &str, line: u32) -> ModuleGraphInstance {
        ModuleGraphInstance {
            name: name.to_owned(),
            module_type: module_type.to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line,
            col: 3,
        }
    }

    fn source_definition(
        name: &str,
        file: &str,
        line: u32,
        children: Vec<ModuleGraphInstance>,
    ) -> ModuleGraphDefinition {
        ModuleGraphDefinition {
            id: crate::features::module_graph_definition_id(name, Some(file), line, 1),
            name: name.to_owned(),
            file: Some(file.to_owned()),
            line,
            col: 1,
            end_line: line + 10,
            end_col: 1,
            ports: Vec::new(),
            params: Vec::new(),
            signals: Vec::new(),
            children,
            generated_scopes: Vec::new(),
        }
    }

    fn source_location(file: &str, line: u32, col: u32, width: u32) -> ModuleGraphLocation {
        ModuleGraphLocation {
            file: file.to_owned(),
            line,
            col,
            end_line: line,
            end_col: col + width,
        }
    }

    fn graph_analysis(
        definitions: Vec<ModuleGraphDefinition>,
        top_instances: Vec<InstanceModel>,
        configured_top: Option<&str>,
    ) -> Analysis {
        let model = DesignModel {
            design_name: "test".to_owned(),
            top_instances,
            modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let mut analysis = Analysis::new(Vec::new(), model, Vec::new(), Vec::new());
        analysis.module_graph = ModuleGraph {
            definitions,
            elaborated_types: Vec::new(),
        };
        analysis.configured_top = configured_top.map(str::to_owned);
        analysis
    }

    fn identity_source(path: &Path) -> Option<PathBuf> {
        Some(path.to_owned())
    }

    fn instance_stats(roots: &[ExplorerInstance]) -> (usize, usize, usize) {
        let mut instances = roots.iter().collect::<Vec<_>>();
        let mut scopes = Vec::new();
        let mut total = 0;
        let mut budget_markers = 0;
        let mut cycle_markers = 0;
        while let Some(instance) = instances.pop() {
            total += 1;
            budget_markers += usize::from(instance.is_budget_truncated);
            cycle_markers += usize::from(instance.is_cycle_truncated);
            instances.extend(instance.children.iter());
            scopes.extend(instance.generated_scopes.iter());
            while let Some(scope) = scopes.pop() {
                instances.extend(scope.children.iter());
                scopes.extend(scope.nested_scopes.iter());
            }
        }
        (total, budget_markers, cycle_markers)
    }

    fn serialized_hierarchy_stats(roots: &[ExplorerInstance]) -> (usize, usize, usize) {
        let mut instances = roots.iter().collect::<Vec<_>>();
        let mut scopes = Vec::new();
        let mut total = 0;
        let mut scope_count = 0;
        let mut budget_markers = 0;
        while let Some(instance) = instances.pop() {
            total += 1;
            budget_markers += usize::from(instance.is_budget_truncated);
            instances.extend(instance.children.iter());
            scopes.extend(instance.generated_scopes.iter());
        }
        while let Some(scope) = scopes.pop() {
            total += 1;
            scope_count += 1;
            instances.extend(scope.children.iter());
            scopes.extend(scope.nested_scopes.iter());
            while let Some(instance) = instances.pop() {
                total += 1;
                budget_markers += usize::from(instance.is_budget_truncated);
                instances.extend(instance.children.iter());
                scopes.extend(instance.generated_scopes.iter());
            }
        }
        (total, scope_count, budget_markers)
    }

    fn serialized_response_work(snapshot: &ExplorerSnapshot) -> usize {
        enum Work<'a> {
            Instance(&'a ExplorerInstance),
            Scope(&'a ExplorerGenerateScope),
        }

        let mut total = 0;
        for module in &snapshot.modules {
            total += 1 + module.ports.len() + module.params.len() + module.signals.len();
        }
        let mut work = snapshot
            .roots
            .iter()
            .map(Work::Instance)
            .collect::<Vec<_>>();
        while let Some(item) = work.pop() {
            match item {
                Work::Instance(instance) => {
                    total +=
                        1 + instance.ports.len() + instance.params.len() + instance.signals.len();
                    work.extend(instance.children.iter().map(Work::Instance));
                    work.extend(instance.generated_scopes.iter().map(Work::Scope));
                }
                Work::Scope(scope) => {
                    total += 1 + scope.params.len();
                    work.extend(scope.children.iter().map(Work::Instance));
                    work.extend(scope.nested_scopes.iter().map(Work::Scope));
                }
            }
        }
        total
    }

    #[test]
    fn snapshot_keeps_multiple_tops_and_recursive_stable_ids() {
        let mut top = instance("top", "work@top", "top");
        let mut child = instance("u_child", "work@top.u_child", "child");
        child.ports.push(PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(1)),
        });
        child.signals.push(SignalModel {
            name: "data".to_owned(),
            kind: "var".to_owned(),
            ty: ty("logic", Some(8)),
        });
        top.children.push(child);
        let second = instance("second", "work@second", "second");
        let model = DesignModel {
            design_name: "design".to_owned(),
            top_instances: vec![top, second],
            modules: vec![
                ModuleDef {
                    name: "top".to_owned(),
                    file: Some("/workspace/top.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 8,
                    end_col: 1,
                },
                ModuleDef {
                    name: "child".to_owned(),
                    file: Some("/workspace/child.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 3,
                    end_col: 1,
                },
                ModuleDef {
                    name: "second".to_owned(),
                    file: Some("/workspace/second.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 2,
                    end_col: 1,
                },
            ],
            packages: Vec::new(),
            classes: Vec::new(),
        };

        let first = snapshot("/workspace", &model);
        let second = snapshot("/workspace", &model);
        assert_eq!(first, second);
        let reordered = DesignModel {
            top_instances: model.top_instances.iter().cloned().rev().collect(),
            modules: model.modules.iter().cloned().rev().collect(),
            ..model.clone()
        };
        assert_eq!(first, snapshot("/workspace", &reordered));
        assert_eq!(first.roots.len(), 2);
        let top_root = first
            .roots
            .iter()
            .find(|root| root.module_type == "top")
            .expect("top root");
        assert_eq!(top_root.children.len(), 1);
        assert_eq!(top_root.children[0].module_type, "child");
        assert_eq!(top_root.children[0].ports[0].ty.width, Some(1));
        assert_eq!(top_root.children[0].signals[0].ty.width, Some(8));
        assert!(top_root.id.contains("/workspace:top"));
    }

    #[test]
    fn generated_scope_children_and_typed_parameters_are_preserved() {
        let mut top = instance("top", "top", "top");
        top.gen_scopes.push(GenScopeModel {
            name: "g[0]".to_owned(),
            full_name: "work@top.g[0]".to_owned(),
            params: vec![ParamModel {
                name: "i".to_owned(),
                value: Some(Val::Bits(Value::from_u64(3, 32, false))),
                ty: ty("int", Some(32)),
                local: true,
            }],
            children: vec![instance("u", "top.g[0].u", "leaf")],
        });
        let model = DesignModel {
            design_name: "design".to_owned(),
            top_instances: vec![top],
            modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let result = snapshot("root", &model);
        let generated = &result.roots[0].generated_scopes[0];
        assert_eq!(generated.name, "g[0]");
        assert_eq!(generated.params[0].value.as_deref(), Some("32'd3"));
        assert_eq!(generated.children[0].module_type, "leaf");
        assert_eq!(generated.children[0].id, "instance:root:top.g[0].u");
    }

    #[test]
    fn wire_shape_keeps_typed_fields_and_camel_case_names() {
        let mut top = instance("top", "top", "top");
        top.ports.push(PortModel {
            name: "clock".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(1)),
        });
        let model = DesignModel {
            design_name: "design".to_owned(),
            top_instances: vec![top],
            modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let value = serde_json::to_value(snapshot("root", &model)).expect("serialize snapshot");
        assert_eq!(value["roots"][0]["instanceName"], "top");
        assert_eq!(value["roots"][0]["moduleType"], "top");
        assert_eq!(value["roots"][0]["ports"][0]["direction"], "input");
        assert_eq!(value["roots"][0]["ports"][0]["type"]["kind"], "logic");
        assert_eq!(value["roots"][0]["ports"][0]["type"]["width"], 1);
    }

    #[test]
    fn syntax_fallback_modules_are_useful_without_instance_data() {
        let model = DesignModel {
            design_name: String::new(),
            top_instances: Vec::new(),
            modules: vec![ModuleDef {
                name: "broken".to_owned(),
                file: Some("/workspace/broken.sv".to_owned()),
                line: 2,
                col: 8,
                end_line: 2,
                end_col: 14,
            }],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let result = snapshot("root", &model);
        assert_eq!(result.roots.len(), 0);
        assert_eq!(result.modules[0].name, "broken");
        assert!(result.modules[0].ports.is_empty());
        assert!(result.modules[0]
            .uri
            .as_deref()
            .is_some_and(|uri| uri.starts_with("file:")));
    }

    #[test]
    fn source_graph_excludes_instantiated_definitions_from_roots() {
        let top = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_child", "child", 4)],
        );
        let child = source_definition(
            "child",
            "/workspace/child.sv",
            1,
            vec![source_instance("u_leaf", "leaf", 5)],
        );
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let unrelated = source_definition("unrelated", "/workspace/unrelated.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![top, child, leaf, unrelated], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let mut root_names = result
            .roots
            .iter()
            .map(|root| root.module_type.as_str())
            .collect::<Vec<_>>();
        root_names.sort_unstable();
        assert_eq!(root_names, vec!["top", "unrelated"]);

        let top_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "top")
            .expect("top root");
        let child_node = top_root
            .children
            .iter()
            .find(|child| child.instance_name == "u_child")
            .expect("source child");
        assert_eq!(child_node.module_type, "child");
        assert_eq!(child_node.content_source.as_deref(), Some("declaration"));
        assert_eq!(child_node.children[0].module_type, "leaf");
        assert_eq!(result.modules.len(), 4);
    }

    #[test]
    fn small_recursive_source_hierarchy_still_expands_normally() {
        let top = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_child", "child", 4)],
        );
        let child = source_definition(
            "child",
            "/workspace/child.sv",
            1,
            vec![source_instance("u_leaf", "leaf", 5)],
        );
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![top, child, leaf], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let top_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "top")
            .expect("top root");
        let child_node = &top_root.children[0];
        assert_eq!(child_node.instance_name, "u_child");
        assert_eq!(child_node.children[0].instance_name, "u_leaf");
        assert!(!top_root.is_budget_truncated);
        assert!(!child_node.is_budget_truncated);
        assert!(!child_node.is_cycle_truncated);
        assert_eq!(instance_stats(&result.roots), (3, 0, 0));
    }

    #[test]
    fn graph_budget_counts_generate_wrappers_across_two_roots() {
        let mut first = source_definition("first", "/workspace/first.sv", 1, Vec::new());
        let mut second = source_definition("second", "/workspace/second.sv", 1, Vec::new());
        for index in 0..(MAX_INSTANCE_NODES / 2) {
            first.generated_scopes.push(ModuleGraphGenerateScope {
                name: format!("first_gen_{index}"),
                file: first.file.clone(),
                line: index as u32 + 2,
                col: 1,
                children: Vec::new(),
                nested: Vec::new(),
            });
            second.generated_scopes.push(ModuleGraphGenerateScope {
                name: format!("second_gen_{index}"),
                file: second.file.clone(),
                line: index as u32 + 2,
                col: 1,
                children: Vec::new(),
                nested: Vec::new(),
            });
        }
        let analysis = graph_analysis(vec![first, second], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let (total, scopes, budget_markers) = serialized_hierarchy_stats(&result.roots);
        assert_eq!(result.roots.len(), 2);
        assert!(scopes < MAX_INSTANCE_NODES);
        assert!(total <= MAX_INSTANCE_NODES);
        assert_eq!(budget_markers, 2);
        assert_eq!(instance_stats(&result.roots).1, 2);
    }

    #[test]
    fn response_budget_is_shared_across_independent_analysis_roots() {
        let mut first = instance("first", "first", "first");
        first.children = (0..6_000)
            .map(|index| {
                instance(
                    &format!("u_first_{index}"),
                    &format!("first.u_first_{index}"),
                    "leaf",
                )
            })
            .collect();
        let mut second = instance("second", "second", "second");
        second.children = (0..6_000)
            .map(|index| {
                instance(
                    &format!("u_second_{index}"),
                    &format!("second.u_second_{index}"),
                    "leaf",
                )
            })
            .collect();

        let first_analysis = graph_analysis(Vec::new(), vec![first], None);
        let second_analysis = graph_analysis(Vec::new(), vec![second], None);
        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        let first_snapshot = snapshot_analysis_with_budget(
            "first-root",
            &first_analysis,
            identity_source,
            &mut budget,
        );
        budget.begin_workspace(1);
        let second_snapshot = snapshot_analysis_with_budget(
            "second-root",
            &second_analysis,
            identity_source,
            &mut budget,
        );
        let merged = merge([first_snapshot, second_snapshot]);

        let (total, budget_markers, _) = instance_stats(&merged.roots);
        assert!(total <= MAX_INSTANCE_NODES);
        assert_eq!(budget_markers, 1);
        assert!(merged.roots.iter().any(|root| root.module_type == "first"));
        assert!(merged.roots.iter().any(|root| root.module_type == "second"));
    }

    #[test]
    fn deep_graph_and_compatibility_chains_stop_before_the_stack_grows_unbounded() {
        const CHAIN_LENGTH: usize = 5_100;

        let definitions = (0..CHAIN_LENGTH)
            .map(|index| {
                let children = (index + 1 < CHAIN_LENGTH).then(|| {
                    vec![source_instance(
                        "u_next",
                        &format!("module_{}", index + 1),
                        index as u32 + 2,
                    )]
                });
                source_definition(
                    &format!("module_{index}"),
                    &format!("/workspace/module_{index}.sv"),
                    index as u32 + 1,
                    children.unwrap_or_default(),
                )
            })
            .collect();
        let graph = graph_analysis(definitions, Vec::new(), None);
        let graph_result = snapshot_analysis("root", &graph, identity_source);
        let (graph_total, graph_budget_markers, graph_cycle_markers) =
            instance_stats(&graph_result.roots);
        assert_eq!(graph_result.roots.len(), 1);
        assert!(graph_total <= MAX_SAFE_HIERARCHY_DEPTH + 1);
        assert_eq!(graph_budget_markers, 1);
        assert_eq!(graph_cycle_markers, 0);

        let mut chain = instance("tail", "chain.tail", "leaf");
        for index in (0..CHAIN_LENGTH).rev() {
            let name = format!("u_{index}");
            let mut parent = instance(&name, &format!("chain.{name}"), "leaf");
            parent.children.push(chain);
            chain = parent;
        }
        let model = DesignModel {
            design_name: "design".to_owned(),
            top_instances: vec![chain],
            modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let compatibility_result = snapshot("root", &model);
        let (compatibility_total, compatibility_budget_markers, compatibility_cycle_markers) =
            instance_stats(&compatibility_result.roots);
        assert!(compatibility_total <= MAX_SAFE_HIERARCHY_DEPTH + 1);
        assert_eq!(compatibility_budget_markers, 1);
        assert_eq!(compatibility_cycle_markers, 0);
    }

    #[test]
    fn configured_top_does_not_promote_nested_occurrence() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![source_instance("u_top", "top", 4)],
        );
        let top = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_leaf", "leaf", 4)],
        );
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let unrelated = source_definition("unrelated", "/workspace/unrelated.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![parent, top, leaf, unrelated], Vec::new(), Some("top"));

        let result = snapshot_analysis("root", &analysis, identity_source);
        assert!(!result.roots.iter().any(|root| root.module_type == "top"));
        assert!(result.roots.iter().any(|root| root.module_type == "parent"));
        assert!(result
            .roots
            .iter()
            .any(|root| root.module_type == "unrelated"));
        let parent_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "parent")
            .expect("parent root");
        assert_eq!(
            parent_root.children[0].module_type, "top",
            "configured top must remain nested under its source parent"
        );
        assert_eq!(result.roots.len(), 2);
    }

    #[test]
    fn configured_nested_top_uses_unique_elaborated_source_occurrence() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![source_instance("u_child", "child", 4)],
        );
        let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
        child.ports.push(ModuleGraphPort {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", None),
            detail: None,
            location: Some(source_location("/workspace/child.sv", 1, 24, 4)),
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });
        child.params.push(ModuleGraphParameter {
            name: "WIDTH".to_owned(),
            ty: ty("int", Some(32)),
            local: false,
            detail: None,
            location: Some(source_location("/workspace/child.sv", 2, 10, 5)),
            display_type: Some("int".to_owned()),
            display_shape: ModuleGraphTypeShape::default(),
        });
        child.signals.push(ModuleGraphSignal {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", None),
            detail: None,
            location: Some(source_location("/workspace/child.sv", 3, 22, 7)),
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });

        let mut configured = instance("child", "work@child", "child");
        configured.file = Some("/workspace/child.sv".to_owned());
        configured.ports.push(PortModel {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(8)),
        });
        configured.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(8, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        configured.signals.push(SignalModel {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(8)),
        });

        let mut analysis = graph_analysis(vec![parent, child], vec![configured], Some("child"));
        analysis.module_graph.elaborated_types.extend([
            ModuleGraphElaboratedType {
                instance: "child".to_owned(),
                name: "data".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
            },
            ModuleGraphElaboratedType {
                instance: "child".to_owned(),
                name: "payload".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
            },
        ]);

        let result = snapshot_analysis("root", &analysis, identity_source);
        assert_eq!(result.roots.len(), 1);
        assert_eq!(result.roots[0].module_type, "parent");
        let nested = result.roots[0]
            .children
            .iter()
            .find(|instance| instance.instance_name == "u_child")
            .expect("configured child source occurrence");
        assert_eq!(nested.content_source.as_deref(), Some("elaborated"));
        assert_eq!(nested.ports[0].ty.width, Some(8));
        assert_eq!(
            nested.ports[0].ty.display_type.as_deref(),
            Some("logic [7:0]")
        );
        assert_eq!(nested.params[0].value.as_deref(), Some("32'd8"));
        assert_eq!(nested.signals[0].ty.width, Some(8));
        assert_eq!(
            nested.signals[0].ty.display_type.as_deref(),
            Some("logic [7:0]")
        );
    }

    #[test]
    fn configured_top_exact_match_beats_earlier_unmatched_same_type() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![source_instance("u_child", "child", 4)],
        );
        let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
        child.params.push(ModuleGraphParameter {
            name: "WIDTH".to_owned(),
            ty: ty("int", Some(32)),
            local: false,
            detail: None,
            location: None,
            display_type: Some("int".to_owned()),
            display_shape: ModuleGraphTypeShape::default(),
        });
        child.signals.push(ModuleGraphSignal {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", None),
            detail: None,
            location: None,
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });

        // This retained same-type instance has no matching source location or
        // name.  It appears first specifically to catch order-dependent
        // type-only fallback consuming the only source occurrence.
        let mut unmatched = instance("retained_child", "work@retained_child", "child");
        unmatched.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(16, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        unmatched.signals.push(SignalModel {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(16)),
        });

        let mut exact = instance("u_child", "work@parent.u_child", "child");
        exact.line = 4;
        exact.col = 3;
        exact.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(8, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        exact.signals.push(SignalModel {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(8)),
        });

        let mut analysis =
            graph_analysis(vec![parent, child], vec![unmatched, exact], Some("child"));
        analysis
            .module_graph
            .elaborated_types
            .push(ModuleGraphElaboratedType {
                instance: "parent.u_child".to_owned(),
                name: "payload".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
            });

        let result = snapshot_analysis("root", &analysis, identity_source);
        let parent_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "parent")
            .expect("parent root");
        let nested = parent_root
            .children
            .iter()
            .find(|child| child.instance_name == "u_child")
            .expect("exact source occurrence");

        assert_eq!(nested.content_source.as_deref(), Some("elaborated"));
        assert_eq!(nested.params[0].value.as_deref(), Some("32'd8"));
        assert_eq!(nested.signals[0].ty.width, Some(8));
        assert_eq!(
            nested.signals[0].ty.display_type.as_deref(),
            Some("logic [7:0]")
        );
    }

    #[test]
    fn configured_top_does_not_guess_between_same_type_source_occurrences() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![
                source_instance("u_first", "child", 4),
                source_instance("u_second", "child", 8),
            ],
        );
        let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
        child.ports.push(ModuleGraphPort {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", None),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        });

        let mut exact = instance("u_first", "work@parent.u_first", "child");
        exact.file = Some("/workspace/top.sv".to_owned());
        exact.line = 4;
        exact.col = 3;
        exact.ports.push(PortModel {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(8)),
        });

        let mut unmatched = instance("retained_child", "work@retained_child", "child");
        unmatched.file = Some("/workspace/child.sv".to_owned());
        unmatched.ports.push(PortModel {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(16)),
        });

        let analysis = graph_analysis(vec![parent, child], vec![exact, unmatched], Some("child"));
        let result = snapshot_analysis("root", &analysis, identity_source);
        let parent_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "parent")
            .expect("parent root");
        let first = parent_root
            .children
            .iter()
            .find(|child| child.instance_name == "u_first")
            .expect("exact source occurrence");
        assert_eq!(first.content_source.as_deref(), Some("elaborated"));
        assert_eq!(first.ports[0].ty.width, Some(8));

        let second = parent_root
            .children
            .iter()
            .find(|child| child.instance_name == "u_second")
            .expect("unmatched source occurrence");
        assert_eq!(second.content_source.as_deref(), Some("declaration"));
        assert_ne!(
            second.ports[0].ty.width,
            Some(16),
            "an unmatched retained instance must not be assigned arbitrarily"
        );
    }

    #[test]
    fn response_budget_preserves_a_hierarchy_root_before_large_module_content() {
        let mut definition = source_definition("large", "/workspace/large.sv", 1, Vec::new());
        definition.signals = (0..(MAX_INSTANCE_NODES * 2))
            .map(|index| ModuleGraphSignal {
                name: format!("signal_{index}"),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(1)),
                detail: None,
                location: None,
                display_type: None,
                display_shape: ModuleGraphTypeShape::default(),
            })
            .collect();
        let analysis = graph_analysis(vec![definition], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let root = result
            .roots
            .iter()
            .find(|root| root.module_type == "large")
            .expect("large module hierarchy root");
        assert!(root.definition_id.is_some());
        assert!(!root.is_ambiguous);
        assert!(root.is_budget_truncated);
        assert!(root.signals.len() < MAX_INSTANCE_NODES);
        assert!(
            result.modules.iter().any(|module| module.name == "large"),
            "hierarchy expansion must leave a usable module catalog prefix"
        );
        assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
    }

    #[test]
    fn response_budget_keeps_a_huge_root_catalog_bounded_and_visible() {
        let definitions = (0..(MAX_INSTANCE_NODES * 2))
            .map(|index| {
                source_definition(
                    &format!("module_{index}"),
                    &format!("/workspace/module_{index}.sv"),
                    index as u32 + 1,
                    Vec::new(),
                )
            })
            .collect();
        let analysis = graph_analysis(definitions, Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        assert!(result
            .modules
            .iter()
            .any(|module| module.is_budget_truncated));
        let root = result
            .roots
            .first()
            .expect("one source root survives the catalog budget");
        assert!(root.definition_id.is_some());
        assert!(result.modules.len() <= MAX_INSTANCE_NODES);
        assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
    }

    #[test]
    fn response_budget_keeps_later_workspace_roots_after_catalog_truncation() {
        let mut first_definition = source_definition("first", "/workspace/first.sv", 1, Vec::new());
        first_definition.signals = (0..(MAX_INSTANCE_NODES * 2))
            .map(|index| ModuleGraphSignal {
                name: format!("signal_{index}"),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(1)),
                detail: None,
                location: None,
                display_type: None,
                display_shape: ModuleGraphTypeShape::default(),
            })
            .collect();
        let first_analysis = graph_analysis(vec![first_definition], Vec::new(), None);
        let second_analysis = graph_analysis(
            vec![
                source_definition(
                    "second",
                    "/workspace/second.sv",
                    1,
                    vec![source_instance("u_leaf", "second_leaf", 2)],
                ),
                source_definition("second_leaf", "/workspace/second_leaf.sv", 1, Vec::new()),
            ],
            Vec::new(),
            None,
        );

        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        let first = snapshot_analysis_with_budget(
            "first-root",
            &first_analysis,
            identity_source,
            &mut budget,
        );
        budget.begin_workspace(1);
        let second = snapshot_analysis_with_budget(
            "second-root",
            &second_analysis,
            identity_source,
            &mut budget,
        );
        let merged = merge([first, second]);

        assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
        let first_root = merged
            .roots
            .iter()
            .find(|root| root.module_type == "first")
            .expect("first workspace root");
        assert!(first_root.is_budget_truncated);
        assert!(merged
            .roots
            .iter()
            .any(|root| root.module_type == "first" && root.definition_id.is_some()));
        let second_root = merged
            .roots
            .iter()
            .find(|root| root.module_type == "second")
            .expect("later workspace root");
        assert!(second_root.definition_id.is_some());
        assert!(second_root.children.is_empty());
        assert!(
            second_root.is_budget_truncated,
            "an omitted child must be represented as hierarchy truncation"
        );
    }

    #[test]
    fn repeated_cycle_terminals_preserve_a_later_workspace_root_slot() {
        // Arrange
        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        assert!(budget.take_root(), "first workspace root");

        // Act: model an early workspace containing enough cycle leaves to
        // consume every non-root slot.
        while budget.take_terminal() {}

        // Assert: stopping cycle expansion must leave the reserved capacity
        // available to a later workspace hierarchy root.
        budget.begin_workspace(1);
        assert!(budget.take_root(), "later workspace root reservation");
    }

    #[test]
    fn exhausted_cycle_leaves_stay_accounted_and_preserve_later_workspace_root() {
        let cycle_children = (0..(MAX_INSTANCE_NODES * 2))
            .map(|index| source_instance(&format!("u_cycle_{index}"), "cycle", index as u32 + 2))
            .collect();
        let first_analysis = graph_analysis(
            vec![source_definition(
                "cycle",
                "/workspace/cycle.sv",
                1,
                cycle_children,
            )],
            Vec::new(),
            None,
        );
        let second_analysis = graph_analysis(
            vec![source_definition(
                "second",
                "/workspace/second.sv",
                1,
                Vec::new(),
            )],
            Vec::new(),
            None,
        );
        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        let first = snapshot_analysis_with_budget(
            "first-root",
            &first_analysis,
            identity_source,
            &mut budget,
        );
        budget.begin_workspace(1);
        let second = snapshot_analysis_with_budget(
            "second-root",
            &second_analysis,
            identity_source,
            &mut budget,
        );
        let merged = merge([first, second]);

        assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
        assert!(merged.roots.iter().any(|root| root.is_cycle_root));
        assert!(merged.roots.iter().any(|root| root.module_type == "second"));
    }

    #[test]
    fn omitted_generate_scope_marks_parent_at_regular_budget_boundary() {
        let mut definition = source_definition("top", "/workspace/top.sv", 1, Vec::new());
        let ordinary_content_slots =
            MAX_INSTANCE_NODES - GRAPH_TERMINAL_SLOTS - GUARANTEED_HIERARCHY_ROOT_SLOTS - 2 - 1;
        definition.signals = (0..ordinary_content_slots)
            .map(|index| ModuleGraphSignal {
                name: format!("signal_{index}"),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(1)),
                detail: None,
                location: None,
                display_type: None,
                display_shape: ModuleGraphTypeShape::default(),
            })
            .collect();
        definition.generated_scopes.push(ModuleGraphGenerateScope {
            name: "late_scope".to_owned(),
            file: definition.file.clone(),
            line: ordinary_content_slots as u32 + 2,
            col: 1,
            children: Vec::new(),
            nested: Vec::new(),
        });
        let analysis = graph_analysis(vec![definition], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);

        let root = result.roots.first().expect("top hierarchy root");
        assert!(root.generated_scopes.is_empty());
        assert!(
            root.is_budget_truncated,
            "omitting a generate scope must mark its containing instance"
        );
        assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
    }

    #[test]
    fn depth_limited_scope_merge_is_explicit_without_a_marker_slot() {
        let analysis = graph_analysis(
            vec![source_definition("top", "/workspace/top.sv", 1, Vec::new())],
            Vec::new(),
            None,
        );
        let catalog = GraphCatalog::new("root", &analysis.module_graph, &identity_source);
        let represented = HashSet::new();
        let lookup = SourceElaborationLookup::new(&catalog, &[], &represented);
        let source = ModuleGraphGenerateScope {
            name: "nested".to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 2,
            col: 1,
            children: Vec::new(),
            nested: Vec::new(),
        };
        let mut elaborated = ExplorerGenerateScope {
            id: "scope:root:top.nested".to_owned(),
            name: "nested".to_owned(),
            is_budget_truncated: false,
            params: Vec::new(),
            children: Vec::new(),
            nested_scopes: Vec::new(),
        };
        let mut budget = InstanceBudget::new(GRAPH_TERMINAL_SLOTS);
        budget.stopped = true;
        budget.budget_marker_emitted = true;

        merge_source_scope(
            "root",
            &catalog,
            &identity_source,
            "top",
            0,
            &mut elaborated,
            &source,
            &lookup,
            &mut Vec::new(),
            &mut budget,
            MAX_SAFE_HIERARCHY_DEPTH,
        );

        assert!(elaborated.is_budget_truncated);
        assert!(elaborated.children.is_empty());
    }

    #[test]
    fn oversized_first_workspace_cannot_consume_later_workspace_root_quota() {
        let first_definitions = (0..(MAX_INSTANCE_NODES * 2))
            .map(|index| {
                source_definition(
                    &format!("first_{index}"),
                    &format!("/workspace/first_{index}.sv"),
                    index as u32 + 1,
                    Vec::new(),
                )
            })
            .collect();
        let first_analysis = graph_analysis(first_definitions, Vec::new(), None);
        let second_analysis = graph_analysis(
            vec![source_definition(
                "second",
                "/workspace/second.sv",
                1,
                Vec::new(),
            )],
            Vec::new(),
            None,
        );
        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        let first = snapshot_analysis_with_budget(
            "first-root",
            &first_analysis,
            identity_source,
            &mut budget,
        );
        budget.begin_workspace(1);
        let second = snapshot_analysis_with_budget(
            "second-root",
            &second_analysis,
            identity_source,
            &mut budget,
        );
        let merged = merge([first, second]);

        assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
        assert!(merged
            .roots
            .iter()
            .any(|root| root.module_type == "second" && root.definition_id.is_some()));
        assert!(merged
            .roots
            .iter()
            .any(|root| root.module_type.starts_with("first_") && root.is_budget_truncated));
    }

    #[test]
    fn response_budget_keeps_catalog_and_hierarchy_bounded_across_analysis_roots() {
        let make_analysis = |prefix: &str| {
            let definitions = (0..6_000)
                .map(|index| {
                    source_definition(
                        &format!("{prefix}_{index}"),
                        &format!("/workspace/{prefix}_{index}.sv"),
                        index as u32 + 1,
                        Vec::new(),
                    )
                })
                .collect();
            graph_analysis(definitions, Vec::new(), None)
        };
        let first_analysis = make_analysis("first");
        let second_analysis = make_analysis("second");
        let mut budget = new_response_budget();
        budget.prepare_workspaces();
        budget.begin_workspace(2);
        let first = snapshot_analysis_with_budget(
            "first-root",
            &first_analysis,
            identity_source,
            &mut budget,
        );
        budget.begin_workspace(1);
        let second = snapshot_analysis_with_budget(
            "second-root",
            &second_analysis,
            identity_source,
            &mut budget,
        );
        let merged = merge([first, second]);

        assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
        assert!(merged.modules.len() < 12_000);
        assert!(merged.modules.len() <= MAX_INSTANCE_NODES);
        assert!(merged
            .modules
            .iter()
            .any(|module| module.is_budget_truncated));
        assert!(merged
            .roots
            .iter()
            .any(|root| root.module_type.starts_with("first_") && root.definition_id.is_some()));
        assert!(merged
            .roots
            .iter()
            .any(|root| root.module_type.starts_with("second_") && root.definition_id.is_some()));
    }

    #[test]
    fn rootless_source_cycle_has_one_bounded_cycle_root() {
        let first = source_definition(
            "first",
            "/workspace/first.sv",
            1,
            vec![source_instance("u_second", "second", 2)],
        );
        let second = source_definition(
            "second",
            "/workspace/second.sv",
            1,
            vec![source_instance("u_first", "first", 2)],
        );
        let analysis = graph_analysis(
            vec![first, second],
            vec![instance("first", "work@first", "first")],
            Some("first"),
        );

        let result = snapshot_analysis("root", &analysis, identity_source);
        assert_eq!(result.roots.len(), 1);
        let root = &result.roots[0];
        assert!(root.is_cycle_root);
        assert_eq!(root.module_type, "first");
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].module_type, "second");
        let cycle_edge = root.children[0]
            .children
            .iter()
            .find(|child| child.module_type == "first")
            .expect("repeated first definition");
        assert!(cycle_edge.is_cycle_truncated);
        assert!(!cycle_edge.is_budget_truncated);
    }

    #[test]
    fn self_cycle_is_visible_beside_an_ordinary_zero_incoming_root() {
        let ordinary = source_definition("ordinary", "/workspace/ordinary.sv", 1, Vec::new());
        let self_cycle = source_definition(
            "self_cycle",
            "/workspace/self_cycle.sv",
            1,
            vec![source_instance("self", "self_cycle", 2)],
        );
        let analysis = graph_analysis(vec![ordinary, self_cycle], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        assert_eq!(
            result
                .roots
                .iter()
                .map(|root| root.module_type.as_str())
                .collect::<Vec<_>>(),
            ["ordinary", "self_cycle"]
        );
        let ordinary_root = &result.roots[0];
        assert!(!ordinary_root.is_cycle_root);
        let cycle_root = &result.roots[1];
        assert!(cycle_root.is_cycle_root);
        assert_eq!(cycle_root.children.len(), 1);
        assert!(cycle_root.children[0].is_cycle_truncated);
    }

    #[test]
    fn declaration_fallback_emits_typed_contents_and_filters_port_backing_signals() {
        let mut definition = source_definition("decl_top", "/workspace/decl.sv", 2, Vec::new());
        definition.ports.push(ModuleGraphPort {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(1)),
            detail: Some("input logic clk".to_owned()),
            location: Some(source_location("/workspace/decl.sv", 2, 25, 3)),
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        });
        definition.params.push(ModuleGraphParameter {
            name: "WIDTH".to_owned(),
            ty: ty("int", Some(32)),
            local: false,
            detail: Some("parameter int WIDTH = 8".to_owned()),
            location: Some(source_location("/workspace/decl.sv", 1, 24, 5)),
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        });
        definition.signals.extend([
            ModuleGraphSignal {
                name: "clk".to_owned(),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(1)),
                detail: None,
                location: None,
                display_type: None,
                display_shape: ModuleGraphTypeShape::default(),
            },
            ModuleGraphSignal {
                name: "payload".to_owned(),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(8)),
                detail: Some("wire logic [7:0] payload".to_owned()),
                location: Some(source_location("/workspace/decl.sv", 3, 23, 7)),
                display_type: Some("logic [7:0]".to_owned()),
                display_shape: ModuleGraphTypeShape::default(),
            },
            ModuleGraphSignal {
                name: "tri_bus".to_owned(),
                kind: "tri".to_owned(),
                ty: ty("logic", Some(4)),
                detail: Some("tri [3:0] tri_bus".to_owned()),
                location: Some(source_location("/workspace/decl.sv", 4, 14, 7)),
                display_type: None,
                display_shape: ModuleGraphTypeShape::default(),
            },
        ]);
        let analysis = graph_analysis(vec![definition], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let root = &result.roots[0];
        assert_eq!(root.content_source.as_deref(), Some("declaration"));
        assert_eq!(root.ports[0].detail.as_deref(), Some("input logic clk"));
        assert_eq!(
            root.ports[0]
                .location
                .as_ref()
                .map(|location| location.uri.as_str()),
            Some("file:///workspace/decl.sv")
        );
        assert_eq!(
            root.ports[0]
                .location
                .as_ref()
                .map(|location| (location.range.start_line, location.range.start_character)),
            Some((1, 24))
        );
        assert_eq!(root.params[0].value, None);
        assert_eq!(
            root.params[0].detail.as_deref(),
            Some("parameter int WIDTH = 8")
        );
        assert_eq!(
            root.params[0]
                .location
                .as_ref()
                .map(|location| location.range.start_line),
            Some(0)
        );
        assert_eq!(
            root.signals
                .iter()
                .map(|signal| signal.name.as_str())
                .collect::<Vec<_>>(),
            ["payload", "tri_bus"]
        );
        assert_eq!(root.signals[0].kind, "wire");
        assert_eq!(root.signals[1].kind, "tri");
        assert_eq!(
            root.signals[0]
                .location
                .as_ref()
                .map(|location| location.range.start_line),
            Some(2)
        );
        assert_eq!(
            result.modules[0].content_source.as_deref(),
            Some("declaration")
        );
    }

    #[test]
    fn elaborated_contents_keep_exact_values_types_and_port_filtering() {
        let mut top = instance("top", "work@top", "top");
        top.file = Some("/workspace/top.sv".to_owned());
        let mut child = instance("u_child", "work@top.u_child", "child");
        child.file = Some("/workspace/child.sv".to_owned());
        child.line = 7;
        child.col = 5;
        child.ports.push(PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(1)),
        });
        child.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(8, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        child.signals.extend([
            SignalModel {
                name: "clk".to_owned(),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(1)),
            },
            SignalModel {
                name: "payload".to_owned(),
                kind: "wire".to_owned(),
                ty: ty("logic", Some(8)),
            },
        ]);
        top.children.push(child);
        let analysis = graph_analysis(
            vec![
                source_definition(
                    "top",
                    "/workspace/top.sv",
                    1,
                    vec![source_instance("u_child", "child", 7)],
                ),
                source_definition("child", "/workspace/child.sv", 1, Vec::new()),
            ],
            vec![top],
            None,
        );

        let result = snapshot_analysis("root", &analysis, identity_source);
        let top_root = &result.roots[0];
        let child_node = &top_root.children[0];
        assert_eq!(top_root.content_source.as_deref(), Some("elaborated"));
        assert_eq!(child_node.content_source.as_deref(), Some("elaborated"));
        assert_eq!(child_node.params[0].value.as_deref(), Some("32'd8"));
        assert_eq!(child_node.params[0].ty.width, Some(32));
        assert_eq!(child_node.signals.len(), 1);
        assert_eq!(child_node.signals[0].name, "payload");
        assert_eq!(child_node.signals[0].kind, "wire");
        assert_eq!(child_node.signals[0].ty.width, Some(8));
        let child_module = result
            .modules
            .iter()
            .find(|module| module.name == "child")
            .expect("child module entry");
        assert_eq!(child_module.content_source.as_deref(), Some("elaborated"));
        assert_eq!(child_module.params[0].value.as_deref(), Some("32'd8"));
    }

    #[test]
    fn elaborated_types_resolve_parameters_for_nested_instances() {
        let mut top = instance("top", "work@top", "top");
        top.file = Some("/workspace/top.sv".to_owned());

        let mut child = instance("u_child", "work@top.u_child", "child");
        child.file = Some("/workspace/child.sv".to_owned());
        child.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(8, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        child.ports.push(PortModel {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(8)),
        });
        child.signals.push(SignalModel {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(8)),
        });

        let mut leaf = instance("u_leaf", "work@top.u_child.u_leaf", "leaf");
        leaf.file = Some("/workspace/leaf.sv".to_owned());
        leaf.params.push(ParamModel {
            name: "WIDTH".to_owned(),
            value: Some(Val::Bits(Value::from_u64(3, 32, false))),
            ty: ty("int", Some(32)),
            local: false,
        });
        leaf.signals.push(SignalModel {
            name: "leaf_payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(3)),
        });
        child.children.push(leaf);
        top.children.push(child);

        let source_top = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_child", "child", 3)],
        );
        let mut source_child = source_definition(
            "child",
            "/workspace/child.sv",
            1,
            vec![source_instance("u_leaf", "leaf", 3)],
        );
        source_child.ports.push(ModuleGraphPort {
            name: "data".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", None),
            detail: None,
            location: Some(source_location("/workspace/child.sv", 1, 45, 4)),
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });
        source_child.params.push(ModuleGraphParameter {
            name: "WIDTH".to_owned(),
            ty: ty("int", Some(32)),
            local: false,
            detail: None,
            location: Some(source_location("/workspace/child.sv", 1, 29, 5)),
            display_type: Some("int".to_owned()),
            display_shape: ModuleGraphTypeShape::default(),
        });
        source_child.signals.push(ModuleGraphSignal {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", None),
            detail: None,
            location: Some(source_location("/workspace/child.sv", 2, 22, 7)),
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });
        let mut source_leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        source_leaf.signals.push(ModuleGraphSignal {
            name: "leaf_payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", None),
            detail: None,
            location: Some(source_location("/workspace/leaf.sv", 2, 22, 12)),
            display_type: Some("logic [WIDTH-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });
        let mut analysis =
            graph_analysis(vec![source_top, source_child, source_leaf], vec![top], None);
        analysis.module_graph.elaborated_types = vec![
            ModuleGraphElaboratedType {
                instance: "top.u_child".to_owned(),
                name: "data".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
            },
            ModuleGraphElaboratedType {
                instance: "top.u_child".to_owned(),
                name: "payload".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
            },
            ModuleGraphElaboratedType {
                instance: "top.u_child.u_leaf".to_owned(),
                name: "leaf_payload".to_owned(),
                packed_ranges: vec![Some(ModuleGraphPackedRange { left: 2, right: 0 })],
            },
        ];

        let result = snapshot_analysis("root", &analysis, identity_source);
        let child_node = &result.roots[0].children[0];
        assert_eq!(
            child_node.signals[0].ty.display_type.as_deref(),
            Some("logic [7:0]")
        );
        assert_eq!(
            child_node.ports[0].ty.display_type.as_deref(),
            Some("logic [7:0]")
        );
        assert_eq!(
            child_node.params[0]
                .location
                .as_ref()
                .map(|location| location.uri.as_str()),
            Some("file:///workspace/child.sv")
        );
        let leaf_node = &child_node.children[0];
        assert_eq!(
            leaf_node.signals[0].ty.display_type.as_deref(),
            Some("logic [2:0]")
        );
    }

    #[test]
    fn unresolved_symbolic_width_is_retained_without_guessing() {
        let mut definition = source_definition("symbolic", "/workspace/symbolic.sv", 1, Vec::new());
        definition.signals.push(ModuleGraphSignal {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", None),
            detail: None,
            location: None,
            display_type: Some("logic [UNKNOWN-1:0]".to_owned()),
            display_shape: ModuleGraphTypeShape {
                packed_dimensions: 1,
                unpacked_dimensions: 0,
            },
        });
        let analysis = graph_analysis(vec![definition], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let payload = &result.roots[0].signals[0];
        assert_eq!(payload.ty.width, None);
        assert_eq!(
            payload.ty.display_type.as_deref(),
            Some("logic [UNKNOWN-1:0]")
        );
        let json = serde_json::to_value(&result).expect("serialize symbolic snapshot");
        assert_eq!(
            json["roots"][0]["signals"][0]["type"]["displayType"],
            "logic [UNKNOWN-1:0]"
        );
    }

    #[test]
    fn concrete_display_retains_symbolic_ranges_without_captured_bounds() {
        let ty = ty("logic", Some(8));
        let shape = ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 1,
        };
        let display = resolved_type_display(
            &ty,
            Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]"),
            Some(shape),
            None,
        );
        assert_eq!(
            display.as_deref(),
            Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]")
        );
        let missing = [None];
        assert_eq!(
            resolved_type_display(
                &ty,
                Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]"),
                Some(shape),
                Some(&missing),
            )
            .as_deref(),
            Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]")
        );
    }

    #[test]
    fn symbolic_dimension_normalization_preserves_tokens_and_compacts_ranges() {
        assert_eq!(normalize_symbolic_expression(" P    +    +1 "), "P+ +1");
        assert_eq!(normalize_symbolic_expression(" P    -    -1 "), "P- -1");
        assert_eq!(normalize_symbolic_expression(r" \WIDTH + 1 "), r"\WIDTH +1");
        assert_eq!(
            normalize_symbolic_expression(" P    inside    { BASE , IDX } : 0 "),
            "P inside {BASE,IDX}:0"
        );
        assert_eq!(normalize_type_display("logic [ 1 : 0 ]"), "logic [1:0]");
        assert_eq!(
            normalize_type_display(r"logic [ \WIDTH + 1 : 0 ]"),
            r"logic [\WIDTH +1:0]"
        );
        assert_eq!(
            normalize_type_display("logic [ P inside { BASE , IDX } : 0 ]"),
            "logic [P inside {BASE,IDX}:0]"
        );
        assert_eq!(
            normalize_type_display("logic [ MODE == \"A  ] B\" : 0 ]"),
            "logic [MODE==\"A  ] B\":0]"
        );
    }

    #[test]
    fn symbolic_dimension_comments_preserve_active_tokens_and_ranges() {
        let line_source = "logic [P // ignored ]\n + 1:0] payload;";
        let line_spans = bracket_spans(line_source);
        assert_eq!(line_spans.len(), 1);
        assert_eq!(line_spans[0].2, "P // ignored ]\n + 1:0");
        assert!(line_spans[0].2.contains("+ 1:0"));
        assert_eq!(normalize_type_display(line_source), line_source);

        let block_source = "logic [P /* ignored ] */ + 1:0] payload;";
        let block_spans = bracket_spans(block_source);
        assert_eq!(block_spans.len(), 1);
        assert_eq!(block_spans[0].2, "P /* ignored ] */ + 1:0");
        assert!(block_spans[0].2.contains("+ 1:0"));
        assert_eq!(normalize_type_display(block_source), block_source);

        let comments_between_operators = "P /* left */ + /* right */ + 1";
        assert_eq!(
            normalize_symbolic_expression(comments_between_operators),
            comments_between_operators
        );
        assert!(normalize_symbolic_expression(comments_between_operators).contains("+ 1"));
        assert_eq!(normalize_type_display("logic [ 1 : 0 ]"), "logic [1:0]");
    }

    #[test]
    fn concrete_display_keeps_each_packed_dimension_separate() {
        let ty = ty("logic", Some(32));
        let shape = ModuleGraphTypeShape {
            packed_dimensions: 2,
            unpacked_dimensions: 0,
        };
        let ranges = [
            Some(ModuleGraphPackedRange { left: 3, right: 0 }),
            Some(ModuleGraphPackedRange { left: 7, right: 4 }),
        ];
        let display = resolved_type_display(
            &ty,
            Some("logic [ROWS-1:0][COLS+3:4]"),
            Some(shape),
            Some(&ranges),
        );
        assert_eq!(display.as_deref(), Some("logic [3:0] [7:4]"));
    }

    #[test]
    fn scalar_source_display_preserves_typedef_and_net_qualifier() {
        let logic = ty("logic", Some(8));
        assert_eq!(
            resolved_type_display(&logic, Some("word_t"), None, None).as_deref(),
            Some("word_t")
        );
        assert_eq!(
            resolved_type_display(&logic, Some("wire"), None, None).as_deref(),
            Some("wire")
        );
    }

    #[test]
    fn elaborated_and_source_generate_scopes_are_deduplicated_with_nested_wrappers() {
        let mut top = instance("top", "work@top", "top");
        top.gen_scopes.push(GenScopeModel {
            name: "g[0]".to_owned(),
            full_name: "work@top.g[0]".to_owned(),
            params: Vec::new(),
            children: vec![instance("u_leaf", "top.g[0].u_leaf", "leaf")],
        });

        let mut source_top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
        source_top.generated_scopes.push(ModuleGraphGenerateScope {
            name: "g".to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 4,
            col: 3,
            children: vec![source_instance("u_leaf", "leaf", 5)],
            nested: vec![ModuleGraphGenerateScope {
                name: "inner".to_owned(),
                file: Some("/workspace/top.sv".to_owned()),
                line: 6,
                col: 5,
                children: vec![source_instance("u_nested", "leaf", 7)],
                nested: Vec::new(),
            }],
        });
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![source_top, leaf], vec![top], None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let generated = &result.roots[0].generated_scopes;
        assert_eq!(
            generated.len(),
            1,
            "source fallback duplicated an elaborated scope"
        );
        assert_eq!(generated[0].children.len(), 1);
        assert_eq!(generated[0].children[0].instance_name, "u_leaf");
        assert_eq!(generated[0].nested_scopes.len(), 1);
        assert_eq!(generated[0].nested_scopes[0].name, "inner");
        assert_eq!(
            generated[0].nested_scopes[0].children[0].instance_name,
            "u_nested"
        );
    }

    #[test]
    fn duplicate_definition_name_is_an_explicit_unexpanded_ambiguous_leaf() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![source_instance("u_dup", "dup", 4)],
        );
        let first = source_definition("dup", "/workspace/one.sv", 1, Vec::new());
        let second = source_definition("dup", "/workspace/two.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![parent, first, second], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let parent_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "parent")
            .expect("parent root");
        let ambiguous = &parent_root.children[0];
        assert_eq!(ambiguous.module_type, "dup");
        assert!(ambiguous.is_ambiguous);
        assert_eq!(ambiguous.definition_id, None);
        assert!(ambiguous.children.is_empty());
        assert_eq!(
            result
                .modules
                .iter()
                .filter(|module| module.name == "dup")
                .count(),
            2
        );
    }

    #[test]
    fn cycles_and_named_generate_boundaries_are_safe() {
        let mut top = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_a", "a", 3)],
        );
        top.generated_scopes.push(ModuleGraphGenerateScope {
            name: "gen_block".to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 5,
            col: 3,
            children: vec![source_instance("u_leaf", "leaf", 6)],
            nested: vec![ModuleGraphGenerateScope {
                name: "inner_block".to_owned(),
                file: Some("/workspace/top.sv".to_owned()),
                line: 7,
                col: 5,
                children: vec![source_instance("u_nested", "leaf", 8)],
                nested: Vec::new(),
            }],
        });
        let a = source_definition(
            "a",
            "/workspace/a.sv",
            1,
            vec![source_instance("u_b", "b", 3)],
        );
        let b = source_definition(
            "b",
            "/workspace/b.sv",
            1,
            vec![source_instance("u_a_again", "a", 3)],
        );
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![top, a, b, leaf], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let top_root = &result.roots[0];
        assert_eq!(top_root.generated_scopes[0].name, "gen_block");
        assert_eq!(top_root.generated_scopes[0].children[0].module_type, "leaf");
        assert_eq!(
            top_root.generated_scopes[0].nested_scopes[0].name,
            "inner_block"
        );
        assert_eq!(
            top_root.generated_scopes[0].nested_scopes[0].children[0].instance_name,
            "u_nested"
        );
        let cycle_leaf = &top_root.children[0].children[0].children[0];
        assert_eq!(cycle_leaf.module_type, "a");
        assert!(
            cycle_leaf.is_cycle_truncated,
            "cycle must terminate as a cycle-marked leaf"
        );
        assert!(!cycle_leaf.is_budget_truncated);
        assert!(cycle_leaf.children.is_empty());
    }

    #[test]
    fn expansion_budget_bounds_flat_source_fanout_to_one_marker() {
        let mut top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
        for index in 0..(MAX_INSTANCE_NODES * 2) {
            top.children.push(source_instance(
                &format!("u_{index}"),
                "leaf",
                index as u32 + 2,
            ));
        }
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![top, leaf], Vec::new(), None);

        let result = snapshot_analysis("root", &analysis, identity_source);
        let top_root = &result.roots[0];
        let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
        assert!(total <= MAX_INSTANCE_NODES);
        assert_eq!(budget_markers, 1);
        assert_eq!(cycle_markers, 0);
        assert!(top_root
            .children
            .iter()
            .any(|child| child.is_budget_truncated));
    }

    #[test]
    fn compatibility_snapshot_bounds_flat_fanout_to_one_marker() {
        let mut top = instance("top", "top", "top");
        for index in 0..(MAX_INSTANCE_NODES * 2) {
            top.children.push(instance(
                &format!("u_{index}"),
                &format!("top.u_{index}"),
                "leaf",
            ));
        }
        let model = DesignModel {
            design_name: "design".to_owned(),
            top_instances: vec![top],
            modules: vec![
                ModuleDef {
                    name: "top".to_owned(),
                    file: Some("/workspace/top.sv".to_owned()),
                    line: 1,
                    col: 1,
                    end_line: 2,
                    end_col: 1,
                },
                ModuleDef {
                    name: "leaf".to_owned(),
                    file: Some("/workspace/leaf.sv".to_owned()),
                    line: 1,
                    col: 1,
                    end_line: 2,
                    end_col: 1,
                },
            ],
            packages: Vec::new(),
            classes: Vec::new(),
        };

        let result = snapshot("root", &model);
        let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
        assert!(total <= MAX_INSTANCE_NODES);
        assert_eq!(budget_markers, 1);
        assert_eq!(cycle_markers, 0);
    }

    #[test]
    fn cycle_at_budget_boundary_keeps_cycle_flag_and_one_remainder_marker() {
        let parent = source_definition(
            "parent",
            "/workspace/parent.sv",
            1,
            vec![source_instance("u_top", "top", 2)],
        );
        let mut top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
        // Hierarchy roots are serialized before module records. Leave enough
        // ordinary slots to place the cycle at the boundary, then include two
        // trailing siblings so the third becomes the one explicit remainder
        // marker.
        for index in 0..(MAX_INSTANCE_NODES - 7) {
            top.children.push(source_instance(
                &format!("u_leaf_{index}"),
                "leaf",
                index as u32 + 2,
            ));
        }
        top.children.push(source_instance(
            "u_cycle",
            "top",
            MAX_INSTANCE_NODES as u32 + 2,
        ));
        top.children.push(source_instance(
            "u_after",
            "leaf",
            MAX_INSTANCE_NODES as u32 + 3,
        ));
        top.children.push(source_instance(
            "u_after_second",
            "leaf",
            MAX_INSTANCE_NODES as u32 + 4,
        ));
        top.children.push(source_instance(
            "u_after_third",
            "leaf",
            MAX_INSTANCE_NODES as u32 + 5,
        ));
        let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
        let analysis = graph_analysis(vec![parent, top, leaf], Vec::new(), Some("top"));

        let result = snapshot_analysis("root", &analysis, identity_source);
        let parent_root = result
            .roots
            .iter()
            .find(|root| root.module_type == "parent")
            .expect("source parent root");
        let top_root = &parent_root.children[0];
        let cycle = top_root
            .children
            .iter()
            .find(|child| child.instance_name == "u_cycle")
            .expect("cycle child");
        assert!(cycle.is_cycle_truncated);
        assert!(!cycle.is_budget_truncated);
        let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
        assert!(total <= MAX_INSTANCE_NODES);
        assert_eq!(budget_markers, 1);
        assert_eq!(cycle_markers, 1);
    }

    #[test]
    fn graph_snapshot_json_has_deterministic_ids_and_camel_case_optional_fields() {
        let mut first_definition = source_definition(
            "top",
            "/workspace/top.sv",
            1,
            vec![source_instance("u_child", "child", 4)],
        );
        first_definition.ports.push(ModuleGraphPort {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: ty("logic", Some(1)),
            detail: Some("input logic clk".to_owned()),
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        });
        let second_definition = source_definition("child", "/workspace/child.sv", 1, Vec::new());
        let first = graph_analysis(
            vec![first_definition.clone(), second_definition.clone()],
            Vec::new(),
            None,
        );
        let second = graph_analysis(vec![second_definition, first_definition], Vec::new(), None);
        let first_json = serde_json::to_value(snapshot_analysis("root", &first, identity_source))
            .expect("serialize first graph snapshot");
        let second_json = serde_json::to_value(snapshot_analysis("root", &second, identity_source))
            .expect("serialize second graph snapshot");
        assert_eq!(first_json, second_json);
        let root = &first_json["roots"][0];
        assert!(root["id"]
            .as_str()
            .is_some_and(|id| id.contains("/workspace/top.sv")));
        assert_eq!(root["contentSource"], "declaration");
        assert!(root.get("content_source").is_none());
        assert!(root.get("isAmbiguous").is_none());
        assert_eq!(root["ports"][0]["detail"], "input logic clk");
    }
}
