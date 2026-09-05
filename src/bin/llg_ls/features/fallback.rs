//! Declaration-level feature extraction for projects without usable UHDM.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnKind {
    /// A named port connection: `.clk(clk)` inside `m u0(...)`.
    Port,
    /// A named parameter override: `.W(4)` inside `m u0 #(.W(4)) (...)`.
    Param,
}

/// One named connection discovered in the parse tree: `.clk(wa)` (port) or
/// `.W(expr)` (parameter override) inside `m u0(...)`.
///
/// Positions are **1-based** (`ParseNode` convention).  `actual` is the
/// first identifier leaf of the connection's expression, when the connection
/// carries one; `inst_type` is the module type name of the enclosing
/// instantiation, when one is resolvable through transparent wrapper nodes.
#[derive(Debug, Clone)]
pub(crate) struct NamedPortConn {
    /// Absolute path of the file holding the connection.
    pub file: String,
    /// `(line, col)` of the label identifier (`.clk` / `.W`).
    pub label: (u32, u32),
    /// The label text — the child port/parameter name being connected.
    pub label_name: String,
    /// `(line, col)` of the connected expression's leading identifier.
    pub actual: Option<(u32, u32)>,
    /// The connected expression's leading identifier text — the parent-scope
    /// object whose declaration the ACTUAL resolves to.
    pub actual_name: Option<String>,
    /// Module type name of the enclosing instantiation (`m` in `m u0(...)`).
    pub inst_type: Option<String>,
    /// Whether the label names a child PORT ([`ConnKind::Port`]) or a child
    /// PARAMETER override ([`ConnKind::Param`]).
    pub kind: ConnKind,
}

/// A parsed enum constant declaration.  Unlike the elaborated package model,
/// this retains the source scope for class members and for syntax-broken
/// files, where no UHDM object exists to carry the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParseEnumDecl {
    pub(super) name: String,
    pub(super) file: String,
    pub(super) line1: u32,
    pub(super) col1: u32,
    pub(super) scope: Option<String>,
}

/// Parse-backed enum references and the exact token positions at which they
/// occur.  Surelog's VPI elaboration folds package/class enum expressions
/// into constants, so these facts are collected from the surviving parse tree
/// and merged with the normal VPI binding map.
#[derive(Debug, Clone, Default)]
pub(super) struct ParseEnumFacts {
    pub(super) declarations: Vec<ParseEnumDecl>,
    pub(super) bindings: RefBindings,
    pub(super) reference_positions: HashSet<(String, u32, u32)>,
    pub(super) unresolved_positions: HashSet<(String, u32, u32)>,
    pub(super) synthetic_tokens: Vec<VObjectInfo>,
}

/// Parse-tree-derived named-connection inputs for
/// [`Analysis::new_with_outcome`].
///
/// * `parse_decls` seeds the symbol index in the syntax-broken fallback
///   path (`None` for elaborated analyses — see
///   [`SymbolIndex::from_parts`]).
/// * `pairs` is the label↔actual pairing scanned from the surviving parse
///   tree (named PORT connections and named PARAMETER overrides); in UHDM
///   mode it pairs resolved label targets with the actual positions
///   (actual → parent-scope declaration), and in fallback mode it drives
///   the resolution below.
/// * `fallback_bindings` holds the connection bindings RESOLVED against the
///   parse tree's recorded declarations (label → child module's port or
///   parameter declaration, actual → parent-scope declaration); they exist
///   only when no UHDM design was produced, where UHDM bindings are empty by
///   construction.
/// * The `parse_enum_*` fields carry exact package/class/import enum facts.
///   Their unresolved-position set is deliberately separate from the binding
///   map so ambiguity suppresses the broad name-based fallback as well.
#[derive(Default)]
pub(crate) struct ConnectionInputs {
    pub parse_decls: Option<ParseDeclPositions>,
    pub pairs: Vec<NamedPortConn>,
    pub fallback_bindings: RefBindings,
    pub parse_enum_decls: Vec<ParseEnumDecl>,
    pub parse_enum_bindings: RefBindings,
    pub parse_enum_ref_positions: HashSet<(String, u32, u32)>,
    pub unresolved_enum_refs: HashSet<(String, u32, u32)>,
    pub parse_enum_tokens: Vec<VObjectInfo>,
}

pub(super) fn parse_tree_feature_parts(
    design: &llg::ffi::surelog::Design,
    conn_pairs: &[NamedPortConn],
    enum_facts: &ParseEnumFacts,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) -> (DesignModel, Vec<FileTokens>, ConnectionInputs) {
    use std::collections::{HashMap, HashSet};

    let mut token_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.parse_fallback.tokens",
        || root.to_owned(),
        generation,
        design.file_content_count() as usize,
        parent_id,
    );
    let token_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=analysis.parse_fallback.tokens.begin root={} generation={} files={}",
        root,
        generation,
        design.file_content_count()
    );
    let (tokens, parse_decls) = tokens::collect_parse_tokens(design);
    let token_count = token_cardinality(&tokens);
    token_span.complete("ok", token_count);
    crate::llg_debug!(
        "event=analysis.parse_fallback.tokens.end outcome=ok root={} generation={} token_files={} token_nodes={} declarations={} elapsed_us={}",
        root,
        generation,
        tokens.len(),
        token_count,
        parse_decls.len(),
        token_started.elapsed().as_micros()
    );
    drop(token_span);

    // Modules-only model from the surviving tokens: every `vpiModule`-typed
    // token IS a module-name identifier under a module header (the classifier
    // emits that type only for header ancestors).  Positions are 1-based,
    // matching the model convention; Surelog's builtin classes live in a
    // virtual `builtin.sv` and are skipped like everywhere else.
    let mut model_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.parse_fallback.model",
        || root.to_owned(),
        generation,
        tokens.len(),
        parent_id,
    );
    let model_started = std::time::Instant::now();
    let mut modules: Vec<ModuleDef> = Vec::new();
    let mut seen: HashSet<(String, u32, u32)> = HashSet::new();
    for ft in &tokens {
        if builtin_file(&ft.path) {
            continue;
        }
        for node in &ft.nodes {
            if node.vpi_type != llg::ffi::vpi::vpiModule {
                continue;
            }
            let Some(name) = node.name.as_deref().filter(|n| !n.is_empty()) else {
                continue;
            };
            if !seen.insert((ft.path.clone(), node.line, node.col)) {
                continue;
            }
            // The token spans the identifier only; keep a sane end position
            // even if the parse node's end fields are unset or degenerate.
            let len = lsp_name_len(name);
            let name_end_col = node.col.saturating_add(len);
            let (end_line, end_col) = if node.end_line > node.line
                || (node.end_line == node.line && node.end_col >= name_end_col)
            {
                (node.end_line, node.end_col)
            } else {
                (node.line, name_end_col)
            };
            modules.push(ModuleDef {
                name: name.to_owned(),
                file: Some(ft.path.clone()),
                line: node.line,
                col: node.col,
                end_line,
                end_col,
            });
        }
    }
    model_span.complete("ok", modules.len());
    crate::llg_debug!(
        "event=analysis.parse_fallback.model.end outcome=ok root={} generation={} modules={} elapsed_us={}",
        root,
        generation,
        modules.len(),
        model_started.elapsed().as_micros()
    );
    drop(model_span);

    // ── Fallback connection bindings ────────────────────────────────────
    // Resolve every parsed named connection: the LABEL binds to the child
    // module's recorded declaration matched by label name (exact match —
    // ports against `declared_ports_by_module`, parameter overrides against
    // `declared_params_by_module`); the ACTUAL binds to the declaration of
    // the actual identifier in the instantiating (parent) scope — selected
    // with the shared containment rule over the parsed module spans
    // (`select_parent_scope_position`).  Connections whose instantiation
    // type or label cannot be resolved are skipped silently; positional
    // matching for unnamed connections stays out of scope.
    let mut binding_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.parse_fallback.bindings",
        || root.to_owned(),
        generation,
        conn_pairs.len(),
        parent_id,
    );
    let binding_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=analysis.parse_fallback.bindings.begin root={} generation={} connections={}",
        root,
        generation,
        conn_pairs.len()
    );
    let fallback_index = ParseFallbackIndex::new(&tokens, &parse_decls, &modules);
    let ports = declared_ports_by_module(&fallback_index);
    let params = declared_params_by_module(&fallback_index);
    let mut fallback_bindings: RefBindings = HashMap::new();
    for pair in conn_pairs {
        let Some(inst_type) = pair.inst_type.as_deref() else {
            continue;
        };
        let child_decl = match pair.kind {
            ConnKind::Port => find_fallback_port(ports, inst_type, &pair.label_name)
                .map(|p| ("port", p.file.clone(), p.line1, p.col1)),
            ConnKind::Param => find_fallback_param(params, inst_type, &pair.label_name)
                .map(|p| ("parameter", p.file.clone(), p.line1, p.col1)),
        };
        let Some((kind_label, decl_file, decl_line1, decl_col1)) = child_decl else {
            continue;
        };
        let target = |via_label: bool, via_connection: bool| DeclTarget {
            name: pair.label_name.clone(),
            kind: kind_label.to_owned(),
            file: decl_file.clone(),
            line0: decl_line1.saturating_sub(1),
            col0: decl_col1.saturating_sub(1),
            via_label,
            via_connection,
        };
        fallback_bindings.insert(
            (
                pair.file.clone(),
                pair.label.0.saturating_sub(1),
                pair.label.1.saturating_sub(1),
            ),
            target(true, false),
        );
        if let (Some((actual_line, actual_col)), Some(actual_name)) =
            (pair.actual, pair.actual_name.as_deref())
        {
            let actual_target = fallback_actual_target(
                &fallback_index,
                &pair.file,
                pair.label.0.saturating_sub(1),
                actual_name,
            );
            if let Some(actual_target) = actual_target {
                fallback_bindings.insert(
                    (
                        pair.file.clone(),
                        actual_line.saturating_sub(1),
                        actual_col.saturating_sub(1),
                    ),
                    actual_target,
                );
            }
        }
    }
    binding_span.complete("ok", fallback_bindings.len());
    crate::llg_debug!(
        "event=analysis.parse_fallback.bindings.end outcome=ok root={} generation={} bindings={} elapsed_us={}",
        root,
        generation,
        fallback_bindings.len(),
        binding_started.elapsed().as_micros()
    );
    drop(binding_span);

    (
        DesignModel {
            design_name: String::new(),
            top_instances: Vec::new(),
            modules,
            packages: Vec::new(),
            classes: Vec::new(),
        },
        tokens,
        ConnectionInputs {
            parse_decls: Some(parse_decls),
            pairs: conn_pairs.to_vec(),
            fallback_bindings,
            parse_enum_decls: enum_facts.declarations.clone(),
            parse_enum_bindings: enum_facts.bindings.clone(),
            parse_enum_ref_positions: enum_facts.reference_positions.clone(),
            unresolved_enum_refs: enum_facts.unresolved_positions.clone(),
            parse_enum_tokens: enum_facts.synthetic_tokens.clone(),
        },
    )
}

/// Resolve a connection ACTUAL to its parent-scope declaration in the
/// parse-fallback mode and build its binding target.
///
/// Candidates are looked up by actual identifier name in the per-file
/// [`ParseFallbackIndex`].  The shared containment rule picks the winner using
/// the parsed module body spans, with the label line as the instantiation-line
/// proxy.  The target kind is derived from the parse-node type at the chosen
/// position (ports / parameters / nets / variables).
pub(super) fn fallback_actual_target(
    fallback_index: &ParseFallbackIndex<'_>,
    inst_file: &str,
    label_line0: u32,
    actual_name: &str,
) -> Option<DeclTarget> {
    let file_index = fallback_index.files.get(inst_file)?;
    let positions = file_index.actual_positions_by_name.get(actual_name)?;
    let (line0, col0) =
        select_parent_scope_position_sorted(positions, &file_index.spans, label_line0)?;
    let position = file_index.positions.get(&(line0 + 1, col0 + 1))?;
    // Several classifier tokens can legitimately share one source position.
    // The old fallback selected the node whose name matched the actual, so
    // retain that detail while restricting the search to this one position.
    let node_index = std::iter::once(position.first_node)
        .chain(position.additional_nodes.iter().flatten().copied())
        .find(|index| file_index.nodes[*index].name.as_deref() == Some(actual_name))?;
    let node = &file_index.nodes[node_index];
    Some(DeclTarget {
        name: actual_name.to_owned(),
        kind: fallback_decl_kind(node.vpi_type).to_owned(),
        file: inst_file.to_owned(),
        line0,
        col0,
        via_label: false,
        via_connection: true,
    })
}

/// Human-readable kind of a parse-tree declaration position for ACTUAL
/// binding targets.  Parse-tree port declarations classify as `vpiPort`
/// (see [`declared_ports_by_module`]); the direction-specific types exist
/// only in elaborated analyses but are accepted for robustness.
pub(super) fn fallback_decl_kind(vpi_type: i32) -> &'static str {
    use llg::ffi::vpi;
    match vpi_type {
        vpi::vpiPort | vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT => {
            "port"
        }
        vpi::vpiParameter | vpi::vpiSpecParam => "parameter",
        t if is_net_type(t) => "net",
        _ => "var",
    }
}

/// One parse-tree PORT declaration attributed to its enclosing module header.
pub(super) struct FallbackPortDecl {
    /// Name of the enclosing module (library prefixes already stripped).
    module: String,
    /// Port name at the declaration position.
    name: String,
    /// Absolute file path of the declaration.
    file: String,
    /// 1-based declaration line.
    pub(super) line1: u32,
    /// 1-based declaration column.
    pub(super) col1: u32,
}

/// One parse-tree PARAMETER declaration attributed to its enclosing module
/// header.
pub(super) struct FallbackParamDecl {
    /// Name of the enclosing module (library prefixes already stripped).
    module: String,
    /// Parameter name at the declaration position.
    name: String,
    /// Absolute file path of the declaration.
    file: String,
    /// 1-based declaration line.
    pub(super) line1: u32,
    /// 1-based declaration column.
    pub(super) col1: u32,
}

#[derive(Debug)]
pub(super) struct ParseFallbackPositionInfo {
    first_node: usize,
    port_node: Option<usize>,
    parameter_node: Option<usize>,
    additional_nodes: Option<Vec<usize>>,
}

pub(super) struct ParseFallbackFileIndex<'a> {
    nodes: &'a [VObjectInfo],
    positions: HashMap<(u32, u32), ParseFallbackPositionInfo>,
    pub(super) actual_positions_by_name: HashMap<String, Vec<ActualCandidatePos>>,
    headers: Vec<(u32, String)>,
    spans: Vec<ModuleSpan0>,
}

pub(super) struct ParseFallbackIndex<'a> {
    pub(super) files: HashMap<&'a str, ParseFallbackFileIndex<'a>>,
    ports: Vec<FallbackPortDecl>,
    params: Vec<FallbackParamDecl>,
}

pub(super) fn fallback_port_type(vpi_type: i32) -> bool {
    matches!(
        vpi_type,
        llg::ffi::vpi::vpiPort
            | llg::ffi::vpi::TOKEN_PORT_INPUT
            | llg::ffi::vpi::TOKEN_PORT_OUTPUT
            | llg::ffi::vpi::TOKEN_PORT_INOUT
    )
}

impl<'a> ParseFallbackIndex<'a> {
    /// Build all parse-fallback position, name, header, and span indexes in
    /// linear passes over the already-owned token/declaration data.  The
    /// resulting maps are reused for every named connection in the fallback
    /// analysis; no connection performs a declaration-by-node scan.
    pub(super) fn new(
        tokens: &'a [FileTokens],
        parse_decls: &ParseDeclPositions,
        modules: &[ModuleDef],
    ) -> Self {
        let mut files: HashMap<&'a str, ParseFallbackFileIndex<'a>> = HashMap::new();
        for file_tokens in tokens {
            let mut positions = HashMap::with_capacity(file_tokens.nodes.len());
            for (node_index, node) in file_tokens.nodes.iter().enumerate() {
                let entry =
                    positions
                        .entry((node.line, node.col))
                        .or_insert(ParseFallbackPositionInfo {
                            first_node: node_index,
                            port_node: None,
                            parameter_node: None,
                            additional_nodes: None,
                        });
                if entry.first_node != node_index {
                    entry
                        .additional_nodes
                        .get_or_insert_with(Vec::new)
                        .push(node_index);
                }
                if entry.port_node.is_none() && fallback_port_type(node.vpi_type) {
                    entry.port_node = Some(node_index);
                }
                if entry.parameter_node.is_none() && node.vpi_type == llg::ffi::vpi::vpiParameter {
                    entry.parameter_node = Some(node_index);
                }
            }
            files.insert(
                file_tokens.path.as_str(),
                ParseFallbackFileIndex {
                    nodes: file_tokens.nodes.as_slice(),
                    positions,
                    actual_positions_by_name: HashMap::new(),
                    headers: Vec::new(),
                    spans: Vec::new(),
                },
            );
        }

        for module in modules {
            let Some(file) = module.file.as_deref() else {
                continue;
            };
            let Some(file_index) = files.get_mut(file) else {
                continue;
            };
            file_index
                .headers
                .push((module.line, clean_name(&module.name).to_owned()));
            // Parsed module spans always carry a computed end line.  Keep
            // the same inclusive coordinates used by the former fallback.
            file_index.spans.push(ModuleSpan0 {
                first0: module.line.saturating_sub(1),
                last0: Some(module.end_line.saturating_sub(1)),
            });
        }
        for file_index in files.values_mut() {
            file_index.headers.sort_by_key(|(line, _)| *line);
        }

        // HashSet iteration is intentionally unordered.  Sort each file's
        // declaration positions once so header attribution and all later
        // nearest-scope lookups are deterministic and linear/binary-searchable.
        let mut declarations_by_file: HashMap<&str, Vec<(u32, u32)>> = HashMap::new();
        for (file, line1, col1) in parse_decls {
            declarations_by_file
                .entry(file.as_str())
                .or_default()
                .push((*line1, *col1));
        }

        let mut ports = Vec::new();
        let mut params = Vec::new();
        for (file, mut declarations) in declarations_by_file {
            declarations.sort_unstable();
            let Some(file_index) = files.get_mut(file) else {
                continue;
            };
            let nodes = file_index.nodes;
            let positions = &file_index.positions;
            let headers = &file_index.headers;
            let actual_positions_by_name = &mut file_index.actual_positions_by_name;
            let mut next_header = 0usize;
            let mut current_header = None;

            for (line1, col1) in declarations {
                while next_header < headers.len() && headers[next_header].0 <= line1 {
                    current_header = Some(next_header);
                    next_header += 1;
                }
                let Some(position) = positions.get(&(line1, col1)) else {
                    continue;
                };
                let actual_position = (line1.saturating_sub(1), col1.saturating_sub(1));
                let node_indices = std::iter::once(position.first_node)
                    .chain(position.additional_nodes.iter().flatten().copied());
                for node_index in node_indices {
                    let Some(actual_name) =
                        nodes[node_index].name.as_deref().filter(|n| !n.is_empty())
                    else {
                        continue;
                    };
                    if let Some(entry) = actual_positions_by_name.get_mut(actual_name) {
                        if entry.last().copied() != Some(actual_position) {
                            entry.push(actual_position);
                        }
                    } else {
                        actual_positions_by_name
                            .insert(actual_name.to_owned(), vec![actual_position]);
                    }
                }
                let Some(header_index) = current_header else {
                    continue;
                };
                let module = headers[header_index].1.as_str();

                if let Some(node_index) = position.port_node {
                    let node = &nodes[node_index];
                    if let Some(name) = node.name.as_deref().filter(|n| !n.is_empty()) {
                        ports.push(FallbackPortDecl {
                            module: module.to_owned(),
                            name: name.to_owned(),
                            file: file.to_owned(),
                            line1,
                            col1,
                        });
                    }
                }
                if let Some(node_index) = position.parameter_node {
                    let node = &nodes[node_index];
                    if let Some(name) = node.name.as_deref().filter(|n| !n.is_empty()) {
                        params.push(FallbackParamDecl {
                            module: module.to_owned(),
                            name: name.to_owned(),
                            file: file.to_owned(),
                            line1,
                            col1,
                        });
                    }
                }
            }
        }

        for positions in files
            .values_mut()
            .flat_map(|file_index| file_index.actual_positions_by_name.values_mut())
        {
            positions.sort_unstable();
        }
        ports.sort_by(|a, b| {
            (&a.module, &a.name, &a.file, a.line1, a.col1)
                .cmp(&(&b.module, &b.name, &b.file, b.line1, b.col1))
        });
        params.sort_by(|a, b| {
            (&a.module, &a.name, &a.file, a.line1, a.col1)
                .cmp(&(&b.module, &b.name, &b.file, b.line1, b.col1))
        });
        Self {
            files,
            ports,
            params,
        }
    }
}

/// Collect the direction-typed port declaration positions recorded by
/// [`collect_parse_tokens`], attributing each to its enclosing module.  The
/// expensive attribution pass is performed once by [`ParseFallbackIndex`].
pub(super) fn declared_ports_by_module<'a>(
    index: &'a ParseFallbackIndex<'_>,
) -> &'a [FallbackPortDecl] {
    &index.ports
}

/// Collect the parameter declaration positions recorded by
/// [`collect_parse_tokens`], attributing each to its enclosing module.  The
/// positions and type checks are pre-indexed by [`ParseFallbackIndex`].
pub(super) fn declared_params_by_module<'a>(
    index: &'a ParseFallbackIndex<'_>,
) -> &'a [FallbackParamDecl] {
    &index.params
}

pub(super) fn find_fallback_port<'a>(
    declarations: &'a [FallbackPortDecl],
    module: &str,
    name: &str,
) -> Option<&'a FallbackPortDecl> {
    let mut low = 0;
    let mut high = declarations.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let ordering = declarations[middle]
            .module
            .as_str()
            .cmp(module)
            .then_with(|| declarations[middle].name.as_str().cmp(name));
        if ordering == std::cmp::Ordering::Less {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    declarations
        .get(low)
        .filter(|declaration| declaration.module == module && declaration.name == name)
}

pub(super) fn find_fallback_param<'a>(
    declarations: &'a [FallbackParamDecl],
    module: &str,
    name: &str,
) -> Option<&'a FallbackParamDecl> {
    let mut low = 0;
    let mut high = declarations.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let ordering = declarations[middle]
            .module
            .as_str()
            .cmp(module)
            .then_with(|| declarations[middle].name.as_str().cmp(name));
        if ordering == std::cmp::Ordering::Less {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    declarations
        .get(low)
        .filter(|declaration| declaration.module == module && declaration.name == name)
}
/// Scan Surelog's parse tree for named connections and return one
/// [`NamedPortConn`] per `paNamed_port_connection` /
/// `paNamed_parameter_assignment` node.
///
/// Pairing strategy (parse-tree sibling order — works identically in UHDM
/// and fallback mode): every identifier leaf (`slStringConst`) of a
/// FileContent is attributed to its nearest structural ancestor by walking
/// the `parent_index` chain until the first of
///
/// * `paNamed_port_connection` → PORT connection leaf (label first, then the
///   leading identifier of the actual expression, in node-array order),
/// * `paNamed_parameter_assignment` → PARAMETER override leaf (same shape:
///   label first, then the leading identifier of the value expression),
/// * `paModule_instantiation` → instantiation-level leaf (the FIRST such
///   leaf is the module TYPE name),
/// * `paName_of_instance` → instance name (ignored),
/// * anything else / depth bound → ignored.
///
/// The walk deliberately passes through expression plumbing
/// (`paPrimary`/`paPrimary_literal`/…) that the DECL/REF classifier treats
/// as terminal: attribution needs the enclosing connection, not a
/// classification.  Include-injected nodes (foreign `file_id`) and nodes
/// without positions are skipped.
pub(crate) fn scan_named_port_connections(
    design: &llg::ffi::surelog::Design,
) -> Vec<NamedPortConn> {
    use llg::core::vobject_types::VObjectType;

    const MAX_ANCESTOR_DEPTH: usize = 32;

    /// What the ancestor walk decided about one identifier leaf.
    enum LeafOwner {
        Connection(u32, ConnKind),
        Instantiation(u32),
        Ignored,
    }

    fn owner_of(nodes: &[llg::ffi::surelog::ParseNode], start: u32) -> LeafOwner {
        let mut cur = start;
        for _ in 0..MAX_ANCESTOR_DEPTH {
            if cur == 0 {
                break;
            }
            let idx = cur as usize;
            if idx >= nodes.len() {
                break;
            }
            match VObjectType::try_from(nodes[idx].type_id) {
                Ok(VObjectType::paNamed_port_connection) => {
                    return LeafOwner::Connection(cur, ConnKind::Port);
                }
                Ok(VObjectType::paNamed_parameter_assignment) => {
                    return LeafOwner::Connection(cur, ConnKind::Param);
                }
                Ok(VObjectType::paModule_instantiation) => {
                    return LeafOwner::Instantiation(cur);
                }
                Ok(VObjectType::paName_of_instance) => return LeafOwner::Ignored,
                _ => cur = nodes[idx].parent_index,
            }
        }
        LeafOwner::Ignored
    }

    fn enclosing_instantiation(nodes: &[llg::ffi::surelog::ParseNode], start: u32) -> Option<u32> {
        let mut cur = start;
        for _ in 0..MAX_ANCESTOR_DEPTH {
            if cur == 0 {
                return None;
            }
            let idx = cur as usize;
            if idx >= nodes.len() {
                return None;
            }
            if VObjectType::try_from(nodes[idx].type_id) == Ok(VObjectType::paModule_instantiation)
            {
                return Some(cur);
            }
            cur = nodes[idx].parent_index;
        }
        None
    }

    let mut out = Vec::new();
    for fc_idx in 0..design.file_content_count() {
        let Some(fc) = design.file_content(fc_idx) else {
            continue;
        };
        let own_id = fc.file_id();
        let path = fc.path();
        let nodes: Vec<llg::ffi::surelog::ParseNode> = (0..fc.node_count())
            .filter_map(|i| fc.get_node(i))
            .collect();

        // Connection leaves in document order (nodes arrive in node-array
        // order, so each connection's leaf vector keeps that order) + first
        // leaf per instantiation.
        let mut conn_leaves: HashMap<u32, Vec<(u32, u32, String)>> = HashMap::new();
        let mut conn_kind: HashMap<u32, ConnKind> = HashMap::new();
        let mut inst_type_leaf: HashMap<u32, String> = HashMap::new();
        for node in nodes.iter() {
            if node.file_id != own_id || node.type_id != VObjectType::slStringConst as u16 {
                continue;
            }
            if node.line == 0 || node.col == 0 {
                continue;
            }
            let Some(sym) = node.symbol_name.as_deref().filter(|s| !s.is_empty()) else {
                continue;
            };
            match owner_of(&nodes, node.parent_index) {
                LeafOwner::Connection(conn_idx, kind) => {
                    conn_leaves.entry(conn_idx).or_default().push((
                        node.line,
                        node.col as u32,
                        sym.to_owned(),
                    ));
                    conn_kind.insert(conn_idx, kind);
                }
                LeafOwner::Instantiation(inst_idx) => {
                    inst_type_leaf
                        .entry(inst_idx)
                        .or_insert_with(|| sym.to_owned());
                }
                LeafOwner::Ignored => {}
            }
        }

        let mut conn_indices: Vec<u32> = conn_leaves.keys().copied().collect();
        conn_indices.sort_unstable();
        for conn_idx in conn_indices {
            let leaves = &conn_leaves[&conn_idx];
            let Some(&(label_line, label_col, ref label_name)) = leaves.first() else {
                continue;
            };
            let actual = leaves.get(1).map(|&(line, col, _)| (line, col));
            let actual_name = leaves.get(1).map(|(_, _, name)| name.clone());
            // The instantiation type comes from the nearest enclosing
            // `paModule_instantiation`'s own (first) identifier leaf.
            let inst_type = enclosing_instantiation(&nodes, conn_idx)
                .and_then(|inst_idx| inst_type_leaf.get(&inst_idx).cloned());
            out.push(NamedPortConn {
                file: path.clone(),
                label: (label_line, label_col),
                label_name: label_name.clone(),
                actual,
                actual_name,
                inst_type,
                kind: conn_kind.get(&conn_idx).copied().unwrap_or(ConnKind::Port),
            });
        }
    }
    out
}

/// Collect enum declarations and expression references from Surelog's parse
/// tree.  UHDM is deliberately not used here: package/class-qualified enum
/// expressions are commonly folded to literals before the VPI walk, while
/// the parse tree still retains both the scope and the exact member range.
///
/// Resolution is intentionally conservative.  A qualified member is bound
/// only to a unique enum declaration in that scope.  A bare member considers
/// the current scope, explicit imports, and finally the workspace; any set
/// with more than one candidate is recorded as unresolved so the ordinary
/// same-name fallback cannot manufacture a misleading definition.
pub(super) fn scan_parse_enum_facts(design: &llg::ffi::surelog::Design) -> ParseEnumFacts {
    use llg::core::vobject_types::VObjectType;

    const MAX_TREE_DEPTH: usize = 128;

    #[derive(Debug, Clone)]
    struct ScopeSpan {
        name: String,
        start_line: u32,
        end_line: u32,
    }

    #[derive(Debug, Clone)]
    struct Import {
        owner: Option<String>,
        package: String,
        item: Option<String>,
    }

    fn node_type(nodes: &[llg::ffi::surelog::ParseNode], index: u32) -> Option<VObjectType> {
        nodes
            .get(index as usize)
            .and_then(|node| VObjectType::try_from(node.type_id).ok())
    }

    fn descendants(nodes: &[llg::ffi::surelog::ParseNode], root: u32) -> Vec<u32> {
        let mut result = Vec::new();
        let mut stack = vec![(root, 0usize)];
        let mut visited = HashSet::new();
        while let Some((index, depth)) = stack.pop() {
            if depth > MAX_TREE_DEPTH || !visited.insert(index) {
                continue;
            }
            let Some(node) = nodes.get(index as usize) else {
                continue;
            };
            result.push(index);
            let mut child = node.child_index;
            let mut siblings = Vec::new();
            while child != 0 {
                let Some(child_node) = nodes.get(child as usize) else {
                    break;
                };
                siblings.push(child);
                child = child_node.sibling_index;
            }
            for child in siblings.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        result
    }

    fn ancestors(nodes: &[llg::ffi::surelog::ParseNode], mut index: u32) -> Vec<u32> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        for _ in 0..MAX_TREE_DEPTH {
            if index == 0 || !visited.insert(index) {
                break;
            }
            let Some(node) = nodes.get(index as usize) else {
                break;
            };
            result.push(index);
            index = node.parent_index;
        }
        result
    }

    fn has_ancestor(
        nodes: &[llg::ffi::surelog::ParseNode],
        parent: u32,
        wanted: VObjectType,
    ) -> bool {
        ancestors(nodes, parent)
            .into_iter()
            .any(|index| node_type(nodes, index) == Some(wanted))
    }

    fn expression_identifier(nodes: &[llg::ffi::surelog::ParseNode], parent: u32) -> bool {
        ancestors(nodes, parent).into_iter().any(|index| {
            matches!(
                node_type(nodes, index),
                Some(
                    VObjectType::paPrimary
                        | VObjectType::paPrimary_literal
                        | VObjectType::paExpression
                        | VObjectType::paConstant_expression
                        | VObjectType::paModule_path_expression
                )
            )
        })
    }

    fn scope_name(nodes: &[llg::ffi::surelog::ParseNode], root: u32) -> Option<String> {
        let root_node = nodes.get(root as usize)?;
        descendants(nodes, root)
            .into_iter()
            .filter_map(|index| {
                let node = nodes.get(index as usize)?;
                (node.type_id == VObjectType::slStringConst as u16
                    && node.file_id == root_node.file_id
                    && node.line == root_node.line
                    && node.col > root_node.col)
                    .then(|| (node.col, node.symbol_name.clone()))
            })
            .min_by_key(|(col, _)| *col)
            .and_then(|(_, name)| name)
            .filter(|name| !name.is_empty())
    }

    fn scope_at(scopes: &[ScopeSpan], line: u32) -> Option<&ScopeSpan> {
        scopes
            .iter()
            .filter(|scope| scope.start_line <= line && line <= scope.end_line)
            .min_by_key(|scope| scope.end_line.saturating_sub(scope.start_line))
    }

    fn add_enum_ref(
        facts: &mut ParseEnumFacts,
        reference: (&str, u32, u32),
        target: &ParseEnumDecl,
    ) {
        let key = (
            reference.0.to_owned(),
            reference.1.saturating_sub(1),
            reference.2.saturating_sub(1),
        );
        facts.reference_positions.insert(key.clone());
        facts.bindings.insert(
            key,
            DeclTarget {
                name: target.name.clone(),
                kind: "enum constant".to_owned(),
                file: target.file.clone(),
                line0: target.line1.saturating_sub(1),
                col0: target.col1.saturating_sub(1),
                via_label: false,
                via_connection: false,
            },
        );
        facts.synthetic_tokens.push(VObjectInfo {
            line: reference.1,
            col: reference.2,
            end_line: reference.1,
            end_col: reference
                .2
                .saturating_add(target.name.encode_utf16().count() as u32),
            vpi_type: llg::ffi::vpi::uhdmenum_const,
            name: Some(target.name.clone()),
            file: reference.0.to_owned(),
        });
    }

    fn add_unresolved(facts: &mut ParseEnumFacts, file: &str, line: u32, col: u32, name: &str) {
        let key = (
            file.to_owned(),
            line.saturating_sub(1),
            col.saturating_sub(1),
        );
        facts.reference_positions.insert(key.clone());
        facts.unresolved_positions.insert(key);
        facts.synthetic_tokens.push(VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col.saturating_add(name.encode_utf16().count() as u32),
            vpi_type: llg::ffi::vpi::uhdmenum_const,
            name: Some(name.to_owned()),
            file: file.to_owned(),
        });
    }

    fn add_candidate(
        facts: &mut ParseEnumFacts,
        file: &str,
        line: u32,
        col: u32,
        name: &str,
        candidates: Vec<ParseEnumDecl>,
    ) {
        let key = (
            file.to_owned(),
            line.saturating_sub(1),
            col.saturating_sub(1),
        );
        if facts.reference_positions.contains(&key) {
            return;
        }
        let mut unique = HashSet::new();
        let candidates: Vec<ParseEnumDecl> = candidates
            .into_iter()
            .filter(|candidate| {
                unique.insert((candidate.file.clone(), candidate.line1, candidate.col1))
            })
            .collect();
        match candidates.as_slice() {
            [target] => add_enum_ref(facts, (file, line, col), target),
            [] => {}
            _ => add_unresolved(facts, file, line, col, name),
        }
    }

    let mut facts = ParseEnumFacts::default();
    let mut imports = Vec::new();
    let mut qualified_candidates: Vec<(String, String, String, u32, u32)> = Vec::new();
    let mut qualified_positions = HashSet::new();
    let mut declaration_positions = HashSet::new();
    let mut import_positions = HashSet::new();
    let mut qualifier_positions = HashSet::new();
    let mut scopes_by_file: HashMap<String, Vec<ScopeSpan>> = HashMap::new();
    type EnumLeaf = (u32, String, u32, u32, bool);
    let mut leaves_by_file: HashMap<String, Vec<EnumLeaf>> = HashMap::new();

    // First pass: retain exact parse declarations, scope spans, imports, and
    // all source-local identifier leaves.  Later passes can then resolve
    // references across files without revisiting the live Surelog session.
    for fc_idx in 0..design.file_content_count() {
        let Some(fc) = design.file_content(fc_idx) else {
            continue;
        };
        let path = fc.path();
        let own_id = fc.file_id();
        let nodes: Vec<llg::ffi::surelog::ParseNode> = (0..fc.node_count())
            .filter_map(|index| fc.get_node(index))
            .collect();

        let mut scopes = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            if node.file_id != own_id || node.line == 0 {
                continue;
            }
            let Some(kind) = VObjectType::try_from(node.type_id).ok() else {
                continue;
            };
            let Some(_scope_kind) = (kind == VObjectType::paPackage_declaration
                || kind == VObjectType::paClass_declaration
                || kind == VObjectType::paModule_declaration)
                .then_some(kind)
            else {
                continue;
            };
            let Some(name) = scope_name(&nodes, index as u32) else {
                continue;
            };
            let end_line = if node.end_line >= node.line {
                node.end_line
            } else {
                u32::MAX
            };
            scopes.push(ScopeSpan {
                name,
                start_line: node.line,
                end_line,
            });
        }
        scopes.sort_by_key(|scope| (scope.start_line, scope.end_line));
        scopes_by_file.insert(path.clone(), scopes.clone());

        for (index, node) in nodes.iter().enumerate() {
            if node.file_id != own_id
                || node.line == 0
                || node.col == 0
                || node.type_id != VObjectType::slStringConst as u16
            {
                continue;
            }
            let Some(name) = node.symbol_name.as_deref().filter(|name| !name.is_empty()) else {
                continue;
            };
            leaves_by_file.entry(path.clone()).or_default().push((
                index as u32,
                name.to_owned(),
                node.line,
                node.col as u32,
                expression_identifier(&nodes, node.parent_index),
            ));
            if has_ancestor(
                &nodes,
                node.parent_index,
                VObjectType::paEnum_name_declaration,
            ) {
                let scope = scope_at(&scopes, node.line).map(|scope| scope.name.clone());
                facts.declarations.push(ParseEnumDecl {
                    name: name.to_owned(),
                    file: path.clone(),
                    line1: node.line,
                    col1: node.col as u32,
                    scope,
                });
                declaration_positions.insert((path.clone(), node.line, node.col as u32));
            }
            if has_ancestor(
                &nodes,
                node.parent_index,
                VObjectType::paPackage_import_item,
            ) {
                import_positions.insert((path.clone(), node.line, node.col as u32));
            }
        }

        for (index, node) in nodes.iter().enumerate() {
            if node.file_id != own_id || node.line == 0 {
                continue;
            }
            if VObjectType::try_from(node.type_id).ok() != Some(VObjectType::paPackage_import_item)
            {
                continue;
            }
            let leaves: Vec<u32> = descendants(&nodes, index as u32)
                .into_iter()
                .filter(|leaf_index| {
                    nodes.get(*leaf_index as usize).is_some_and(|leaf| {
                        leaf.type_id == VObjectType::slStringConst as u16
                            && leaf.file_id == own_id
                            && leaf.line != 0
                    })
                })
                .collect();
            let Some(package_leaf) = leaves.first().and_then(|index| nodes.get(*index as usize))
            else {
                continue;
            };
            let Some(package_text) = package_leaf.symbol_name.as_deref() else {
                continue;
            };
            let (package, full_item) = package_text
                .rsplit_once("::")
                .map(|(package, item)| (package.to_owned(), Some(item.to_owned())))
                .unwrap_or_else(|| (package_text.to_owned(), None));
            let item = leaves
                .get(1)
                .and_then(|leaf_index| nodes.get(*leaf_index as usize))
                .and_then(|leaf| leaf.symbol_name.clone())
                .or(full_item);
            imports.push(Import {
                owner: scope_at(&scopes, node.line).map(|scope| scope.name.clone()),
                package,
                item,
            });
        }

        // Package/class scope nodes carry the qualifier as their own child;
        // the member is a sibling in the smallest expression subtree.  This
        // follows the tree links rather than matching arbitrary text on a
        // line, which keeps declaration/type uses out of the reference map.
        for (index, node) in nodes.iter().enumerate() {
            if node.file_id != own_id || node.line == 0 {
                continue;
            }
            let Some(kind) = VObjectType::try_from(node.type_id).ok() else {
                continue;
            };
            if kind != VObjectType::paPackage_scope && kind != VObjectType::paClass_scope {
                continue;
            }
            let qualifier = descendants(&nodes, index as u32)
                .into_iter()
                .filter_map(|leaf_index| nodes.get(leaf_index as usize))
                .find(|leaf| {
                    leaf.type_id == VObjectType::slStringConst as u16
                        && leaf.file_id == own_id
                        && leaf.line != 0
                });
            let Some(qualifier) = qualifier else {
                continue;
            };
            let Some(qualifier_name) = qualifier.symbol_name.as_deref() else {
                continue;
            };
            qualifier_positions.insert((path.clone(), qualifier.line, qualifier.col as u32));
            let qualifier_leaves: HashSet<u32> =
                descendants(&nodes, index as u32).into_iter().collect();
            let sibling_root = node.parent_index;
            let member = descendants(&nodes, sibling_root)
                .into_iter()
                .filter(|leaf_index| !qualifier_leaves.contains(leaf_index))
                .filter_map(|leaf_index| nodes.get(leaf_index as usize))
                .filter(|leaf| {
                    leaf.type_id == VObjectType::slStringConst as u16
                        && leaf.file_id == own_id
                        && (leaf.line, leaf.col) > (qualifier.line, qualifier.col)
                        && expression_identifier(&nodes, leaf.parent_index)
                })
                .min_by_key(|leaf| (leaf.line, leaf.col));
            if let Some(member) = member {
                if let Some(name) = member
                    .symbol_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                {
                    qualified_candidates.push((
                        path.clone(),
                        qualifier_name.to_owned(),
                        name.to_owned(),
                        member.line,
                        member.col as u32,
                    ));
                    qualified_positions.insert((path.clone(), member.line, member.col as u32));
                }
            }
        }

        // `exitPs_identifier` may combine `pkg::member` into one leaf.  Split
        // that spelling back into the member's exact UTF-16 range when the
        // parser did not retain a separate package-scope node.
        for (_, name, line, col, expressionish) in leaves_by_file.get(&path).into_iter().flatten() {
            let Some((qualifier, member)) = name.rsplit_once("::") else {
                continue;
            };
            if member.is_empty() || !name.contains("::") {
                continue;
            }
            let member_col = col.saturating_add(
                qualifier.encode_utf16().count() as u32 + "::".encode_utf16().count() as u32,
            );
            if *expressionish {
                qualified_candidates.push((
                    path.clone(),
                    qualifier.to_owned(),
                    member.to_owned(),
                    *line,
                    member_col,
                ));
                qualified_positions.insert((path.clone(), *line, member_col));
            }
        }
    }

    // Declarations from malformed/included parse trees can repeat at one
    // coordinate.  Keeping one fact is enough for binding, while duplicate
    // coordinates still count as ambiguous only when they are distinct.
    facts.declarations.sort_by_key(|decl| {
        (
            decl.name.clone(),
            decl.scope.clone(),
            decl.file.clone(),
            decl.line1,
            decl.col1,
        )
    });
    facts.declarations.dedup_by(|left, right| {
        left.name == right.name
            && left.scope == right.scope
            && left.file == right.file
            && left.line1 == right.line1
            && left.col1 == right.col1
    });

    let enum_names: HashSet<String> = facts
        .declarations
        .iter()
        .map(|decl| decl.name.clone())
        .collect();
    let known_scope_names: HashSet<String> = facts
        .declarations
        .iter()
        .filter_map(|decl| decl.scope.clone())
        .collect();

    for (file, qualifier, name, line, col) in qualified_candidates {
        let candidates: Vec<ParseEnumDecl> = facts
            .declarations
            .iter()
            .filter(|decl| decl.name == name && decl.scope.as_deref() == Some(qualifier.as_str()))
            .cloned()
            .collect();
        if !candidates.is_empty() || known_scope_names.contains(&qualifier) {
            add_candidate(&mut facts, &file, line, col, &name, candidates);
        }
    }

    for (file, entries) in leaves_by_file {
        let scopes = scopes_by_file.get(&file).map(Vec::as_slice).unwrap_or(&[]);
        for (_index, name, line, col, expressionish) in entries {
            let key = (file.clone(), line, col);
            if !enum_names.contains(&name)
                || declaration_positions.contains(&key)
                || import_positions.contains(&key)
                || qualifier_positions.contains(&key)
                || qualified_positions.contains(&key)
                || facts.reference_positions.contains(&(
                    file.clone(),
                    line.saturating_sub(1),
                    col.saturating_sub(1),
                ))
                || !expressionish
            {
                continue;
            }
            // Reconstructing the parent from this compact leaf list is not
            // possible, so only parse leaves already known to be expression
            // references are considered in this pass.  Surelog's enum-use
            // token is `uhdmenum_const`; declaration positions are the only
            // other enum-typed positions and were removed above.
            let owner = scope_at(scopes, line).map(|scope| scope.name.as_str());
            let local: Vec<ParseEnumDecl> = facts
                .declarations
                .iter()
                .filter(|decl| decl.name == name && decl.scope.as_deref() == owner)
                .cloned()
                .collect();
            if !local.is_empty() {
                add_candidate(&mut facts, &file, line, col, &name, local);
                continue;
            }
            let imported: Vec<ParseEnumDecl> = imports
                .iter()
                .filter(|import| import.owner.as_deref() == owner)
                .filter(|import| import.item.as_deref().is_none_or(|item| item == name))
                .flat_map(|import| {
                    facts
                        .declarations
                        .iter()
                        .filter(|decl| {
                            decl.name == name
                                && decl.scope.as_deref() == Some(import.package.as_str())
                        })
                        .cloned()
                })
                .collect();
            if !imported.is_empty() {
                add_candidate(&mut facts, &file, line, col, &name, imported);
            } else {
                let global: Vec<ParseEnumDecl> = facts
                    .declarations
                    .iter()
                    .filter(|decl| decl.name == name)
                    .cloned()
                    .collect();
                add_candidate(&mut facts, &file, line, col, &name, global);
            }
        }
    }

    facts
}
