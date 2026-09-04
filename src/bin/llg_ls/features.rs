//! features — LSP feature implementations over the shared compile/model layer.
//!
//! Cached feature queries are pure: they take a shared [`Analysis`] plus a
//! position and return LSP payloads.  [`semantic_tokens_for_open_document`]
//! is the narrow exception: it performs one blocking, isolated parse-only run
//! over a caller-staged open buffer.  No `Client` and no async here — the
//! tower-lsp backend (`src/lsp.rs`) owns the cache, staging, debounce logic and
//! actual request handling.
//!
//! # Position conventions
//!
//! * [`Diag`] positions are **1-based** (as reported by Surelog); 0 means
//!   "unknown".  [`lsp_diagnostics`] converts them to 0-based LSP ranges.
//! * `VObjectInfo` / `FileTokens` positions are **1-based** (as returned by
//!   the VPI interface); [`semantic_tokens_for`] converts them.
//! * Every other public feature function (`hover_at`, `definition_at`,
//!   `references_at`, `document_symbols`, `completion_at`) takes **0-based**
//!   line/column values, matching the LSP wire format directly, and converts
//!   internally when comparing against the 1-based token data.
//! * [`Analysis`] itself is fully owned (`Send`): the Surelog session is
//!   dropped inside [`analyze`] before the result returns.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic as LspDiagnostic, DiagnosticSeverity,
    DocumentSymbol, Hover, HoverContents, LSPAny, LSPObject, Location, MarkupContent, MarkupKind,
    NumberOrString, Position, Range, SemanticTokens, SymbolInformation, SymbolKind, Url,
};

use crate::semantic_tokens;
use llg::core::compile::{self, CompileOpts};
use llg::core::elab::Val;
use llg::core::lint::{self, LintConfig, LintDiag, LintRegistry, LintSeverity, RuleConfig};
use llg::core::macros;
use llg::core::model::{
    ClassDef, ClassFieldDef, DesignModel, Direction, EnumConstDef, FuncArgDef, FuncDef,
    InstanceModel, ModuleDef, PackageDef, ParamModel, PortModel, SignalModel, SymKind, TypeInfo,
};
use llg::core::tokens::{self, DeclTarget, FileTokens, ParseDeclPositions, RefBindings};
use llg::ffi::surelog::{Diag, Severity, VObjectInfo};

// ── Analysis ──────────────────────────────────────────────────────────────────

/// Outcome of an analysis pass.
///
/// `Valid` means that the pass produced a snapshot which is safe for a
/// backend to publish and retain as its last-good snapshot.  The other
/// variants distinguish the diagnostics that make a pass unsuitable for
/// replacement: a fatal frontend failure, a parse failure, or a compile/
/// elaboration failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisOutcome {
    Valid,
    Fatal,
    Parse,
    Compile,
}

// ── Source module graph ──────────────────────────────────────────────────────

/// A source declaration retained while the Surelog session is alive.
///
/// The graph is intentionally independent from UHDM.  Surelog's configured
/// `-top` can omit otherwise valid module definitions from the elaborated
/// instance tree, but the parse tree still contains their declarations and
/// instantiations.  The owned graph lets the module explorer expose those
/// definitions without a second compile or request-time file access.
#[derive(Debug, Clone, PartialEq, Default)]
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

type GraphInstanceKey = (String, String, Option<String>, u32, u32);
type GraphScopeKey = (String, u32, u32);
type GraphDefinitionLineRanges = HashMap<String, HashMap<String, BTreeMap<u32, u32>>>;

/// Transient keyed state used while assembling one source graph.  The public
/// graph deliberately keeps ordered vectors for stable explorer output; these
/// sets/maps make duplicate checks independent of the number of entries
/// already collected for a module.
struct GraphAssemblyIndexes {
    ports: Vec<HashSet<(String, Option<String>)>>,
    params: Vec<HashSet<(String, Option<String>)>>,
    signals: Vec<HashSet<(String, Option<String>)>>,
    children: Vec<HashSet<GraphInstanceKey>>,
    generated_scopes: Vec<HashMap<Vec<GraphScopeKey>, usize>>,
    generated_children: Vec<HashMap<Vec<GraphScopeKey>, HashSet<GraphInstanceKey>>>,
}

impl GraphAssemblyIndexes {
    fn new(definition_count: usize) -> Self {
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

fn graph_definition_line_ranges(
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

fn insert_graph_definition_line_range(
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

fn graph_definition_line_is_retained(
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

impl AnalysisOutcome {
    /// Whether this outcome contains a usable feature snapshot.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// The complete result of one compile+model pass over the open documents.
///
/// Owned and `Send`; the Surelog session it was produced from has already been
/// dropped when this value becomes visible to callers.
pub struct Analysis {
    /// Whether this analysis is safe to publish as a replacement snapshot.
    pub outcome: AnalysisOutcome,
    /// Surelog diagnostics (1-based positions; `file`/`line`/`col` may be
    /// unknown).
    pub diagnostics: Vec<Diag>,
    /// Elaborated design model.  A default/empty model when compilation
    /// failed or neither UHDM nor a parse tree was produced; a modules-only
    /// parse-tree model when syntax errors made Surelog skip UHDM (see
    /// [`parse_tree_feature_parts`]).
    pub model: DesignModel,
    /// Per-file semantic token lists collected while the session was alive.
    pub tokens: Vec<FileTokens>,
    /// Workspace symbol index (declarations + reference sites) built from the
    /// model and the tokens.
    pub index: SymbolIndex,
    /// Reference occurrence → bound declaration, captured during the same VPI
    /// walk as the tokens (`core::tokens::collect_all_tokens`).  Keys are the
    /// 0-based positions of emitted reference tokens; values point at the
    /// declaration elaboration bound the reference to (`vpiActual`), so a
    /// definition request at the exact key position is binding-precise.
    /// Resolved named connection labels are folded in as well, targeting the
    /// child module's declaration — together with the paired connection
    /// ACTUALS (`.clk(wa)` binds `.clk` to the child's port and `wa` to
    /// `wa`'s own declaration in the instantiating/parent scope;
    /// `#(.W(expr))` binds `.W` to the child's parameter and `expr`'s
    /// leading identifier to its own parent-scope declaration).
    /// Includes parse-backed enum bindings when UHDM folded a package/class
    /// use into a literal; the same facts are available in the parse-tree
    /// fallback alongside connection bindings.
    pub ref_bindings: RefBindings,
    /// Rendered declaration snippets (`logic [3:0] val`, …) keyed by
    /// `(file, line1, col1)` of the declaration token, captured from UHDM
    /// during the same walk as [`Analysis::ref_bindings`].  Position-accurate
    /// even where several same-named declarations live in one module (inner-
    /// scope shadowing), so hover text describes exactly the object at that
    /// position instead of the first name match in the model.  Empty for
    /// parse-fallback analyses (the model supplies details there).
    pub decl_details: tokens::DeclDetails,
    /// Lint findings from the shared linter (`source: "llg-lint"` when
    /// published), produced alongside the Surelog diagnostics.  1-based
    /// positions.  Empty when compilation failed or no UHDM design was built.
    pub lint: Vec<LintDiag>,
    /// Source-level module definitions and instance edges retained while the
    /// Surelog parse tree was alive.  The module explorer uses this graph for
    /// definitions that configured elaboration did not instantiate.
    pub(crate) module_graph: ModuleGraph,
    /// The effective `[compile] top` used for this analysis, retained so the
    /// explorer can apply the configured-top root union without rereading
    /// configuration during a request.
    pub(crate) configured_top: Option<String>,
    /// Preprocessor macro table ([`core::macros::MacroTable`]) over the exact
    /// compiled sources: config `[compile] defines` seed every file and one
    /// conservative scan per file resolves in-source `` `define ``/`` `undef ``
    /// positionally (see `llg/src/core/macros.rs` for the documented
    /// semantics).  Built once per analysis commit — never inside a request —
    /// so macro-usage hover stays a pure read over committed data.  Empty for
    /// hand-built analyses.
    macros: macros::MacroTable,
}

impl Analysis {
    /// Assemble an [`Analysis`] from its parts, computing the symbol index.
    ///
    /// Public so tests can build an [`Analysis`] without running Surelog; the
    /// production pipeline uses the same constructor inside [`analyze`].
    ///
    /// Passes an empty UHDM binding map — hand-built analyses have no
    /// elaborated design behind them, matching the parse-tree fallback shape.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(
        diagnostics: Vec<Diag>,
        model: DesignModel,
        tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
    ) -> Analysis {
        let outcome = outcome_from_diagnostics(&diagnostics);
        Self::new_with_outcome(
            outcome,
            diagnostics,
            model,
            tokens,
            lint,
            HashMap::new(),
            ConnectionInputs::default(),
        )
    }

    /// Assemble an [`Analysis`] when the caller has observed the complete
    /// pipeline state and can therefore provide a more precise outcome than
    /// diagnostics alone allow.
    ///
    /// `uhdm_bindings` carries the reference→declaration bindings captured
    /// during the VPI token walk (`core::tokens::collect_all_tokens`).  The
    /// final map is the union of those, the port-connection-derived ones (see
    /// [`ConnectionInputs`]), and nothing else:
    ///
    /// * resolved port-label targets are inserted FIRST (tagged
    ///   `via_label`), each followed by its paired connection ACTUAL target
    ///   (tagged `via_connection`) so the label navigates to the child port and
    ///   the ACTUAL to the parent-scope declaration;
    /// * parse-backed enum bindings are inserted before fallback and UHDM;
    /// * parse-fallback connection bindings are inserted next (they only
    ///   exist when no UHDM design was produced);
    /// * UHDM bindings are inserted after them, winning collisions against the
    ///   label/fallback inputs because they reflect what elaboration actually
    ///   bound;
    /// * UHDM-mode ACTUAL bindings are re-inserted LAST but never override an
    ///   existing entry: elaboration-backed targets at an actual position are
    ///   already binding-precise, so the parent-scope fold only fills positions
    ///   no explicit binding captured.
    pub fn new_with_outcome(
        outcome: AnalysisOutcome,
        diagnostics: Vec<Diag>,
        mut model: DesignModel,
        mut tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
        mut uhdm_bindings: RefBindings,
        mut connections: ConnectionInputs,
    ) -> Analysis {
        normalize_feature_positions(
            &mut model,
            &mut tokens,
            &mut uhdm_bindings,
            &mut connections,
        );
        append_synthetic_tokens(&mut tokens, &connections.parse_enum_tokens);
        let index = SymbolIndex::from_parts(
            &model,
            &tokens,
            connections.parse_decls.as_ref(),
            &connections.pairs,
            &connections.parse_enum_decls,
            &connections.parse_enum_ref_positions,
            &connections.unresolved_enum_refs,
        );
        let ref_bindings = merged_ref_bindings(&index, &model, uhdm_bindings, &connections);
        Analysis {
            outcome,
            diagnostics,
            model,
            tokens,
            index,
            ref_bindings,
            decl_details: HashMap::new(),
            lint,
            module_graph: ModuleGraph::default(),
            configured_top: None,
            macros: macros::MacroTable::default(),
        }
    }

    /// Attach the macro table captured during this analysis commit.
    ///
    /// Production pipeline only; hand-built analyses keep the empty table
    /// (no macro hover), matching the parse-fallback shape of
    /// [`Analysis::fatal_preflight`].
    fn with_macros(mut self, macros: macros::MacroTable) -> Analysis {
        self.macros = macros;
        self
    }

    /// Attach the source graph captured during the same Surelog pass.
    fn with_module_graph(mut self, module_graph: ModuleGraph) -> Analysis {
        self.module_graph = module_graph;
        self
    }

    /// Retain the configured compile top used by this analysis.
    fn with_configured_top(mut self, configured_top: Option<String>) -> Analysis {
        self.configured_top = configured_top;
        self
    }

    /// Name the configuration source for undefined-macro messages (called by
    /// the backend at commit time with the root's effective config path).
    pub fn attach_macro_config_note(&mut self, note: impl Into<String>) {
        self.macros.set_config_note(note);
    }

    /// Attach the declaration snippets captured during the VPI token walk.
    ///
    /// Every indexed DECLARATION entry whose position matches a captured
    /// snippet gets its hover text replaced by the position-accurate one —
    /// under inner-scope shadowing the name-based model lookup would describe
    /// the outer same-named object.  The map is also retained on the analysis
    /// so hover can render bound targets whose declaration has no indexed
    /// entry.  Production pipeline only; hand-built analyses have no UHDM
    /// behind them and keep the model-derived details.
    fn with_decl_details(mut self, mut details: tokens::DeclDetails) -> Analysis {
        let maps = FeatureSourceMaps::from_parts(
            &self.model,
            &self.tokens,
            &self.ref_bindings,
            &ConnectionInputs::default(),
        );
        normalize_decl_details(&maps, &mut details);
        // Ports/nets/vars only: their model detail is a NAME-only lookup and
        // is exactly what breaks under inner-scope shadowing.  Parameters
        // keep the model detail — it carries the resolved value and the
        // package/instance scope, which the token walk cannot reproduce.
        for decl in self.index.decls.iter_mut() {
            if !decl.is_decl || !matches!(decl.kind, SymKind::Port | SymKind::Net | SymKind::Var) {
                continue;
            }
            if let Some(text) = details.get(&(decl.file.clone(), decl.line + 1, decl.col + 1)) {
                if !text.is_empty() {
                    decl.detail = Some(text.clone());
                }
            }
        }
        // The position/name lookup maps hold clones of the entries; refresh
        // them so `entry_at` serves the patched details.
        self.index.rebuild_lookup_maps();
        self.decl_details = details;
        self
    }

    /// Construct a fatal preflight result with no feature data.
    pub fn fatal_preflight(message: impl Into<String>) -> Analysis {
        Self::new_with_outcome(
            AnalysisOutcome::Fatal,
            vec![Diag {
                severity: Severity::Fatal,
                file: None,
                line: 0,
                col: 0,
                message: message.into(),
            }],
            empty_design(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            ConnectionInputs::default(),
        )
    }

    /// Whether this analysis is a valid replacement for a retained snapshot.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_valid(&self) -> bool {
        self.outcome.is_valid()
    }

    /// Whether this analysis carries servable navigation data.
    ///
    /// Feature-serving gate for hover, definition, references, document/
    /// workspace symbols, completion and semantic tokens: an analysis serves
    /// features whenever its outcome is not [`AnalysisOutcome::Fatal`] AND it
    /// produced any servable data at all (index declarations, model modules,
    /// or semantic tokens).  [`AnalysisOutcome::Fatal`] means no usable UHDM
    /// existed (poisoned database, preflight aborts), so those analyses stay
    /// feature-less; a partial analysis — some files failed to parse or
    /// compile while Surelog still elaborated the surviving set — serves
    /// best-effort navigation, and a syntax-broken project (Surelog skipped
    /// its whole compile/UHDM stage) serves declaration-level data through the
    /// parse-tree fallback ([`parse_tree_feature_parts`]).  An analysis whose
    /// db built but yielded no data (e.g. an empty project) legitimately has
    /// none.  Diagnostics are always published regardless of this predicate.
    ///
    /// Strict validity ([`Analysis::is_valid`]) remains the replacement
    /// criterion for full-fidelity snapshots; this predicate only decides
    /// whether *something* is servable.
    pub fn has_feature_data(&self) -> bool {
        self.outcome != AnalysisOutcome::Fatal
            && (!self.index.decls.is_empty()
                || !self.model.modules.is_empty()
                || !self.tokens.is_empty())
    }
}

/// Add parse-backed qualified enum members that Surelog does not expose as a
/// standalone VPI/parse token.  Existing positions win so a normal parse/VPI
/// token keeps its richer classification.
fn append_synthetic_tokens(tokens: &mut Vec<FileTokens>, synthetic: &[VObjectInfo]) {
    for node in synthetic {
        let Some(file_tokens) = tokens.iter_mut().find(|ft| ft.path == node.file) else {
            tokens.push(FileTokens {
                path: node.file.clone(),
                nodes: vec![node.clone()],
            });
            continue;
        };
        if file_tokens
            .nodes
            .iter()
            .any(|existing| existing.line == node.line && existing.col == node.col)
        {
            continue;
        }
        file_tokens.nodes.push(node.clone());
    }
}

/// Which side of an instantiation a scanned connection belongs to.
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
    name: String,
    file: String,
    line1: u32,
    col1: u32,
    scope: Option<String>,
}

/// Parse-backed enum references and the exact token positions at which they
/// occur.  Surelog's VPI elaboration folds package/class enum expressions
/// into constants, so these facts are collected from the surviving parse tree
/// and merged with the normal VPI binding map.
#[derive(Debug, Clone, Default)]
struct ParseEnumFacts {
    declarations: Vec<ParseEnumDecl>,
    bindings: RefBindings,
    reference_positions: HashSet<(String, u32, u32)>,
    unresolved_positions: HashSet<(String, u32, u32)>,
    synthetic_tokens: Vec<VObjectInfo>,
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

/// Source facts needed to translate a raw Surelog column into an LSP UTF-16
/// column.  Only files containing non-ASCII text are retained: for ASCII the
/// scalar, UTF-16, and byte columns are identical and no map is needed.
#[derive(Debug)]
struct FeatureSourceMap {
    source: String,
    line_starts: Vec<usize>,
}

impl FeatureSourceMap {
    fn new(source: String) -> Self {
        let line_starts = graph_line_starts(&source);
        Self {
            source,
            line_starts,
        }
    }

    fn line_bounds(&self, line: u32) -> Option<(usize, usize)> {
        let index = usize::try_from(line.checked_sub(1)?).ok()?;
        let start = *self.line_starts.get(index)?;
        let mut end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.source.len());
        while end > start && matches!(self.source.as_bytes().get(end - 1), Some(b'\n' | b'\r')) {
            end -= 1;
        }
        Some((start, end))
    }

    fn scalar_offset(&self, line: u32, character: usize) -> Option<usize> {
        let (start, end) = self.line_bounds(line)?;
        let text = self.source.get(start..end)?;
        text.char_indices()
            .nth(character)
            .map(|(offset, _)| start + offset)
            .or_else(|| (character == text.chars().count()).then_some(end))
    }

    fn utf16_offset(&self, line: u32, character: usize) -> Option<usize> {
        let (start, end) = self.line_bounds(line)?;
        let text = self.source.get(start..end)?;
        let mut units = 0usize;
        for (offset, value) in text.char_indices() {
            if units == character {
                return Some(start + offset);
            }
            units += value.len_utf16();
            if units > character {
                return None;
            }
        }
        (units == character).then_some(end)
    }

    fn byte_offset(&self, line: u32, character: usize) -> Option<usize> {
        let (start, end) = self.line_bounds(line)?;
        let offset = start.checked_add(character)?;
        (offset <= end && self.source.is_char_boundary(offset)).then_some(offset)
    }

    /// Choose the coordinate interpretation whose byte offset actually starts
    /// `name`.  The UTF-16 candidate is preferred for tokens produced by the
    /// source-local scanner; raw Surelog scalar/byte columns win when those are
    /// the only candidates matching the source text.
    fn offset_for(&self, line: u32, col: u32, name: Option<&str>) -> Option<usize> {
        let character = usize::try_from(col.checked_sub(1)?).ok()?;
        let utf16 = self.utf16_offset(line, character);
        let scalar = self.scalar_offset(line, character);
        let byte = self.byte_offset(line, character);
        [utf16, scalar, byte]
            .into_iter()
            .flatten()
            .find(|offset| {
                name.is_some_and(|name| {
                    self.source
                        .get(*offset..)
                        .is_some_and(|tail| tail.starts_with(name))
                })
            })
            .or(scalar)
            .or(byte)
            .or(utf16)
    }

    fn lsp_column(&self, line: u32, col: u32, name: Option<&str>) -> u32 {
        let Some(offset) = self.offset_for(line, col, name) else {
            return col;
        };
        let Some((start, _)) = self.line_bounds(line) else {
            return col;
        };
        self.source
            .get(start..offset)
            .map_or(col, |prefix| prefix.encode_utf16().count() as u32 + 1)
    }

    fn normalize_1based(&self, line: u32, col: u32, name: Option<&str>) -> (u32, u32) {
        (line, self.lsp_column(line, col, name))
    }

    fn normalize_0based(&self, line: u32, col: u32, name: Option<&str>) -> (u32, u32) {
        let (line1, col1) =
            self.normalize_1based(line.saturating_add(1), col.saturating_add(1), name);
        (line1.saturating_sub(1), col1.saturating_sub(1))
    }
}

#[derive(Debug, Default)]
struct FeatureSourceMaps {
    by_file: HashMap<String, FeatureSourceMap>,
}

impl FeatureSourceMaps {
    fn get(&self, file: &str) -> Option<&FeatureSourceMap> {
        self.by_file.get(file)
    }

    fn from_parts(
        model: &DesignModel,
        tokens: &[FileTokens],
        bindings: &RefBindings,
        connections: &ConnectionInputs,
    ) -> Self {
        let mut paths = BTreeSet::new();
        for file_tokens in tokens {
            paths.insert(file_tokens.path.clone());
            for node in &file_tokens.nodes {
                paths.insert(node.file.clone());
            }
        }
        collect_model_source_paths(model, &mut paths);
        for ((file, _, _), target) in bindings {
            paths.insert(file.clone());
            paths.insert(target.file.clone());
        }
        for pair in &connections.pairs {
            paths.insert(pair.file.clone());
        }
        for (file, _, _) in connections
            .parse_decls
            .iter()
            .flatten()
            .chain(connections.parse_enum_ref_positions.iter())
            .chain(connections.unresolved_enum_refs.iter())
        {
            paths.insert(file.clone());
        }
        for declaration in &connections.parse_enum_decls {
            paths.insert(declaration.file.clone());
        }
        for ((file, _, _), target) in &connections.fallback_bindings {
            paths.insert(file.clone());
            paths.insert(target.file.clone());
        }
        for ((file, _, _), target) in &connections.parse_enum_bindings {
            paths.insert(file.clone());
            paths.insert(target.file.clone());
        }
        for node in &connections.parse_enum_tokens {
            paths.insert(node.file.clone());
        }

        let by_file = paths
            .into_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(&file).ok()?;
                (!source.is_ascii()).then(|| (file, FeatureSourceMap::new(source)))
            })
            .collect();
        Self { by_file }
    }
}

fn collect_model_source_paths(model: &DesignModel, paths: &mut BTreeSet<String>) {
    for module in &model.modules {
        if let Some(file) = &module.file {
            paths.insert(file.clone());
        }
    }
    for package in &model.packages {
        if let Some(file) = &package.file {
            paths.insert(file.clone());
        }
        for constant in &package.enum_consts {
            if let Some(file) = &constant.file {
                paths.insert(file.clone());
            }
        }
    }
    for class in &model.classes {
        if let Some(file) = &class.file {
            paths.insert(file.clone());
        }
        for method in &class.methods {
            if let Some(file) = &method.file {
                paths.insert(file.clone());
            }
        }
    }
    collect_instance_source_paths(&model.top_instances, paths);
}

fn collect_instance_source_paths(instances: &[InstanceModel], paths: &mut BTreeSet<String>) {
    for instance in instances {
        if let Some(file) = &instance.file {
            paths.insert(file.clone());
        }
        for function in &instance.funcs {
            if let Some(file) = &function.file {
                paths.insert(file.clone());
            }
        }
        collect_instance_source_paths(&instance.children, paths);
    }
}

fn feature_token_names(
    tokens: &[FileTokens],
    synthetic: &[VObjectInfo],
) -> HashMap<(String, u32, u32), String> {
    let mut names = HashMap::new();
    for node in tokens
        .iter()
        .flat_map(|file| file.nodes.iter())
        .chain(synthetic)
    {
        let Some(name) = node.name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        names
            .entry((node.file.clone(), node.line, node.col))
            .or_insert_with(|| name.to_owned());
    }
    names
}

fn normalize_vobject_positions(
    maps: &FeatureSourceMaps,
    fallback_file: Option<&str>,
    nodes: &mut [VObjectInfo],
) {
    for node in nodes {
        let Some(map) = maps
            .get(&node.file)
            .or_else(|| fallback_file.and_then(|file| maps.get(file)))
        else {
            continue;
        };
        let old_line = node.line;
        let old_col = node.col;
        let name = node.name.as_deref().filter(|name| !name.is_empty());
        let (line, col) = map.normalize_1based(old_line, old_col, name);
        node.line = line;
        node.col = col;
        if let Some(name) = name {
            if node.end_line == old_line {
                node.end_line = line;
                node.end_col = col.saturating_add(lsp_name_len(name));
                continue;
            }
        }
        if node.end_line != 0 && node.end_col != 0 {
            let (end_line, end_col) = map.normalize_1based(node.end_line, node.end_col, None);
            node.end_line = end_line;
            node.end_col = end_col;
        }
    }
}

fn normalize_one_based_positions(
    maps: &FeatureSourceMaps,
    positions: &mut HashSet<(String, u32, u32)>,
    names: &HashMap<(String, u32, u32), String>,
) {
    let old = std::mem::take(positions);
    *positions = old
        .into_iter()
        .map(|(file, line, col)| {
            let name = names.get(&(file.clone(), line, col)).map(String::as_str);
            let (line, col) = maps
                .get(&file)
                .map_or((line, col), |map| map.normalize_1based(line, col, name));
            (file, line, col)
        })
        .collect();
}

fn normalize_zero_based_positions(
    maps: &FeatureSourceMaps,
    positions: &mut HashSet<(String, u32, u32)>,
    names: &HashMap<(String, u32, u32), String>,
) {
    let old = std::mem::take(positions);
    *positions = old
        .into_iter()
        .map(|(file, line, col)| {
            let name = names
                .get(&(file.clone(), line.saturating_add(1), col.saturating_add(1)))
                .map(String::as_str);
            let (line, col) = maps
                .get(&file)
                .map_or((line, col), |map| map.normalize_0based(line, col, name));
            (file, line, col)
        })
        .collect();
}

fn normalize_ref_bindings(
    maps: &FeatureSourceMaps,
    names: &HashMap<(String, u32, u32), String>,
    bindings: &mut RefBindings,
) {
    let old = std::mem::take(bindings);
    let mut normalized = HashMap::with_capacity(old.len());
    for ((file, line, col), mut target) in old {
        let reference_name = names
            .get(&(file.clone(), line.saturating_add(1), col.saturating_add(1)))
            .map(String::as_str)
            .or(Some(target.name.as_str()));
        let (line, col) = maps.get(&file).map_or((line, col), |map| {
            map.normalize_0based(line, col, reference_name)
        });
        if let Some(map) = maps.get(&target.file) {
            (target.line0, target.col0) =
                map.normalize_0based(target.line0, target.col0, Some(&target.name));
        }
        normalized.insert((file, line, col), target);
    }
    *bindings = normalized;
}

fn normalize_connection_inputs(
    maps: &FeatureSourceMaps,
    names: &HashMap<(String, u32, u32), String>,
    connections: &mut ConnectionInputs,
) {
    for pair in &mut connections.pairs {
        if let Some(map) = maps.get(&pair.file) {
            pair.label = map.normalize_1based(pair.label.0, pair.label.1, Some(&pair.label_name));
            if let (Some(position), Some(name)) = (pair.actual, pair.actual_name.as_deref()) {
                pair.actual = Some(map.normalize_1based(position.0, position.1, Some(name)));
            }
        }
    }
    if let Some(positions) = &mut connections.parse_decls {
        normalize_one_based_positions(maps, positions, names);
    }
    for declaration in &mut connections.parse_enum_decls {
        if let Some(map) = maps.get(&declaration.file) {
            (declaration.line1, declaration.col1) =
                map.normalize_1based(declaration.line1, declaration.col1, Some(&declaration.name));
        }
    }
    normalize_zero_based_positions(maps, &mut connections.parse_enum_ref_positions, names);
    normalize_zero_based_positions(maps, &mut connections.unresolved_enum_refs, names);
    normalize_ref_bindings(maps, names, &mut connections.fallback_bindings);
    normalize_ref_bindings(maps, names, &mut connections.parse_enum_bindings);
    normalize_vobject_positions(maps, None, &mut connections.parse_enum_tokens);
}

fn normalize_model_positions(maps: &FeatureSourceMaps, model: &mut DesignModel) {
    for module in &mut model.modules {
        if let Some(file) = module.file.as_deref().and_then(|file| maps.get(file)) {
            (module.line, module.col) =
                file.normalize_1based(module.line, module.col, Some(clean_name(&module.name)));
            if module.end_line != 0 && module.end_col != 0 {
                (module.end_line, module.end_col) =
                    file.normalize_1based(module.end_line, module.end_col, None);
            }
        }
    }
    for package in &mut model.packages {
        if let Some(path) = package.file.as_deref().and_then(|file| maps.get(file)) {
            (package.line, package.col) =
                path.normalize_1based(package.line, package.col, Some(clean_name(&package.name)));
        }
        for constant in &mut package.enum_consts {
            if let Some(path) = constant.file.as_deref().and_then(|file| maps.get(file)) {
                (constant.line, constant.col) =
                    path.normalize_1based(constant.line, constant.col, Some(&constant.name));
            }
        }
    }
    for class in &mut model.classes {
        if let Some(path) = class.file.as_deref().and_then(|file| maps.get(file)) {
            (class.line, class.col) =
                path.normalize_1based(class.line, class.col, Some(clean_name(&class.name)));
        }
        for method in &mut class.methods {
            if let Some(path) = method.file.as_deref().and_then(|file| maps.get(file)) {
                (method.line, method.col) =
                    path.normalize_1based(method.line, method.col, Some(clean_name(&method.name)));
            }
        }
        if let Some(path) = class.file.as_deref().and_then(|file| maps.get(file)) {
            for field in &mut class.fields {
                (field.line, field.col) =
                    path.normalize_1based(field.line, field.col, Some(&field.name));
            }
        }
    }
    normalize_instance_positions(maps, &mut model.top_instances);
}

fn normalize_instance_positions(maps: &FeatureSourceMaps, instances: &mut [InstanceModel]) {
    for instance in instances {
        if let Some(path) = instance.file.as_deref().and_then(|file| maps.get(file)) {
            (instance.line, instance.col) = path.normalize_1based(
                instance.line,
                instance.col,
                Some(clean_name(&instance.name)),
            );
        }
        for function in &mut instance.funcs {
            if let Some(path) = function.file.as_deref().and_then(|file| maps.get(file)) {
                (function.line, function.col) = path.normalize_1based(
                    function.line,
                    function.col,
                    Some(clean_name(&function.name)),
                );
            }
        }
        normalize_instance_positions(maps, &mut instance.children);
    }
}

fn normalize_feature_positions(
    model: &mut DesignModel,
    tokens: &mut [FileTokens],
    uhdm_bindings: &mut RefBindings,
    connections: &mut ConnectionInputs,
) {
    let maps = FeatureSourceMaps::from_parts(model, tokens, uhdm_bindings, connections);
    let names = feature_token_names(tokens, &connections.parse_enum_tokens);
    normalize_model_positions(&maps, model);
    for file_tokens in tokens {
        normalize_vobject_positions(&maps, Some(&file_tokens.path), &mut file_tokens.nodes);
    }
    normalize_connection_inputs(&maps, &names, connections);
    normalize_ref_bindings(&maps, &names, uhdm_bindings);
}

fn declaration_detail_name(detail: &str) -> Option<&str> {
    detail
        .split_whitespace()
        .last()
        .map(|name| name.trim_matches(|character: char| matches!(character, ',' | ';' | ')')))
        .filter(|name| !name.is_empty())
}

fn normalize_decl_details(maps: &FeatureSourceMaps, details: &mut tokens::DeclDetails) {
    let old = std::mem::take(details);
    *details = old
        .into_iter()
        .map(|((file, line, col), detail)| {
            let position = maps.get(&file).map_or((line, col), |map| {
                map.normalize_1based(line, col, declaration_detail_name(&detail))
            });
            ((file, position.0, position.1), detail)
        })
        .collect();
}

/// The final `Analysis.ref_bindings` map.
///
/// Insertion order defines collision resolution (later insert wins, except
/// for the connection-ACTUAL fold, which never overrides):
///
/// 1. **Connection-label fold** (`via_label`): resolved named connection
///    labels are binding-precise by construction — port labels point at the
///    child module's port declaration ([`SymbolIndex::port_labels`]),
///    parameter-override labels at the child module's parameter declaration
///    ([`SymbolIndex::param_labels`]) — so they seed the map from the index.
/// 2. **Parse-fallback connection bindings**: port/parameter label entries
///    resolved against the recorded child declarations plus actual entries
///    resolved to the parent-scope declaration; only non-empty when no UHDM
///    design exists.
/// 3. **UHDM bindings** captured during the VPI token walk
///    (`core::tokens::collect_all_tokens`): where both capture paths produced
///    an entry for the same position, the elaboration-backed (`vpiActual`)
///    UHDM target wins because it reflects what elaboration actually bound,
///    including for connections the label heuristic cannot classify.
/// 4. **Connection ACTUAL bindings** (`via_connection`, UHDM mode): the
///    paired actual of every resolved label points at the actual signal's
///    OWN declaration in the instantiating (parent) scope.  These are
///    inserted last but only into positions that have no binding yet — an
///    existing explicit binding (elaboration-backed or fallback) wins.
fn merged_ref_bindings(
    index: &SymbolIndex,
    model: &DesignModel,
    uhdm_bindings: RefBindings,
    connections: &ConnectionInputs,
) -> RefBindings {
    let mut out: RefBindings = HashMap::new();
    // Pairing map: 0-based label position → (0-based actual position, actual
    // identifier text).  Both sides of a connection live in the instantiating
    // file.
    type SourcePosition = (String, u32, u32);
    type ActualBinding = ((u32, u32), String);
    let mut actual_of: HashMap<SourcePosition, ActualBinding> = HashMap::new();
    for pair in &connections.pairs {
        if let (Some((line0, col0)), Some(name)) = (pair.actual, pair.actual_name.as_deref()) {
            actual_of.insert(
                (
                    pair.file.clone(),
                    pair.label.0.saturating_sub(1),
                    pair.label.1.saturating_sub(1),
                ),
                (
                    (line0.saturating_sub(1), col0.saturating_sub(1)),
                    name.to_owned(),
                ),
            );
        }
    }
    // Connection ACTUAL bindings collected separately so they can be
    // inserted AFTER the UHDM map without overriding it.
    let mut actual_bindings: RefBindings = HashMap::new();
    for ((file, line, col), idx) in index.port_labels.iter().chain(index.param_labels.iter()) {
        let Some(decl) = index.decls.get(*idx) else {
            continue;
        };
        out.insert(
            (file.clone(), *line, *col),
            DeclTarget {
                name: decl.name.clone(),
                kind: kind_label(decl.kind).to_owned(),
                file: decl.file.clone(),
                line0: decl.line,
                col0: decl.col,
                via_label: true,
                via_connection: false,
            },
        );
        // Bind the connected signal's position to its own declaration in the
        // enclosing (parent) scope — NOT to the child object.
        if let Some(((actual_line, actual_col), actual_name)) =
            actual_of.get(&(file.clone(), *line, *col))
        {
            if let Some(target) =
                parent_scope_actual_target(&index.decls, model, file, *line, actual_name)
            {
                actual_bindings.insert((file.clone(), *actual_line, *actual_col), target);
            }
        }
    }
    // Parse-backed enum bindings fill the gap left when UHDM folds a
    // package/class-qualified constant use into a literal.  UHDM remains the
    // authoritative winner if it did emit a binding at the same coordinate.
    for (key, target) in &connections.parse_enum_bindings {
        out.insert(key.clone(), target.clone());
    }
    for (key, target) in &connections.fallback_bindings {
        out.insert(key.clone(), target.clone());
    }
    for (key, target) in uhdm_bindings {
        out.insert(key, target);
    }
    // Existing explicit binding wins: never overwrite an UHDM/fallback entry.
    for (key, target) in actual_bindings {
        out.entry(key).or_insert(target);
    }
    out
}

/// One candidate declaration for parent-scope ACTUAL resolution:
/// a `(0-based line, 0-based column)` position of a declared identifier.
type ActualCandidatePos = (u32, u32);

/// A module body span in inclusive 0-based line coordinates.
///
/// `last0 == None` when the end position is unknown (unset `vpiEndLineNo`);
/// such spans never win containment.
struct ModuleSpan0 {
    first0: u32,
    last0: Option<u32>,
}

impl ModuleSpan0 {
    fn contains(&self, line0: u32) -> bool {
        match self.last0 {
            Some(last) => self.first0 <= line0 && line0 <= last,
            None => false,
        }
    }

    fn len(&self) -> u32 {
        self.last0
            .unwrap_or(self.first0)
            .saturating_sub(self.first0)
    }
}

/// Resolve the declaration of a connection ACTUAL identifier in the
/// instantiating (parent) scope and build its binding target.
///
/// Selection over `decls` (already restricted by the caller to the wanted
/// name):
///
/// 1. Candidates are the declarations in the instantiating file whose
///    enclosing module span contains the instantiation line (`spans`
///    supplies those spans; innermost wins).  Among them the nearest ABOVE
///    the instantiation line is chosen (nearest below as a last resort —
///    still the right namespace).
/// 2. Without a containing span or an in-span candidate: the nearest
///    candidate above the instantiation line.
/// 3. Otherwise NO target — the actual stays unbound and the existing
///    fallback resolution applies.
fn select_parent_scope_position(
    positions: &[ActualCandidatePos],
    spans: &[ModuleSpan0],
    inst_line0: u32,
) -> Option<ActualCandidatePos> {
    if positions.is_empty() {
        return None;
    }
    // Innermost module span containing the instantiation line.
    let scope = spans
        .iter()
        .filter(|span| span.contains(inst_line0))
        .min_by_key(|span| (span.len(), span.first0));
    let pool: Vec<ActualCandidatePos> = match scope {
        Some(span) => {
            let inside: Vec<ActualCandidatePos> = positions
                .iter()
                .copied()
                .filter(|&(line0, _)| span.contains(line0))
                .collect();
            if inside.is_empty() {
                positions.to_vec()
            } else {
                inside
            }
        }
        None => positions.to_vec(),
    };
    // Nearest above the instantiation line; ties break to the rightmost
    // column.  Only when nothing sits above does the nearest below win
    // (same parent scope, declared after the instantiation).
    let mut ranked: Vec<ActualCandidatePos> = pool;
    ranked.sort_by_key(|&(line0, col0)| (line0 > inst_line0, line0.abs_diff(inst_line0), col0));
    ranked.into_iter().next()
}

fn first_position_at_or_after_line(
    positions: &[ActualCandidatePos],
    mut low: usize,
    mut high: usize,
    line0: u32,
) -> usize {
    while low < high {
        let middle = low + (high - low) / 2;
        if positions[middle].0 < line0 {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

fn first_position_after_line(
    positions: &[ActualCandidatePos],
    mut low: usize,
    mut high: usize,
    line0: u32,
) -> usize {
    while low < high {
        let middle = low + (high - low) / 2;
        if positions[middle].0 <= line0 {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// The parse-fallback variant of [`select_parent_scope_position`].  Its
/// candidate list is pre-sorted once per file/name by [`ParseFallbackIndex`],
/// so selecting the nearest declaration uses binary searches and borrows a
/// slice instead of allocating, filtering, and sorting for every connection.
fn select_parent_scope_position_sorted(
    positions: &[ActualCandidatePos],
    spans: &[ModuleSpan0],
    inst_line0: u32,
) -> Option<ActualCandidatePos> {
    if positions.is_empty() {
        return None;
    }
    let scope = spans
        .iter()
        .filter(|span| span.contains(inst_line0))
        .min_by_key(|span| (span.len(), span.first0));
    let (pool_start, pool_end) = match scope {
        Some(span) => {
            let inside_start =
                first_position_at_or_after_line(positions, 0, positions.len(), span.first0);
            let inside_end = span.last0.map_or(inside_start, |last0| {
                first_position_after_line(positions, inside_start, positions.len(), last0)
            });
            if inside_start < inside_end {
                (inside_start, inside_end)
            } else {
                (0, positions.len())
            }
        }
        None => (0, positions.len()),
    };
    if pool_start >= pool_end {
        return None;
    }

    let above_end = first_position_after_line(positions, pool_start, pool_end, inst_line0);
    if above_end > pool_start {
        // The old ranking chooses the smallest column on the nearest line.
        let chosen_line = positions[above_end - 1].0;
        let chosen = first_position_at_or_after_line(positions, pool_start, above_end, chosen_line);
        Some(positions[chosen])
    } else {
        // Positions are sorted by line and then column, matching the old
        // `(line > inst_line, distance, column)` ranking for below-only
        // candidates.
        Some(positions[pool_start])
    }
}

/// Build the parent-scope binding target for one connection ACTUAL
/// (UHDM mode): candidates come from the symbol index, the kind from
/// [`SymKind`] via [`kind_label`].
fn parent_scope_actual_target(
    decls: &[SymEntry],
    model: &DesignModel,
    inst_file: &str,
    inst_line0: u32,
    actual_name: &str,
) -> Option<DeclTarget> {
    let positions: Vec<ActualCandidatePos> = decls
        .iter()
        .filter(|d| d.is_decl && d.name == actual_name && d.file == inst_file)
        .map(|d| (d.line, d.col))
        .collect();
    let spans: Vec<ModuleSpan0> = model
        .modules
        .iter()
        .filter(|m| m.file.as_deref() == Some(inst_file))
        .map(|m| ModuleSpan0 {
            first0: m.line.saturating_sub(1),
            last0: (m.end_line > 0).then(|| m.end_line.saturating_sub(1)),
        })
        .collect();
    let (line0, col0) = select_parent_scope_position(&positions, &spans, inst_line0)?;
    let decl = decls
        .iter()
        .find(|d| d.is_decl && d.file == inst_file && d.line == line0 && d.col == col0)?;
    Some(DeclTarget {
        name: decl.name.clone(),
        kind: kind_label(decl.kind).to_owned(),
        file: decl.file.clone(),
        line0: decl.line,
        col0: decl.col,
        via_label: false,
        via_connection: true,
    })
}

fn outcome_from_diagnostics(diagnostics: &[Diag]) -> AnalysisOutcome {
    if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Fatal))
    {
        AnalysisOutcome::Fatal
    } else if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Syntax))
    {
        AnalysisOutcome::Parse
    } else if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error))
    {
        AnalysisOutcome::Compile
    } else {
        AnalysisOutcome::Valid
    }
}

fn outcome_from_pipeline(
    out: &compile::CompileOut,
    has_uhdm: bool,
    has_design: bool,
    db_built: bool,
) -> AnalysisOutcome {
    let diagnostic_outcome = outcome_from_diagnostics(&out.diagnostics);
    if !matches!(diagnostic_outcome, AnalysisOutcome::Valid) {
        return diagnostic_outcome;
    }
    if !out.ok() || !has_uhdm || !has_design || !db_built {
        AnalysisOutcome::Compile
    } else {
        AnalysisOutcome::Valid
    }
}

fn db_build_diagnostic(error: &str) -> Diag {
    Diag {
        severity: Severity::Error,
        file: None,
        line: 0,
        col: 0,
        message: format!("UHDM database build failed: {error}"),
    }
}

fn token_node_count(tokens: &[FileTokens]) -> usize {
    tokens.iter().map(|file| file.nodes.len()).sum()
}

fn token_cardinality(tokens: &[FileTokens]) -> usize {
    if crate::logging::enabled(crate::logging::Level::Debug) {
        token_node_count(tokens)
    } else {
        tokens.len()
    }
}

const SURELOG_LOG_ARG_MAX: usize = 128;
const SURELOG_LOG_ARGV_MAX: usize = 2_048;
const SURELOG_LOG_ERROR_MAX: usize = 256;

fn bounded_log_text(value: &str, max_bytes: usize) -> String {
    let mut result = String::new();
    let mut truncated = false;
    for character in value.chars() {
        let escaped = match character {
            '\n' => "\\n".to_owned(),
            '\r' => "\\r".to_owned(),
            '\t' => "\\t".to_owned(),
            character if character.is_control() => "?".to_owned(),
            character => character.to_string(),
        };
        if result.len().saturating_add(escaped.len()) > max_bytes {
            truncated = true;
            break;
        }
        result.push_str(&escaped);
    }
    if truncated {
        if max_bytes < 3 {
            return ".".repeat(max_bytes);
        }
        while result.len().saturating_add(3) > max_bytes {
            result.pop();
        }
        result.push_str("...");
    }
    result
}

fn bounded_surelog_arg(arg: &str) -> String {
    let (prefix, payload) = if let Some(payload) = arg.strip_prefix("-D") {
        ("-D", Some(payload))
    } else if let Some(payload) = arg.strip_prefix("-P") {
        ("-P", Some(payload))
    } else if let Some(payload) = arg.strip_prefix("-I") {
        ("-I", Some(payload))
    } else if let Some(payload) = arg.strip_prefix("+incdir+") {
        ("+incdir+", Some(payload))
    } else {
        return bounded_log_text(arg, SURELOG_LOG_ARG_MAX);
    };

    if matches!(prefix, "-D" | "-P") {
        let payload = payload.expect("define/parameter prefix always has a payload");
        let (name, has_value) = payload
            .split_once('=')
            .map_or((payload, false), |(name, _)| (name, true));
        let name = bounded_log_text(name, SURELOG_LOG_ARG_MAX.saturating_sub(16));
        let value_suffix = if has_value { "=<redacted>" } else { "" };
        return bounded_log_text(
            &format!("{prefix}{name}{value_suffix}"),
            SURELOG_LOG_ARG_MAX,
        );
    }

    bounded_log_text(
        &format!(
            "{prefix}{}",
            bounded_log_text(payload.unwrap_or_default(), SURELOG_LOG_ARG_MAX)
        ),
        SURELOG_LOG_ARG_MAX,
    )
}

fn surelog_argv_log_details(argv: &[String]) -> (String, String) {
    let mut representation = String::from("[");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut redact_next_value = false;
    for (index, arg) in argv.iter().enumerate() {
        let safe_arg = if redact_next_value {
            redact_next_value = false;
            "<redacted>".to_owned()
        } else {
            let safe_arg = bounded_surelog_arg(arg);
            redact_next_value = matches!(arg.as_str(), "-D" | "-P");
            safe_arg
        };
        index.hash(&mut hasher);
        safe_arg.hash(&mut hasher);
        let separator = if index == 0 { "" } else { "," };
        let addition = format!("{separator}{safe_arg:?}");
        if representation
            .len()
            .saturating_add(addition.len())
            .saturating_add(1) // reserve the closing bracket
            > SURELOG_LOG_ARGV_MAX
        {
            while representation.len().saturating_add(5) > SURELOG_LOG_ARGV_MAX {
                representation.pop();
            }
            representation.push_str(",...");
            break;
        }
        representation.push_str(&addition);
    }
    representation.push(']');
    (representation, format!("{:016x}", hasher.finish()))
}

/// Emit the native Surelog configuration only when debug logging is enabled.
/// The invocation builder is intentionally called inside the guard: the
/// normal compile path uses the same ordered visitor without allocating this
/// diagnostic copy. Values from '-D'/'-P' are redacted; paths and all other
/// fields are escaped and bounded by the constants above.
fn log_surelog_invocation(
    kind: &str,
    invocation: &compile::SurelogInvocation,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) {
    if !crate::logging::enabled(crate::logging::Level::Debug) {
        return;
    }
    let setters = invocation.setters;
    let (argv_repr, argv_fingerprint) = surelog_argv_log_details(&invocation.argv);
    let root = bounded_log_text(root, SURELOG_LOG_ARG_MAX);
    crate::llg_debug!(
        "event=surelog.invoke kind={} root={} generation={} parent_id={:?} argv_count={} argv_repr={} argv_fingerprint={} setters=parse:{} write_pp_output:{} compile:{} elaborate:{} elab_uhdm:{} mute:{} quiet:{}",
        kind,
        root,
        generation,
        parent_id,
        invocation.argv.len(),
        argv_repr,
        argv_fingerprint,
        setters.parse,
        setters.write_pp_output,
        setters.compile,
        setters.elaborate,
        setters.elab_uhdm,
        setters.mute_stdout,
        setters.quiet,
    );
}

fn log_surelog_invocation_rejected(
    kind: &str,
    error: &str,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) {
    if !crate::logging::enabled(crate::logging::Level::Debug) {
        return;
    }
    let root = bounded_log_text(root, SURELOG_LOG_ARG_MAX);
    crate::llg_debug!(
        "event=surelog.invoke kind={} root={} generation={} parent_id={:?} outcome=rejected argv_count=0 argv_repr=[] argv_fingerprint=none error={}",
        kind,
        root,
        generation,
        parent_id,
        bounded_log_text(error, SURELOG_LOG_ERROR_MAX),
    );
}

/// Serialises [`analyze`] calls: Surelog's global C++ singletons are not
/// thread-safe, and the stdout redirect below must not nest.
static ANALYZE_LOCK: Mutex<()> = Mutex::new(());

/// Run the full pipeline (compile + elaborate + model build + token
/// collection + lint) in one blocking call with the default lint
/// configuration and return owned results.
///
/// The Surelog session is dropped before returning.  This never panics: if the
/// compile step itself fails to start, the returned [`Analysis`] carries a
/// single synthetic fatal [`Diag`] with no file, an empty model, and no tokens.
#[cfg_attr(not(test), allow(dead_code))]
pub fn analyze(opts: &CompileOpts) -> Analysis {
    analyze_with_config(opts, &LintConfig::default())
}

/// Run the full pipeline (compile + elaborate + model build + token
/// collection + lint) in one blocking call with `lint_cfg` and return owned
/// results.
///
/// The lint pass honors `lint_cfg` (per-rule enablement + severity overrides);
/// the findings are resolved before the result is returned, so [`Analysis`]
/// itself does not carry the config.  Same guarantees as [`analyze`].
///
/// The process CWD is parked inside [`analysis_scratch_dir`] for the duration
/// of the blocking compile (see that function for why this is required and
/// why a single process-wide directory is used).
pub fn analyze_with_config(opts: &CompileOpts, lint_cfg: &LintConfig) -> Analysis {
    analyze_with_config_context(opts, lint_cfg, "-", 0)
}

/// Analyze with root/generation metadata used only for lifecycle logging.
pub(crate) fn analyze_with_config_context(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
) -> Analysis {
    analyze_with_config_context_parent(opts, lint_cfg, root, generation, None)
}

/// Analyze with an optional parent lifecycle ID supplied by the LSP job
/// scheduler.  The parent is metadata only; all work remains serialized by
/// the same Surelog/CWD guard as the ordinary entry point.
pub(crate) fn analyze_with_config_context_parent(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) -> Analysis {
    let mut analysis_span = crate::logging::LifecycleSpan::analysis_with_parent(
        "workspace",
        || root.to_owned(),
        generation,
        opts.files.len(),
        parent_id,
    );
    let mut wait_span = crate::logging::LifecycleSpan::phase_with_parent(
        "surelog.wait_global_mutex",
        || root.to_owned(),
        generation,
        opts.files.len(),
        Some(analysis_span.id()),
    );
    let _guard = ANALYZE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    wait_span.outcome("ok");
    drop(wait_span);
    let _cwd = ScratchCwd::enter(&analysis_scratch_dir());
    let analysis = analyze_inner(opts, lint_cfg, root, generation, Some(analysis_span.id()));
    analysis_span.complete(
        match analysis.outcome {
            AnalysisOutcome::Valid => "ok",
            AnalysisOutcome::Fatal | AnalysisOutcome::Parse | AnalysisOutcome::Compile => "error",
        },
        analysis.diagnostics.len() + analysis.lint.len(),
    );
    analysis
}

/// The process-wide scratch directory Surelog writes its CWD-relative
/// side-effects into: `<shadow base>/work/analyze/`.
///
/// Surelog enables `set_write_pp_output()` and dumps `slpp_all/`,
/// `surelog.log`, cache files and UHDM output into the *process* CWD.  Worse,
/// Surelog's internal `FileSystem` singleton captures `current_path()` when
/// the FIRST session of the process is created and keeps using it for every
/// later session — a per-job chdir therefore cannot redirect individual jobs.
/// Every analysis in this process thus runs with the CWD parked inside this
/// one directory under the private per-process shadow base, so no project or
/// external tree ever receives those artifacts no matter which analysis runs
/// first.  The redirect happens inside the [`ANALYZE_LOCK`] critical section
/// because chdir is process-global; jobs are serialized there (and by the
/// backend's shadow-staging lock, which also cleans this directory between
/// jobs).
pub fn analysis_scratch_dir() -> std::path::PathBuf {
    process_shadow_base().join("work").join("analyze")
}

/// Guard that parks the process CWD inside a scratch directory and restores
/// the previous directory on drop (including on panic/unwind).
///
/// On failure to create or enter the scratch dir the guard is a no-op and
/// analysis runs with the inherited CWD, matching the pre-containment
/// behavior instead of failing the job.
struct ScratchCwd {
    previous: Option<PathBuf>,
}

impl ScratchCwd {
    fn enter(scratch: &Path) -> Self {
        let previous = std::env::current_dir().ok();
        let redirected = previous.is_some()
            && std::fs::create_dir_all(scratch).is_ok()
            && std::env::set_current_dir(scratch).is_ok();
        Self {
            previous: if redirected { previous } else { None },
        }
    }
}

impl Drop for ScratchCwd {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            let _ = std::env::set_current_dir(previous);
        }
    }
}

fn analyze_inner(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) -> Analysis {
    let files = opts.files.len();
    let mut surelog_span = crate::logging::LifecycleSpan::phase_with_parent(
        "surelog.session_construct_parse_compile_elaborate",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let surelog_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=surelog.session_construct.begin root={} generation={} parent_id={:?} files={}",
        root,
        generation,
        parent_id,
        files
    );
    if crate::logging::enabled(crate::logging::Level::Debug) {
        match compile::compile_invocation(opts) {
            Ok(invocation) => {
                log_surelog_invocation("compile", &invocation, root, generation, parent_id)
            }
            Err(error) => {
                log_surelog_invocation_rejected("compile", &error, root, generation, parent_id)
            }
        }
    }
    let out = match compile::compile(opts) {
        Ok(out) => out,
        Err(msg) => {
            crate::llg_debug!(
                "event=surelog.session_construct.end outcome=error root={} generation={} parent_id={:?} elapsed_us={} error={}",
                root,
                generation,
                parent_id,
                surelog_started.elapsed().as_micros(),
                bounded_log_text(&msg, SURELOG_LOG_ERROR_MAX)
            );
            surelog_span.outcome("error");
            return Analysis::fatal_preflight(msg);
        }
    };
    crate::llg_debug!(
        "event=surelog.session_construct.end outcome=ok root={} generation={} parent_id={:?} diagnostics={} elapsed_us={}",
        root,
        generation,
        parent_id,
        out.diagnostics.len(),
        surelog_started.elapsed().as_micros()
    );
    crate::llg_debug!(
        "event=surelog.compile.return outcome={} root={} generation={} diagnostics={} ok={} uhdm={} design={} elapsed_us={}",
        if out.ok() { "ok" } else { "error" },
        root,
        generation,
        out.diagnostics.len(),
        out.ok(),
        out.uhdm_design().is_some(),
        out.design().is_some(),
        surelog_started.elapsed().as_micros()
    );
    surelog_span.complete(if out.ok() { "ok" } else { "error" }, out.diagnostics.len());
    drop(surelog_span);

    // `uhdm_design` / `design` are session-scoped: everything must be done
    // while `out` (and its session) is still alive, before we move the
    // owned diagnostics out at the end.
    let uhdm = out.uhdm_design();
    let design = out.design();
    // Capture the source graph before the session is dropped.  This is a
    // single parse-tree walk alongside the existing token/connection walks;
    // the owned result is the only source-graph data a request may read.
    let mut graph_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.source_graph",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let graph_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=analysis.source_graph.begin root={} generation={} files={}",
        root,
        generation,
        files
    );
    let mut module_graph = design
        .as_ref()
        .map(collect_module_graph)
        .unwrap_or_default();
    graph_span.complete("ok", module_graph.definitions.len());
    crate::llg_debug!(
        "event=analysis.source_graph.end outcome=ok root={} generation={} definitions={} elaborated_types={} elapsed_us={}",
        root,
        generation,
        module_graph.definitions.len(),
        module_graph.elaborated_types.len(),
        graph_started.elapsed().as_micros()
    );
    drop(graph_span);
    let mut elaborated_type_ranges = Vec::new();

    // The db is owned and outlives the session, so the lint pass can run here
    // over the same db + model that the LSP features consume.
    //
    // Parse-tree fallback: any Severity::Syntax error makes Surelog skip its
    // whole compile/elaborate/UHDM stage (`CheckCompile` gates on the syntax
    // count), so `uhdm` is None while the parse-tree design survives.  In
    // that shape the model/tokens are synthesized from the parse tree alone
    // (declaration-level data; see [`parse_tree_feature_parts`]), the lint
    // pass stays off, and diagnostics are unchanged — the outcome below is
    // still Parse, but [`Analysis::has_feature_data`] now holds.
    // Parse-tree named-port-connection pairing, scanned while the design is
    // alive.  Used in BOTH modes: UHDM mode pairs resolved label targets
    // (child port) with the actual positions (parent-scope declaration);
    // the parse fallback resolves the label against the recorded port
    // declarations and the actual against the parent scope.
    let mut parse_facts_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.parse_facts",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let parse_facts_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=analysis.parse_facts.begin root={} generation={} design={}",
        root,
        generation,
        design.is_some()
    );
    let conn_pairs = match design.as_ref() {
        Some(d) => scan_named_port_connections(d),
        None => Vec::new(),
    };
    let enum_facts = match design.as_ref() {
        Some(d) => scan_parse_enum_facts(d),
        None => ParseEnumFacts::default(),
    };
    parse_facts_span.complete("ok", conn_pairs.len());
    crate::llg_debug!(
        "event=analysis.parse_facts.end outcome=ok root={} generation={} connections={} enum_decls={} enum_bindings={} enum_unresolved={} elapsed_us={}",
        root,
        generation,
        conn_pairs.len(),
        enum_facts.declarations.len(),
        enum_facts.bindings.len(),
        enum_facts.unresolved_positions.len(),
        parse_facts_started.elapsed().as_micros()
    );
    drop(parse_facts_span);

    let (model, lint_diags, db_built, db_error, tokens, uhdm_bindings, decl_details, connections) =
        match uhdm {
            Some(h) => {
                let mut db_span = crate::logging::LifecycleSpan::phase_with_parent(
                    "analysis.db_build",
                    || root.to_owned(),
                    generation,
                    files,
                    parent_id,
                );
                let db_started = std::time::Instant::now();
                crate::llg_debug!(
                    "event=analysis.db_build.begin root={} generation={} uhdm=true",
                    root,
                    generation
                );
                match llg::core::db::Db::build(h) {
                    Ok(db) => {
                        db_span.complete("ok", db.node_count());
                        crate::llg_debug!(
                            "event=analysis.db_build.end outcome=ok root={} generation={} nodes={} tops={} flat_modules={} packages={} classes={} type_ranges={} elapsed_us={}",
                            root,
                            generation,
                            db.node_count(),
                            db.tops.len(),
                            db.flat_modules.len(),
                            db.packages.len(),
                            db.classes.len(),
                            db.elaborated_type_ranges().len(),
                            db_started.elapsed().as_micros()
                        );
                        drop(db_span);
                        elaborated_type_ranges = db
                            .elaborated_type_ranges()
                            .iter()
                            .map(|entry| ModuleGraphElaboratedType {
                                instance: entry.instance.clone(),
                                name: entry.name.clone(),
                                packed_ranges: entry
                                    .packed_ranges
                                    .iter()
                                    .map(|range| {
                                        range.map(|range| ModuleGraphPackedRange {
                                            left: range.left,
                                            right: range.right,
                                        })
                                    })
                                    .collect(),
                            })
                            .collect();
                        let mut model_span = crate::logging::LifecycleSpan::phase_with_parent(
                            "analysis.model_projection",
                            || root.to_owned(),
                            generation,
                            files,
                            parent_id,
                        );
                        let model_started = std::time::Instant::now();
                        let model = llg::core::model::DesignModel::from_db(&db);
                        model_span.complete("ok", model.modules.len() + model.top_instances.len());
                        crate::llg_debug!(
                            "event=analysis.model.end outcome=ok root={} generation={} modules={} tops={} packages={} classes={} elapsed_us={}",
                            root,
                            generation,
                            model.modules.len(),
                            model.top_instances.len(),
                            model.packages.len(),
                            model.classes.len(),
                            model_started.elapsed().as_micros()
                        );
                        drop(model_span);
                        let mut lint_span = crate::logging::LifecycleSpan::phase_with_parent(
                            "analysis.lint",
                            || root.to_owned(),
                            generation,
                            files,
                            parent_id,
                        );
                        let lint_started = std::time::Instant::now();
                        let lint_diags = lint::lint_with_config(&db, &model, lint_cfg);
                        lint_span.complete("ok", lint_diags.len());
                        crate::llg_debug!(
                            "event=analysis.lint.end outcome=ok root={} generation={} findings={} elapsed_us={}",
                            root,
                            generation,
                            lint_diags.len(),
                            lint_started.elapsed().as_micros()
                        );
                        drop(lint_span);
                        let mut token_span = crate::logging::LifecycleSpan::phase_with_parent(
                            "analysis.token_collection",
                            || root.to_owned(),
                            generation,
                            files,
                            parent_id,
                        );
                        let tokens_started = std::time::Instant::now();
                        let (tokens, uhdm_bindings, decl_details) = match design.as_ref() {
                            Some(d) => tokens::collect_all_tokens(h, d),
                            None => (Vec::new(), HashMap::new(), HashMap::new()),
                        };
                        let token_count = token_cardinality(&tokens);
                        token_span.complete("ok", token_count);
                        crate::llg_debug!(
                            "event=analysis.tokens.end outcome=ok root={} generation={} files={} nodes={} bindings={} decl_details={} elapsed_us={}",
                            root,
                            generation,
                            tokens.len(),
                            token_count,
                            uhdm_bindings.len(),
                            decl_details.len(),
                            tokens_started.elapsed().as_micros()
                        );
                        drop(token_span);
                        // Elaborated analysis: DECL/REF classification comes from
                        // the model + multi-view histogram; parse-side positions
                        // must NOT override it (a use-before-decl would flip to
                        // DECL).
                        (
                            model,
                            lint_diags,
                            true,
                            None,
                            tokens,
                            uhdm_bindings,
                            decl_details,
                            ConnectionInputs {
                                parse_decls: None,
                                pairs: conn_pairs,
                                fallback_bindings: HashMap::new(),
                                parse_enum_decls: enum_facts.declarations.clone(),
                                parse_enum_bindings: enum_facts.bindings.clone(),
                                parse_enum_ref_positions: enum_facts.reference_positions.clone(),
                                unresolved_enum_refs: enum_facts.unresolved_positions.clone(),
                                parse_enum_tokens: enum_facts.synthetic_tokens.clone(),
                            },
                        )
                    }
                    Err(error) => {
                        db_span.outcome("error");
                        crate::llg_debug!(
                            "event=analysis.db_build.end outcome=error root={} generation={} error={} elapsed_us={}",
                            root,
                            generation,
                            error,
                            db_started.elapsed().as_micros()
                        );
                        (
                            empty_design(),
                            Vec::new(),
                            false,
                            Some(error),
                            Vec::new(),
                            HashMap::new(),
                            HashMap::new(),
                            ConnectionInputs::default(),
                        )
                    }
                }
            }
            None => {
                let mut fallback_span = crate::logging::LifecycleSpan::phase_with_parent(
                    "analysis.parse_fallback",
                    || root.to_owned(),
                    generation,
                    files,
                    parent_id,
                );
                crate::llg_debug!(
                    "event=analysis.parse_fallback.begin root={} generation={} files={}",
                    root,
                    generation,
                    files
                );
                let fallback_started = std::time::Instant::now();
                let (model, tokens, connections) = match design.as_ref() {
                    Some(d) => parse_tree_feature_parts(
                        d,
                        &conn_pairs,
                        &enum_facts,
                        root,
                        generation,
                        parent_id,
                    ),
                    None => (empty_design(), Vec::new(), ConnectionInputs::default()),
                };
                let token_count = token_cardinality(&tokens);
                fallback_span.complete("ok", token_count);
                crate::llg_debug!(
                    "event=analysis.parse_fallback.end outcome=ok root={} generation={} modules={} token_files={} token_nodes={} bindings={} elapsed_us={}",
                    root,
                    generation,
                    model.modules.len(),
                    tokens.len(),
                    token_count,
                    connections.fallback_bindings.len(),
                    fallback_started.elapsed().as_micros()
                );
                (
                    model,
                    Vec::new(),
                    false,
                    None,
                    tokens,
                    HashMap::new(),
                    HashMap::new(),
                    connections,
                )
            }
        };

    module_graph.elaborated_types = elaborated_type_ranges;

    let has_design = design.is_some();
    let outcome = outcome_from_pipeline(&out, uhdm.is_some(), has_design, db_built);
    // `design` borrows from `out`, so release the last native borrow before moving
    // the diagnostics field. Every borrowed Design/VPI value has now been converted
    // to owned Rust data, making post-processing independent of the native session.
    let _ = design;

    let mut diagnostics = out.diagnostics;
    if let Some(error) = db_error {
        diagnostics.push(db_build_diagnostic(&error));
    }

    // Release the native compiler/session before reading source files again or
    // allocating the macro table and symbol index.
    let mut drop_span = crate::logging::LifecycleSpan::phase_with_parent(
        "surelog.session_drop",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let session_drop_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=surelog.session_drop.begin root={} generation={} diagnostics={} uhdm={} design={}",
        root,
        generation,
        diagnostics.len(),
        uhdm.is_some(),
        has_design
    );
    drop(out.session);
    drop_span.complete("ok", diagnostics.len());
    crate::llg_debug!(
        "event=surelog.session_drop.end outcome=ok root={} generation={} elapsed_us={}",
        root,
        generation,
        session_drop_started.elapsed().as_micros()
    );
    drop(drop_span);
    // Macro table: config `[compile] defines` plus one conservative scan per
    // compiled file.  The files are read exactly as Surelog saw them (shadow
    // paths carry open-buffer text), so the table matches what preprocessing
    // consumed; unreadable files simply contribute no macro data.  Built
    // HERE — once per commit, never in a request.
    let mut macro_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.macro_table",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let macro_started = std::time::Instant::now();
    let macro_sources: Vec<(String, String)> = opts
        .files
        .iter()
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| (path.clone(), text))
        })
        .collect();
    let macro_borrowed: Vec<(&str, &str)> = macro_sources
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let macro_table = macros::build_table(&opts.defines, &macro_borrowed, None);
    macro_span.complete("ok", macro_sources.len());
    crate::llg_debug!(
        "event=analysis.macro_table.end outcome=ok root={} generation={} source_files={} config_defines={} empty={} elapsed_us={}",
        root,
        generation,
        macro_sources.len(),
        macro_table.config_defines(),
        macro_table.is_empty(),
        macro_started.elapsed().as_micros()
    );
    drop(macro_span);
    let mut index_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.symbol_index",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let index_started = std::time::Instant::now();
    let analysis = Analysis::new_with_outcome(
        outcome,
        diagnostics,
        model,
        tokens,
        lint_diags,
        uhdm_bindings,
        connections,
    );
    index_span.complete("ok", analysis.index.decls.len());
    crate::llg_debug!(
        "event=analysis.symbol_index.end outcome=ok root={} generation={} declarations={} references={} bindings={} lint={} elapsed_us={}",
        root,
        generation,
        analysis.index.decls.len(),
        analysis.index.refs.len(),
        analysis.ref_bindings.len(),
        analysis.lint.len(),
        index_started.elapsed().as_micros()
    );
    drop(index_span);
    let mut assembly_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.assembly",
        || root.to_owned(),
        generation,
        files,
        parent_id,
    );
    let assembly_started = std::time::Instant::now();
    let analysis = analysis
        .with_macros(macro_table)
        .with_decl_details(decl_details)
        .with_module_graph(module_graph)
        .with_configured_top(opts.top.clone());
    assembly_span.complete("ok", analysis.tokens.len());
    crate::llg_debug!(
        "event=analysis.assembly.end outcome=ok root={} generation={} outcome_analysis={:?} diagnostics={} lint={} token_files={} token_nodes={} declarations={} references={} elapsed_us={}",
        root,
        generation,
        analysis.outcome,
        analysis.diagnostics.len(),
        analysis.lint.len(),
        analysis.tokens.len(),
        token_node_count(&analysis.tokens),
        analysis.index.decls.len(),
        analysis.index.refs.len(),
        assembly_started.elapsed().as_micros()
    );
    analysis
}

/// Build the owned source-level module graph while the Surelog parse tree is
/// still available.  This deliberately does not ask Surelog to elaborate
/// anything: the graph is the declaration/instantiation evidence that must
/// survive when `-top` removes a definition from UHDM's elaborated tree.
fn collect_module_graph(design: &llg::ffi::surelog::Design) -> ModuleGraph {
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
enum GraphDeclarationKind {
    Port(Direction),
    Parameter(bool),
    Signal(String),
}

#[derive(Debug, Clone)]
struct GraphDefinitionSpan {
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
struct GraphDeclarationSubtreeFacts {
    type_info: TypeInfo,
    direction: Direction,
    net_kind: String,
    variable_kind: String,
}

#[derive(Debug, Default)]
struct GraphDeclarationFactsCache {
    by_root: HashMap<usize, GraphDeclarationSubtreeFacts>,
    #[cfg(test)]
    subtree_walks: usize,
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
struct GraphLineMetadata {
    start: usize,
    end: usize,
    /// Number of Unicode scalar values in the line, excluding its line break.
    character_count: usize,
    /// Relative byte offsets for every `GRAPH_CHARACTER_CHECKPOINT_STRIDE`
    /// scalar values on a non-ASCII line.  ASCII lines use the direct byte
    /// offset path and keep this empty, avoiding one entry per character.
    character_checkpoints: Vec<usize>,
    clause_start_offsets: Vec<(usize, char)>,
    clause_end_offsets: Vec<usize>,
}

const GRAPH_CHARACTER_CHECKPOINT_STRIDE: usize = 64;

/// Immutable, per-file source facts used by the source graph.
///
/// The original text is retained for exact detail/type spelling.  `masked`
/// has the same byte length as the original and replaces comment bytes with
/// spaces, so structural scans can walk it without repeatedly rebuilding
/// comment ranges.  `stripped` retains the historical one-space-per-comment
/// character form used for declaration-line details; unlike `masked`, it is
/// not required to preserve byte offsets.
#[derive(Debug)]
struct GraphSourceIndex {
    source: String,
    masked: String,
    stripped: String,
    line_starts: Vec<usize>,
    source_lines: Vec<GraphLineMetadata>,
    stripped_lines: Vec<GraphLineMetadata>,
    comment_ranges: Vec<(usize, usize)>,
    declaration_boundary_events: Vec<(usize, usize)>,
    delimiter_state_events: Vec<(usize, GraphDelimiterState)>,
    commas_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    declaration_tail_events_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    equals_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
}

impl GraphSourceIndex {
    fn new(source: String) -> Self {
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

    fn line_start(&self, line: u32) -> Option<usize> {
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
    fn line_text(&self, line: u32) -> Option<&str> {
        self.stripped_line_text(line)
    }

    fn stripped_position_offset(&self, line: u32, col: u32) -> Option<usize> {
        let line = self.stripped_line(line)?;
        let character = usize::try_from(col.checked_sub(1)?).ok()?;
        graph_line_character_offset(&self.stripped, line, character)
    }

    fn position_offset(&self, line: u32, col: u32) -> Option<usize> {
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
struct GraphDelimiterState {
    square: usize,
    paren: usize,
    brace: usize,
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
struct GraphDelimiterFrame {
    character: char,
    offset: usize,
    previous_boundary: Option<usize>,
    previous_tops: [Option<usize>; 3],
}

fn graph_delimiter_slot(delimiter: char) -> Option<usize> {
    match delimiter {
        '[' => Some(0),
        '(' => Some(1),
        '{' => Some(2),
        _ => None,
    }
}

#[derive(Debug)]
struct GraphSourceSyntaxFacts {
    declaration_boundary_events: Vec<(usize, usize)>,
    delimiter_state_events: Vec<(usize, GraphDelimiterState)>,
    commas_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    declaration_tail_events_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
    equals_by_state: HashMap<GraphDelimiterState, Vec<usize>>,
}

fn graph_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
    );
    starts
}

fn graph_line_metadata(
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

fn graph_line_character_metadata(line: &str) -> (usize, Vec<usize>) {
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

fn graph_line_character_offset(
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

fn graph_indexed_line_start(starts: &[usize], text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let index = usize::try_from(line - 1).ok()?;
    starts.get(index).copied().or_else(|| {
        (index == starts.len() && !text.is_empty() && !text.ends_with('\n')).then_some(text.len())
    })
}

fn graph_masked_delimiter_events(masked: &str) -> Vec<(usize, char)> {
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

fn graph_matching_openers(events: &[(usize, char)]) -> HashSet<usize> {
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
fn graph_source_syntax_facts(masked: &str) -> GraphSourceSyntaxFacts {
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
fn graph_declaration_boundary_events(masked: &str) -> Vec<(usize, usize)> {
    graph_source_syntax_facts(masked).declaration_boundary_events
}

fn mask_graph_comments(text: &str, ranges: &[(usize, usize)]) -> String {
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

fn strip_graph_comments_with_ranges(text: &str, ranges: &[(usize, usize)]) -> String {
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

fn graph_node_type(
    _nodes: &[llg::ffi::surelog::ParseNode],
    type_id: u16,
) -> Option<llg::core::vobject_types::VObjectType> {
    llg::core::vobject_types::VObjectType::try_from(type_id).ok()
}

fn graph_position(node: &llg::ffi::surelog::ParseNode, index: usize) -> (u32, u32, usize) {
    (node.line, node.col as u32, index)
}

fn graph_subtree_indices(nodes: &[llg::ffi::surelog::ParseNode], root: usize) -> Vec<usize> {
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

fn graph_first_string(
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

fn graph_ancestors(nodes: &[llg::ffi::surelog::ParseNode], start: usize) -> Vec<usize> {
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

fn graph_has_ancestor(
    nodes: &[llg::ffi::surelog::ParseNode],
    start: usize,
    wanted: impl Fn(llg::core::vobject_types::VObjectType) -> bool,
) -> bool {
    graph_ancestors(nodes, start)
        .into_iter()
        .filter_map(|index| graph_node_type(nodes, nodes[index].type_id))
        .any(wanted)
}

fn graph_declaration_subtree_facts(
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

fn graph_declaration_kind(
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

fn graph_token_matches_declaration(token_type: i32, kind: &GraphDeclarationKind) -> bool {
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
fn graph_type_info(
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

fn graph_source_type_info(parts: Option<&GraphSourceTypeParts>) -> Option<TypeInfo> {
    let parts = parts?;
    let (ty, saw_decl_qualifier) = graph_source_type_words_clean(&parts.base);
    (ty.kind != "other" || saw_decl_qualifier).then_some(ty)
}

#[cfg(test)]
fn graph_source_type_words(text: &str) -> (TypeInfo, bool) {
    let text = strip_hdl_comments(text);
    graph_source_type_words_clean(&text)
}

fn graph_source_type_words_clean(text: &str) -> (TypeInfo, bool) {
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

fn graph_source_words(text: &str) -> Vec<String> {
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

fn graph_width_from_source(parts: Option<&GraphSourceTypeParts>) -> Option<u32> {
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

fn graph_declaration_location(
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
struct GraphSourceTypeParts {
    base: String,
    packed_dimensions: Vec<String>,
    unpacked_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphTypeDisplay {
    text: Option<String>,
    shape: ModuleGraphTypeShape,
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
fn graph_type_prefix(
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
fn graph_source_declaration_start(source: &GraphSourceIndex, name_start: usize) -> Option<usize> {
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

fn graph_trim_range(source: &str, start: usize, end: usize) -> (usize, usize) {
    let text = &source[start..end];
    let trimmed_start = start + text.len() - text.trim_start().len();
    let trimmed = &source[trimmed_start..end];
    (trimmed_start, trimmed_start + trimmed.trim_end().len())
}

fn graph_select_type_prefix_index(
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

fn graph_strip_trailing_declarator_name_index(
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

fn graph_top_level_commas_index(
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

fn graph_bracket_spans_index(
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

fn graph_bracket_dimensions_index(
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

fn remove_bracket_dimensions_index(
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

fn graph_comment_ranges(text: &str) -> Vec<(usize, usize)> {
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
struct GraphCommentCursor<'a> {
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
fn strip_hdl_comments(text: &str) -> String {
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

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
}

#[cfg(test)]
fn graph_bracket_spans(text: &str) -> Vec<(usize, usize, String)> {
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
fn normalize_graph_type_display(text: &str) -> String {
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
fn normalize_graph_lexical_whitespace(text: &str) -> String {
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
fn normalize_graph_lexical_whitespace_clean(text: &str) -> String {
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

fn normalize_graph_lexical_whitespace_masked(text: &str) -> String {
    normalize_graph_lexical_whitespace_clean(text)
}

fn source_line_start(source: &GraphSourceIndex, line: u32) -> Option<usize> {
    source.line_start(line)
}

/// Convert a 1-based source line/character position into a UTF-8 byte offset.
/// Surelog's parse columns are character-based, while source slicing is
/// byte-based.
fn source_position_offset(source: &GraphSourceIndex, line: u32, col: u32) -> Option<usize> {
    source.position_offset(line, col)
}

/// Convert a graph source column to a 1-based UTF-16 column for owned graph
/// locations.  Parse columns are not consistent across all Surelog paths when
/// non-ASCII text precedes a token, so an available name is used to choose the
/// scalar, UTF-16, or byte interpretation that actually starts that name.
fn graph_lsp_column(
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

fn graph_line_utf16_offset(
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
fn graph_declaration_tail_end(source: &GraphSourceIndex, start: usize) -> usize {
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

fn graph_top_level_equals_index(
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
fn graph_bracket_dimensions(prefix: &str) -> Vec<String> {
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
fn normalize_symbolic_expression(expression: &str) -> String {
    let comment_ranges = graph_comment_ranges(expression);
    let mut comment_cursor = GraphCommentCursor::new(&comment_ranges);
    normalize_symbolic_expression_with_cursor(expression, 0, &mut comment_cursor)
}

fn normalize_symbolic_expression_index(
    source: &GraphSourceIndex,
    start: usize,
    end: usize,
) -> String {
    let expression = &source.source[start..end];
    let mut comment_cursor = source.comment_cursor_at(start);
    normalize_symbolic_expression_with_cursor(expression, start, &mut comment_cursor)
}

fn normalize_symbolic_expression_with_cursor(
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

/// Canonical type text retained beside the structured [`TypeInfo`].  The
/// source graph keeps packed and unpacked dimensions in declaration order,
/// while the shape tells the explorer which suffix dimensions must remain
/// unpacked when a committed instance width replaces the packed portion.
// Keep the structured type, source spelling, and declaration coordinates
// explicit at this analysis boundary.
#[allow(clippy::too_many_arguments)]
fn graph_type_display(
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

fn graph_display_base(text: &str) -> Option<String> {
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

fn graph_parameter_display_base(text: &str) -> Option<String> {
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

fn graph_declaration_detail(
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

fn graph_direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "port",
    }
}

fn graph_instantiation_type(
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

fn graph_generate_ancestors(
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

fn graph_is_generate_scope(ty: llg::core::vobject_types::VObjectType) -> bool {
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

fn graph_generate_name(
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

fn graph_instance_key(instance: &ModuleGraphInstance) -> GraphInstanceKey {
    (
        instance.name.clone(),
        instance.module_type.clone(),
        instance.file.clone(),
        instance.line,
        instance.col,
    )
}

fn graph_push_instance(
    target: &mut Vec<ModuleGraphInstance>,
    keys: &mut HashSet<GraphInstanceKey>,
    instance: ModuleGraphInstance,
) {
    if keys.insert(graph_instance_key(&instance)) {
        target.push(instance);
    }
}

fn graph_scope_key(scope: &ModuleGraphGenerateScope) -> GraphScopeKey {
    (scope.name.clone(), scope.line, scope.col)
}

fn graph_scope_indices(
    indexes: &HashMap<Vec<GraphScopeKey>, usize>,
    path: &[GraphScopeKey],
) -> Option<Vec<usize>> {
    let mut result = Vec::with_capacity(path.len());
    for depth in 1..=path.len() {
        result.push(*indexes.get(&path[..depth].to_vec())?);
    }
    Some(result)
}

fn graph_generated_scope_mut<'a>(
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

fn graph_push_generated_instance(
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

fn graph_instance_cmp(
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

fn graph_sort_scopes(scopes: &mut [ModuleGraphGenerateScope]) {
    scopes.sort_by(|left, right| {
        (left.name.as_str(), left.line, left.col).cmp(&(right.name.as_str(), right.line, right.col))
    });
    for scope in scopes {
        scope.children.sort_by(graph_instance_cmp);
        graph_sort_scopes(&mut scope.nested);
    }
}

/// Build declaration-level feature parts from Surelog's parse tree alone.
///
/// Serves syntax-broken projects: Surelog skips compile/elaborate/UHDM when a
/// `Severity::Syntax` diagnostic exists (`vendor/Surelog/src/SourceCompile/
/// Compiler.cpp`: `parseOk && …compile()` behind `CheckCompile`), leaving
/// `uhdm_design() == None` while the PARSE-tree design is available.  The
/// synthesized parts mirror what the full pipeline would provide, reduced to
/// what the parse tree can classify:
///
/// * **Tokens** come from `core::tokens::collect_parse_tokens` (keywords,
///   macro usage/definition sites, declaration-name identifiers).  Named
///   connection labels (`.clk` in `m u0(.clk(c))`, `.W` in `m #(.W(4))`)
///   arrive pre-classified as the LSP-internal `TOKEN_*_CONN_LABEL` types,
///   which [`classify_token`] maps to plain reference entries — so they stay
///   indexed, visible to the dump and bindable (see below) without ever
///   surfacing phantom function/task declarations.
/// * **Model** is modules-only: one [`ModuleDef`] per module-name identifier
///   the parse pass classified under a module header
///   (`paModule_ansi_header`/`paModule_nonansi_header`, surfaced as
///   `vpiModule`-typed tokens), carrying name + declaration position.  Ports,
///   signals and parameters are NOT modeled: the model represents them per
///   elaborated instance, and there is none.  `design_name` and
///   `top_instances` stay empty.  Parse-backed enum declarations are indexed
///   separately so package/class members remain navigable without UHDM.
/// * **Connection bindings** come from the parse-tree pairing
///   ([`scan_named_port_connections`]): for every named port connection
///   whose enclosing instantiation's module type declares a port matching
///   the label name, the label position is bound to that child-module port
///   declaration (`via_label`) while the connected signal's position is
///   bound to its OWN declaration in the instantiating scope
///   (`via_connection`).  Unresolvable connections are skipped silently.
///
/// Consumers degrade gracefully over this shape: the symbol index picks up
/// module/class/interface/function/task/typedef/enum-const declarations plus
/// module-type references from the parse tokens.  Parse-backed enum uses are
/// bound when their package/class/import target is unique; ports/signals/params
/// (gated on instance data in `SymbolIndex::from_parts`) remain unindexed.
/// The outcome is unaffected — diagnostics alone drive it — this only makes
/// [`Analysis::has_feature_data`] true for the no-UHDM-but-parsed shape.
fn parse_tree_feature_parts(
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
fn fallback_actual_target(
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
fn fallback_decl_kind(vpi_type: i32) -> &'static str {
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
struct FallbackPortDecl {
    /// Name of the enclosing module (library prefixes already stripped).
    module: String,
    /// Port name at the declaration position.
    name: String,
    /// Absolute file path of the declaration.
    file: String,
    /// 1-based declaration line.
    line1: u32,
    /// 1-based declaration column.
    col1: u32,
}

/// One parse-tree PARAMETER declaration attributed to its enclosing module
/// header.
struct FallbackParamDecl {
    /// Name of the enclosing module (library prefixes already stripped).
    module: String,
    /// Parameter name at the declaration position.
    name: String,
    /// Absolute file path of the declaration.
    file: String,
    /// 1-based declaration line.
    line1: u32,
    /// 1-based declaration column.
    col1: u32,
}

#[derive(Debug)]
struct ParseFallbackPositionInfo {
    first_node: usize,
    port_node: Option<usize>,
    parameter_node: Option<usize>,
    additional_nodes: Option<Vec<usize>>,
}

struct ParseFallbackFileIndex<'a> {
    nodes: &'a [VObjectInfo],
    positions: HashMap<(u32, u32), ParseFallbackPositionInfo>,
    actual_positions_by_name: HashMap<String, Vec<ActualCandidatePos>>,
    headers: Vec<(u32, String)>,
    spans: Vec<ModuleSpan0>,
}

struct ParseFallbackIndex<'a> {
    files: HashMap<&'a str, ParseFallbackFileIndex<'a>>,
    ports: Vec<FallbackPortDecl>,
    params: Vec<FallbackParamDecl>,
}

fn fallback_port_type(vpi_type: i32) -> bool {
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
    fn new(
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
fn declared_ports_by_module<'a>(index: &'a ParseFallbackIndex<'_>) -> &'a [FallbackPortDecl] {
    &index.ports
}

/// Collect the parameter declaration positions recorded by
/// [`collect_parse_tokens`], attributing each to its enclosing module.  The
/// positions and type checks are pre-indexed by [`ParseFallbackIndex`].
fn declared_params_by_module<'a>(index: &'a ParseFallbackIndex<'_>) -> &'a [FallbackParamDecl] {
    &index.params
}

fn find_fallback_port<'a>(
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

fn find_fallback_param<'a>(
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
fn scan_parse_enum_facts(design: &llg::ffi::surelog::Design) -> ParseEnumFacts {
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

/// An [`Analysis`] with no diagnostics, no lint findings, an empty model, and
/// no tokens.
///
/// Used as the fallback when a compile never ran (e.g. the blocking task
/// panicked) so callers always have a value to serve from.
pub fn empty_analysis() -> Analysis {
    Analysis::new_with_outcome(
        AnalysisOutcome::Compile,
        Vec::new(),
        empty_design(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        ConnectionInputs::default(),
    )
}

fn empty_design() -> DesignModel {
    DesignModel {
        design_name: String::new(),
        top_instances: Vec::new(),
        modules: Vec::new(),
        packages: Vec::new(),
        classes: Vec::new(),
    }
}

// ── Lint settings ─────────────────────────────────────────────────────────────

/// Build a [`LintConfig`] from an LSP settings payload (`initializationOptions`
/// or the `workspace/didChangeConfiguration` `settings` field).
///
/// The documented shape mirrors `llg-lint.toml`:
///
/// ```json
/// {
///   "lint": {
///     "enabled": true,
///     "rules": {
///       "unused-signal": { "enabled": false, "severity": "error" }
///     }
///   }
/// }
/// ```
///
/// `enabled` is the global switch: `false` disables every known rule (per-rule
/// entries below it can re-enable individual rules).  Each `rules.<id>` entry
/// accepts `enabled` (bool) and `severity` (`"error"`, `"warning"` or
/// `"info"`).  A bare `{"rules": ...}` / `{"enabled": ...}` object is also
/// accepted as a shorthand for the `lint` wrapper.  Rules not mentioned keep
/// their defaults (enabled, rule's own severity).
#[allow(dead_code)] // public API entry point (tests / external consumers)
pub fn settings_to_lint_config(settings: &LSPAny) -> LintConfig {
    let mut cfg = LintConfig::default();
    apply_lint_settings(&mut cfg, settings);
    cfg
}

/// Apply a client settings payload to `cfg`, overwriting per-rule entries.
///
/// Rules not mentioned in the payload keep their current configuration, so
/// callers can layer the payload over a base config (e.g. a workspace
/// `llg-lint.toml`) with the client winning per rule.
pub fn apply_lint_settings(cfg: &mut LintConfig, settings: &LSPAny) {
    let Some(lint) = lint_settings_object(settings) else {
        return;
    };
    // Global kill-switch: `"enabled": false` disables every known rule;
    // per-rule entries below can re-enable individual rules.
    if lint.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
        for rule in LintRegistry::default_rules().all() {
            let id = rule.id();
            cfg.set(
                id,
                RuleConfig {
                    enabled: false,
                    severity: cfg.severity(id),
                },
            );
        }
    }
    let Some(rules) = lint.get("rules").and_then(|v| v.as_object()) else {
        return;
    };
    for (id, value) in rules {
        let Some(rule) = value.as_object() else {
            continue;
        };
        let mut rule_cfg = cfg.get(id);
        if let Some(enabled) = rule.get("enabled").and_then(|v| v.as_bool()) {
            rule_cfg.enabled = enabled;
        }
        if let Some(severity) = rule
            .get("severity")
            .and_then(|v| v.as_str())
            .and_then(parse_severity)
        {
            rule_cfg.severity = Some(severity);
        }
        cfg.set(id.clone(), rule_cfg);
    }
}

/// The `lint` settings object from a settings payload: the documented
/// `{"lint": {...}}` wrapper, or the payload itself when it is already shaped
/// like the lint settings (`{"enabled": ...}` / `{"rules": ...}`).
fn lint_settings_object(settings: &LSPAny) -> Option<&LSPObject> {
    let obj = settings.as_object()?;
    match obj.get("lint") {
        Some(LSPAny::Object(lint)) => Some(lint),
        _ if obj.contains_key("rules") || obj.contains_key("enabled") => Some(obj),
        _ => None,
    }
}

/// Map a settings `severity` string to a [`LintSeverity`] (`LintSeverity` has
/// no `FromStr` impl; `core::lint`'s own parser is private).
fn parse_severity(value: &str) -> Option<LintSeverity> {
    match value {
        "error" => Some(LintSeverity::Error),
        "warning" => Some(LintSeverity::Warning),
        "info" => Some(LintSeverity::Info),
        _ => None,
    }
}

// ── Shadow paths ──────────────────────────────────────────────────────────────

/// The deterministic shadow path for a real source file, used by the LSP to
/// compile unsaved editor buffers without touching the on-disk file.
///
/// The mirror keeps the full absolute layout under the private per-process
/// shadow base (a directory under the OS temp dir): `<base>/<real minus
/// leading slash>`.  `real` must be absolute (the LSP only feeds
/// `Url::to_file_path` results).  The mapping is a bijection over any absolute
/// path — workspace or external — so [`real_path`] always recovers the
/// original path.
pub fn shadow_path(real: &Path, base: &Path) -> PathBuf {
    let relative = real.strip_prefix("/").unwrap_or(real);
    base.join(relative)
}

/// Reverse of [`shadow_path`]: recover the real absolute path from a shadow
/// path.  Returns `None` when `shadow` is not under `base`.
pub fn real_path(shadow: &Path, base: &Path) -> Option<PathBuf> {
    let relative = shadow.strip_prefix(base).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Some(PathBuf::from("/").join(relative))
}

/// The process-global private shadow base: `<os temp dir>/llg-<pid>-<rand>`.
///
/// Created once per process and reused by every root.  All staged copies live
/// here, never in the project tree, so compilation and reloads never create,
/// modify, or delete any configured project/external file.  The directory is
/// created eagerly and exclusively (`create_dir`), retrying with a fresh
/// `rand` component on collision, so a pathological PID reuse + identical
/// timestamp can never silently share — or let `cleanup_process_shadow` delete
/// — another process's base.
pub fn process_shadow_base() -> std::path::PathBuf {
    use std::sync::OnceLock;
    static BASE: OnceLock<std::path::PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        let pid = std::process::id();
        let mut last = std::env::temp_dir().join(format!("llg-{pid}"));
        // Eager exclusive creation: a stale directory from a crashed process
        // with the same PID and timestamp must never be silently shared — or
        // deleted by `cleanup_process_shadow`.  Retry with a fresh `rand`
        // component on collision; the bound only guards a pathological
        // frozen-clock/filesystem case (the final fallback matches the
        // historical lazy behavior — call sites create subdirs with
        // `create_dir_all` anyway).
        for _ in 0..16 {
            let rand = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            last = std::env::temp_dir().join(format!("llg-{pid}-{rand}"));
            match std::fs::create_dir(&last) {
                Ok(()) => return last,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return last,
            }
        }
        last
    })
    .clone()
}

/// Remove the entire process shadow base and everything staged in it.
pub fn cleanup_process_shadow() {
    let base = process_shadow_base();
    let _ = std::fs::remove_dir_all(&base);
}

/// Serializes tests (across this binary's unit-test modules) that create,
/// stage into, or clean the shared process-global shadow base, so one test's
/// `cleanup_process_shadow` cannot delete another test's staged files.
#[cfg(test)]
pub(crate) static TEST_PROCESS_SHADOW_LOCK: Mutex<()> = Mutex::new(());

// ── Symbol index ──────────────────────────────────────────────────────────────

/// One indexed symbol occurrence: a declaration site or a reference site.
///
/// Positions are **0-based**, matching the LSP wire format (source positions
/// are converted from Surelog's 1-based values at build time).  `end_col` is
/// exclusive-ish: it equals `col + name length` when the name length is known,
/// which is the common case.
#[derive(Debug, Clone, PartialEq)]
pub struct SymEntry {
    pub name: String,
    pub kind: SymKind,
    /// Absolute source file path.
    pub file: String,
    /// 0-based line of the occurrence.
    pub line: u32,
    /// 0-based column of the occurrence.
    pub col: u32,
    /// 0-based end line (usually `== line`).
    pub end_line: u32,
    /// 0-based end column (exclusive-ish; `col + name length`).
    pub end_col: u32,
    /// `true` for declaration sites, `false` for reference sites.
    pub is_decl: bool,
    /// Enclosing scope name (module/package/instance full name) used to
    /// disambiguate same-named objects; best-effort for references.
    pub scope: Option<String>,
    /// Hover text for the object (SystemVerilog declaration snippet).
    pub detail: Option<String>,
}

/// Workspace-wide symbol index: every declaration and reference site the
/// pipeline knows about, with lookup structures for position/name queries.
///
/// Built by [`SymbolIndex::build`] (or `Analysis::new`) from the design model
/// and the token lists; never touches Surelog afterwards.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// All declaration sites, in build order.
    pub decls: Vec<SymEntry>,
    /// All reference sites, in build order.
    pub refs: Vec<SymEntry>,
    /// Declarations per file, sorted by (line, col).
    decls_by_file: HashMap<String, Vec<SymEntry>>,
    /// References per file, sorted by (line, col).
    refs_by_file: HashMap<String, Vec<SymEntry>>,
    /// Name → indices into `decls` (exact, case-sensitive match).
    decls_by_name: HashMap<String, Vec<usize>>,
    /// Decl index → clean module definition name (for `Instance` decls).
    instance_def: HashMap<usize, String>,
    /// Named port connection labels (`.clk` in `m u0(.clk(c))`), keyed by
    /// label position → index into `decls` of the child module's port
    /// declaration.
    port_labels: HashMap<(String, u32, u32), usize>,
    /// Named parameter override labels (the `W` in `m u0 #(.W(4)) (...)`),
    /// keyed by label position → index into `decls` of the CHILD module's
    /// parameter declaration.
    param_labels: HashMap<(String, u32, u32), usize>,
    /// Positions of scanned named PARAMETER override labels that did NOT
    /// resolve to a child-module parameter.  A definition request at such a
    /// position must return NO result rather than falling through to the
    /// name-based rules, which would jump to a same-named object of the
    /// instantiating scope (a `localparam W` decoy) or an arbitrary
    /// workspace match.
    unresolved_param_labels: HashSet<(String, u32, u32)>,
    /// Parse-backed enum uses for which no unique declaration was found.
    /// These positions must not fall through to the broad name-based resolver.
    unresolved_enum_refs: HashSet<(String, u32, u32)>,
}

/// VPI types that always denote a reference site (from `walk_expr` in
/// `core::tokens`; verified empirically that refs report `vpiRefObj`).
const REF_TOKEN_TYPES: &[i32] = &[
    llg::ffi::vpi::vpiRefObj,
    llg::ffi::vpi::uhdmref_obj,
    llg::ffi::vpi::uhdmref_var,
    llg::ffi::vpi::vpiVarSelect,
];

/// VPI types that denote a net/var/port/parameter object — ambiguous between
/// declaration and reference sites (see the classification in
/// [`SymbolIndex::from_parts`]).
const SIGNAL_DECL_TYPES: &[i32] = &[
    llg::ffi::vpi::vpiNet,
    llg::ffi::vpi::vpiNetBit,
    llg::ffi::vpi::vpiReg,
    llg::ffi::vpi::vpiRegBit,
    llg::ffi::vpi::vpiPort,
    llg::ffi::vpi::vpiPortBit,
    llg::ffi::vpi::vpiLogicVar,
    llg::ffi::vpi::vpiIntegerVar,
    llg::ffi::vpi::vpiRealVar,
    llg::ffi::vpi::vpiTimeVar,
    llg::ffi::vpi::uhdmlogic_var,
    llg::ffi::vpi::uhdmnet,
    llg::ffi::vpi::uhdmlogic_net,
    llg::ffi::vpi::uhdmint_var,
    llg::ffi::vpi::uhdmreal_var,
    llg::ffi::vpi::uhdmbit_var,
    llg::ffi::vpi::uhdmbyte_var,
    llg::ffi::vpi::uhdmshort_int_var,
    llg::ffi::vpi::uhdmlong_int_var,
    llg::ffi::vpi::vpiParameter,
    llg::ffi::vpi::vpiSpecParam,
    llg::ffi::vpi::uhdmparameter,
];

/// VPI types that denote a module/interface instance name at its
/// instantiation site (parse-tree classified; see `paName_of_instance`).
const INSTANCE_NAME_TOKEN_TYPES: &[i32] = &[
    llg::ffi::vpi::uhdmlogic_var,
    llg::ffi::vpi::uhdmmodule_inst,
    llg::ffi::vpi::uhdminterface_inst,
];

/// Maximum distance (0-based lines) between an instance declaration and a
/// named port-connection label on a later line for the multi-line heuristic in
/// [`port_label_candidate`].  Labels farther below their instance than this are
/// not associated with it (they are more likely to belong to a different
/// instantiation or to be unrelated `vpiFunction`/`vpiTask` tokens).
const PORT_LABEL_MAX_SPAN: u32 = 50;

impl SymbolIndex {
    /// Build the index from an [`Analysis`].
    ///
    /// `Analysis::new` builds the index internally via [`SymbolIndex::from_parts`];
    /// this entry point is part of the public API for external consumers.
    #[allow(dead_code)]
    pub fn build(a: &Analysis) -> SymbolIndex {
        SymbolIndex::from_parts(
            &a.model,
            &a.tokens,
            None,
            &[],
            &[],
            &HashSet::new(),
            &HashSet::new(),
        )
    }

    /// Merge indexes produced by independent analysis passes.
    ///
    /// Input order is preserved, and duplicate occurrences at the same
    /// position are kept from the first index.  The lookup maps are rebuilt
    /// from the merged vectors so declaration indices remain deterministic.
    /// Metadata whose values refer to declaration indices (`instance_def`,
    /// `port_labels` and `param_labels`) is remapped as entries are appended;
    /// unresolved-label positions union as-is.
    pub fn merge<I, T>(indices: I) -> SymbolIndex
    where
        I: IntoIterator<Item = T>,
        T: Borrow<SymbolIndex>,
    {
        let mut merged = SymbolIndex::default();
        for index in indices {
            merged.merge_one(index.borrow());
        }
        merged.rebuild_lookup_maps();
        merged
    }

    fn merge_one(&mut self, other: &SymbolIndex) {
        let mut decl_positions: HashMap<(String, u32, u32), usize> = self
            .decls
            .iter()
            .enumerate()
            .map(|(idx, d)| ((d.file.clone(), d.line, d.col), idx))
            .collect();
        let mut decl_remap = Vec::with_capacity(other.decls.len());
        for decl in &other.decls {
            let key = (decl.file.clone(), decl.line, decl.col);
            let new_idx = if let Some(&idx) = decl_positions.get(&key) {
                idx
            } else {
                let idx = self.decls.len();
                self.decls.push(decl.clone());
                decl_positions.insert(key, idx);
                idx
            };
            decl_remap.push(new_idx);
        }

        let mut ref_positions: HashSet<(String, u32, u32)> = self
            .refs
            .iter()
            .map(|r| (r.file.clone(), r.line, r.col))
            .collect();
        for reference in &other.refs {
            let key = (reference.file.clone(), reference.line, reference.col);
            if ref_positions.insert(key) {
                self.refs.push(reference.clone());
            }
        }

        for (&old_idx, def_name) in &other.instance_def {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.instance_def
                    .entry(new_idx)
                    .or_insert_with(|| def_name.clone());
            }
        }
        for (key, &old_idx) in &other.port_labels {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.port_labels.entry(key.clone()).or_insert(new_idx);
            }
        }
        for (key, &old_idx) in &other.param_labels {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.param_labels.entry(key.clone()).or_insert(new_idx);
            }
        }
        self.unresolved_param_labels
            .extend(other.unresolved_param_labels.iter().cloned());
        self.unresolved_enum_refs
            .extend(other.unresolved_enum_refs.iter().cloned());
    }

    fn rebuild_lookup_maps(&mut self) {
        self.decls_by_file.clear();
        self.refs_by_file.clear();
        self.decls_by_name.clear();

        for (idx, decl) in self.decls.iter().enumerate() {
            self.decls_by_name
                .entry(decl.name.clone())
                .or_default()
                .push(idx);
            self.decls_by_file
                .entry(decl.file.clone())
                .or_default()
                .push(decl.clone());
        }
        for reference in &self.refs {
            self.refs_by_file
                .entry(reference.file.clone())
                .or_default()
                .push(reference.clone());
        }
        for entries in self.decls_by_file.values_mut() {
            entries.sort_by_key(|entry| (entry.line, entry.col));
        }
        for entries in self.refs_by_file.values_mut() {
            entries.sort_by_key(|entry| (entry.line, entry.col));
        }
    }

    /// Build the index from the owned model + token parts.
    ///
    /// # Declarations
    ///
    /// * modules and packages from the model (position refined to the *name*
    ///   token in the declaration file when available),
    /// * one entry per instance at its instantiation site,
    /// * functions/tasks from the model (per-instance clones with the
    ///   definition file/position and a signature detail),
    /// * ports / nets / vars / parameters from tokens whose position is a
    ///   declaration site (see below).
    ///
    /// # References
    ///
    /// * token nodes with a reference VPI type (`vpiRefObj`, `uhdmref_obj`,
    ///   `uhdmref_var`, `vpiVarSelect`) or a connection-label synthetic type
    ///   (`TOKEN_PORT_CONN_LABEL`, `TOKEN_PARAM_CONN_LABEL`),
    /// * module type names at instantiation sites (`uhdmclass_defn` tokens
    ///   whose name matches a module definition),
    /// * named port connections (`vpiFunction`/`vpiTask` tokens whose name is
    ///   a known port/signal/parameter, plus every `TOKEN_PORT_CONN_LABEL`
    ///   token); their child port declaration is precomputed into
    ///   `port_labels` (see [`port_label_candidate`] and
    ///   [`resolve_port_label`]),
    /// * named parameter overrides (classifier-labeled `TOKEN_PARAM_CONN_LABEL`
    ///   tokens at scanned `paNamed_parameter_assignment` positions — `pairs`
    ///   supplies both the positions and each instantiation's module type, so
    ///   no positional guessing is involved); their child parameter declaration
    ///   is precomputed into `param_labels` (see [`resolve_param_override`]),
    ///   while unresolvable label positions land in `unresolved_param_labels`.
    ///
    /// # Decl vs. ref for ambiguous signal tokens
    ///
    /// Empirically (Surelog v1.86 + UHDM): a true declaration site carries at
    /// least two tokens (VPI walker + parse tree) at the same position and no
    /// `vpiRefObj` companion, while reference sites have a `vpiRefObj`
    /// companion or a single token.  Port declarations additionally emit the
    /// direction-specific `TOKEN_PORT_*` types, which appear only at
    /// declaration sites.
    /// `parse_decls` carries the parse-tree declaration positions recorded by
    /// [`collect_parse_tokens`].  It is `Some` ONLY for the syntax-broken
    /// fallback path: without instances, `signal_names` would stay empty and
    /// every parse-classified port/net/var token would be dropped by
    /// [`classify_token`]'s gate; seeded sets plus per-position
    /// `forced_decl` restore them.  The elaborated pipeline passes `None` so
    /// UHDM multi-view classification is untouched.
    fn from_parts(
        model: &DesignModel,
        tokens: &[FileTokens],
        parse_decls: Option<&ParseDeclPositions>,
        pairs: &[NamedPortConn],
        parse_enum_decls: &[ParseEnumDecl],
        parse_enum_ref_positions: &HashSet<(String, u32, u32)>,
        unresolved_enum_refs: &HashSet<(String, u32, u32)>,
    ) -> SymbolIndex {
        use llg::ffi::vpi;

        let mut decls: Vec<SymEntry> = Vec::new();
        let mut refs: Vec<SymEntry> = Vec::new();
        let mut claimed: HashSet<(String, u32, u32)> = HashSet::new();
        let mut instance_def: HashMap<usize, String> = HashMap::new();
        let mut port_label_candidates: Vec<PortLabelCandidate> = Vec::new();
        // 0-based positions of scanned parameter-override labels: the exact
        // parse-tree evidence backing `TOKEN_PARAM_CONN_LABEL` tokens (and
        // recorded independently of token classification).
        let param_pair_positions: HashSet<(String, u32, u32)> = pairs
            .iter()
            .filter(|p| p.kind == ConnKind::Param)
            .map(|p| {
                (
                    p.file.clone(),
                    p.label.0.saturating_sub(1),
                    p.label.1.saturating_sub(1),
                )
            })
            .collect();

        // ── Name sets from the model ─────────────────────────────────────────
        let mut signal_names: HashSet<String> = HashSet::new();
        let mut port_names: HashSet<String> = HashSet::new();
        let mut param_names: HashSet<String> = HashSet::new();
        let mut module_names: HashSet<String> = HashSet::new();
        for m in &model.modules {
            module_names.insert(clean_name(&m.name).to_owned());
        }
        for inst in all_instances(&model.top_instances) {
            for p in &inst.ports {
                port_names.insert(p.name.clone());
                signal_names.insert(p.name.clone());
            }
            for s in &inst.signals {
                signal_names.insert(s.name.clone());
            }
            for pa in &inst.params {
                param_names.insert(pa.name.clone());
                signal_names.insert(pa.name.clone());
            }
            for gs in &inst.gen_scopes {
                for pa in &gs.params {
                    param_names.insert(pa.name.clone());
                    signal_names.insert(pa.name.clone());
                }
            }
        }

        let tokens_by_file: HashMap<&str, &FileTokens> =
            tokens.iter().map(|ft| (ft.path.as_str(), ft)).collect();

        // The syntax-fallback declaration set is a HashSet, so walking it
        // directly cannot reuse the token order.  Build the same-position
        // lookup once over the owned token stream; this replaces one
        // `nodes.iter().find` scan per declaration while preserving the old
        // first-node-at-position choice.
        let parse_nodes_by_position = parse_decls.map(|_| {
            let mut nodes_by_position: HashMap<(&str, u32, u32), &llg::ffi::surelog::VObjectInfo> =
                HashMap::with_capacity(tokens.iter().map(|ft| ft.nodes.len()).sum());
            for file_tokens in tokens_by_file.values() {
                for node in &file_tokens.nodes {
                    nodes_by_position
                        .entry((file_tokens.path.as_str(), node.line, node.col))
                        .or_insert(node);
                }
            }
            nodes_by_position
        });

        // ── Parse-fallback declaration seeds ────────────────────────────────
        // Seed the name sets from the collector's recorded declaration
        // positions so `classify_token` accepts them despite the absent
        // instance data.
        if let Some(decl_positions) = parse_decls {
            for (file, line1, col1) in decl_positions {
                let Some(node) = parse_nodes_by_position
                    .as_ref()
                    .and_then(|nodes| nodes.get(&(file.as_str(), *line1, *col1)))
                else {
                    continue;
                };
                let Some(name) = node.name.as_deref() else {
                    continue;
                };
                let nm = name.to_owned();
                match node.vpi_type {
                    vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT => {
                        port_names.insert(nm.clone());
                        signal_names.insert(nm);
                    }
                    vpi::vpiParameter => {
                        param_names.insert(nm.clone());
                        signal_names.insert(nm);
                    }
                    _ => {
                        signal_names.insert(nm);
                    }
                }
            }
        }

        // ── Module declarations (model; position refined by token) ──────────
        for m in &model.modules {
            let Some(file) = m.file.clone() else { continue };
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::vpiModule
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&m.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((m.line, m.col));
            let name = clean_name(&m.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Module,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format!("module {name}")),
            });
            claimed.insert((file, line1, col1));
        }

        // ── Package declarations ────────────────────────────────────────────
        for p in &model.packages {
            let Some(file) = p.file.clone() else { continue };
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::uhdmpackage
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&p.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((p.line, p.col));
            let name = clean_name(&p.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Package,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format!("package {name}")),
            });
            claimed.insert((file, line1, col1));
        }

        // ── Package item declarations (model) ───────────────────────────────
        // Parameters and enum constants of every package, positioned from the
        // model (enum consts) or from the declaration token in the package
        // file (params — `ParamModel` carries no position).  These win over
        // the token pass via `claimed`, so their hover detail (type/value)
        // comes from the model.
        for p in &model.packages {
            let Some(file) = p.file.clone() else { continue };
            let pkg_name = clean_name(&p.name).to_owned();
            let ft = tokens_by_file.get(file.as_str());
            for param in &p.params {
                // `ParamModel` carries no position; the package-file token
                // (vpiParameter at the declaration site) supplies it.  Params
                // without a token are skipped rather than mis-positioned.
                let Some((line1, col1)) = ft
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            n.vpi_type == vpi::vpiParameter
                                && clean_name(n.name.as_deref().unwrap_or("")) == param.name
                        })
                    })
                    .map(|n| (n.line, n.col))
                else {
                    continue;
                };
                let name = clean_name(&param.name).to_owned();
                let len = lsp_name_len(&name);
                let line = line1.saturating_sub(1);
                let col = col1.saturating_sub(1);
                decls.push(SymEntry {
                    name,
                    kind: SymKind::Param,
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(pkg_name.clone()),
                    detail: Some(format_param(param)),
                });
                claimed.insert((file.clone(), line1, col1));
            }
            for ec in &p.enum_consts {
                let name = clean_name(&ec.name).to_owned();
                let len = lsp_name_len(&name);
                let line = ec.line.saturating_sub(1);
                let col = ec.col.saturating_sub(1);
                decls.push(SymEntry {
                    name,
                    kind: SymKind::EnumConst,
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(pkg_name.clone()),
                    detail: Some(format_enum_const(ec)),
                });
                claimed.insert((file.clone(), ec.line, ec.col));
            }
        }

        // ── Class declarations and class members (model) ───────────────────
        // Classes are per-file definitions (not per-instance clones).  The
        // class decl position is refined to the class *name* token (Surelog's
        // own position points at the `class` keyword); methods and fields are
        // indexed with the class name as their scope so `Class::member`
        // resolution and class-scoped completion work.  Surelog's builtin
        // classes (mailbox/process/semaphore) report a virtual `<cwd>/builtin.sv`
        // file that never exists on disk, so they are skipped the same way the
        // builtin package is (no user code to navigate to).
        for c in &model.classes {
            let Some(file) = c.file.clone() else { continue };
            if builtin_file(&file) {
                continue;
            }
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::uhdmclass_defn
                            && n.line == c.line
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&c.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((c.line, c.col));
            let name = clean_name(&c.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Class,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format_class(c)),
            });
            claimed.insert((file.clone(), line1, col1));

            let scope = Some(name.clone());
            for m in &c.methods {
                let Some(mfile) = m.file.clone() else {
                    continue;
                };
                let mname = clean_name(&m.name).to_owned();
                let mlen = lsp_name_len(&mname);
                let mline = m.line.saturating_sub(1);
                let mcol = m.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: mname.clone(),
                    kind: if m.is_task {
                        SymKind::Task
                    } else {
                        SymKind::Function
                    },
                    file: mfile.clone(),
                    line: mline,
                    col: mcol,
                    end_line: mline,
                    end_col: mcol.saturating_add(mlen),
                    is_decl: true,
                    scope: scope.clone(),
                    detail: Some(func_signature(m)),
                });
                claimed.insert((mfile.clone(), m.line, m.col));
                // Surelog's own method position points at the `function`/
                // `task` keyword; the parse-tree name token sits on the same
                // line.  Claim it too so the token pass does not emit a
                // second, detail-less method decl at the name.
                if let Some((l1, c1)) = tokens_by_file
                    .get(file.as_str())
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            (n.vpi_type == vpi::vpiFunction || n.vpi_type == vpi::vpiTask)
                                && n.line == m.line
                                && clean_name(n.name.as_deref().unwrap_or("")) == mname
                        })
                    })
                    .map(|n| (n.line, n.col))
                {
                    claimed.insert((file.clone(), l1, c1));
                }
            }
            // Class fields reuse `SymKind::Var` (no dedicated Field kind).
            for f in &c.fields {
                let fname = clean_name(&f.name).to_owned();
                let flen = lsp_name_len(&fname);
                let fline = f.line.saturating_sub(1);
                let fcol = f.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: fname.clone(),
                    kind: SymKind::Var,
                    file: file.clone(),
                    line: fline,
                    col: fcol,
                    end_line: fline,
                    end_col: fcol.saturating_add(flen),
                    is_decl: true,
                    scope: scope.clone(),
                    detail: Some(format_class_field(f)),
                });
                claimed.insert((file.clone(), f.line, f.col));
            }
        }

        // ── Instance declarations (model, at the instantiation site) ────────
        // Surelog's `vpiColumnNo` on an instance points at the module *type*
        // name; the position is refined to the instance-name token (when
        // present) so `entry_at` hits the identifier the user actually clicks.
        fn push_instances(
            insts: &[InstanceModel],
            parent_scope: Option<String>,
            tokens_by_file: &HashMap<&str, &FileTokens>,
            decls: &mut Vec<SymEntry>,
            claimed: &mut HashSet<(String, u32, u32)>,
            instance_def: &mut HashMap<usize, String>,
        ) {
            for i in insts {
                let scope = parent_scope.clone();
                if let Some(file) = i.file.clone() {
                    let name = clean_name(&i.name).to_owned();
                    let (line1, col1) = tokens_by_file
                        .get(file.as_str())
                        .and_then(|ft| {
                            ft.nodes.iter().find(|n| {
                                n.line == i.line
                                    && clean_name(n.name.as_deref().unwrap_or("")) == name
                                    && INSTANCE_NAME_TOKEN_TYPES.contains(&n.vpi_type)
                            })
                        })
                        .map(|n| (n.line, n.col))
                        .unwrap_or((i.line, i.col));
                    let len = lsp_name_len(&name);
                    let line = line1.saturating_sub(1);
                    let col = col1.saturating_sub(1);
                    let def = clean_name(&i.def_name).to_owned();
                    let detail = format!("{} {}\n\ndefined at {file}:{line1}", def, i.name);
                    let idx = decls.len();
                    decls.push(SymEntry {
                        name: name.clone(),
                        kind: SymKind::Instance,
                        file: file.clone(),
                        line,
                        col,
                        end_line: line,
                        end_col: col.saturating_add(len),
                        is_decl: true,
                        scope,
                        detail: Some(detail),
                    });
                    instance_def.insert(idx, def);
                    claimed.insert((file, line1, col1));
                }
                push_instances(
                    &i.children,
                    Some(i.full_name.clone()),
                    tokens_by_file,
                    decls,
                    claimed,
                    instance_def,
                );
            }
        }
        push_instances(
            &model.top_instances,
            None,
            &tokens_by_file,
            &mut decls,
            &mut claimed,
            &mut instance_def,
        );

        // ── Function/task declarations (model, per-instance clones) ─────────
        // Every instance carries its elaborated function/task clones with the
        // definition file/position; these win over the token-based entries
        // (which remain a fallback for clones the model did not capture).
        for inst in all_instances(&model.top_instances) {
            for f in &inst.funcs {
                let Some(file) = f.file.clone() else { continue };
                let name = clean_name(&f.name).to_owned();
                let len = lsp_name_len(&name);
                let line = f.line.saturating_sub(1);
                let col = f.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: name.clone(),
                    kind: if f.is_task {
                        SymKind::Task
                    } else {
                        SymKind::Function
                    },
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(f.scope.clone()),
                    detail: Some(func_signature(f)),
                });
                claimed.insert((file, f.line, f.col));
            }
        }

        // Parse-backed enum declarations supplement the model.  In a valid
        // design package enum constants already arrived through UHDM and were
        // inserted above, so this pass runs afterward and keeps the richer
        // model entry at duplicate positions while retaining class-local and
        // syntax-broken declarations absent from UHDM.
        for parsed in parse_enum_decls {
            let key = (
                parsed.file.clone(),
                parsed.line1.saturating_sub(1),
                parsed.col1.saturating_sub(1),
            );
            if decls
                .iter()
                .any(|d| d.file == key.0 && d.line == key.1 && d.col == key.2)
            {
                continue;
            }
            decls.push(SymEntry {
                name: parsed.name.clone(),
                kind: SymKind::EnumConst,
                file: parsed.file.clone(),
                line: key.1,
                col: key.2,
                end_line: key.1,
                end_col: key.2.saturating_add(lsp_name_len(&parsed.name)),
                is_decl: true,
                scope: parsed.scope.clone(),
                detail: Some(format!("enum constant {}", parsed.name)),
            });
        }

        // ── Scope providers (module/package/class/... decls) for ref scopes ──
        let scope_providers: Vec<SymEntry> = decls
            .iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    SymKind::Module
                        | SymKind::Interface
                        | SymKind::Package
                        | SymKind::Class
                        | SymKind::Program
                )
            })
            .cloned()
            .collect();

        fn enclosing_scope(providers: &[SymEntry], file: &str, line1: u32) -> Option<String> {
            let line0 = line1.saturating_sub(1);
            providers
                .iter()
                .filter(|d| d.file == file && d.line <= line0)
                .max_by_key(|d| d.line)
                .map(|d| d.name.clone())
        }

        // ── Token pass: ports/signals/params decls and references ───────────
        for ft in tokens {
            // Type histogram per position drives the decl/ref classification.
            let mut pos_types: HashMap<(u32, u32), Vec<i32>> = HashMap::new();
            for n in &ft.nodes {
                if n.name.is_some() {
                    pos_types
                        .entry((n.line, n.col))
                        .or_default()
                        .push(n.vpi_type);
                }
            }

            for n in &ft.nodes {
                let Some(name) = n.name.as_deref() else {
                    continue;
                };
                if name.is_empty() || claimed.contains(&(ft.path.clone(), n.line, n.col)) {
                    continue;
                }
                let forced_decl =
                    parse_decls.is_some_and(|set| set.contains(&(ft.path.clone(), n.line, n.col)));
                let Some((kind, is_decl)) = classify_token(
                    n.vpi_type,
                    name,
                    &pos_types,
                    n.line,
                    n.col,
                    &signal_names,
                    &port_names,
                    &param_names,
                    &module_names,
                    forced_decl,
                    parse_enum_ref_positions.contains(&(
                        ft.path.clone(),
                        n.line.saturating_sub(1),
                        n.col.saturating_sub(1),
                    )),
                ) else {
                    continue;
                };
                // Named port connections: a `vpiFunction`/`vpiTask` reference
                // with an instance declaration on the same or an earlier line
                // (see `port_label_candidate` for the exact heuristic), or a
                // classifier-labeled `TOKEN_PORT_CONN_LABEL` token — the same
                // structural evidence, needing no name gate.
                if !is_decl
                    && kind == SymKind::Var
                    && (n.vpi_type == vpi::vpiFunction
                        || n.vpi_type == vpi::vpiTask
                        || n.vpi_type == vpi::TOKEN_PORT_CONN_LABEL)
                {
                    if let Some(cand) = port_label_candidate(
                        &decls,
                        &instance_def,
                        &ft.path,
                        n.line.saturating_sub(1),
                        n.col.saturating_sub(1),
                        name,
                    ) {
                        port_label_candidates.push(cand);
                    }
                }
                let len = lsp_name_len(name);
                let line = n.line.saturating_sub(1);
                let col = n.col.saturating_sub(1);
                let scope = enclosing_scope(&scope_providers, &ft.path, n.line);
                let detail = if is_decl {
                    match kind {
                        SymKind::Port | SymKind::Net | SymKind::Var | SymKind::Param => {
                            decl_detail(model, kind, name)
                        }
                        _ => Some(format!("{} {name}", kind_label(kind))),
                    }
                } else {
                    None
                };
                let entry = SymEntry {
                    name: name.to_owned(),
                    kind,
                    file: ft.path.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl,
                    scope,
                    detail,
                };
                if is_decl {
                    decls.push(entry);
                } else {
                    refs.push(entry);
                }
            }
        }

        // ── Deduplicate by position (keep the first / richest entry) ────────
        let mut seen: HashSet<(String, u32, u32)> = HashSet::new();
        decls.retain(|e| seen.insert((e.file.clone(), e.line, e.col)));
        seen.clear();
        refs.retain(|e| seen.insert((e.file.clone(), e.line, e.col)));

        // ── Named port connections → child port declarations ───────────────
        // Resolved after dedup so `decls` indices are final; declarations
        // synthesized for missing port entries are appended here, before the
        // lookup maps are built.
        let ref_positions: HashSet<(String, u32, u32)> = refs
            .iter()
            .map(|r| (r.file.clone(), r.line, r.col))
            .collect();
        let mut port_labels: HashMap<(String, u32, u32), usize> = HashMap::new();
        for cand in &port_label_candidates {
            let Some(((pos_file, pos_line, pos_col), idx)) =
                resolve_port_label(cand, model, &mut decls)
            else {
                continue; // not a genuine named connection (e.g. function call)
            };
            if ref_positions.contains(&(pos_file.clone(), pos_line, pos_col)) {
                port_labels.insert((pos_file, pos_line, pos_col), idx);
            }
        }

        // ── Named parameter overrides → child parameter declarations ───────
        // Resolved straight from the scanned pairs (exact parse-tree
        // evidence; no positional heuristic — in valid SV the override list
        // `child #(.W(4)) u0 (...)` PRECEDES the instance name, so the port
        // label's same-line rule cannot apply).  Every scanned override
        // label position that did NOT resolve is remembered so definition
        // requests there yield no result instead of a wrong same-name jump.
        let mut param_labels: HashMap<(String, u32, u32), usize> = HashMap::new();
        for pair in pairs.iter().filter(|p| p.kind == ConnKind::Param) {
            if let Some(((pos_file, pos_line, pos_col), idx)) =
                resolve_param_override(pair, model, &mut decls)
            {
                if ref_positions.contains(&(pos_file.clone(), pos_line, pos_col)) {
                    param_labels.insert((pos_file, pos_line, pos_col), idx);
                }
            }
        }
        let unresolved_param_labels: HashSet<(String, u32, u32)> = param_pair_positions
            .iter()
            .filter(|pos| !param_labels.contains_key(pos))
            .cloned()
            .collect();
        let unresolved_enum_refs = unresolved_enum_refs.clone();

        // ── Lookup maps ──────────────────────────────────────────────────────
        let mut decls_by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, d) in decls.iter().enumerate() {
            decls_by_name.entry(d.name.clone()).or_default().push(i);
        }
        let mut decls_by_file: HashMap<String, Vec<SymEntry>> = HashMap::new();
        for d in &decls {
            decls_by_file
                .entry(d.file.clone())
                .or_default()
                .push(d.clone());
        }
        let mut refs_by_file: HashMap<String, Vec<SymEntry>> = HashMap::new();
        for r in &refs {
            refs_by_file
                .entry(r.file.clone())
                .or_default()
                .push(r.clone());
        }
        for v in decls_by_file.values_mut() {
            v.sort_by_key(|e| (e.line, e.col));
        }
        for v in refs_by_file.values_mut() {
            v.sort_by_key(|e| (e.line, e.col));
        }

        SymbolIndex {
            decls,
            refs,
            decls_by_file,
            refs_by_file,
            decls_by_name,
            instance_def,
            port_labels,
            param_labels,
            unresolved_param_labels,
            unresolved_enum_refs,
        }
    }

    /// The symbol entry whose range covers the 0-based `(line, col)` position
    /// in `file` (declarations and references merged; the longest name wins so
    /// nested identifiers like `pkg::item` match their full spelling).
    pub fn entry_at(&self, file: &str, line: u32, col: u32) -> Option<&SymEntry> {
        // Parse-backed qualified enum references can coexist with Surelog's
        // folded `pkg::member` token.  An exact token start is the most
        // precise cursor anchor, so prefer it before the broader spelling's
        // containing range.
        if let Some(exact) = self
            .decls_in_file(file)
            .iter()
            .chain(self.refs_in_file(file).iter())
            .find(|entry| entry.line == line && entry.col == col)
        {
            return Some(exact);
        }
        let mut best: Option<&SymEntry> = None;
        let mut best_len: usize = 0;
        for e in self
            .decls_in_file(file)
            .iter()
            .chain(self.refs_in_file(file).iter())
        {
            if e.line != line {
                continue;
            }
            let len = lsp_name_len(&e.name) as usize;
            if col >= e.col && col < e.col.saturating_add(len as u32) && len > best_len {
                best = Some(e);
                best_len = len;
            }
        }
        best
    }

    /// Declarations in `file`, sorted by (line, col).
    pub fn decls_in_file(&self, file: &str) -> &[SymEntry] {
        self.decls_by_file.get(file).map_or(&[], |v| v.as_slice())
    }

    /// References in `file`, sorted by (line, col).
    fn refs_in_file(&self, file: &str) -> &[SymEntry] {
        self.refs_by_file.get(file).map_or(&[], |v| v.as_slice())
    }

    /// Resolve `e` to the declaration(s) it refers to.
    ///
    /// Rules (v1, documented):
    /// 1. A named connection label resolves to the CHILD module's
    ///    declaration (precomputed in [`SymbolIndex::from_parts`], cross-file
    ///    included): a port label (`.clk` in `m u0(.clk(c))`) to the port, a
    ///    parameter override label (`W` in `m u0 #(.W(4)) (...)`) to the
    ///    parameter.
    /// 2. An `Instance` declaration resolves to the module definition named by
    ///    its `def_name` (stored during the build).
    /// 3. Package-qualified names (`pkg::item` in either spelling — the token
    ///    name may be the full `pkg::item` or just `item` when the reference
    ///    sits inside package scope) resolve to the named package's item
    ///    declarations (parameters and enum constants by name, cross-file);
    ///    `pkg` alone resolves to the package declaration.
    ///    Parse-backed enum bindings take precedence at the exact member
    ///    coordinate, including class scopes and imported bare members.
    /// 4. A reference resolves to: the same-named declaration in the same
    ///    scope, else the same-named declaration in the same file nearest by
    ///    line, else any same-named declaration in the workspace.  Module-type
    ///    references apply that order only to module declarations, so an
    ///    instance with the same name cannot capture the type reference.
    ///    Scanned parameter-override labels that failed rule 1 are excluded:
    ///    their namespace is the INSTANTIATED module, so a same-name
    ///    declaration of the instantiating scope or an arbitrary workspace
    ///    match is wrong by construction — they resolve to nothing instead.
    /// 5. Any other declaration resolves to itself.
    pub fn resolve<'a>(&'a self, e: &'a SymEntry) -> Vec<&'a SymEntry> {
        // Named connection labels resolve to the child module's port /
        // parameter declaration, computed at index-build time.
        if !e.is_decl {
            let key = (e.file.clone(), e.line, e.col);
            if let Some(&idx) = self.port_labels.get(&key) {
                if let Some(d) = self.decls.get(idx) {
                    return vec![d];
                }
            }
            if let Some(&idx) = self.param_labels.get(&key) {
                if let Some(d) = self.decls.get(idx) {
                    return vec![d];
                }
            }
            if self.unresolved_param_labels.contains(&key) {
                return Vec::new();
            }
            if self.unresolved_enum_refs.contains(&key) {
                return Vec::new();
            }
        }
        if let Some((pkg, item)) = e.name.split_once("::") {
            let mut out: Vec<&SymEntry> = Vec::new();
            // Item declarations scoped to the named package or class.
            // `clean_name` handles both the bare spelling and a `work@my_pkg`
            // prefix.  Class members carry the (library-stripped) class name
            // as their scope, so `Counter::get` resolves the same way
            // `my_pkg::P` does.
            if let Some(indices) = self.decls_by_name.get(item) {
                out.extend(indices.iter().filter_map(|&i| {
                    let d = &self.decls[i];
                    let in_scope = d
                        .scope
                        .as_deref()
                        .map(|s| clean_name(s) == clean_name(pkg))
                        .unwrap_or(false);
                    (in_scope
                        && matches!(
                            d.kind,
                            SymKind::Param
                                | SymKind::EnumConst
                                | SymKind::Typedef
                                | SymKind::Function
                                | SymKind::Task
                                | SymKind::Var
                        ))
                    .then_some(d)
                }));
            }
            if !out.is_empty() {
                return out;
            }
            // Fallback: the package/class declaration plus any same-named
            // declarations workspace-wide (unchanged v1 behavior).
            if let Some(indices) = self.decls_by_name.get(pkg) {
                out.extend(indices.iter().filter_map(|&i| {
                    matches!(self.decls[i].kind, SymKind::Package | SymKind::Class)
                        .then_some(&self.decls[i])
                }));
            }
            if let Some(indices) = self.decls_by_name.get(item) {
                out.extend(indices.iter().map(|&i| &self.decls[i]));
            }
            return out;
        }
        if e.is_decl {
            if e.kind == SymKind::Instance {
                if let Some(def) = self.instance_def_of(e) {
                    let mut out: Vec<&SymEntry> = Vec::new();
                    if let Some(indices) = self.decls_by_name.get(&def) {
                        out.extend(indices.iter().filter_map(|&i| {
                            let d = &self.decls[i];
                            matches!(
                                d.kind,
                                SymKind::Module
                                    | SymKind::Interface
                                    | SymKind::Package
                                    | SymKind::Class
                                    | SymKind::Program
                            )
                            .then_some(d)
                        }));
                    }
                    if !out.is_empty() {
                        return out;
                    }
                }
            }
            return vec![e];
        }
        // Reference: in-scope declaration first, then same-file, then
        // workspace-wide.  Module-type references occupy the module namespace;
        // ordinary identifier references retain the historical name-only
        // behavior.
        let module_type_reference = e.kind == SymKind::Module;
        if let Some(indices) = self.decls_by_name.get(&e.name) {
            let in_scope: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| {
                    self.decls[i].scope == e.scope
                        && (!module_type_reference || self.decls[i].kind == SymKind::Module)
                })
                .map(|&i| &self.decls[i])
                .collect();
            if !in_scope.is_empty() {
                return in_scope;
            }
            let mut same_file: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| {
                    self.decls[i].file == e.file
                        && (!module_type_reference || self.decls[i].kind == SymKind::Module)
                })
                .map(|&i| &self.decls[i])
                .collect();
            same_file.sort_by_key(|d| d.line.abs_diff(e.line));
            if !same_file.is_empty() {
                return same_file;
            }
            let all: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| !module_type_reference || self.decls[i].kind == SymKind::Module)
                .map(|&i| &self.decls[i])
                .collect();
            if !all.is_empty() {
                return all;
            }
        }
        Vec::new()
    }

    /// The declaration at the head of `e` plus every reference resolving to
    /// the same declaration, deduplicated by position.
    ///
    /// When resolution fails (e.g. an unknown name), all same-named
    /// declarations are used as the resolution head.
    pub fn all_references(&self, e: &SymEntry) -> Vec<SymEntry> {
        let mut resolved: Vec<&SymEntry> = self.resolve(e);
        let unresolved_enum = !e.is_decl
            && self
                .unresolved_enum_refs
                .contains(&(e.file.clone(), e.line, e.col));
        if resolved.is_empty() && !unresolved_enum {
            if let Some(indices) = self.decls_by_name.get(&e.name) {
                resolved = indices.iter().map(|&i| &self.decls[i]).collect();
            }
        }
        let resolved_set: HashSet<(String, u32, u32)> = resolved
            .iter()
            .map(|d| (d.file.clone(), d.line, d.col))
            .collect();
        let mut out: Vec<SymEntry> = Vec::new();
        let mut seen: HashSet<(String, u32, u32)> = HashSet::new();
        for d in &resolved {
            if seen.insert((d.file.clone(), d.line, d.col)) {
                out.push((*d).clone());
            }
        }
        for r in &self.refs {
            let hits = self
                .resolve(r)
                .iter()
                .any(|d| resolved_set.contains(&(d.file.clone(), d.line, d.col)));
            if hits && seen.insert((r.file.clone(), r.line, r.col)) {
                out.push(r.clone());
            }
        }
        out
    }

    /// Whether `(file, line, col)` is a scanned named PARAMETER override
    /// label position that did not resolve to a child-module declaration.
    ///
    /// Definition serving uses this to return NO result for such labels: their
    /// namespace is the instantiated module, so every name-based fallback
    /// (same scope / same file / workspace) would be wrong by construction.
    pub fn is_unresolved_param_label(&self, file: &str, line: u32, col: u32) -> bool {
        self.unresolved_param_labels
            .contains(&(file.to_owned(), line, col))
    }

    /// Whether a parse-backed enum reference was intentionally left
    /// unresolved because its visible declarations were ambiguous.
    pub fn is_unresolved_enum_ref(&self, file: &str, line: u32, col: u32) -> bool {
        self.unresolved_enum_refs
            .contains(&(file.to_owned(), line, col))
    }

    /// The decl index of `e`, used to look up stored per-decl data.
    fn instance_def_of(&self, e: &SymEntry) -> Option<String> {
        let idx = self.decls.iter().position(|d| d == e)?;
        self.instance_def.get(&idx).cloned()
    }
}

/// A detected named port connection (`.clk` in `m u0(.clk(c))`) awaiting
/// resolution to the child module's port declaration.
struct PortLabelCandidate {
    /// 0-based position of the label identifier (the `.clk` text).
    file: String,
    line: u32,
    col: u32,
    /// The label text == the child port name.
    name: String,
    /// The associated instance name (e.g. `u0`).
    inst_name: String,
    /// The instance's module definition name (e.g. `m`).
    def_name: String,
    /// 0-based line of the associated instance declaration: the same line for
    /// same-line labels, an earlier line for continuation-line labels.
    inst_line: u32,
}

/// Detect a named connection label (port or parameter override) at an
/// indexed reference site.
///
/// The index has no source text, so the check is positional (verified against
/// the hand-built fixtures in the test module):
///
/// 1. the token at the position was classified as a *reference* (not a decl)
///    of kind `Var` by the `vpiFunction`/`vpiTask` heuristic — the existing
///    signal-name test already excludes real function/task declarations;
/// 2. **same-line rule** (v1, unchanged): the same line holds an `Instance`
///    declaration at an earlier column (the instance name); the label column
///    must be at least `instance name length + 2` past it, i.e. room for the
///    `(` and `.` that separate `<name>` from the label — this also rejects
///    hierarchical references like `u0.clk` (gap 1) and
///    first-connection-less positions; the rightmost such instance wins, so
///    several instantiations on one line attribute each label to its own
///    instance;
/// 3. **continuation-line rule** (multi-line instantiations like
///    `m u0(\n  .clk(c),\n  .o(o)\n);`): when no same-line instance exists,
///    the label is associated with the nearest preceding `Instance`
///    declaration in the same file (highest line, then rightmost column)
///    provided that:
///    - no other `Instance`, `Module`, or `Package` declaration lies strictly
///      between the instance line and the label line (the label cannot belong
///      to a different scope or to a later instantiation),
///    - the label line is within [`PORT_LABEL_MAX_SPAN`] lines of the
///      instance line,
///    - the label column is > 0, i.e. the connection is indented (a
///      continuation-line connection is never flush against the left margin).
///
/// [`resolve_port_label`] then verifies the name is an actual port of that
/// instance, which rejects function calls and other false positives.
fn port_label_candidate(
    decls: &[SymEntry],
    instance_def: &HashMap<usize, String>,
    file: &str,
    line: u32,
    col: u32,
    name: &str,
) -> Option<PortLabelCandidate> {
    // ── Same-line rule ─────────────────────────────────────────────────────
    if let Some((idx, inst)) = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.kind == SymKind::Instance && d.file == file && d.line == line && d.col < col
        })
        .max_by_key(|(_, d)| d.col)
    {
        let min_label_col = inst.col.saturating_add(lsp_name_len(&inst.name) + 2);
        if col >= min_label_col {
            let def_name = instance_def.get(&idx)?.clone();
            return Some(PortLabelCandidate {
                file: file.to_owned(),
                line,
                col,
                name: name.to_owned(),
                inst_name: inst.name.clone(),
                def_name,
                inst_line: line,
            });
        }
    }

    // ── Continuation-line rule ─────────────────────────────────────────────
    if col == 0 || line == 0 {
        return None;
    }
    let (idx, inst) = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == SymKind::Instance && d.file == file && d.line < line)
        .max_by_key(|(_, d)| (d.line, d.col))?;
    if line - inst.line > PORT_LABEL_MAX_SPAN {
        return None;
    }
    let scope_blocked = decls.iter().any(|d| {
        d.file == file
            && d.line > inst.line
            && d.line < line
            && matches!(
                d.kind,
                SymKind::Instance | SymKind::Module | SymKind::Package
            )
    });
    if scope_blocked {
        return None;
    }
    let def_name = instance_def.get(&idx)?.clone();
    Some(PortLabelCandidate {
        file: file.to_owned(),
        line,
        col,
        name: name.to_owned(),
        inst_name: inst.name.clone(),
        def_name,
        inst_line: inst.line,
    })
}

/// Resolve a detected named port connection to the child module's port
/// declaration.
///
/// Returns `(label position, decl index)` when the label is a genuine named
/// connection; `None` when the name is not a port of the instance (a function
/// call or a false positive) or the instance's module definition cannot be
/// found — the caller then falls back to ordinary name-based resolution.
///
/// The port declaration is looked up by (name, def file, kind `Port`),
/// preferring the entry whose scope is the instance's module definition.  When
/// the index has no such declaration (hand-built models), one is synthesized
/// anchored at the module header, offset by the port index so multiple missing
/// ports of the same module keep unique positions.
fn resolve_port_label(
    cand: &PortLabelCandidate,
    model: &DesignModel,
    decls: &mut Vec<SymEntry>,
) -> Option<((String, u32, u32), usize)> {
    let inst = find_instance(
        &model.top_instances,
        &cand.inst_name,
        &cand.file,
        cand.inst_line,
    )?;
    let port = inst.ports.iter().find(|p| p.name == cand.name)?;
    let module = model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == clean_name(&cand.def_name))?;
    let def_file = module.file.as_ref()?;
    let scope = clean_name(&cand.def_name).to_owned();
    let key = (cand.file.clone(), cand.line, cand.col);

    let in_def_file: Vec<(usize, &SymEntry)> = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.kind == SymKind::Port && d.name == cand.name && d.file == def_file.as_str()
        })
        .collect();
    // Prefer the port declaration of the instance's module definition.
    if let Some(&(idx, _)) = in_def_file
        .iter()
        .find(|(_, d)| d.scope.as_deref() == Some(scope.as_str()))
    {
        return Some((key, idx));
    }
    // A single unscoped port declaration of the name in the def file.
    if in_def_file.len() == 1 && in_def_file[0].1.scope.is_none() {
        return Some((key, in_def_file[0].0));
    }
    // No indexed declaration: synthesize one (reusing an existing entry at
    // the anchor position, e.g. from a previous label of the same port).
    let module_name_len = lsp_name_len(clean_name(&module.name));
    let port_idx = inst
        .ports
        .iter()
        .position(|p| p.name == cand.name)
        .unwrap_or(0) as u32;
    let line0 = module.line.saturating_sub(1);
    let col0 = module.col.saturating_sub(1) + module_name_len + port_idx;
    if let Some((idx, _)) = decls
        .iter()
        .enumerate()
        .find(|(_, d)| d.file == def_file.as_str() && d.line == line0 && d.col == col0)
    {
        return Some((key, idx));
    }
    let len = lsp_name_len(&cand.name);
    let idx = decls.len();
    decls.push(SymEntry {
        name: cand.name.clone(),
        kind: SymKind::Port,
        file: def_file.clone(),
        line: line0,
        col: col0,
        end_line: line0,
        end_col: col0 + len,
        is_decl: true,
        scope: Some(scope),
        detail: Some(format_port(port)),
    });
    Some((key, idx))
}

/// Column stride that keeps synthesized PARAMETER anchors disjoint from the
/// port anchors [`resolve_port_label`] places at the same module header.
///
/// Synthesized declarations are virtual positions (the index had no real
/// declaration for the name); without the stride a child module's i-th
/// missing parameter and i-th missing port would synthesize onto the SAME
/// header position, and whichever resolved second would silently reuse the
/// first one's entry — pointing parameter labels at port-kind declarations.
const PARAM_SYNTH_COL_STRIDE: u32 = 64;

/// Resolve a scanned named PARAMETER override to the child module's
/// parameter declaration.
///
/// Unlike the port path (which guesses the owning instance positionally),
/// the pair itself carries the instantiation's module TYPE (`inst_type`),
/// which IS the definition namespace:
///
/// 1. any elaborated clone of that type supplies the parameter list —
///    preferring a clone in the instantiating file; the pair's label name
///    must be one of its parameters;
/// 2. the declaration is looked up by `(name, def file, kind Param)`,
///    preferring the entry whose scope is the module definition; a single
///    unscoped match in the def file is accepted;
/// 3. otherwise a declaration is synthesized anchored at the module header
///    (offset by [`PARAM_SYNTH_COL_STRIDE`] plus the parameter index so
///    anchors never collide with synthesized ports).
///
/// Returns `None` when the override cannot be tied to a parameter of the
/// instantiated type (unknown type, unknown parameter, or a definition
/// without a file) — the caller records the position as unresolved.
fn resolve_param_override(
    pair: &NamedPortConn,
    model: &DesignModel,
    decls: &mut Vec<SymEntry>,
) -> Option<((String, u32, u32), usize)> {
    let def_name = clean_name(pair.inst_type.as_deref()?).to_owned();
    let key = (
        pair.file.clone(),
        pair.label.0.saturating_sub(1),
        pair.label.1.saturating_sub(1),
    );
    let name = pair.label_name.as_str();
    // Any elaborated clone of the instantiated type carries the same
    // parameter list (declaration order included).
    let inst = all_instances(&model.top_instances)
        .into_iter()
        .filter(|i| clean_name(&i.def_name) == def_name)
        .min_by_key(|i| i.file.as_deref() != Some(pair.file.as_str()))?;
    let param = inst.params.iter().find(|p| p.name == name)?;
    let module = model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == def_name)?;
    let def_file = module.file.as_ref()?;
    let scope = def_name;
    let in_def_file: Vec<(usize, &SymEntry)> = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == SymKind::Param && d.name == name && d.file == def_file.as_str())
        .collect();
    // Prefer the parameter declaration of the instance's module definition.
    if let Some(&(idx, _)) = in_def_file
        .iter()
        .find(|(_, d)| d.scope.as_deref() == Some(scope.as_str()))
    {
        return Some((key, idx));
    }
    // A single unscoped parameter declaration of the name in the def file.
    if in_def_file.len() == 1 && in_def_file[0].1.scope.is_none() {
        return Some((key, in_def_file[0].0));
    }
    // No indexed declaration: synthesize one (reusing an existing entry at
    // the anchor position, e.g. from a previous label of the same parameter).
    let module_name_len = lsp_name_len(&scope);
    let param_idx = inst.params.iter().position(|p| p.name == name).unwrap_or(0) as u32;
    let line0 = module.line.saturating_sub(1);
    let col0 = module.col.saturating_sub(1) + module_name_len + PARAM_SYNTH_COL_STRIDE + param_idx;
    if let Some((idx, _)) = decls
        .iter()
        .enumerate()
        .find(|(_, d)| d.file == def_file.as_str() && d.line == line0 && d.col == col0)
    {
        return Some((key, idx));
    }
    let len = lsp_name_len(name);
    let idx = decls.len();
    decls.push(SymEntry {
        name: name.to_owned(),
        kind: SymKind::Param,
        file: def_file.clone(),
        line: line0,
        col: col0,
        end_line: line0,
        end_col: col0 + len,
        is_decl: true,
        scope: Some(scope),
        detail: Some(format_param(param)),
    });
    Some((key, idx))
}

/// The instance of `name` in `file`, walking `top_instances` recursively.
///
/// When several instances share the name (different scopes), the one whose
/// instantiation line matches `line` wins.  For continuation-line labels the
/// caller passes the associated instance declaration's line (see
/// [`PortLabelCandidate::inst_line`]), so the exact-line preference still
/// applies; without a line match the first same-name/same-file instance is
/// returned as a fallback.
fn find_instance<'m>(
    insts: &'m [InstanceModel],
    name: &str,
    file: &str,
    line: u32,
) -> Option<&'m InstanceModel> {
    let mut matches: Vec<&InstanceModel> = all_instances(insts)
        .into_iter()
        .filter(|i| clean_name(&i.name) == name && i.file.as_deref() == Some(file))
        .collect();
    if let Some(pos) = matches
        .iter()
        .position(|i| i.line.saturating_sub(1) == line)
    {
        return Some(matches.remove(pos));
    }
    matches.into_iter().next()
}

/// Classify a token node as (kind, is_decl); `None` when it is not an indexed
/// symbol site.  See [`SymbolIndex::from_parts`] for the empirical basis.
#[allow(clippy::too_many_arguments)]
fn classify_token(
    t: i32,
    name: &str,
    pos_types: &HashMap<(u32, u32), Vec<i32>>,
    line: u32,
    col: u32,
    signal_names: &HashSet<String>,
    port_names: &HashSet<String>,
    param_names: &HashSet<String>,
    module_names: &HashSet<String>,
    forced_decl: bool,
    forced_enum_ref: bool,
) -> Option<(SymKind, bool)> {
    use llg::ffi::vpi;

    // Pure reference types (expression operands walked by the VPI walker).
    if REF_TOKEN_TYPES.contains(&t) {
        return Some((SymKind::Var, false));
    }
    // Named-connection labels are reference sites by construction: the
    // classifier emits the dedicated synthetic types ONLY under
    // `paNamed_port_connection` / `paNamed_parameter_assignment`, and their
    // resolution to the child module's declaration happens through
    // `port_labels`/`param_labels`.  Classifying them as references
    // unconditionally keeps an unresolvable label indexed (cursor
    // normalization, dump visibility) without ever surfacing it as a phantom
    // function/task/parameter DECLARATION.
    if t == vpi::TOKEN_PORT_CONN_LABEL || t == vpi::TOKEN_PARAM_CONN_LABEL {
        return Some((SymKind::Var, false));
    }
    // Port declarations: the direction-specific synthetic types appear only at
    // declaration sites.
    if matches!(
        t,
        vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT
    ) {
        return Some((SymKind::Port, true));
    }
    // Signal/parameter-like objects.  In the parse-fallback path
    // (`forced_decl`) the position was recorded as a declaration by the core
    // collector, bypassing the instance-derived name gate; single-view parse
    // tokens would otherwise never satisfy the multi-view heuristic below.
    if SIGNAL_DECL_TYPES.contains(&t) {
        if !forced_decl && !signal_names.contains(name) {
            return None;
        }
        let types_at_pos: Vec<i32> = pos_types.get(&(line, col)).cloned().unwrap_or_default();
        let has_ref = types_at_pos.iter().any(|x| REF_TOKEN_TYPES.contains(x));
        let is_decl = forced_decl || (!has_ref && types_at_pos.len() >= 2);
        let kind = if port_names.contains(name) {
            SymKind::Port
        } else if param_names.contains(name) {
            SymKind::Param
        } else if is_net_type(t) {
            SymKind::Net
        } else {
            SymKind::Var
        };
        return Some((kind, is_decl));
    }
    // Module instantiation sites: the type name is a reference to the module
    // definition; class declarations are declarations.
    if t == vpi::uhdmclass_defn {
        if module_names.contains(name) {
            return Some((SymKind::Module, false));
        }
        return Some((SymKind::Class, true));
    }
    // Functions/tasks: named port connections carry the port name (a
    // reference); declarations of functions/tasks are declarations.
    if t == vpi::vpiFunction || t == vpi::vpiTask {
        if signal_names.contains(name) {
            return Some((SymKind::Var, false));
        }
        return Some((
            if t == vpi::vpiFunction {
                SymKind::Function
            } else {
                SymKind::Task
            },
            true,
        ));
    }
    match t {
        vpi::uhdmenum_const => {
            return Some((SymKind::EnumConst, !forced_enum_ref));
        }
        vpi::TOKEN_TYPEDEF_NAME => return Some((SymKind::Typedef, true)),
        vpi::uhdminterface_inst => return Some((SymKind::Interface, true)),
        vpi::vpiProgram | vpi::uhdmprogram => return Some((SymKind::Program, true)),
        _ => {}
    }
    None
}

/// Whether a VPI object type denotes a net (vs. a variable).
fn is_net_type(t: i32) -> bool {
    use llg::ffi::vpi;
    matches!(
        t,
        vpi::vpiNet
            | vpi::vpiNetBit
            | vpi::vpiReg
            | vpi::vpiRegBit
            | vpi::vpiLogicVar
            | vpi::uhdmnet
            | vpi::uhdmlogic_net
    )
}

/// Short human label for a symbol kind, used in hover details.
fn kind_label(kind: SymKind) -> &'static str {
    match kind {
        SymKind::Module => "module",
        SymKind::Interface => "interface",
        SymKind::Package => "package",
        SymKind::Instance => "instance",
        SymKind::Port => "port",
        SymKind::Net => "net",
        SymKind::Var => "var",
        SymKind::Param => "parameter",
        SymKind::GenScope => "generate scope",
        SymKind::EnumConst => "enum constant",
        SymKind::Typedef => "typedef",
        SymKind::Function => "function",
        SymKind::Task => "task",
        SymKind::Class => "class",
        SymKind::Program => "program",
    }
}

/// Hover text for a port/net/var/param declaration looked up in the model.
fn decl_detail(model: &DesignModel, kind: SymKind, name: &str) -> Option<String> {
    match kind {
        SymKind::Port => all_instances(&model.top_instances)
            .into_iter()
            .find_map(|i| i.ports.iter().find(|p| p.name == name).map(format_port)),
        SymKind::Net | SymKind::Var => all_instances(&model.top_instances)
            .into_iter()
            .find_map(|i| i.signals.iter().find(|s| s.name == name).map(format_signal)),
        SymKind::Param => param_display_model(&all_instances(&model.top_instances), name)
            .map(|p| format_param(&p)),
        _ => None,
    }
}

/// The display model for a same-named parameter across all instances.
///
/// The candidate flavor follows the historical precedence — direct non-local
/// parameters first (a same-named `localparam` of the instantiating scope
/// would otherwise shadow the child module's `parameter` in the hover text;
/// connection labels resolve INTO the child, so its parameter is the relevant
/// declaration), then any direct parameter, then generate-scope parameters —
/// but the displayed VALUE must be unanimous across every candidate of the
/// chosen flavor: under divergent per-instance overrides an arbitrary
/// instance's number would be a guess, so the value is cleared instead
/// ([`format_param`] then renders without a value).
fn param_display_model(insts: &[&InstanceModel], name: &str) -> Option<ParamModel> {
    fn direct<'m>(insts: &[&'m InstanceModel], name: &str, local: bool) -> Vec<&'m ParamModel> {
        insts
            .iter()
            .filter_map(|i| i.params.iter().find(|p| p.name == name && p.local == local))
            .collect()
    }
    let mut group = direct(insts, name, false);
    if group.is_empty() {
        group = direct(insts, name, true);
    }
    if group.is_empty() {
        group = insts
            .iter()
            .filter_map(|i| {
                i.gen_scopes
                    .iter()
                    .find_map(|gs| gs.params.iter().find(|p| p.name == name))
            })
            .collect();
    }
    let first = group.first()?;
    let mut distinct: Vec<&Val> = Vec::new();
    for p in &group {
        if let Some(v) = &p.value {
            if !distinct.contains(&v) {
                distinct.push(v);
            }
        }
    }
    let mut display = (*first).clone();
    if distinct.len() != 1 {
        display.value = None;
    }
    Some(display)
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// Convert Surelog diagnostics and lint findings into LSP
/// `textDocument/publishDiagnostics` payloads, keyed by file path.
///
/// Surelog severity mapping: Fatal/Syntax/Error → `ERROR`, Warning →
/// `WARNING`, Note → `INFORMATION`, Info → `HINT`.  Diagnostics without a
/// file are dropped (nothing to publish them to).
///
/// Lint findings (from [`Analysis::lint`]) are merged into the same per-file
/// map with `source: "llg-lint"`; lint severity maps Error → `ERROR`,
/// Warning → `WARNING`, Info → `INFORMATION` (the same ladder as Surelog's
/// Note/Info, so the two diagnostics kinds read consistently).  The rule id is
/// carried as the diagnostic `code`.  Files with only lint findings still get
/// a map entry.  Both position schemes are 1-based; 0 means unknown → report
/// at (0,0).
pub fn lsp_diagnostics(a: &Analysis) -> HashMap<String, Vec<LspDiagnostic>> {
    lsp_diagnostics_with_fallback(a, None)
}

/// Convert diagnostics like [`lsp_diagnostics`], optionally routing fileless
/// Surelog diagnostics to a caller-provided source path.
///
/// The fallback is used only when a diagnostic has no file of its own.  It is
/// intentionally a concrete source path supplied by the caller; no synthetic
/// URI or workspace-global destination is introduced.  Diagnostics that carry
/// a file, and all lint mappings, retain the behavior of [`lsp_diagnostics`].
pub fn lsp_diagnostics_with_fallback(
    a: &Analysis,
    fallback_file: Option<&Path>,
) -> HashMap<String, Vec<LspDiagnostic>> {
    let mut out: HashMap<String, Vec<LspDiagnostic>> = HashMap::new();
    let fallback_file = fallback_file.map(|path| path.to_string_lossy().into_owned());
    for d in &a.diagnostics {
        let Some(path) = d.file.as_deref().or(fallback_file.as_deref()) else {
            continue;
        };
        let severity = match d.severity {
            Severity::Fatal | Severity::Syntax | Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Note => DiagnosticSeverity::INFORMATION,
            Severity::Info => DiagnosticSeverity::HINT,
        };
        // Surelog positions are 1-based; 0 means unknown → report at (0,0).
        let (line, col) = if d.line == 0 {
            (0, 0)
        } else {
            (d.line - 1, d.col.saturating_sub(1))
        };
        let range = Range::new(Position::new(line, col), Position::new(line, col));
        out.entry(path.to_owned()).or_default().push(LspDiagnostic {
            range,
            severity: Some(severity),
            code: None,
            code_description: None,
            source: Some("surelog".to_owned()),
            message: d.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        });
    }
    for d in &a.lint {
        let Some(path) = d.file.as_deref() else {
            continue;
        };
        let severity = match d.severity {
            LintSeverity::Error => DiagnosticSeverity::ERROR,
            LintSeverity::Warning => DiagnosticSeverity::WARNING,
            LintSeverity::Info => DiagnosticSeverity::INFORMATION,
        };
        let (line, col) = if d.line == 0 {
            (0, 0)
        } else {
            (d.line - 1, d.col.saturating_sub(1))
        };
        let range = Range::new(Position::new(line, col), Position::new(line, col));
        out.entry(path.to_owned()).or_default().push(LspDiagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(d.rule.clone())),
            code_description: None,
            source: Some("llg-lint".to_owned()),
            message: d.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        });
    }
    out
}

// ── Semantic tokens ───────────────────────────────────────────────────────────

/// Encode the semantic tokens for `file` from the cached token lists.
///
/// Token-list matching retains its compatibility filename fallback. A syntax
/// diagnostic, however, is associated only by an exact or successfully
/// canonicalized path: a same-named file elsewhere in the workspace must not
/// suppress this file's tokens. A matching syntax diagnostic makes the empty
/// result authoritative.
pub fn semantic_tokens_for(a: &Analysis, file: &str) -> SemanticTokens {
    if a.diagnostics.iter().any(|diagnostic| {
        matches!(diagnostic.severity, Severity::Syntax)
            && diagnostic
                .file
                .as_deref()
                .is_some_and(|diagnostic_file| semantic_file_matches(diagnostic_file, file))
    }) {
        return empty_semantic_tokens();
    }
    match file_tokens(a, file) {
        Some(ft) => semantic_tokens::encode(&ft.nodes),
        None => empty_semantic_tokens(),
    }
}

fn empty_semantic_tokens() -> SemanticTokens {
    SemanticTokens {
        result_id: None,
        data: Vec::new(),
    }
}

fn semantic_file_matches(left: &str, right: &str) -> bool {
    if Path::new(left) == Path::new(right) {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Parse one staged open document in isolation and encode its parse-tree
/// semantic tokens.
///
/// This path deliberately does not build or update an [`Analysis`].  It runs
/// Surelog's parse-only mode behind the same process-wide lock and scratch-CWD
/// guard as project analysis, then drops the session before returning owned
/// LSP data.  The caller owns staging and cleanup of `file`.  Frontend syntax
/// diagnostics do not make this return an error.  Instead, any syntax
/// diagnostic produces an authoritative empty stream so an incomplete edit
/// cannot expose unstable partial highlighting or fall back to stale tokens.
#[allow(dead_code)] // compatibility wrapper; production uses the parent-aware variant
pub fn semantic_tokens_for_open_document(
    file: &str,
    defines: &[String],
) -> Result<SemanticTokens, String> {
    semantic_tokens_for_open_document_with_parent(file, defines, None)
}

/// Parent-aware variant used by an LSP semantic-token request.  Direct
/// callers retain the wrapper above and intentionally produce a no-parent
/// parse trace.
pub(crate) fn semantic_tokens_for_open_document_with_parent(
    file: &str,
    defines: &[String],
    parent_id: Option<u64>,
) -> Result<SemanticTokens, String> {
    let wait_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=surelog.parse_only.wait.begin file={} parent_id={:?}",
        file,
        parent_id
    );
    let mut wait_span = crate::logging::LifecycleSpan::phase_with_parent(
        "surelog.wait_global_mutex.parse_only",
        || file.to_owned(),
        0,
        1,
        parent_id,
    );
    let _guard = ANALYZE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    wait_span.complete("ok", 0);
    crate::llg_debug!(
        "event=surelog.parse_only.wait.end outcome=ok file={} elapsed_us={}",
        file,
        wait_started.elapsed().as_micros()
    );
    drop(wait_span);
    let _cwd = ScratchCwd::enter(&analysis_scratch_dir());
    let mut parse_span = crate::logging::LifecycleSpan::phase_with_parent(
        "surelog.parse_only",
        || file.to_owned(),
        0,
        1,
        parent_id,
    );
    let parse_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=surelog.parse_only.session_construct.begin file={} parent_id={:?}",
        file,
        parent_id
    );
    if crate::logging::enabled(crate::logging::Level::Debug) {
        match compile::parse_only_invocation(file, defines) {
            Ok(invocation) => log_surelog_invocation("parse_only", &invocation, file, 0, parent_id),
            Err(error) => log_surelog_invocation_rejected("parse_only", &error, file, 0, parent_id),
        }
    }
    let parsed = match compile::parse_only(file, defines) {
        Ok(parsed) => parsed,
        Err(error) => {
            crate::llg_debug!(
                "event=surelog.parse_only.return outcome=error file={} parent_id={:?} elapsed_us={} error={}",
                file,
                parent_id,
                parse_started.elapsed().as_micros(),
                bounded_log_text(&error, SURELOG_LOG_ERROR_MAX)
            );
            parse_span.outcome("error");
            return Err(error);
        }
    };
    let token_count = token_cardinality(&parsed.tokens);
    let diagnostic_count = parsed.diagnostics.len();
    let syntax_error_count = parsed
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Syntax))
        .count();
    if syntax_error_count > 0 && crate::logging::enabled(crate::logging::Level::Debug) {
        let first_syntax = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| matches!(diagnostic.severity, Severity::Syntax))
            .map(|diagnostic| bounded_log_text(&diagnostic.message, SURELOG_LOG_ERROR_MAX))
            .unwrap_or_else(|| "-".to_owned());
        crate::llg_debug!(
            "event=surelog.parse_only.syntax_error file={} parent_id={:?} count={} first_message={}",
            file,
            parent_id,
            syntax_error_count,
            first_syntax
        );
    }
    crate::llg_debug!(
        "event=surelog.parse_only.return outcome=ok file={} parent_id={:?} diagnostics={} token_files={} token_nodes={} elapsed_us={}",
        file,
        parent_id,
        diagnostic_count,
        parsed.tokens.len(),
        token_count,
        parse_started.elapsed().as_micros()
    );
    crate::llg_debug!(
        "event=surelog.parse_only.diagnostics.end outcome=extracted file={} parent_id={:?} diagnostics={} error_like={} elapsed_us={}",
        file,
        parent_id,
        diagnostic_count,
        parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.severity,
                    Severity::Fatal | Severity::Syntax | Severity::Error
                )
            })
            .count(),
        parse_started.elapsed().as_micros()
    );
    crate::llg_debug!(
        "event=analysis.parse_only.token_collection.end outcome=collected file={} parent_id={:?} parsed_nodes={} supplemented_nodes={} token_files={} token_nodes={}",
        file,
        parent_id,
        parsed.parsed_token_count,
        parsed.supplemented_token_count,
        parsed.tokens.len(),
        token_count,
    );
    crate::llg_debug!(
        "event=analysis.parse_only.source_supplementation.end outcome=completed file={} parent_id={:?} supplemented_nodes={}",
        file,
        parent_id,
        parsed.supplemented_token_count,
    );
    crate::llg_debug!(
        "event=surelog.parse_only.session_drop.end outcome=complete file={} parent_id={:?}",
        file,
        parent_id
    );
    parse_span.complete("ok", token_count);
    drop(parse_span);
    let mut encode_span = crate::logging::LifecycleSpan::phase_with_parent(
        "analysis.parse_only_token_encoding",
        || file.to_owned(),
        0,
        1,
        parent_id,
    );
    let encode_started = std::time::Instant::now();
    let file_name = Path::new(file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let tokens = parsed
        .tokens
        .iter()
        .find(|tokens| tokens.path == file)
        .or_else(|| {
            parsed.tokens.iter().find(|tokens| {
                !file_name.is_empty()
                    && Path::new(&tokens.path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        == Some(file_name)
            })
        });
    let result = if syntax_error_count > 0 {
        empty_semantic_tokens()
    } else {
        tokens
            .map(|tokens| semantic_tokens::encode(&tokens.nodes))
            .unwrap_or_else(empty_semantic_tokens)
    };
    let outcome = if syntax_error_count > 0 {
        "syntax-error"
    } else {
        "ok"
    };
    encode_span.complete(outcome, result.data.len());
    crate::llg_debug!(
        "event=analysis.parse_only_token_encoding.end outcome={} file={} token_count={} syntax_errors={} suppressed_token_nodes={} elapsed_us={}",
        outcome,
        file,
        result.data.len(),
        syntax_error_count,
        if syntax_error_count > 0 { token_count } else { 0 },
        encode_started.elapsed().as_micros()
    );
    drop(encode_span);
    Ok(result)
}

// ── Hover ─────────────────────────────────────────────────────────────────────

/// Elaborated value shown for a parameter/localparam declaration, read ONLY
/// from the committed model ([`Analysis::model`] — the root's last-good
/// snapshot): instance parameters, generate-scope parameters, or package
/// parameters.  Nothing is parsed or elaborated in the request path; when the
/// value is not in the committed model the line is omitted.
///
/// Scoping: the declaration position selects the enclosing module definition
/// (span containment) and every elaborated instance of that module
/// contributes its same-named parameter (direct or generate-scope).  All
/// resolved values must agree on one distinct constant — divergent per-
/// instance overrides make the value ambiguous at module granularity (the
/// binding map keys source positions shared by every clone), and ambiguity
/// omits rather than guesses.  Positions outside any module resolve against
/// same-file packages.  `None` when unresolved.
fn param_elab_value<'a>(a: &'a Analysis, file: &str, line0: u32, name: &str) -> Option<&'a Val> {
    let line1 = line0.saturating_add(1);
    if let Some(def_name) = a
        .model
        .modules
        .iter()
        .find(|m| {
            m.file.as_deref() == Some(file) && m.line <= line1 && line1 <= m.end_line.max(m.line)
        })
        .map(|m| clean_name(&m.name))
    {
        return unique_value(inst_param_values(&a.model.top_instances, def_name, name));
    }
    let package_values = a.model.packages.iter().flat_map(|p| {
        if p.file.as_deref() != Some(file) {
            return [].as_slice();
        }
        p.params.as_slice()
    });
    let values: Vec<&Val> = package_values
        .filter(|p| p.name == name)
        .filter_map(|p| p.value.as_ref())
        .collect();
    unique_value(values)
}

/// Same-named parameter values (direct + generate-scope) across every
/// elaborated instance of the module named `def_name`.
fn inst_param_values<'m>(tops: &'m [InstanceModel], def_name: &str, name: &str) -> Vec<&'m Val> {
    let mut values: Vec<&Val> = Vec::new();
    for inst in all_instances(tops) {
        if clean_name(&inst.def_name) != def_name {
            continue;
        }
        for p in &inst.params {
            if p.name == name {
                if let Some(v) = &p.value {
                    values.push(v);
                }
            }
        }
        for gs in &inst.gen_scopes {
            for p in &gs.params {
                if p.name == name {
                    if let Some(v) = &p.value {
                        values.push(v);
                    }
                }
            }
        }
    }
    values
}

/// The single distinct value of `values`; `None` when empty or ambiguous.
fn unique_value(values: Vec<&Val>) -> Option<&Val> {
    let mut distinct: Vec<&Val> = Vec::new();
    for v in values {
        if !distinct.contains(&v) {
            distinct.push(v);
        }
    }
    match distinct.len() {
        1 => distinct.into_iter().next(),
        _ => None,
    }
}

/// Append the elaborated-value line (`value = <const>`) to a parameter hover
/// detail.  An identical inline tail (`… = <value>` from [`format_param`] or
/// the model detail) is normalized into the dedicated line so the value is
/// rendered exactly once, matching the house two-line shape:
/// `localparam int WIDTH` / `value = 8`.
fn with_elab_value(detail: String, value: &Val) -> String {
    let rendered = value.format_verilog();
    match detail
        .strip_suffix(&rendered)
        .and_then(|r| r.strip_suffix(" = "))
    {
        Some(base) => format!("{base}\nvalue = {rendered}"),
        None => format!("{detail}\nvalue = {rendered}"),
    }
}

/// Return a hover for the symbol at the 0-based `(line, col)` position, if any.
///
/// Macro usages (`` `NAME ``) and `` `define `` NAME identifiers are served
/// FIRST from the analysis' macro table: their positions never carry indexed
/// symbols (preprocessing removes the directives and expands the usages), so
/// the macro answer is authoritative there.  Everything else follows the
/// reference-binding map (mirroring [`definition_at`]): a position that
/// exactly matches a captured binding key serves the bound declaration's
/// detail, which is precise even under inner-scope shadowing where the
/// name-based index would describe the outer same-named object.  Otherwise
/// the identifier is resolved through the symbol index (so reference sites
/// resolve to their declaration, cross-file included); the hover body is a
/// SystemVerilog code block (e.g. `input logic [7:0] count`).
/// Parameter/localparam hovers additionally show the elaborated value from
/// the committed model as a `value = <const>` line (omitted when
/// unresolved/ambiguous).  Function/task declarations additionally show the
/// instance scope and the `automatic`/`static` storage class from the model's
/// per-instance clone.  Falls back to the token-based v1 lookup when the
/// index has nothing.
pub fn hover_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Hover> {
    // Macro positions are disjoint from every indexed symbol, so checking
    // them first is behavior-preserving for all non-macro clicks while a
    // click on a usage span can never be captured by an expanded-content
    // reference bound near the preprocessed columns.
    if let Some(hover) = macro_hover_at(a, file, line, col) {
        return Some(hover);
    }
    // Cursor normalization mirrors `definition_at`: a click mid-identifier
    // must reuse the token's start column, where bindings are keyed.
    let clicked = a.index.entry_at(file, line, col);
    let bound = a
        .ref_bindings
        .get(&(file.to_string(), line, col))
        .or_else(|| clicked.and_then(|e| a.ref_bindings.get(&(file.to_string(), line, e.col))));
    if let Some(target) = bound {
        let anchor_len = clicked
            .map(|e| lsp_name_len(&e.name))
            .unwrap_or_else(|| lsp_name_len(&target.name));
        let range_col = clicked.map(|e| e.col).unwrap_or(col);
        if let Some(hover) = hover_for_target(a, target, line, range_col, anchor_len) {
            return Some(hover);
        }
    }
    if let Some(e) = a.index.entry_at(file, line, col) {
        let decl = a.index.resolve(e).into_iter().next().unwrap_or(e);
        let mut detail = decl
            .detail
            .clone()
            .or_else(|| hover_detail(a, file, &e.name));
        if matches!(decl.kind, SymKind::Param) {
            if let (Some(d), Some(v)) = (
                detail.as_ref(),
                param_elab_value(a, &decl.file, decl.line, &decl.name),
            ) {
                detail = Some(with_elab_value(d.clone(), v));
            }
        }
        if let Some(f) = func_from_decl(&a.model, decl) {
            let extra = format!(
                "scope: {}\n{}",
                f.scope,
                if f.automatic { "automatic" } else { "static" }
            );
            detail = Some(match detail {
                Some(d) => format!("{d}\n\n{extra}"),
                None => extra,
            });
        }
        if let Some(detail) = detail {
            let len = lsp_name_len(&e.name);
            let range = Range::new(
                Position::new(e.line, e.col),
                Position::new(e.line, e.col + len),
            );
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```systemverilog\n{detail}\n```"),
                }),
                range: Some(range),
            });
        }
    }
    hover_fallback(a, file, line, col)
}

/// Macro-usage / `` `define ``-site hover, rendered purely from the committed
/// macro table ([`Analysis`]'s `core::macros::MacroTable`).
///
/// House style mirrors parameter hovers: a fenced SystemVerilog block with
/// one short detail line — `macro WIDTH = 8`, `macro MAX(a, b) = …`, or
/// `macro ENABLE` for a body-less macro — plus a `defined at <file>:<line>`
/// line for source-origin definitions (config defines name no site; the
/// shared-file hover annotation already labels config-derived text).  An
/// undefined usage renders an explicit not-defined message naming the
/// configuration source instead of any value.
fn macro_hover_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Hover> {
    if a.macros.is_empty() {
        return None;
    }
    let range_of = |col_start0: u32, col_end0: u32| -> Range {
        Range::new(
            Position::new(line, col_start0),
            Position::new(line, col_end0),
        )
    };
    if let Some(usage) = a.macros.usage_at(file, line, col) {
        let range = range_of(usage.col_start0, usage.col_end0);
        return Some(build_macro_hover(
            usage.name(),
            usage.definition.as_ref(),
            a.macros.config_note(),
            range,
        ));
    }
    if let Some(decl) = a.macros.decl_at(file, line, col) {
        let range = range_of(decl.col_start0, decl.col_end0);
        return Some(build_macro_hover(
            &decl.definition.name,
            Some(&decl.definition),
            a.macros.config_note(),
            range,
        ));
    }
    None
}

/// Assemble the macro hover payload: defined macros render their value in
/// the standard code fence, undefined usages render the not-defined message
/// as plain markdown.
fn build_macro_hover(
    name: &str,
    definition: Option<&macros::MacroDefinition>,
    config_note: Option<&str>,
    range: Range,
) -> Hover {
    let value = match definition {
        Some(def) => {
            let mut detail = format!("macro {}", def.name);
            if let Some(args) = &def.args {
                detail.push_str(&format!("({})", args.join(", ")));
            }
            if !def.body.is_empty() {
                detail.push_str(&format!(" = {}", def.body));
            }
            if let (Some(file), 1..) = (&def.file, def.line1) {
                // Open buffers compile under shadow paths; display the real
                // project path so the site is recognizable to the user.
                let display = real_path(Path::new(file), &process_shadow_base())
                    .unwrap_or_else(|| PathBuf::from(file));
                detail.push_str(&format!(
                    "\n\ndefined at {}:{}",
                    display.display(),
                    def.line1
                ));
            }
            format!("```systemverilog\n{detail}\n```")
        }
        None => {
            let origin = match config_note {
                Some(note) => format!(
                    "Checked the `[compile] defines` in {note} and preceding \
                     source `` `define `` directives."
                ),
                None => "Checked `[compile] defines` and preceding source `` `define `` \
                     directives."
                    .to_owned(),
            };
            format!("`{name}` is not defined under the current configuration.\n\n{origin}")
        }
    };
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    }
}

/// Hover for a binding-bound declaration target.
///
/// Detail precedence: the indexed DECLARATION entry at the target position
/// (carries model data for functions/classes/instances), then the
/// position-accurate snippet captured during the VPI walk
/// ([`Analysis::decl_details`]), then the name-based model lookup.  Parameter
/// targets additionally gain the elaborated-value line from the committed
/// model (omitted when unresolved/ambiguous).  `None` when no detail can be
/// produced — the caller falls through to the index path instead of rendering
/// an empty hover.  The hover range stays anchored at the CLICKED identifier
/// (`line`, `range_col`, `len`).
fn hover_for_target(
    a: &Analysis,
    target: &DeclTarget,
    line: u32,
    range_col: u32,
    len: u32,
) -> Option<Hover> {
    let entry_detail = a
        .index
        .entry_at(&target.file, target.line0, target.col0)
        .filter(|e| e.is_decl)
        .and_then(|e| e.detail.clone());
    let detail = entry_detail.or_else(|| {
        a.decl_details
            .get(&(target.file.clone(), target.line0 + 1, target.col0 + 1))
            .filter(|text| !text.is_empty())
            .cloned()
            .or_else(|| {
                // The captured snippet names the declared object; fall
                // back to it even when only the name matches the model.
                hover_detail(a, &target.file, &target.name)
            })
    })?;
    // Parameter targets (elaboration-bound refs, override labels included)
    // gain the elaborated-value line from the committed model.
    let detail = if target.kind == "parameter" {
        match param_elab_value(a, &target.file, target.line0, &target.name) {
            Some(v) => with_elab_value(detail, v),
            None => detail,
        }
    } else {
        detail
    };
    let range = Range::new(
        Position::new(line, range_col),
        Position::new(line, range_col + len),
    );
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```systemverilog\n{detail}\n```"),
        }),
        range: Some(range),
    })
}

fn hover_fallback(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Hover> {
    let node = token_at(a, file, line, col)?;
    let name = node.name.as_deref()?;
    let detail = hover_detail(a, file, name)?;
    let len = lsp_name_len(name);
    let start_col = node.col.saturating_sub(1);
    let range = Range::new(
        Position::new(line, start_col),
        Position::new(line, start_col + len),
    );
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```systemverilog\n{detail}\n```"),
        }),
        range: Some(range),
    })
}

fn hover_detail(a: &Analysis, file: &str, name: &str) -> Option<String> {
    // Module definitions declared in this file.
    for m in a.model.modules_in(file) {
        if clean_name(&m.name) == name {
            return Some(format!("module {name}"));
        }
    }
    // Packages declared in this file.
    for p in &a.model.packages {
        if clean_name(&p.name) == name && p.file.as_deref() == Some(file) {
            return Some(format!("package {name}"));
        }
        // Package items (parameters and enum constants) declared in this file.
        for param in &p.params {
            if clean_name(&param.name) == name {
                return Some(format_param(param));
            }
        }
        for ec in &p.enum_consts {
            if clean_name(&ec.name) == name {
                return Some(format_enum_const(ec));
            }
        }
    }
    // Instances (declared in this file) and the ports/signals/params/gen-scope
    // params of every instantiated module.
    for inst in all_instances(&a.model.top_instances) {
        if inst.file.as_deref() == Some(file) && inst.name == name {
            let mut detail = format!("{} {}", clean_name(&inst.def_name), inst.name);
            if let Some((f, l)) = def_site_line(a, &inst.def_name) {
                detail.push_str(&format!("\n\ndefined at {f}:{l}"));
            }
            return Some(detail);
        }
        for port in &inst.ports {
            if port.name == name {
                return Some(format_port(port));
            }
        }
        for sig in &inst.signals {
            if sig.name == name {
                return Some(format_signal(sig));
            }
        }
        for p in &inst.params {
            if p.name == name {
                return Some(format_param(p));
            }
        }
        for gs in &inst.gen_scopes {
            for p in &gs.params {
                if p.name == name {
                    return Some(format_param(p));
                }
            }
        }
    }
    // Classes declared in this file (and their methods/fields).
    for c in &a.model.classes {
        if c.file.as_deref() != Some(file) {
            continue;
        }
        if clean_name(&c.name) == name {
            return Some(format_class(c));
        }
        for m in &c.methods {
            if clean_name(&m.name) == name {
                return Some(func_signature(m));
            }
        }
        for f in &c.fields {
            if clean_name(&f.name) == name {
                return Some(format_class_field(f));
            }
        }
    }
    None
}

fn format_port(p: &PortModel) -> String {
    let dir = match p.direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "",
    };
    if dir.is_empty() {
        format!("{} {}", p.ty.render(), p.name)
    } else {
        format!("{dir} {} {}", p.ty.render(), p.name)
    }
}

fn format_signal(s: &SignalModel) -> String {
    if s.kind == "array" {
        format!("array {} {}", s.ty.render(), s.name)
    } else {
        format!("{} {}", s.ty.render(), s.name)
    }
}

fn format_param(p: &ParamModel) -> String {
    let kw = if p.local { "localparam" } else { "parameter" };
    match &p.value {
        Some(v) => format!(
            "{kw} {}: {} = {}",
            p.name,
            p.ty.render(),
            v.format_verilog()
        ),
        None => format!("{kw} {}: {}", p.name, p.ty.render()),
    }
}

/// Hover text for a package enum constant, e.g. `enum const IDLE = 2'sd0`.
fn format_enum_const(ec: &EnumConstDef) -> String {
    match &ec.value {
        Some(v) => format!(
            "enum const {} = {}",
            clean_name(&ec.name),
            v.format_verilog()
        ),
        None => format!("enum const {}", clean_name(&ec.name)),
    }
}

/// One class data member as SystemVerilog text, e.g. `int count`.
fn format_class_field(f: &ClassFieldDef) -> String {
    format!("{} {}", f.ty.render(), clean_name(&f.name))
}

/// Hover text for a class definition: the declaration plus its methods and
/// fields, e.g. `class Counter` followed by `function int get()` and
/// `int count` entries.
fn format_class(c: &ClassDef) -> String {
    let name = clean_name(&c.name);
    let mut out = format!("class {name}");
    if !c.methods.is_empty() {
        out.push_str("\n\nmethods:");
        for m in &c.methods {
            out.push_str(&format!("\n  {}", func_signature(m)));
        }
    }
    if !c.fields.is_empty() {
        out.push_str("\n\nfields:");
        for f in &c.fields {
            out.push_str(&format!("\n  {}", format_class_field(f)));
        }
    }
    out
}

/// One formal argument of a function/task as SystemVerilog text, e.g.
/// `input int a`.
fn format_func_arg(a: &FuncArgDef) -> String {
    let dir = match a.direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "",
    };
    if dir.is_empty() {
        format!("{} {}", a.ty.render(), a.name)
    } else {
        format!("{dir} {} {}", a.ty.render(), a.name)
    }
}

/// SystemVerilog signature of a function/task, e.g.
/// `function int add(input int a, input int b)` / `task run(input int n)`.
/// Void functions render as `function void name(...)`.
fn func_signature(f: &FuncDef) -> String {
    let args: Vec<String> = f.args.iter().map(format_func_arg).collect();
    let args = args.join(", ");
    let name = clean_name(&f.name);
    if f.is_task {
        format!("task {name}({args})")
    } else {
        match &f.ret {
            Some(ty) => format!("function {} {name}({args})", ty.render()),
            None => format!("function void {name}({args})"),
        }
    }
}

/// The model [`FuncDef`] backing a function/task declaration entry, matched
/// by (instance scope, name, declaration line).  `None` for token-only
/// entries the model did not capture.
fn func_from_decl<'m>(model: &'m DesignModel, decl: &SymEntry) -> Option<&'m FuncDef> {
    let want_scope = decl.scope.as_deref()?;
    // Index positions are 0-based; model positions are 1-based.
    let want_line = decl.line.saturating_add(1);
    all_instances(&model.top_instances)
        .into_iter()
        .find_map(|inst| {
            if inst.full_name != want_scope && clean_name(&inst.full_name) != want_scope {
                return None;
            }
            inst.funcs
                .iter()
                .find(|f| f.name == decl.name && f.line == want_line)
        })
}

fn def_site_line(a: &Analysis, def_name: &str) -> Option<(String, u32)> {
    a.model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == clean_name(def_name))
        .and_then(|m| m.file.as_ref().map(|f| (f.clone(), m.line)))
}

// ── Definition ────────────────────────────────────────────────────────────────

/// Resolve the declaration of the identifier at the 0-based position.
///
/// The reference-binding map is consulted FIRST: a position that exactly
/// matches a captured binding key (the 0-based start of an emitted reference
/// token — UHDM `vpiActual` capture or resolved port/parameter connection
/// label) serves the single bound declaration location, which is precise
/// even where the name-based index would be ambiguous.  On a miss the symbol
/// index resolves the entry at the position to its declaration (cross-file
/// included); when the index has nothing to offer, the v1 name-based
/// fallback applies:
/// 1. a module definition declared in the same file;
/// 2. an instance name → the module definition of its type (`def_name`);
/// 3. a package name → the package declaration;
/// 4. a same-file port/signal/parameter declaration nearest by line — never
///    the clicked connection-label token itself, so an unresolvable label
///    yields NO result instead of a no-op self jump;
/// 5. a module definition in any file, as a cross-file fallback.
pub fn definition_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Location> {
    if let Some(target) = a.ref_bindings.get(&(file.to_string(), line, col)) {
        return Some(ref_target_location(target));
    }
    if a.index.is_unresolved_enum_ref(file, line, col) {
        return None;
    }
    // Cursor normalization: a click mid-identifier must reuse the token's
    // start column, where UHDM ref->decl bindings are keyed.
    if let Some(e) = a.index.entry_at(file, line, col) {
        if let Some(target) = a.ref_bindings.get(&(file.to_string(), line, e.col)) {
            return Some(ref_target_location(target));
        }
    }
    if let Some(e) = a.index.entry_at(file, line, col) {
        if let Some(decl) = a.index.resolve(e).into_iter().next() {
            return Some(entry_location(decl));
        }
        // A scanned parameter-override label that failed resolution: its
        // namespace is the instantiated module, so the name-based fallback
        // below would jump to a same-named object of the instantiating scope
        // (or an arbitrary workspace match).  No result is the honest answer.
        if a.index.is_unresolved_param_label(file, line, e.col) {
            return None;
        }
        if a.index.is_unresolved_enum_ref(file, line, e.col) {
            return None;
        }
    }
    definition_fallback(a, file, line, col)
}

/// LSP location for a bound declaration target (already 0-based), spanning
/// the target name with the same identifier-width convention as
/// [`entry_location`].
fn ref_target_location(t: &DeclTarget) -> Location {
    let uri = Url::from_file_path(&t.file)
        .unwrap_or_else(|_| Url::parse("untitled:llg").expect("static URL is valid"));
    let len = lsp_name_len(&t.name);
    Location {
        uri,
        range: Range::new(
            Position::new(t.line0, t.col0),
            Position::new(t.line0, t.col0 + len),
        ),
    }
}

fn definition_fallback(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Location> {
    let node = token_at(a, file, line, col)?;
    let name = node.name.as_deref()?;

    for m in a.model.modules_in(file) {
        if clean_name(&m.name) == name {
            return module_def_location(a, m);
        }
    }

    if let Some(inst) = all_instances(&a.model.top_instances)
        .into_iter()
        .find(|i| i.name == name)
    {
        if let Some(def) = a
            .model
            .modules
            .iter()
            .find(|m| clean_name(&m.name) == clean_name(&inst.def_name))
        {
            return module_def_location(a, def);
        }
        return instance_location(inst);
    }

    if let Some(pkg) = a
        .model
        .packages
        .iter()
        .find(|p| clean_name(&p.name) == name)
    {
        return package_location(pkg);
    }

    if let Some((l1, c1)) =
        nearest_declaration(a, file, name, line, skip_self_label(a, file, line, col))
    {
        return Some(location(file, l1, c1, lsp_name_len(name) as usize));
    }

    a.model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == name)
        .and_then(|m| module_def_location(a, m))
}

// ── References ────────────────────────────────────────────────────────────────

/// All occurrences of the identifier at the 0-based position, including the
/// declaration itself, deduplicated by position.  The symbol index resolves
/// the entry to its declaration and collects every reference that resolves to
/// the same declaration across the whole workspace; when the index has
/// nothing, the v1 same-file name-based search applies.
#[cfg_attr(not(test), allow(dead_code))]
pub fn references_at(a: &Analysis, file: &str, line: u32, col: u32) -> Vec<Location> {
    references_at_with_options(a, file, line, col, true)
}

/// All occurrences of the identifier at the 0-based position, honoring the
/// LSP `ReferenceContext.includeDeclaration` option.
///
/// Binding-aware under inner-scope shadowing: when the queried identifier is
/// itself bound (`ref_bindings`), the reference set belongs to exactly the
/// bound declaration — occurrences whose own binding points at a DIFFERENT
/// declaration are excluded even though the name-based resolution would
/// conflate them, and precisely-bound occurrences outside the name-based
/// candidate pool are included.  Positions without bindings keep the v1
/// name+scope behavior (parse-fallback analyses have no bindings at all).
pub fn references_at_with_options(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
    include_declaration: bool,
) -> Vec<Location> {
    if let Some(e) = a.index.entry_at(file, line, col) {
        if a.index.is_unresolved_enum_ref(file, line, e.col) {
            return Vec::new();
        }
        // Head declarations this query belongs to.  A binding on the query
        // position anchors the set to exactly that declaration; otherwise the
        // index resolution decides (a declaration resolves to itself).
        let mut heads: HashSet<(String, u32, u32)> = HashSet::new();
        match binding_target_at(a, file, line, col, e) {
            Some(target) => {
                heads.insert((target.file.clone(), target.line0, target.col0));
            }
            None => {
                for d in a.index.resolve(e) {
                    heads.insert((d.file.clone(), d.line, d.col));
                }
            }
        }
        if !heads.is_empty() {
            let locations = shadow_aware_reference_locations(a, e, &heads, include_declaration);
            if !locations.is_empty() {
                return locations;
            }
        }
    }
    references_fallback_with_options(a, file, line, col, include_declaration)
}

/// The binding target for the query position: exact key first, then the
/// containing identifier's start column (cursor normalization, matching
/// [`definition_at`]).
fn binding_target_at<'a>(
    a: &'a Analysis,
    file: &str,
    line: u32,
    col: u32,
    e: &SymEntry,
) -> Option<&'a DeclTarget> {
    a.ref_bindings
        .get(&(file.to_string(), line, col))
        .or_else(|| a.ref_bindings.get(&(file.to_string(), line, e.col)))
}

/// Collect the binding-aware occurrence set of `e`'s identifier for the head
/// declarations in `heads`.
///
/// The v1 pool ([`SymbolIndex::all_references`]) stays the candidate base —
/// it carries the resolution chains (instance → module definition,
/// connection labels → child declarations, name/scope approximations).  On
/// top of it:
///
/// * DECLARATION occurrences are kept iff their position is one of the heads
///   (gated by `include_declaration`) — under shadowing the v1 pool contains
///   every same-named declaration the conflated resolution produced, and
///   only the queried declaration's own site may stay;
/// * REFERENCE occurrences whose captured binding targets a DIFFERENT
///   declaration are dropped — the property that keeps shadowed declarations'
///   reference sets disjoint;
/// * precisely-bound positions outside the v1 pool whose binding targets a
///   head are added (connection labels/actuals, interior refs).
fn shadow_aware_reference_locations(
    a: &Analysis,
    e: &SymEntry,
    heads: &HashSet<(String, u32, u32)>,
    include_declaration: bool,
) -> Vec<Location> {
    let mut out: Vec<SymEntry> = Vec::new();
    let mut seen: HashSet<(String, u32, u32)> = HashSet::new();

    // True declaration sites per the UHDM capture: positions recorded as
    // declared objects behave like declarations even when the multi-view
    // classification left them REF-shaped.
    let is_decl_position = |key: &(String, u32, u32)| a.decl_details.contains_key(key);

    let targets_head =
        |target: &DeclTarget| heads.contains(&(target.file.clone(), target.line0, target.col0));

    for occurrence in a.index.all_references(e) {
        let key = (occurrence.file.clone(), occurrence.line, occurrence.col);
        if !seen.insert(key.clone()) {
            continue;
        }
        if occurrence.is_decl || is_decl_position(&key) {
            if include_declaration && heads.contains(&key) {
                out.push(occurrence);
            } else {
                seen.remove(&key);
            }
            continue;
        }
        let keep = match binding_target_at(
            a,
            &occurrence.file,
            occurrence.line,
            occurrence.col,
            &occurrence,
        ) {
            Some(target) => targets_head(target),
            None => a
                .index
                .resolve(&occurrence)
                .iter()
                .any(|d| heads.contains(&(d.file.clone(), d.line, d.col))),
        };
        if keep {
            out.push(occurrence);
        } else {
            seen.remove(&key);
        }
    }

    for ((bfile, bline, bcol), target) in &a.ref_bindings {
        if target.name != e.name || !targets_head(target) {
            continue;
        }
        let key = (bfile.clone(), *bline, *bcol);
        if seen.insert(key.clone()) {
            out.push(SymEntry {
                name: e.name.clone(),
                kind: SymKind::Var,
                file: bfile.clone(),
                line: *bline,
                col: *bcol,
                end_line: *bline,
                end_col: bcol.saturating_add(lsp_name_len(&e.name)),
                is_decl: false,
                scope: None,
                detail: None,
            });
        }
    }

    out.into_iter()
        .map(|entry| entry_location(&entry))
        .collect()
}

fn references_fallback_with_options(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
    include_declaration: bool,
) -> Vec<Location> {
    let Some(node) = token_at(a, file, line, col) else {
        return Vec::new();
    };
    let Some(name) = node.name.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    if let Some(ft) = file_tokens(a, file) {
        for n in &ft.nodes {
            if n.name.as_deref() == Some(name)
                && (include_declaration || !is_declaration_vpi_type(n.vpi_type))
                && seen.insert((n.line, n.col))
            {
                out.push(location(file, n.line, n.col, lsp_name_len(name) as usize));
            }
        }
    }
    out
}

// ── Workspace symbols ─────────────────────────────────────────────────────────

/// All workspace declarations whose name contains `query` (case-insensitive),
/// mapped to LSP `SymbolInformation` for `workspace/symbol`.
pub fn workspace_symbols(a: &Analysis, query: &str) -> Vec<SymbolInformation> {
    let q = query.to_lowercase();
    a.index
        .decls
        .iter()
        .filter(|d| d.name.to_lowercase().contains(&q))
        .map(symbol_info)
        .collect()
}

#[allow(deprecated)] // `SymbolInformation::deprecated` is deprecated in lsp-types
fn symbol_info(d: &SymEntry) -> SymbolInformation {
    SymbolInformation {
        name: d.name.clone(),
        kind: sym_kind_to_lsp(d.kind),
        tags: None,
        deprecated: None,
        location: entry_location(d),
        container_name: d.scope.clone(),
    }
}

/// Map a model [`SymKind`] to the LSP [`SymbolKind`] used by document/workspace
/// symbols.
fn sym_kind_to_lsp(kind: SymKind) -> SymbolKind {
    match kind {
        SymKind::Module => SymbolKind::MODULE,
        SymKind::Interface => SymbolKind::INTERFACE,
        SymKind::Package => SymbolKind::PACKAGE,
        SymKind::Instance => SymbolKind::OBJECT,
        SymKind::Port => SymbolKind::PROPERTY,
        SymKind::Net | SymKind::Var => SymbolKind::VARIABLE,
        SymKind::Param => SymbolKind::CONSTANT,
        SymKind::GenScope => SymbolKind::NAMESPACE,
        SymKind::EnumConst => SymbolKind::ENUM_MEMBER,
        SymKind::Typedef => SymbolKind::TYPE_PARAMETER,
        SymKind::Function | SymKind::Task => SymbolKind::FUNCTION,
        SymKind::Class => SymbolKind::CLASS,
        SymKind::Program => SymbolKind::MODULE,
    }
}

// ── Document symbols ──────────────────────────────────────────────────────────

/// Hierarchical symbols for `file`: one `DocumentSymbol` per module definition
/// (with its ports/signals/parameters as type-detail children and its
/// instantiation sites as Object-kind leaf children whose `detail` is the
/// instantiated module type), one per package, one per class definition (with
/// its methods and fields as children), plus one per function/task declared
/// in the file (the per-instance clones all point at the same definition
/// site, so the symbols dedupe by position in the LSP client).
///
/// v1 keeps package symbols flat: package parameters/enum constants are not
/// emitted as document children — they surface through completion (after
/// `pkg::`), hover, and goto-definition instead.
pub fn document_symbols(a: &Analysis, file: &str) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for m in a.model.modules_in(file) {
        out.push(module_symbol(a, file, m));
    }
    for p in &a.model.packages {
        if p.file.as_deref() == Some(file) {
            out.push(package_symbol(p));
        }
    }
    for c in &a.model.classes {
        if c.file.as_deref() == Some(file) {
            out.push(class_symbol(a, file, c));
        }
    }
    for inst in all_instances(&a.model.top_instances) {
        for f in &inst.funcs {
            if f.file.as_deref() == Some(file) {
                out.push(func_symbol(f));
            }
        }
    }
    out
}

/// A `DocumentSymbol` for a function/task definition (flat: no argument
/// children), ranged over the definition name.
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn func_symbol(f: &FuncDef) -> DocumentSymbol {
    let name = clean_name(&f.name).to_owned();
    let len = lsp_name_len(&name);
    let line = f.line.saturating_sub(1);
    let col = f.col.saturating_sub(1);
    let range = Range::new(Position::new(line, col), Position::new(line, col + len));
    DocumentSymbol {
        name,
        detail: Some(func_signature(f)),
        kind: SymbolKind::FUNCTION,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn module_symbol(a: &Analysis, file: &str, m: &ModuleDef) -> DocumentSymbol {
    let name = clean_name(&m.name).to_owned();
    let len = lsp_name_len(&name);
    let start_line = m.line.saturating_sub(1);
    let start_col = m.col.saturating_sub(1);
    let range = Range::new(
        Position::new(start_line, start_col),
        Position::new(m.end_line.saturating_sub(1), m.end_col.saturating_sub(1)),
    );
    let selection_range = Range::new(
        Position::new(start_line, start_col),
        Position::new(start_line, start_col + len),
    );
    DocumentSymbol {
        name,
        detail: None,
        kind: SymbolKind::MODULE,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: module_children(a, file, &m.name),
    }
}

#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn package_symbol(p: &PackageDef) -> DocumentSymbol {
    let name = clean_name(&p.name).to_owned();
    let len = lsp_name_len(&name);
    let line = p.line.saturating_sub(1);
    let col = p.col.saturating_sub(1);
    let range = Range::new(Position::new(line, col), Position::new(line, col + len));
    DocumentSymbol {
        name,
        detail: None,
        kind: SymbolKind::PACKAGE,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

/// A `DocumentSymbol` for a class definition, ranged over the class name with
/// its methods and fields as children (see [`class_children`]).
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn class_symbol(a: &Analysis, file: &str, c: &ClassDef) -> DocumentSymbol {
    let name = clean_name(&c.name).to_owned();
    let len = lsp_name_len(&name);
    let line = c.line.saturating_sub(1);
    let col = c.col.saturating_sub(1);
    let range = Range::new(Position::new(line, col), Position::new(line, col + len));
    DocumentSymbol {
        name: name.clone(),
        detail: Some(format!("class {name}")),
        kind: SymbolKind::CLASS,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: class_children(a, file, c),
    }
}

/// Methods and fields of a class as document children, positioned via the
/// symbol index (entries whose scope is the class).  Falls back to the model
/// when the index has nothing for the class.
fn class_children(a: &Analysis, file: &str, c: &ClassDef) -> Option<Vec<DocumentSymbol>> {
    let scope = clean_name(&c.name);
    let kids: Vec<DocumentSymbol> = a
        .index
        .decls_in_file(file)
        .iter()
        .filter(|d| {
            d.is_decl
                && d.scope.as_deref() == Some(scope)
                && matches!(d.kind, SymKind::Function | SymKind::Task | SymKind::Var)
        })
        .map(child_symbol_from_entry)
        .collect();
    if !kids.is_empty() {
        return Some(kids);
    }
    class_children_fallback(c, file)
}

/// Model-based fallback for [`class_children`]: methods (with their
/// declaration positions) and fields (with their declaration positions).
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn class_children_fallback(c: &ClassDef, file: &str) -> Option<Vec<DocumentSymbol>> {
    if c.file.as_deref() != Some(file) {
        return None;
    }
    let mut kids = Vec::new();
    for m in &c.methods {
        if m.file.as_deref() == Some(file) {
            kids.push(func_symbol(m));
        }
    }
    for f in &c.fields {
        let name = clean_name(&f.name).to_owned();
        let len = lsp_name_len(&name);
        let line = f.line.saturating_sub(1);
        let col = f.col.saturating_sub(1);
        let range = Range::new(Position::new(line, col), Position::new(line, col + len));
        kids.push(DocumentSymbol {
            name,
            detail: Some(format_class_field(f)),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        });
    }
    (!kids.is_empty()).then_some(kids)
}

/// Ports, signals, parameters, and INSTANCES of the module, positioned via
/// the symbol index (entries whose scope is the module) with a fallback to
/// the v1 first-instance approach when the index has nothing for the module.
///
/// Port/net/var/param children carry a type-only `detail` (see
/// [`child_type_detail`]); instance children (see
/// [`module_instance_children`]) are appended so hierarchy consumers can
/// resolve instance→module-type edges.
fn module_children(a: &Analysis, file: &str, def_name: &str) -> Option<Vec<DocumentSymbol>> {
    let scope = clean_name(def_name);
    let mut kids: Vec<DocumentSymbol> = a
        .index
        .decls_in_file(file)
        .iter()
        .filter(|d| {
            d.is_decl
                && d.scope.as_deref() == Some(scope)
                && matches!(
                    d.kind,
                    SymKind::Port | SymKind::Net | SymKind::Var | SymKind::Param
                )
        })
        .map(|d| {
            let mut sym = child_symbol_from_entry(d);
            sym.detail = child_type_detail(a, d);
            sym
        })
        .collect();
    if kids.is_empty() {
        kids = module_children_fallback(a, file, def_name).unwrap_or_default();
    }
    let instances = module_instance_children(a, file, scope);
    if kids.is_empty() && instances.is_empty() {
        return None;
    }
    kids.extend(instances);
    Some(kids)
}

/// SystemVerilog plain identifier, used to spot the declared NAME at the tail
/// of a declaration snippet.
fn is_sv_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Type words that mean "nothing usable was captured" — rendering them as a
/// detail would be a wrong guess.
const UNKNOWN_TYPE_WORDS: &[&str] = &["var", "other", "unknown"];

/// Strips the trailing identifier (the declared NAME) from a declaration
/// snippet (`input logic [7:0] q` → `input logic [7:0]`), leaving the
/// source-declared type text.  `None` when nothing safe can be said:
/// single-token snippets, non-identifier tails, or degenerate type words all
/// degrade instead of guessing.
fn strip_trailing_name(snippet: &str) -> Option<String> {
    let tokens: Vec<&str> = snippet.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }
    let last = tokens[tokens.len() - 1];
    if !is_sv_identifier(last) {
        return None;
    }
    let ty = tokens[..tokens.len() - 1].join(" ");
    if ty.is_empty() || UNKNOWN_TYPE_WORDS.contains(&ty.to_lowercase().as_str()) {
        return None;
    }
    Some(ty)
}

/// Type word(s) of the legacy value-rich model shape
/// `parameter NAME: TYPE[ = VALUE]`; `None` when unknown or degenerate.
fn param_colon_type(detail: &str) -> Option<String> {
    let rest = detail.split_once(':')?.1.trim();
    let ty = match rest.split_once(" = ") {
        Some((ty, _)) => ty.trim(),
        None => rest,
    };
    if ty.is_empty() || UNKNOWN_TYPE_WORDS.contains(&ty.to_lowercase().as_str()) {
        return None;
    }
    Some(ty.to_owned())
}

/// Type-only detail for a parameter/localparam child: the keyword from the
/// model detail plus the declared type word when it is known (`parameter`,
/// `localparam int`).
fn param_type_detail(model_detail: Option<&str>, decl_snippet: Option<&str>) -> Option<String> {
    let detail = model_detail?;
    let kw = detail.split_whitespace().next()?;
    if kw != "parameter" && kw != "localparam" {
        return None;
    }
    match decl_snippet
        .and_then(strip_trailing_name)
        .or_else(|| param_colon_type(detail))
    {
        Some(ty) => Some(format!("{kw} {ty}")),
        None => Some(kw.to_owned()),
    }
}

/// Document-symbol child detail carrying the SOURCE-DECLARED type (no name,
/// no resolved value): ports render `[direction] type`, nets/vars render
/// `type`, parameters render `parameter|localparam [type]`.  Prefers the
/// position-accurate UHDM declaration snippet ([`Analysis::decl_details`])
/// over the name-based model detail; missing information degrades to `None`
/// rather than a guess.
fn child_type_detail(a: &Analysis, d: &SymEntry) -> Option<String> {
    let snippet = a
        .decl_details
        .get(&(d.file.clone(), d.line + 1, d.col + 1))
        .map(String::as_str)
        .or(d.detail.as_deref());
    match d.kind {
        SymKind::Param => param_type_detail(d.detail.as_deref(), snippet),
        _ => snippet.and_then(strip_trailing_name),
    }
}

/// Instantiation-site children of the module definition named `def_scope` in
/// `file`, served from the symbol index: Instance declarations whose parent
/// instance elaborates an instance OF `def_scope`.  Positions are the index's
/// refined identifier positions; each `detail` carries the instantiated
/// module TYPE so hierarchy consumers can resolve edges.  Instances nested
/// inside generate blocks are indexed under their gen scope and are not
/// attached here (consistent with the instance tree the index is built from).
fn module_instance_children(a: &Analysis, file: &str, def_scope: &str) -> Vec<DocumentSymbol> {
    // Parent-instance full name (exactly what entry scopes record) → clean
    // instantiated definition name.
    let parent_defs: HashMap<&str, &str> = all_instances(&a.model.top_instances)
        .into_iter()
        .map(|i| (i.full_name.as_str(), clean_name(&i.def_name)))
        .collect();
    let instance_types: HashMap<(u32, u32), &String> = a
        .index
        .decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.is_decl && d.kind == SymKind::Instance && d.file == file)
        .filter_map(|(idx, d)| {
            a.index
                .instance_def
                .get(&idx)
                .map(|ty| ((d.line, d.col), ty))
        })
        .collect();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut kids = Vec::new();
    for d in a.index.decls_in_file(file) {
        if !d.is_decl || d.kind != SymKind::Instance {
            continue;
        }
        // Top instances have no enclosing module definition in any file.
        let Some(parent_scope) = d.scope.as_deref() else {
            continue;
        };
        if parent_defs.get(parent_scope).copied() != Some(def_scope) {
            continue;
        }
        // Interface views / duplicated clones can share an instantiation site.
        if !seen.insert((d.line, d.col)) {
            continue;
        }
        let ty = instance_types.get(&(d.line, d.col)).copied();
        kids.push(instance_child_symbol(d, ty.map(String::as_str)));
    }
    kids
}

/// A leaf `DocumentSymbol` for one instantiation site: Object kind, ranged
/// over the instance name, `detail` = instantiated module type.
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn instance_child_symbol(d: &SymEntry, type_text: Option<&str>) -> DocumentSymbol {
    let len = lsp_name_len(&d.name);
    let range = Range::new(
        Position::new(d.line, d.col),
        Position::new(d.line, d.col + len),
    );
    DocumentSymbol {
        name: d.name.clone(),
        detail: type_text.map(str::to_owned),
        kind: SymbolKind::OBJECT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

/// Build a `DocumentSymbol` for a declaration entry.
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn child_symbol_from_entry(d: &SymEntry) -> DocumentSymbol {
    let len = lsp_name_len(&d.name);
    let range = Range::new(
        Position::new(d.line, d.col),
        Position::new(d.line, d.col + len),
    );
    DocumentSymbol {
        name: d.name.clone(),
        detail: d.detail.clone(),
        kind: sym_kind_to_lsp(d.kind),
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

/// v1 fallback: ports, signals, and parameters of the first instance whose
/// `def_name` matches the module, positioned via the token index in the
/// module's file.  Details carry model-derived type-only text (no names).
fn module_children_fallback(
    a: &Analysis,
    file: &str,
    def_name: &str,
) -> Option<Vec<DocumentSymbol>> {
    let inst = all_instances(&a.model.top_instances)
        .into_iter()
        .find(|i| clean_name(&i.def_name) == clean_name(def_name))?;
    let mut kids = Vec::new();
    for port in &inst.ports {
        kids.push(
            child_symbol(a, file, &port.name, SymbolKind::PROPERTY).map(|mut s| {
                s.detail = port_type_only(port);
                s
            }),
        );
    }
    for sig in &inst.signals {
        kids.push(
            child_symbol(a, file, &sig.name, SymbolKind::VARIABLE).map(|mut s| {
                s.detail = signal_type_only(sig);
                s
            }),
        );
    }
    for p in &inst.params {
        kids.push(
            child_symbol(a, file, &p.name, SymbolKind::CONSTANT).map(|mut s| {
                s.detail = param_type_only(p);
                s
            }),
        );
    }
    for gs in &inst.gen_scopes {
        for p in &gs.params {
            kids.push(
                child_symbol(a, file, &p.name, SymbolKind::CONSTANT).map(|mut s| {
                    s.detail = param_type_only(p);
                    s
                }),
            );
        }
    }
    Some(kids.into_iter().flatten().collect())
}

/// Type-only port text (`input logic [7:0]`); direction alone when the type
/// is unknown, `None` when neither is known.
fn port_type_only(p: &PortModel) -> Option<String> {
    let dir = match p.direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "",
    };
    let ty = p.ty.render();
    let ty = (ty != "other").then_some(ty);
    match (dir.is_empty(), ty) {
        (true, None) => None,
        (true, Some(ty)) => Some(ty),
        (false, None) => Some(dir.to_owned()),
        (false, Some(ty)) => Some(format!("{dir} {ty}")),
    }
}

/// Type-only signal text (`logic [7:0]`, `array logic [3:0]`); `None` when
/// nothing usable was captured.
fn signal_type_only(s: &SignalModel) -> Option<String> {
    let ty = s.ty.render();
    if ty == "other" {
        return None;
    }
    Some(if s.kind == "array" {
        format!("array {ty}")
    } else {
        ty
    })
}

/// Type-only parameter text (`parameter`, `localparam int`).
fn param_type_only(p: &ParamModel) -> Option<String> {
    let kw = if p.local { "localparam" } else { "parameter" };
    let ty = p.ty.render();
    if ty == "other" {
        Some(kw.to_owned())
    } else {
        Some(format!("{kw} {ty}"))
    }
}

/// Build a `DocumentSymbol` for `name` at its (preferred declaration) token
/// position in `file`; `None` when no such token exists.
#[allow(deprecated)] // `DocumentSymbol::deprecated` is deprecated in lsp-types
fn child_symbol(a: &Analysis, file: &str, name: &str, kind: SymbolKind) -> Option<DocumentSymbol> {
    let node = file_tokens(a, file)?
        .nodes
        .iter()
        .filter(|n| n.name.as_deref() == Some(name))
        .min_by_key(|n| {
            let decl_rank = if is_declaration_vpi_type(n.vpi_type) {
                0
            } else {
                1
            };
            (decl_rank, n.line, n.col)
        })?;
    let line = node.line.saturating_sub(1);
    let col = node.col.saturating_sub(1);
    let len = lsp_name_len(name);
    let range = Range::new(Position::new(line, col), Position::new(line, col + len));
    Some(DocumentSymbol {
        name: name.to_owned(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    })
}

// ── Completion ────────────────────────────────────────────────────────────────

/// Completion items for the position, filtered by the identifier prefix before
/// the cursor on `line_text`.
///
/// After a `pkg::` scope prefix the candidates are the named package's items
/// only (parameters as constants, enum constants as enum members); after a
/// `Class::` prefix the candidates are the named class's members (methods as
/// functions, fields as variables).  Otherwise the candidates are: module
/// names (kind Module), package names (kind Module), instance names (kind
/// Keyword), function/task names from the model (kind Function), every indexed
/// declaration (functions, tasks, classes, typedefs, enum constants, ports,
/// signals, parameters — kind mapped from the symbol kind), and a curated list
/// of SystemVerilog keywords.
pub fn completion_at(
    a: &Analysis,
    _file: &str,
    _line: u32,
    col: u32,
    line_text: &str,
) -> Vec<CompletionItem> {
    // After a `pkg::` or `Class::` scope prefix, offer only the named
    // package's/class's members.
    if let Some((scope, item_prefix)) = package_scope_prefix(line_text, col) {
        // Package items (parameters as constants, enum constants as members).
        if let Some(p) = a
            .model
            .packages
            .iter()
            .find(|p| clean_name(&p.name) == clean_name(&scope))
        {
            let mut items = Vec::new();
            for param in &p.params {
                if param.name.starts_with(&item_prefix) {
                    items.push(CompletionItem {
                        label: param.name.clone(),
                        kind: Some(CompletionItemKind::CONSTANT),
                        ..Default::default()
                    });
                }
            }
            for ec in &p.enum_consts {
                if ec.name.starts_with(&item_prefix) {
                    items.push(CompletionItem {
                        label: ec.name.clone(),
                        kind: Some(CompletionItemKind::ENUM_MEMBER),
                        ..Default::default()
                    });
                }
            }
            items.dedup_by(|a, b| a.label == b.label);
            items.sort_by(|a, b| a.label.cmp(&b.label));
            return items;
        }
        // Class members (methods as functions, fields as variables).
        if let Some(c) = a
            .model
            .classes
            .iter()
            .find(|c| clean_name(&c.name) == clean_name(&scope))
        {
            let mut items = Vec::new();
            for m in &c.methods {
                let label = clean_name(&m.name).to_owned();
                if label.starts_with(&item_prefix) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::FUNCTION),
                        ..Default::default()
                    });
                }
            }
            for f in &c.fields {
                let label = clean_name(&f.name).to_owned();
                if label.starts_with(&item_prefix) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..Default::default()
                    });
                }
            }
            items.dedup_by(|a, b| a.label == b.label);
            items.sort_by(|a, b| a.label.cmp(&b.label));
            return items;
        }
        // Unknown scope: nothing to offer.
        return Vec::new();
    }
    let prefix = prefix_before_cursor(line_text, col);
    let mut items = Vec::new();
    for m in &a.model.modules {
        items.push(CompletionItem {
            label: clean_name(&m.name).to_owned(),
            kind: Some(CompletionItemKind::MODULE),
            ..Default::default()
        });
    }
    for p in &a.model.packages {
        items.push(CompletionItem {
            label: clean_name(&p.name).to_owned(),
            kind: Some(CompletionItemKind::MODULE),
            ..Default::default()
        });
    }
    for inst in all_instances(&a.model.top_instances) {
        items.push(CompletionItem {
            label: clean_name(&inst.name).to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }
    // Function/task names from the model (per-instance clones deduped by name).
    let mut seen_funcs: HashSet<String> = HashSet::new();
    for inst in all_instances(&a.model.top_instances) {
        for f in &inst.funcs {
            let label = clean_name(&f.name).to_owned();
            if seen_funcs.insert(label.clone()) {
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::FUNCTION),
                    ..Default::default()
                });
            }
        }
    }
    // Index-backed declarations (deduplicated against the model candidates).
    let mut seen: HashSet<String> = items.iter().map(|i| i.label.clone()).collect();
    for d in &a.index.decls {
        if seen.insert(d.name.clone()) {
            items.push(CompletionItem {
                label: d.name.clone(),
                kind: Some(completion_kind_for(d.kind)),
                ..Default::default()
            });
        }
    }
    for kw in KEYWORDS {
        items.push(CompletionItem {
            label: (*kw).to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }
    items.retain(|i| i.label.starts_with(&prefix));
    items.dedup_by(|a, b| a.label == b.label);
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

/// Map a model [`SymKind`] to a completion-item kind.
fn completion_kind_for(kind: SymKind) -> CompletionItemKind {
    match kind {
        SymKind::Module | SymKind::Package | SymKind::Program => CompletionItemKind::MODULE,
        SymKind::Interface => CompletionItemKind::INTERFACE,
        SymKind::Class => CompletionItemKind::CLASS,
        SymKind::Instance | SymKind::GenScope => CompletionItemKind::KEYWORD,
        SymKind::Port => CompletionItemKind::FIELD,
        SymKind::Net | SymKind::Var => CompletionItemKind::VARIABLE,
        SymKind::Param => CompletionItemKind::CONSTANT,
        SymKind::EnumConst => CompletionItemKind::ENUM_MEMBER,
        SymKind::Typedef => CompletionItemKind::TYPE_PARAMETER,
        SymKind::Function | SymKind::Task => CompletionItemKind::FUNCTION,
    }
}

const KEYWORDS: &[&str] = &[
    "module",
    "endmodule",
    "input",
    "output",
    "inout",
    "logic",
    "wire",
    "reg",
    "assign",
    "always",
    "always_ff",
    "always_comb",
    "initial",
    "begin",
    "end",
    "if",
    "else",
    "case",
    "endcase",
    "for",
    "while",
    "parameter",
    "localparam",
    "typedef",
    "struct",
    "enum",
    "interface",
    "endinterface",
    "package",
    "endpackage",
    "class",
    "endclass",
    "function",
    "endfunction",
    "task",
    "endtask",
    "genvar",
    "generate",
    "endgenerate",
    "posedge",
    "negedge",
    "or",
    "and",
    "not",
];

/// The identifier prefix immediately before the cursor: the longest trailing
/// run of alphanumeric/`_` characters on the line before UTF-16 offset `col`.
fn prefix_before_cursor(line: &str, col: u32) -> String {
    let col = utf16_byte_offset(line, col);
    let before = line.get(..col).unwrap_or(line);
    before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Detect a `pkg::`/`Class::` scope prefix immediately before the cursor: the
/// scope name and the (possibly partial) item identifier after `::`.  `None`
/// when there is no `::` with an identifier before it on the line.
fn package_scope_prefix(line: &str, col: u32) -> Option<(String, String)> {
    let col = utf16_byte_offset(line, col);
    let before = line.get(..col).unwrap_or(line);
    let pkg_end = before.rfind("::")?;
    let item_start = pkg_end + 2;
    let pkg: String = before[..pkg_end]
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if pkg.is_empty() {
        return None;
    }
    let item: String = before[item_start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    Some((pkg, item))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Length of an identifier in the coordinate unit required by LSP: UTF-16
/// code units, not Unicode scalar values or UTF-8 bytes.
fn lsp_name_len(name: &str) -> u32 {
    name.encode_utf16().count() as u32
}

/// Convert a 0-based LSP UTF-16 column into a UTF-8 byte offset at a char
/// boundary.  A position in the middle of a supplementary character is
/// rounded to that character's start, which is the only safe slice boundary.
fn utf16_byte_offset(line: &str, col: u32) -> usize {
    let target = col as usize;
    let mut units = 0usize;
    for (offset, ch) in line.char_indices() {
        if units >= target {
            return offset;
        }
        let next = units + ch.len_utf16();
        if target < next {
            return offset;
        }
        units = next;
    }
    line.len()
}

/// Strip a Surelog library prefix (`lib@name` → `name`) from a design name.
///
/// Elaborated module/package names arrive as `work@param_top` etc.; SV
/// identifiers cannot contain `@`, so the prefix is unambiguous to strip for
/// display and for comparing against source identifiers.
fn clean_name(name: &str) -> &str {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => name,
    }
}

/// `true` when `file` is Surelog's virtual builtin file (`<cwd>/builtin.sv`).
///
/// Surelog parses the builtin classes (mailbox/process/semaphore) from a
/// string and reports them under this path, which never exists on disk;
/// declarations there are skipped so goto-definition cannot dead-end.
fn builtin_file(file: &str) -> bool {
    Path::new(file).file_name().and_then(|n| n.to_str()) == Some("builtin.sv")
}

/// The token list for `file`, by exact path with a filename fallback.
fn file_tokens<'a>(a: &'a Analysis, file: &str) -> Option<&'a FileTokens> {
    let file_name = Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    a.tokens.iter().find(|ft| ft.path == file).or_else(|| {
        if file_name.is_empty() {
            None
        } else {
            a.tokens.iter().find(|ft| ft.path.ends_with(file_name))
        }
    })
}

/// The named token at 0-based `(line, col)`: prefer a token whose name range
/// covers the column; otherwise the nearest token on the same line (longest
/// name wins ties).
fn token_at<'a>(a: &'a Analysis, file: &str, line: u32, col: u32) -> Option<&'a VObjectInfo> {
    let line1 = line + 1;
    let col1 = col + 1;
    let ft = file_tokens(a, file)?;
    let mut candidates: Vec<&VObjectInfo> = ft
        .nodes
        .iter()
        .filter(|n| n.line == line1 && n.name.as_deref().is_some_and(|s| !s.is_empty()))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    if let Some(found) = candidates.iter().find(|n| {
        let len = n.name.as_deref().map_or(0, lsp_name_len);
        col1 >= n.col && col1 < n.col.saturating_add(len)
    }) {
        return Some(found);
    }
    candidates.sort_by(|x, y| {
        let dx = (i64::from(x.col) - i64::from(col1)).abs();
        let dy = (i64::from(y.col) - i64::from(col1)).abs();
        dx.cmp(&dy).then_with(|| {
            let lx = x.name.as_deref().map_or(0, |s| lsp_name_len(s) as usize);
            let ly = y.name.as_deref().map_or(0, |s| lsp_name_len(s) as usize);
            ly.cmp(&lx)
        })
    });
    candidates.into_iter().next()
}

/// All instances in the tree, depth-first.
fn all_instances(insts: &[InstanceModel]) -> Vec<&InstanceModel> {
    let mut out = Vec::new();
    let mut stack: Vec<&InstanceModel> = insts.iter().collect();
    while let Some(i) = stack.pop() {
        out.push(i);
        stack.extend(i.children.iter());
    }
    out
}

/// The 1-based position of the token at the exact 0-based `(line, col)` when
/// that token's VPI type is a named-connection LABEL flavor (the dedicated
/// `TOKEN_*_CONN_LABEL` synthetic types, plus the historical
/// `vpiFunction`/`vpiTask` port-label and `vpiParameter` override-label types);
/// `None` otherwise.
///
/// Used to keep [`nearest_declaration`] from matching a connection label's
/// OWN token: an unresolvable label must yield no definition, not a no-op
/// jump onto itself.
fn skip_self_label(a: &Analysis, file: &str, line: u32, col: u32) -> Option<(u32, u32)> {
    use llg::ffi::vpi;
    let ft = file_tokens(a, file)?;
    let node = ft
        .nodes
        .iter()
        .find(|n| n.line == line + 1 && n.col == col + 1)?;
    let is_label_flavor = matches!(
        node.vpi_type,
        vpi::TOKEN_PORT_CONN_LABEL
            | vpi::TOKEN_PARAM_CONN_LABEL
            | vpi::vpiFunction
            | vpi::vpiTask
            | vpi::vpiParameter
    );
    is_label_flavor.then_some((node.line, node.col))
}

/// The same-file port/signal/parameter declaration of `name` nearest to
/// 0-based `line`, from tokens whose VPI type is a declaration type.
///
/// `skip` excludes one 1-based position — the clicked connection-label token
/// itself (see [`skip_self_label`]) — so a dropped/unbound label cannot
/// self-match at distance zero.
fn nearest_declaration(
    a: &Analysis,
    file: &str,
    name: &str,
    line: u32,
    skip: Option<(u32, u32)>,
) -> Option<(u32, u32)> {
    let ft = file_tokens(a, file)?;
    ft.nodes
        .iter()
        .filter(|n| {
            n.name.as_deref() == Some(name)
                && is_declaration_vpi_type(n.vpi_type)
                && Some((n.line, n.col)) != skip
        })
        .min_by_key(|n| (n.line.abs_diff(line + 1), n.col))
        .map(|n| (n.line, n.col))
}

/// VPI object types that represent declarations (rather than references).
fn is_declaration_vpi_type(t: i32) -> bool {
    use llg::ffi::vpi;
    matches!(
        t,
        vpi::vpiModule
            | vpi::vpiPort
            | vpi::vpiPortBit
            | vpi::vpiNet
            | vpi::vpiNetBit
            | vpi::vpiReg
            | vpi::vpiRegBit
            | vpi::vpiIntegerVar
            | vpi::vpiRealVar
            | vpi::vpiTimeVar
            | vpi::vpiParameter
            | vpi::vpiSpecParam
            | vpi::vpiLogicVar
            | vpi::vpiFunction
            | vpi::vpiTask
            | vpi::uhdmpackage
            | vpi::uhdmclass_defn
            | vpi::uhdmenum_const
            | vpi::uhdmlogic_net
            | vpi::uhdmnet
            | vpi::uhdmlogic_var
            | vpi::uhdmint_var
            | vpi::uhdmreal_var
            | vpi::uhdmbit_var
            | vpi::uhdmbyte_var
            | vpi::uhdmshort_int_var
            | vpi::uhdmlong_int_var
            | vpi::uhdmparameter
            | vpi::uhdmfunction
            | vpi::uhdmtask
            | vpi::uhdminterface_inst
            | vpi::TOKEN_PORT_INPUT
            | vpi::TOKEN_PORT_OUTPUT
            | vpi::TOKEN_PORT_INOUT
            | vpi::TOKEN_TYPEDEF_NAME
    )
}

// ── Location construction ─────────────────────────────────────────────────────

fn location(file: &str, line1: u32, col1: u32, len: usize) -> Location {
    let uri = Url::from_file_path(file)
        .unwrap_or_else(|_| Url::parse("untitled:llg").expect("static URL is valid"));
    let line = line1.saturating_sub(1);
    let col = col1.saturating_sub(1);
    let len = len as u32;
    Location {
        uri,
        range: Range::new(Position::new(line, col), Position::new(line, col + len)),
    }
}

/// LSP location for an index entry (already 0-based).
fn entry_location(e: &SymEntry) -> Location {
    let uri = Url::from_file_path(&e.file)
        .unwrap_or_else(|_| Url::parse("untitled:llg").expect("static URL is valid"));
    let len = lsp_name_len(&e.name);
    Location {
        uri,
        range: Range::new(
            Position::new(e.line, e.col),
            Position::new(e.line, e.col + len),
        ),
    }
}

fn module_def_location(a: &Analysis, m: &ModuleDef) -> Option<Location> {
    // Prefer the token-refined position from the index (points at the module
    // *name*, not the `module` keyword).
    if let Some(d) = a.index.decls.iter().find(|d| {
        d.kind == SymKind::Module
            && d.name == clean_name(&m.name)
            && Some(d.file.as_str()) == m.file.as_deref()
    }) {
        return Some(entry_location(d));
    }
    let file = m.file.as_deref()?;
    let len = lsp_name_len(clean_name(&m.name)) as usize;
    Some(location(file, m.line, m.col, len))
}

fn package_location(p: &PackageDef) -> Option<Location> {
    let file = p.file.as_deref()?;
    let len = lsp_name_len(clean_name(&p.name)) as usize;
    Some(location(file, p.line, p.col, len))
}

fn instance_location(i: &InstanceModel) -> Option<Location> {
    let file = i.file.as_deref()?;
    Some(location(
        file,
        i.line,
        i.col,
        lsp_name_len(&i.name) as usize,
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use llg::core::elab::{Val, Value};
    use llg::core::model::{GenScopeModel, TypeInfo};

    /// Serializes the Surelog-touching tests in this binary: Surelog's global
    /// C++ singletons are not thread-safe and it writes `slpp_all/` into the
    /// CWD, so the compile-based tests must not interleave.
    static SURELOG_LOCK: Mutex<()> = Mutex::new(());

    /// Guards for tests that run real analyses.  Analyses CREATE the process
    /// shadow base, park the process CWD inside it and let Surelog write
    /// there — so they must be serialized against other Surelog runs AND
    /// against the shadow staging/cleanup tests.  Lock order is fixed:
    /// SURELOG first, then TEST_PROCESS_SHADOW_LOCK (never reversed).
    fn analysis_guards() -> (
        std::sync::MutexGuard<'static, ()>,
        std::sync::MutexGuard<'static, ()>,
    ) {
        let surelog = SURELOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let shadow = crate::features::TEST_PROCESS_SHADOW_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (surelog, shadow)
    }

    #[test]
    fn graph_assembly_indexes_keep_large_declaration_and_instance_sets_keyed() {
        let mut indexes = GraphAssemblyIndexes::new(1);
        let mut definition = ModuleGraphDefinition {
            id: "m|/tmp/m.sv|1|1".to_owned(),
            name: "m".to_owned(),
            file: Some("/tmp/m.sv".to_owned()),
            line: 1,
            col: 1,
            end_line: 20_000,
            end_col: 1,
            ports: Vec::new(),
            params: Vec::new(),
            signals: Vec::new(),
            children: Vec::new(),
            generated_scopes: Vec::new(),
        };

        for index in 0..4_096 {
            assert!(indexes.ports[0].insert((format!("p{index}"), None)));
            assert!(indexes.params[0].insert((format!("P{index}"), None)));
            assert!(indexes.signals[0].insert((format!("s{index}"), None)));
            let instance = ModuleGraphInstance {
                name: format!("u{index}"),
                module_type: "child".to_owned(),
                file: definition.file.clone(),
                line: index + 1,
                col: 1,
            };
            graph_push_instance(
                &mut definition.children,
                &mut indexes.children[0],
                instance.clone(),
            );
            graph_push_instance(&mut definition.children, &mut indexes.children[0], instance);
        }

        let path = vec![
            ModuleGraphGenerateScope {
                name: "g_outer".to_owned(),
                file: definition.file.clone(),
                line: 2,
                col: 1,
                children: Vec::new(),
                nested: Vec::new(),
            },
            ModuleGraphGenerateScope {
                name: "g_inner".to_owned(),
                file: definition.file.clone(),
                line: 3,
                col: 1,
                children: Vec::new(),
                nested: Vec::new(),
            },
        ];
        for index in 0..4_096 {
            let instance = ModuleGraphInstance {
                name: format!("gu{index}"),
                module_type: "generated_child".to_owned(),
                file: definition.file.clone(),
                line: index + 1,
                col: 2,
            };
            graph_push_generated_instance(
                0,
                &mut definition,
                &path,
                instance.clone(),
                &mut indexes,
            );
            graph_push_generated_instance(0, &mut definition, &path, instance, &mut indexes);
        }

        assert_eq!(indexes.ports[0].len(), 4_096);
        assert_eq!(indexes.params[0].len(), 4_096);
        assert_eq!(indexes.signals[0].len(), 4_096);
        assert_eq!(definition.children.len(), 4_096);
        assert_eq!(
            definition.children.first().map(|child| child.name.as_str()),
            Some("u0")
        );
        assert_eq!(
            definition.children.last().map(|child| child.name.as_str()),
            Some("u4095")
        );
        assert_eq!(definition.generated_scopes.len(), 1);
        assert_eq!(definition.generated_scopes[0].nested.len(), 1);
        assert_eq!(
            definition.generated_scopes[0].nested[0].children.len(),
            4_096
        );
    }

    #[test]
    fn surelog_invocation_log_details_are_bounded_and_redacted() {
        let argv = vec![
            "llg".to_owned(),
            "-D".to_owned(),
            "SECRET=separate-value".to_owned(),
            "-P".to_owned(),
            "WIDTH=999999".to_owned(),
            "-DSECRET=do-not-log-this-value".to_owned(),
            format!("-I{}", "include/".to_owned() + &"nested/".repeat(64)),
            "top.sv".to_owned(),
        ];
        let (representation, fingerprint) = surelog_argv_log_details(&argv);

        assert!(representation.len() <= SURELOG_LOG_ARGV_MAX);
        assert!(representation.contains("-DSECRET=<redacted>"));
        assert!(!representation.contains("do-not-log-this-value"));
        assert!(!representation.contains("separate-value"));
        assert!(!representation.contains("999999"));
        assert!(representation.contains("-I"));
        assert_eq!(fingerprint.len(), 16);
        assert!(fingerprint
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
    }

    /// Restores the process CWD and removes the temp dir even when the body
    /// panics, so a failing test cannot strand other tests in a deleted CWD.
    struct TempDirGuard {
        dir: std::path::PathBuf,
        orig: std::path::PathBuf,
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.orig);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Hand-built model + tokens for /x/top.sv:
    /// - module `m` (line 1..3) with input port `clk` and parameter `W` = 32'sd8,
    /// - instance `top.u0` of type `m`,
    /// - package `p`,
    /// - function `add` (line 4) and task `run` (line 5) on `u0`,
    /// - tokens for `clk`, `u0`, `W`, `add`, and `run`.
    ///
    /// Returned as parts so tests can assemble the analysis with or without
    /// synthetic UHDM bindings.
    fn sample_parts() -> (DesignModel, Vec<FileTokens>) {
        let module = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 3,
            end_col: 12,
        };
        let port = PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        };
        let param = ParamModel {
            name: "W".to_owned(),
            value: Some(Val::Bits(Value::from_u64(8, 32, true))),
            ty: TypeInfo {
                kind: "int".to_owned(),
                width: None,
                signed: true,
                type_name: None,
            },
            local: false,
        };
        let int_ty = || TypeInfo {
            kind: "int".to_owned(),
            width: None,
            signed: true,
            type_name: None,
        };
        let add = FuncDef {
            name: "add".to_owned(),
            is_task: false,
            automatic: true,
            file: Some("/x/top.sv".to_owned()),
            line: 4,
            col: 8,
            ret: Some(int_ty()),
            args: vec![
                FuncArgDef {
                    name: "a".to_owned(),
                    direction: Direction::Input,
                    ty: int_ty(),
                    has_default: false,
                },
                FuncArgDef {
                    name: "b".to_owned(),
                    direction: Direction::Input,
                    ty: int_ty(),
                    has_default: true,
                },
            ],
            scope: "top.u0".to_owned(),
        };
        let run = FuncDef {
            name: "run".to_owned(),
            is_task: true,
            automatic: false,
            file: Some("/x/top.sv".to_owned()),
            line: 5,
            col: 8,
            ret: None,
            args: vec![FuncArgDef {
                name: "n".to_owned(),
                direction: Direction::Input,
                ty: int_ty(),
                has_default: false,
            }],
            scope: "top.u0".to_owned(),
        };
        let inst = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 1,
            col: 20,
            ports: vec![port],
            signals: Vec::new(),
            params: vec![param],
            gen_scopes: Vec::new(),
            funcs: vec![add, run],
            children: Vec::new(),
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![inst],
            modules: vec![module],
            packages: vec![PackageDef {
                name: "p".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 5,
                col: 1,
                params: Vec::new(),
                enum_consts: Vec::new(),
            }],
            classes: Vec::new(),
        };
        let tokens = vec![FileTokens {
            path: "/x/top.sv".to_owned(),
            nodes: vec![
                VObjectInfo {
                    line: 1,
                    col: 8,
                    end_line: 1,
                    end_col: 9,
                    vpi_type: llg::ffi::vpi::vpiModule,
                    name: Some("m".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 1,
                    col: 5,
                    end_line: 1,
                    end_col: 8,
                    vpi_type: llg::ffi::vpi::TOKEN_PORT_INPUT,
                    name: Some("clk".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 1,
                    col: 20,
                    end_line: 1,
                    end_col: 22,
                    vpi_type: llg::ffi::vpi::uhdmmodule_inst,
                    name: Some("u0".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                // Real pipelines emit the parameter declaration site three
                // times (VPI walker + UHDM iteration + parse tree); the index
                // uses the multiplicity to separate decls from references.
                VObjectInfo {
                    line: 2,
                    col: 5,
                    end_line: 2,
                    end_col: 6,
                    vpi_type: llg::ffi::vpi::uhdmparameter,
                    name: Some("W".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 2,
                    col: 5,
                    end_line: 2,
                    end_col: 6,
                    vpi_type: llg::ffi::vpi::uhdmparameter,
                    name: Some("W".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 2,
                    col: 5,
                    end_line: 2,
                    end_col: 6,
                    vpi_type: llg::ffi::vpi::uhdmparameter,
                    name: Some("W".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 4,
                    col: 8,
                    end_line: 4,
                    end_col: 11,
                    vpi_type: llg::ffi::vpi::vpiFunction,
                    name: Some("add".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 5,
                    col: 8,
                    end_line: 5,
                    end_col: 11,
                    vpi_type: llg::ffi::vpi::vpiTask,
                    name: Some("run".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
            ],
        }];
        (model, tokens)
    }

    /// [`sample_parts`] assembled through [`Analysis::new`].
    fn sample_analysis() -> Analysis {
        let (model, tokens) = sample_parts();
        Analysis::new(Vec::new(), model, tokens, Vec::new())
    }

    /// [`sample_parts`] assembled with synthetic UHDM reference bindings.
    fn sample_analysis_with_bindings(bindings: RefBindings) -> Analysis {
        let (model, tokens) = sample_parts();
        Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            model,
            tokens,
            Vec::new(),
            bindings,
            ConnectionInputs::default(),
        )
    }

    #[test]
    fn navigation_ranges_use_utf16_after_supplementary_text() {
        let dir = std::env::temp_dir().join(format!(
            "llg-features-utf16-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temporary source directory");
        let _guard = TempDirGuard {
            dir: dir.clone(),
            orig: std::env::current_dir().expect("current directory"),
        };
        let path = dir.join("unicode.sv");
        let file = path.to_string_lossy().into_owned();
        let source = "module top;\nlogic /* 😀 */ data;\nassign data = data;\nendmodule\n";
        std::fs::write(&path, source).expect("write temporary source");

        let ty = TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![InstanceModel {
                name: "top".to_owned(),
                def_name: "top".to_owned(),
                full_name: "top".to_owned(),
                file: Some(file.clone()),
                line: 1,
                col: 1,
                ports: Vec::new(),
                signals: vec![SignalModel {
                    name: "data".to_owned(),
                    kind: "wire".to_owned(),
                    ty: ty.clone(),
                }],
                params: Vec::new(),
                gen_scopes: Vec::new(),
                funcs: Vec::new(),
                children: Vec::new(),
            }],
            modules: vec![ModuleDef {
                name: "top".to_owned(),
                file: Some(file.clone()),
                line: 1,
                col: 1,
                end_line: 4,
                end_col: 1,
            }],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let token = |line: u32, col: u32, vpi_type: i32| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + 4,
            vpi_type,
            name: Some("data".to_owned()),
            file: file.clone(),
        };
        let tokens = vec![FileTokens {
            path: file.clone(),
            nodes: vec![
                VObjectInfo {
                    line: 1,
                    col: 8,
                    end_line: 1,
                    end_col: 12,
                    vpi_type: llg::ffi::vpi::vpiModule,
                    name: Some("top".to_owned()),
                    file: file.clone(),
                },
                // Scalar column 15 points at `data`; the emoji in the comment
                // adds one extra UTF-16 code unit before the identifier.
                token(2, 15, llg::ffi::vpi::vpiNet),
                token(2, 15, llg::ffi::vpi::vpiNet),
                token(3, 8, llg::ffi::vpi::vpiRefObj),
                token(3, 15, llg::ffi::vpi::vpiRefObj),
            ],
        }];
        let analysis = Analysis::new(Vec::new(), model, tokens, Vec::new());

        let declaration = analysis
            .tokens
            .iter()
            .flat_map(|file_tokens| file_tokens.nodes.iter())
            .find(|node| node.name.as_deref() == Some("data") && node.line == 2)
            .expect("normalized declaration token");
        assert_eq!(declaration.col, 16);
        assert_eq!(declaration.end_col, 20);
        assert_eq!(
            FeatureSourceMap::new("😀data\nwire \\escaped😀name ;\n".to_owned()).normalize_1based(
                1,
                2,
                Some("data")
            ),
            (1, 3),
            "a supplementary character before a name consumes two UTF-16 units"
        );
        assert_eq!(lsp_name_len("escaped😀name"), 13);

        let entry = analysis
            .index
            .entry_at(&file, 1, 15)
            .expect("data declaration at UTF-16 column");
        assert_eq!(
            entry_location(entry).range,
            Range::new(Position::new(1, 15), Position::new(1, 19),)
        );
        assert!(token_at(&analysis, &file, 1, 15).is_some());

        let hover = hover_at(&analysis, &file, 1, 15).expect("hover on data");
        assert_eq!(
            hover.range,
            Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
        );
        let fallback_hover = hover_fallback(&analysis, &file, 1, 16).expect("fallback hover");
        assert_eq!(
            fallback_hover.range,
            Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
        );

        let definition = definition_at(&analysis, &file, 2, 7).expect("definition of data use");
        assert_eq!(definition.range.start, Position::new(1, 15));
        assert_eq!(definition.range.end, Position::new(1, 19));

        let references = references_at(&analysis, &file, 1, 15);
        let reference_starts: HashSet<_> = references
            .iter()
            .map(|location| (location.range.start.line, location.range.start.character))
            .collect();
        assert!(reference_starts.contains(&(1, 15)));
        assert!(reference_starts.contains(&(2, 7)));
        assert!(reference_starts.contains(&(2, 14)));

        let (rename_range, placeholder) = crate::rename::prepare_rename(&analysis, &file, 1, 15)
            .expect("rename on data declaration");
        assert_eq!(placeholder, "data");
        assert_eq!(
            rename_range,
            Range::new(Position::new(1, 15), Position::new(1, 19))
        );

        let semantic = semantic_tokens_for(&analysis, &file);
        let mut line = 0u32;
        let mut col = 0u32;
        let mut found_data = false;
        for token in semantic.data {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            if (line, col) == (1, 15) {
                assert_eq!(token.length, 4);
                found_data = true;
            }
        }
        assert!(
            found_data,
            "semantic token must use the UTF-16 declaration column"
        );
    }

    #[test]
    fn hover_on_port_shows_direction_and_type() {
        let a = sample_analysis();
        let hover = hover_at(&a, "/x/top.sv", 0, 4).expect("hover on port");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("input logic"), "value: {value}");
        assert!(value.contains("clk"), "value: {value}");
    }

    #[test]
    fn hover_on_param_shows_value() {
        let a = sample_analysis();
        let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on param");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("= 32'sd8"), "value: {value}");
        assert!(value.contains("parameter W"), "value: {value}");
    }

    #[test]
    fn hover_on_missing_position_is_none() {
        let a = sample_analysis();
        assert!(hover_at(&a, "/x/top.sv", 9, 9).is_none());
    }

    /// An [`Analysis`] carrying a macro table over one synthetic source:
    /// `` `define WIDTH 8 `` on line 1, a usage of it on line 4, an
    /// undefined usage on line 5.
    fn macro_analysis() -> Analysis {
        let text = concat!(
            "`define WIDTH 8\n",
            "module m;\n",
            "endmodule\n",
            "x = `WIDTH;\n",
            "y = `MISSING;\n",
        );
        let (model, tokens) = sample_parts();
        Analysis::new(Vec::new(), model, tokens, Vec::new()).with_macros(macros::build_table(
            &[],
            &[("/x/top.sv", text)],
            Some("llg.toml"),
        ))
    }

    #[test]
    fn hover_on_macro_usage_shows_resolved_value() {
        let a = macro_analysis();
        // `` `WIDTH `` starts at 0-based col 4 on line 3; click mid-name.
        let hover = hover_at(&a, "/x/top.sv", 3, 6).expect("hover on macro usage");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert_eq!(
            value,
            "```systemverilog\nmacro WIDTH = 8\n\ndefined at /x/top.sv:1\n```"
        );
        let range = hover.range.expect("hover range");
        assert_eq!(range.start, Position::new(3, 4));
        assert_eq!(range.end, Position::new(3, 10));
    }

    #[test]
    fn hover_on_undefined_macro_names_the_config() {
        let a = macro_analysis();
        let hover = hover_at(&a, "/x/top.sv", 4, 5).expect("hover on undefined macro");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(
            value.contains("`MISSING` is not defined under the current configuration."),
            "value: {value}"
        );
        assert!(
            value.contains("Checked the `[compile] defines` in llg.toml"),
            "the config note must name the checked source: {value}"
        );
    }

    #[test]
    fn hover_on_define_site_shows_the_same_value() {
        let a = macro_analysis();
        // The NAME identifier of `` `define WIDTH 8 `` (0-based col 8).
        let hover = hover_at(&a, "/x/top.sv", 0, 9).expect("hover on define site");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("macro WIDTH = 8"), "value: {value}");
    }

    #[test]
    fn hover_on_function_like_macro_renders_args() {
        let text = "`define MAX(a, b) ((a) > (b)) ? (a) : (b)\nx = `MAX(p, q);\n";
        let (model, tokens) = sample_parts();
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new())
            .with_macros(macros::build_table(&[], &[("/x/top.sv", text)], None));
        let hover = hover_at(&a, "/x/top.sv", 1, 5).expect("hover on function-like usage");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert_eq!(
            value,
            concat!(
                "```systemverilog\nmacro MAX(a, b) = ((a) > (b)) ? (a) : (b)\n",
                "\ndefined at /x/top.sv:1\n```"
            )
        );
    }

    #[test]
    fn hover_on_function_name_shows_signature_and_scope() {
        let a = sample_analysis();
        // `function int add(...)` at 1-based (4, 8) → 0-based (3, 7).
        let hover = hover_at(&a, "/x/top.sv", 3, 7).expect("hover on function name");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(
            value.contains("function int add(input int a, input int b)"),
            "value: {value}"
        );
        assert!(value.contains("top.u0"), "scope missing: {value}");
        assert!(
            value.contains("automatic"),
            "storage class missing: {value}"
        );
    }

    #[test]
    fn hover_on_task_name_shows_signature_and_static() {
        let a = sample_analysis();
        // `task run(...)` at 1-based (5, 8) → 0-based (4, 7).
        let hover = hover_at(&a, "/x/top.sv", 4, 7).expect("hover on task name");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("task run(input int n)"), "value: {value}");
        assert!(value.contains("static"), "storage class missing: {value}");
    }

    #[test]
    fn hover_on_param_decl_shows_elaborated_value_line() {
        let a = sample_analysis();
        // `parameter W` at 1-based (2, 5) → 0-based (1, 4).
        let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on param decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert_eq!(
            value, "```systemverilog\nparameter W: int\nvalue = 32'sd8\n```",
            "the elaborated value must be its own short line: {value}"
        );
    }

    #[test]
    fn hover_on_bound_param_reference_shows_elaborated_value() {
        // A ref position bound through `ref_bindings` to the W declaration
        // (0-based line0=1, col0=4): the value must come from the committed
        // model via the TARGET's coordinates — even when the reference sits
        // in a different file than its declaration.
        let mut bindings: RefBindings = HashMap::new();
        bindings.insert(
            ("/x/other.sv".to_owned(), 6, 2),
            DeclTarget {
                name: "W".to_owned(),
                kind: "parameter".to_owned(),
                file: "/x/top.sv".to_owned(),
                line0: 1,
                col0: 4,
                via_label: false,
                via_connection: false,
            },
        );
        let a = sample_analysis_with_bindings(bindings);
        let hover = hover_at(&a, "/x/other.sv", 6, 2).expect("hover on bound W ref");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(
            value.contains("parameter W") && value.contains("\nvalue = 32'sd8"),
            "cross-file ref-site hover must show the declaration's value: {value}"
        );
        assert!(
            !value.contains("= 32'sd8\nvalue"),
            "the value must not render twice: {value}"
        );
    }

    #[test]
    fn hover_on_unresolved_param_omits_the_value_line() {
        let (mut model, tokens) = sample_parts();
        model.top_instances[0].params[0].value = None;
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
        let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on unresolved param");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert_eq!(
            value, "```systemverilog\nparameter W: int\n```",
            "an unresolved value must omit the line silently: {value}"
        );
    }

    #[test]
    fn hover_on_non_param_symbols_has_no_value_line() {
        let a = sample_analysis();
        for (line, col, what) in [(0u32, 4u32, "port clk"), (0, 19, "instance u0")] {
            let hover =
                hover_at(&a, "/x/top.sv", line, col).unwrap_or_else(|| panic!("hover on {what}"));
            let value = match hover.contents {
                HoverContents::Markup(m) => m.value,
                _ => panic!("expected markup hover on {what}"),
            };
            assert!(
                !value.contains("value = "),
                "{what} hover must not gain a value line: {value}"
            );
        }
    }

    #[test]
    fn param_elab_value_is_scoped_and_omits_divergent_overrides() {
        fn bits(v: u64) -> Val {
            Val::Bits(Value::from_u64(v, 32, true))
        }
        let (mut model, tokens) = sample_parts();
        // A second clone of module `m` overrides W differently; both carry a
        // generate scope with a branch-local parameter.
        let mut u1 = model.top_instances[0].clone();
        u1.name = "u1".to_owned();
        u1.full_name = "top.u1".to_owned();
        u1.params[0].value = Some(bits(4));
        u1.gen_scopes = vec![GenScopeModel {
            name: "g_wide".to_owned(),
            full_name: "top.u1.g_wide".to_owned(),
            params: vec![ParamModel {
                name: "BRANCH".to_owned(),
                value: Some(bits(4)),
                ty: TypeInfo {
                    kind: "int".to_owned(),
                    width: None,
                    signed: true,
                    type_name: None,
                },
                local: true,
            }],
            children: Vec::new(),
        }];
        model.top_instances[0].gen_scopes = u1.gen_scopes.clone();
        model.top_instances[0].gen_scopes[0].params[0].value = Some(bits(8));
        model.top_instances.push(u1);
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new());

        // W diverges across clones → ambiguous at module granularity → None.
        assert_eq!(param_elab_value(&a, "/x/top.sv", 1, "W"), None);
        // BRANCH diverges across the clones' g_wide scopes → ambiguous too.
        assert_eq!(param_elab_value(&a, "/x/top.sv", 2, "BRANCH"), None);

        // Making both clones agree resolves the value even from a gen-scope.
        let (mut model, tokens) = sample_parts();
        let mut u1 = model.top_instances[0].clone();
        u1.name = "u1".to_owned();
        u1.full_name = "top.u1".to_owned();
        model.top_instances.push(u1);
        model.top_instances[0].gen_scopes = vec![GenScopeModel {
            name: "g_wide".to_owned(),
            full_name: "top.g_wide".to_owned(),
            params: vec![ParamModel {
                name: "BRANCH".to_owned(),
                value: Some(bits(8)),
                ty: TypeInfo {
                    kind: "int".to_owned(),
                    width: None,
                    signed: true,
                    type_name: None,
                },
                local: true,
            }],
            children: Vec::new(),
        }];
        model.top_instances[1].gen_scopes = model.top_instances[0].gen_scopes.clone();
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
        assert_eq!(
            param_elab_value(&a, "/x/top.sv", 1, "W"),
            Some(&bits(8)),
            "unanimous clones resolve to the shared value"
        );
        assert_eq!(
            param_elab_value(&a, "/x/top.sv", 2, "BRANCH"),
            Some(&bits(8)),
            "generate-scope parameters resolve through their gen scopes"
        );

        // The divergent case renders WITHOUT a value at all (no stale inline
        // number either — the display model clears it).
        let (mut model, tokens) = sample_parts();
        let mut u1 = model.top_instances[0].clone();
        u1.name = "u1".to_owned();
        u1.full_name = "top.u1".to_owned();
        u1.params[0].value = Some(bits(4));
        model.top_instances.push(u1);
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
        let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on ambiguous param");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert_eq!(
            value, "```systemverilog\nparameter W: int\n```",
            "ambiguity omits rather than guesses: {value}"
        );
    }

    #[test]
    fn definition_on_instance_resolves_to_module_def() {
        let a = sample_analysis();
        let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("definition of u0");
        assert_eq!(loc.uri, Url::from_file_path("/x/top.sv").unwrap());
        assert_eq!(loc.range.start.line, 0);
        assert_eq!(loc.range.start.character, 7); // module m at col 8 → 0-based 7
    }

    #[test]
    fn definition_on_module_name_resolves_in_place() {
        let a = sample_analysis();
        let loc = definition_at(&a, "/x/top.sv", 0, 7).expect("definition of m");
        assert_eq!(loc.range.start.line, 0);
        assert_eq!(loc.range.start.character, 7);
    }

    #[test]
    fn definition_at_bound_position_serves_the_uhdm_binding_target() {
        // Synthetic UHDM capture: the reference at 0-based (0,19) (`u0`) is
        // bound to a declaration in another file.  The binding path must win
        // over the index (which would resolve the instance to /x/top.sv).
        let mut bindings: RefBindings = HashMap::new();
        bindings.insert(
            ("/x/top.sv".to_owned(), 0, 19),
            DeclTarget {
                name: "u0".to_owned(),
                kind: "module".to_owned(),
                file: "/x/bound.sv".to_owned(),
                line0: 4,
                col0: 2,
                via_label: false,
                via_connection: false,
            },
        );
        let a = sample_analysis_with_bindings(bindings);
        let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("binding-precise definition");
        assert_eq!(loc.uri, Url::from_file_path("/x/bound.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(4, 2));
        // Same identifier-width range convention as `entry_location`: the
        // range spans exactly the target name.
        assert_eq!(loc.range.end, Position::new(4, 4));
    }

    #[test]
    fn definition_at_unbound_position_falls_back_to_index_resolution() {
        let a = sample_analysis_with_bindings(HashMap::new());
        // (0,19) is the `u0` instance declaration; with no binding for the
        // position the index resolves the instance to its module definition.
        let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("fallback definition");
        assert_eq!(loc.uri, Url::from_file_path("/x/top.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 7));
    }

    #[test]
    fn port_labels_join_ref_bindings_in_new_with_outcome() {
        let a = multiline_port_analysis();
        // Both continuation-line labels are registered as port labels AND
        // folded into `ref_bindings`, targeting the child module's port
        // declarations in /x/a.sv.
        let clk = a
            .ref_bindings
            .get(&("/x/b.sv".to_owned(), 1, 3))
            .expect("clk label binding");
        assert_eq!(clk.name, "clk");
        assert_eq!(clk.kind, "port");
        assert_eq!(clk.file, "/x/a.sv");
        assert_eq!((clk.line0, clk.col0), (0, 23));
        assert!(
            clk.via_label,
            "label-folded bindings must be tagged via_label"
        );
        let o = a
            .ref_bindings
            .get(&("/x/b.sv".to_owned(), 2, 3))
            .expect("o label binding");
        assert_eq!(o.name, "o");
        assert_eq!(o.kind, "port");
        assert_eq!(o.file, "/x/a.sv");
        // Definition at the bound label positions serves the folded targets.
        let loc = definition_at(&a, "/x/b.sv", 1, 3).expect("definition of .clk label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
    }

    #[test]
    fn merged_ref_bindings_prefers_uhdm_targets_on_collision() {
        // Collision policy: UHDM bindings are inserted after port-label ones,
        // so where both captured the same position the elaboration-backed
        // UHDM target wins; disjoint entries from BOTH sources survive.
        let mut index = SymbolIndex::default();
        index.decls.push(SymEntry {
            name: "clk".to_owned(),
            kind: SymKind::Port,
            file: "/x/child.sv".to_owned(),
            line: 3,
            col: 4,
            end_line: 3,
            end_col: 7,
            is_decl: true,
            scope: None,
            detail: None,
        });
        index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);
        index
            .port_labels
            .insert(("/x/top.sv".to_owned(), 10, 11), 0);

        let mut uhdm: RefBindings = HashMap::new();
        // Overlaps the (8,9) port-label entry with a different target...
        uhdm.insert(
            ("/x/top.sv".to_owned(), 8, 9),
            DeclTarget {
                name: "clk".to_owned(),
                kind: "net".to_owned(),
                file: "/x/elab.sv".to_owned(),
                line0: 6,
                col0: 1,
                via_label: false,
                via_connection: false,
            },
        );
        // ...and adds a position the label heuristic never saw.
        uhdm.insert(
            ("/x/top.sv".to_owned(), 20, 21),
            DeclTarget {
                name: "rst".to_owned(),
                kind: "net".to_owned(),
                file: "/x/elab.sv".to_owned(),
                line0: 12,
                col0: 2,
                via_label: false,
                via_connection: false,
            },
        );

        let merged =
            merged_ref_bindings(&index, &empty_design(), uhdm, &ConnectionInputs::default());
        assert_eq!(
            merged
                .get(&("/x/top.sv".to_owned(), 8, 9))
                .map(|t| t.file.as_str()),
            Some("/x/elab.sv"),
            "UHDM binding must win on collision"
        );
        let label_only = merged
            .get(&("/x/top.sv".to_owned(), 10, 11))
            .expect("disjoint port-label entry survives");
        assert_eq!(
            (label_only.file.as_str(), label_only.line0, label_only.col0),
            ("/x/child.sv", 3, 4)
        );
        let uhdm_only = merged
            .get(&("/x/top.sv".to_owned(), 20, 21))
            .expect("UHDM-only entry survives");
        assert_eq!(uhdm_only.name, "rst");
    }

    #[test]
    fn connection_pairs_bind_actuals_to_the_parent_scope_declaration() {
        // UHDM mode: a resolved label (`.clk` at 0-based (8,9)) keeps its
        // child-port target (`via_label`); the paired ACTUAL identifier binds
        // to its OWN declaration in the instantiating (parent) scope, NOT to
        // the child port.  Two same-named declarations exist in the file in
        // DIFFERENT module spans; the innermost span containing the
        // instantiation line picks the parent module's declaration.
        let mut index = SymbolIndex::default();
        // Child port `clk` in the child definition file.
        index.decls.push(SymEntry {
            name: "clk".to_owned(),
            kind: SymKind::Port,
            file: "/x/child.sv".to_owned(),
            line: 3,
            col: 4,
            end_line: 3,
            end_col: 7,
            is_decl: true,
            scope: None,
            detail: None,
        });
        // Parent-scope net `wa` inside module `other` (decoy span).
        index.decls.push(SymEntry {
            name: "wa".to_owned(),
            kind: SymKind::Net,
            file: "/x/top.sv".to_owned(),
            line: 1,
            col: 2,
            end_line: 1,
            end_col: 4,
            is_decl: true,
            scope: Some("other".to_owned()),
            detail: None,
        });
        // Parent-scope net `wa` inside module `parent` — the enclosing scope.
        index.decls.push(SymEntry {
            name: "wa".to_owned(),
            kind: SymKind::Net,
            file: "/x/top.sv".to_owned(),
            line: 6,
            col: 2,
            end_line: 6,
            end_col: 4,
            is_decl: true,
            scope: Some("parent".to_owned()),
            detail: None,
        });
        index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: Vec::new(),
            modules: vec![
                ModuleDef {
                    name: "other".to_owned(),
                    file: Some("/x/top.sv".to_owned()),
                    line: 1,
                    col: 1,
                    end_line: 3,
                    end_col: 10,
                },
                ModuleDef {
                    name: "parent".to_owned(),
                    file: Some("/x/top.sv".to_owned()),
                    line: 5,
                    col: 1,
                    end_line: 20,
                    end_col: 10,
                },
            ],
            packages: Vec::new(),
            classes: Vec::new(),
        };

        let connections = ConnectionInputs {
            parse_decls: None,
            pairs: vec![NamedPortConn {
                file: "/x/top.sv".to_owned(),
                label: (9, 10),
                label_name: "clk".to_owned(),
                kind: ConnKind::Port,
                actual: Some((9, 13)),
                actual_name: Some("wa".to_owned()),
                inst_type: Some("m".to_owned()),
            }],
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        };
        let merged = merged_ref_bindings(&index, &model, HashMap::new(), &connections);
        let label = merged
            .get(&("/x/top.sv".to_owned(), 8, 9))
            .expect("label binding");
        assert_eq!(
            (label.file.as_str(), label.line0, label.col0),
            ("/x/child.sv", 3, 4),
            "the label must still navigate to the CHILD port"
        );
        assert!(label.via_label && !label.via_connection);
        let actual = merged
            .get(&("/x/top.sv".to_owned(), 8, 12))
            .expect("actual binding");
        assert_eq!(
            (actual.file.as_str(), actual.line0, actual.col0),
            ("/x/top.sv", 6, 2),
            "the actual must navigate to its parent-scope declaration"
        );
        assert_eq!(actual.name, "wa");
        assert_eq!(actual.kind, "net");
        assert!(actual.via_connection && !actual.via_label);
    }

    #[test]
    fn connection_actual_fold_never_overrides_an_explicit_binding() {
        // Collision rule: the ACTUAL fold only fills unbound positions; an
        // existing explicit binding (elaboration-backed or fallback) wins.
        let mut index = SymbolIndex::default();
        index.decls.push(SymEntry {
            name: "wa".to_owned(),
            kind: SymKind::Net,
            file: "/x/top.sv".to_owned(),
            line: 1,
            col: 2,
            end_line: 1,
            end_col: 4,
            is_decl: true,
            scope: None,
            detail: None,
        });
        index.decls.push(SymEntry {
            name: "clk".to_owned(),
            kind: SymKind::Port,
            file: "/x/child.sv".to_owned(),
            line: 3,
            col: 4,
            end_line: 3,
            end_col: 7,
            is_decl: true,
            scope: None,
            detail: None,
        });
        index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

        let mut uhdm: RefBindings = HashMap::new();
        uhdm.insert(
            ("/x/top.sv".to_owned(), 8, 12),
            DeclTarget {
                name: "wa".to_owned(),
                kind: "net".to_owned(),
                file: "/x/elab.sv".to_owned(),
                line0: 6,
                col0: 1,
                via_label: false,
                via_connection: false,
            },
        );

        let connections = ConnectionInputs {
            parse_decls: None,
            pairs: vec![NamedPortConn {
                file: "/x/top.sv".to_owned(),
                label: (9, 10),
                label_name: "clk".to_owned(),
                kind: ConnKind::Port,
                actual: Some((9, 13)),
                actual_name: Some("wa".to_owned()),
                inst_type: Some("m".to_owned()),
            }],
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        };
        let merged = merged_ref_bindings(&index, &empty_design(), uhdm, &connections);
        let kept = merged
            .get(&("/x/top.sv".to_owned(), 8, 12))
            .expect("pre-existing binding survives");
        assert_eq!(
            (kept.file.as_str(), kept.line0, kept.col0),
            ("/x/elab.sv", 6, 1),
            "existing explicit binding must win at the actual position"
        );
        assert!(!kept.via_connection);
    }

    #[test]
    fn connection_actual_without_parent_scope_candidate_stays_unbound() {
        // No same-name declaration exists in the instantiating file: NO
        // binding is emitted for the actual (never the child port).
        let mut index = SymbolIndex::default();
        index.decls.push(SymEntry {
            name: "clk".to_owned(),
            kind: SymKind::Port,
            file: "/x/child.sv".to_owned(),
            line: 3,
            col: 4,
            end_line: 3,
            end_col: 7,
            is_decl: true,
            scope: None,
            detail: None,
        });
        index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

        let connections = ConnectionInputs {
            parse_decls: None,
            pairs: vec![NamedPortConn {
                file: "/x/top.sv".to_owned(),
                label: (9, 10),
                label_name: "clk".to_owned(),
                kind: ConnKind::Port,
                actual: Some((9, 13)),
                actual_name: Some("ghost".to_owned()),
                inst_type: Some("m".to_owned()),
            }],
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        };
        let merged = merged_ref_bindings(&index, &empty_design(), HashMap::new(), &connections);
        assert!(!merged.contains_key(&("/x/top.sv".to_owned(), 8, 12)));
        assert!(merged.contains_key(&("/x/top.sv".to_owned(), 8, 9)));
    }

    /// End-to-end over the classifier-labeled connection types: a hand-built
    /// UHDM-mode analysis whose port/override LABEL tokens carry
    /// `TOKEN_PORT_CONN_LABEL` / `TOKEN_PARAM_CONN_LABEL` (as
    /// `collect_parse_tokens` emits them).
    ///
    /// Pins three facts at once:
    ///
    /// * labels stay indexed as REFERENCES (never declarations) and keep
    ///   navigating to the child module's declaration;
    /// * connected-signal/RHS positions navigate to their PARENT-scope
    ///   declaration;
    /// * the semantic-token stream marks exactly the label positions with the
    ///   `connectionLabel` modifier (`function/connectionLabel`,
    ///   `property/readonly+connectionLabel`) while the connected signals
    ///   stay plain `variable`.
    #[test]
    fn connection_label_tokens_index_as_references_and_highlight_as_labels() {
        use llg::ffi::vpi;

        // `/x/a.sv`:
        //   line 1: `module m(input logic clk);`   — port `clk` at col 22
        //   line 2: `  parameter int W = 4;`       — param `W` at col 17
        // `/x/b.sv`:
        //   line 1: `module top; m #(.W(w)) u0 (.clk(c));`
        //     type `m` col 13, override label `W` col 18, RHS `w` col 20,
        //     instance `u0` col 24, port label `clk` col 29, actual `c` col 33
        //   line 2: `  logic w;` — parent net `w` at col 9
        //   line 3: `  logic c;` — parent net `c` at col 9
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: String::new(),
        };
        let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
            FileTokens {
                path: path.to_owned(),
                nodes: nodes
                    .into_iter()
                    .map(|(l, c, t, n)| {
                        let mut v = node(l, c, t, n);
                        v.file = path.to_owned();
                        v
                    })
                    .collect(),
            }
        };

        let a_file = mk(
            vec![
                (1, 8, vpi::vpiModule, "m"),
                (1, 22, vpi::TOKEN_PORT_INPUT, "clk"),
                (1, 22, vpi::vpiPort, "clk"),
                (2, 17, vpi::vpiParameter, "W"),
                (2, 17, vpi::uhdmparameter, "W"),
            ],
            "/x/a.sv",
        );
        let b_file = mk(
            vec![
                (1, 13, vpi::uhdmclass_defn, "m"),
                (1, 18, vpi::TOKEN_PARAM_CONN_LABEL, "W"),
                (1, 20, vpi::vpiRefObj, "w"),
                (1, 24, vpi::uhdmlogic_var, "u0"),
                (1, 29, vpi::TOKEN_PORT_CONN_LABEL, "clk"),
                (1, 33, vpi::vpiRefObj, "c"),
                (2, 9, vpi::uhdmlogic_var, "w"),
                (2, 9, vpi::vpiNet, "w"),
                (3, 9, vpi::uhdmlogic_var, "c"),
                (3, 9, vpi::vpiNet, "c"),
            ],
            "/x/b.sv",
        );

        let ty = TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        };
        let module_m = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 2,
            end_col: 26,
        };
        let module_top = ModuleDef {
            name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 4,
            end_col: 12,
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 24,
            ports: vec![PortModel {
                name: "clk".to_owned(),
                direction: Direction::Input,
                ty: ty.clone(),
            }],
            signals: Vec::new(),
            params: vec![ParamModel {
                name: "W".to_owned(),
                value: None,
                ty: ty.clone(),
                local: false,
            }],
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: vec![
                SignalModel {
                    name: "w".to_owned(),
                    kind: "net".to_owned(),
                    ty: ty.clone(),
                },
                SignalModel {
                    name: "c".to_owned(),
                    kind: "net".to_owned(),
                    ty: ty.clone(),
                },
            ],
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: vec![module_top, module_m],
            packages: Vec::new(),
            classes: Vec::new(),
        };

        let pairs = vec![
            NamedPortConn {
                file: "/x/b.sv".to_owned(),
                label: (1, 29),
                label_name: "clk".to_owned(),
                kind: ConnKind::Port,
                actual: Some((1, 33)),
                actual_name: Some("c".to_owned()),
                inst_type: Some("m".to_owned()),
            },
            NamedPortConn {
                file: "/x/b.sv".to_owned(),
                label: (1, 18),
                label_name: "W".to_owned(),
                kind: ConnKind::Param,
                actual: Some((1, 20)),
                actual_name: Some("w".to_owned()),
                inst_type: Some("m".to_owned()),
            },
        ];
        let a = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            model,
            vec![a_file, b_file],
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_decls: None,
                pairs,
                fallback_bindings: HashMap::new(),
                ..ConnectionInputs::default()
            },
        );

        // Labels are REF entries, never declarations.
        let port_label_entry = a
            .index
            .entry_at("/x/b.sv", 0, 28)
            .expect("port label indexed");
        assert!(!port_label_entry.is_decl);
        let param_label_entry = a
            .index
            .entry_at("/x/b.sv", 0, 17)
            .expect("param override label indexed");
        assert!(!param_label_entry.is_decl);

        // Labels navigate to the CHILD module's declarations…
        let loc = definition_at(&a, "/x/b.sv", 0, 28).expect("definition at .clk label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 21));
        let loc = definition_at(&a, "/x/b.sv", 0, 17).expect("definition at .W label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(1, 16));

        // …while the connected signal / override RHS stay parent-scope.
        let loc = definition_at(&a, "/x/b.sv", 0, 32).expect("definition at actual c");
        assert_eq!(loc.uri, Url::from_file_path("/x/b.sv").unwrap());
        let loc = definition_at(&a, "/x/b.sv", 0, 19).expect("definition at override RHS w");
        assert_eq!(loc.uri, Url::from_file_path("/x/b.sv").unwrap());

        // Semantic surface: exactly the label rows carry `connectionLabel`.
        let legend = crate::semantic_tokens::legend();
        let data = semantic_tokens_for(&a, "/x/b.sv").data;
        let decode_sym = |want_line: u64, want_col: u64| -> String {
            let mut line = 0u64;
            let mut col = 0u64;
            for token in &data {
                line += token.delta_line as u64;
                col = if token.delta_line == 0 {
                    col + token.delta_start as u64
                } else {
                    token.delta_start as u64
                };
                if (line, col) == (want_line, want_col) {
                    let base = legend.token_types[token.token_type as usize].as_str();
                    let modifiers: Vec<String> = legend
                        .token_modifiers
                        .iter()
                        .enumerate()
                        .filter(|(bit, _)| token.token_modifiers_bitset & (1u32 << bit) != 0)
                        .map(|(_, name)| name.as_str().to_owned())
                        .collect();
                    return if modifiers.is_empty() {
                        base.to_owned()
                    } else {
                        format!("{base}/{}", modifiers.join("+"))
                    };
                }
            }
            panic!("no semantic token at {want_line}:{want_col}");
        };
        assert_eq!(decode_sym(0, 28), "function/connectionLabel");
        assert_eq!(decode_sym(0, 17), "property/readonly+connectionLabel");
        assert_eq!(decode_sym(0, 32), "variable");
        assert_eq!(decode_sym(0, 19), "variable");
    }

    #[test]
    fn fallback_connection_bindings_survive_the_merge() {
        // Parse-fallback mode carries pre-resolved label+actual bindings;
        // they are inserted before UHDM (which is empty there) and survive.
        let connections = ConnectionInputs {
            parse_decls: None,
            pairs: Vec::new(),
            fallback_bindings: [
                (
                    ("/x/tb.sv".to_owned(), 4, 12),
                    DeclTarget {
                        name: "clk".to_owned(),
                        kind: "port".to_owned(),
                        file: "/x/child.sv".to_owned(),
                        line0: 0,
                        col0: 23,
                        via_label: true,
                        via_connection: false,
                    },
                ),
                (
                    ("/x/tb.sv".to_owned(), 4, 16),
                    DeclTarget {
                        name: "clk".to_owned(),
                        kind: "port".to_owned(),
                        file: "/x/child.sv".to_owned(),
                        line0: 0,
                        col0: 23,
                        via_label: false,
                        via_connection: true,
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..ConnectionInputs::default()
        };
        let index = SymbolIndex::default();
        let merged = merged_ref_bindings(&index, &empty_design(), HashMap::new(), &connections);
        assert_eq!(merged.len(), 2, "both fallback entries survive: {merged:?}");
        assert!(merged[&("/x/tb.sv".to_owned(), 4, 12)].via_label);
        assert!(merged[&("/x/tb.sv".to_owned(), 4, 16)].via_connection);
    }

    #[test]
    fn references_include_declaration() {
        let a = sample_analysis();
        let refs = references_at(&a, "/x/top.sv", 0, 4);
        assert!(refs
            .iter()
            .any(|l| l.range.start.line == 0 && l.range.start.character == 4));
    }

    #[test]
    fn references_options_include_declaration_preserves_existing_results() {
        let a = cross_file_analysis();
        let existing = references_at(&a, "/x/a.sv", 0, 7);
        let with_option = references_at_with_options(&a, "/x/a.sv", 0, 7, true);

        assert_eq!(with_option, existing);
        assert!(with_option.iter().any(|location| {
            location.uri == Url::from_file_path("/x/a.sv").unwrap()
                && location.range.start == Position::new(0, 7)
        }));
        assert!(with_option.iter().any(|location| {
            location.uri == Url::from_file_path("/x/b.sv").unwrap()
                && location.range.start == Position::new(0, 12)
        }));
    }

    #[test]
    fn references_options_exclude_indexed_declaration() {
        let a = cross_file_analysis();
        let refs = references_at_with_options(&a, "/x/a.sv", 0, 7, false);

        assert!(!refs.iter().any(|location| {
            location.uri == Url::from_file_path("/x/a.sv").unwrap()
                && location.range.start == Position::new(0, 7)
        }));
        assert!(refs.iter().any(|location| {
            location.uri == Url::from_file_path("/x/b.sv").unwrap()
                && location.range.start == Position::new(0, 12)
        }));
    }

    #[test]
    fn references_options_filter_fallback_declaration() {
        let node = |line: u32, ty: i32| VObjectInfo {
            line,
            col: 1,
            end_line: line,
            end_col: 7,
            vpi_type: ty,
            name: Some("thing".to_owned()),
            file: "/x/fallback.sv".to_owned(),
        };
        let a = Analysis::new(
            Vec::new(),
            empty_design(),
            vec![FileTokens {
                path: "/x/fallback.sv".to_owned(),
                nodes: vec![
                    node(1, llg::ffi::vpi::vpiModule),
                    node(2, llg::ffi::vpi::vpiRefObj),
                ],
            }],
            Vec::new(),
        );

        assert!(a.index.entry_at("/x/fallback.sv", 0, 0).is_none());
        let with_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, true);
        let without_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, false);

        assert_eq!(with_declaration.len(), 2);
        assert_eq!(without_declaration.len(), 1);
        assert_eq!(without_declaration[0].range.start, Position::new(1, 0));
    }

    #[test]
    fn fatal_preflight_contains_only_the_supplied_fatal_diagnostic() {
        let analysis = Analysis::fatal_preflight("workspace is not ready");

        assert_eq!(analysis.outcome, AnalysisOutcome::Fatal);
        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].severity, Severity::Fatal);
        assert_eq!(analysis.diagnostics[0].message, "workspace is not ready");
        assert!(analysis.diagnostics[0].file.is_none());
        assert!(analysis.model.modules.is_empty());
        assert!(analysis.tokens.is_empty());
        assert!(analysis.lint.is_empty());
        assert!(analysis.index.decls.is_empty());
        assert!(analysis.index.refs.is_empty());
    }

    #[test]
    fn db_build_failure_is_reported_at_unknown_source_position_and_invalidates_analysis() {
        let error = "node walk failed";
        let diagnostic = db_build_diagnostic(error);
        let analysis = Analysis::new_with_outcome(
            AnalysisOutcome::Compile,
            vec![diagnostic],
            empty_design(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            ConnectionInputs::default(),
        );

        assert_eq!(analysis.outcome, AnalysisOutcome::Compile);
        assert!(!analysis.is_valid());
        assert_eq!(analysis.diagnostics[0].severity, Severity::Error);
        assert!(analysis.diagnostics[0].file.is_none());
        assert_eq!(
            (analysis.diagnostics[0].line, analysis.diagnostics[0].col),
            (0, 0)
        );
        assert!(analysis.diagnostics[0].message.contains(error));
        assert!(analysis.model.modules.is_empty());
        assert!(analysis.tokens.is_empty());
        assert!(analysis.lint.is_empty());
    }

    /// A minimal model + token set that [`Analysis::has_feature_data`]
    /// recognizes as servable.
    fn served_feature_parts() -> (DesignModel, Vec<FileTokens>) {
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: Vec::new(),
            modules: vec![ModuleDef {
                name: "m".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 3,
                end_col: 12,
            }],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let tokens = vec![FileTokens {
            path: "/x/top.sv".to_owned(),
            nodes: vec![VObjectInfo {
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 9,
                vpi_type: llg::ffi::vpi::vpiModule,
                name: Some("m".to_owned()),
                file: "/x/top.sv".to_owned(),
            }],
        }];
        (model, tokens)
    }

    /// Feature-serving gate truth table: Parse/Compile outcomes serve
    /// best-effort like a valid one whenever any servable data exists;
    /// Fatal never serves; and an analysis without any data (empty project)
    /// never serves either.
    #[test]
    fn has_feature_data_truth_table() {
        for outcome in [
            AnalysisOutcome::Valid,
            AnalysisOutcome::Compile,
            AnalysisOutcome::Parse,
        ] {
            let (model, tokens) = served_feature_parts();
            let analysis = Analysis::new_with_outcome(
                outcome,
                Vec::new(),
                model,
                tokens,
                Vec::new(),
                HashMap::new(),
                ConnectionInputs::default(),
            );
            assert!(
                analysis.has_feature_data(),
                "{outcome:?} with feature data must serve"
            );
        }

        // Fatal stays feature-less even if data were present.
        let (model, tokens) = served_feature_parts();
        let fatal = Analysis::new_with_outcome(
            AnalysisOutcome::Fatal,
            Vec::new(),
            model,
            tokens,
            Vec::new(),
            HashMap::new(),
            ConnectionInputs::default(),
        );
        assert!(!fatal.has_feature_data());

        // No data at all (db built but produced nothing): never serves.
        for outcome in [
            AnalysisOutcome::Valid,
            AnalysisOutcome::Compile,
            AnalysisOutcome::Parse,
        ] {
            let analysis = Analysis::new_with_outcome(
                outcome,
                Vec::new(),
                empty_design(),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                ConnectionInputs::default(),
            );
            assert!(
                !analysis.has_feature_data(),
                "{outcome:?} without any servable data must not serve"
            );
        }

        // The preflight constructor produces the documented shape: Fatal,
        // no feature data.
        assert!(!Analysis::fatal_preflight("aborted").has_feature_data());
    }

    #[test]
    fn document_symbols_contain_module_with_port_child() {
        let a = sample_analysis();
        let syms = document_symbols(&a, "/x/top.sv");
        let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
        assert_eq!(module.kind, SymbolKind::MODULE);
        assert_eq!(module.range.start.line, 0);
        let children = module.children.as_ref().expect("children");
        let port = children
            .iter()
            .find(|c| c.name == "clk")
            .expect("port child");
        assert_eq!(port.kind, SymbolKind::PROPERTY);
        assert_eq!(port.range.start.line, 0);
        assert_eq!(port.range.start.character, 4);
        assert!(syms
            .iter()
            .any(|s| s.name == "p" && s.kind == SymbolKind::PACKAGE));
    }

    #[test]
    fn document_symbols_include_functions_and_tasks() {
        let a = sample_analysis();
        let syms = document_symbols(&a, "/x/top.sv");
        let add = syms
            .iter()
            .find(|s| s.name == "add")
            .expect("function symbol");
        assert_eq!(add.kind, SymbolKind::FUNCTION);
        assert_eq!(add.range.start, Position::new(3, 7));
        assert_eq!(
            add.detail.as_deref(),
            Some("function int add(input int a, input int b)")
        );
        let run = syms.iter().find(|s| s.name == "run").expect("task symbol");
        assert_eq!(run.kind, SymbolKind::FUNCTION);
        assert_eq!(run.range.start, Position::new(4, 7));
        assert_eq!(run.detail.as_deref(), Some("task run(input int n)"));
    }

    #[test]
    fn document_symbol_children_carry_type_only_details() {
        // Model details carry names/values (`input logic clk`,
        // `parameter W: int = 32'sd8`); document-symbol children must render
        // the type text without the declared name.
        let a = sample_analysis();
        let syms = document_symbols(&a, "/x/top.sv");
        let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
        let children = module.children.as_ref().expect("children");
        let port = children
            .iter()
            .find(|c| c.name == "clk")
            .expect("port child");
        assert_eq!(port.detail.as_deref(), Some("input logic"));
        let param = children.iter().find(|c| c.name == "W").expect("param");
        assert_eq!(param.detail.as_deref(), Some("parameter int"));
    }

    #[test]
    fn decl_details_drive_type_only_details() {
        let a = sample_analysis();
        // Keys are the 1-based declaration positions of `clk` and `W`.
        let mut snippets = HashMap::new();
        snippets.insert(
            ("/x/top.sv".to_owned(), 1u32, 5u32),
            "input logic [1:0] clk".to_owned(),
        );
        snippets.insert(("/x/top.sv".to_owned(), 2u32, 5u32), "int W".to_owned());
        let a = a.with_decl_details(snippets);
        let syms = document_symbols(&a, "/x/top.sv");
        let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
        let children = module.children.as_ref().expect("children");
        let port = children
            .iter()
            .find(|c| c.name == "clk")
            .expect("port child");
        assert_eq!(port.detail.as_deref(), Some("input logic [1:0]"));
        let param = children.iter().find(|c| c.name == "W").expect("param");
        // Parameters keep their model keyword and gain the declared type word.
        assert_eq!(param.detail.as_deref(), Some("parameter int"));
    }

    /// Model + tokens for /x/top.sv with an instantiation chain: top-level
    /// instance `tb` (of module `top`) containing `u0` (of module `m`).
    fn hierarchy_parts() -> (DesignModel, Vec<FileTokens>) {
        let leaf_port = PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        };
        let leaf = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "tb.u0".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 2,
            col: 9,
            ports: vec![leaf_port],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let tb = InstanceModel {
            name: "tb".to_owned(),
            def_name: "top".to_owned(),
            full_name: "tb".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 6,
            col: 3,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![leaf],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![tb],
            modules: vec![
                ModuleDef {
                    name: "top".to_owned(),
                    file: Some("/x/top.sv".to_owned()),
                    line: 5,
                    col: 8,
                    end_line: 7,
                    end_col: 12,
                },
                ModuleDef {
                    name: "m".to_owned(),
                    file: Some("/x/top.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 3,
                    end_col: 12,
                },
            ],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let tokens = vec![FileTokens {
            path: "/x/top.sv".to_owned(),
            nodes: vec![
                VObjectInfo {
                    line: 2,
                    col: 9,
                    end_line: 2,
                    end_col: 11,
                    vpi_type: llg::ffi::vpi::uhdmmodule_inst,
                    name: Some("u0".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
                VObjectInfo {
                    line: 6,
                    col: 3,
                    end_line: 6,
                    end_col: 5,
                    vpi_type: llg::ffi::vpi::uhdmmodule_inst,
                    name: Some("tb".to_owned()),
                    file: "/x/top.sv".to_owned(),
                },
            ],
        }];
        (model, tokens)
    }

    #[test]
    fn document_symbols_attach_instance_children_with_type_detail() {
        let (model, tokens) = hierarchy_parts();
        let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
        let syms = document_symbols(&a, "/x/top.sv");

        // `top` instantiates `m` through `u0`: an Object-kind leaf whose
        // detail is the instantiated TYPE, ranged over the refined
        // instance-name token (1-based (2,9) → 0-based (1,8)).
        let top = syms
            .iter()
            .find(|s| s.name == "top" && s.kind == SymbolKind::MODULE)
            .expect("top symbol");
        let children = top.children.as_ref().expect("top children");
        let inst = children
            .iter()
            .find(|c| c.name == "u0")
            .expect("instance child");
        assert_eq!(inst.kind, SymbolKind::OBJECT);
        assert_eq!(inst.detail.as_deref(), Some("m"));
        assert_eq!(inst.range.start, Position::new(1, 8));
        assert_eq!(inst.selection_range, inst.range);
        assert!(inst.children.is_none(), "instances are leaf symbols");

        // `m` itself instantiates nothing: no Object children.
        let m = syms
            .iter()
            .find(|s| s.name == "m" && s.kind == SymbolKind::MODULE)
            .expect("m symbol");
        let m_children = m.children.as_deref().unwrap_or_default();
        assert!(
            !m_children.iter().any(|c| c.kind == SymbolKind::OBJECT),
            "m children: {m_children:?}"
        );
    }

    #[test]
    fn type_only_detail_helpers_degrade_without_guessing() {
        // Trailing-name stripping.
        assert_eq!(
            strip_trailing_name("input logic [31:0] q").as_deref(),
            Some("input logic [31:0]")
        );
        assert_eq!(strip_trailing_name("wire w1").as_deref(), Some("wire"));
        assert_eq!(
            strip_trailing_name("array logic [3:0] mem").as_deref(),
            Some("array logic [3:0]")
        );
        // Degenerate inputs yield nothing instead of a wrong guess.
        assert_eq!(strip_trailing_name(""), None);
        assert_eq!(strip_trailing_name("clk"), None);
        assert_eq!(strip_trailing_name("var w1"), None, "unknown-type marker");
        assert_eq!(strip_trailing_name("logic [7:0]"), None, "no name tail");
        // Parameter detail shapes.
        assert_eq!(
            param_type_detail(Some("parameter W: int = 32'sd8"), None).as_deref(),
            Some("parameter int")
        );
        assert_eq!(
            param_type_detail(Some("localparam DEPTH: int"), None).as_deref(),
            Some("localparam int")
        );
        assert_eq!(
            param_type_detail(Some("localparam S: struct pair_t"), None).as_deref(),
            Some("localparam struct pair_t")
        );
        // Unknown types degrade to the bare keyword; non-parameter shapes to
        // nothing.
        assert_eq!(
            param_type_detail(Some("localparam X: other"), None).as_deref(),
            Some("localparam")
        );
        assert_eq!(param_type_detail(Some("module top"), None), None);
        // A captured snippet wins over the legacy colon shape.
        assert_eq!(
            param_type_detail(Some("parameter W: int = 4"), Some("logic [7:0] W")).as_deref(),
            Some("parameter logic [7:0]")
        );
        // Model-derived fallback texts.
        let port = PortModel {
            name: "bus".to_owned(),
            direction: Direction::Inout,
            ty: TypeInfo {
                kind: "other".to_owned(),
                width: None,
                signed: false,
                type_name: None,
            },
        };
        assert_eq!(port_type_only(&port).as_deref(), Some("inout"));
        let sig = SignalModel {
            name: "mem".to_owned(),
            kind: "array".to_owned(),
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(8),
                signed: false,
                type_name: None,
            },
        };
        assert_eq!(signal_type_only(&sig).as_deref(), Some("array logic [7:0]"));
    }

    #[test]
    fn symbolic_dimension_normalization_preserves_token_boundaries() {
        assert_eq!(normalize_symbolic_expression(" P    +    +1 "), "P+ +1");
        assert_eq!(normalize_symbolic_expression(" P    -    -1 "), "P- -1");
        assert_eq!(normalize_symbolic_expression(r" \WIDTH + 1 "), r"\WIDTH +1");
        // The closing bracket is outside the helper's input, so the escaped
        // identifier's terminating separator is retained at the end too.
        assert_eq!(normalize_symbolic_expression(r" \WIDTH "), r"\WIDTH ");
        assert_eq!(
            normalize_symbolic_expression(" P    inside    { BASE , IDX } : 0 "),
            "P inside {BASE,IDX}:0"
        );
        assert_eq!(
            normalize_symbolic_expression(" MODE == \"A  ] B\" : 0 "),
            "MODE==\"A  ] B\":0"
        );
        assert_eq!(normalize_symbolic_expression(" 1 : 0 "), "1:0");
    }

    #[test]
    fn graph_source_index_reuses_comment_and_unicode_line_facts() {
        let source = concat!(
            "module Ω;\r\n",
            "// ignored [bracket]\r\n",
            "logic [7:0] λ /* ignored ; , [ ] */;\r\n",
            "string text = \"// not a comment /* [ ] */\";\r\n",
            r"wire \name//not-comment/*also-not-comment */ ;",
            "\r\nendmodule\r\n",
        );
        let index = GraphSourceIndex::new(source.to_owned());

        assert_eq!(index.source, source);
        assert_eq!(index.masked.len(), source.len());
        assert_eq!(index.stripped, strip_hdl_comments(source));
        assert_eq!(index.comment_ranges.len(), 2);
        assert!(index
            .comment_ranges
            .iter()
            .all(|(start, end)| source[*start..*end].starts_with("//")
                || source[*start..*end].starts_with("/*")));
        assert!(!index.masked.contains("ignored ; , [ ]"));
        assert!(index.masked.contains(r#""// not a comment /* [ ] */""#));
        assert!(index
            .masked
            .contains(r"\name//not-comment/*also-not-comment */"));

        let logic_start = source.find("logic").expect("logic line");
        let lambda_start = source.find('λ').expect("unicode declaration name");
        let lambda_col = "logic [7:0] ".chars().count() as u32 + 1;
        assert_eq!(
            index.line_starts,
            vec![
                0,
                source.find("// ignored").expect("comment line"),
                logic_start,
                source.find("string text").expect("string line"),
                source.find(r"wire \name").expect("escaped identifier line"),
                source.find("endmodule").expect("endmodule line"),
                source.len(),
            ]
        );
        assert_eq!(index.line_start(3), Some(logic_start));
        assert_eq!(index.line_start(7), Some(source.len()));
        assert_eq!(index.line_start(8), None);
        assert_eq!(index.line_text(1), Some("module Ω;"));
        assert!(index
            .line_text(3)
            .is_some_and(|line| line.starts_with("logic [7:0] λ") && !line.contains("ignored")));
        assert_eq!(index.line_text(7), None);
        assert_eq!(
            source_position_offset(&index, 3, lambda_col),
            Some(lambda_start)
        );
        assert_eq!(index.position_offset(3, lambda_col), Some(lambda_start));

        let declaration = llg::ffi::surelog::ParseNode {
            line: 3,
            col: 1,
            end_line: 3,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        };
        let parts = graph_type_prefix(Some(&index), Some(&declaration), 3, lambda_col, "λ")
            .expect("indexed source type");
        assert_eq!(parts.base, "logic");
        assert_eq!(parts.packed_dimensions, vec!["[7:0]"]);
        assert!(parts.unpacked_dimensions.is_empty());

        let detail = graph_declaration_detail(
            Some(&index),
            3,
            lambda_col,
            "λ",
            &TypeInfo {
                kind: "logic".to_owned(),
                width: Some(8),
                signed: false,
                type_name: None,
            },
            &GraphDeclarationKind::Signal("var".to_owned()),
        );
        assert_eq!(detail.as_deref(), Some("logic [7:0] λ"));
    }

    #[test]
    fn graph_source_index_handles_many_same_line_declarations() {
        let count = 2_048;
        let mut source = String::from("logic [7:0] ");
        let mut declarations = Vec::with_capacity(count);
        for index in 0..count {
            if index > 0 {
                source.push_str(", ");
            }
            let name = format!("signal_{index}");
            let col = source.chars().count() as u32 + 1;
            declarations.push((name.clone(), col));
            source.push_str(&name);
        }
        source.push_str(";\n");

        let index = GraphSourceIndex::new(source.clone());
        assert_eq!(index.line_text(1), Some(source.trim_end_matches('\n')));
        assert_eq!(
            index.source_lines[0].character_count,
            source.trim_end_matches('\n').chars().count()
        );
        assert!(index.source_lines[0].character_checkpoints.is_empty());
        assert!(index.stripped_lines[0].character_checkpoints.is_empty());
        for (name, col) in declarations {
            let offset = index
                .position_offset(1, col)
                .expect("same-line declaration position");
            assert_eq!(&source[offset..offset + name.len()], name);
            let detail = graph_declaration_detail(
                Some(&index),
                1,
                col,
                &name,
                &TypeInfo {
                    kind: "logic".to_owned(),
                    width: Some(8),
                    signed: false,
                    type_name: None,
                },
                &GraphDeclarationKind::Signal("var".to_owned()),
            )
            .expect("same-line declaration detail");
            assert!(detail.contains(&name), "detail {detail:?} misses {name}");
        }
    }

    #[test]
    fn graph_source_index_keeps_ascii_metadata_sparse_at_scale() {
        let source = "x".repeat(8 * 1024 * 1024);
        let index = GraphSourceIndex::new(source.clone());

        assert_eq!(index.source_lines.len(), 1);
        assert_eq!(index.stripped_lines.len(), 1);
        assert_eq!(index.source_lines[0].character_count, source.len());
        assert_eq!(index.stripped_lines[0].character_count, source.len());
        assert!(
            index.source_lines[0].character_checkpoints.is_empty(),
            "ASCII source must not allocate one offset per character"
        );
        assert!(
            index.stripped_lines[0].character_checkpoints.is_empty(),
            "ASCII stripped source must not allocate one offset per character"
        );
    }

    #[test]
    fn graph_declaration_facts_are_cached_per_root_for_many_names() {
        use llg::core::vobject_types::VObjectType;

        const NAME_COUNT: usize = 512;
        let mut nodes = vec![llg::ffi::surelog::ParseNode {
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        }];
        nodes.push(llg::ffi::surelog::ParseNode {
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
            type_id: VObjectType::paData_declaration as u16,
            file_id: 1,
            parent_index: 0,
            child_index: 2,
            sibling_index: 0,
            symbol_name: None,
        });
        for index in 0..NAME_COUNT {
            nodes.push(llg::ffi::surelog::ParseNode {
                line: 1,
                col: (index + 2) as u16,
                end_line: 1,
                end_col: (index + 3) as u16,
                type_id: VObjectType::slStringConst as u16,
                file_id: 1,
                parent_index: 1,
                child_index: 0,
                sibling_index: if index + 1 == NAME_COUNT {
                    0
                } else {
                    (index + 3) as u32
                },
                symbol_name: Some(format!("name_{index}")),
            });
        }

        let mut cache = GraphDeclarationFactsCache::default();
        for index in 2..nodes.len() {
            let (root, kind) = graph_declaration_kind(&nodes, index, &mut cache)
                .expect("each declarator belongs to the data declaration");
            assert_eq!(root, 1);
            assert_eq!(kind, GraphDeclarationKind::Signal("var".to_owned()));
        }
        assert_eq!(cache.by_root.len(), 1);
        assert_eq!(cache.subtree_walks, 1);
    }

    #[test]
    fn graph_fallback_module_lookup_uses_keyed_line_ranges() {
        let count = 2_048;
        let definitions = (0..count)
            .map(|index| ModuleGraphDefinition {
                id: format!("module-{index}"),
                name: format!("module_{index}"),
                file: Some("/x/modules.sv".to_owned()),
                line: (index * 2 + 1) as u32,
                col: 1,
                end_line: (index * 2 + 1) as u32,
                end_col: 1,
                ports: Vec::new(),
                params: Vec::new(),
                signals: Vec::new(),
                children: Vec::new(),
                generated_scopes: Vec::new(),
            })
            .collect::<Vec<_>>();
        let ranges = graph_definition_line_ranges(&definitions);
        for index in 0..count {
            let name = format!("module_{index}");
            let line = (index * 2 + 1) as u32;
            assert!(graph_definition_line_is_retained(
                &ranges,
                "/x/modules.sv",
                &name,
                line
            ));
            assert!(!graph_definition_line_is_retained(
                &ranges,
                "/x/modules.sv",
                &name,
                line + 1
            ));
        }
        assert!(!graph_definition_line_is_retained(
            &ranges,
            "/x/other.sv",
            "module_0",
            1
        ));
    }

    #[test]
    fn graph_declaration_boundary_index_matches_reverse_fallback() {
        fn reverse_boundary(text: &str, name_start: usize) -> Option<usize> {
            let prefix = text.get(..name_start)?;
            let mut square = 0usize;
            let mut paren = 0usize;
            let mut brace = 0usize;
            for (offset, character) in prefix.char_indices().rev() {
                match character {
                    ']' => square += 1,
                    '[' => square = square.saturating_sub(1),
                    ')' => paren += 1,
                    '(' if paren > 0 => paren -= 1,
                    '(' if square == 0 && brace == 0 => return Some(offset + 1),
                    '}' => brace += 1,
                    '{' => brace = brace.saturating_sub(1),
                    ';' if square == 0 && paren == 0 && brace == 0 => return Some(offset + 1),
                    _ => {}
                }
            }
            Some(0)
        }

        let source = concat!(
            "module m #(parameter int W) (input logic p);\r\n",
            "logic [7:0] value; /* ignored ; ( ) */\r\n",
            "always @ (value) begin\r\n",
            "  value = value;\r\n",
            "end\r\nendmodule\r\n",
        );
        let index = GraphSourceIndex::new(source.to_owned());
        let mut offsets = source
            .char_indices()
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        offsets.push(source.len());
        for offset in offsets {
            assert_eq!(
                graph_source_declaration_start(&index, offset),
                reverse_boundary(&index.masked, offset),
                "boundary at byte offset {offset}"
            );
        }
    }

    #[test]
    fn graph_declaration_boundaries_ignore_quoted_and_escaped_delimiters() {
        let source = concat!(
            "module m;\r\n",
            "// ignored ( [ { ; , ] } )\r\n",
            "logic quote = \"( [ { ; , ) ] }\";\r\n",
            r"logic \escaped([;,{)]} name;",
            "\r\n",
            "logic broken( [7:0] later;\r\n",
            "logic after;\r\n",
            "endmodule\r\n",
        );
        let index = GraphSourceIndex::new(source.to_owned());
        let events = graph_declaration_boundary_events(&index.masked);

        let module_end = source.find("module m;").expect("module") + "module m;".len();
        let quote_name = source.find("quote").expect("quoted initializer");
        let escaped_name = source.find(r"\escaped").expect("escaped identifier");
        let escaped_tail = source.find("name;").expect("escaped identifier tail");
        let string_start = source.find('"').expect("string");
        let string_end =
            string_start + 1 + source[string_start + 1..].find('"').expect("string end");
        let quote_semicolon = string_end + source[string_end..].find(';').expect("quote semicolon");
        assert_eq!(
            graph_source_declaration_start(&index, quote_name),
            Some(module_end)
        );
        assert_eq!(
            graph_source_declaration_start(&index, escaped_name),
            Some(quote_semicolon + 1)
        );
        assert_eq!(
            graph_source_declaration_start(&index, escaped_tail),
            Some(quote_semicolon + 1)
        );

        let broken_start = source.find("logic broken").expect("malformed declaration");
        let broken_open = broken_start + source[broken_start..].find('(').expect("open paren");
        let later_name = source.find("later").expect("later declaration token");
        let broken_semicolon = later_name + source[later_name..].find(';').expect("semicolon");
        let after_name = source.find("after").expect("later declaration");
        assert_eq!(
            graph_source_declaration_start(&index, later_name),
            Some(broken_open + 1),
            "a balanced bracket inside the malformed parenthesized clause is local"
        );
        assert_eq!(
            graph_source_declaration_start(&index, after_name),
            Some(broken_semicolon + 1),
            "an unmatched opener must not poison later declarations"
        );

        assert!(events
            .iter()
            .all(|(offset, _)| !(*offset >= string_start && *offset < string_end)));
        let escaped_start = escaped_name;
        let escaped_end = escaped_tail;
        assert!(events
            .iter()
            .all(|(offset, _)| !(*offset >= escaped_start && *offset < escaped_end)));
        assert!(index
            .line_starts
            .windows(2)
            .any(|pair| pair[1] > pair[0] && &source[pair[1] - 2..pair[1]] == "\r\n"));
    }

    #[test]
    fn graph_source_index_reuses_top_level_comma_facts_at_scale() {
        fn source_with_declarations(count: usize) -> String {
            let mut source = String::from(
                "module m #(parameter int P = 1, parameter int Q = 2) (input logic p, q);\r\n",
            );
            for index in 0..count {
                source.push_str(&format!(
                    "logic [7:0] first_{index}, second_{index} = 1;\r\n"
                ));
            }
            source.push_str("endmodule\r\n");
            source
        }

        fn assert_indexed_declarations(source: &str, count: usize) {
            let index = GraphSourceIndex::new(source.to_owned());
            let zero_state = GraphDelimiterState::default();
            assert_eq!(
                index.commas_by_state.get(&zero_state).map_or(0, Vec::len),
                count,
                "each declaration comma is indexed once at its nesting state"
            );
            let header_state = GraphDelimiterState {
                paren: 1,
                ..GraphDelimiterState::default()
            };
            assert_eq!(
                index.commas_by_state.get(&header_state).map_or(0, Vec::len),
                2,
                "ANSI parameter and port separators share the parenthesis state"
            );

            for declaration_index in 0..count {
                let marker = format!("logic [7:0] first_{declaration_index}");
                let start = source.find(&marker).expect("declaration marker");
                let end = start + source[start..].find(';').expect("declaration semicolon");
                let comma = start + source[start..end].find(',').expect("declarator comma");
                assert_eq!(
                    graph_top_level_commas_index(&index, start, end),
                    (Some(comma), Some(comma))
                );
                let (type_start, type_end) = graph_select_type_prefix_index(&index, start, end);
                assert_eq!(&source[type_start..type_end], "logic [7:0]");
            }
        }

        let n = 32;
        let source_n = source_with_declarations(n);
        let source_2n = source_with_declarations(2 * n);
        assert_indexed_declarations(&source_n, n);
        assert_indexed_declarations(&source_2n, 2 * n);
    }

    #[test]
    fn parse_fallback_indexes_declarations_and_actuals_at_scale() {
        fn fixture(
            count: usize,
        ) -> (
            Vec<FileTokens>,
            ParseDeclPositions,
            Vec<ModuleDef>,
            String,
            u32,
        ) {
            let file = "/x/fallback.sv".to_owned();
            let child_end = (2 * count + 1) as u32;
            let parent_line = child_end + 1;
            let parent_end = parent_line + count as u32 + 1;
            let mut nodes = Vec::with_capacity(3 * count);
            let mut declarations = ParseDeclPositions::new();
            let mut add = |line: u32, col: u32, vpi_type: i32, name: String| {
                declarations.insert((file.clone(), line, col));
                nodes.push(VObjectInfo {
                    line,
                    col,
                    end_line: line,
                    end_col: col + name.chars().count() as u32,
                    vpi_type,
                    name: Some(name),
                    file: file.clone(),
                });
            };

            for index in 0..count {
                add(
                    2 + index as u32,
                    3,
                    llg::ffi::vpi::vpiPort,
                    format!("port_{index}"),
                );
                add(
                    count as u32 + 2 + index as u32,
                    5,
                    llg::ffi::vpi::vpiParameter,
                    format!("PARAM_{index}"),
                );
                add(
                    parent_line + 1 + index as u32,
                    7,
                    llg::ffi::vpi::vpiNet,
                    format!("signal_{index}"),
                );
            }

            let tokens = vec![FileTokens {
                path: file.clone(),
                nodes,
            }];
            let modules = vec![
                ModuleDef {
                    name: "child".to_owned(),
                    file: Some(file.clone()),
                    line: 1,
                    col: 1,
                    end_line: child_end,
                    end_col: 1,
                },
                ModuleDef {
                    name: "parent".to_owned(),
                    file: Some(file.clone()),
                    line: parent_line,
                    col: 1,
                    end_line: parent_end,
                    end_col: 1,
                },
            ];
            (tokens, declarations, modules, file, parent_line - 1)
        }

        for count in [16, 32] {
            let (tokens, declarations, modules, file, parent_line0) = fixture(count);
            let index = ParseFallbackIndex::new(&tokens, &declarations, &modules);
            let ports = declared_ports_by_module(&index);
            let params = declared_params_by_module(&index);
            assert_eq!(ports.len(), count);
            assert_eq!(params.len(), count);
            assert_eq!(
                index
                    .files
                    .get(file.as_str())
                    .expect("file index")
                    .actual_positions_by_name
                    .len(),
                3 * count
            );

            for declaration_index in 0..count {
                let port_name = format!("port_{declaration_index}");
                let param_name = format!("PARAM_{declaration_index}");
                assert_eq!(
                    find_fallback_port(ports, "child", &port_name)
                        .map(|decl| (decl.line1, decl.col1)),
                    Some((2 + declaration_index as u32, 3))
                );
                assert_eq!(
                    find_fallback_param(params, "child", &param_name)
                        .map(|decl| (decl.line1, decl.col1)),
                    Some((count as u32 + 2 + declaration_index as u32, 5))
                );
                let actual_name = format!("signal_{declaration_index}");
                let target = fallback_actual_target(
                    &index,
                    &file,
                    parent_line0 + declaration_index as u32 + 1,
                    &actual_name,
                )
                .expect("parent actual target");
                assert_eq!(
                    (target.line0, target.col0, target.kind.as_str()),
                    (parent_line0 + declaration_index as u32 + 1, 6, "net")
                );
            }
        }
    }

    #[test]
    fn parse_fallback_actual_kind_matches_duplicate_position_name() {
        let file = "/x/fallback-duplicate.sv".to_owned();
        let tokens = vec![FileTokens {
            path: file.clone(),
            nodes: vec![
                VObjectInfo {
                    line: 4,
                    col: 2,
                    end_line: 4,
                    end_col: 7,
                    vpi_type: llg::ffi::vpi::vpiNet,
                    name: Some("decoy".to_owned()),
                    file: file.clone(),
                },
                VObjectInfo {
                    line: 4,
                    col: 2,
                    end_line: 4,
                    end_col: 8,
                    vpi_type: llg::ffi::vpi::vpiParameter,
                    name: Some("actual".to_owned()),
                    file: file.clone(),
                },
            ],
        }];
        let mut declarations = ParseDeclPositions::new();
        declarations.insert((file.clone(), 4, 2));
        let modules = vec![ModuleDef {
            name: "parent".to_owned(),
            file: Some(file.clone()),
            line: 1,
            col: 1,
            end_line: 10,
            end_col: 1,
        }];
        let index = ParseFallbackIndex::new(&tokens, &declarations, &modules);

        let target = fallback_actual_target(&index, &file, 5, "actual").expect("actual target");
        assert_eq!(target.kind, "parameter");
        assert_eq!((target.line0, target.col0), (3, 1));
    }

    #[test]
    fn graph_display_type_preserves_qualified_port_types() {
        let ty = TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        };
        for (prefix, expected) in [
            ("input ref logic", "ref logic"),
            ("input const ref logic", "const ref logic"),
            ("input var logic", "var logic"),
            ("input buffer logic", "buffer logic"),
            ("input linkage signed bus_t", "linkage signed bus_t"),
        ] {
            let source = format!("{prefix} p;");
            let source_index = GraphSourceIndex::new(source.clone());
            let name_col = source.find("p;").expect("port name") as u32 + 1;
            let declaration = llg::ffi::surelog::ParseNode {
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 1,
                type_id: 0,
                file_id: 0,
                parent_index: 0,
                child_index: 0,
                sibling_index: 0,
                symbol_name: None,
            };
            let display = graph_type_display(
                Some(&source_index),
                &declaration,
                1,
                name_col,
                "p",
                &ty,
                &GraphDeclarationKind::Port(Direction::Input),
                None,
            );
            assert_eq!(display.text.as_deref(), Some(expected), "{prefix}");
        }
    }

    #[test]
    fn graph_display_type_preserves_brackets_inside_quoted_dimension() {
        let source = r#"logic [MODE == "A ] B" ? 7 : 3:0] payload;"#;
        let source_index = GraphSourceIndex::new(source.to_owned());
        let name_col = source.find("payload").expect("payload name") as u32 + 1;
        let declaration = llg::ffi::surelog::ParseNode {
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        };
        let display = graph_type_display(
            Some(&source_index),
            &declaration,
            1,
            name_col,
            "payload",
            &TypeInfo {
                kind: "logic".to_owned(),
                width: None,
                signed: false,
                type_name: None,
            },
            &GraphDeclarationKind::Signal("var".to_owned()),
            None,
        );

        assert_eq!(display.shape.packed_dimensions, 1);
        assert_eq!(display.shape.unpacked_dimensions, 0);
        assert_eq!(
            display.text.as_deref(),
            Some(r#"logic [MODE=="A ] B"?7:3:0]"#)
        );
    }

    #[test]
    fn graph_display_type_preserves_unpacked_dimension_operators() {
        let source = r#"logic [1:0] payload [MODE == "A ] B" ? 1 : 0];"#;
        let source_index = GraphSourceIndex::new(source.to_owned());
        let name_col = source.find("payload").expect("payload name") as u32 + 1;
        let declaration = llg::ffi::surelog::ParseNode {
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        };
        let display = graph_type_display(
            Some(&source_index),
            &declaration,
            1,
            name_col,
            "payload",
            &TypeInfo {
                kind: "logic".to_owned(),
                width: None,
                signed: false,
                type_name: None,
            },
            &GraphDeclarationKind::Signal("var".to_owned()),
            None,
        );

        assert_eq!(display.shape.packed_dimensions, 1);
        assert_eq!(display.shape.unpacked_dimensions, 1);
        assert_eq!(
            display.text.as_deref(),
            Some(r#"logic [1:0] [MODE=="A ] B"?1:0]"#)
        );
    }

    #[test]
    fn graph_bracket_dimensions_ignore_brackets_inside_escaped_identifiers() {
        let source = r"logic [\MODE[A]B == 7 ? 3 : 0] payload;";
        let expected_start = source.find('[').expect("dimension start");
        let expected_end = source.find("] payload").expect("dimension end") + 1;

        assert_eq!(
            graph_bracket_spans(source),
            vec![(
                expected_start,
                expected_end,
                r"\MODE[A]B == 7 ? 3 : 0".to_owned()
            )]
        );
        assert_eq!(
            graph_bracket_dimensions(source),
            vec![r"[\MODE[A]B ==7?3:0]".to_owned()]
        );
    }

    #[test]
    fn graph_bracket_dimensions_preserve_comments_and_active_tokens() {
        let line_source = "logic [P // ignored ]\n + 1:0] payload;";
        let line_source_index = GraphSourceIndex::new(line_source.to_owned());
        let line_spans = graph_bracket_spans(line_source);
        assert_eq!(line_spans.len(), 1);
        assert_eq!(line_spans[0].2, "P // ignored ]\n + 1:0");
        assert_eq!(
            graph_bracket_dimensions(line_source),
            vec!["[P +1:0]".to_owned()]
        );

        let block_source = "logic [P /* ignored ] */ + 1:0] payload;";
        let block_spans = graph_bracket_spans(block_source);
        assert_eq!(block_spans.len(), 1);
        assert_eq!(block_spans[0].2, "P /* ignored ] */ + 1:0");
        assert!(block_spans[0].2.contains("+ 1:0"));
        assert_eq!(
            graph_bracket_dimensions(block_source),
            vec!["[P +1:0]".to_owned()]
        );

        let comments_between_operators = "P /* left */ + /* right */ + 1";
        assert_eq!(
            normalize_symbolic_expression(comments_between_operators),
            "P + +1"
        );
        assert_eq!(
            normalize_graph_type_display("logic [P /* left */ + /* right */ + 1:0]"),
            "logic [P + +1:0]"
        );
        assert_eq!(
            normalize_graph_type_display("logic [ 1 : 0 ]"),
            "logic [1:0]"
        );

        let declaration = llg::ffi::surelog::ParseNode {
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        };
        let name_col = 9;
        let display = graph_type_display(
            Some(&line_source_index),
            &declaration,
            2,
            name_col,
            "payload",
            &TypeInfo {
                kind: "logic".to_owned(),
                width: None,
                signed: false,
                type_name: None,
            },
            &GraphDeclarationKind::Signal("var".to_owned()),
            None,
        );
        assert_eq!(display.shape.packed_dimensions, 1);
        assert_eq!(display.text.as_deref(), Some("logic [P +1:0]"));
    }

    #[test]
    fn graph_display_ignores_leading_line_comment_before_wire() {
        let source = "module Foo();\n\n// This is a line comment\nwire start;\n\nendmodule";
        let source_index = GraphSourceIndex::new(source.to_owned());
        let declaration = llg::ffi::surelog::ParseNode {
            line: 3,
            col: 1,
            end_line: 3,
            end_col: 1,
            type_id: 0,
            file_id: 0,
            parent_index: 0,
            child_index: 0,
            sibling_index: 0,
            symbol_name: None,
        };
        let display = graph_type_display(
            Some(&source_index),
            &declaration,
            4,
            6,
            "start",
            &TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
            &GraphDeclarationKind::Signal("wire".to_owned()),
            None,
        );

        assert_eq!(display.text.as_deref(), Some("wire"));
        assert!(!display
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("This is a line comment"));
    }

    #[test]
    fn graph_type_words_ignore_comment_tokens_and_preserve_quoted_text() {
        let block_comment = "/* int */ wire start";
        let (ty, saw_decl_qualifier) = graph_source_type_words(block_comment);
        assert_eq!(ty.kind, "logic");
        assert!(saw_decl_qualifier);
        assert_eq!(normalize_graph_type_display(block_comment), "wire start");

        let comment_only_type = "/* int */ p";
        let (comment_only_ty, comment_only_qualifier) = graph_source_type_words(comment_only_type);
        assert_eq!(comment_only_ty.kind, "other");
        assert!(!comment_only_qualifier);

        let quoted = r#"logic "http://x /* int */""#;
        assert!(graph_comment_ranges(quoted).is_empty());
        assert_eq!(strip_hdl_comments(quoted), quoted);
        let (quoted_ty, _) = graph_source_type_words(quoted);
        assert_eq!(quoted_ty.kind, "logic");

        let escaped = r"wire \int//not_a_comment/*also_not_a_comment ";
        assert!(graph_comment_ranges(escaped).is_empty());
        assert_eq!(strip_hdl_comments(escaped), escaped);
        let (escaped_ty, escaped_qualifier) = graph_source_type_words(escaped);
        assert_eq!(escaped_ty.kind, "logic");
        assert!(escaped_qualifier);
    }

    #[test]
    fn malformed_sibling_links_terminate_subtree_traversal() {
        fn node(child_index: u32, sibling_index: u32) -> llg::ffi::surelog::ParseNode {
            llg::ffi::surelog::ParseNode {
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 1,
                type_id: 0,
                file_id: 0,
                parent_index: 0,
                child_index,
                sibling_index,
                symbol_name: None,
            }
        }

        let self_link = vec![node(1, 0), node(0, 1)];
        assert_eq!(graph_subtree_indices(&self_link, 0), vec![0, 1]);

        let cyclic_links = vec![node(1, 0), node(0, 2), node(0, 1)];
        assert_eq!(graph_subtree_indices(&cyclic_links, 0), vec![0, 1, 2]);
    }

    #[test]
    fn completion_filters_by_prefix() {
        let a = sample_analysis();
        let items = completion_at(&a, "/x/top.sv", 0, 3, "mod");
        assert!(
            items.iter().any(|i| i.label == "module"),
            "items: {items:?}"
        );
        let all = completion_at(&a, "/x/top.sv", 0, 0, "");
        assert!(
            all.iter()
                .any(|i| i.label == "m" && i.kind == Some(CompletionItemKind::MODULE)),
            "items: {all:?}"
        );
    }

    #[test]
    fn completion_includes_function_and_task_names() {
        let a = sample_analysis();
        let all = completion_at(&a, "/x/top.sv", 0, 0, "");
        assert!(
            all.iter()
                .any(|i| i.label == "add" && i.kind == Some(CompletionItemKind::FUNCTION)),
            "items: {all:?}"
        );
        assert!(
            all.iter()
                .any(|i| i.label == "run" && i.kind == Some(CompletionItemKind::FUNCTION)),
            "items: {all:?}"
        );
        // Prefix filtering applies to function candidates too.
        let pre = completion_at(&a, "/x/top.sv", 0, 2, "ad");
        assert!(pre.iter().any(|i| i.label == "add"), "items: {pre:?}");
        // The model and index-backed sources must not double-list a function.
        assert_eq!(all.iter().filter(|i| i.label == "add").count(), 1);
    }

    #[test]
    fn diagnostics_severity_mapping() {
        let a = Analysis::new(
            vec![
                Diag {
                    severity: Severity::Error,
                    file: Some("/x/a.sv".to_owned()),
                    line: 3,
                    col: 5,
                    message: "bad".to_owned(),
                },
                Diag {
                    severity: Severity::Warning,
                    file: Some("/x/a.sv".to_owned()),
                    line: 4,
                    col: 1,
                    message: "warn".to_owned(),
                },
                Diag {
                    severity: Severity::Note,
                    file: Some("/x/a.sv".to_owned()),
                    line: 0,
                    col: 0,
                    message: "note".to_owned(),
                },
                Diag {
                    severity: Severity::Info,
                    file: Some("/x/a.sv".to_owned()),
                    line: 6,
                    col: 2,
                    message: "info".to_owned(),
                },
            ],
            empty_design(),
            Vec::new(),
            Vec::new(),
        );
        let map = lsp_diagnostics(&a);
        let diags = map.get("/x/a.sv").expect("diags for a.sv");
        assert_eq!(diags.len(), 4);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].range.start.line, 2); // 1-based → 0-based
        assert_eq!(diags[0].range.start.character, 4);
        assert_eq!(diags[1].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diags[2].severity, Some(DiagnosticSeverity::INFORMATION));
        assert_eq!(diags[2].range.start.line, 0); // unknown line → (0,0)
        assert_eq!(diags[3].severity, Some(DiagnosticSeverity::HINT));
    }

    #[test]
    fn fileless_synthetic_diagnostic_uses_supplied_fallback_path() {
        let message = "UHDM database build failed: node walk failed";
        let a = Analysis::new(
            vec![db_build_diagnostic("node walk failed")],
            empty_design(),
            Vec::new(),
            Vec::new(),
        );

        let map = lsp_diagnostics_with_fallback(&a, Some(Path::new("/x/top.sv")));
        let diagnostics = map.get("/x/top.sv").expect("fallback-file diagnostics");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, message);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn lint_diagnostics_mapping() {
        use llg::core::lint::{LintDiag, LintSeverity};
        let a = Analysis::new(
            vec![Diag {
                severity: Severity::Error,
                file: Some("/x/a.sv".to_owned()),
                line: 1,
                col: 1,
                message: "compile error".to_owned(),
            }],
            empty_design(),
            Vec::new(),
            vec![
                LintDiag {
                    rule: "unused-signal".to_owned(),
                    severity: LintSeverity::Error,
                    file: Some("/x/a.sv".to_owned()),
                    line: 3,
                    col: 5,
                    message: "signal `x` in `m` is never used".to_owned(),
                },
                LintDiag {
                    rule: "width-mismatch".to_owned(),
                    severity: LintSeverity::Warning,
                    file: Some("/x/a.sv".to_owned()),
                    line: 4,
                    col: 1,
                    message: "truncation".to_owned(),
                },
                LintDiag {
                    rule: "multi-driver".to_owned(),
                    severity: LintSeverity::Info,
                    file: Some("/x/b.sv".to_owned()),
                    line: 0,
                    col: 0,
                    message: "multiple drivers".to_owned(),
                },
            ],
        );
        let map = lsp_diagnostics(&a);
        let a_diags = map.get("/x/a.sv").expect("diags for a.sv");
        assert_eq!(a_diags.len(), 3, "surelog + two lint diags: {a_diags:?}");
        let lint: Vec<&LspDiagnostic> = a_diags
            .iter()
            .filter(|d| d.source.as_deref() == Some("llg-lint"))
            .collect();
        assert_eq!(lint.len(), 2, "lint diags: {lint:?}");
        assert_eq!(lint[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            lint[0].code,
            Some(NumberOrString::String("unused-signal".to_owned()))
        );
        assert_eq!(lint[0].range.start.line, 2); // 1-based → 0-based
        assert_eq!(lint[0].range.start.character, 4);
        assert!(lint[0].message.contains("never used"));
        assert_eq!(lint[1].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            lint[1].code,
            Some(NumberOrString::String("width-mismatch".to_owned()))
        );
        assert_eq!(lint[1].range.start.line, 3);
        assert_eq!(lint[1].range.start.character, 0);
        // /x/b.sv has only a lint finding; it must still get a map entry.
        let b_diags = map.get("/x/b.sv").expect("diags for b.sv");
        assert_eq!(b_diags.len(), 1);
        assert_eq!(b_diags[0].source.as_deref(), Some("llg-lint"));
        assert_eq!(b_diags[0].severity, Some(DiagnosticSeverity::INFORMATION));
        assert_eq!(b_diags[0].range.start, Position::new(0, 0)); // unknown line
    }

    #[test]
    fn semantic_tokens_matched_by_name_fallback() {
        let a = sample_analysis();
        let tokens = semantic_tokens_for(&a, "/symlink/top.sv");
        assert!(!tokens.data.is_empty());
    }

    #[test]
    fn semantic_tokens_are_empty_for_a_file_with_a_syntax_error() {
        // Arrange
        let mut analysis = sample_analysis();
        analysis.diagnostics.push(Diag {
            severity: Severity::Syntax,
            file: Some("/x/top.sv".to_owned()),
            line: 1,
            col: 1,
            message: "incomplete module".to_owned(),
        });

        // Act
        let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

        // Assert
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn semantic_tokens_remain_available_when_another_file_has_a_syntax_error() {
        // Arrange
        let mut analysis = sample_analysis();
        analysis.diagnostics.push(Diag {
            severity: Severity::Syntax,
            file: Some("/other/top.sv".to_owned()),
            line: 1,
            col: 1,
            message: "incomplete module".to_owned(),
        });

        // Act
        let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

        // Assert
        assert!(!tokens.data.is_empty());
    }

    /// Exercises the full compile+model+tokens pipeline against a checked-in
    /// SystemVerilog file.  Skips gracefully when the file is missing.
    #[test]
    fn analyze_full_pipeline_on_params() {
        let _guards = analysis_guards();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elaboration/params.sv");
        if !path.exists() {
            return;
        }
        let path_str = path.to_string_lossy().into_owned();
        let opts = CompileOpts {
            files: vec![path_str.clone()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        assert!(!a.model.top_instances.is_empty(), "expected top instances");
        assert!(
            a.tokens
                .iter()
                .any(|ft| ft.path == path_str || ft.path.ends_with("params.sv")),
            "expected tokens for params.sv"
        );
        let file = a
            .model
            .modules
            .iter()
            .find(|m| m.file.as_deref().is_some_and(|f| f.ends_with("params.sv")))
            .and_then(|m| m.file.clone())
            .unwrap_or_else(|| path_str.clone());
        let syms = document_symbols(&a, &file);
        assert!(
            syms.iter()
                .any(|s| s.name == "param_top" || s.name == "param_child"),
            "syms: {syms:?}"
        );
    }

    /// Full compile of a design whose module declares a function and a task,
    /// instantiated once: the model must carry per-instance clones and the LSP
    /// features must surface them (signature hover, document symbols,
    /// completion).  Runs in a fresh temp dir (Surelog writes `slpp_all/` into
    /// the CWD).
    #[test]
    fn analyze_full_pipeline_extracts_funcs() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_funcs_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let sv = dir.join("funcs_fixture.sv");
        std::fs::write(
            &sv,
            "module calc;\n\
             function automatic int add(input int a, input int b);\n\
               add = a + b;\n\
             endfunction\n\
             task automatic run(input int n);\n\
               $display(\"%d\", n);\n\
             endtask\n\
             endmodule\n\
             module top;\n\
               calc c0();\n\
             endmodule\n",
        )
        .expect("write design");
        let path = sv.to_string_lossy().into_owned();
        let opts = CompileOpts {
            files: vec![path.clone()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );

        let c0 = a
            .model
            .instance("top.c0")
            .or_else(|| a.model.top_instances.iter().find(|i| i.name == "c0"))
            .expect("c0 instance");
        let add = c0.func("add").expect("add func");
        assert!(!add.is_task);
        assert!(add.automatic);
        assert_eq!(add.ret.as_ref().map(|t| t.kind.as_str()), Some("int"));
        assert_eq!(add.args.len(), 2);
        assert_eq!(add.args[0].direction, Direction::Input);
        assert_eq!(add.args[0].name, "a");
        let run = c0.func("run").expect("run task");
        assert!(run.is_task);
        assert_eq!(run.args.len(), 1);

        // Index decl carries the signature; hover at that position shows it
        // plus the instance scope and storage class.
        let decl = a
            .index
            .decls
            .iter()
            .find(|d| d.name == "add" && d.kind == SymKind::Function)
            .expect("add decl");
        assert_eq!(
            decl.detail.as_deref(),
            Some("function int add(input int a, input int b)")
        );
        let hover = hover_at(&a, &path, decl.line, decl.col).expect("hover on add");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(
            value.contains("function int add(input int a, input int b)"),
            "value: {value}"
        );
        assert!(value.contains("top.c0"), "scope missing: {value}");
        assert!(
            value.contains("automatic"),
            "storage class missing: {value}"
        );

        // Document symbols and completion include the function.
        let syms = document_symbols(&a, &path);
        assert!(
            syms.iter()
                .any(|s| s.name == "add" && s.kind == SymbolKind::FUNCTION),
            "syms: {syms:?}"
        );
        let items = completion_at(&a, &path, 0, 0, "");
        assert!(
            items
                .iter()
                .any(|i| { i.label == "add" && i.kind == Some(CompletionItemKind::FUNCTION) }),
            "items: {items:?}"
        );
    }

    /// Full compile of a tiny design with a known lint finding: `unused_sig`
    /// is never read or written, so the `unused-signal` rule (Warning) fires
    /// and must surface as a `llg-lint` diagnostic.  Runs in a fresh temp
    /// dir (Surelog writes `slpp_all/` into the CWD).
    #[test]
    fn analyze_full_pipeline_reports_unused_signal_lint() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let sv = dir.join("lint_fixture.sv");
        std::fs::write(
            &sv,
            "module t;\n  logic used;\n  logic unused_sig;\n  assign used = 1'b0;\nendmodule\n",
        )
        .expect("write design");
        let opts = CompileOpts {
            files: vec![sv.to_string_lossy().into_owned()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected compile diagnostics: {:?}",
            a.diagnostics
        );
        let map = lsp_diagnostics(&a);
        let diags = map
            .iter()
            .find(|(f, _)| f.ends_with("lint_fixture.sv"))
            .map(|(_, v)| v)
            .expect("diagnostics for the fixture file");
        let lint: Vec<&LspDiagnostic> = diags
            .iter()
            .filter(|d| d.source.as_deref() == Some("llg-lint"))
            .collect();
        assert!(
            lint.iter()
                .any(|d| { d.message.contains("unused_sig") && d.message.contains("never used") }),
            "unused-signal finding missing: {lint:?}"
        );
        assert!(
            lint.iter().any(|d| {
                d.code == Some(NumberOrString::String("unused-signal".to_owned()))
                    && d.severity == Some(DiagnosticSeverity::WARNING)
            }),
            "unused-signal code/severity missing: {lint:?}"
        );
    }

    /// Full pipeline over a syntax-broken project: one clean unit plus one
    /// file with a real syntax error (an unterminated module).  Surelog skips
    /// its whole compile/UHDM stage on any syntax error, so this exercises
    /// the parse-tree fallback: the outcome stays Parse with unchanged
    /// diagnostics, but `has_feature_data()` holds and declaration-level
    /// navigation serves from the parse tree.
    #[test]
    fn analyze_syntax_broken_project_serves_parse_tree_declarations() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_pfb_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let clean_sv = dir.join("fb_clean.sv");
        std::fs::write(
            &clean_sv,
            "module fb_clean(input logic clk);\n  logic value;\nendmodule\n",
        )
        .expect("write clean design");
        let broken_sv = dir.join("fb_broken.sv");
        std::fs::write(&broken_sv, "module fb_broken;\n  assign broken_sig = ;\n")
            .expect("write unterminated design");
        let opts = CompileOpts {
            files: vec![
                clean_sv.to_string_lossy().into_owned(),
                broken_sv.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);

        // Diagnostics are unchanged: the syntax error is reported and the
        // outcome stays Parse.
        assert!(
            a.diagnostics
                .iter()
                .any(|d| matches!(d.severity, Severity::Syntax)),
            "expected a syntax diagnostic: {:?}",
            a.diagnostics
        );
        assert!(!a
            .diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Fatal)));
        assert_eq!(a.outcome, AnalysisOutcome::Parse);
        assert!(a.has_feature_data(), "fallback must carry feature data");

        // Modules-only model synthesized from the parse tree; no instances.
        // Even the unterminated module keeps its header through Surelog's
        // parse-error recovery, so both declarations are covered.
        assert!(
            !a.model.modules.is_empty(),
            "modules: {:?}",
            a.model.modules
        );
        assert!(a.model.modules.iter().any(|m| m.name == "fb_clean"));
        assert!(a.model.modules.iter().any(|m| m.name == "fb_broken"));
        assert!(
            a.model
                .modules
                .iter()
                .all(|m| m.file.as_deref().is_some_and(|f| !f.is_empty()) && m.line > 0),
            "module decl positions: {:?}",
            a.model.modules
        );

        // Tokens come from the parse tree (keywords at minimum).
        assert!(
            a.tokens.iter().any(|ft| {
                ft.path.ends_with("fb_clean.sv")
                    && ft.nodes.iter().any(|n| n.name.as_deref() == Some("module"))
            }),
            "parse tokens missing: {:?}",
            a.tokens
        );

        // The declared module is navigable: index decl + workspace symbol +
        // hover on the declaration name.
        let clean_path = clean_sv.to_string_lossy().into_owned();
        let decl = a
            .index
            .decls
            .iter()
            .find(|d| d.kind == SymKind::Module && d.name == "fb_clean")
            .expect("fb_clean decl in index");
        assert_eq!(decl.file, clean_path);
        let syms = workspace_symbols(&a, "fb_clean");
        assert!(
            syms.iter().any(|s| s.name == "fb_clean"),
            "workspace symbols: {syms:?}"
        );
        // The broken file's declaration serves too (parse-error recovery
        // keeps its module header).
        assert!(
            workspace_symbols(&a, "fb_broken")
                .iter()
                .any(|s| s.name == "fb_broken"),
            "workspace symbols for the broken unit: {syms:?}"
        );
        let hover = hover_at(&a, &clean_path, decl.line, decl.col).expect("hover on fb_clean");
        match hover.contents {
            HoverContents::Markup(m) => {
                assert!(m.value.contains("module fb_clean"), "value: {}", m.value)
            }
            _ => panic!("expected markup hover"),
        }
        // Document symbols expose the module for its file even though no
        // elaborated instance data exists.
        let doc = document_symbols(&a, &clean_path);
        assert!(
            doc.iter()
                .any(|s| s.name == "fb_clean" && s.kind == SymbolKind::MODULE),
            "document symbols: {doc:?}"
        );

        cleanup_process_shadow();
    }

    /// Build an `LSPAny::Object` from string-keyed entries.  The settings
    /// payloads in these tests are hand-built because the bin has no direct
    /// `serde_json` dependency (only the `lsp_types` aliases are nameable).
    fn settings_obj(entries: Vec<(&str, LSPAny)>) -> LSPAny {
        let mut map = LSPObject::new();
        for (k, v) in entries {
            map.insert(k.to_owned(), v);
        }
        LSPAny::Object(map)
    }

    /// `analyze_with_config` honors a per-rule `enabled: false`: the fixture
    /// that produces an `unused-signal` finding under the default config is
    /// quiet when the rule is disabled.  Runs in a fresh temp dir (Surelog
    /// writes `slpp_all/` into the CWD).
    #[test]
    fn analyze_with_config_disables_rule() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_cfg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let sv = dir.join("lint_cfg_fixture.sv");
        std::fs::write(
            &sv,
            "module t;\n  logic used;\n  logic unused_sig;\n  assign used = 1'b0;\nendmodule\n",
        )
        .expect("write design");
        let opts = CompileOpts {
            files: vec![sv.to_string_lossy().into_owned()],
            top: None,
            ..Default::default()
        };

        let default = analyze(&opts);
        assert!(
            default.lint.iter().any(|d| d.rule == "unused-signal"),
            "expected unused-signal finding under default config: {:?}",
            default.lint
        );

        let mut cfg = LintConfig::default();
        cfg.set(
            "unused-signal",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        let disabled = analyze_with_config(&opts, &cfg);
        assert!(
            !disabled.lint.iter().any(|d| d.rule == "unused-signal"),
            "unused-signal finding present despite being disabled: {:?}",
            disabled.lint
        );
    }

    /// `analyze_with_config` applies a `severity` override: the
    /// `width-mismatch` extension finding (Info by default) is reported as
    /// Error when configured.  Runs in a fresh temp dir (Surelog writes
    /// `slpp_all/` into the CWD).
    #[test]
    fn analyze_with_config_severity_override() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_sev_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let sv = dir.join("lint_sev_fixture.sv");
        std::fs::write(
            &sv,
            "module wm;\n  logic [3:0] x;\n  logic [7:0] y;\n  assign y = x;\nendmodule\n",
        )
        .expect("write design");
        let opts = CompileOpts {
            files: vec![sv.to_string_lossy().into_owned()],
            top: None,
            ..Default::default()
        };

        let default = analyze(&opts);
        let width = default.lint.iter().find(|d| d.rule == "width-mismatch");
        assert!(
            width.is_some(),
            "expected width-mismatch finding under default config: {:?}",
            default.lint
        );
        assert_eq!(width.expect("width finding").severity, LintSeverity::Info);

        let mut cfg = LintConfig::default();
        cfg.set(
            "width-mismatch",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let overridden = analyze_with_config(&opts, &cfg);
        let width = overridden.lint.iter().find(|d| d.rule == "width-mismatch");
        assert!(
            width.is_some(),
            "expected width-mismatch finding under configured run: {:?}",
            overridden.lint
        );
        assert_eq!(width.expect("width finding").severity, LintSeverity::Error);
    }

    /// `analyze_with_config` contains Surelog's filesystem side-effects: with
    /// the process CWD parked *inside* a fixture tree, the fixture tree gains
    /// no entries (`slpp_all/`, `surelog.log`, …), the artifacts live under
    /// the analysis scratch dir inside the process shadow base, and the
    /// previous CWD is restored afterwards.
    #[test]
    fn analyze_in_scratch_contains_surelog_side_effects() {
        let _guards = analysis_guards();
        let fixture =
            std::env::temp_dir().join(format!("llg_scratch_probe_{}", std::process::id()));
        let rtl = fixture.join("rtl");
        std::fs::create_dir_all(&rtl).expect("create fixture tree");
        let sv = rtl.join("top.sv");
        std::fs::write(&sv, "module top; endmodule\n").expect("write design");

        fn listing(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
            let mut out = Vec::new();
            let mut pending = vec![dir.to_path_buf()];
            while let Some(current) = pending.pop() {
                for entry in std::fs::read_dir(&current).expect("read dir") {
                    let path = entry.expect("dir entry").path();
                    if path.is_dir() {
                        pending.push(path.clone());
                    }
                    out.push(path);
                }
            }
            out.sort();
            out
        }
        let before = listing(&fixture);

        // Run the analysis with the CWD pointing INSIDE the fixture tree.
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: fixture.clone(),
            orig: orig_cwd.clone(),
        };
        std::env::set_current_dir(&rtl).expect("chdir into fixture tree");

        let opts = CompileOpts {
            files: vec![sv.to_string_lossy().into_owned()],
            top: None,
            ..Default::default()
        };
        let analysis = analyze_with_config(&opts, &LintConfig::default());
        assert!(
            analysis.is_valid(),
            "analysis failed: {:?}",
            analysis.diagnostics
        );

        assert_eq!(listing(&fixture), before, "fixture tree gained entries");
        let scratch = analysis_scratch_dir();
        assert!(scratch.starts_with(process_shadow_base()));
        // The guard restored the pre-analysis CWD (the fixture rtl dir this
        // test chdir'd into), not the process default.
        assert_eq!(
            std::env::current_dir().expect("cwd after restore"),
            std::fs::canonicalize(&rtl).unwrap_or(rtl.clone())
        );
        assert!(
            scratch.join("slpp_all").is_dir(),
            "slpp_all must live under the analysis scratch dir {}",
            scratch.display()
        );

        cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(fixture);
    }

    /// Regression: the `module` declaration keyword must be tokenized the
    /// same way `endmodule` is.  Surelog types the leading keyword node
    /// `paModule_keyword` (the `paMODULE` discriminant belongs to the whole
    /// declaration design element), which the parse-token pass previously
    /// ignored — so the first token on a declaration line started at the
    /// identifier column.
    #[test]
    fn semantic_tokens_cover_the_module_declaration_keyword() {
        use tower_lsp::lsp_types::SemanticTokenType;

        let _guards = analysis_guards();
        let fixture = std::env::temp_dir().join(format!("llg_modkw_{}", std::process::id()));
        let rtl = fixture.join("rtl");
        std::fs::create_dir_all(&rtl).expect("create fixture tree");
        let sv = rtl.join("modkw.sv");
        std::fs::write(
            &sv,
            "module mod_kw(input logic clk);\n  wire w;\nendmodule\n",
        )
        .expect("write design");

        let opts = CompileOpts {
            files: vec![sv.to_string_lossy().into_owned()],
            ..Default::default()
        };
        let analysis = analyze_with_config(&opts, &LintConfig::default());
        assert!(
            analysis.is_valid(),
            "analysis failed: {:?}",
            analysis.diagnostics
        );

        let legend = crate::semantic_tokens::legend();
        let keyword_index = legend
            .token_types
            .iter()
            .position(|t| *t == SemanticTokenType::KEYWORD)
            .expect("keyword type in legend") as u32;

        // Decode the delta-encoded stream back to absolute (line, col, len).
        let tokens = semantic_tokens_for(&analysis, &sv.to_string_lossy());
        let mut line = 0u32;
        let mut col = 0u32;
        let mut keywords: Vec<(u32, u32, u32)> = Vec::new();
        for token in &tokens.data {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            if token.token_type == keyword_index {
                keywords.push((line, col, token.length));
            }
        }
        assert!(
            keywords.contains(&(0, 0, "module".len() as u32)),
            "expected a keyword token over `module` at 0:0, got {keywords:?}"
        );
        assert!(
            keywords.contains(&(2, 0, "endmodule".len() as u32)),
            "expected a keyword token over `endmodule` at 2:0, got {keywords:?}"
        );

        cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(fixture);
    }

    /// `settings_to_lint_config` maps the documented settings shape: a disabled
    /// rule, a severity override, and untouched defaults for unmentioned rules.
    #[test]
    fn settings_to_lint_config_maps_rule_overrides() {
        let settings = settings_obj(vec![(
            "lint",
            settings_obj(vec![(
                "rules",
                settings_obj(vec![
                    (
                        "unused-signal",
                        settings_obj(vec![("enabled", LSPAny::Bool(false))]),
                    ),
                    (
                        "width-mismatch",
                        settings_obj(vec![("severity", LSPAny::String("error".to_owned()))]),
                    ),
                ]),
            )]),
        )]);

        let cfg = settings_to_lint_config(&settings);
        assert!(
            !cfg.is_enabled("unused-signal"),
            "unused-signal should be disabled"
        );
        assert_eq!(cfg.severity("width-mismatch"), Some(LintSeverity::Error));
        assert!(
            cfg.is_enabled("incomplete-case"),
            "unmentioned rule should stay enabled"
        );
        assert_eq!(cfg.severity("incomplete-case"), None);
    }

    /// A global `"enabled": false` disables every known rule; a per-rule entry
    /// can re-enable one.
    #[test]
    fn settings_to_lint_config_global_enabled_false_disables_all() {
        let settings = settings_obj(vec![(
            "lint",
            settings_obj(vec![("enabled", LSPAny::Bool(false))]),
        )]);
        let cfg = settings_to_lint_config(&settings);
        assert!(!cfg.is_enabled("unused-signal"));
        assert!(!cfg.is_enabled("naming-style"));

        let with_override = settings_obj(vec![(
            "lint",
            settings_obj(vec![
                ("enabled", LSPAny::Bool(false)),
                (
                    "rules",
                    settings_obj(vec![(
                        "casez-misuse",
                        settings_obj(vec![("enabled", LSPAny::Bool(true))]),
                    )]),
                ),
            ]),
        )]);
        let cfg = settings_to_lint_config(&with_override);
        assert!(!cfg.is_enabled("unused-signal"));
        assert!(cfg.is_enabled("casez-misuse"));
    }

    /// A bare `{"rules": ...}` payload (no `lint` wrapper) is accepted.
    #[test]
    fn settings_to_lint_config_accepts_bare_rules_object() {
        let settings = settings_obj(vec![(
            "rules",
            settings_obj(vec![(
                "casez-misuse",
                settings_obj(vec![("enabled", LSPAny::Bool(false))]),
            )]),
        )]);
        let cfg = settings_to_lint_config(&settings);
        assert!(!cfg.is_enabled("casez-misuse"));
        assert!(cfg.is_enabled("unused-signal"));
    }

    /// Hand-built two-file `Analysis` proving cross-file resolution:
    ///
    /// * `/x/a.sv`: `module m(input logic clk, output logic [3:0] o); assign o = clk; endmodule`
    ///   (module decl + port decls + assignment references), plus `package p`.
    /// * `/x/b.sv`: `module top; m u0(.clk(c), .o(o)); endmodule`
    ///   (instance `u0` of `m`; the `m` type name and the named port
    ///   connections are reference sites).
    ///
    /// Token types mirror the real pipeline (verified empirically): module
    /// names `vpiModule`, port decls `TOKEN_PORT_*` with `vpiNet`/`vpiPort`
    /// companions, expression references `vpiRefObj` (+ companion), module
    /// type names at instantiation sites `uhdmclass_defn`, instance names
    /// `uhdmlogic_var`, named port connections `vpiFunction`.
    fn cross_file_analysis() -> Analysis {
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: String::new(), // filled below
        };
        let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
            FileTokens {
                path: path.to_owned(),
                nodes: nodes
                    .into_iter()
                    .map(|(l, c, t, n)| {
                        let mut v = node(l, c, t, n);
                        v.file = path.to_owned();
                        v
                    })
                    .collect(),
            }
        };

        let a_file = mk(
            vec![
                (1, 8, llg::ffi::vpi::vpiModule, "m"),
                (1, 24, llg::ffi::vpi::TOKEN_PORT_INPUT, "clk"),
                (1, 24, llg::ffi::vpi::vpiNet, "clk"),
                (1, 24, llg::ffi::vpi::vpiPort, "clk"),
                (1, 47, llg::ffi::vpi::TOKEN_PORT_OUTPUT, "o"),
                (1, 47, llg::ffi::vpi::vpiNet, "o"),
                (1, 47, llg::ffi::vpi::vpiPort, "o"),
                (2, 10, llg::ffi::vpi::vpiRefObj, "o"),
                (2, 10, llg::ffi::vpi::vpiNet, "o"),
                (2, 14, llg::ffi::vpi::vpiRefObj, "clk"),
                (2, 14, llg::ffi::vpi::vpiPort, "clk"),
            ],
            "/x/a.sv",
        );
        let b_file = mk(
            vec![
                (1, 8, llg::ffi::vpi::vpiModule, "top"),
                (1, 13, llg::ffi::vpi::uhdmclass_defn, "m"),
                (1, 15, llg::ffi::vpi::uhdmlogic_var, "u0"),
                (1, 19, llg::ffi::vpi::vpiFunction, "clk"),
                (1, 23, llg::ffi::vpi::vpiRefObj, "c"),
                (1, 28, llg::ffi::vpi::vpiFunction, "o"),
                (1, 30, llg::ffi::vpi::vpiPort, "o"),
            ],
            "/x/b.sv",
        );

        let module_m = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 2,
            end_col: 16,
        };
        let port = |name: &str, dir: Direction| PortModel {
            name: name.to_owned(),
            direction: dir,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 15,
            ports: vec![port("clk", Direction::Input), port("o", Direction::Output)],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: vec![port("c", Direction::Input), port("o", Direction::Output)],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: vec![module_m],
            packages: vec![PackageDef {
                name: "p".to_owned(),
                file: Some("/x/a.sv".to_owned()),
                line: 3,
                col: 1,
                params: Vec::new(),
                enum_consts: Vec::new(),
            }],
            classes: Vec::new(),
        };
        Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new())
    }

    /// Hand-built analysis for two `Bar` instantiations inside `Foo`:
    /// `Bar Bar(...)` and `Bar u_bar(...)`.  The first instance deliberately
    /// shares its name with the module type so the type-reference resolver's
    /// scope and kind behavior can be tested independently from Surelog.
    fn module_type_instance_collision_analysis() -> Analysis {
        use llg::ffi::vpi;

        let mut analysis = cross_file_analysis();
        analysis.model.design_name = "Foo".to_owned();
        analysis.model.modules[0].name = "Bar".to_owned();
        analysis.model.modules.push(ModuleDef {
            name: "Foo".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 4,
            end_col: 12,
        });
        {
            let top = &mut analysis.model.top_instances[0];
            top.name = "Foo".to_owned();
            top.def_name = "Foo".to_owned();
            top.full_name = "Foo".to_owned();

            let first = &mut top.children[0];
            first.name = "Bar".to_owned();
            first.def_name = "Bar".to_owned();
            first.full_name = "Foo.Bar".to_owned();

            let mut second = first.clone();
            second.name = "u_bar".to_owned();
            second.full_name = "Foo.u_bar".to_owned();
            second.line = 2;
            second.col = 7;
            top.children.push(second);
        }

        for file_tokens in &mut analysis.tokens {
            for node in &mut file_tokens.nodes {
                if file_tokens.path == "/x/a.sv"
                    && node.vpi_type == vpi::vpiModule
                    && node.name.as_deref() == Some("m")
                {
                    node.name = Some("Bar".to_owned());
                }
                if file_tokens.path == "/x/b.sv" {
                    if node.vpi_type == vpi::vpiModule && node.name.as_deref() == Some("top") {
                        node.name = Some("Foo".to_owned());
                    }
                    if node.vpi_type == vpi::uhdmclass_defn && node.name.as_deref() == Some("m") {
                        node.name = Some("Bar".to_owned());
                    }
                    if node.vpi_type == vpi::uhdmlogic_var && node.name.as_deref() == Some("u0") {
                        node.name = Some("Bar".to_owned());
                    }
                }
            }
        }
        let b_file = analysis
            .tokens
            .iter_mut()
            .find(|file_tokens| file_tokens.path == "/x/b.sv")
            .expect("collision fixture file");
        b_file.nodes.extend([
            VObjectInfo {
                line: 2,
                col: 3,
                end_line: 2,
                end_col: 6,
                vpi_type: vpi::uhdmclass_defn,
                name: Some("Bar".to_owned()),
                file: "/x/b.sv".to_owned(),
            },
            VObjectInfo {
                line: 2,
                col: 7,
                end_line: 2,
                end_col: 12,
                vpi_type: vpi::uhdmlogic_var,
                name: Some("u_bar".to_owned()),
                file: "/x/b.sv".to_owned(),
            },
        ]);
        analysis.index = SymbolIndex::build(&analysis);
        analysis
    }

    #[test]
    fn module_type_definition_ignores_same_named_instance_in_scope() {
        // Arrange
        let analysis = module_type_instance_collision_analysis();

        // Act
        let first_type = definition_at(&analysis, "/x/b.sv", 0, 12);
        let second_type = definition_at(&analysis, "/x/b.sv", 1, 2);

        // Assert
        for location in [first_type, second_type] {
            let location = location.expect("module type definition");
            assert_eq!(location.uri, Url::from_file_path("/x/a.sv").unwrap());
            assert_eq!(location.range.start, Position::new(0, 7));
        }
    }

    #[test]
    fn instance_name_definition_still_resolves_when_name_matches_module_type() {
        // Arrange
        let analysis = module_type_instance_collision_analysis();

        // Act
        let colliding_instance = definition_at(&analysis, "/x/b.sv", 0, 14);
        let ordinary_instance = definition_at(&analysis, "/x/b.sv", 1, 6);

        // Assert
        for location in [colliding_instance, ordinary_instance] {
            let location = location.expect("instance definition");
            assert_eq!(location.uri, Url::from_file_path("/x/a.sv").unwrap());
            assert_eq!(location.range.start, Position::new(0, 7));
        }
    }

    /// Hand-built two-file `Analysis` for a multi-line instantiation:
    ///
    /// * `/x/a.sv`: same as [`cross_file_analysis`] (`module m(input logic
    ///   clk, output logic [3:0] o); assign o = clk; endmodule`).
    /// * `/x/b.sv`: the instantiation is spread over several lines, so the
    ///   labels live on continuation lines below the instance name:
    ///
    ///   ```text
    ///   module top; m u0(    ← instance `u0` at 0-based (0, 8)
    ///       .clk(c),         ← label at 0-based (1, 3)
    ///       .o(o)            ← label at 0-based (2, 3)
    ///   );                   ← closing at 0-based (3, 0)
    ///   ```
    ///
    ///   plus a decoy `vpiFunction`-typed token named `clk` at 0-based
    ///   (10, 0) that must NOT be treated as a port label: its column is 0,
    ///   so it is not an indented continuation line.
    ///
    /// The `mk` helper takes 1-based positions (as `VObjectInfo` reports
    /// them); the index converts them to 0-based.
    fn multiline_port_analysis() -> Analysis {
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: String::new(), // filled below
        };
        let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
            FileTokens {
                path: path.to_owned(),
                nodes: nodes
                    .into_iter()
                    .map(|(l, c, t, n)| {
                        let mut v = node(l, c, t, n);
                        v.file = path.to_owned();
                        v
                    })
                    .collect(),
            }
        };

        let a_file = mk(
            vec![
                (1, 8, llg::ffi::vpi::vpiModule, "m"),
                (1, 24, llg::ffi::vpi::TOKEN_PORT_INPUT, "clk"),
                (1, 24, llg::ffi::vpi::vpiNet, "clk"),
                (1, 24, llg::ffi::vpi::vpiPort, "clk"),
                (1, 47, llg::ffi::vpi::TOKEN_PORT_OUTPUT, "o"),
                (1, 47, llg::ffi::vpi::vpiNet, "o"),
                (1, 47, llg::ffi::vpi::vpiPort, "o"),
                (2, 10, llg::ffi::vpi::vpiRefObj, "o"),
                (2, 10, llg::ffi::vpi::vpiNet, "o"),
                (2, 14, llg::ffi::vpi::vpiRefObj, "clk"),
                (2, 14, llg::ffi::vpi::vpiPort, "clk"),
            ],
            "/x/a.sv",
        );
        let mut b_file = mk(
            vec![
                // Instance name token (1-based (1,9) → 0-based (0,8)).
                (1, 9, llg::ffi::vpi::uhdmlogic_var, "u0"),
                // `.clk` label (1-based (2,4) → 0-based (1,3)).
                (2, 4, llg::ffi::vpi::vpiFunction, "clk"),
                // Inner expression ref `c` in `.clk(c)`.
                (2, 8, llg::ffi::vpi::vpiRefObj, "c"),
                // `.o` label (1-based (3,4) → 0-based (2,3)).
                (3, 4, llg::ffi::vpi::vpiFunction, "o"),
                // Inner expression ref `o` in `.o(o)`.
                (3, 8, llg::ffi::vpi::vpiPort, "o"),
                // Decoy `vpiFunction` token at 0-based (10, 0): named `clk`
                // so it would pass the Var-ref classification, but its column
                // is 0 → not an indented continuation line.
                (11, 1, llg::ffi::vpi::vpiFunction, "clk"),
            ],
            "/x/b.sv",
        );
        // Nameless closing `);` at 0-based (3, 0) (1-based (4, 1)): included
        // for fixture fidelity; the index skips nameless tokens.
        b_file.nodes.push(VObjectInfo {
            line: 4,
            col: 1,
            end_line: 4,
            end_col: 2,
            vpi_type: 0,
            name: None,
            file: "/x/b.sv".to_owned(),
        });

        let module_m = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 2,
            end_col: 16,
        };
        let port = |name: &str, dir: Direction| PortModel {
            name: name.to_owned(),
            direction: dir,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 9,
            ports: vec![port("clk", Direction::Input), port("o", Direction::Output)],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: vec![port("c", Direction::Input), port("o", Direction::Output)],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: vec![module_m],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new())
    }

    #[test]
    fn port_label_multiline_definition_jumps_to_child_port_decl_across_files() {
        let a = multiline_port_analysis();
        // Both continuation-line labels must be registered as port labels.
        assert!(
            a.index
                .port_labels
                .contains_key(&("/x/b.sv".to_owned(), 1, 3)),
            "port_labels: {:?}",
            a.index.port_labels
        );
        assert!(
            a.index
                .port_labels
                .contains_key(&("/x/b.sv".to_owned(), 2, 3)),
            "port_labels: {:?}",
            a.index.port_labels
        );
        // `.clk` at 0-based (1, 3) in file B → m's clk port decl in file A
        // (0-based (0, 23)), not the enclosing scope's same-named object.
        let loc = definition_at(&a, "/x/b.sv", 1, 3).expect("definition of .clk label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
    }

    #[test]
    fn port_label_multiline_hover_shows_child_port() {
        let a = multiline_port_analysis();
        // `.o` label at 0-based (2, 3) in file B.
        let hover = hover_at(&a, "/x/b.sv", 2, 3).expect("hover on .o label");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("output"), "value: {value}");
        assert!(value.contains("o"), "value: {value}");
    }

    #[test]
    fn port_label_multiline_decoy_at_column_zero_is_not_a_port_label() {
        let a = multiline_port_analysis();
        // The decoy at 0-based (10, 0) is a `vpiFunction`/Var ref (named
        // `clk`, a known signal) but its column is 0, so the continuation-line
        // rule rejects it: it must not be registered as a port label and falls
        // back to ordinary name-based resolution (which lands on the same
        // workspace `clk` declaration here).
        assert!(
            !a.index
                .port_labels
                .contains_key(&("/x/b.sv".to_owned(), 10, 0)),
            "decoy must not be a port label: {:?}",
            a.index.port_labels
        );
        let loc = definition_at(&a, "/x/b.sv", 10, 0).expect("decoy name-based resolution");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
    }

    #[test]
    fn entry_at_on_instance_returns_instance_decl() {
        let a = cross_file_analysis();
        let e = a.index.entry_at("/x/b.sv", 0, 14).expect("entry at u0");
        assert_eq!(e.name, "u0");
        assert_eq!(e.kind, SymKind::Instance);
        assert!(e.is_decl);
        assert_eq!(e.scope.as_deref(), Some("top"));
    }

    #[test]
    fn definition_on_instance_jumps_to_module_def_across_files() {
        let a = cross_file_analysis();
        let loc = definition_at(&a, "/x/b.sv", 0, 14).expect("definition of u0");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 7)); // module m at col 8 → 0-based 7
    }

    #[test]
    fn definition_on_module_type_ref_jumps_to_def_across_files() {
        let a = cross_file_analysis();
        // The `m` type name at the instantiation site in file B.
        let loc = definition_at(&a, "/x/b.sv", 0, 12).expect("definition of m ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 7));
    }

    #[test]
    fn references_on_module_decl_span_both_files() {
        let a = cross_file_analysis();
        let refs = references_at(&a, "/x/a.sv", 0, 7); // module m decl
        assert!(
            refs.iter().any(|l| {
                l.uri == Url::from_file_path("/x/a.sv").unwrap()
                    && l.range.start == Position::new(0, 7)
            }),
            "missing m decl in refs: {refs:?}"
        );
        assert!(
            refs.iter().any(|l| {
                l.uri == Url::from_file_path("/x/b.sv").unwrap()
                    && l.range.start == Position::new(0, 12)
            }),
            "missing u0-site ref in refs: {refs:?}"
        );
    }

    #[test]
    fn workspace_symbols_filters_by_query() {
        let a = cross_file_analysis();
        let syms = workspace_symbols(&a, "m");
        assert_eq!(syms.len(), 1, "syms: {syms:?}");
        assert_eq!(syms[0].name, "m");
        assert_eq!(syms[0].kind, SymbolKind::MODULE);
        assert_eq!(
            syms[0].location.uri,
            Url::from_file_path("/x/a.sv").unwrap()
        );

        let top_syms = workspace_symbols(&a, "TOP");
        assert!(
            top_syms
                .iter()
                .any(|s| s.name == "top" && s.kind == SymbolKind::OBJECT),
            "top_syms: {top_syms:?}"
        );
    }

    #[test]
    fn symbol_index_merge_unions_and_dedupes_shared_occurrences() {
        let a = cross_file_analysis();
        // Merging an index with itself must dedupe every declaration and
        // reference by position while keeping the lookup maps functional.
        let merged = SymbolIndex::merge([&a.index, &a.index]);
        assert_eq!(
            merged.decls.len(),
            a.index.decls.len(),
            "duplicate declarations leaked through merge: {} vs {}",
            merged.decls.len(),
            a.index.decls.len()
        );
        assert_eq!(
            merged.refs.len(),
            a.index.refs.len(),
            "duplicate references leaked through merge: {} vs {}",
            merged.refs.len(),
            a.index.refs.len()
        );
        let e = merged
            .entry_at("/x/a.sv", 0, 23)
            .expect("entry at clk port after merge");
        assert_eq!(e.name, "clk");
        assert_eq!(e.kind, SymKind::Port);
        assert_eq!(merged.resolve(e).len(), 1, "resolution after merge");
    }

    #[test]
    fn entry_at_on_port_token_returns_port_decl() {
        let a = cross_file_analysis();
        let e = a
            .index
            .entry_at("/x/a.sv", 0, 23)
            .expect("entry at clk port");
        assert_eq!(e.name, "clk");
        assert_eq!(e.kind, SymKind::Port);
        assert!(e.is_decl);
        assert_eq!(e.scope.as_deref(), Some("m"));
    }

    #[test]
    fn ref_inside_assign_resolves_to_port_decl() {
        let a = cross_file_analysis();
        // `clk` in `assign o = clk;` at (line 2, col 14) → 0-based (1, 13).
        let e = a
            .index
            .entry_at("/x/a.sv", 1, 13)
            .expect("entry at clk ref");
        assert!(!e.is_decl);
        let resolved = a.index.resolve(e);
        assert_eq!(resolved.len(), 1, "resolved: {resolved:?}");
        assert_eq!(resolved[0].kind, SymKind::Port);
        assert_eq!(resolved[0].file, "/x/a.sv");
        assert_eq!((resolved[0].line, resolved[0].col), (0, 23));
        let loc = definition_at(&a, "/x/a.sv", 1, 13).expect("definition of clk ref");
        assert_eq!(loc.range.start, Position::new(0, 23));
    }

    #[test]
    fn port_references_span_both_files() {
        let a = cross_file_analysis();
        // All references of the `o` port (file A decl) include the assignment
        // site in file A and the named connection `.o(o)` in file B.
        let o_decl = a.index.entry_at("/x/a.sv", 0, 46).expect("o port decl");
        assert_eq!(o_decl.name, "o");
        let refs = a.index.all_references(o_decl);
        let sites: Vec<(String, u32, u32)> = refs
            .iter()
            .map(|r| (r.file.clone(), r.line, r.col))
            .collect();
        assert!(
            sites.contains(&("/x/a.sv".to_owned(), 0, 46)),
            "missing o decl: {sites:?}"
        );
        assert!(
            sites.contains(&("/x/a.sv".to_owned(), 1, 9)),
            "missing assign o site: {sites:?}"
        );
        assert!(
            sites.contains(&("/x/b.sv".to_owned(), 0, 27)),
            "missing .o named connection: {sites:?}"
        );
        assert!(
            sites.contains(&("/x/b.sv".to_owned(), 0, 29)),
            "missing .o inner ref: {sites:?}"
        );
    }

    #[test]
    fn port_label_definition_jumps_to_child_port_decl_across_files() {
        let a = cross_file_analysis();
        // `.clk` in `m u0(.clk(c), .o(o));` at 0-based (0, 18) in file B must
        // resolve to module m's `clk` port declaration in file A, not the
        // enclosing module's scope.
        let loc = definition_at(&a, "/x/b.sv", 0, 18).expect("definition of .clk label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 23)); // clk port decl in file A
    }

    #[test]
    fn port_label_hover_shows_child_port() {
        let a = cross_file_analysis();
        // `.o` label at 0-based (0, 27) in file B.
        let hover = hover_at(&a, "/x/b.sv", 0, 27).expect("hover on .o label");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("output"), "value: {value}");
        assert!(value.contains("o"), "value: {value}");
    }

    #[test]
    fn references_on_port_decl_include_instantiation_labels() {
        let a = cross_file_analysis();
        // References of the `o` port decl in file A (0, 46) include the
        // instantiation-site `.o` label in file B.
        let refs = references_at(&a, "/x/a.sv", 0, 46);
        assert!(
            refs.iter().any(|l| {
                l.uri == Url::from_file_path("/x/b.sv").unwrap()
                    && l.range.start == Position::new(0, 27)
            }),
            "missing .o label site: {refs:?}"
        );
    }

    #[test]
    fn port_label_synthesizes_missing_port_decl() {
        // File A declares module `m` but its token stream has no port-decl
        // tokens; file B instantiates it with a named connection.  The port
        // declaration is synthesized at the module header so goto-definition
        // still lands in the def file.
        let node = |line: u32, col: u32, t: i32, name: &str, file: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: file.to_owned(),
        };
        let a_file = FileTokens {
            path: "/x/a.sv".to_owned(),
            nodes: vec![node(1, 8, llg::ffi::vpi::vpiModule, "m", "/x/a.sv")],
        };
        let b_file = FileTokens {
            path: "/x/b.sv".to_owned(),
            nodes: vec![
                node(1, 13, llg::ffi::vpi::uhdmclass_defn, "m", "/x/b.sv"),
                node(1, 15, llg::ffi::vpi::uhdmlogic_var, "u0", "/x/b.sv"),
                node(1, 19, llg::ffi::vpi::vpiFunction, "clk", "/x/b.sv"),
            ],
        };
        let module_m = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 1,
            end_col: 10,
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 15,
            ports: vec![PortModel {
                name: "clk".to_owned(),
                direction: Direction::Input,
                ty: TypeInfo {
                    kind: "logic".to_owned(),
                    width: Some(1),
                    signed: false,
                    type_name: None,
                },
            }],
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: vec![module_m],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let a = Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new());
        let loc = definition_at(&a, "/x/b.sv", 0, 18).expect("definition of .clk label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        // module m at 1-based col 8 (0-based 7) + name len 1 + port index 0.
        assert_eq!(loc.range.start, Position::new(0, 8));
    }

    #[test]
    fn index_is_built_by_analyze() {
        let a = empty_analysis();
        assert!(a.index.decls.is_empty());
        assert!(a.index.refs.is_empty());
        assert!(a.index.entry_at("/nope.sv", 0, 0).is_none());
        assert!(a.index.decls_in_file("/nope.sv").is_empty());
    }

    /// Real compile of `tests/elaboration/top3.sv`: line 34 instantiates
    /// `hier_ref u_hier (.clk(clk), .o(o));` inside module `tb`, which itself
    /// declares ports named `clk`/`o`.  The labels must resolve to the CHILD
    /// module's ports (hier_ref), not the enclosing module's same-named ones;
    /// the inner expression refs must keep resolving to the enclosing scope.
    #[test]
    fn analyze_full_pipeline_named_ports_resolve_to_child() {
        let _guards = analysis_guards();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elaboration/top3.sv");
        if !path.exists() {
            return;
        }
        let path_str = path.to_string_lossy().into_owned();
        let opts = CompileOpts {
            files: vec![path_str.clone()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        // `.clk` label at 0-based (33, 22) → hier_ref's clk port decl
        // (20, 16), NOT tb's clk port decl (29, 16).
        let loc = definition_at(&a, &path_str, 33, 22).expect("definition of .clk label");
        assert_eq!(loc.range.start, Position::new(20, 16), "loc: {loc:?}");
        // `.o` label at (33, 33) → hier_ref's o port decl (21, 23).
        let loc = definition_at(&a, &path_str, 33, 33).expect("definition of .o label");
        assert_eq!(loc.range.start, Position::new(21, 23), "loc: {loc:?}");
        // The inner `clk` ref (the connection ACTUAL) resolves to the
        // ACTUAL signal's own declaration in the instantiating scope —
        // tb's clk port decl (29, 16), NOT the child module's same-named
        // port, even though both modules declare `clk`.
        let loc = definition_at(&a, &path_str, 33, 26).expect("definition of inner clk ref");
        assert_eq!(loc.range.start, Position::new(29, 16), "loc: {loc:?}");
        assert_ne!(
            loc.range.start,
            Position::new(20, 16),
            "the actual must not jump into the child module"
        );
        // The label and the actual of the SAME connection resolve to
        // DIFFERENT declarations (child port vs parent-scope decl).
        let label_loc = definition_at(&a, &path_str, 33, 22).expect("label location");
        assert_ne!(label_loc.range.start, loc.range.start);
        // Hover on the `.o` label shows the child port.
        let hover = hover_at(&a, &path_str, 33, 33).expect("hover on .o label");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("output"), "value: {value}");
        // References of hier_ref's `o` port decl include the `.o` label site.
        let refs = references_at(&a, &path_str, 21, 23);
        assert!(
            refs.iter().any(|l| l.range.start == Position::new(33, 33)),
            "missing .o label site: {refs:?}"
        );
    }

    /// Full compile of a two-file design whose instantiation is spread over
    /// several lines: `m u0(\n  .clk(clk),\n  .o(o)\n);` inside module `top`,
    /// which itself declares signals named `clk`/`o`.  The continuation-line
    /// labels must resolve to the CHILD module's ports in a.sv, not the
    /// enclosing module's same-named signals.  Runs in a fresh temp dir
    /// (Surelog writes `slpp_all/` into the CWD).
    #[test]
    fn analyze_full_pipeline_multiline_named_ports_resolve_to_child() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_mlport_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let a_sv = dir.join("a.sv");
        let b_sv = dir.join("b.sv");
        std::fs::write(
            &a_sv,
            "module m(\n  input logic clk,\n  output logic [3:0] o\n);\n  assign o = clk;\nendmodule\n",
        )
        .expect("write a.sv");
        std::fs::write(
            &b_sv,
            "module top;\n  m u0(\n    .clk(clk),\n    .o(o)\n  );\n  logic clk;\n  logic [3:0] o;\nendmodule\n",
        )
        .expect("write b.sv");
        let opts = CompileOpts {
            files: vec![
                a_sv.to_string_lossy().into_owned(),
                b_sv.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        let b_path = a
            .tokens
            .iter()
            .find(|ft| ft.path.ends_with("b.sv"))
            .expect("tokens for b.sv")
            .path
            .clone();
        let ft = file_tokens(&a, &b_path).expect("b.sv tokens");
        // The port-connection labels are the only tokens in b.sv carrying the
        // classifier's connection-label synthetic type; find them by name and
        // continuation line rather than hard-coding positions.
        let label = |name: &str| -> VObjectInfo {
            ft.nodes
                .iter()
                .find(|n| {
                    n.vpi_type == llg::ffi::vpi::TOKEN_PORT_CONN_LABEL
                        && n.name.as_deref() == Some(name)
                        && n.line > 2
                })
                .expect("label token")
                .clone()
        };
        let clk_label = label("clk");
        let o_label = label("o");
        assert!(
            o_label.line > clk_label.line,
            "labels must be on consecutive continuation lines: {clk_label:?} {o_label:?}"
        );
        let clk_pos = (clk_label.line - 1, clk_label.col - 1);
        let o_pos = (o_label.line - 1, o_label.col - 1);
        // Both continuation-line labels are registered as port labels.
        assert!(
            a.index
                .port_labels
                .contains_key(&(b_path.clone(), clk_pos.0, clk_pos.1)),
            "port_labels: {:?}",
            a.index.port_labels
        );
        assert!(
            a.index
                .port_labels
                .contains_key(&(b_path.clone(), o_pos.0, o_pos.1)),
            "port_labels: {:?}",
            a.index.port_labels
        );
        // `.clk` → m's clk port decl in a.sv (1-based (2,15) → 0-based
        // (1,14)), NOT top's `logic clk` (0-based (5,8)).
        let loc =
            definition_at(&a, &b_path, clk_pos.0, clk_pos.1).expect("definition of .clk label");
        assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(1, 14), "loc: {loc:?}");
        // `.o` → m's o port decl in a.sv (1-based (3,22) → 0-based (2,21)).
        let loc = definition_at(&a, &b_path, o_pos.0, o_pos.1).expect("definition of .o label");
        assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(2, 21), "loc: {loc:?}");
        // Hover on the `.o` label shows the child port.
        let hover = hover_at(&a, &b_path, o_pos.0, o_pos.1).expect("hover on .o label");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("output"), "value: {value}");
        assert!(value.contains("o"), "value: {value}");
    }

    /// 0-based `(line, col)` of the `occurrence`-th (0-based) `needle` in
    /// `text` — the same convention as the stdio suite's `position_at`.
    fn pos_of(text: &str, needle: &str, occurrence: usize) -> (u32, u32) {
        let mut start = 0;
        for _ in 0..=occurrence {
            let found = text[start..]
                .find(needle)
                .unwrap_or_else(|| panic!("needle {needle:?} not found"));
            start += found;
        }
        let line = text[..start].matches('\n').count() as u32;
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        (line, (start - line_start) as u32)
    }

    /// Real compile of a design with a named PARAMETER override:
    /// `child u0 #(.W(4), .D(W)) (.clk(clk), .q(t_q));` inside module `top`,
    /// which declares a DECOY same-name `localparam int W`.  The `.W`/`.D`
    /// labels must resolve to the CHILD module's parameter declarations while
    /// the RHS reference `W` inside `.D(W)` stays on the decoy localparam in
    /// the instantiating scope — never crossing namespaces in either
    /// direction.
    #[test]
    fn analyze_full_pipeline_named_param_overrides_resolve_to_child() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_param_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let child_sv = dir.join("child.sv");
        let top_sv = dir.join("top.sv");
        let child_text = concat!(
            "module child #(\n",
            "  parameter int W = 8,\n",
            "  parameter int D = 3\n",
            ") (\n",
            "  input logic clk,\n",
            "  output logic [7:0] q\n",
            ");\n",
            "  assign q = '0;\n",
            "endmodule\n",
        );
        let top_text = concat!(
            "module top;\n",
            "  localparam int W = 1;\n",
            "  logic clk;\n",
            "  logic [7:0] t_q;\n",
            "  child #(.W(4), .D(W)) u0 (.clk(clk), .q(t_q));\n",
            "endmodule\n",
        );
        std::fs::write(&child_sv, child_text).expect("write child.sv");
        std::fs::write(&top_sv, top_text).expect("write top.sv");
        let opts = CompileOpts {
            files: vec![
                child_sv.to_string_lossy().into_owned(),
                top_sv.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        let top_path = a
            .tokens
            .iter()
            .find(|ft| ft.path.ends_with("top.sv"))
            .expect("tokens for top.sv")
            .path
            .clone();
        // The override labels sit inside the `#(...)` clause, which in valid
        // SV PRECEDES the instance name.
        let (wl, wc) = pos_of(top_text, "#(.W", 0);
        let w_label = (wl, wc + 3);
        let (dl, dc) = pos_of(top_text, ", .D", 0);
        let d_label = (dl, dc + 3);
        assert_eq!(w_label, (4, 11), "sanity: .W label position");
        // The scanned pair must be recorded with Param flavor.
        assert!(
            a.index
                .param_labels
                .contains_key(&(top_path.clone(), w_label.0, w_label.1)),
            "param_labels must contain the .W label: {:?}",
            a.index.param_labels
        );
        // `.W` label → CHILD module's parameter declaration in child.sv
        // (1-based (2,17) → 0-based (1,16)), NOT the decoy localparam.
        let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
        let decoy = (dcl, dcc + 15);
        assert_eq!(decoy, (1, 17), "sanity: decoy position");
        let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
        assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
        assert_ne!(loc.range.start, Position::new(decoy.0, decoy.1));
        // `.D` label → child's D parameter (0-based (2,16)).
        let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
        assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
        // The RHS `W` inside `.D(W)` resolves to the DECOY localparam in the
        // instantiating scope — the exact opposite direction of the label.
        let (rl, rc) = pos_of(top_text, "(W)", 0);
        let rhs = (rl, rc + 1);
        let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
        assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
        assert_eq!(
            loc.range.start,
            Position::new(decoy.0, decoy.1),
            "loc: {loc:?}"
        );
        // Hover on the `.W` label shows the child parameter.
        let hover = hover_at(&a, &top_path, w_label.0, w_label.1).expect("hover on .W label");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("parameter"), "value: {value}");
    }

    /// Multi-line variant of the parameter override navigation: the labels
    /// sit on continuation lines below `child u0 #(` and must still resolve
    /// to the CHILD module's parameters, while the RHS stays parent-scope.
    #[test]
    fn analyze_full_pipeline_multiline_named_param_overrides_resolve_to_child() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_mlparam_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let child_sv = dir.join("child.sv");
        let top_sv = dir.join("top.sv");
        let child_text = concat!(
            "module child #(\n",
            "  parameter int W = 8,\n",
            "  parameter int D = 3\n",
            ") (\n",
            "  input logic clk,\n",
            "  output logic [7:0] q\n",
            ");\n",
            "  assign q = '0;\n",
            "endmodule\n",
        );
        let top_text = concat!(
            "module top;\n",
            "  localparam int W = 1;\n",
            "  logic clk;\n",
            "  logic [7:0] t_q;\n",
            "  child #(\n",
            "    .W(4),\n",
            "    .D(W)\n",
            "  ) u0 (\n",
            "    .clk(clk),\n",
            "    .q(t_q)\n",
            "  );\n",
            "endmodule\n",
        );
        std::fs::write(&child_sv, child_text).expect("write child.sv");
        std::fs::write(&top_sv, top_text).expect("write top.sv");
        let opts = CompileOpts {
            files: vec![
                child_sv.to_string_lossy().into_owned(),
                top_sv.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        let top_path = a
            .tokens
            .iter()
            .find(|ft| ft.path.ends_with("top.sv"))
            .expect("tokens for top.sv")
            .path
            .clone();
        let (wl, wc) = pos_of(top_text, ".W(", 0);
        let w_label = (wl, wc + 1);
        let (dl, dc) = pos_of(top_text, ".D(", 0);
        let d_label = (dl, dc + 1);
        assert_eq!(w_label, (5, 5), "sanity: continuation-line .W position");
        // Continuation-line labels are registered as parameter labels.
        assert!(
            a.index
                .param_labels
                .contains_key(&(top_path.clone(), w_label.0, w_label.1)),
            "param_labels: {:?}",
            a.index.param_labels
        );
        assert!(
            a.index
                .param_labels
                .contains_key(&(top_path.clone(), d_label.0, d_label.1)),
            "param_labels: {:?}",
            a.index.param_labels
        );
        let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
        let decoy = (dcl, dcc + 15);
        // `.W` → child's W parameter in child.sv (0-based (1,16)).
        let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
        assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
        // `.D` → child's D parameter (0-based (2,16)).
        let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
        assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
        assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
        // RHS `W` stays on the decoy localparam in the instantiating scope.
        let (rl, rc) = pos_of(top_text, "(W)", 0);
        let rhs = (rl, rc + 1);
        let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
        assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
        assert_eq!(
            loc.range.start,
            Position::new(decoy.0, decoy.1),
            "loc: {loc:?}"
        );
    }

    /// Hand-built analysis proving the UNSOLVABLE-label guard: an override
    /// label whose instance's module definition carries no such parameter
    /// yields NO definition — not the same-file `localparam W` decoy that
    /// name-based resolution would pick.
    #[test]
    fn unresolved_param_override_label_yields_no_definition() {
        use llg::ffi::vpi;
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: "/x/b.sv".to_owned(),
        };
        // Two identical views make (1,6) a genuine `localparam W` DECL entry;
        // the single-view token at (2,14) is the dropped-looking LABEL ref.
        let b_file = FileTokens {
            path: "/x/b.sv".to_owned(),
            nodes: vec![
                node(1, 9, vpi::vpiModule, "top"),
                node(2, 6, vpi::vpiParameter, "W"),
                node(2, 6, vpi::vpiParameter, "W"),
                node(3, 10, vpi::uhdmlogic_var, "u0"),
                node(3, 15, vpi::vpiParameter, "W"),
            ],
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 3,
            col: 10,
            ports: Vec::new(),
            signals: Vec::new(),
            // The override target exists on the instance (so the label token
            // passes the signal-name gate and is indexed as a REF), but the
            // DEFINITION module `m` is absent from the model below — the
            // override can never resolve to a declaration.
            params: vec![ParamModel {
                name: "W".to_owned(),
                value: None,
                ty: TypeInfo {
                    kind: "int".to_owned(),
                    width: None,
                    signed: true,
                    type_name: None,
                },
                local: false,
            }],
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: Vec::new(), // def module unknown → resolution impossible
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let pairs = vec![NamedPortConn {
            file: "/x/b.sv".to_owned(),
            label: (3, 15),
            label_name: "W".to_owned(),
            actual: None,
            actual_name: None,
            inst_type: Some("m".to_owned()),
            kind: ConnKind::Param,
        }];
        let a = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            model,
            vec![b_file],
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_decls: None,
                pairs,
                fallback_bindings: HashMap::new(),
                ..ConnectionInputs::default()
            },
        );
        // Sanity: the label is indexed as a REF and known-unresolved.
        let entry = a.index.entry_at("/x/b.sv", 2, 14).expect("label entry");
        assert!(!entry.is_decl);
        assert!(
            a.index.is_unresolved_param_label("/x/b.sv", 2, 14),
            "label must be recorded unresolved"
        );
        // NO definition — especially not the decoy localparam at (1,5).
        assert!(
            definition_at(&a, "/x/b.sv", 2, 14).is_none(),
            "unresolvable override label must yield no result"
        );
    }

    /// Hand-built analysis proving [`resolve_param_label`] synthesis: when the
    /// index has no parameter declaration tokens for the child module, the
    /// override label still binds to a synthesized decl anchored at the
    /// module header, kept disjoint from synthesized port anchors.
    #[test]
    fn resolved_param_override_synthesizes_missing_child_decl() {
        use llg::ffi::vpi;
        let node = |line: u32, col: u32, t: i32, name: &str, file: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: file.to_owned(),
        };
        let a_file = FileTokens {
            path: "/x/a.sv".to_owned(),
            nodes: vec![node(1, 8, vpi::vpiModule, "m", "/x/a.sv")],
        };
        let b_file = FileTokens {
            path: "/x/b.sv".to_owned(),
            nodes: vec![
                node(1, 13, vpi::uhdmclass_defn, "m", "/x/b.sv"),
                node(1, 15, vpi::uhdmlogic_var, "u0", "/x/b.sv"),
                node(1, 20, vpi::vpiParameter, "W", "/x/b.sv"),
            ],
        };
        let module_m = ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 1,
            end_col: 10,
        };
        let u0 = InstanceModel {
            name: "u0".to_owned(),
            def_name: "m".to_owned(),
            full_name: "top.u0".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 15,
            ports: Vec::new(),
            signals: Vec::new(),
            params: vec![ParamModel {
                name: "W".to_owned(),
                value: None,
                ty: TypeInfo {
                    kind: "int".to_owned(),
                    width: None,
                    signed: true,
                    type_name: None,
                },
                local: false,
            }],
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        };
        let top = InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/b.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: vec![u0],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![top],
            modules: vec![module_m],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        let pairs = vec![NamedPortConn {
            file: "/x/b.sv".to_owned(),
            label: (1, 20),
            label_name: "W".to_owned(),
            actual: None,
            actual_name: None,
            inst_type: Some("m".to_owned()),
            kind: ConnKind::Param,
        }];
        let a = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            model,
            vec![a_file, b_file],
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_decls: None,
                pairs,
                fallback_bindings: HashMap::new(),
                ..ConnectionInputs::default()
            },
        );
        let loc = definition_at(&a, "/x/b.sv", 0, 19).expect("definition of .W label");
        assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
        // Synthesized anchor: header col 7 + name len 1 + stride 64 + idx 0.
        assert_eq!(loc.range.start, Position::new(0, 72), "loc: {loc:?}");
        let bound = a
            .ref_bindings
            .get(&("/x/b.sv".to_owned(), 0, 19))
            .expect("label binding folded into ref_bindings");
        assert_eq!(bound.kind, "parameter");
        assert!(bound.via_label);
    }

    /// Hand-built two-file `Analysis` proving package-item resolution:
    ///
    /// * `/x/p.sv`: `package my_pkg; parameter int P = 3; typedef enum logic
    ///   [1:0] { IDLE, RUN } state_t; endpackage` — package decl at 0-based
    ///   (0,8), `P` param decl at (1,16), `IDLE`/`RUN` enum const decls at
    ///   (2,29)/(2,35).
    /// * `/x/u.sv`: `module top` with reference tokens in both spellings the
    ///   index must handle: full `my_pkg::P` / `my_pkg::IDLE` at (2,15)/
    ///   (3,15), bare `P` / `IDLE` at (4,15)/(5,15), and a bare `my_pkg` at
    ///   (6,15).
    ///
    /// Token types mirror the real pipeline: package name `uhdmpackage`, param
    /// decls `vpiParameter`, enum const decls `uhdmenum_const`, expression
    /// references `vpiRefObj`.
    fn package_item_analysis() -> Analysis {
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: String::new(), // filled below
        };
        let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
            FileTokens {
                path: path.to_owned(),
                nodes: nodes
                    .into_iter()
                    .map(|(l, c, t, n)| {
                        let mut v = node(l, c, t, n);
                        v.file = path.to_owned();
                        v
                    })
                    .collect(),
            }
        };

        let p_file = mk(
            vec![
                (1, 9, llg::ffi::vpi::uhdmpackage, "my_pkg"),
                (2, 17, llg::ffi::vpi::vpiParameter, "P"),
                (2, 17, llg::ffi::vpi::vpiParameter, "P"),
                (2, 17, llg::ffi::vpi::vpiParameter, "P"),
                (3, 30, llg::ffi::vpi::uhdmenum_const, "IDLE"),
                (3, 36, llg::ffi::vpi::uhdmenum_const, "RUN"),
            ],
            "/x/p.sv",
        );
        let u_file = mk(
            vec![
                (1, 8, llg::ffi::vpi::vpiModule, "top"),
                (3, 16, llg::ffi::vpi::vpiRefObj, "my_pkg::P"),
                (4, 16, llg::ffi::vpi::vpiRefObj, "my_pkg::IDLE"),
                (5, 16, llg::ffi::vpi::vpiRefObj, "P"),
                (6, 16, llg::ffi::vpi::vpiRefObj, "IDLE"),
                (7, 16, llg::ffi::vpi::vpiRefObj, "my_pkg"),
            ],
            "/x/u.sv",
        );

        let int_ty = || TypeInfo {
            kind: "int".to_owned(),
            width: None,
            signed: true,
            type_name: None,
        };
        let int_val = |v: u64| Val::Bits(Value::from_u64(v, 32, true));
        let enum_val = |v: u64| Val::Bits(Value::from_u64(v, 2, true));
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![InstanceModel {
                name: "top".to_owned(),
                def_name: "top".to_owned(),
                full_name: "top".to_owned(),
                file: Some("/x/u.sv".to_owned()),
                line: 1,
                col: 1,
                ports: Vec::new(),
                signals: Vec::new(),
                params: Vec::new(),
                gen_scopes: Vec::new(),
                funcs: Vec::new(),
                children: Vec::new(),
            }],
            modules: vec![ModuleDef {
                name: "top".to_owned(),
                file: Some("/x/u.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 11,
            }],
            packages: vec![PackageDef {
                name: "my_pkg".to_owned(),
                file: Some("/x/p.sv".to_owned()),
                line: 1,
                col: 1,
                params: vec![ParamModel {
                    name: "P".to_owned(),
                    value: Some(int_val(3)),
                    ty: int_ty(),
                    local: false,
                }],
                enum_consts: vec![
                    EnumConstDef {
                        name: "IDLE".to_owned(),
                        value: Some(enum_val(0)),
                        file: Some("/x/p.sv".to_owned()),
                        line: 3,
                        col: 30,
                    },
                    EnumConstDef {
                        name: "RUN".to_owned(),
                        value: Some(enum_val(1)),
                        file: Some("/x/p.sv".to_owned()),
                        line: 3,
                        col: 36,
                    },
                ],
            }],
            classes: Vec::new(),
        };
        Analysis::new(Vec::new(), model, vec![p_file, u_file], Vec::new())
    }

    #[test]
    fn package_item_definition_resolves_full_and_bare_spellings() {
        let a = package_item_analysis();
        // `my_pkg::P` (0-based (2,15) in u.sv) → P decl at (1,16) in p.sv.
        let loc = definition_at(&a, "/x/u.sv", 2, 15).expect("definition of my_pkg::P ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
        // `my_pkg::IDLE` (0-based (3,15)) → IDLE decl at (2,29) in p.sv.
        let loc = definition_at(&a, "/x/u.sv", 3, 15).expect("definition of my_pkg::IDLE ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(2, 29), "loc: {loc:?}");
        // Bare `P` (0-based (4,15)) falls back to the workspace-wide same-name
        // declaration: the package param.
        let loc = definition_at(&a, "/x/u.sv", 4, 15).expect("definition of bare P ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
        // Bare `IDLE` (0-based (5,15)) likewise resolves to the package enum
        // const.
        let loc = definition_at(&a, "/x/u.sv", 5, 15).expect("definition of bare IDLE ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(2, 29), "loc: {loc:?}");
        // `my_pkg` alone (0-based (6,15)) → the package declaration.
        let loc = definition_at(&a, "/x/u.sv", 6, 15).expect("definition of my_pkg ref");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(0, 8), "loc: {loc:?}");
    }

    #[test]
    fn parse_enum_binding_uses_member_range_and_rejects_ambiguous_target() {
        let base = package_item_analysis();
        let target = ParseEnumDecl {
            name: "IDLE".to_owned(),
            file: "/x/p.sv".to_owned(),
            line1: 3,
            col1: 30,
            scope: Some("my_pkg".to_owned()),
        };
        let use_key = ("/x/u.sv".to_owned(), 3, 23);
        let mut bindings = HashMap::new();
        bindings.insert(
            use_key.clone(),
            DeclTarget {
                name: "IDLE".to_owned(),
                kind: "enum constant".to_owned(),
                file: "/x/p.sv".to_owned(),
                line0: 2,
                col0: 29,
                via_label: false,
                via_connection: false,
            },
        );
        let a = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            base.model,
            base.tokens,
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_enum_decls: vec![target.clone()],
                parse_enum_bindings: bindings,
                parse_enum_ref_positions: [use_key.clone()].into_iter().collect(),
                parse_enum_tokens: vec![VObjectInfo {
                    line: 4,
                    col: 24,
                    end_line: 4,
                    end_col: 28,
                    vpi_type: llg::ffi::vpi::uhdmenum_const,
                    name: Some("IDLE".to_owned()),
                    file: "/x/u.sv".to_owned(),
                }],
                ..ConnectionInputs::default()
            },
        );
        // The qualified token's package prefix starts at column 15, but the
        // binding key and returned range are anchored to the member at 23.
        let loc = definition_at(&a, "/x/u.sv", 3, 23).expect("enum member binding");
        assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
        assert_eq!(
            loc.range,
            Range::new(Position::new(2, 29), Position::new(2, 33))
        );
        assert!(references_at(&a, "/x/u.sv", 3, 23)
            .iter()
            .any(|location| location.range.start == Position::new(2, 29)));

        let ambiguous = package_item_analysis();
        let second = ParseEnumDecl {
            name: "IDLE".to_owned(),
            file: "/x/other.sv".to_owned(),
            line1: 7,
            col1: 12,
            scope: Some("my_pkg".to_owned()),
        };
        let ambiguous = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            ambiguous.model,
            ambiguous.tokens,
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_enum_decls: vec![target, second],
                unresolved_enum_refs: [use_key.clone()].into_iter().collect(),
                parse_enum_ref_positions: [use_key.clone()].into_iter().collect(),
                parse_enum_tokens: vec![VObjectInfo {
                    line: 4,
                    col: 24,
                    end_line: 4,
                    end_col: 28,
                    vpi_type: llg::ffi::vpi::uhdmenum_const,
                    name: Some("IDLE".to_owned()),
                    file: "/x/u.sv".to_owned(),
                }],
                ..ConnectionInputs::default()
            },
        );
        assert!(definition_at(&ambiguous, "/x/u.sv", 3, 23).is_none());
        assert!(references_at(&ambiguous, "/x/u.sv", 3, 23).is_empty());
    }

    #[test]
    fn parse_class_qualified_enum_binding_targets_the_member() {
        let target = ParseEnumDecl {
            name: "READY".to_owned(),
            file: "/x/classes.sv".to_owned(),
            line1: 2,
            col1: 27,
            scope: Some("StateHolder".to_owned()),
        };
        let key = ("/x/use.sv".to_owned(), 3, 23);
        let mut bindings = HashMap::new();
        bindings.insert(
            key.clone(),
            DeclTarget {
                name: "READY".to_owned(),
                kind: "enum constant".to_owned(),
                file: target.file.clone(),
                line0: 1,
                col0: 26,
                via_label: false,
                via_connection: false,
            },
        );
        let a = Analysis::new_with_outcome(
            AnalysisOutcome::Valid,
            Vec::new(),
            empty_design(),
            vec![FileTokens {
                path: "/x/use.sv".to_owned(),
                nodes: vec![VObjectInfo {
                    line: 4,
                    col: 11,
                    end_line: 4,
                    end_col: 28,
                    vpi_type: llg::ffi::vpi::vpiRefObj,
                    name: Some("StateHolder::READY".to_owned()),
                    file: "/x/use.sv".to_owned(),
                }],
            }],
            Vec::new(),
            HashMap::new(),
            ConnectionInputs {
                parse_enum_decls: vec![target],
                parse_enum_bindings: bindings,
                parse_enum_ref_positions: [key].into_iter().collect(),
                parse_enum_tokens: vec![VObjectInfo {
                    line: 4,
                    col: 24,
                    end_line: 4,
                    end_col: 24,
                    vpi_type: llg::ffi::vpi::uhdmenum_const,
                    name: Some("READY".to_owned()),
                    file: "/x/use.sv".to_owned(),
                }],
                ..ConnectionInputs::default()
            },
        );
        let loc = definition_at(&a, "/x/use.sv", 3, 23).expect("class enum member");
        assert_eq!(loc.uri, Url::from_file_path("/x/classes.sv").unwrap());
        assert_eq!(loc.range.start, Position::new(1, 26));
        assert_eq!(loc.range.end, Position::new(1, 31));
    }

    #[test]
    fn package_item_hover_shows_enum_value() {
        let a = package_item_analysis();
        // IDLE decl at 0-based (2,29) in p.sv.
        let hover = hover_at(&a, "/x/p.sv", 2, 29).expect("hover on IDLE decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("IDLE"), "value: {value}");
        assert!(value.contains("enum const"), "value: {value}");
        assert!(value.contains("0"), "value: {value}");
        // Hover through the use-site ref shows the same package item.
        let hover = hover_at(&a, "/x/u.sv", 3, 15).expect("hover on my_pkg::IDLE ref");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("IDLE"), "value: {value}");
    }

    #[test]
    fn package_item_completion_after_scope_prefix() {
        let a = package_item_analysis();
        // Cursor at the end of `my_pkg::` (8 chars) → the scope branch fires.
        let items = completion_at(&a, "/x/u.sv", 0, 8, "my_pkg::");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"P"), "items: {labels:?}");
        assert!(labels.contains(&"IDLE"), "items: {labels:?}");
        assert!(labels.contains(&"RUN"), "items: {labels:?}");
        // Prefix filtering applies to the item after `::` (cursor at the end
        // of the typed prefix, so col == line length).
        let items = completion_at(&a, "/x/u.sv", 0, 9, "my_pkg::I");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"IDLE"), "items: {labels:?}");
        assert!(!labels.contains(&"P"), "items: {labels:?}");
        // Unknown packages offer nothing.
        let items = completion_at(&a, "/x/u.sv", 0, 6, "nope::");
        assert!(items.is_empty(), "items: {items:?}");
    }

    #[test]
    fn package_document_symbol_stays_flat() {
        let a = package_item_analysis();
        let syms = document_symbols(&a, "/x/p.sv");
        let pkg = syms
            .iter()
            .find(|s| s.name == "my_pkg" && s.kind == SymbolKind::PACKAGE)
            .expect("package symbol");
        // v1: package items are not document children (they surface through
        // completion/hover/goto); the package symbol itself is flat and uses
        // the model declaration position (1-based (1,1) → 0-based (0,0)).
        assert!(pkg.children.is_none(), "children: {:?}", pkg.children);
        assert_eq!(pkg.range.start, Position::new(0, 0));
    }

    /// Full compile of a two-file design with package items used from a
    /// module: `my_pkg::P` in a parameter context, qualified enum members,
    /// an imported bare member, and `my_pkg::RUN` in a case.  Runs in a fresh
    /// temp dir (Surelog writes `slpp_all/` into the CWD).
    ///
    /// UHDM folds package enum expressions into constants.  The parse-backed
    /// pass preserves the member coordinates and supplies the missing
    /// reference bindings without changing the normal LSP provider path.
    #[test]
    fn analyze_full_pipeline_package_items() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_pkg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let p_sv = dir.join("p.sv");
        let u_sv = dir.join("u.sv");
        std::fs::write(
            &p_sv,
            "package my_pkg;\n  parameter int P = 3;\n  typedef enum logic [1:0] { IDLE, RUN } state_t;\nendpackage\n",
        )
        .expect("write p.sv");
        std::fs::write(
            &u_sv,
            "module top;\n  import my_pkg::*;\n  parameter int W = my_pkg::P;\n  logic [1:0] s;\n  logic [1:0] x;\n  always_comb begin\n    s = my_pkg::IDLE;\n    s = IDLE;\n    case (x)\n      my_pkg::IDLE: s = 2'b00;\n      default: s = my_pkg::RUN;\n    endcase\n  end\nendmodule\n",
        )
        .expect("write u.sv");
        let opts = CompileOpts {
            files: vec![
                p_sv.to_string_lossy().into_owned(),
                u_sv.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );

        // The model carries the package items.
        let pkg = a
            .model
            .packages
            .iter()
            .find(|p| clean_name(&p.name) == "my_pkg")
            .expect("my_pkg in model");
        assert_eq!(pkg.params.len(), 1, "params: {:?}", pkg.params);
        let p = &pkg.params[0];
        assert_eq!(p.name, "P");
        assert_eq!(
            p.value.as_ref().and_then(|v| match v {
                Val::Bits(b) => b.to_u64(),
                Val::Str(_) | Val::Real(_) => None,
            }),
            Some(3),
            "P value: {:?}",
            p.value
        );
        let const_names: Vec<&str> = pkg.enum_consts.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            const_names,
            vec!["IDLE", "RUN"],
            "enum consts: {:?}",
            pkg.enum_consts
        );
        let idle = pkg
            .enum_consts
            .iter()
            .find(|e| e.name == "IDLE")
            .expect("IDLE enum const");
        assert_eq!(
            idle.value.as_ref().and_then(|v| match v {
                Val::Bits(b) => b.to_u64(),
                Val::Str(_) | Val::Real(_) => None,
            }),
            Some(0)
        );

        // The index carries the item declarations with their package scope.
        let p_decl = a
            .index
            .decls
            .iter()
            .find(|d| {
                d.name == "P" && d.kind == SymKind::Param && d.scope.as_deref() == Some("my_pkg")
            })
            .expect("P decl in index");
        assert_eq!(p_decl.file, p_sv.to_string_lossy());
        assert_eq!(p_decl.detail.as_deref(), Some("parameter P: int = 32'sd3"));
        let idle_decl = a
            .index
            .decls
            .iter()
            .find(|d| {
                d.name == "IDLE"
                    && d.kind == SymKind::EnumConst
                    && d.scope.as_deref() == Some("my_pkg")
            })
            .expect("IDLE decl in index");
        assert!(idle_decl.detail.as_deref().unwrap_or("").contains("IDLE"));

        // Definition on the decl positions resolves in place (cross-file from
        // the package file's perspective is trivially the same file).
        let loc = definition_at(&a, &p_sv.to_string_lossy(), p_decl.line, p_decl.col)
            .expect("definition of P decl");
        assert_eq!(loc.range.start, Position::new(p_decl.line, p_decl.col));
        let loc = definition_at(&a, &p_sv.to_string_lossy(), idle_decl.line, idle_decl.col)
            .expect("definition of IDLE decl");
        assert_eq!(
            loc.range.start,
            Position::new(idle_decl.line, idle_decl.col)
        );

        // Hover on the enum const decl shows its name/value.
        let hover = hover_at(&a, &p_sv.to_string_lossy(), idle_decl.line, idle_decl.col)
            .expect("hover on IDLE decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("IDLE"), "value: {value}");

        let source = std::fs::read_to_string(&u_sv).expect("read u.sv");
        let member_position = |line0: u32| {
            let line = source.lines().nth(line0 as usize).expect("source line");
            let byte_col = line.find("IDLE").expect("IDLE member");
            (line0, line[..byte_col].encode_utf16().count() as u32)
        };
        for line0 in [6, 9] {
            let (use_line, use_col) = member_position(line0);
            let loc = definition_at(&a, &u_sv.to_string_lossy(), use_line, use_col)
                .expect("qualified enum member definition");
            assert_eq!(
                loc.range.start,
                Position::new(idle_decl.line, idle_decl.col),
                "qualified use at {use_line}:{use_col}"
            );
        }
        let (bare_line, bare_col) = member_position(7);
        let loc = definition_at(&a, &u_sv.to_string_lossy(), bare_line, bare_col)
            .expect("imported bare enum member definition");
        assert_eq!(
            loc.range.start,
            Position::new(idle_decl.line, idle_decl.col)
        );

        // Completion after `my_pkg::` offers the package items.
        let items = completion_at(&a, &u_sv.to_string_lossy(), 0, 8, "my_pkg::");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"P"), "items: {labels:?}");
        assert!(labels.contains(&"IDLE"), "items: {labels:?}");
        assert!(labels.contains(&"RUN"), "items: {labels:?}");
    }

    /// Hand-built `Analysis` for /x/c.sv:
    ///
    /// ```text
    /// class Counter;                  ← class name at 1-based (1,7)
    ///   int count;                    ← field at 1-based (2,7)
    ///   function new();               ← constructor at 1-based (3,3)
    ///     count = 0;
    ///   endfunction
    ///   function int get();           ← get at 1-based (6,3)
    ///     get = count;
    ///   endfunction
    /// endclass
    /// ```
    ///
    /// Model positions mirror the real pipeline: the class points at the
    /// `class` keyword, methods at the `function` keyword (Surelog's own
    /// position), the field at its identifier.  Tokens mirror the parse-tree
    /// name tokens (`uhdmclass_defn` for the class name, `vpiFunction` for
    /// method names) plus the VPI-walker field token (`uhdmint_var`).
    fn class_analysis() -> Analysis {
        let int_ty = || TypeInfo {
            kind: "int".to_owned(),
            width: None,
            signed: true,
            type_name: None,
        };
        let counter = ClassDef {
            name: "Counter".to_owned(),
            file: Some("/x/c.sv".to_owned()),
            line: 1,
            col: 1,
            methods: vec![
                FuncDef {
                    name: "new".to_owned(),
                    is_task: false,
                    automatic: false,
                    file: Some("/x/c.sv".to_owned()),
                    line: 3,
                    col: 3,
                    ret: None,
                    args: Vec::new(),
                    scope: "Counter".to_owned(),
                },
                FuncDef {
                    name: "get".to_owned(),
                    is_task: false,
                    automatic: false,
                    file: Some("/x/c.sv".to_owned()),
                    line: 6,
                    col: 3,
                    ret: Some(int_ty()),
                    args: Vec::new(),
                    scope: "Counter".to_owned(),
                },
            ],
            fields: vec![ClassFieldDef {
                name: "count".to_owned(),
                ty: int_ty(),
                line: 2,
                col: 7,
            }],
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: Vec::new(),
            modules: Vec::new(),
            packages: Vec::new(),
            classes: vec![counter],
        };
        let tokens = vec![FileTokens {
            path: "/x/c.sv".to_owned(),
            nodes: vec![
                VObjectInfo {
                    line: 1,
                    col: 7,
                    end_line: 1,
                    end_col: 14,
                    vpi_type: llg::ffi::vpi::uhdmclass_defn,
                    name: Some("Counter".to_owned()),
                    file: "/x/c.sv".to_owned(),
                },
                VObjectInfo {
                    line: 2,
                    col: 7,
                    end_line: 2,
                    end_col: 12,
                    vpi_type: llg::ffi::vpi::uhdmint_var,
                    name: Some("count".to_owned()),
                    file: "/x/c.sv".to_owned(),
                },
                VObjectInfo {
                    line: 3,
                    col: 13,
                    end_line: 3,
                    end_col: 16,
                    vpi_type: llg::ffi::vpi::vpiFunction,
                    name: Some("new".to_owned()),
                    file: "/x/c.sv".to_owned(),
                },
                VObjectInfo {
                    line: 6,
                    col: 16,
                    end_line: 6,
                    end_col: 19,
                    vpi_type: llg::ffi::vpi::vpiFunction,
                    name: Some("get".to_owned()),
                    file: "/x/c.sv".to_owned(),
                },
            ],
        }];
        Analysis::new(Vec::new(), model, tokens, Vec::new())
    }

    #[test]
    fn hover_on_class_name_shows_members() {
        let a = class_analysis();
        // `Counter` at 1-based (1,7) → 0-based (0,6).
        let hover = hover_at(&a, "/x/c.sv", 0, 6).expect("hover on class name");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("class Counter"), "value: {value}");
        assert!(value.contains("get"), "method list missing: {value}");
        assert!(value.contains("count"), "field list missing: {value}");
    }

    #[test]
    fn hover_on_class_method_shows_signature() {
        let a = class_analysis();
        // `function int get()` at 1-based (6,3) → 0-based (5,2).
        let hover = hover_at(&a, "/x/c.sv", 5, 2).expect("hover on get decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("function int get()"), "value: {value}");
        // Hover on the method *name* (0-based (5,15)) falls back to the
        // parse-tree token and still shows the signature.
        let hover = hover_at(&a, "/x/c.sv", 5, 15).expect("hover on get name");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("function int get()"), "value: {value}");
    }

    #[test]
    fn document_symbols_include_class_with_members() {
        let a = class_analysis();
        let syms = document_symbols(&a, "/x/c.sv");
        let cls = syms
            .iter()
            .find(|s| s.name == "Counter")
            .expect("class symbol");
        assert_eq!(cls.kind, SymbolKind::CLASS);
        let children = cls.children.as_ref().expect("class children");
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"new"), "children: {names:?}");
        assert!(names.contains(&"get"), "children: {names:?}");
        assert!(names.contains(&"count"), "children: {names:?}");
        let get = children
            .iter()
            .find(|c| c.name == "get")
            .expect("get child");
        assert_eq!(
            get.detail.as_deref(),
            Some("function int get()"),
            "child detail missing: {get:?}"
        );
    }

    #[test]
    fn completion_after_class_scope_prefix_offers_members() {
        let a = class_analysis();
        let items = completion_at(&a, "/x/c.sv", 0, 9, "Counter::");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"new"), "items: {labels:?}");
        assert!(labels.contains(&"get"), "items: {labels:?}");
        assert!(labels.contains(&"count"), "items: {labels:?}");
        assert!(
            items
                .iter()
                .any(|i| i.label == "get" && i.kind == Some(CompletionItemKind::FUNCTION)),
            "get must be a function: {items:?}"
        );
        assert!(
            items
                .iter()
                .any(|i| i.label == "count" && i.kind == Some(CompletionItemKind::VARIABLE)),
            "count must be a variable: {items:?}"
        );
        // Prefix filtering applies after `::`.
        let items = completion_at(&a, "/x/c.sv", 0, 10, "Counter::g");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"get"), "items: {labels:?}");
        assert!(!labels.contains(&"count"), "items: {labels:?}");
        // Unknown classes offer nothing.
        let items = completion_at(&a, "/x/c.sv", 0, 8, "Nope::");
        assert!(items.is_empty(), "items: {items:?}");
    }

    /// Full compile of a design with a class declaration used from a module:
    /// the model and index must pick up the class, its methods (`new`/`get`)
    /// and its field (`count`), and the LSP features must surface them.  Runs
    /// in a fresh temp dir (Surelog writes `slpp_all/` into the CWD).
    #[test]
    fn analyze_full_pipeline_classes() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_class_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let sv = dir.join("c.sv");
        std::fs::write(
            &sv,
            "class Counter;\n  int count;\n  function new();\n    count = 0;\n  endfunction\n  function int get();\n    get = count;\n  endfunction\nendclass\nmodule top;\n  Counter c;\nendmodule\n",
        )
        .expect("write design");
        let path = sv.to_string_lossy().into_owned();
        let opts = CompileOpts {
            files: vec![path.clone()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );

        // Model: the class carries methods (constructor first) and fields.
        let cls = a
            .model
            .classes
            .iter()
            .find(|c| clean_name(&c.name) == "Counter")
            .expect("Counter class in model");
        let method_names: Vec<&str> = cls.methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            method_names,
            vec!["new", "get"],
            "methods: {:?}",
            cls.methods
        );
        let get = cls
            .methods
            .iter()
            .find(|m| m.name == "get")
            .expect("get method");
        assert_eq!(get.ret.as_ref().map(|t| t.kind.as_str()), Some("int"));
        assert_eq!(get.scope, "Counter", "method scope: {get:?}");
        let new = cls.methods.iter().find(|m| m.name == "new").expect("ctor");
        assert_eq!(new.ret, None, "constructor has no source return type");
        let field_names: Vec<&str> = cls.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(field_names, vec!["count"], "fields: {:?}", cls.fields);

        // Index: class/method/field decls with the class scope.
        let class_decl = a
            .index
            .decls
            .iter()
            .find(|d| d.name == "Counter" && d.kind == SymKind::Class)
            .expect("class decl");
        assert_eq!(class_decl.file, path);
        let get_decl = a
            .index
            .decls
            .iter()
            .find(|d| {
                d.name == "get"
                    && d.kind == SymKind::Function
                    && d.scope.as_deref() == Some("Counter")
            })
            .expect("get method decl");
        let field_decl = a
            .index
            .decls
            .iter()
            .find(|d| {
                d.name == "count" && d.kind == SymKind::Var && d.scope.as_deref() == Some("Counter")
            })
            .expect("count field decl");
        assert_eq!(
            get_decl.detail.as_deref(),
            Some("function int get()"),
            "get detail: {get_decl:?}"
        );
        assert_eq!(
            field_decl.detail.as_deref(),
            Some("int count"),
            "count detail: {field_decl:?}"
        );

        // Hover on the class decl shows the members; on the method, the
        // signature.
        let hover =
            hover_at(&a, &path, class_decl.line, class_decl.col).expect("hover on Counter decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("class Counter"), "value: {value}");
        assert!(value.contains("get"), "value: {value}");
        let hover = hover_at(&a, &path, get_decl.line, get_decl.col).expect("hover on get decl");
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        };
        assert!(value.contains("function int get()"), "value: {value}");

        // Document symbols: Counter with new/get/count children.
        let syms = document_symbols(&a, &path);
        let cls_sym = syms
            .iter()
            .find(|s| s.name == "Counter")
            .expect("Counter symbol");
        assert_eq!(cls_sym.kind, SymbolKind::CLASS);
        let children = cls_sym.children.as_ref().expect("class children");
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        for want in ["new", "get", "count"] {
            assert!(names.contains(&want), "children: {names:?}");
        }

        // Completion after `Counter::` offers methods and fields.
        let items = completion_at(&a, &path, 0, 9, "Counter::");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        for want in ["new", "get", "count"] {
            assert!(labels.contains(&want), "items: {labels:?}");
        }
    }

    #[test]
    fn shadow_path_round_trips_absolute_paths() {
        let base = process_shadow_base();
        for real in [
            "/repo/rtl/top.sv",
            "/tmp/proj/sub dir/top.sv",
            "/a/b/c/d.sv",
            "/workspaces/llg/src/bin/llg/features.rs",
        ] {
            let shadow = shadow_path(Path::new(real), &base);
            assert!(
                shadow.starts_with(&base),
                "shadow not under the tree: {shadow:?}"
            );
            assert_eq!(
                real_path(&shadow, &base),
                Some(PathBuf::from(real)),
                "round-trip failed for {real}"
            );
        }
    }

    #[test]
    fn real_path_rejects_paths_outside_shadow_tree() {
        let base = process_shadow_base();
        assert_eq!(real_path(Path::new("/repo/rtl/top.sv"), &base), None);
        assert_eq!(real_path(Path::new("/other/x.sv"), &base), None);
        // The shadow tree root itself has no real path.
        assert_eq!(real_path(&base, &base), None);
    }

    /// Full compile of a design staged at its deterministic shadow path: the
    /// analysis must be keyed by the shadow path (model, tokens, lint).
    /// Runs in a fresh temp dir (Surelog writes `slpp_all/` into the CWD).
    #[test]
    fn analyze_full_pipeline_compiles_shadow_path() {
        let _guards = analysis_guards();
        let dir = std::env::temp_dir().join(format!("llg_llg_bin_shadow_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");

        // Stage an unsaved buffer: write the *shadow* copy only; the real file
        // exists on disk too (as in a workspace) but the compile must read the
        // shadow copy.
        let real = dir.join("rtl").join("top.sv");
        std::fs::create_dir_all(real.parent().expect("parent dir")).expect("create rtl dir");
        std::fs::write(&real, "module top; endmodule\n").expect("write real file");
        let shadow = shadow_path(&real, &dir);
        std::fs::create_dir_all(shadow.parent().expect("shadow parent"))
            .expect("create shadow dir");
        std::fs::write(&shadow, "module top; logic unused_sig; endmodule\n")
            .expect("write shadow file");

        let shadow_str = shadow.to_string_lossy().into_owned();
        let opts = CompileOpts {
            files: vec![shadow_str.clone()],
            top: None,
            ..Default::default()
        };
        let a = analyze(&opts);
        assert!(
            !a.diagnostics.iter().any(|d| matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )),
            "unexpected diagnostics: {:?}",
            a.diagnostics
        );
        assert!(
            a.model
                .modules
                .iter()
                .any(|m| m.file.as_deref() == Some(shadow_str.as_str())),
            "module files: {:?}",
            a.model
                .modules
                .iter()
                .map(|m| m.file.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            a.tokens.iter().any(|ft| ft.path == shadow_str),
            "token files: {:?}",
            a.tokens
                .iter()
                .map(|ft| ft.path.clone())
                .collect::<Vec<_>>()
        );
        // The lint finding (unused signal) is keyed by the shadow path too.
        let map = lsp_diagnostics(&a);
        let diags = map
            .iter()
            .find(|(f, _)| *f == &shadow_str)
            .map(|(_, v)| v)
            .expect("diagnostics for the shadow path");
        assert!(
            diags.iter().any(|d| {
                d.source.as_deref() == Some("llg-lint") && d.message.contains("unused_sig")
            }),
            "unused-signal lint missing: {diags:?}"
        );
    }
}
