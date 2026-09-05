//! Owned source module graph captured while the Surelog parse tree is alive.

use super::*;

#[derive(Debug, Clone, PartialEq, Default)]
/// Source declarations and per-instance elaborated type facts.
pub(crate) struct ModuleGraph {
    pub definitions: Vec<ModuleGraphDefinition>,
    /// Per-instance packed bounds captured from the elaborated VPI tree while
    /// the analysis session is alive.  A source definition can be instantiated
    /// more than once with different parameter values, so this cannot live on
    /// `ModuleGraphDefinition` alone.
    pub elaborated_types: Vec<ModuleGraphElaboratedType>,
}

/// One concrete packed dimension from an elaborated typespec.  `None` in the
/// range list means that Surelog retained the dimension but did not fold one
/// of its bounds; the explorer then keeps that dimension's normalized source
/// spelling instead of inventing a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModuleGraphPackedRange {
    pub left: i128,
    pub right: i128,
}

/// Elaborated packed dimensions for one object in one instance scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleGraphElaboratedType {
    /// Cleaned `vpiFullName`/instance hierarchy, for example `top.u_child`.
    pub instance: String,
    pub name: String,
    pub packed_ranges: Vec<Option<ModuleGraphPackedRange>>,
}

/// One source-level module definition, identified by source identity rather
/// than by module name alone (duplicate definitions are legal and must not be
/// conflated).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphDefinition {
    pub id: String,
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub ports: Vec<ModuleGraphPort>,
    pub params: Vec<ModuleGraphParameter>,
    pub signals: Vec<ModuleGraphSignal>,
    pub children: Vec<ModuleGraphInstance>,
    pub generated_scopes: Vec<ModuleGraphGenerateScope>,
}

/// Declaration identity retained with source-graph contents.  The file is
/// kept as the analyzed (possibly shadow-tree) path until the backend remaps
/// it at response presentation time; the line/column range itself is already
/// zero-independent source data captured during analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleGraphLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Shape metadata for the normalized source type retained beside a graph
/// entry.  `TypeInfo` owns the authoritative elaborated packed width; this
/// metadata only tells the explorer which source dimensions are unpacked so
/// they can survive when the packed part is rendered from that width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ModuleGraphTypeShape {
    pub packed_dimensions: usize,
    pub unpacked_dimensions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphPort {
    pub name: String,
    pub direction: Direction,
    pub ty: llg::core::model::TypeInfo,
    /// Source declaration text when it could be recovered safely.
    pub detail: Option<String>,
    /// Identifier range of the formal declaration, never the connection site.
    pub location: Option<ModuleGraphLocation>,
    /// Normalized type text retained for symbolic fallback (for example
    /// `logic [WIDTH-1:0]`).
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphSignal {
    pub name: String,
    /// Source net/variable kind (`wire`, `reg`, `var`, …).
    pub kind: String,
    pub ty: llg::core::model::TypeInfo,
    pub detail: Option<String>,
    /// Identifier range of the internal declaration, never a use site.
    pub location: Option<ModuleGraphLocation>,
    /// Normalized type text retained for symbolic fallback.
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphParameter {
    pub name: String,
    pub ty: llg::core::model::TypeInfo,
    pub local: bool,
    pub detail: Option<String>,
    /// Identifier range of the parameter declaration, never an override site.
    pub location: Option<ModuleGraphLocation>,
    /// Normalized type text retained for symbolic fallback.
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphInstance {
    pub name: String,
    pub module_type: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphGenerateScope {
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub children: Vec<ModuleGraphInstance>,
    pub nested: Vec<ModuleGraphGenerateScope>,
}

pub(super) type GraphInstanceKey = (String, String, Option<String>, u32, u32);
pub(super) type GraphScopeKey = (String, u32, u32);
pub(super) type GraphDefinitionLineRanges = HashMap<String, HashMap<String, BTreeMap<u32, u32>>>;

/// Transient keyed state used while assembling one source graph.  The public
/// graph deliberately keeps ordered vectors for stable explorer output; these
/// sets/maps make duplicate checks independent of the number of entries
/// already collected for a module.
pub(super) struct GraphAssemblyIndexes {
    pub(super) ports: Vec<HashSet<(String, Option<String>)>>,
    pub(super) params: Vec<HashSet<(String, Option<String>)>>,
    pub(super) signals: Vec<HashSet<(String, Option<String>)>>,
    pub(super) children: Vec<HashSet<GraphInstanceKey>>,
    generated_scopes: Vec<HashMap<Vec<GraphScopeKey>, usize>>,
    generated_children: Vec<HashMap<Vec<GraphScopeKey>, HashSet<GraphInstanceKey>>>,
}

impl GraphAssemblyIndexes {
    pub(super) fn new(definition_count: usize) -> Self {
        Self {
            ports: vec![HashSet::new(); definition_count],
            params: vec![HashSet::new(); definition_count],
            signals: vec![HashSet::new(); definition_count],
            children: vec![HashSet::new(); definition_count],
            generated_scopes: (0..definition_count).map(|_| HashMap::new()).collect(),
            generated_children: (0..definition_count).map(|_| HashMap::new()).collect(),
        }
    }
}

pub(super) fn graph_definition_line_ranges(
    definitions: &[ModuleGraphDefinition],
) -> GraphDefinitionLineRanges {
    let mut ranges = HashMap::new();
    for definition in definitions {
        let Some(file) = definition.file.as_deref() else {
            continue;
        };
        insert_graph_definition_line_range(
            &mut ranges,
            file,
            clean_name(&definition.name),
            definition.line,
            definition.end_line,
        );
    }
    ranges
}

pub(super) fn insert_graph_definition_line_range(
    ranges: &mut GraphDefinitionLineRanges,
    file: &str,
    name: &str,
    start: u32,
    end: u32,
) {
    let end = if end == 0 { u32::MAX } else { end };
    if end < start {
        return;
    }
    let spans = ranges
        .entry(file.to_owned())
        .or_default()
        .entry(name.to_owned())
        .or_default();
    let mut merged_start = start;
    let mut merged_end = end;
    if let Some((&previous_start, &previous_end)) = spans.range(..=start).next_back() {
        if previous_end.saturating_add(1) >= start {
            merged_start = previous_start;
            merged_end = merged_end.max(previous_end);
            spans.remove(&previous_start);
        }
    }
    while let Some((&next_start, &next_end)) = spans.range(merged_start..).next() {
        if next_start > merged_end.saturating_add(1) {
            break;
        }
        spans.remove(&next_start);
        merged_end = merged_end.max(next_end);
    }
    spans.insert(merged_start, merged_end);
}

pub(super) fn graph_definition_line_is_retained(
    ranges: &GraphDefinitionLineRanges,
    file: &str,
    name: &str,
    line: u32,
) -> bool {
    let Some(spans) = ranges.get(file).and_then(|by_name| by_name.get(name)) else {
        return false;
    };
    spans
        .range(..=line)
        .next_back()
        .is_some_and(|(_, end)| line <= *end)
}

/// Stable source identity shared by the graph and explorer definition IDs.
pub(crate) fn module_graph_definition_id(
    name: &str,
    file: Option<&str>,
    line: u32,
    col: u32,
) -> String {
    format!(
        "{}|{}|{line}|{col}",
        clean_name(name),
        file.unwrap_or("<unknown>")
    )
}

pub(super) fn collect_module_graph(design: &llg::ffi::surelog::Design) -> ModuleGraph {
    use llg::core::vobject_types::VObjectType;
    use llg::ffi::vpi;

    struct GraphFile {
        path: String,
        file_id: u32,
        nodes: Vec<llg::ffi::surelog::ParseNode>,
        source: Option<GraphSourceIndex>,
    }

    let (parse_tokens, _) = tokens::collect_parse_tokens(design);
    let mut token_types: HashMap<&str, HashMap<(u32, u32), i32>> = HashMap::new();
    for file in &parse_tokens {
        let file_types = token_types.entry(file.path.as_str()).or_default();
        for node in &file.nodes {
            file_types
                .entry((node.line, node.col))
                .or_insert(node.vpi_type);
        }
    }

    let mut files = Vec::new();
    for file_index in 0..design.file_content_count() {
        let Some(file_content) = design.file_content(file_index) else {
            continue;
        };
        let path = file_content.path();
        let file_id = file_content.file_id();
        let nodes = (0..file_content.node_count())
            .filter_map(|index| file_content.get_node(index))
            .collect::<Vec<_>>();
        // This is analysis-time source recovery, not request-time access.  It
        // is used only for optional declaration detail strings and never for
        // graph membership or parsing decisions.
        let source = std::fs::read_to_string(&path)
            .ok()
            .map(GraphSourceIndex::new);
        files.push(GraphFile {
            path,
            file_id,
            nodes,
            source,
        });
    }
    let source_by_path: HashMap<&str, &GraphSourceIndex> = files
        .iter()
        .filter_map(|file| {
            file.source
                .as_ref()
                .map(|source| (file.path.as_str(), source))
        })
        .collect();

    let mut graph = ModuleGraph::default();
    let mut owners: HashMap<(usize, usize), usize> = HashMap::new();
    let mut seen_definitions: HashSet<(String, String, u32, u32)> = HashSet::new();

    // First retain every module declaration and establish node → definition
    // ownership from the actual parse-tree parent/child links.  The range
    // fallback below is only for malformed trees whose links omit a body.
    for (file_index, file) in files.iter().enumerate() {
        let mut roots = file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.file_id == file.file_id
                    && graph_node_type(&file.nodes, node.type_id)
                        == Some(VObjectType::paModule_declaration)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if roots.is_empty() {
            roots = file
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    node.file_id == file.file_id
                        && matches!(
                            graph_node_type(&file.nodes, node.type_id),
                            Some(VObjectType::paModule_ansi_header)
                                | Some(VObjectType::paModule_nonansi_header)
                        )
                })
                .map(|(index, _)| index)
                .collect();
        }
        roots.sort_by_key(|index| graph_position(&file.nodes[*index], *index));

        for root in roots {
            let header = graph_subtree_indices(&file.nodes, root)
                .into_iter()
                .filter(|index| {
                    matches!(
                        graph_node_type(&file.nodes, file.nodes[*index].type_id),
                        Some(VObjectType::paModule_ansi_header)
                            | Some(VObjectType::paModule_nonansi_header)
                    )
                })
                .min_by_key(|index| graph_position(&file.nodes[*index], *index))
                .unwrap_or(root);
            let Some((_, name)) = graph_first_string(&file.nodes, header, file.file_id) else {
                continue;
            };
            let name = clean_name(&name).to_owned();
            if name.is_empty() {
                continue;
            }
            let declaration = &file.nodes[root];
            let anchor = if declaration.line == 0 || declaration.col == 0 {
                &file.nodes[header]
            } else {
                declaration
            };
            let end_line = anchor.end_line.max(anchor.line);
            let col = graph_lsp_column(file.source.as_ref(), anchor.line, anchor.col as u32, None);
            let end_col = if end_line == anchor.line {
                col.saturating_add(lsp_name_len(&name))
            } else {
                graph_lsp_column(file.source.as_ref(), end_line, anchor.end_col as u32, None)
            };
            let key = (
                file.path.clone(),
                name.clone(),
                anchor.line,
                anchor.col as u32,
            );
            if !seen_definitions.insert(key) {
                continue;
            }
            let definition_index = graph.definitions.len();
            graph.definitions.push(ModuleGraphDefinition {
                id: module_graph_definition_id(&name, Some(file.path.as_str()), anchor.line, col),
                name,
                file: Some(file.path.clone()),
                line: anchor.line,
                col,
                end_line,
                end_col,
                ports: Vec::new(),
                params: Vec::new(),
                signals: Vec::new(),
                children: Vec::new(),
                generated_scopes: Vec::new(),
            });
            for node_index in graph_subtree_indices(&file.nodes, root) {
                owners.insert((file_index, node_index), definition_index);
            }
            // Keep the header as an owner even when the module declaration's
            // child links are incomplete.
            owners.insert((file_index, header), definition_index);
        }
    }

    let mut retained_definition_ranges = graph_definition_line_ranges(&graph.definitions);

    // A parse-token module is a safe final fallback for a malformed tree that
    // retained the name token but lost its enclosing declaration node.  It is
    // an empty definition (no guessed edges).  The token often also appears
    // inside a real `paModule_declaration`, so deduplicate by the retained
    // source span before adding it; distinct same-named declarations in one
    // file remain distinct when their spans do not overlap.  The keyed range
    // index keeps this fallback linear in the number of module tokens instead
    // of scanning every retained definition for each token.
    for file in &parse_tokens {
        for node in &file.nodes {
            if node.vpi_type != vpi::vpiModule
                || node.name.as_deref().unwrap_or_default().is_empty()
            {
                continue;
            }
            let name = clean_name(node.name.as_deref().unwrap_or_default()).to_owned();
            if graph_definition_line_is_retained(
                &retained_definition_ranges,
                &file.path,
                &name,
                node.line,
            ) {
                continue;
            }
            let key = (file.path.clone(), name.clone(), node.line, node.col);
            if !seen_definitions.insert(key) {
                continue;
            }
            let end_line = node.end_line.max(node.line);
            let source = source_by_path.get(file.path.as_str()).copied();
            let col = graph_lsp_column(source, node.line, node.col, Some(&name));
            let end_col = if end_line == node.line {
                col.saturating_add(lsp_name_len(&name))
            } else {
                graph_lsp_column(source, end_line, node.end_col, None)
            };
            graph.definitions.push(ModuleGraphDefinition {
                id: module_graph_definition_id(&name, Some(file.path.as_str()), node.line, col),
                name: name.clone(),
                file: Some(file.path.clone()),
                line: node.line,
                col,
                end_line,
                end_col,
                ports: Vec::new(),
                params: Vec::new(),
                signals: Vec::new(),
                children: Vec::new(),
                generated_scopes: Vec::new(),
            });
            insert_graph_definition_line_range(
                &mut retained_definition_ranges,
                &file.path,
                &name,
                node.line,
                end_line,
            );
        }
    }

    let mut assembly_indexes = GraphAssemblyIndexes::new(graph.definitions.len());

    // Attribute nodes whose parent links do not reach a retained module root
    // by source range.  We choose only a single containing definition; ties
    // are left unresolved instead of guessing across duplicate ranges.  The
    // span index keeps this malformed-tree fallback from testing every
    // definition for every node (the common case is a single active module
    // span per file).
    let mut definition_spans_by_file: HashMap<&str, Vec<GraphDefinitionSpan>> = HashMap::new();
    for (definition_index, definition) in graph.definitions.iter().enumerate() {
        let Some(file) = definition.file.as_deref() else {
            continue;
        };
        definition_spans_by_file
            .entry(file)
            .or_default()
            .push(GraphDefinitionSpan {
                definition_index,
                start_line: definition.line,
                end_line: if definition.end_line == 0 {
                    u32::MAX
                } else {
                    definition.end_line
                },
                rank: (
                    definition.end_line.saturating_sub(definition.line),
                    definition.id.clone(),
                    definition_index,
                ),
            });
    }
    for spans in definition_spans_by_file.values_mut() {
        spans.sort_by_key(|span| (span.start_line, span.end_line, span.definition_index));
    }
    for (file_index, file) in files.iter().enumerate() {
        let Some(spans) = definition_spans_by_file.get(file.path.as_str()) else {
            continue;
        };
        let mut node_indices = file
            .nodes
            .iter()
            .enumerate()
            .filter(|(node_index, node)| {
                node.file_id == file.file_id && !owners.contains_key(&(file_index, *node_index))
            })
            .map(|(node_index, _)| node_index)
            .collect::<Vec<_>>();
        node_indices.sort_by_key(|index| graph_position(&file.nodes[*index], *index));

        let mut next_span = 0usize;
        let mut active_by_end = BTreeSet::new();
        let mut active_by_rank = BTreeSet::new();
        for node_index in node_indices {
            let line = file.nodes[node_index].line;
            while next_span < spans.len() && spans[next_span].start_line <= line {
                let span = &spans[next_span];
                active_by_end.insert((span.end_line, next_span));
                active_by_rank.insert(span.rank.clone());
                next_span += 1;
            }
            while let Some(&(end_line, span_index)) = active_by_end.first() {
                if end_line >= line {
                    break;
                }
                active_by_end.remove(&(end_line, span_index));
                let span = &spans[span_index];
                active_by_rank.remove(&span.rank);
            }
            if let Some((_, _, definition_index)) = active_by_rank.first() {
                owners.insert((file_index, node_index), *definition_index);
            }
        }
    }

    // Declaration contents are recovered from the same parse/token data as
    // the graph edges.  `graph_declaration_kind` rejects expression leaves,
    // instance names, connection labels, and nested function/task locals, so
    // only declaration identifiers become explorer contents.
    for (file_index, file) in files.iter().enumerate() {
        let mut declaration_facts = GraphDeclarationFactsCache::default();
        for (node_index, node) in file.nodes.iter().enumerate() {
            if node.file_id != file.file_id
                || node.type_id != VObjectType::slStringConst as u16
                || node.line == 0
                || node.col == 0
            {
                continue;
            }
            let Some(definition_index) = owners.get(&(file_index, node_index)).copied() else {
                continue;
            };
            let Some(name) = node.symbol_name.as_deref().filter(|name| !name.is_empty()) else {
                continue;
            };
            let Some((declaration_root, declaration_kind)) =
                graph_declaration_kind(&file.nodes, node_index, &mut declaration_facts)
            else {
                continue;
            };
            let Some(token_type) = token_types
                .get(file.path.as_str())
                .and_then(|types| types.get(&(node.line, node.col as u32)))
            else {
                continue;
            };
            if !graph_token_matches_declaration(*token_type, &declaration_kind) {
                continue;
            }
            let declaration_facts = declaration_facts.facts(&file.nodes, declaration_root);
            let source_parts = file.source.as_ref().and_then(|source| {
                graph_type_prefix(
                    Some(source),
                    Some(&file.nodes[declaration_root]),
                    node.line,
                    node.col as u32,
                    name,
                )
            });
            let ty = graph_type_info(
                &file.nodes,
                declaration_root,
                &declaration_facts.type_info,
                file.source.as_ref(),
                source_parts.as_ref(),
                node.line,
                node.col as u32,
                name,
            );
            let location = Some(graph_declaration_location(
                &file.path,
                file.source.as_ref(),
                node,
                name,
            ));
            let graph_display = graph_type_display(
                file.source.as_ref(),
                &file.nodes[declaration_root],
                node.line,
                node.col as u32,
                name,
                &ty,
                &declaration_kind,
                source_parts.as_ref(),
            );
            let display_type = graph_display.text;
            let display_shape = graph_display.shape;
            let detail = graph_declaration_detail(
                file.source.as_ref(),
                node.line,
                node.col as u32,
                name,
                &ty,
                &declaration_kind,
            );
            match declaration_kind {
                GraphDeclarationKind::Port(direction) => {
                    if assembly_indexes.ports[definition_index]
                        .insert((name.to_owned(), detail.clone()))
                    {
                        graph.definitions[definition_index]
                            .ports
                            .push(ModuleGraphPort {
                                name: name.to_owned(),
                                direction,
                                ty,
                                detail,
                                location,
                                display_type,
                                display_shape,
                            });
                    }
                }
                GraphDeclarationKind::Parameter(local) => {
                    if assembly_indexes.params[definition_index]
                        .insert((name.to_owned(), detail.clone()))
                    {
                        graph.definitions[definition_index]
                            .params
                            .push(ModuleGraphParameter {
                                name: name.to_owned(),
                                ty,
                                local,
                                detail,
                                location,
                                display_type,
                                display_shape,
                            });
                    }
                }
                GraphDeclarationKind::Signal(kind) => {
                    if assembly_indexes.signals[definition_index]
                        .insert((name.to_owned(), detail.clone()))
                    {
                        graph.definitions[definition_index]
                            .signals
                            .push(ModuleGraphSignal {
                                name: name.to_owned(),
                                kind,
                                ty,
                                detail,
                                location,
                                display_type,
                                display_shape,
                            });
                    }
                }
            }
        }
    }

    // Source-level instance edges.  The type name is accepted only from the
    // parse classifier's module-type token (`uhdmclass_defn`); if Surelog
    // cannot expose that evidence, the edge is omitted rather than guessed.
    for (file_index, file) in files.iter().enumerate() {
        for (inst_index, inst_node) in file.nodes.iter().enumerate() {
            if inst_node.file_id != file.file_id
                || graph_node_type(&file.nodes, inst_node.type_id)
                    != Some(VObjectType::paModule_instantiation)
            {
                continue;
            }
            let Some(parent_definition) = owners.get(&(file_index, inst_index)).copied() else {
                continue;
            };
            let Some(module_type) = graph_instantiation_type(
                &file.nodes,
                inst_index,
                file.file_id,
                &file.path,
                &token_types,
            ) else {
                continue;
            };
            let mut instance_nodes = graph_subtree_indices(&file.nodes, inst_index)
                .into_iter()
                .filter(|index| {
                    graph_node_type(&file.nodes, file.nodes[*index].type_id)
                        == Some(VObjectType::paName_of_instance)
                })
                .collect::<Vec<_>>();
            instance_nodes.sort_by_key(|index| graph_position(&file.nodes[*index], *index));
            for name_node in instance_nodes {
                let Some((name_index, instance_name)) =
                    graph_first_string(&file.nodes, name_node, file.file_id)
                else {
                    continue;
                };
                let instance = ModuleGraphInstance {
                    name: instance_name,
                    module_type: clean_name(&module_type).to_owned(),
                    file: Some(file.path.clone()),
                    line: file.nodes[name_index].line,
                    col: graph_lsp_column(
                        file.source.as_ref(),
                        file.nodes[name_index].line,
                        file.nodes[name_index].col as u32,
                        file.nodes[name_index].symbol_name.as_deref(),
                    ),
                };
                let mut scopes = graph_generate_ancestors(&file.nodes, inst_index);
                for scope in &mut scopes {
                    scope.col = graph_lsp_column(file.source.as_ref(), scope.line, scope.col, None);
                }
                if scopes.is_empty() {
                    graph_push_instance(
                        &mut graph.definitions[parent_definition].children,
                        &mut assembly_indexes.children[parent_definition],
                        instance,
                    );
                } else {
                    graph_push_generated_instance(
                        parent_definition,
                        &mut graph.definitions[parent_definition],
                        &scopes,
                        instance,
                        &mut assembly_indexes,
                    );
                }
            }
        }
    }

    for definition in &mut graph.definitions {
        definition.ports.sort_by(|left, right| {
            (
                left.name.as_str(),
                left.detail.as_deref().unwrap_or_default(),
            )
                .cmp(&(
                    right.name.as_str(),
                    right.detail.as_deref().unwrap_or_default(),
                ))
        });
        definition.params.sort_by(|left, right| {
            (
                left.name.as_str(),
                left.local,
                left.detail.as_deref().unwrap_or_default(),
            )
                .cmp(&(
                    right.name.as_str(),
                    right.local,
                    right.detail.as_deref().unwrap_or_default(),
                ))
        });
        definition.signals.sort_by(|left, right| {
            (
                left.name.as_str(),
                left.kind.as_str(),
                left.detail.as_deref().unwrap_or_default(),
            )
                .cmp(&(
                    right.name.as_str(),
                    right.kind.as_str(),
                    right.detail.as_deref().unwrap_or_default(),
                ))
        });
        definition.children.sort_by(graph_instance_cmp);
        graph_sort_scopes(&mut definition.generated_scopes);
    }
    graph
        .definitions
        .sort_by(|left, right| left.id.cmp(&right.id));
    graph
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GraphDeclarationKind {
    Port(Direction),
    Parameter(bool),
    Signal(String),
}

#[derive(Debug, Clone)]
pub(super) struct GraphDefinitionSpan {
    definition_index: usize,
    start_line: u32,
    end_line: u32,
    rank: (u32, String, usize),
}

/// Facts that are shared by every declarator under one parse-tree declaration
/// root.  Keeping the subtree walk here makes compact declarations such as
/// `logic a, b, c;` linear in the root size rather than repeating the same
/// traversal for every name.
#[derive(Debug, Clone)]
pub(super) struct GraphDeclarationSubtreeFacts {
    type_info: TypeInfo,
    direction: Direction,
    net_kind: String,
    variable_kind: String,
}

#[derive(Debug, Default)]
pub(super) struct GraphDeclarationFactsCache {
    pub(super) by_root: HashMap<usize, GraphDeclarationSubtreeFacts>,
    #[cfg(test)]
    pub(super) subtree_walks: usize,
}

impl GraphDeclarationFactsCache {
    fn facts(
        &mut self,
        nodes: &[llg::ffi::surelog::ParseNode],
        root: usize,
    ) -> &GraphDeclarationSubtreeFacts {
        self.by_root.entry(root).or_insert_with(|| {
            #[cfg(test)]
            {
                self.subtree_walks += 1;
            }
            graph_declaration_subtree_facts(nodes, root)
        });
        self.by_root
            .get(&root)
            .expect("declaration facts inserted above")
    }
}

/// Byte ranges and character boundaries for one source line.  The
/// declaration recovery pass asks for the same line repeatedly when an ANSI
/// header or a compact declaration contains many names, so these facts are
/// built once instead of rescanning the line for every name.
#[derive(Debug)]
pub(super) struct GraphLineMetadata {
    pub(super) start: usize,
    pub(super) end: usize,
    /// Number of Unicode scalar values in the line, excluding its line break.
    pub(super) character_count: usize,
    /// Relative byte offsets for every `GRAPH_CHARACTER_CHECKPOINT_STRIDE`
    /// scalar values on a non-ASCII line.  ASCII lines use the direct byte
    /// offset path and keep this empty, avoiding one entry per character.
    pub(super) character_checkpoints: Vec<usize>,
    clause_start_offsets: Vec<(usize, char)>,
    clause_end_offsets: Vec<usize>,
}

pub(super) const GRAPH_CHARACTER_CHECKPOINT_STRIDE: usize = 64;

/// Immutable, per-file source facts used by the source graph.
///
/// The original text is retained for exact detail/type spelling.  `masked`
/// has the same byte length as the original and replaces comment bytes with
/// spaces, so structural scans can walk it without repeatedly rebuilding
/// comment ranges.  `stripped` retains the historical one-space-per-comment
/// character form used for declaration-line details; unlike `masked`, it is
/// not required to preserve byte offsets.
#[derive(Debug)]
pub(super) struct GraphSourceIndex {
    pub(super) source: String,
    pub(super) masked: String,
    pub(super) stripped: String,
    pub(super) line_starts: Vec<usize>,
    pub(super) source_lines: Vec<GraphLineMetadata>,
    pub(super) stripped_lines: Vec<GraphLineMetadata>,
    pub(super) comment_ranges: Vec<(usize, usize)>,
    declaration_boundary_events: Vec<(usize, usize)>,
    delimiter_state_events: Vec<(usize, GraphDelimiterState)>,
    pub(super) commas_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    declaration_tail_events_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    equals_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
}

impl GraphSourceIndex {
    pub(super) fn new(source: String) -> Self {
        let comment_ranges = graph_comment_ranges(&source);
        let masked = mask_graph_comments(&source, &comment_ranges);
        let stripped = strip_graph_comments_with_ranges(&source, &comment_ranges);
        let line_starts = graph_line_starts(&source);
        let stripped_line_starts = graph_line_starts(&stripped);
        let source_lines = graph_line_metadata(&source, &line_starts, false, false);
        let stripped_lines = graph_line_metadata(&stripped, &stripped_line_starts, true, true);
        let syntax = graph_source_syntax_facts(&masked);
        Self {
            source,
            masked,
            stripped,
            line_starts,
            source_lines,
            stripped_lines,
            comment_ranges,
            declaration_boundary_events: syntax.declaration_boundary_events,
            delimiter_state_events: syntax.delimiter_state_events,
            commas_by_state: syntax.commas_by_state,
            declaration_tail_events_by_state: syntax.declaration_tail_events_by_state,
            equals_by_state: syntax.equals_by_state,
        }
    }

    pub(super) fn line_start(&self, line: u32) -> Option<usize> {
        graph_indexed_line_start(&self.line_starts, &self.source, line)
    }

    fn source_line(&self, line: u32) -> Option<&GraphLineMetadata> {
        let line_index = usize::try_from(line.checked_sub(1)?).ok()?;
        self.source_lines.get(line_index)
    }

    fn stripped_line(&self, line: u32) -> Option<&GraphLineMetadata> {
        let line_index = usize::try_from(line.checked_sub(1)?).ok()?;
        self.stripped_lines.get(line_index)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn stripped_line_text(&self, line: u32) -> Option<&str> {
        let line = self.stripped_line(line)?;
        if line.start == self.stripped.len()
            && (self.stripped.is_empty() || self.stripped.ends_with('\n'))
        {
            return None;
        }
        Some(&self.stripped[line.start..line.end])
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn line_text(&self, line: u32) -> Option<&str> {
        self.stripped_line_text(line)
    }

    fn stripped_position_offset(&self, line: u32, col: u32) -> Option<usize> {
        let line = self.stripped_line(line)?;
        let character = usize::try_from(col.checked_sub(1)?).ok()?;
        graph_line_character_offset(&self.stripped, line, character)
    }

    pub(super) fn position_offset(&self, line: u32, col: u32) -> Option<usize> {
        let line = self.source_line(line)?;
        let character = usize::try_from(col.checked_sub(1)?).ok()?;
        graph_line_character_offset(&self.source, line, character)
    }

    fn comment_cursor_at(&self, offset: usize) -> GraphCommentCursor<'_> {
        GraphCommentCursor::at_offset(&self.comment_ranges, offset)
    }

    fn delimiter_state_at(&self, offset: usize) -> GraphDelimiterState {
        let mut low = 0;
        let mut high = self.delimiter_state_events.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if self.delimiter_state_events[middle].0 <= offset {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        self.delimiter_state_events
            .get(low.saturating_sub(1))
            .map_or_else(GraphDelimiterState::default, |(_, state)| *state)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(super) struct GraphDelimiterState {
    pub(super) square: usize,
    pub(super) paren: usize,
    pub(super) brace: usize,
}

impl GraphDelimiterState {
    fn is_zero(self) -> bool {
        self.square == 0 && self.paren == 0 && self.brace == 0
    }

    fn increment(&mut self, delimiter: char) {
        match delimiter {
            '[' => self.square += 1,
            '(' => self.paren += 1,
            '{' => self.brace += 1,
            _ => {}
        }
    }

    fn decrement_open(&mut self, delimiter: char) {
        match delimiter {
            '[' => self.square = self.square.saturating_sub(1),
            '(' => self.paren = self.paren.saturating_sub(1),
            '{' => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GraphDelimiterFrame {
    character: char,
    offset: usize,
    previous_boundary: Option<usize>,
    previous_tops: [Option<usize>; 3],
}

pub(super) fn graph_delimiter_slot(delimiter: char) -> Option<usize> {
    match delimiter {
        '[' => Some(0),
        '(' => Some(1),
        '{' => Some(2),
        _ => None,
    }
}

#[derive(Debug)]
pub(super) struct GraphSourceSyntaxFacts {
    declaration_boundary_events: Vec<(usize, usize)>,
    delimiter_state_events: Vec<(usize, GraphDelimiterState)>,
    pub(super) commas_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    declaration_tail_events_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    equals_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
}

pub(super) fn graph_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
    );
    starts
}

pub(super) fn graph_line_metadata(
    text: &str,
    starts: &[usize],
    trim_carriage_return: bool,
    collect_separators: bool,
) -> Vec<GraphLineMetadata> {
    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            // The next line starts immediately after its `\n`; retaining the
            // preceding `\r` here preserves the old source-position behavior
            // for CRLF input.  Stripped display lines opt out of that byte.
            let raw_end = starts
                .get(index + 1)
                .copied()
                .map_or(text.len(), |next| next.saturating_sub(1));
            let end = if trim_carriage_return
                && raw_end > start
                && text.as_bytes().get(raw_end - 1) == Some(&b'\r')
            {
                raw_end - 1
            } else {
                raw_end
            };
            let (character_count, character_checkpoints) =
                graph_line_character_metadata(&text[start..end]);
            let (clause_start_offsets, clause_end_offsets) = if collect_separators {
                let mut starts = Vec::new();
                let mut ends = Vec::new();
                for (offset, character) in text[start..end].char_indices() {
                    let offset = start + offset;
                    if matches!(character, ';' | ',' | '(') {
                        starts.push((offset, character));
                    }
                    if matches!(character, ';' | ',' | ')') {
                        ends.push(offset);
                    }
                }
                (starts, ends)
            } else {
                (Vec::new(), Vec::new())
            };
            GraphLineMetadata {
                start,
                end,
                character_count,
                character_checkpoints,
                clause_start_offsets,
                clause_end_offsets,
            }
        })
        .collect()
}

pub(super) fn graph_line_character_metadata(line: &str) -> (usize, Vec<usize>) {
    if line.is_ascii() {
        return (line.len(), Vec::new());
    }

    let mut character_count = 0;
    let mut character_checkpoints = Vec::new();
    for (offset, _) in line.char_indices() {
        if character_count % GRAPH_CHARACTER_CHECKPOINT_STRIDE == 0 {
            character_checkpoints.push(offset);
        }
        character_count += 1;
    }
    (character_count, character_checkpoints)
}

pub(super) fn graph_line_character_offset(
    text: &str,
    line: &GraphLineMetadata,
    character: usize,
) -> Option<usize> {
    if character > line.character_count {
        return None;
    }
    if character == line.character_count {
        return Some(line.end);
    }
    if line.character_checkpoints.is_empty() {
        // ASCII lines have one byte per scalar value, so no metadata is
        // necessary for the common case.
        return Some(line.start + character);
    }

    let checkpoint_character =
        character / GRAPH_CHARACTER_CHECKPOINT_STRIDE * GRAPH_CHARACTER_CHECKPOINT_STRIDE;
    let checkpoint = *line
        .character_checkpoints
        .get(character / GRAPH_CHARACTER_CHECKPOINT_STRIDE)?;
    if checkpoint_character == character {
        return Some(line.start + checkpoint);
    }
    text.get(line.start + checkpoint..line.end)?
        .char_indices()
        .nth(character - checkpoint_character)
        .map(|(offset, _)| line.start + checkpoint + offset)
}

pub(super) fn graph_indexed_line_start(starts: &[usize], text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let index = usize::try_from(line - 1).ok()?;
    starts.get(index).copied().or_else(|| {
        (index == starts.len() && !text.is_empty() && !text.ends_with('\n')).then_some(text.len())
    })
}

pub(super) fn graph_masked_delimiter_events(masked: &str) -> Vec<(usize, char)> {
    let mut events = Vec::new();
    let mut in_string = false;
    let mut escaped_string_character = false;
    let mut escaped_identifier = false;

    for (offset, character) in masked.char_indices() {
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
        if matches!(
            character,
            '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';' | '='
        ) {
            events.push((offset, character));
        }
    }
    events
}

pub(super) fn graph_matching_openers(events: &[(usize, char)]) -> HashSet<usize> {
    let mut matched = HashSet::new();
    let mut stack: Vec<GraphDelimiterFrame> = Vec::new();
    let mut top: [Option<usize>; 3] = [None; 3];
    for &(offset, character) in events {
        if let Some(expected_open) = match character {
            ']' => Some('['),
            ')' => Some('('),
            '}' => Some('{'),
            _ => None,
        } {
            let Some(slot) = graph_delimiter_slot(expected_open) else {
                continue;
            };
            if let Some(index) = top[slot] {
                matched.insert(stack[index].offset);
                top = stack[index].previous_tops;
                stack.truncate(index);
            }
        } else if matches!(character, '[' | '(' | '{') {
            let Some(slot) = graph_delimiter_slot(character) else {
                continue;
            };
            let frame = GraphDelimiterFrame {
                character,
                offset,
                previous_boundary: None,
                previous_tops: top,
            };
            stack.push(frame);
            top[slot] = Some(stack.len() - 1);
        }
    }
    matched
}

/// Record reusable source-structure facts in one per-file preprocessing pass.
/// Parenthesized regions temporarily select the opening parenthesis for the
/// malformed-node declaration fallback; semicolons at the outer level replace
/// that boundary.  The comma index is keyed by the delimiter nesting state at
/// each comma, which lets a declaration slice that starts inside an ANSI port
/// or parameter list find its first/last relative top-level comma by binary
/// search without rescanning or allocating a comma vector.
pub(super) fn graph_source_syntax_facts(masked: &str) -> GraphSourceSyntaxFacts {
    let delimiter_events = graph_masked_delimiter_events(masked);
    let matched_openers = graph_matching_openers(&delimiter_events);
    let mut declaration_boundary_events = vec![(0, 0)];
    let mut delimiter_state_events = vec![(0, GraphDelimiterState::default())];
    let mut commas_by_state: HashMap<GraphDelimiterState, Vec<usize>> = HashMap::new();
    let mut declaration_tail_events_by_state: HashMap<GraphDelimiterState, Vec<usize>> =
        HashMap::new();
    let mut equals_by_state: HashMap<GraphDelimiterState, Vec<usize>> = HashMap::new();
    let mut current_boundary = 0usize;
    let mut state = GraphDelimiterState::default();
    let mut stack: Vec<GraphDelimiterFrame> = Vec::new();
    let mut top: [Option<usize>; 3] = [None; 3];

    for &(offset, character) in &delimiter_events {
        let end = offset + character.len_utf8();
        if matches!(character, ')' | ',' | ';' | '=') {
            declaration_tail_events_by_state
                .entry(state)
                .or_default()
                .push(offset);
        }
        if character == '=' {
            equals_by_state.entry(state).or_default().push(offset);
        }
        match character {
            '[' | '(' | '{' => {
                let previous_boundary =
                    (character == '(' && state.is_zero()).then_some(current_boundary);
                let Some(slot) = graph_delimiter_slot(character) else {
                    continue;
                };
                let frame = GraphDelimiterFrame {
                    character,
                    offset,
                    previous_boundary,
                    previous_tops: top,
                };
                stack.push(frame);
                top[slot] = Some(stack.len() - 1);
                state.increment(character);
                delimiter_state_events.push((end, state));
                if previous_boundary.is_some() {
                    current_boundary = end;
                    declaration_boundary_events.push((end, current_boundary));
                }
            }
            ']' | ')' | '}' => {
                let Some(expected_open) = (match character {
                    ']' => Some('['),
                    ')' => Some('('),
                    '}' => Some('{'),
                    _ => None,
                }) else {
                    continue;
                };
                let Some(slot) = graph_delimiter_slot(expected_open) else {
                    continue;
                };
                let Some(index) = top[slot] else {
                    // Unmatched closers are ignored rather than changing a
                    // later declaration's nesting state.
                    continue;
                };
                while stack.len() > index + 1 {
                    let frame = stack.pop().expect("delimiter stack is non-empty");
                    state.decrement_open(frame.character);
                }
                let frame = stack.pop().expect("matching delimiter is on the stack");
                top = frame.previous_tops;
                state.decrement_open(frame.character);
                delimiter_state_events.push((end, state));
                if let Some(previous_boundary) = frame.previous_boundary {
                    current_boundary = previous_boundary;
                    declaration_boundary_events.push((end, current_boundary));
                }
            }
            ',' => {
                commas_by_state.entry(state).or_default().push(offset);
            }
            '=' => {}
            ';' => {
                let malformed_open = stack
                    .iter()
                    .any(|frame| !matched_openers.contains(&frame.offset));
                if state.is_zero() || malformed_open {
                    if !state.is_zero() {
                        stack.clear();
                        state = GraphDelimiterState::default();
                        delimiter_state_events.push((end, state));
                    }
                    current_boundary = end;
                    declaration_boundary_events.push((end, current_boundary));
                }
            }
            _ => unreachable!("graph delimiter event is structural"),
        }
    }

    GraphSourceSyntaxFacts {
        declaration_boundary_events,
        delimiter_state_events,
        commas_by_state,
        declaration_tail_events_by_state,
        equals_by_state,
    }
}

/// Compatibility helper for focused tests and the declaration-start fallback.
/// Production callers use the facts retained in [`GraphSourceIndex`], so this
/// wrapper is never rerun once per declaration.
#[cfg(test)]
pub(super) fn graph_declaration_boundary_events(masked: &str) -> Vec<(usize, usize)> {
    graph_source_syntax_facts(masked).declaration_boundary_events
}

pub(super) fn mask_graph_comments(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut masked = Vec::with_capacity(text.len());
    let mut cursor = 0usize;
    for (offset, byte) in text.bytes().enumerate() {
        while cursor < ranges.len() && offset >= ranges[cursor].1 {
            cursor += 1;
        }
        let in_comment = ranges
            .get(cursor)
            .is_some_and(|(start, end)| *start <= offset && offset < *end);
        masked.push(if in_comment && !matches!(byte, b'\r' | b'\n') {
            b' '
        } else {
            byte
        });
    }
    String::from_utf8(masked).expect("comment masking preserves source UTF-8")
}

pub(super) fn strip_graph_comments_with_ranges(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut cursor = GraphCommentCursor::new(ranges);
    for (offset, character) in text.char_indices() {
        if cursor.contains(offset) {
            if matches!(character, '\n' | '\r') {
                stripped.push(character);
            } else {
                stripped.push(' ');
            }
        } else {
            stripped.push(character);
        }
    }
    stripped
}

pub(super) fn graph_node_type(
    _nodes: &[llg::ffi::surelog::ParseNode],
    type_id: u16,
) -> Option<llg::core::vobject_types::VObjectType> {
    llg::core::vobject_types::VObjectType::try_from(type_id).ok()
}

pub(super) fn graph_position(
    node: &llg::ffi::surelog::ParseNode,
    index: usize,
) -> (u32, u32, usize) {
    (node.line, node.col as u32, index)
}

pub(super) fn graph_subtree_indices(
    nodes: &[llg::ffi::surelog::ParseNode],
    root: usize,
) -> Vec<usize> {
    if root >= nodes.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    while let Some(index) = stack.pop() {
        if index >= nodes.len() || !seen.insert(index) {
            continue;
        }
        out.push(index);
        let mut child = nodes[index].child_index as usize;
        let mut siblings = Vec::new();
        let mut sibling_seen = HashSet::new();
        let mut sibling_steps = 0usize;
        while child != 0
            && child < nodes.len()
            && sibling_steps < nodes.len()
            && sibling_seen.insert(child)
        {
            siblings.push(child);
            sibling_steps += 1;
            child = nodes[child].sibling_index as usize;
        }
        stack.extend(siblings.into_iter().rev());
    }
    out
}

pub(super) fn graph_first_string(
    nodes: &[llg::ffi::surelog::ParseNode],
    root: usize,
    file_id: u32,
) -> Option<(usize, String)> {
    graph_subtree_indices(nodes, root)
        .into_iter()
        .filter(|index| {
            let node = &nodes[*index];
            node.file_id == file_id
                && node.type_id == llg::core::vobject_types::VObjectType::slStringConst as u16
                && node.line != 0
                && node.col != 0
        })
        .min_by_key(|index| graph_position(&nodes[*index], *index))
        .and_then(|index| {
            nodes[index]
                .symbol_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map(|name| (index, name.to_owned()))
        })
}

pub(super) fn graph_ancestors(nodes: &[llg::ffi::surelog::ParseNode], start: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut current = start;
    let mut seen = HashSet::new();
    for _ in 0..128 {
        if current >= nodes.len() || !seen.insert(current) {
            break;
        }
        out.push(current);
        let parent = nodes[current].parent_index as usize;
        if parent == 0 {
            break;
        }
        current = parent;
    }
    out
}

pub(super) fn graph_has_ancestor(
    nodes: &[llg::ffi::surelog::ParseNode],
    start: usize,
    wanted: impl Fn(llg::core::vobject_types::VObjectType) -> bool,
) -> bool {
    graph_ancestors(nodes, start)
        .into_iter()
        .filter_map(|index| graph_node_type(nodes, nodes[index].type_id))
        .any(wanted)
}

pub(super) fn graph_declaration_subtree_facts(
    nodes: &[llg::ffi::surelog::ParseNode],
    declaration_root: usize,
) -> GraphDeclarationSubtreeFacts {
    use llg::core::vobject_types::VObjectType;

    let mut type_info = TypeInfo::default();
    let mut direction = Direction::None;
    let mut net_kind = None;
    let mut has_reg = false;
    for index in graph_subtree_indices(nodes, declaration_root) {
        match graph_node_type(nodes, nodes[index].type_id) {
            Some(VObjectType::paInput_declaration | VObjectType::paPortDir_Inp)
                if direction == Direction::None =>
            {
                direction = Direction::Input;
            }
            Some(VObjectType::paOutput_declaration | VObjectType::paPortDir_Out)
                if direction == Direction::None =>
            {
                direction = Direction::Output;
            }
            Some(VObjectType::paInout_declaration | VObjectType::paPortDir_Inout)
                if direction == Direction::None =>
            {
                direction = Direction::Inout;
            }
            Some(VObjectType::paNetType_Wire | VObjectType::paWIRE) if net_kind.is_none() => {
                net_kind = Some("wire".to_owned());
            }
            Some(VObjectType::paNetType_Wand | VObjectType::paWAND) if net_kind.is_none() => {
                net_kind = Some("wand".to_owned());
            }
            Some(VObjectType::paNetType_Wor | VObjectType::paWOR) if net_kind.is_none() => {
                net_kind = Some("wor".to_owned());
            }
            Some(VObjectType::paNetType_Tri | VObjectType::paTRI) if net_kind.is_none() => {
                net_kind = Some("tri".to_owned());
            }
            Some(VObjectType::paNetType_Tri0 | VObjectType::paTRI0) if net_kind.is_none() => {
                net_kind = Some("tri0".to_owned());
            }
            Some(VObjectType::paNetType_Tri1 | VObjectType::paTRI1) if net_kind.is_none() => {
                net_kind = Some("tri1".to_owned());
            }
            Some(VObjectType::paNetType_Uwire | VObjectType::paUWIRE) if net_kind.is_none() => {
                net_kind = Some("uwire".to_owned());
            }
            Some(VObjectType::paNetType_Supply0 | VObjectType::paSUPPLY0) if net_kind.is_none() => {
                net_kind = Some("supply0".to_owned());
            }
            Some(VObjectType::paNetType_Supply1 | VObjectType::paSUPPLY1) if net_kind.is_none() => {
                net_kind = Some("supply1".to_owned());
            }
            Some(VObjectType::paREG) => {
                has_reg = true;
            }
            Some(VObjectType::paLOGIC) => type_info.kind = "logic".to_owned(),
            Some(VObjectType::paBIT) => type_info.kind = "bit".to_owned(),
            Some(VObjectType::paINT) => type_info.kind = "int".to_owned(),
            Some(VObjectType::paINTEGER) => type_info.kind = "integer".to_owned(),
            Some(VObjectType::paLONGINT) => type_info.kind = "longint".to_owned(),
            Some(VObjectType::paBYTE) => type_info.kind = "byte".to_owned(),
            Some(VObjectType::paSHORTINT) => type_info.kind = "shortint".to_owned(),
            Some(VObjectType::paTIME) => type_info.kind = "time".to_owned(),
            Some(VObjectType::paREAL) => type_info.kind = "real".to_owned(),
            Some(VObjectType::paSHORTREAL) => type_info.kind = "shortreal".to_owned(),
            Some(VObjectType::paSTRING) => type_info.kind = "string".to_owned(),
            Some(VObjectType::paENUM | VObjectType::paEnum_keyword) => {
                type_info.kind = "enum".to_owned();
            }
            Some(VObjectType::paSTRUCT | VObjectType::paStruct_keyword) => {
                type_info.kind = "struct".to_owned();
            }
            Some(VObjectType::paUNION | VObjectType::paUnion_keyword) => {
                type_info.kind = "union".to_owned();
            }
            Some(VObjectType::paSIGNED | VObjectType::paSigning_Signed) => {
                type_info.signed = true;
            }
            _ => {}
        }
    }

    GraphDeclarationSubtreeFacts {
        type_info,
        direction,
        net_kind: net_kind.unwrap_or_else(|| "wire".to_owned()),
        variable_kind: if has_reg {
            "reg".to_owned()
        } else {
            "var".to_owned()
        },
    }
}

pub(super) fn graph_declaration_kind(
    nodes: &[llg::ffi::surelog::ParseNode],
    start: usize,
    cache: &mut GraphDeclarationFactsCache,
) -> Option<(usize, GraphDeclarationKind)> {
    use llg::core::vobject_types::VObjectType;

    // A declaration node is encountered before its enclosing function/task or
    // class/package node while walking upward. Reject those scopes up front so
    // procedural locals and class fields cannot leak into module Signals in a
    // declaration-only explorer entry.
    if graph_has_ancestor(nodes, start, |ancestor| {
        matches!(
            ancestor,
            VObjectType::paFunction_declaration
                | VObjectType::paTask_declaration
                | VObjectType::paClass_declaration
                | VObjectType::paPackage_declaration
        )
    }) {
        return None;
    }

    let mut current = nodes.get(start)?.parent_index as usize;
    let mut seen = HashSet::new();
    for _ in 0..128 {
        if current >= nodes.len() || !seen.insert(current) {
            return None;
        }
        let ty = graph_node_type(nodes, nodes[current].type_id)?;
        if matches!(
            ty,
            VObjectType::paPrimary
                | VObjectType::paPrimary_literal
                | VObjectType::paExpression
                | VObjectType::paNet_lvalue
                | VObjectType::paVariable_lvalue
                | VObjectType::paName_of_instance
                | VObjectType::paNamed_port_connection
                | VObjectType::paNamed_parameter_assignment
                | VObjectType::paModule_instantiation
                | VObjectType::paFunction_declaration
                | VObjectType::paTask_declaration
                | VObjectType::paClass_declaration
                | VObjectType::paPackage_declaration
        ) {
            return None;
        }
        if matches!(
            ty,
            VObjectType::paData_type
                | VObjectType::paData_type_or_implicit
                | VObjectType::paData_type_or_void
                | VObjectType::paSimple_type
                | VObjectType::paClass_type
                | VObjectType::paInteger_type
                | VObjectType::paInteger_vector_type
                | VObjectType::paInteger_atom_type
                | VObjectType::paEnum_base_type
                | VObjectType::paNet_type
        ) {
            return None;
        }
        let declaration = match ty {
            VObjectType::paPort_declaration
            | VObjectType::paAnsi_port_declaration
            | VObjectType::paNet_port_header
            | VObjectType::paVariable_port_header
            | VObjectType::paInput_declaration
            | VObjectType::paOutput_declaration
            | VObjectType::paInout_declaration => {
                let direction = cache.facts(nodes, current).direction;
                Some(GraphDeclarationKind::Port(direction))
            }
            VObjectType::paParameter_declaration
            | VObjectType::paParameter_port_declaration
            | VObjectType::paParam_assignment => Some(GraphDeclarationKind::Parameter(
                graph_has_ancestor(nodes, current, |ancestor| {
                    ancestor == VObjectType::paLocal_parameter_declaration
                }),
            )),
            VObjectType::paLocal_parameter_declaration => {
                Some(GraphDeclarationKind::Parameter(true))
            }
            VObjectType::paNet_declaration | VObjectType::paNet_decl_assignment => Some(
                GraphDeclarationKind::Signal(cache.facts(nodes, current).net_kind.clone()),
            ),
            VObjectType::paData_declaration
            | VObjectType::paVariable_declaration
            | VObjectType::paVariable_decl_assignment => Some(GraphDeclarationKind::Signal(
                cache.facts(nodes, current).variable_kind.clone(),
            )),
            VObjectType::paModule_declaration
            | VObjectType::paModule_ansi_header
            | VObjectType::paModule_nonansi_header => return None,
            _ => None,
        };
        if let Some(declaration) = declaration {
            return Some((current, declaration));
        }
        current = nodes[current].parent_index as usize;
        if current == 0 {
            break;
        }
    }
    None
}

pub(super) fn graph_token_matches_declaration(
    token_type: i32,
    kind: &GraphDeclarationKind,
) -> bool {
    use llg::ffi::vpi;
    match kind {
        GraphDeclarationKind::Port(_) => matches!(
            token_type,
            vpi::vpiPort | vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT
        ),
        GraphDeclarationKind::Parameter(_) => token_type == vpi::vpiParameter,
        GraphDeclarationKind::Signal(_) => matches!(
            token_type,
            vpi::vpiNet
                | vpi::vpiReg
                | vpi::uhdmlogic_var
                | vpi::uhdmnet
                | vpi::uhdmlogic_net
                | vpi::uhdmint_var
                | vpi::uhdmreal_var
                | vpi::uhdmbit_var
                | vpi::uhdmbyte_var
                | vpi::uhdmshort_int_var
                | vpi::uhdmlong_int_var
                | vpi::uhdmparameter
        ),
    }
}

// The arguments are independent parse/source facts; collecting them into a
// mutable context object would make this pure type-recovery helper less clear.
#[allow(clippy::too_many_arguments)]
pub(super) fn graph_type_info(
    nodes: &[llg::ffi::surelog::ParseNode],
    declaration_root: usize,
    base_type: &TypeInfo,
    source: Option<&GraphSourceIndex>,
    source_parts: Option<&GraphSourceTypeParts>,
    line: u32,
    col: u32,
    name: &str,
) -> TypeInfo {
    let mut ty = base_type.clone();
    // Some parse trees retain the declaration structure but omit the
    // primitive type keyword from the VObject subtree.  Recover only the
    // standard, unambiguous keywords before the declaration identifier from
    // the source line captured during analysis.  This keeps declaration-only
    // contents typed without trying to resolve user-defined types or doing
    // any request-time file access.
    let recovered_parts = if source_parts.is_none() {
        source.and_then(|source| {
            graph_type_prefix(Some(source), nodes.get(declaration_root), line, col, name)
        })
    } else {
        None
    };
    let parts = source_parts.or(recovered_parts.as_ref());
    if let Some(source_ty) = graph_source_type_info(parts) {
        if source_ty.kind != "other" {
            ty.kind = source_ty.kind;
        }
        ty.signed |= source_ty.signed;
    }
    if ty.width.is_none() {
        ty.width = graph_width_from_source(parts);
    }
    // A scalar primitive has a useful known width even when its parse-tree
    // typespec did not expose one.  A ranged declaration whose bounds remain
    // symbolic must stay unknown; treating it as a scalar would cause the
    // explorer to replace the symbolic range with `[0:0]`.
    if ty.width.is_none() && !parts.is_some_and(|parts| !parts.packed_dimensions.is_empty()) {
        ty.width = match ty.kind.as_str() {
            "logic" | "bit" => Some(1),
            "byte" => Some(8),
            "shortint" => Some(16),
            "int" | "integer" => Some(32),
            "longint" | "time" => Some(64),
            _ => None,
        };
    }
    ty
}

pub(super) fn graph_source_type_info(parts: Option<&GraphSourceTypeParts>) -> Option<TypeInfo> {
    let parts = parts?;
    let (ty, saw_decl_qualifier) = graph_source_type_words_clean(&parts.base);
    (ty.kind != "other" || saw_decl_qualifier).then_some(ty)
}

#[cfg(test)]
pub(super) fn graph_source_type_words(text: &str) -> (TypeInfo, bool) {
    let text = strip_hdl_comments(text);
    graph_source_type_words_clean(&text)
}

pub(super) fn graph_source_type_words_clean(text: &str) -> (TypeInfo, bool) {
    let mut ty = TypeInfo::default();
    let mut saw_decl_qualifier = false;
    let mut saw_net_or_reg = false;
    for word in graph_source_words(text) {
        let word = word.to_ascii_lowercase();
        match word.as_str() {
            "input" | "output" | "inout" | "parameter" | "localparam" | "var" | "const" | "ref"
            | "wire" | "wand" | "wor" | "tri" | "tri0" | "tri1" | "trireg" | "triand" | "trior"
            | "uwire" | "supply0" | "supply1" | "reg" => {
                saw_decl_qualifier = true;
                if matches!(
                    word.as_str(),
                    "wire"
                        | "wand"
                        | "wor"
                        | "tri"
                        | "tri0"
                        | "tri1"
                        | "trireg"
                        | "triand"
                        | "trior"
                        | "uwire"
                        | "supply0"
                        | "supply1"
                        | "reg"
                ) {
                    saw_net_or_reg = true;
                }
            }
            "signed" => ty.signed = true,
            "unsigned" => ty.signed = false,
            "logic" | "bit" | "int" | "integer" | "longint" | "byte" | "shortint" | "time"
            | "real" | "shortreal" | "string" => {
                ty.kind = word;
                saw_decl_qualifier = true;
            }
            _ => {}
        }
    }
    if ty.kind == "other" && saw_net_or_reg {
        // Net kind is carried separately by ModuleGraphSignal.kind.  The
        // default four-state element type gives the explorer useful width
        // and signedness information for `wire [N:0]` and `reg` declarations.
        ty.kind = "logic".to_owned();
    }
    (ty, saw_decl_qualifier)
}

pub(super) fn graph_source_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for character in text.chars() {
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
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            in_string = true;
            escaped_string_character = false;
        } else if character == '\\' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            escaped_identifier = true;
        } else if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

pub(super) fn graph_width_from_source(parts: Option<&GraphSourceTypeParts>) -> Option<u32> {
    let parts = parts?;
    if parts.packed_dimensions.is_empty() {
        return None;
    }
    let mut width = 1u64;
    for dimension in &parts.packed_dimensions {
        let expression = dimension.strip_prefix('[')?.strip_suffix(']')?;
        let (high, low) = expression.split_once(':')?;
        let high = high.trim().parse::<i128>().ok()?;
        let low = low.trim().parse::<i128>().ok()?;
        let dimension_width = (high - low).unsigned_abs().checked_add(1)?;
        width = width.checked_mul(dimension_width as u64)?;
    }
    u32::try_from(width).ok()
}

pub(super) fn graph_declaration_location(
    file: &str,
    source: Option<&GraphSourceIndex>,
    node: &llg::ffi::surelog::ParseNode,
    name: &str,
) -> ModuleGraphLocation {
    let end_line = if node.end_line == 0 {
        node.line
    } else {
        node.end_line
    };
    let col = graph_lsp_column(source, node.line, node.col as u32, Some(name));
    let end_col = if end_line == node.line {
        col.saturating_add(lsp_name_len(name))
    } else {
        graph_lsp_column(source, end_line, node.end_col as u32, None)
    };
    ModuleGraphLocation {
        file: file.to_owned(),
        line: node.line,
        col,
        end_line,
        end_col,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphSourceTypeParts {
    pub(super) base: String,
    pub(super) packed_dimensions: Vec<String>,
    pub(super) unpacked_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphTypeDisplay {
    pub(super) text: Option<String>,
    pub(super) shape: ModuleGraphTypeShape,
}

/// Return the declaration's source type and dimension shape.  The source span
/// starts at the syntax declaration node and the tail is bounded at the
/// current declarator's first top-level initializer/separator.  This keeps a
/// declaration such as
///
/// ```text
/// logic [WIDTH-1:0]
///   mem [DEPTH-1:0];
/// ```
///
/// intact even though the identifier and its unpacked dimensions are on
/// different lines.  The result is captured during analysis; request code
/// only consumes the resulting owned string and shape.
pub(super) fn graph_type_prefix(
    source: Option<&GraphSourceIndex>,
    declaration: Option<&llg::ffi::surelog::ParseNode>,
    line: u32,
    col: u32,
    name: &str,
) -> Option<GraphSourceTypeParts> {
    let source = source?;
    let name_start = source_position_offset(source, line, col)?;
    let name_end = source_position_offset(
        source,
        line,
        col.saturating_add(name.chars().count() as u32),
    )
    .filter(|offset| *offset >= name_start)
    .unwrap_or_else(|| {
        name_start
            .saturating_add(name.len())
            .min(source.source.len())
    });

    let declaration_start = declaration
        .and_then(|node| source_position_offset(source, node.line, node.col as u32))
        .filter(|offset| *offset < name_start)
        .or_else(|| graph_source_declaration_start(source, name_start))
        .or_else(|| source_line_start(source, line))?;
    let (prefix_start, prefix_end) =
        graph_trim_range(&source.source, declaration_start, name_start);
    if prefix_start >= prefix_end {
        return None;
    }
    let (type_start, type_end) = graph_select_type_prefix_index(source, prefix_start, prefix_end);
    if type_start >= type_end {
        return None;
    }

    let packed_spans = graph_bracket_spans_index(source, type_start, type_end);
    let packed_dimensions = graph_bracket_dimensions_index(source, &packed_spans);
    let base = remove_bracket_dimensions_index(source, type_start, type_end, &packed_spans);

    let tail_end = graph_declaration_tail_end(source, name_end);
    let unpacked_end = graph_top_level_equals_index(source, name_end, tail_end).unwrap_or(tail_end);
    let unpacked_spans = graph_bracket_spans_index(source, name_end, unpacked_end);
    let unpacked_dimensions = graph_bracket_dimensions_index(source, &unpacked_spans);

    Some(GraphSourceTypeParts {
        base: normalize_graph_lexical_whitespace_masked(&base),
        packed_dimensions,
        unpacked_dimensions,
    })
}

/// Parse nodes for declarators commonly begin at the identifier rather than
/// at the declaration keyword.  Walk back to the nearest statement/header
/// boundary so the source type can span lines before that identifier.
pub(super) fn graph_source_declaration_start(
    source: &GraphSourceIndex,
    name_start: usize,
) -> Option<usize> {
    source.masked.get(..name_start)?;
    let mut low = 0;
    let mut high = source.declaration_boundary_events.len();
    while low < high {
        let middle = low + (high - low) / 2;
        if source.declaration_boundary_events[middle].0 <= name_start {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    source
        .declaration_boundary_events
        .get(low.saturating_sub(1))
        .map(|(_, boundary)| *boundary)
        .or(Some(0))
}

pub(super) fn graph_trim_range(source: &str, start: usize, end: usize) -> (usize, usize) {
    let text = &source[start..end];
    let trimmed_start = start + text.len() - text.trim_start().len();
    let trimmed = &source[trimmed_start..end];
    (trimmed_start, trimmed_start + trimmed.trim_end().len())
}

pub(super) fn graph_select_type_prefix_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> (usize, usize) {
    let (start, end) = graph_trim_range(&source.source, start, end);
    let (first_comma, last_comma) = graph_top_level_commas_index(source, start, end);
    let Some(last_comma) = last_comma else {
        return (start, end);
    };
    let local = graph_trim_range(&source.source, last_comma + 1, end);
    let (_, has_type_word) = graph_source_type_words_clean(&source.masked[local.0..local.1]);
    if has_type_word || source.masked[local.0..local.1].contains('[') {
        return local;
    }
    let first = graph_trim_range(&source.source, start, first_comma.unwrap_or(last_comma));
    graph_strip_trailing_declarator_name_index(source, first.0, first.1)
}

pub(super) fn graph_strip_trailing_declarator_name_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> (usize, usize) {
    let (start, end) = graph_trim_range(&source.source, start, end);
    let text = &source.source[start..end];
    let Some((separator, character)) = text
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
    else {
        return (start, end);
    };
    let token_start = separator + character.len_utf8();
    if token_start >= text.len() || !text[token_start..].chars().all(is_identifier_character) {
        return (start, end);
    }
    let (_, prefix_end) = graph_trim_range(&source.source, start, start + separator);
    (start, prefix_end)
}

pub(super) fn graph_top_level_commas_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> (Option<usize>, Option<usize>) {
    let state = source.delimiter_state_at(start);
    let Some(commas) = source.commas_by_state.get(&state) else {
        return (None, None);
    };
    let first_index = commas.partition_point(|offset| *offset < start);
    let end_index = commas.partition_point(|offset| *offset < end);
    if first_index >= end_index {
        return (None, None);
    }
    (Some(commas[first_index]), Some(commas[end_index - 1]))
}

pub(super) fn graph_bracket_spans_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut bracket_start = None;
    let mut depth = 0usize;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;
    for (offset, character) in source.masked[start..end].char_indices() {
        let index = start + offset;
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
        match character {
            '[' if depth == 0 => {
                bracket_start = Some(index);
                depth = 1;
            }
            '[' if depth > 0 => depth += 1,
            ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(bracket_start) = bracket_start.take() {
                        spans.push((bracket_start, index + 1));
                    }
                }
            }
            _ => {}
        }
    }
    spans
}

pub(super) fn graph_bracket_dimensions_index(
    source: &GraphSourceIndex,
    spans: &[(usize, usize)],
) -> Vec<String> {
    spans
        .iter()
        .map(|(start, end)| {
            format!(
                "[{}]",
                normalize_symbolic_expression_index(source, start + 1, end - 1)
            )
        })
        .collect()
}

pub(super) fn remove_bracket_dimensions_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
    spans: &[(usize, usize)],
) -> String {
    if spans.is_empty() {
        return source.masked[start..end].to_owned();
    }
    let mut result = String::new();
    let mut cursor = start;
    for (span_start, span_end) in spans {
        result.push_str(&source.masked[cursor..*span_start]);
        cursor = *span_end;
    }
    result.push_str(&source.masked[cursor..end]);
    result
}

pub(super) fn graph_comment_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut comment_start = None;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in text.char_indices() {
        if in_line_comment {
            if matches!(character, '\n' | '\r') {
                if let Some(start) = comment_start.take() {
                    ranges.push((start, index));
                }
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && text[index..].starts_with("*/") {
                if let Some(start) = comment_start.take() {
                    ranges.push((start, index + 2));
                }
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
            comment_start = Some(index);
            in_line_comment = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("/*") {
            comment_start = Some(index);
            in_block_comment = true;
            continue;
        }
    }

    if let Some(start) = comment_start {
        ranges.push((start, text.len()));
    }
    ranges
}

/// Forward-only membership cursor for the sorted, non-overlapping ranges
/// produced by [`graph_comment_ranges`].  Source graph scans are monotonic,
/// so advancing this cursor avoids a range walk for every source character.
#[derive(Debug, Clone, Copy)]
pub(super) struct GraphCommentCursor<'a> {
    ranges: &'a [(usize, usize)],
    next: usize,
}

impl<'a> GraphCommentCursor<'a> {
    fn new(ranges: &'a [(usize, usize)]) -> Self {
        Self { ranges, next: 0 }
    }

    fn at_offset(ranges: &'a [(usize, usize)], offset: usize) -> Self {
        let mut low = 0;
        let mut high = ranges.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if ranges[middle].1 <= offset {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Self { ranges, next: low }
    }

    fn contains(&mut self, offset: usize) -> bool {
        while self.next < self.ranges.len() && offset >= self.ranges[self.next].1 {
            self.next += 1;
        }
        self.ranges
            .get(self.next)
            .is_some_and(|(start, end)| *start <= offset && offset < *end)
    }
}

/// Replace comments with safe whitespace while preserving line terminators,
/// strings, escaped identifiers, and character-column positions.  The raw
/// comment ranges are still useful to bracket/structure scanners because
/// those scanners need offsets into the original source text.
#[cfg(test)]
pub(super) fn strip_hdl_comments(text: &str) -> String {
    let comment_ranges = graph_comment_ranges(text);
    let mut stripped = String::with_capacity(text.len());
    let mut comment_cursor = GraphCommentCursor::new(&comment_ranges);
    for (index, character) in text.char_indices() {
        if comment_cursor.contains(index) {
            if matches!(character, '\n' | '\r') {
                stripped.push(character);
            } else {
                stripped.push(' ');
            }
        } else {
            stripped.push(character);
        }
    }
    stripped
}

pub(super) fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
}

#[cfg(test)]
pub(super) fn graph_bracket_spans(text: &str) -> Vec<(usize, usize, String)> {
    let comment_ranges = graph_comment_ranges(text);
    let mut comment_cursor = GraphCommentCursor::new(&comment_ranges);
    let mut spans = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;
    for (index, character) in text.char_indices() {
        if comment_cursor.contains(index) {
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
                        spans.push((
                            start,
                            index + character.len_utf8(),
                            text[start + 1..index].to_owned(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    spans
}

#[cfg(test)]
pub(super) fn normalize_graph_type_display(text: &str) -> String {
    let spans = graph_bracket_spans(text);
    if spans.is_empty() {
        return normalize_graph_lexical_whitespace(text);
    }

    // Replace each active bracket span with a private sentinel while the
    // surrounding source whitespace/comments are normalized.  The raw span
    // expression is normalized separately, so a comment cannot disappear
    // into the surrounding whitespace pass and make two lexical tokens join.
    let sentinel = (0xe000..=0xf8ff)
        .filter_map(char::from_u32)
        .find(|character| !text.contains(*character))
        .unwrap_or('\u{fffc}');
    let mut placeholder_text = String::new();
    let mut replacements = Vec::with_capacity(spans.len());
    let mut cursor = 0;
    for (start, end, expression) in spans {
        placeholder_text.push_str(&text[cursor..start]);
        placeholder_text.push(sentinel);
        replacements.push(format!("[{}]", normalize_symbolic_expression(&expression)));
        cursor = end;
    }
    placeholder_text.push_str(&text[cursor..]);

    let normalized = normalize_graph_lexical_whitespace(&placeholder_text);
    let mut replacement = replacements.into_iter();
    normalized
        .chars()
        .fold(String::new(), |mut result, character| {
            if character == sentinel {
                if let Some(span) = replacement.next() {
                    result.push_str(&span);
                } else {
                    result.push(character);
                }
            } else {
                result.push(character);
            }
            result
        })
}

/// Collapse source whitespace outside strings and escaped identifiers while
/// keeping those lexical units intact.  Comments are treated as separators,
/// so their contents cannot cause an active token to be joined to a neighbor.
#[cfg(test)]
pub(super) fn normalize_graph_lexical_whitespace(text: &str) -> String {
    let comment_ranges = graph_comment_ranges(text);
    let mut comment_cursor = GraphCommentCursor::new(&comment_ranges);
    let mut normalized = String::new();
    let mut pending_space = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in text.char_indices() {
        if comment_cursor.contains(index) {
            pending_space = true;
            continue;
        }
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
        if escaped_identifier {
            if character.is_whitespace() {
                pending_space = true;
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
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;
        normalized.push(character);
        if character == '\\' {
            escaped_identifier = true;
        } else if character == '"' {
            in_string = true;
            escaped_string_character = false;
        }
    }
    normalized
}

/// Comment-free counterpart used with [`GraphSourceIndex::masked`].
pub(super) fn normalize_graph_lexical_whitespace_clean(text: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for character in text.chars() {
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
        if escaped_identifier {
            if character.is_whitespace() {
                pending_space = true;
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
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;
        normalized.push(character);
        if character == '\\' {
            escaped_identifier = true;
        } else if character == '"' {
            in_string = true;
            escaped_string_character = false;
        }
    }
    normalized
}

pub(super) fn normalize_graph_lexical_whitespace_masked(text: &str) -> String {
    normalize_graph_lexical_whitespace_clean(text)
}

pub(super) fn source_line_start(source: &GraphSourceIndex, line: u32) -> Option<usize> {
    source.line_start(line)
}

/// Convert a 1-based source line/character position into a UTF-8 byte offset.
/// Surelog's parse columns are character-based, while source slicing is
/// byte-based.
pub(super) fn source_position_offset(
    source: &GraphSourceIndex,
    line: u32,
    col: u32,
) -> Option<usize> {
    source.position_offset(line, col)
}

/// Convert a graph source column to a 1-based UTF-16 column for owned graph
/// locations.  Parse columns are not consistent across all Surelog paths when
/// non-ASCII text precedes a token, so an available name is used to choose the
/// scalar, UTF-16, or byte interpretation that actually starts that name.
pub(super) fn graph_lsp_column(
    source: Option<&GraphSourceIndex>,
    line: u32,
    col: u32,
    name: Option<&str>,
) -> u32 {
    let Some(source) = source else {
        return col;
    };
    let Some(line_info) = source.source_line(line) else {
        return col;
    };
    let Some(raw_character) = col
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
    else {
        return col;
    };
    let scalar = graph_line_character_offset(&source.source, line_info, raw_character);
    let utf16 = graph_line_utf16_offset(&source.source, line_info, raw_character);
    let byte = (raw_character <= line_info.end.saturating_sub(line_info.start))
        .then_some(line_info.start + raw_character);

    let offset = [utf16, scalar, byte]
        .into_iter()
        .flatten()
        .find(|offset| {
            name.is_some_and(|name| {
                source
                    .source
                    .get(*offset..)
                    .is_some_and(|tail| tail.starts_with(name))
            })
        })
        .or(scalar)
        .or(byte)
        .or(utf16);
    let Some(offset) = offset else {
        return col;
    };
    let Some(prefix) = source.source.get(line_info.start..offset) else {
        return col;
    };
    prefix.encode_utf16().count() as u32 + 1
}

pub(super) fn graph_line_utf16_offset(
    text: &str,
    line: &GraphLineMetadata,
    character: usize,
) -> Option<usize> {
    let line_text = text.get(line.start..line.end)?;
    let mut units = 0usize;
    for (offset, value) in line_text.char_indices() {
        if units == character {
            return Some(line.start + offset);
        }
        units += value.len_utf16();
        if units > character {
            return None;
        }
    }
    (units == character).then_some(line.end)
}

/// Find the end of the current declarator tail, including multiline unpacked
/// dimensions but excluding an initializer or the next declarator/port.
pub(super) fn graph_declaration_tail_end(source: &GraphSourceIndex, start: usize) -> usize {
    let state = source.delimiter_state_at(start);
    source
        .declaration_tail_events_by_state
        .get(&state)
        .and_then(|events| {
            let index = events.partition_point(|offset| *offset < start);
            events.get(index).copied()
        })
        .unwrap_or(source.source.len())
}

pub(super) fn graph_top_level_equals_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> Option<usize> {
    let state = source.delimiter_state_at(start);
    let equals = source.equals_by_state.get(&state)?;
    let index = equals.partition_point(|offset| *offset < start);
    equals.get(index).copied().filter(|offset| *offset < end)
}

#[cfg(test)]
pub(super) fn graph_bracket_dimensions(prefix: &str) -> Vec<String> {
    graph_bracket_spans(prefix)
        .into_iter()
        .map(|(_, _, expression)| format!("[{}]", normalize_symbolic_expression(&expression)))
        .collect()
}

/// Normalize only source spelling.  This is deliberately not a Verilog
/// evaluator: safe punctuation is compacted, while whitespace that separates
/// lexical tokens is retained.  In particular, an escaped identifier owns all
/// characters up to its terminating whitespace, so that separator must never
/// be discarded.
#[cfg(test)]
pub(super) fn normalize_symbolic_expression(expression: &str) -> String {
    let comment_ranges = graph_comment_ranges(expression);
    let mut comment_cursor = GraphCommentCursor::new(&comment_ranges);
    normalize_symbolic_expression_with_cursor(expression, 0, &mut comment_cursor)
}

pub(super) fn normalize_symbolic_expression_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> String {
    let expression = &source.source[start..end];
    let mut comment_cursor = source.comment_cursor_at(start);
    normalize_symbolic_expression_with_cursor(expression, start, &mut comment_cursor)
}

pub(super) fn normalize_symbolic_expression_with_cursor(
    expression: &str,
    base_offset: usize,
    comment_cursor: &mut GraphCommentCursor<'_>,
) -> String {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut normalized = String::new();
    let mut pending_space = false;
    let mut pending_comment = false;
    let mut pending_after_escaped_identifier = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, (byte_offset, character)) in expression.char_indices().enumerate() {
        if comment_cursor.contains(base_offset + byte_offset) {
            pending_space = true;
            pending_comment = true;
            continue;
        }
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
        // first whitespace.  Keep that terminating separator even when the
        // following token is punctuation (or the expression ends before the
        // enclosing `]`).
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
            if (pending_comment
                || should_retain_symbolic_separator(
                    &normalized,
                    &chars,
                    index,
                    pending_after_escaped_identifier,
                ))
                && !normalized.ends_with(' ')
            {
                normalized.push(' ');
            }
            pending_space = false;
            pending_comment = false;
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

    // The enclosing bracket is not part of `expression`; retain the required
    // terminator if an escaped identifier was the final token in the slice.
    if pending_space && pending_after_escaped_identifier && !normalized.ends_with(' ') {
        normalized.push(' ');
    }
    normalized
}

pub(super) fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

pub(super) fn should_retain_symbolic_separator(
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

    // `inside` is a keyword operator.  Keep its separator from the operands
    // and the set literal so the normalized form remains readable and agrees
    // with the client-side Module Contents normalizer.
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

    // A backslash starts a new escaped identifier.  Keep a separator before
    // it as well; otherwise a preceding identifier could be joined to the
    // escaped token in a way the source did not contain.
    if next == '\\' {
        return true;
    }

    // Removing a separator here could merge two source operators into a
    // different token (`+ +` → `++`, `/ *` → `/*`, `: :` → `::`, ...).
    if operator_pair_requires_separator(previous, next) {
        return true;
    }

    // Only compact boundaries made from the known SystemVerilog word and
    // punctuation alphabet.  An unfamiliar character gets the conservative
    // treatment and keeps its source separator.
    !is_known_symbolic_character(previous) || !is_known_symbolic_character(next)
}

pub(super) fn next_symbolic_word(chars: &[char], start: usize) -> Option<String> {
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

pub(super) fn is_dimension_keyword(word: &str) -> bool {
    word.eq_ignore_ascii_case("inside")
}

pub(super) fn is_known_symbolic_character(character: char) -> bool {
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

pub(super) fn operator_pair_requires_separator(previous: char, next: char) -> bool {
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

/// Canonical type text retained beside the structured [`TypeInfo`].  The
/// source graph keeps packed and unpacked dimensions in declaration order,
/// while the shape tells the explorer which suffix dimensions must remain
/// unpacked when a committed instance width replaces the packed portion.
// Keep the structured type, source spelling, and declaration coordinates
// explicit at this analysis boundary.
#[allow(clippy::too_many_arguments)]
pub(super) fn graph_type_display(
    source: Option<&GraphSourceIndex>,
    declaration: &llg::ffi::surelog::ParseNode,
    line: u32,
    col: u32,
    name: &str,
    ty: &TypeInfo,
    declaration_kind: &GraphDeclarationKind,
    source_parts: Option<&GraphSourceTypeParts>,
) -> GraphTypeDisplay {
    let recovered_parts = if source_parts.is_none() {
        source
            .and_then(|source| graph_type_prefix(Some(source), Some(declaration), line, col, name))
    } else {
        None
    };
    let parts = source_parts.or(recovered_parts.as_ref());
    let shape = parts
        .as_ref()
        .map_or_else(ModuleGraphTypeShape::default, |parts| {
            ModuleGraphTypeShape {
                packed_dimensions: parts.packed_dimensions.len(),
                unpacked_dimensions: parts.unpacked_dimensions.len(),
            }
        });
    let dimensions = parts.as_ref().map_or_else(Vec::new, |parts| {
        parts
            .packed_dimensions
            .iter()
            .chain(parts.unpacked_dimensions.iter())
            .cloned()
            .collect::<Vec<_>>()
    });
    let base = parts
        .as_ref()
        .and_then(|parts| graph_display_base(&parts.base))
        .and_then(|base| match declaration_kind {
            // Keep the established parameter `displayType` shape (`int`,
            // `struct pair_t`, ...) while the shared source filter below
            // preserves every non-direction port qualifier.
            GraphDeclarationKind::Parameter(_) => graph_parameter_display_base(&base),
            GraphDeclarationKind::Port(_) | GraphDeclarationKind::Signal(_) => Some(base),
        })
        .or_else(|| {
            (ty.kind != "other").then(|| {
                let mut base_ty = ty.clone();
                base_ty.width = None;
                base_ty.render()
            })
        });
    let text = if let Some(base) = base.filter(|base| !base.is_empty()) {
        if dimensions.is_empty() {
            Some(base)
        } else {
            Some(format!("{base} {}", dimensions.join(" ")))
        }
    } else {
        None
    };
    GraphTypeDisplay { text, shape }
}

pub(super) fn graph_display_base(text: &str) -> Option<String> {
    let normalized = normalize_graph_lexical_whitespace_clean(text);
    let base = normalized
        .split_whitespace()
        .filter(|word| {
            !matches!(
                word.to_ascii_lowercase().as_str(),
                "input" | "output" | "inout"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    (!base.is_empty()).then_some(base)
}

pub(super) fn graph_parameter_display_base(text: &str) -> Option<String> {
    let normalized = normalize_graph_lexical_whitespace_clean(text);
    let base = normalized
        .split_whitespace()
        .filter(|word| {
            !matches!(
                word.to_ascii_lowercase().as_str(),
                "parameter" | "localparam"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    (!base.is_empty()).then_some(base)
}

pub(super) fn graph_declaration_detail(
    source: Option<&GraphSourceIndex>,
    line: u32,
    col: u32,
    name: &str,
    ty: &TypeInfo,
    kind: &GraphDeclarationKind,
) -> Option<String> {
    let fallback = match kind {
        GraphDeclarationKind::Port(direction) => {
            format!(
                "{} {} {}",
                graph_direction_text(*direction),
                ty.render(),
                name
            )
        }
        GraphDeclarationKind::Parameter(local) => format!(
            "{} {} {}",
            if *local { "localparam" } else { "parameter" },
            ty.render(),
            name
        ),
        GraphDeclarationKind::Signal(signal_kind) => {
            format!("{} {} {}", signal_kind, ty.render(), name)
        }
    };
    let Some(source) = source else {
        return Some(fallback);
    };
    let Some(line_info) = source.stripped_line(line) else {
        return Some(fallback);
    };
    let line_text = &source.stripped[line_info.start..line_info.end];
    if line_text.trim().is_empty() {
        return Some(fallback);
    }

    // Keep only the declaration clause containing this identifier.  In an
    // ANSI header, using the entire source line would turn
    // `module m #(parameter int W)(input logic clk)` into a parameter detail
    // that also contains the port declaration.  The identifier column gives
    // us a safe split even when names repeat elsewhere on the line.
    let name_start = source
        .stripped_position_offset(line, col)
        .unwrap_or(line_info.end)
        .min(line_info.end);
    let separator_before = line_info
        .clause_start_offsets
        .partition_point(|(offset, _)| *offset < name_start);
    let clause_start = separator_before
        .checked_sub(1)
        .and_then(|index| line_info.clause_start_offsets.get(index))
        .map_or(line_info.start, |(offset, character)| {
            offset + character.len_utf8()
        })
        .saturating_sub(line_info.start);
    let name_end = source
        .stripped_position_offset(line, col.saturating_add(name.chars().count() as u32))
        .filter(|offset| *offset >= name_start)
        .unwrap_or_else(|| name_start.saturating_add(name.len()).min(line_info.end));
    let separator_after = line_info
        .clause_end_offsets
        .partition_point(|offset| *offset < name_end);
    let clause_end = line_info
        .clause_end_offsets
        .get(separator_after)
        .map_or(line_info.end, |offset| *offset)
        .saturating_sub(line_info.start);
    let candidate = line_text[clause_start..clause_end].trim();
    // A later declarator in `logic a, b;` has only `b` in its local clause;
    // do not mistake that identifier-only fragment for a complete type. Use
    // the typed fallback, which was recovered from the declaration prefix,
    // unless the candidate itself contains a recognized declaration word.
    Some(
        if candidate.contains(name) && graph_source_type_words_clean(candidate).1 {
            candidate.to_owned()
        } else {
            fallback
        },
    )
}

pub(super) fn graph_direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "port",
    }
}

pub(super) fn graph_instantiation_type(
    nodes: &[llg::ffi::surelog::ParseNode],
    root: usize,
    file_id: u32,
    path: &str,
    token_types: &HashMap<&str, HashMap<(u32, u32), i32>>,
) -> Option<String> {
    use llg::core::vobject_types::VObjectType;
    use llg::ffi::vpi;
    graph_subtree_indices(nodes, root)
        .into_iter()
        .filter(|index| {
            let node = &nodes[*index];
            node.file_id == file_id
                && node.type_id == VObjectType::slStringConst as u16
                && !graph_has_ancestor(nodes, *index, |ty| ty == VObjectType::paName_of_instance)
        })
        .filter_map(|index| {
            let node = &nodes[index];
            (token_types
                .get(path)
                .and_then(|types| types.get(&(node.line, node.col as u32)))
                == Some(&vpi::uhdmclass_defn))
            .then(|| {
                node.symbol_name
                    .clone()
                    .map(|name| (graph_position(node, index), name))
            })
            .flatten()
        })
        .min_by_key(|(position, _)| *position)
        .map(|(_, name)| name)
        .filter(|name| !name.is_empty())
}

pub(super) fn graph_generate_ancestors(
    nodes: &[llg::ffi::surelog::ParseNode],
    start: usize,
) -> Vec<ModuleGraphGenerateScope> {
    let mut scopes = graph_ancestors(nodes, start)
        .into_iter()
        .filter_map(|index| {
            let ty = graph_node_type(nodes, nodes[index].type_id)?;
            if !graph_is_generate_scope(ty) {
                return None;
            }
            let name = graph_generate_name(nodes, index, ty);
            Some(ModuleGraphGenerateScope {
                name,
                file: None,
                line: nodes[index].line,
                col: nodes[index].col as u32,
                children: Vec::new(),
                nested: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    // `graph_ancestors` starts at the instance and walks inward; source tree
    // insertion expects outer → inner.  The bounded ancestor walk already
    // stops at the module declaration (or a malformed/cyclic parent chain).
    scopes.reverse();
    scopes
}

pub(super) fn graph_is_generate_scope(ty: llg::core::vobject_types::VObjectType) -> bool {
    use llg::core::vobject_types::VObjectType;
    matches!(
        ty,
        VObjectType::paGenerate_begin_end_block
            | VObjectType::paGenerate_interface_block
            | VObjectType::paGenerate_interface_conditional_statement
            | VObjectType::paGenerate_interface_loop_statement
            | VObjectType::paGenerate_interface_named_block
            | VObjectType::paGenerate_module_block
            | VObjectType::paGenerate_module_conditional_statement
            | VObjectType::paGenerate_module_loop_statement
            | VObjectType::paGenerate_module_named_block
    )
}

pub(super) fn graph_generate_name(
    nodes: &[llg::ffi::surelog::ParseNode],
    root: usize,
    ty: llg::core::vobject_types::VObjectType,
) -> String {
    use llg::core::vobject_types::VObjectType;
    let named = matches!(
        ty,
        VObjectType::paGenerate_begin_end_block
            | VObjectType::paGenerate_interface_named_block
            | VObjectType::paGenerate_module_named_block
    );
    if named {
        if let Some((_, name)) = graph_first_string(nodes, root, nodes[root].file_id) {
            return name;
        }
    }
    format!("generate@{}:{}", nodes[root].line, nodes[root].col)
}

pub(super) fn graph_instance_key(instance: &ModuleGraphInstance) -> GraphInstanceKey {
    (
        instance.name.clone(),
        instance.module_type.clone(),
        instance.file.clone(),
        instance.line,
        instance.col,
    )
}

pub(super) fn graph_push_instance(
    target: &mut Vec<ModuleGraphInstance>,
    keys: &mut HashSet<GraphInstanceKey>,
    instance: ModuleGraphInstance,
) {
    if keys.insert(graph_instance_key(&instance)) {
        target.push(instance);
    }
}

pub(super) fn graph_scope_key(scope: &ModuleGraphGenerateScope) -> GraphScopeKey {
    (scope.name.clone(), scope.line, scope.col)
}

pub(super) fn graph_scope_indices(
    indexes: &HashMap<Vec<GraphScopeKey>, usize>,
    path: &[GraphScopeKey],
) -> Option<Vec<usize>> {
    let mut result = Vec::with_capacity(path.len());
    for depth in 1..=path.len() {
        result.push(*indexes.get(&path[..depth].to_vec())?);
    }
    Some(result)
}

pub(super) fn graph_generated_scope_mut<'a>(
    scopes: &'a mut [ModuleGraphGenerateScope],
    indices: &[usize],
) -> Option<&'a mut ModuleGraphGenerateScope> {
    let (index, rest) = indices.split_first()?;
    let scope = scopes.get_mut(*index)?;
    if rest.is_empty() {
        Some(scope)
    } else {
        graph_generated_scope_mut(&mut scope.nested, rest)
    }
}

pub(super) fn graph_push_generated_instance(
    definition_index: usize,
    definition: &mut ModuleGraphDefinition,
    path: &[ModuleGraphGenerateScope],
    instance: ModuleGraphInstance,
    indexes: &mut GraphAssemblyIndexes,
) {
    if path.is_empty() {
        return;
    }

    let path_keys = path.iter().map(graph_scope_key).collect::<Vec<_>>();
    for depth in 0..path.len() {
        let prefix = path_keys[..=depth].to_vec();
        if indexes.generated_scopes[definition_index].contains_key(&prefix) {
            continue;
        }
        let mut scope = path[depth].clone();
        scope.children.clear();
        scope.nested.clear();
        let scope_index = if depth == 0 {
            definition.generated_scopes.push(scope);
            definition.generated_scopes.len() - 1
        } else {
            let parent_indices = graph_scope_indices(
                &indexes.generated_scopes[definition_index],
                &path_keys[..depth],
            )
            .expect("generated scope parent is inserted before its child");
            let parent =
                graph_generated_scope_mut(&mut definition.generated_scopes, &parent_indices)
                    .expect("generated scope parent index remains valid");
            parent.nested.push(scope);
            parent.nested.len() - 1
        };
        indexes.generated_scopes[definition_index].insert(prefix, scope_index);
    }

    let instance_key = graph_instance_key(&instance);
    let is_new = indexes.generated_children[definition_index]
        .entry(path_keys.clone())
        .or_default()
        .insert(instance_key);
    if !is_new {
        return;
    }
    let scope_indices =
        graph_scope_indices(&indexes.generated_scopes[definition_index], &path_keys)
            .expect("generated scope path is inserted before its children");
    if let Some(scope) = graph_generated_scope_mut(&mut definition.generated_scopes, &scope_indices)
    {
        scope.children.push(instance);
    }
}

pub(super) fn graph_instance_cmp(
    left: &ModuleGraphInstance,
    right: &ModuleGraphInstance,
) -> std::cmp::Ordering {
    (
        left.name.as_str(),
        left.module_type.as_str(),
        left.file.as_deref().unwrap_or_default(),
        left.line,
        left.col,
    )
        .cmp(&(
            right.name.as_str(),
            right.module_type.as_str(),
            right.file.as_deref().unwrap_or_default(),
            right.line,
            right.col,
        ))
}

pub(super) fn graph_sort_scopes(scopes: &mut [ModuleGraphGenerateScope]) {
    scopes.sort_by(|left, right| {
        (left.name.as_str(), left.line, left.col).cmp(&(right.name.as_str(), right.line, right.col))
    });
    for scope in scopes {
        scope.children.sort_by(graph_instance_cmp);
        graph_sort_scopes(&mut scope.nested);
    }
}
