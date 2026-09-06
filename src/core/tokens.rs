//! tokens — collect design objects from the UHDM VPI tree and from Surelog's
//! parse-tree (FileContent) for semantic highlighting.
//!
//! Two complementary sources are combined:
//!
//! 1. **UHDM VPI tree** (`collect_vpi_tokens`) — provides named design objects
//!    (modules, ports, nets, parameters, signal references, …) identified by
//!    stable IEEE-1800 VPI type constants.
//!
//! 2. **Surelog parse tree** (`collect_parse_tokens`) — provides keyword tokens
//!    (wire, reg, logic, module, input, …) and macro-usage sites that are
//!    absent from the UHDM model.  Node types are identified by Surelog's
//!    internal `VObjectType` discriminant (shifted by `PARSE_OFFSET` to avoid
//!    collision with VPI type constants).
//!
//! `collect_all_tokens` calls both, merges the results, and additionally
//! returns the reference→declaration bindings ([`RefBindings`]) captured
//! during the same VPI walk: every reference leaf emitted from the UHDM tree
//! is resolved through `vpiActual` to its bound declaration, keyed at exactly
//! the emitted 0-based reference position.  This is what makes goto-definition
//! binding-precise in the LSP without any extra VPI traversal.

use std::collections::{HashMap, HashSet};

use crate::core::vobject_types::*;
use crate::ffi::surelog::{self, VObjectInfo};
use crate::ffi::vpi::{self, VpiHandle};

// ── Public types ──────────────────────────────────────────────────────────────

/// Semantic token data for a single source file.
#[derive(Debug)]
pub struct FileTokens {
    /// Canonical file-system path for this file.
    pub path: String,
    /// All design objects collected for this file, in arbitrary order.
    /// `semantic_tokens::encode` will sort them by position.
    pub nodes: Vec<VObjectInfo>,
}

/// The declared object that a reference occurrence is bound to by UHDM
/// elaboration (`vpiActual` on a `ref_obj`).
///
/// Fully owned so it can cross the session boundary like the rest of the
/// collected data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclTarget {
    /// Name of the declared object (`vpiName` of the bound target).
    pub name: String,
    /// Human-readable kind of the declared object ("net", "var",
    /// "parameter", "port", …).
    pub kind: String,
    /// Absolute path of the declaration file (`vpiFile` of the target).
    pub file: String,
    /// 0-based declaration line.
    pub line0: u32,
    /// 0-based declaration column.
    pub col0: u32,
    /// `true` when this binding was derived from a named port-connection
    /// label fold (`.clk` in `.clk(c)` → child port declaration) rather
    /// than captured from UHDM elaboration (`vpiActual`).  Diagnostics
    /// consumers use this to distinguish heuristic targets from
    /// elaboration-backed ones.
    pub via_label: bool,
    /// `true` when this binding targets the connected expression (the ACTUAL
    /// or an override RHS) of a named connection instead of its label —
    /// `.clk(wa)`'s `wa` binds to its own parent-scope declaration while the
    /// label binds to the child port, and `.W(rhs)`'s `rhs` behaves the same
    /// against the child parameter.
    pub via_connection: bool,
}

impl DeclTarget {
    /// Build a target from 1-based VPI coordinates, rejecting targets without
    /// usable coordinates (unknown file/line/column or empty name): such
    /// targets cannot anchor a precise definition location and are skipped.
    ///
    /// Targets built here are always elaboration-backed, so [`DeclTarget::via_label`]
    /// and [`DeclTarget::via_connection`] start `false`; only the LSP's
    /// port-connection folds flip them.
    pub fn from_decl_coords(
        name: &str,
        kind: &str,
        file: &str,
        line1: u32,
        col1: u32,
    ) -> Option<DeclTarget> {
        if name.is_empty() || kind.is_empty() || file.is_empty() || line1 == 0 || col1 == 0 {
            return None;
        }
        Some(DeclTarget {
            name: name.to_owned(),
            kind: kind.to_owned(),
            file: file.to_owned(),
            line0: line1 - 1,
            col0: col1 - 1,
            via_label: false,
            via_connection: false,
        })
    }
}

/// Reference occurrence → bound declaration, keyed by `(file, line0, col0)`.
///
/// Keys are created at exactly the position of the emitted reference token
/// (the same `vpiFile`/`vpiLineNo`/`vpiColumnNo` values converted to 0-based),
/// so a lookup with the 0-based LSP click position aligns by construction.
pub type RefBindings = HashMap<(String, u32, u32), DeclTarget>;

/// Rendered declaration snippet (`logic [3:0] val`, `input logic [1:0] sel`,
/// …) per declared object, keyed by `(file, line1, col1)` of the DECLARATION
/// token.
///
/// Captured during the same VPI walk as the tokens, straight from each
/// declared object's typespec, so the text is position-accurate even where
/// several same-named declarations live in one module (inner-scope shadowing)
/// and the name-based model lookup would describe the wrong object.  The
/// rendering mirrors `core::model`'s `TypeInfo::render()` conventions
/// (`logic [7:0]`, scalar kinds bare, `signed` prefix for non-integer signed
/// kinds).
pub type DeclDetails = HashMap<(String, u32, u32), String>;

// ── Public API ────────────────────────────────────────────────────────────────

/// Walk the UHDM design tree using the VPI interface and collect semantic
/// token information for every source file referenced by the design.
///
/// The returned `Vec` has one entry per unique `vpiFile` path encountered.
/// Callers typically filter the result to the file they are interested in.
/// Reference→declaration bindings are not retained by this entry point; use
/// [`collect_all_tokens`] when they are needed.
///
/// # Safety
/// `design` must be a valid UHDM VPI design handle obtained from
/// `sl_get_uhdm_design`.  It must remain valid for the duration of this call.
pub fn collect_vpi_tokens(design: VpiHandle) -> Vec<FileTokens> {
    let mut all_nodes: Vec<VObjectInfo> = Vec::new();
    let mut bindings: RefBindings = HashMap::new();
    let mut details: DeclDetails = HashMap::new();

    walk_design(design, &mut all_nodes, &mut bindings, &mut details);

    group_nodes_by_file(all_nodes)
}

/// Walk the Surelog parse-tree (FileContent) and collect keyword, macro-usage,
/// and declaration-name identifier tokens for every source file in the design.
///
/// This is the counterpart to `collect_vpi_tokens`.  It provides tokens for:
/// - Net-type keywords: `wire`, `reg`, `logic`, `tri`, `supply0`, …
/// - Port-direction keywords: `input`, `output`, `inout`
/// - Scope keywords: `module`, `endmodule`, `class`, `package`, …
/// - Data-type keywords: `integer`, `byte`, `real`, `string`, …
/// - Qualifier keywords: `parameter`, `localparam`, `static`, `virtual`, …
/// - Macro usages: `` `TICK_DEFINE ``, `` `MY_MACRO(...) ``
/// - Declaration name identifiers: module, class, package, net, port, … names
///   (classified by their ancestor declaration-context node in the parse tree)
///
/// Declaration-site positions recorded by [`collect_parse_tokens`]:
/// `(file path, 1-based line, 1-based column)` for every identifier the
/// parse-tree classifier marked as a DECLARATION.  Threading this set into
/// `SymbolIndex::from_parts` lets the syntax-broken fallback path
/// distinguish declarations from same-typed references without the
/// multi-view hints only an elaborated (UHDM) analysis provides.
pub type ParseDeclPositions = HashSet<(String, u32, u32)>;

/// LSP-internal token kinds for source-level generate variables. UHDM turns
/// generate-loop variables into per-iteration localparams, so their logical
/// declaration and references must come from the parse tree instead.
pub const TOKEN_GENVAR_DECL: i32 = 10_007;
pub const TOKEN_GENVAR_REF: i32 = 10_008;

/// Source-level generate-variable bindings recovered from the parse tree.
#[derive(Debug, Default)]
pub struct ParseGenvarFacts {
    pub bindings: RefBindings,
}

#[derive(Debug, Clone)]
struct ParseGenvarDecl {
    name: String,
    node_index: usize,
    scope_index: usize,
    initializer_index: Option<usize>,
    line: u32,
    col: u32,
    keyword: Option<(u32, u32)>,
}

struct ParseShadowDecl {
    name: String,
    kind: &'static str,
    scope_index: usize,
    line: u32,
    col: u32,
}

fn parse_ancestor_of_type(
    nodes: &[surelog::ParseNode],
    mut parent_idx: u32,
    wanted: impl Fn(VObjectType) -> bool,
) -> Option<usize> {
    let mut remaining = nodes.len().min(256);
    while parent_idx != 0 && remaining != 0 {
        let idx = parent_idx as usize;
        let node = nodes.get(idx)?;
        if VObjectType::try_from(node.type_id).is_ok_and(&wanted) {
            return Some(idx);
        }
        parent_idx = node.parent_index;
        remaining -= 1;
    }
    None
}

fn parse_node_is_descendant_of(
    nodes: &[surelog::ParseNode],
    mut node_index: usize,
    ancestor_index: usize,
) -> bool {
    let mut remaining = nodes.len().min(256);
    while remaining != 0 {
        if node_index == ancestor_index {
            return true;
        }
        let Some(node) = nodes.get(node_index) else {
            return false;
        };
        if node.parent_index == 0 {
            return false;
        }
        node_index = node.parent_index as usize;
        remaining -= 1;
    }
    false
}

fn parse_ancestor_distance(
    nodes: &[surelog::ParseNode],
    mut node_index: usize,
    ancestor_index: usize,
) -> Option<usize> {
    let mut distance = 0;
    let mut remaining = nodes.len().min(256);
    while remaining != 0 {
        if node_index == ancestor_index {
            return Some(distance);
        }
        let node = nodes.get(node_index)?;
        if node.parent_index == 0 {
            return None;
        }
        node_index = node.parent_index as usize;
        distance += 1;
        remaining -= 1;
    }
    None
}

fn direct_identifier_context(
    nodes: &[surelog::ParseNode],
    mut parent_idx: u32,
) -> Option<(usize, VObjectType)> {
    const MAX_DEPTH: usize = 8;
    for _ in 0..MAX_DEPTH {
        let idx = parent_idx as usize;
        let node = nodes.get(idx)?;
        let kind = VObjectType::try_from(node.type_id).ok()?;
        if matches!(
            kind,
            VObjectType::paSimple_identifier
                | VObjectType::paIdentifier
                | VObjectType::paEscaped_identifier
        ) {
            parent_idx = node.parent_index;
            continue;
        }
        return Some((idx, kind));
    }
    None
}

fn scan_file_genvars(
    nodes: &[surelog::ParseNode],
    path: &str,
    own_id: u32,
) -> (Vec<VObjectInfo>, ParseDeclPositions, ParseGenvarFacts) {
    let mut declarations = Vec::new();
    for (node_index, node) in nodes.iter().enumerate() {
        if node.file_id != own_id
            || node.type_id != VObjectType::slStringConst as u16
            || node.line == 0
        {
            continue;
        }
        let Some(name) = node.symbol_name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        let Some((context_index, context)) = direct_identifier_context(nodes, node.parent_index)
        else {
            continue;
        };
        let (scope_index, initializer_index, keyword) = match context {
            VObjectType::paIdentifier_list => {
                let Some(declaration_index) =
                    parse_ancestor_of_type(nodes, nodes[context_index].parent_index, |kind| {
                        kind == VObjectType::paGenvar_declaration
                    })
                else {
                    continue;
                };
                let Some(scope_index) =
                    parse_ancestor_of_type(nodes, nodes[declaration_index].parent_index, |kind| {
                        matches!(
                            kind,
                            VObjectType::paGenerate_begin_end_block
                                | VObjectType::paGenerate_module_block
                                | VObjectType::paGenerate_interface_block
                                | VObjectType::paGenerate_module_named_block
                                | VObjectType::paGenerate_interface_named_block
                                | VObjectType::paModule_declaration
                                | VObjectType::paInterface_declaration
                        )
                    })
                else {
                    continue;
                };
                let first_in_declaration = !declarations.iter().any(|decl: &ParseGenvarDecl| {
                    parse_node_is_descendant_of(nodes, decl.node_index, declaration_index)
                });
                let declaration = &nodes[declaration_index];
                (
                    scope_index,
                    None,
                    first_in_declaration.then_some((declaration.line, declaration.col as u32)),
                )
            }
            VObjectType::paGenvar_initialization | VObjectType::paGenvar_decl_assignment
                if (nodes[context_index].line, nodes[context_index].col)
                    < (node.line, node.col) =>
            {
                let Some(scope_index) =
                    parse_ancestor_of_type(nodes, nodes[context_index].parent_index, |kind| {
                        matches!(
                            kind,
                            VObjectType::paLoop_generate_construct
                                | VObjectType::paGenerate_module_loop_statement
                                | VObjectType::paGenerate_interface_loop_statement
                        )
                    })
                else {
                    continue;
                };
                (
                    scope_index,
                    Some(context_index),
                    Some((nodes[context_index].line, nodes[context_index].col as u32)),
                )
            }
            _ => continue,
        };
        declarations.push(ParseGenvarDecl {
            name: name.to_owned(),
            node_index,
            scope_index,
            initializer_index,
            line: node.line,
            col: node.col as u32,
            keyword,
        });
    }

    let shadow_declarations: Vec<ParseShadowDecl> = nodes
        .iter()
        .filter(|node| {
            node.file_id == own_id
                && node.type_id == VObjectType::slStringConst as u16
                && node.line != 0
        })
        .filter_map(|node| {
            let name = node.symbol_name.as_deref()?.to_owned();
            let (_, context) = direct_identifier_context(nodes, node.parent_index)?;
            if !matches!(
                context,
                VObjectType::paParam_assignment
                    | VObjectType::paVariable_decl_assignment
                    | VObjectType::paNet_decl_assignment
                    | VObjectType::paTf_port_item
                    | VObjectType::paList_of_tf_variable_identifiers
                    | VObjectType::paFor_variable_declaration
            ) {
                return None;
            }
            if context == VObjectType::paVariable_decl_assignment
                && parse_ancestor_of_type(nodes, node.parent_index, |kind| {
                    kind == VObjectType::paType_declaration
                })
                .is_some()
            {
                return None;
            }
            let scope_index = parse_ancestor_of_type(nodes, node.parent_index, |kind| {
                matches!(
                    kind,
                    VObjectType::paFunction_body_declaration
                        | VObjectType::paFunction_declaration
                        | VObjectType::paTask_body_declaration
                        | VObjectType::paTask_declaration
                        | VObjectType::paGenerate_begin_end_block
                        | VObjectType::paGenerate_module_block
                        | VObjectType::paGenerate_interface_block
                        | VObjectType::paSeq_block
                        | VObjectType::paModule_declaration
                        | VObjectType::paInterface_declaration
                )
            })?;
            Some(ParseShadowDecl {
                name,
                kind: if context == VObjectType::paParam_assignment {
                    "parameter"
                } else if context == VObjectType::paNet_decl_assignment {
                    "net"
                } else {
                    "var"
                },
                scope_index,
                line: node.line,
                col: node.col as u32,
            })
        })
        .collect();

    let declaration_nodes: HashSet<usize> =
        declarations.iter().map(|decl| decl.node_index).collect();
    let mut tokens = Vec::new();
    let mut positions = HashSet::new();
    let mut facts = ParseGenvarFacts::default();
    for declaration in &declarations {
        let node = &nodes[declaration.node_index];
        positions.insert((path.to_owned(), declaration.line, declaration.col));
        tokens.push(VObjectInfo {
            line: declaration.line,
            col: declaration.col,
            end_line: node.end_line,
            end_col: node.end_col as u32,
            vpi_type: TOKEN_GENVAR_DECL,
            name: Some(declaration.name.clone()),
            file: path.to_owned(),
        });
        facts.bindings.insert(
            (
                path.to_owned(),
                declaration.line.saturating_sub(1),
                declaration.col.saturating_sub(1),
            ),
            DeclTarget {
                name: declaration.name.clone(),
                kind: "genvar".to_owned(),
                file: path.to_owned(),
                line0: declaration.line.saturating_sub(1),
                col0: declaration.col.saturating_sub(1),
                via_label: false,
                via_connection: false,
            },
        );
        if let Some((keyword_line, keyword_col)) = declaration.keyword {
            tokens.push(VObjectInfo {
                line: keyword_line,
                col: keyword_col,
                end_line: keyword_line,
                end_col: keyword_col.saturating_add(6),
                vpi_type: VObjectTypeShifted::paGENVAR.into(),
                name: Some("genvar".to_owned()),
                file: path.to_owned(),
            });
        }
    }

    for (node_index, node) in nodes.iter().enumerate() {
        if node.file_id != own_id
            || node.type_id != VObjectType::slStringConst as u16
            || node.line == 0
            || declaration_nodes.contains(&node_index)
        {
            continue;
        }
        let Some(name) = node.symbol_name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        let Some((_, context)) = direct_identifier_context(nodes, node.parent_index) else {
            continue;
        };
        if !matches!(
            context,
            VObjectType::paGenvar_initialization
                | VObjectType::paGenvar_iteration
                | VObjectType::paPrimary_literal
        ) {
            continue;
        }
        let Some(_) = parse_ancestor_of_type(nodes, node.parent_index, |kind| {
            matches!(
                kind,
                VObjectType::paLoop_generate_construct
                    | VObjectType::paGenerate_module_loop_statement
                    | VObjectType::paGenerate_interface_loop_statement
            )
        }) else {
            continue;
        };
        let Some(declaration) = declarations
            .iter()
            .filter(|decl| {
                decl.name == name
                    && parse_node_is_descendant_of(nodes, node_index, decl.scope_index)
                    && decl.line <= node.line
                    && !decl.initializer_index.is_some_and(|initializer| {
                        parse_node_is_descendant_of(nodes, node_index, initializer)
                    })
            })
            .min_by_key(|decl| {
                parse_ancestor_distance(nodes, node_index, decl.scope_index).unwrap_or(usize::MAX)
            })
        else {
            continue;
        };
        let genvar_distance = parse_ancestor_distance(nodes, node_index, declaration.scope_index)
            .unwrap_or(usize::MAX);
        let shadow = shadow_declarations
            .iter()
            .filter(|shadow| {
                shadow.name == name && (shadow.line, shadow.col) <= (node.line, node.col as u32)
            })
            .filter_map(|shadow| {
                parse_ancestor_distance(nodes, node_index, shadow.scope_index)
                    .filter(|distance| *distance < genvar_distance)
                    .map(|distance| (distance, shadow))
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, shadow)| shadow);
        if let Some(shadow) = shadow {
            facts.bindings.insert(
                (
                    path.to_owned(),
                    node.line.saturating_sub(1),
                    (node.col as u32).saturating_sub(1),
                ),
                DeclTarget {
                    name: shadow.name.clone(),
                    kind: shadow.kind.to_owned(),
                    file: path.to_owned(),
                    line0: shadow.line.saturating_sub(1),
                    col0: shadow.col.saturating_sub(1),
                    via_label: false,
                    via_connection: false,
                },
            );
            continue;
        }
        tokens.push(VObjectInfo {
            line: node.line,
            col: node.col as u32,
            end_line: node.end_line,
            end_col: node.end_col as u32,
            vpi_type: TOKEN_GENVAR_REF,
            name: Some(name.to_owned()),
            file: path.to_owned(),
        });
        facts.bindings.insert(
            (
                path.to_owned(),
                node.line.saturating_sub(1),
                (node.col as u32).saturating_sub(1),
            ),
            DeclTarget {
                name: declaration.name.clone(),
                kind: "genvar".to_owned(),
                file: path.to_owned(),
                line0: declaration.line.saturating_sub(1),
                col0: declaration.col.saturating_sub(1),
                via_label: false,
                via_connection: false,
            },
        );
    }
    (tokens, positions, facts)
}

/// Whether any ancestor of `parent_idx` (bounded walk) is an assignment
/// left-hand-side context (`paNet_lvalue` / `paVariable_lvalue`).  Such
/// identifiers are references to an existing object, not declarations, even
/// though the generic classifier maps them to net/var declaration types.
fn ancestor_is_assignment_lvalue(nodes: &[surelog::ParseNode], mut parent_idx: u32) -> bool {
    const MAX_DEPTH: usize = 8;
    for _ in 0..MAX_DEPTH {
        if parent_idx == 0 {
            return false;
        }
        let idx = parent_idx as usize;
        if idx >= nodes.len() {
            return false;
        }
        match VObjectType::try_from(nodes[idx].type_id) {
            Ok(t) if t == VObjectType::paNet_lvalue || t == VObjectType::paVariable_lvalue => {
                return true;
            }
            // Transparent identifier wrappers keep the chain going; anything
            // else ends the walk without a verdict.
            Ok(t)
                if t == VObjectType::paSimple_identifier
                    || t == VObjectType::paIdentifier
                    || t == VObjectType::paPs_or_hierarchical_identifier =>
            {
                parent_idx = nodes[idx].parent_index;
            }
            _ => return false,
        }
    }
    false
}

pub fn collect_parse_tokens(design: &surelog::Design) -> (Vec<FileTokens>, ParseDeclPositions) {
    let mut by_file: HashMap<String, Vec<VObjectInfo>> = HashMap::new();
    let mut decl_positions: ParseDeclPositions = HashSet::new();

    let fc_count = design.file_content_count();
    for fc_idx in 0..fc_count {
        let fc = match design.file_content(fc_idx) {
            Some(f) => f,
            None => continue,
        };

        let path = fc.path();
        let own_id = fc.file_id();
        let n_nodes = fc.node_count();

        // ── Pass 1: collect all parse-tree nodes for this FileContent ──────
        // We need the full list so that identifier classification can follow
        // `parent_index` links to find declaration-context ancestor nodes.
        let all_nodes: Vec<surelog::ParseNode> =
            (0..n_nodes).filter_map(|i| fc.get_node(i)).collect();

        let (genvar_tokens, genvar_decls, _) = scan_file_genvars(&all_nodes, &path, own_id);
        decl_positions.extend(genvar_decls);

        let file_nodes = by_file.entry(path.clone()).or_default();
        file_nodes.extend(genvar_tokens);
        let mut declarations: HashMap<String, i32> = HashMap::new();

        // ── Pass 2: classify and emit tokens ──────────────────────────────
        for node in &all_nodes {
            // Skip nodes that were injected by `\`include` from a different
            // file — they will be visited through their own FileContent entry.
            if node.file_id != own_id {
                continue;
            }

            // Skip nodes with no position information.
            if node.line == 0 {
                continue;
            }

            let type_id = node.type_id;
            let raw_type = PARSE_OFFSET + type_id as i32;

            // ── Declaration name identifiers (slStringConst) ───────────────
            // All SV identifier leaf nodes share type_id 7 (`slStringConst`).
            // Classify them by the nearest ancestor declaration-context node
            // so that module names, class names, net names, etc. can be
            // highlighted distinctly.
            if type_id == VObjectType::slStringConst as u16 {
                if let Some(sym) = &node.symbol_name {
                    if !sym.is_empty() {
                        let classified = classify_identifier_ancestor(
                            &all_nodes,
                            node.parent_index,
                            sym,
                            &declarations,
                        );
                        if let Some((vpi_type, is_decl)) = classified {
                            if is_decl {
                                declarations.insert(sym.clone(), vpi_type);
                                // Assignment left-hand sides (`paNet_lvalue`,
                                // `paVariable_lvalue`) classify as
                                // declaration-context nodes but are really
                                // REFERENCES to an existing object; they must
                                // not be recorded as declaration positions,
                                // otherwise goto-definition would treat every
                                // assignment target as a fresh declaration.
                                if !ancestor_is_assignment_lvalue(&all_nodes, node.parent_index) {
                                    decl_positions.insert((
                                        path.clone(),
                                        node.line,
                                        node.col as u32,
                                    ));
                                }
                            }
                            file_nodes.push(VObjectInfo {
                                line: node.line,
                                col: node.col as u32,
                                end_line: node.end_line,
                                end_col: node.end_col as u32,
                                vpi_type,
                                name: Some(sym.clone()),
                                file: path.clone(),
                            });
                        }
                    }
                }
                continue;
            }

            // ── Macro usage sites ──────────────────────────────────────────
            if raw_type == VObjectTypeShifted::ppMacroInstanceNoArgs
                || raw_type == VObjectTypeShifted::ppMacroInstanceWithArgs
            {
                // symbol_name is the bare macro name (without the leading
                // backtick).  We prepend "`" so the token length is correct.
                let sym = match &node.symbol_name {
                    Some(s) if !s.is_empty() => format!("`{s}"),
                    _ => continue, // no name → skip
                };
                file_nodes.push(VObjectInfo {
                    line: node.line,
                    col: node.col as u32,
                    end_line: node.end_line,
                    end_col: node.end_col as u32,
                    vpi_type: raw_type,
                    name: Some(sym),
                    file: path.clone(),
                });
                continue;
            }

            // ── Macro definition name (the identifier after `define) ───────
            if raw_type == VObjectTypeShifted::ppMacro_definition {
                if let Some(sym) = &node.symbol_name {
                    if !sym.is_empty() {
                        file_nodes.push(VObjectInfo {
                            line: node.line,
                            col: node.col as u32,
                            end_line: node.end_line,
                            end_col: node.end_col as u32,
                            vpi_type: raw_type,
                            name: Some(sym.clone()),
                            file: path.clone(),
                        });
                    }
                }
                continue;
            }

            // ── Keyword tokens ─────────────────────────────────────────────
            // Only emit nodes whose type_id maps to a known keyword.  The
            // keyword text becomes the token name (drives the length
            // computation in semantic_tokens::encode).
            if let Some(kw) = keyword_text_for_raw_type(raw_type) {
                // The module-keyword node covers both spellings (`module`,
                // `macromodule`); prefer the node's own source text so the
                // token length stays accurate.
                let text = if raw_type == VObjectTypeShifted::paModule_keyword {
                    node.symbol_name
                        .clone()
                        .filter(|sym| !sym.is_empty())
                        .unwrap_or_else(|| kw.to_owned())
                } else {
                    kw.to_owned()
                };
                file_nodes.push(VObjectInfo {
                    line: node.line,
                    col: node.col as u32,
                    end_line: node.end_line,
                    end_col: node.end_col as u32,
                    vpi_type: raw_type,
                    name: Some(text),
                    file: path.clone(),
                });
            }

            if type_id == VObjectType::paAssignment_pattern_key
                || type_id == VObjectType::paStructure_pattern_key
            {
                file_nodes.push(VObjectInfo {
                    line: node.line,
                    col: node.col as u32,
                    end_line: node.end_line,
                    end_col: node.end_col as u32,
                    vpi_type: raw_type,
                    name: None,
                    file: path.clone(),
                });
            }
        }
    }

    (
        by_file
            .into_iter()
            .filter(|(_, nodes)| !nodes.is_empty())
            .map(|(path, nodes)| FileTokens { path, nodes })
            .collect(),
        decl_positions,
    )
}

/// Collect lexical generate-variable bindings without walking elaborated VPI.
pub fn collect_parse_genvar_facts(design: &surelog::Design) -> ParseGenvarFacts {
    let mut facts = ParseGenvarFacts::default();
    for fc_idx in 0..design.file_content_count() {
        let Some(fc) = design.file_content(fc_idx) else {
            continue;
        };
        let nodes: Vec<surelog::ParseNode> = (0..fc.node_count())
            .filter_map(|index| fc.get_node(index))
            .collect();
        let (_, _, file_facts) = scan_file_genvars(&nodes, &fc.path(), fc.file_id());
        facts.bindings.extend(file_facts.bindings);
    }
    facts
}

/// Supplement an isolated parse with literal module-boundary tokens from the
/// requested source file.
///
/// `-parseonly` intentionally does not preprocess includes.  If an unresolved
/// macro occurs in a module header, Surelog may still return a partial parse
/// tree that omits the literal `module` keyword, module name, or `endmodule`.
/// This fallback scans only `source` (never an include target) and fills those
/// missing positions.  Existing parse-tree tokens win at duplicate positions.
pub fn supplement_source_local_module_tokens(
    file: &str,
    source: &str,
    parsed: &mut Vec<FileTokens>,
) {
    let mut supplemental = Vec::new();
    let words = source_local_words(source);

    for (index, word) in words.iter().enumerate() {
        let keyword = match word.text.as_str() {
            "module" | "macromodule" => word,
            "endmodule" => {
                supplemental.push(source_local_token(
                    file,
                    word,
                    PARSE_OFFSET + VObjectType::paENDMODULE as i32,
                ));
                continue;
            }
            _ => continue,
        };

        supplemental.push(source_local_token(
            file,
            keyword,
            PARSE_OFFSET + VObjectType::paModule_keyword as i32,
        ));

        if let Some(name) = words[index + 1..]
            .iter()
            .take_while(|candidate| {
                !candidate.escaped
                    && !matches!(
                        candidate.text.as_str(),
                        "module" | "macromodule" | "endmodule"
                    )
            })
            .find(|candidate| !matches!(candidate.text.as_str(), "automatic" | "static"))
        {
            supplemental.push(source_local_token(file, name, crate::ffi::vpi::vpiModule));
        }
    }

    if supplemental.is_empty() {
        return;
    }

    let file_index = if let Some(index) = parsed.iter().position(|tokens| tokens.path == file) {
        index
    } else {
        parsed.push(FileTokens {
            path: file.to_owned(),
            nodes: Vec::new(),
        });
        parsed.len() - 1
    };
    let file_tokens = &mut parsed[file_index];
    let existing_positions: HashSet<(u32, u32)> = file_tokens
        .nodes
        .iter()
        .map(|node| (node.line, node.col))
        .collect();
    file_tokens.nodes.extend(
        supplemental
            .into_iter()
            .filter(|node| !existing_positions.contains(&(node.line, node.col))),
    );
}

#[derive(Debug)]
struct SourceLocalWord {
    text: String,
    line: u32,
    col: u32,
    escaped: bool,
}

fn source_local_token(file: &str, word: &SourceLocalWord, vpi_type: i32) -> VObjectInfo {
    VObjectInfo {
        line: word.line,
        col: word.col,
        end_line: word.line,
        end_col: word.col + word.text.encode_utf16().count() as u32,
        vpi_type,
        name: Some(word.text.clone()),
        file: file.to_owned(),
    }
}

/// Lex only enough of a source buffer to find literal identifiers while
/// ignoring comments, strings, and preprocessor directive lines.
fn source_local_words(source: &str) -> Vec<SourceLocalWord> {
    let bytes = source.as_bytes();
    let mut words = Vec::new();
    let mut index = 0;
    let mut line = 1;
    let mut col = 1;
    let mut line_has_text = false;

    while index < bytes.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if ch == '\n' {
            advance_source_char(source, &mut index, &mut line, &mut col);
            line_has_text = false;
            continue;
        }
        if ch.is_whitespace() {
            advance_source_char(source, &mut index, &mut line, &mut col);
            continue;
        }

        if ch == '/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            col += 2;
            while index < bytes.len() {
                let Some(ch) = source[index..].chars().next() else {
                    break;
                };
                if ch == '\n' {
                    break;
                }
                advance_source_char(source, &mut index, &mut line, &mut col);
            }
            continue;
        }
        if ch == '/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            col += 2;
            while index < bytes.len() {
                let Some(ch) = source[index..].chars().next() else {
                    break;
                };
                if ch == '*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 2;
                    col += 2;
                    break;
                } else {
                    advance_source_char(source, &mut index, &mut line, &mut col);
                }
            }
            continue;
        }
        if ch == '"' {
            advance_source_char(source, &mut index, &mut line, &mut col);
            while index < bytes.len() {
                let Some(ch) = source[index..].chars().next() else {
                    break;
                };
                if ch == '\\' {
                    advance_source_char(source, &mut index, &mut line, &mut col);
                    if index < bytes.len() {
                        advance_source_char(source, &mut index, &mut line, &mut col);
                    }
                } else {
                    advance_source_char(source, &mut index, &mut line, &mut col);
                    if ch == '"' {
                        break;
                    }
                }
            }
            line_has_text = true;
            continue;
        }

        // An escaped identifier is a backslash followed by everything up to
        // its terminating whitespace.  In particular, `\endmodule` and
        // `\module` are identifiers, not the corresponding keywords.
        if ch == '\\' {
            words.push(SourceLocalWord {
                text: String::new(),
                line,
                col,
                escaped: true,
            });
            line_has_text = true;
            while index < bytes.len() {
                let Some(ch) = source[index..].chars().next() else {
                    break;
                };
                advance_source_char(source, &mut index, &mut line, &mut col);
                if ch.is_whitespace() {
                    if ch == '\n' {
                        line_has_text = false;
                    }
                    break;
                }
            }
            continue;
        }

        // A source-local fallback must not inspect the body of an include or
        // macro definition.  Directives begin with a backtick as the first
        // non-whitespace character on a line.
        if ch == '`' && !line_has_text {
            let mut continued = false;
            while index < bytes.len() {
                let Some(ch) = source[index..].chars().next() else {
                    break;
                };
                advance_source_char(source, &mut index, &mut line, &mut col);
                if ch == '\n' {
                    if continued {
                        continued = false;
                        continue;
                    }
                    line_has_text = false;
                    break;
                }
                // Treat CRLF as one continued newline when the backslash is
                // immediately before the CR.  Any other character clears
                // the continuation marker, so only a trailing backslash
                // continues a directive.
                if ch == '\r' && continued {
                    continue;
                }
                continued = ch == '\\';
            }
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let start_col = col;
            let start = index;
            index += 1;
            col += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'_'
                    || bytes[index] == b'$')
            {
                index += 1;
                col += 1;
            }
            words.push(SourceLocalWord {
                text: source[start..index].to_owned(),
                line,
                col: start_col,
                escaped: false,
            });
            line_has_text = true;
            continue;
        }

        advance_source_char(source, &mut index, &mut line, &mut col);
        line_has_text = true;
    }

    words
}

fn advance_source_char(source: &str, index: &mut usize, line: &mut u32, col: &mut u32) {
    let Some(ch) = source[*index..].chars().next() else {
        return;
    };
    *index += ch.len_utf8();
    if ch == '\n' {
        *line += 1;
        *col = 1;
    } else {
        *col += ch.len_utf16() as u32;
    }
}

/// Collect both VPI and parse-tree tokens, merging them into a single
/// per-file token list, and return the reference→declaration bindings plus
/// the position-keyed declaration snippets captured during the same VPI walk.
///
/// The bindings map every UHDM-bound reference occurrence (including named
/// port-connection actuals) to the declared object elaboration bound it to,
/// keyed by the 0-based reference position.  Targets without usable
/// coordinates are skipped.  The details map carries one rendered
/// declaration snippet per declared object (see [`DeclDetails`]).
///
/// # Safety
/// `vpi_design` must be a valid UHDM VPI design handle; see `collect_vpi_tokens`.
pub fn collect_all_tokens(
    vpi_design: VpiHandle,
    design: &surelog::Design,
) -> (Vec<FileTokens>, RefBindings, DeclDetails) {
    let mut all_nodes: Vec<VObjectInfo> = Vec::new();
    let mut bindings: RefBindings = HashMap::new();
    let mut details: DeclDetails = HashMap::new();
    walk_design(vpi_design, &mut all_nodes, &mut bindings, &mut details);
    let vpi = group_nodes_by_file(all_nodes);
    // The merged stream deliberately ignores the parse pass's declaration
    // positions: in an elaborated (UHDM) analysis the multi-view token
    // histogram plus `vpiActual` bindings decide DECL vs REF, and forcing
    // parse-side declarations here could contradict them.
    let (parse, _parse_decl_positions) = collect_parse_tokens(design);
    (merge_file_tokens(vpi, parse), bindings, details)
}

fn group_nodes_by_file(all_nodes: Vec<VObjectInfo>) -> Vec<FileTokens> {
    let mut by_file: HashMap<String, Vec<VObjectInfo>> = HashMap::new();
    for node in all_nodes {
        by_file.entry(node.file.clone()).or_default().push(node);
    }
    by_file
        .into_iter()
        .map(|(path, nodes)| FileTokens { path, nodes })
        .collect()
}

// ── Parse-tree helpers ────────────────────────────────────────────────────────

/// Walk up the ancestor chain of a parse-tree node and return the VPI/synthetic
/// type constant that corresponds to the declaration context, or `None` if the
/// identifier is not in a recognised declaration-name position.
///
/// Only transparent identifier-wrapper nodes (`paSimple_identifier`,
/// `paIdentifier`, `paEscaped_identifier`, etc.) are traversed; any other
/// non-declaration node type terminates the walk without classification.
/// This prevents identifiers inside expression or statement contexts from
/// being mistakenly classified as declaration names.
///
/// `parent_index` is the raw `NodeId` (0 = `InvalidRawNodeId` = no parent).
pub fn classify_identifier_ancestor(
    nodes: &[surelog::ParseNode],
    mut parent_idx: u32,
    sym: &str,
    declarations: &HashMap<String, i32>,
) -> Option<(i32, bool)> {
    const MAX_DEPTH: usize = 8; // guard against degenerate or cyclic trees
    for _ in 0..MAX_DEPTH {
        if parent_idx == 0 {
            return None; // reached the root or no-parent sentinel
        }
        let idx = parent_idx as usize;
        if idx >= nodes.len() {
            return None; // malformed index
        }
        match VObjectType::try_from(nodes[idx].type_id) {
            // ── Declaration-context types → classify and return ───────────
            Ok(t)
                if t == VObjectType::paModule_ansi_header
                    || t == VObjectType::paModule_nonansi_header =>
            {
                return Some((vpi::vpiModule, true));
            }
            Ok(VObjectType::paClass_declaration) => {
                return Some((vpi::uhdmclass_defn, true));
            }
            Ok(VObjectType::paModule_instantiation) => {
                return Some((vpi::uhdmclass_defn, false));
            }
            Ok(VObjectType::paPackage_declaration) => {
                return Some((vpi::uhdmpackage, true));
            }
            Ok(t)
                if t == VObjectType::paInterface_ansi_header
                    || t == VObjectType::paInterface_nonansi_header
                    || t == VObjectType::paInterface_declaration =>
            {
                return Some((vpi::uhdminterface_inst, true));
            }
            Ok(t)
                if t == VObjectType::paFunction_declaration
                    || t == VObjectType::paFunction_body_declaration =>
            {
                return Some((vpi::vpiFunction, true));
            }
            Ok(t)
                if t == VObjectType::paTask_declaration
                    || t == VObjectType::paTask_body_declaration =>
            {
                return Some((vpi::vpiTask, true));
            }
            Ok(t)
                if t == VObjectType::paNet_declaration
                    || t == VObjectType::paNet_decl_assignment
                    || t == VObjectType::paNet_lvalue =>
            {
                return Some((vpi::vpiNet, true));
            }
            Ok(t)
                if t == VObjectType::paPort_declaration
                    || t == VObjectType::paAnsi_port_declaration =>
            {
                return Some((vpi::vpiPort, true));
            }
            Ok(t)
                if t == VObjectType::paParameter_declaration
                    || t == VObjectType::paLocal_parameter_declaration
                    || t == VObjectType::paParam_assignment =>
            {
                return Some((vpi::vpiParameter, true));
            }
            Ok(VObjectType::paNamed_parameter_assignment) => {
                return Some((vpi::TOKEN_PARAM_CONN_LABEL, false));
            }
            Ok(t)
                if t == VObjectType::paData_declaration
                    || t == VObjectType::paVariable_declaration
                    || t == VObjectType::paVariable_lvalue
                    || t == VObjectType::paVariable_decl_assignment
                    || t == VObjectType::paName_of_instance =>
            {
                return Some((vpi::uhdmlogic_var, true));
            }
            Ok(VObjectType::paType_declaration) => {
                return Some((vpi::TOKEN_TYPEDEF_NAME, true));
            }
            Ok(VObjectType::paEnum_name_declaration) => {
                return Some((vpi::uhdmenum_const, true));
            }
            // Connection labels carry dedicated LSP-internal synthetic types
            // (see `ffi::vpi::TOKEN_*_CONN_LABEL`): `semantic_tokens::encode`
            // renders them as their usual token type plus the `connectionLabel`
            // modifier, and consumers can tell a `.label` from an ordinary
            // identifier structurally instead of by position.  Both flavors are
            // reference sites by construction (`is_decl == false`): a port
            // label names the child module's port, an override label the
            // child's parameter.
            Ok(VObjectType::paNamed_port_connection) => {
                return Some((vpi::TOKEN_PORT_CONN_LABEL, false));
            }
            Ok(VObjectType::paStructure_pattern_key) => {
                return Some((VObjectTypeShifted::paStructure_pattern_key.into(), false));
            }
            // slStringConst usually means the signal is used before declaration.
            Ok(t) if t == VObjectType::paPrimary_literal || t == VObjectType::paPrimary => {
                return declarations.get(sym).map(|&vt| (vt, false));
            }
            // ── Transparent identifier wrappers → keep walking ────────────
            Ok(t)
                if t == VObjectType::paSimple_identifier
                    || t == VObjectType::paIdentifier
                    || t == VObjectType::paEscaped_identifier
                    || t == VObjectType::paHierarchical_identifier
                    || t == VObjectType::paInterface_identifier
                    || t == VObjectType::paPs_identifier
                    || t == VObjectType::paPs_type_identifier
                    || t == VObjectType::paPs_or_hierarchical_identifier
                    || t == VObjectType::paComplex_func_call =>
            {
                parent_idx = nodes[idx].parent_index;
            }

            Err(_) => {
                return None;
            }
            // ── Any other node type → not a declaration name, stop ────────
            _ => {
                return None;
            }
        }
    }
    None
}

/// Return the lowercase keyword text for a PARSE_OFFSET-shifted type ID, or
/// `None` if the type is not a recognised keyword.
///
/// The returned string is a `'static` slice — no allocation.
fn keyword_text_for_raw_type(raw_type: i32) -> Option<&'static str> {
    match raw_type {
        // ── Module / scope ───────────────────────────────────────────────────
        // The `module`/`macromodule` declaration keyword (Surelog types the
        // keyword node itself `paModule_keyword`, distinct from the
        // `paMODULE` design element).
        t if t == VObjectTypeShifted::paModule_keyword => Some("module"),
        t if t == VObjectTypeShifted::paENDMODULE => Some("endmodule"),
        t if t == VObjectTypeShifted::paPACKAGE => Some("package"),
        t if t == VObjectTypeShifted::paENDPACKAGE => Some("endpackage"),
        t if t == VObjectTypeShifted::paINTERFACE => Some("interface"),
        t if t == VObjectTypeShifted::paENDINTERFACE => Some("endinterface"),
        t if t == VObjectTypeShifted::paCLASS => Some("class"),
        t if t == VObjectTypeShifted::paENDCLASS => Some("endclass"),
        t if t == VObjectTypeShifted::paPROGRAM => Some("program"),
        t if t == VObjectTypeShifted::paENDPROGRAM => Some("endprogram"),
        t if t == VObjectTypeShifted::paFUNCTION => Some("function"),
        t if t == VObjectTypeShifted::paENDFUNCTION => Some("endfunction"),
        t if t == VObjectTypeShifted::paTASK => Some("task"),
        t if t == VObjectTypeShifted::paENDTASK => Some("endtask"),
        t if t == VObjectTypeShifted::paGENERATE => Some("generate"),
        t if t == VObjectTypeShifted::paENDGENERATE => Some("endgenerate"),
        // ── Port direction ───────────────────────────────────────────────────
        t if t == VObjectTypeShifted::paINPUT => Some("input"),
        t if t == VObjectTypeShifted::paOUTPUT => Some("output"),
        t if t == VObjectTypeShifted::paINOUT => Some("inout"),
        // ── Net type ─────────────────────────────────────────────────────────
        t if t == VObjectTypeShifted::paWIRE => Some("wire"),
        t if t == VObjectTypeShifted::paWAND => Some("wand"),
        t if t == VObjectTypeShifted::paWOR => Some("wor"),
        t if t == VObjectTypeShifted::paUWIRE => Some("uwire"),
        t if t == VObjectTypeShifted::paTRI => Some("tri"),
        t if t == VObjectTypeShifted::paTRI0 => Some("tri0"),
        t if t == VObjectTypeShifted::paTRI1 => Some("tri1"),
        t if t == VObjectTypeShifted::paTRIAND => Some("triand"),
        t if t == VObjectTypeShifted::paTRIOR => Some("trior"),
        t if t == VObjectTypeShifted::paTRIREG => Some("trireg"),
        t if t == VObjectTypeShifted::paSUPPLY0 => Some("supply0"),
        t if t == VObjectTypeShifted::paSUPPLY1 => Some("supply1"),
        t if t == VObjectTypeShifted::paREG => Some("reg"),
        // ── Data type ────────────────────────────────────────────────────────
        t if t == VObjectTypeShifted::paLOGIC => Some("logic"),
        t if t == VObjectTypeShifted::paBIT => Some("bit"),
        t if t == VObjectTypeShifted::paBYTE => Some("byte"),
        t if t == VObjectTypeShifted::paSHORTINT => Some("shortint"),
        t if t == VObjectTypeShifted::paINT => Some("int"),
        t if t == VObjectTypeShifted::paLONGINT => Some("longint"),
        t if t == VObjectTypeShifted::paINTEGER => Some("integer"),
        t if t == VObjectTypeShifted::paREAL => Some("real"),
        t if t == VObjectTypeShifted::paSHORTREAL => Some("shortreal"),
        t if t == VObjectTypeShifted::paREALTIME => Some("realtime"),
        t if t == VObjectTypeShifted::paTIME => Some("time"),
        t if t == VObjectTypeShifted::paCHANDLE => Some("chandle"),
        t if t == VObjectTypeShifted::paSTRING => Some("string"),
        t if t == VObjectTypeShifted::paVOID => Some("void"),
        t if t == VObjectTypeShifted::paGENVAR => Some("genvar"),
        t if t == VObjectTypeShifted::paENUM => Some("enum"),
        t if t == VObjectTypeShifted::paSTRUCT => Some("struct"),
        t if t == VObjectTypeShifted::paUNION => Some("union"),
        t if t == VObjectTypeShifted::paTYPEDEF => Some("typedef"),
        t if t == VObjectTypeShifted::paTYPE => Some("type"),
        // ── Qualifier ────────────────────────────────────────────────────────
        t if t == VObjectTypeShifted::paPARAMETER => Some("parameter"),
        t if t == VObjectTypeShifted::paLOCALPARAM => Some("localparam"),
        t if t == VObjectTypeShifted::paDEFPARAM => Some("defparam"),
        t if t == VObjectTypeShifted::paSTATIC => Some("static"),
        t if t == VObjectTypeShifted::paAUTOMATIC => Some("automatic"),
        t if t == VObjectTypeShifted::paVIRTUAL => Some("virtual"),
        t if t == VObjectTypeShifted::paEXTENDS => Some("extends"),
        t if t == VObjectTypeShifted::paIMPLEMENTS => Some("implements"),
        _ => None,
    }
}

/// Merge two `Vec<FileTokens>` into one, combining nodes for the same path.
fn merge_file_tokens(mut a: Vec<FileTokens>, b: Vec<FileTokens>) -> Vec<FileTokens> {
    // Build a path → index map for `a` so we can extend existing entries.
    let mut idx: HashMap<String, usize> = a
        .iter()
        .enumerate()
        .map(|(i, ft)| (ft.path.clone(), i))
        .collect();

    for ft in b {
        if let Some(&i) = idx.get(&ft.path) {
            a[i].nodes.extend(ft.nodes);
        } else {
            idx.insert(ft.path.clone(), a.len());
            a.push(ft);
        }
    }
    a
}

// ── VPI tree walker ───────────────────────────────────────────────────────────

/// Emit a `VObjectInfo` for `h` if it has a valid source location.
fn maybe_emit(h: VpiHandle, result: &mut Vec<VObjectInfo>) {
    // Skip objects with no line information.
    let line = vpi::get(vpi::vpiLineNo, h) as u32;
    if line == 0 {
        return;
    }

    let file = vpi::get_str(vpi::vpiFile, h);
    if file.is_empty() {
        return;
    }

    let col = vpi::get(vpi::vpiColumnNo, h) as u32;
    let end_line = vpi::get(vpi::vpiEndLineNo, h) as u32;
    let end_col = vpi::get(vpi::vpiEndColumnNo, h) as u32;
    let vpi_type = vpi::get(vpi::vpiType, h);
    let name_str = vpi::get_str(vpi::vpiName, h);
    let name = if name_str.is_empty() {
        None
    } else {
        Some(name_str)
    };

    result.push(VObjectInfo {
        line,
        col,
        end_line,
        end_col,
        vpi_type,
        name,
        file,
    });
}

/// Like `maybe_emit` but records `forced_vpi_type` instead of querying the
/// object's own type.  Used when the token category is determined by context
/// (e.g. port direction, module instance vs. module definition) rather than
/// the raw VPI object type.
fn maybe_emit_as(h: VpiHandle, forced_vpi_type: i32, result: &mut Vec<VObjectInfo>) {
    let line = vpi::get(vpi::vpiLineNo, h) as u32;
    if line == 0 {
        return;
    }

    let file = vpi::get_str(vpi::vpiFile, h);
    if file.is_empty() {
        return;
    }

    let col = vpi::get(vpi::vpiColumnNo, h) as u32;
    let end_line = vpi::get(vpi::vpiEndLineNo, h) as u32;
    let end_col = vpi::get(vpi::vpiEndColumnNo, h) as u32;
    let name_str = vpi::get_str(vpi::vpiName, h);
    let name = if name_str.is_empty() {
        None
    } else {
        Some(name_str)
    };

    result.push(VObjectInfo {
        line,
        col,
        end_line,
        end_col,
        vpi_type: forced_vpi_type,
        name,
        file,
    });
}

/// Emit a reference leaf like `maybe_emit` and record its elaboration binding.
///
/// The binding key is derived from the *emitted* token's file/line/column
/// (converted to 0-based), so keys and reference-token positions align by
/// construction.  When the bound target (`vpiActual`) lacks usable
/// coordinates the reference is still emitted but no binding is recorded.
fn emit_ref(h: VpiHandle, result: &mut Vec<VObjectInfo>, bindings: &mut RefBindings) {
    let before = result.len();
    maybe_emit(h, result);
    if result.len() == before {
        return; // nothing emitted → no position to key a binding on
    }
    let emitted = result.last().expect("token just pushed");
    let key_file = emitted.file.clone();
    let key_line = emitted.line.saturating_sub(1);
    let key_col = emitted.col.saturating_sub(1);

    let Some(actual) = vpi::handle(vpi::vpiActual, h) else {
        return;
    };
    let raw = actual.raw();
    let name = vpi::obj_name(raw);
    let kind = decl_kind_label(vpi::obj_type(raw));
    let line1 = vpi::get(vpi::vpiLineNo, raw) as u32;
    let col1 = vpi::get(vpi::vpiColumnNo, raw) as u32;
    let file = vpi::get_str(vpi::vpiFile, raw);
    if let Some(target) = DeclTarget::from_decl_coords(&name, kind, &file, line1, col1) {
        bindings.insert((key_file, key_line, key_col), target);
    }
}

/// Short human label for a declared object's VPI type, stored in
/// [`DeclTarget::kind`].
fn decl_kind_label(t: i32) -> &'static str {
    use crate::ffi::vpi;
    match t {
        vpi::vpiPort | vpi::vpiPortBit => "port",
        vpi::vpiParameter | vpi::vpiSpecParam | vpi::uhdmparameter => "parameter",
        vpi::vpiNet
        | vpi::vpiNetBit
        | vpi::vpiReg
        | vpi::vpiRegBit
        | vpi::uhdmnet
        | vpi::uhdmlogic_net => "net",
        vpi::vpiModule | vpi::uhdmmodule_inst | vpi::uhdminterface_inst => "module",
        vpi::vpiFunction | vpi::uhdmfunction => "function",
        vpi::vpiTask | vpi::uhdmtask => "task",
        _ => "var",
    }
}

/// Port-direction word for the hover snippet; `None` renders no prefix.
fn direction_text(dir: i32) -> Option<&'static str> {
    match dir {
        vpi::vpiInput => Some("input"),
        vpi::vpiOutput => Some("output"),
        vpi::vpiInout => Some("inout"),
        _ => None,
    }
}

/// Render one declared object's snippet (`logic [3:0] val`,
/// `input logic [1:0] sel`, …) and key it at its 1-based source position.
///
/// Skipped silently for objects without a usable position/name — those cannot
/// anchor an unambiguous hover either.  The rendering mirrors
/// `core::model::TypeInfo::render()`: `logic [7:0]` for ranged logic/bit,
/// bare kind otherwise, `signed` prefix only for non-integer signed kinds.
/// First writer wins: UHDM frequently exposes a port AND its underlying
/// variable at the same source position, and the port view (recorded first,
/// with its direction) is the more informative one.
fn record_decl_detail(h: VpiHandle, direction: Option<&str>, details: &mut DeclDetails) {
    let line = vpi::get(vpi::vpiLineNo, h) as u32;
    let col = vpi::get(vpi::vpiColumnNo, h) as u32;
    if line == 0 || col == 0 {
        return;
    }
    let file = vpi::get_str(vpi::vpiFile, h);
    if file.is_empty() {
        return;
    }
    let name = vpi::get_str(vpi::vpiName, h);
    if name.is_empty() {
        return;
    }
    let ty = decl_type_text(h);
    let text = match direction {
        Some(dir) => format!("{dir} {ty} {name}"),
        None => format!("{ty} {name}"),
    };
    details.entry((file, line, col)).or_insert(text);
}

/// Type text of a declared object from its typespec (`logic [7:0]`, `int`,
/// `enum state_t`, …), mirroring `core::model::TypeInfo::render()`.
///
/// Follows `ref_typespec → vpiActual` chains (guarded against cycles) like
/// `core::db`; falls back to the object discriminator for the Surelog quirk
/// where real/shortreal variables carry no typespec handle.
fn decl_type_text(h: VpiHandle) -> String {
    const MAX_CHAIN: usize = 8;
    let mut ts = match vpi::handle(vpi::vpiTypespec, h).or_else(|| vpi::handle(vpi::vpiTypedef, h))
    {
        Some(ts) => ts,
        None => return fallback_var_text(h),
    };
    for _ in 0..MAX_CHAIN {
        if vpi::obj_type(ts.raw()) != vpi::vpiRefTypespec {
            break;
        }
        match ts.child(vpi::vpiActual) {
            Some(actual) => {
                ts = actual;
            }
            None => return fallback_var_text(h),
        }
    }
    concrete_type_text(ts.raw()).unwrap_or_else(|| fallback_var_text(h))
}

/// Type text of a concrete (non-ref) typespec object.
fn concrete_type_text(ts: VpiHandle) -> Option<String> {
    let signed = vpi::get(vpi::vpiSigned, ts) != 0;
    let type_name = {
        let n = vpi::obj_name(ts);
        (!n.is_empty()).then_some(n)
    };
    let width = range_width(ts);
    let base = match vpi::obj_type(ts) {
        vpi::vpiIntTypespec => "int".to_owned(),
        vpi::vpiIntegerTypespec => "integer".to_owned(),
        vpi::vpiTimeTypespec => "time".to_owned(),
        vpi::vpiLongIntTypespec => "longint".to_owned(),
        vpi::vpiByteTypespec => "byte".to_owned(),
        vpi::vpiShortIntTypespec => "shortint".to_owned(),
        vpi::vpiLogicTypespec => format_ranged("logic", signed, width),
        vpi::vpiBitTypespec => format_ranged("bit", signed, width),
        vpi::vpiEnumTypespec => match type_name {
            Some(n) => format!("enum {n}"),
            None => "enum".to_owned(),
        },
        vpi::vpiStructTypespec => match type_name {
            Some(n) => format!("struct {n}"),
            None => "struct".to_owned(),
        },
        vpi::vpiUnionTypespec => match type_name {
            Some(n) => format!("union {n}"),
            None => "union".to_owned(),
        },
        vpi::vpiStringTypespec => "string".to_owned(),
        vpi::vpiRealTypespec => "real".to_owned(),
        vpi::vpiShortRealTypespec => "shortreal".to_owned(),
        vpi::vpiClassTypespec => match type_name {
            Some(n) => format!("class {n}"),
            None => "class".to_owned(),
        },
        vpi::vpiArrayTypespec | vpi::vpiPackedArrayTypespec => "array".to_owned(),
        vpi::vpiVoidTypespec => "void".to_owned(),
        vpi::vpiChandleTypespec => "chandle".to_owned(),
        _ => return None,
    };
    Some(base)
}

/// Ranged scalar rendering (`logic [7:0]`); mirrors `TypeInfo::render()`'s
/// `[width-1:0]` convention with the `signed` prefix for non-integer kinds.
fn format_ranged(kind: &str, signed: bool, width: Option<u32>) -> String {
    let prefix = if signed { "signed " } else { "" };
    match width {
        Some(w) if w > 1 => format!("{prefix}{kind} [{}:0]", w - 1),
        _ => format!("{prefix}{kind}"),
    }
}

/// Packed width across all `vpiRange`s (`|left - right| + 1`, multiplied for
/// multi-dimension ranges); `None` when a bound is not a plain constant.
fn range_width(ts: VpiHandle) -> Option<u32> {
    let mut total: u64 = 1;
    let mut any = false;
    for r in vpi::iterate(vpi::vpiRange, ts).into_iter().flatten() {
        any = true;
        let left = range_bound(vpi::vpiLeftRange, r.raw())?;
        let right = range_bound(vpi::vpiRightRange, r.raw())?;
        total = total.saturating_mul((left - right).unsigned_abs() + 1);
    }
    if any {
        Some(total.min(u32::MAX as u64) as u32)
    } else {
        Some(1)
    }
}

/// One range bound as a clean integer; `None` unless it is a plain
/// Int/UInt/Scalar constant (elaborated output folds every bound to those).
fn range_bound(rel: i32, r: VpiHandle) -> Option<i64> {
    let b = vpi::handle(rel, r)?;
    match vpi::read_value(b.raw()) {
        crate::ffi::vpi::ValueData::Int(v) => Some(v),
        crate::ffi::vpi::ValueData::UInt(v) => Some(v as i64),
        crate::ffi::vpi::ValueData::Scalar(v) => Some(v as i64),
        _ => None,
    }
}

/// Fallback for objects whose typespec is unavailable: shortreal/real
/// variables keep their scalar kind from the discriminator, everything else
/// degrades to `var`-style text keyed on nothing better than `logic`-less
/// `unknown`.
fn fallback_var_text(h: VpiHandle) -> String {
    match vpi::obj_type(h) {
        vpi::vpiRealVar => "real".to_owned(),
        vpi::vpiShortRealVar => "shortreal".to_owned(),
        vpi::vpiIntegerVar => "integer".to_owned(),
        vpi::vpiTimeVar => "time".to_owned(),
        vpi::vpiStringVar => "string".to_owned(),
        vpi::vpiByteVar => "byte".to_owned(),
        vpi::vpiShortIntVar => "shortint".to_owned(),
        vpi::vpiLongIntVar => "longint".to_owned(),
        vpi::vpiIntVar => "int".to_owned(),
        _ => "var".to_owned(),
    }
}

/// Relationships walked when descending expression and statement trees.  The
/// set covers operands, call arguments, assignment sides, conditions, select
/// indexes and statement children (process bodies), so references inside
/// complex expressions and procedural code are all captured.  `vpiActual` is
/// deliberately NOT descended: it points at the *bound declaration*, not at
/// more reference occurrences.
const REF_DESCENT_RELS: &[i32] = &[
    vpi::vpiOperand,
    vpi::vpiArgument,
    vpi::vpiLhs,
    vpi::vpiRhs,
    vpi::vpiCondition,
    vpi::vpiStmt,
    vpi::vpiElseStmt,
    vpi::vpiIndex,
    vpi::vpiExpr,
];

/// Visit every child reachable via `rel` — works for both 1-to-many
/// (`vpi_iterate`) and 1-to-1 (`vpi_handle`) relationships.
fn each_rel<F: FnMut(VpiHandle)>(rel: i32, obj: VpiHandle, f: &mut F) {
    if let Some(it) = vpi::iterate(rel, obj) {
        for h in it {
            f(h.raw());
        }
    } else if let Some(h) = vpi::handle(rel, obj) {
        // `h` (OwnedHandle) stays alive until `f` returns.
        f(h.raw());
    }
}

/// Recursively walk an expression or statement handle, emitting any
/// signal-reference leaves (`uhdmref_obj`, `uhdmref_var`, `vpiRefObj`,
/// `vpiParameter`) together with their elaboration bindings.
///
/// Recurses through [`REF_DESCENT_RELS`] so that references inside complex
/// expressions (concatenations, function calls, conditional expressions,
/// part-selects, …) and inside process statement trees are all captured.
/// Statement nodes that introduce a named scope ([`SCOPE_STMT_TYPES`])
/// additionally contribute their local declarations.
///
/// # Macros
/// Preprocessor macro expansion sites are not visible through the VPI/UHDM
/// interface; macro highlighting is therefore not supported at this layer.
fn walk_expr(
    h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
) {
    walk_expr_depth(h, result, bindings, details, seen, 0);
}

fn walk_expr_depth(
    h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
    depth: u32,
) {
    if h.is_null() || depth > 64 {
        return;
    }

    let typ = vpi::get(vpi::vpiType, h);

    // Named begin/fork blocks carry their own declaration scope; emit their
    // locals before descending into the statement tree (the descent below
    // reaches nested blocks again, guarded by `seen`).
    if SCOPE_STMT_TYPES.contains(&typ) {
        walk_block_scope_objects(h, result, bindings, details, seen);
    }

    // Emit signal/variable reference leaves directly.
    let ref_types = [
        vpi::uhdmref_obj,
        vpi::uhdmref_var,
        vpi::vpiParameter,
        vpi::vpiRefObj,
    ];
    if ref_types.contains(&typ) {
        emit_ref(h, result, bindings);
        return; // ref nodes have no meaningful sub-structure to descend
    }

    for rel in REF_DESCENT_RELS {
        each_rel(*rel, h, &mut |child| {
            walk_expr_depth(child, result, bindings, details, seen, depth + 1)
        });
    }
}

/// Walk one scope object (module, interface, package, class, function, task,
/// or generate block) and collect all named children.
///
/// The scope itself is emitted first, then its children in VPI relationship
/// order.  Processes, generate scopes and task/function bodies are recursed
/// into; every declared child (ports, nets, variables, parameters) also
/// records its rendered snippet in `details`.  Direct child module instances
/// contribute only their *name* token plus the reference occurrences of their
/// named port connections (the `vpiHighConn` expressions) and of their
/// OVERRIDDEN parameter assignments (the `vpiParamAssign` RHS hanging off the
/// INSTANCE handle): their nets/vars/processes keep the module definition's
/// source coordinates and are already collected through the definition-level
/// walk, while emitting their declaration tokens at the instantiation site
/// would misclassify connection labels.
///
/// `seen` guards against walking one scope object twice (a named begin/fork
/// block is reachable both from its process statement tree and from the
/// enclosing scope's `vpiInternalScope`; a generate scope array likewise).
/// `seen` guards against walking one scope object twice (a named begin/fork
/// block is reachable both from its process statement tree and from the
/// enclosing scope's `vpiInternalScope`; a generate scope likewise).  The key
/// is the object's `(vpiType, vpiFullName, line, col)` identity rather than
/// the raw handle: UHDM recycles freed handle addresses within one process,
/// so pointer equality would wrongly skip scopes whose address collides with
/// a handle released by an earlier VPI traversal (e.g. the db build).
type ScopeKey = (i32, String, u32, u32);

fn scope_key(h: VpiHandle) -> ScopeKey {
    (
        vpi::get(vpi::vpiType, h),
        vpi::obj_full_name(h),
        vpi::get(vpi::vpiLineNo, h) as u32,
        vpi::get(vpi::vpiColumnNo, h) as u32,
    )
}

fn walk_scope(
    scope_h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
) {
    if !seen.insert(scope_key(scope_h)) {
        return;
    }
    // Emit the scope definition (module/interface/package/class name token).
    maybe_emit(scope_h, result);

    // ── Child module instances ─────────────────────────────────────────────────
    // vpiTopModule on a scope returns any hierarchically-top sub-instances;
    // emit these with their natural type.
    for h in vpi::iterate(vpi::vpiTopModule, scope_h)
        .into_iter()
        .flatten()
    {
        maybe_emit(h.raw(), result);
    }
    // Non-top child module instances are emitted as `uhdmmodule_inst` so that
    // editors can highlight instantiations differently from module definitions.
    for h in vpi::iterate(vpi::vpiModule, scope_h).into_iter().flatten() {
        maybe_emit_as(h.raw(), vpi::uhdmmodule_inst, result);
        walk_port_high_conns(h.raw(), result, bindings, details, seen);
        walk_instance_param_overrides(h.raw(), result, bindings, details, seen);
    }

    // ── Ports (with direction-specific token types) ────────────────────────────
    // Use LSP-internal synthetic type IDs so that input/output/inout ports can
    // be coloured differently.  Falls back to generic `vpiPort` when direction
    // information is unavailable.
    for h in vpi::iterate(vpi::vpiPort, scope_h).into_iter().flatten() {
        let dir = vpi::get(vpi::vpiDirection, h.raw());
        let synthetic = if dir == vpi::vpiInput {
            vpi::TOKEN_PORT_INPUT
        } else if dir == vpi::vpiOutput {
            vpi::TOKEN_PORT_OUTPUT
        } else if dir == vpi::vpiInout {
            vpi::TOKEN_PORT_INOUT
        } else {
            vpi::vpiPort
        };
        maybe_emit_as(h.raw(), synthetic, result);
        record_decl_detail(h.raw(), direction_text(dir), details);
    }

    // ── Nets (wire / logic_net) ───────────────────────────────────────────────
    for h in vpi::iterate(vpi::vpiNet, scope_h).into_iter().flatten() {
        maybe_emit(h.raw(), result);
        record_decl_detail(h.raw(), None, details);
    }

    // ── Parameter ─────────────────────────────────────────────────────────────
    for h in vpi::iterate(vpi::vpiParameter, scope_h)
        .into_iter()
        .flatten()
    {
        maybe_emit(h.raw(), result);
        record_decl_detail(h.raw(), None, details);
    }

    // ── Logic variables (UHDM extension iterator) ─────────────────────────────
    for h in vpi::iterate(vpi::vpiLogicVar, scope_h)
        .into_iter()
        .flatten()
    {
        maybe_emit(h.raw(), result);
        record_decl_detail(h.raw(), None, details);
    }

    // ── Other variables (int_var, real_var, …) ────────────────────────────────
    for h in vpi::iterate(vpi::vpiVariables, scope_h)
        .into_iter()
        .flatten()
    {
        maybe_emit(h.raw(), result);
        record_decl_detail(h.raw(), None, details);
    }

    // ── Parameters (UHDM extension iterator) ──────────────────────────────────
    for h in vpi::iterate(vpi::vpiParameters, scope_h)
        .into_iter()
        .flatten()
    {
        maybe_emit(h.raw(), result);
        record_decl_detail(h.raw(), None, details);
    }

    // ── Continuous assignments — collect signal references from LHS / RHS ──────
    for vpi_type in [vpi::vpiContAssign, vpi::vpiParamAssign] {
        for assign_h in vpi::iterate(vpi_type, scope_h).into_iter().flatten() {
            if let Some(lhs) = vpi::handle(vpi::vpiLhs, assign_h.raw()) {
                walk_expr(lhs.raw(), result, bindings, details, seen);
            }
            if let Some(rhs) = vpi::handle(vpi::vpiRhs, assign_h.raw()) {
                walk_expr(rhs.raw(), result, bindings, details, seen);
            }
        }
    }

    // ── Processes (always/initial) — collect references from their bodies ──────
    for proc_h in vpi::iterate(vpi::vpiProcess, scope_h).into_iter().flatten() {
        each_rel(vpi::vpiStmt, proc_h.raw(), &mut |stmt| {
            walk_expr(stmt, result, bindings, details, seen);
        });
    }

    // ── Tasks and functions (recurse to capture their local declarations) ──────
    // The body statement hangs off the task/function handle itself (`vpiStmt`)
    // and is NOT covered by a process iteration, so it is descended explicitly:
    // this is what gives references inside function/task bodies their UHDM
    // elaboration bindings instead of falling through to the name-based index
    // resolution (which resolves to the wrong declaration under shadowing).
    for h in vpi::iterate(vpi::vpiTaskFunc, scope_h)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, seen);
        each_rel(vpi::vpiStmt, h.raw(), &mut |stmt| {
            walk_expr(stmt, result, bindings, details, seen);
        });
    }

    // ── Internal named scopes (named begin/fork blocks) ────────────────────────
    for h in vpi::iterate(vpi::vpiInternalScope, scope_h)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, seen);
    }

    // ── Generate scopes (gen_scope_array → gen_scope) ──────────────────────────
    // Generate-block interiors are NOT reachable through `vpiInternalScope`;
    // per the UHDM model they hang off `vpiGenScopeArray`, and each array's
    // per-iteration `gen_scope` objects expose the block's nets/variables/
    // processes/continuous assignments.  Walking them here gives the
    // generate-local declarations their tokens AND binding-precise references
    // inside the block (`assign val = ...` in a genblk).
    for gsa in vpi::iterate(vpi::vpiGenScopeArray, scope_h)
        .into_iter()
        .flatten()
    {
        for gs in vpi::iterate(vpi::vpiGenScope, gsa.raw())
            .into_iter()
            .flatten()
        {
            walk_scope(gs.raw(), result, bindings, details, seen);
        }
    }

    for h in vpi::iterate(vpi::uhdmref_obj, scope_h)
        .into_iter()
        .flatten()
    {
        emit_ref(h.raw(), result, bindings);
    }
}

/// Statement object types that introduce an inner NAMED SCOPE when they appear
/// inside a process statement tree (`begin : blk`, `fork : fk`, and plain
/// unnamed begin/fork which may still declare block items).  `vpi_get(vpiType)`
/// reports the standard VPI-mapped constants here (Surelog maps its
/// `named_begin`/`named_fork` UHDM objects onto `vpiNamedBegin`/`vpiNamedFork`),
/// NOT the `uhdm*` discriminants.  Their local declarations are not reachable
/// from the enclosing scope's iterators, so [`walk_expr_depth`] emits them when
/// it descends into such a statement.
const SCOPE_STMT_TYPES: &[i32] = &[
    vpi::vpiNamedBegin,
    vpi::vpiNamedFork,
    vpi::vpiBegin,
    vpi::vpiFork,
];

/// Emit the declared objects of one statement-introduced scope (named
/// begin/fork block): local variables, nets and parameters, plus any nested
/// internal scopes.  Guarded by `seen` exactly like [`walk_scope`] so a block
/// also reachable via `vpiInternalScope` is only ever walked once.
fn walk_block_scope_objects(
    block_h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
) {
    if !seen.insert(scope_key(block_h)) {
        return;
    }
    for rel in [
        vpi::vpiNet,
        vpi::vpiParameter,
        vpi::vpiLogicVar,
        vpi::vpiReg,
        vpi::vpiParameters,
        vpi::vpiVariables,
    ] {
        for h in vpi::iterate(rel, block_h).into_iter().flatten() {
            maybe_emit(h.raw(), result);
            record_decl_detail(h.raw(), None, details);
        }
    }
    for h in vpi::iterate(vpi::vpiInternalScope, block_h)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, seen);
    }
}

/// Walk the `vpiHighConn` (parent-side connection expression) of every port
/// of an instance, emitting reference leaves plus their elaboration bindings.
///
/// The high connections live at the *instantiating* file's port-connection
/// positions (`.clk(wa)`); elaboration binds them to the parent-scope object,
/// so a definition request on the connection actual lands on that object's
/// declaration.  Low connections are deliberately skipped: they point at the
/// child-side objects whose declaration tokens are collected through the
/// definition-level walk.
fn walk_port_high_conns(
    h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
) {
    for port in vpi::iterate(vpi::vpiPort, h).into_iter().flatten() {
        if let Some(high) = vpi::handle(vpi::vpiHighConn, port.raw()) {
            walk_expr(high.raw(), result, bindings, details, seen);
        }
    }
}

/// Walk the RHS of every OVERRIDDEN parameter assignment hanging directly off
/// an instance handle, emitting reference leaves plus their elaboration
/// bindings.
///
/// Named overrides (`child u0 #(.W(PARENT_SIG)) (...)`) live ONLY on the
/// INSTANCE (`vpiParamAssign` per instance; the definition-level scope walk
/// sees just the default-value assignments), and their RHS expressions sit at
/// the *instantiating* file's override positions — elaboration binds them to
/// the parent-scope objects, so a definition request inside `.W(...)` lands
/// on that object's declaration.  Non-overridden assignments are skipped:
/// their RHS is the definition-file default expression, already collected by
/// the definition-level walk, and re-walking it here would duplicate tokens.
fn walk_instance_param_overrides(
    h: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
    seen: &mut HashSet<ScopeKey>,
) {
    for pa in vpi::iterate(vpi::vpiParamAssign, h).into_iter().flatten() {
        if vpi::get(vpi::vpiOverriden, pa.raw()) == 0 {
            continue;
        }
        if let Some(rhs) = vpi::handle(vpi::vpiRhs, pa.raw()) {
            walk_expr(rhs.raw(), result, bindings, details, seen);
        }
    }
}

/// Top-level walker: iterate all design-level scopes and delegate to
/// `walk_scope`.
fn walk_design(
    design: VpiHandle,
    result: &mut Vec<VObjectInfo>,
    bindings: &mut RefBindings,
    details: &mut DeclDetails,
) {
    let mut seen: HashSet<ScopeKey> = HashSet::new();
    // All module definitions.
    for h in vpi::iterate(vpi::uhdmallModules, design)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, &mut seen);
    }

    // All interface definitions.
    for h in vpi::iterate(vpi::uhdmallInterfaces, design)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, &mut seen);
    }

    // All package definitions.
    for h in vpi::iterate(vpi::uhdmallPackages, design)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, &mut seen);
    }

    // All class definitions.
    for h in vpi::iterate(vpi::uhdmallClasses, design)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, &mut seen);
    }

    // All program definitions (SystemVerilog programs).
    for h in vpi::iterate(vpi::uhdmallPrograms, design)
        .into_iter()
        .flatten()
    {
        walk_scope(h.raw(), result, bindings, details, &mut seen);
    }

    // Generate scopes hang off the ELABORATED INSTANCE tree (`uhdmallModules`
    // definition handles expose none), so the tops are visited here solely to
    // reach their `vpiGenScopeArray`s.  Each per-iteration `gen_scope` is then
    // walked like any scope, giving generate-local declarations their tokens
    // and binding-precise references inside the block.  Child module instances
    // are descended only for their own gen scopes — their other contents keep
    // the definition coordinates already collected above.
    fn walk_instance_gen_scopes(
        inst: VpiHandle,
        result: &mut Vec<VObjectInfo>,
        bindings: &mut RefBindings,
        details: &mut DeclDetails,
        seen: &mut HashSet<ScopeKey>,
    ) {
        for gsa in vpi::iterate(vpi::vpiGenScopeArray, inst)
            .into_iter()
            .flatten()
        {
            for gs in vpi::iterate(vpi::vpiGenScope, gsa.raw())
                .into_iter()
                .flatten()
            {
                walk_scope(gs.raw(), result, bindings, details, seen);
            }
        }
        for child in vpi::iterate(vpi::vpiModule, inst).into_iter().flatten() {
            walk_instance_gen_scopes(child.raw(), result, bindings, details, seen);
        }
    }
    for top in vpi::iterate(vpi::uhdmtopModules, design)
        .into_iter()
        .flatten()
    {
        walk_instance_gen_scopes(top.raw(), result, bindings, details, &mut seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_local_supplement_recovers_boundaries_without_false_words() {
        // Arrange
        let file = "/virtual/opened.sv";
        let source = "`define GENERATED module DirectiveModule endmodule\n\
/* module CommentModule endmodule */\n\
\"module StringModule endmodule\";\n\
module Recovered `BROKEN_HEADER;\n\
  \\endmodule \\module\n\
endmodule\n";
        let mut parsed = Vec::new();

        // Act
        supplement_source_local_module_tokens(file, source, &mut parsed);

        // Assert
        let nodes = &parsed
            .iter()
            .find(|tokens| tokens.path == file)
            .expect("source-local file entry")
            .nodes;
        let names: Vec<_> = nodes
            .iter()
            .filter_map(|node| node.name.as_deref())
            .collect();
        assert_eq!(names, ["module", "Recovered", "endmodule"]);
        assert_eq!(
            nodes
                .iter()
                .map(|node| (node.line, node.col, node.end_col, node.vpi_type))
                .collect::<Vec<_>>(),
            vec![
                (4, 1, 7, PARSE_OFFSET + VObjectType::paModule_keyword as i32),
                (4, 8, 17, vpi::vpiModule),
                (6, 1, 10, PARSE_OFFSET + VObjectType::paENDMODULE as i32),
            ]
        );
    }

    #[test]
    fn source_local_supplement_counts_utf16_columns() {
        // Arrange
        let file = "/virtual/unicode.sv";
        let source = "/* 😀 */ module Recovered; endmodule\n";
        let mut parsed = Vec::new();

        // Act
        supplement_source_local_module_tokens(file, source, &mut parsed);

        // Assert
        let nodes = &parsed[0].nodes;
        assert_eq!(
            (nodes[0].name.as_deref(), nodes[0].col, nodes[0].end_col),
            (Some("module"), 10, 16)
        );
        assert_eq!(
            (nodes[1].name.as_deref(), nodes[1].col, nodes[1].end_col),
            (Some("Recovered"), 17, 26)
        );
        assert_eq!(
            (nodes[2].name.as_deref(), nodes[2].col, nodes[2].end_col),
            (Some("endmodule"), 28, 37)
        );
    }

    #[test]
    fn source_local_supplement_does_not_name_an_escaped_module_identifier() {
        // Arrange
        let file = "/virtual/escaped_module.sv";
        let mut parsed = Vec::new();

        // Act
        supplement_source_local_module_tokens(
            file,
            "module \\module;\n  logic escaped_body;\nendmodule\n",
            &mut parsed,
        );

        // Assert
        let names: Vec<_> = parsed[0]
            .nodes
            .iter()
            .filter_map(|node| node.name.as_deref())
            .collect();
        assert_eq!(names, ["module", "endmodule"]);
    }

    #[test]
    fn source_local_supplement_skips_continued_directive_lines() {
        // Arrange
        let file = "/virtual/continued_directive.sv";
        let source = concat!(
            "`define GENERATED module FakeFromDefine \\\n",
            "  endmodule \\\n",
            "  module AnotherFakeFromDefine endmodule\n",
            "module Recovered; endmodule\n",
        );
        let mut parsed = Vec::new();

        // Act
        supplement_source_local_module_tokens(file, source, &mut parsed);

        // Assert
        let names: Vec<_> = parsed[0]
            .nodes
            .iter()
            .filter_map(|node| node.name.as_deref())
            .collect();
        assert_eq!(names, ["module", "Recovered", "endmodule"]);
    }
}
