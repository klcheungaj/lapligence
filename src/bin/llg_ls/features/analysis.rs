//! Blocking frontend analysis and assembly of owned feature snapshots.

use super::*;

/// Result class for a completed analysis pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisOutcome {
    Valid,
    Fatal,
    Parse,
    Compile,
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
/// Owned and `Send`; the Slang session it was produced from has already been
/// dropped when this value becomes visible to callers.
pub struct Analysis {
    /// Whether this analysis is safe to publish as a replacement snapshot.
    pub outcome: AnalysisOutcome,
    /// Slang diagnostics (1-based positions; `file`/`line`/`col` may be
    /// unknown).
    pub diagnostics: Vec<Diag>,
    /// Complete named Slang diagnostics projected while admitted buffers are
    /// available, including related source ranges.
    pub frontend_diagnostics: Vec<(String, LspDiagnostic)>,
    /// Elaborated design model. A default/empty model is used when compilation
    /// or semantic database construction fails.
    pub model: DesignModel,
    /// Per-file semantic token lists collected while the session was alive.
    pub tokens: Vec<FileTokens>,
    /// Workspace symbol index (declarations + reference sites) built from the
    /// model and the tokens.
    pub index: SymbolIndex,
    /// Reference occurrence → bound declaration, captured from Slang lexical
    /// semantic IDs. Keys are the 0-based positions of emitted reference
    /// tokens, so definition requests at exact positions are binding-precise.
    /// Resolved named connection labels are folded in as well, targeting the
    /// child module's declaration — together with the paired connection
    /// ACTUALS (`.clk(wa)` binds `.clk` to the child's port and `wa` to
    /// `wa`'s own declaration in the instantiating/parent scope;
    /// `#(.W(expr))` binds `.W` to the child's parameter and `expr`'s
    /// leading identifier to its own parent-scope declaration).
    /// Includes parse-backed enum bindings when semantic DB folded a package/class
    /// use into a literal; the same facts are available in the lexical
    /// fallback alongside connection bindings.
    pub ref_bindings: RefBindings,
    /// Slang reference positions that are unresolved or ambiguous. Request
    /// handling must not replace the missing semantic identity with a name
    /// match from another scope.
    pub(crate) unresolved_bindings: tokens::UnresolvedBindings,
    /// Rendered declaration snippets (`logic [3:0] val`, …) keyed by
    /// `(file, line1, col1)` of the declaration token, captured from semantic DB
    /// during the same walk as [`Analysis::ref_bindings`].  Position-accurate
    /// even where several same-named declarations live in one module (inner-
    /// scope shadowing), so hover text describes exactly the object at that
    /// position instead of the first name match in the model.  Empty for
    /// parse-fallback analyses (the model supplies details there).
    pub decl_details: tokens::DeclDetails,
    /// Lint findings from the shared linter (`source: "llg-lint"` when
    /// published), produced alongside the Slang diagnostics.  1-based
    /// positions.  Empty when compilation failed or no semantic DB design was built.
    pub lint: Vec<LintDiag>,
    /// Source-level module definitions and instance edges retained while the
    /// Slang parse tree was alive.  The module explorer uses this graph for
    /// definitions that configured elaboration did not instantiate.
    pub(crate) module_graph: ModuleGraph,
    /// The effective `[compile] top` used for this analysis, retained so the
    /// explorer can apply the configured-top root union without rereading
    /// configuration during a request.
    pub(crate) configured_top: Option<String>,
    /// Preprocessor macro table ([`core::macros::MacroTable`]) over the exact
    /// compiled sources: config `[compile] defines` seed every file and one
    /// conservative scan per file resolves in-source `` `define ``/`` `undef ``
    /// positionally (see `core::macros` for the documented
    /// semantics).  Built once per analysis commit — never inside a request —
    /// so macro-usage hover stays a pure read over committed data.  Empty for
    /// hand-built analyses.
    pub(super) macros: macros::MacroTable,
}

impl Analysis {
    /// Assemble an [`Analysis`] from its parts, computing the symbol index.
    ///
    /// Public so tests can build an [`Analysis`] without running Slang; the
    /// production pipeline uses the same constructor inside [`analyze`].
    ///
    /// Passes an empty semantic DB binding map — hand-built analyses have no
    /// elaborated design behind them, matching the lexical fallback shape.
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
    /// `semantic_bindings` carries the reference→declaration identities
    /// captured from Slang's lexical table. The
    /// final map is the union of those, the port-connection-derived ones (see
    /// [`ConnectionInputs`]), and nothing else:
    ///
    /// * resolved port-label targets are inserted FIRST (tagged
    ///   `via_label`), each followed by its paired connection ACTUAL target
    ///   (tagged `via_connection`) so the label navigates to the child port and
    ///   the ACTUAL to the parent-scope declaration;
    /// * parse-backed enum bindings are inserted before fallback and semantic DB;
    /// * parse-fallback connection bindings are inserted next (they only
    ///   exist when no semantic DB design was produced);
    /// * semantic DB bindings are inserted after them, winning collisions against the
    ///   label/fallback inputs because they reflect what elaboration actually
    ///   bound;
    /// * semantic DB-mode ACTUAL bindings are re-inserted LAST but never override an
    ///   existing entry: elaboration-backed targets at an actual position are
    ///   already binding-precise, so the parent-scope fold only fills positions
    ///   no explicit binding captured.
    pub fn new_with_outcome(
        outcome: AnalysisOutcome,
        diagnostics: Vec<Diag>,
        model: DesignModel,
        tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
        semantic_bindings: RefBindings,
        connections: ConnectionInputs,
    ) -> Analysis {
        Self::new_with_outcome_and_sources(
            outcome,
            diagnostics,
            model,
            tokens,
            lint,
            semantic_bindings,
            connections,
            &FeatureSourceMaps::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_outcome_and_sources(
        outcome: AnalysisOutcome,
        diagnostics: Vec<Diag>,
        mut model: DesignModel,
        mut tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
        semantic_bindings: RefBindings,
        connections: ConnectionInputs,
        source_maps: &FeatureSourceMaps,
    ) -> Analysis {
        normalize_model_positions(source_maps, &mut model);
        append_synthetic_tokens(&mut tokens, &connections.parse_enum_tokens);
        let mut index = SymbolIndex::from_parts(
            &model,
            &tokens,
            connections.parse_decls.as_ref(),
            &connections.pairs,
            &connections.parse_enum_decls,
            &connections.parse_enum_ref_positions,
            &connections.unresolved_enum_refs,
        );
        index.attach_exact_label_bindings(&tokens, &semantic_bindings);
        let ref_bindings = merged_ref_bindings(&index, &model, semantic_bindings, &connections);
        Analysis {
            outcome,
            diagnostics,
            frontend_diagnostics: Vec::new(),
            model,
            tokens,
            index,
            ref_bindings,
            unresolved_bindings: connections.unresolved_bindings,
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
    pub(super) fn with_macros(mut self, macros: macros::MacroTable) -> Analysis {
        self.macros = macros;
        self
    }

    /// Attach the source graph captured during the same Slang pass.
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

    /// Attach the declaration snippets captured during the semantic token walk.
    ///
    /// Every indexed DECLARATION entry whose position matches a captured
    /// snippet gets its hover text replaced by the position-accurate one —
    /// under inner-scope shadowing the name-based model lookup would describe
    /// the outer same-named object.  The map is also retained on the analysis
    /// so hover can render bound targets whose declaration has no indexed
    /// entry.  Production pipeline only; hand-built analyses have no semantic DB
    /// behind them and keep the model-derived details.
    pub(super) fn with_decl_details(mut self, details: tokens::DeclDetails) -> Analysis {
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
    /// or semantic tokens).  [`AnalysisOutcome::Fatal`] means no usable semantic DB
    /// existed (poisoned database, preflight aborts), so those analyses stay
    /// feature-less; a partial analysis — some files failed to parse or
    /// compile while Slang still elaborated the surviving set — serves
    /// best-effort navigation, and a syntax-broken project (Slang skipped
    /// its whole compile/semantic DB stage) serves declaration-level data through the
    /// lexical fallback built from [`module_graph_from_slang`]. An analysis whose
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

    /// Attach the complete Slang diagnostic projection.
    pub(crate) fn attach_frontend_diagnostics(
        &mut self,
        diagnostics: Vec<(String, LspDiagnostic)>,
    ) {
        self.frontend_diagnostics = diagnostics;
    }
}

/// Add parse-backed qualified enum members that Slang does not expose as a
/// standalone semantic/parse token.  Existing positions win so a normal parse/semantic
/// token keeps its richer classification.
pub(super) fn append_synthetic_tokens(tokens: &mut Vec<FileTokens>, synthetic: &[TokenInfo]) {
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

#[derive(Debug)]
pub(super) struct FeatureSourceMap {
    source: String,
    line_starts: Vec<usize>,
}

impl FeatureSourceMap {
    pub(super) fn new(source: String) -> Self {
        let mut line_starts = vec![0];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
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
    /// source-local scanner; raw Slang scalar/byte columns win when those are
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

    pub(super) fn normalize_1based(&self, line: u32, col: u32, name: Option<&str>) -> (u32, u32) {
        (line, self.lsp_column(line, col, name))
    }
}

#[derive(Debug, Default)]
pub(super) struct FeatureSourceMaps {
    by_file: HashMap<String, FeatureSourceMap>,
}

impl FeatureSourceMaps {
    fn get(&self, file: &str) -> Option<&FeatureSourceMap> {
        self.by_file.get(file)
    }

    fn from_sources(sources: &[(&str, &str)]) -> Self {
        let by_file = sources
            .iter()
            .filter(|(_, source)| !source.is_ascii())
            .map(|(file, source)| {
                (
                    (*file).to_owned(),
                    FeatureSourceMap::new((*source).to_owned()),
                )
            })
            .collect();
        Self { by_file }
    }
}

pub(super) fn normalize_model_positions(maps: &FeatureSourceMaps, model: &mut DesignModel) {
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

pub(super) fn normalize_instance_positions(
    maps: &FeatureSourceMaps,
    instances: &mut [InstanceModel],
) {
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
///    resolved to the parent-scope declaration; only non-empty when no semantic DB
///    design exists.
/// 3. **semantic DB bindings** captured during the semantic token walk
///    (`core::tokens::collect_all_tokens`): where both capture paths produced
///    an entry for the same position, the Slang semantic target wins because
///    it reflects what elaboration actually bound,
///    including for connections the label heuristic cannot classify.
/// 4. **Connection ACTUAL bindings** (`via_connection`, semantic DB mode): the
///    paired actual of every resolved label points at the actual signal's
///    OWN declaration in the instantiating (parent) scope.  These are
///    inserted last but only into positions that have no binding yet — an
///    existing explicit binding (elaboration-backed or fallback) wins.
pub(super) fn merged_ref_bindings(
    index: &SymbolIndex,
    model: &DesignModel,
    semantic_bindings: RefBindings,
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
    // inserted AFTER the semantic DB map without overriding it.
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
    // Parse-backed enum bindings fill the gap left when semantic DB folds a
    // package/class-qualified constant use into a literal.  semantic DB remains the
    // authoritative winner if it did emit a binding at the same coordinate.
    for (key, target) in &connections.parse_enum_bindings {
        out.insert(key.clone(), target.clone());
    }
    for (key, target) in &connections.fallback_bindings {
        out.insert(key.clone(), target.clone());
    }
    for (key, target) in semantic_bindings {
        out.insert(key, target);
    }
    // Existing explicit binding wins: never overwrite an semantic DB/fallback entry.
    for (key, target) in actual_bindings {
        out.entry(key).or_insert(target);
    }
    for key in &connections.unresolved_bindings {
        out.remove(key);
    }
    out
}

/// One candidate declaration for parent-scope ACTUAL resolution:
/// a `(0-based line, 0-based column)` position of a declared identifier.
pub(super) type ActualCandidatePos = (u32, u32);

/// A module body span in inclusive 0-based line coordinates.
///
/// `last0 == None` when the end position is unknown;
/// such spans never win containment.
pub(super) struct ModuleSpan0 {
    pub(super) first0: u32,
    pub(super) last0: Option<u32>,
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
pub(super) fn select_parent_scope_position(
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

/// Build the parent-scope binding target for one connection ACTUAL
/// (semantic DB mode): candidates come from the symbol index, the kind from
/// [`SymKind`] via [`kind_label`].
pub(super) fn parent_scope_actual_target(
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

pub(super) fn outcome_from_diagnostics(diagnostics: &[Diag]) -> AnalysisOutcome {
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

pub(super) fn db_build_diagnostic(error: &str) -> Diag {
    Diag {
        severity: Severity::Error,
        file: None,
        line: 0,
        col: 0,
        message: format!("semantic database build failed: {error}"),
    }
}

pub(super) fn bounded_log_text(value: &str, max_bytes: usize) -> String {
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

/// Serializes native compilation so root jobs cannot oversubscribe the process.
pub(super) static COMPILE_LOCK: Mutex<()> = Mutex::new(());

/// Run the full pipeline (compile + elaborate + model build + token
/// collection + lint) in one blocking call with the default lint
/// configuration and return owned results.
///
/// This never panics: if the compile step itself fails to start, the returned
/// [`Analysis`] carries a
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
/// scheduler. The parent is metadata only.
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
        opts.files.len()
            + opts
                .sources
                .iter()
                .filter(|source| source.is_compilation_unit)
                .count(),
        parent_id,
    );
    let mut wait_span = crate::logging::LifecycleSpan::phase_with_parent(
        "slang.wait_global_mutex",
        || root.to_owned(),
        generation,
        opts.files.len()
            + opts
                .sources
                .iter()
                .filter(|source| source.is_compilation_unit)
                .count(),
        Some(analysis_span.id()),
    );
    let _guard = COMPILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    wait_span.outcome("ok");
    drop(wait_span);
    let analysis = analyze_inner_slang(opts, lint_cfg, root, generation, Some(analysis_span.id()));
    analysis_span.complete(
        match analysis.outcome {
            AnalysisOutcome::Valid => "ok",
            AnalysisOutcome::Fatal | AnalysisOutcome::Parse | AnalysisOutcome::Compile => "error",
        },
        analysis.diagnostics.len() + analysis.lint.len(),
    );
    analysis
}

/// Private workspace used for admitted buffer mirrors and lifecycle cleanup.
pub fn analysis_scratch_dir() -> std::path::PathBuf {
    process_shadow_base().join("work").join("analyze")
}

pub(super) fn analyze_inner_slang(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) -> Analysis {
    let files = opts.files.len()
        + opts
            .sources
            .iter()
            .filter(|source| source.is_compilation_unit)
            .count();
    let started = std::time::Instant::now();
    crate::llg_debug!(
        "event=slang.compile.begin root={} generation={} parent_id={:?} files={}",
        root,
        generation,
        parent_id,
        files
    );
    let out = match compile::compile(opts) {
        Ok(out) => out,
        Err(error) => {
            crate::llg_debug!("event=slang.compile.end outcome=error root={} generation={} elapsed_us={} error={}", root, generation, started.elapsed().as_micros(), bounded_log_text(&error.to_string(), 512));
            if error.kind() == compile::StartupErrorKind::LimitExceeded {
                crate::llg_error!(
                    "event=slang.resource_limit root={} generation={} library_units={} max_source_bytes={} max_output_bytes={} max_semantic_nodes={} error={} advice={}",
                    crate::logging::bounded_field(root), generation, opts.library_units,
                    opts.limits.max_source_bytes, opts.limits.max_output_bytes,
                    opts.limits.max_semantic_nodes,
                    bounded_log_text(&error.to_string(), 512),
                    crate::config::FRONTEND_LIMIT_GUIDANCE
                );
            }
            if error.kind() == llg::core::compile::StartupErrorKind::LimitExceeded
                && !opts.library_units
                && opts.files.is_empty()
                && opts.sources.iter().any(|source| source.is_compilation_unit)
            {
                return analyze_library_units_after_limit(
                    opts,
                    lint_cfg,
                    root,
                    generation,
                    parent_id,
                    error.to_string(),
                );
            }
            let message = if error.kind() == compile::StartupErrorKind::LimitExceeded {
                format!("{error}. {}", crate::config::FRONTEND_LIMIT_GUIDANCE)
            } else {
                error.to_string()
            };
            return Analysis::fatal_preflight(message);
        }
    };
    crate::llg_debug!("event=slang.compile.end outcome={} root={} generation={} diagnostics={} semantic_nodes={} lexical_tokens={} elapsed_us={}", if out.ok() { "ok" } else { "error" }, root, generation, out.diagnostics.len(), out.snapshot.semantic_nodes.len(), out.snapshot.lexical_tokens.len(), started.elapsed().as_micros());

    let source_texts: Vec<_> = out
        .snapshot
        .files
        .iter()
        .map(|source| (source.name.as_str(), source.text.as_str()))
        .collect();
    let (tokens, bindings, unresolved_bindings, decl_details, declarations) =
        tokens::project_slang(&out.snapshot, &source_texts);
    crate::llg_debug!(
        "event=slang.tokens.end root={} generation={} tokens={} elapsed_us={}",
        root,
        generation,
        out.snapshot.lexical_tokens.len(),
        started.elapsed().as_micros()
    );
    let frontend_diagnostics = project_snapshot_diagnostics(
        &out.snapshot
            .files
            .iter()
            .map(|source| SlangSource {
                name: &source.name,
                text: &source.text,
                is_compilation_unit: true,
            })
            .collect::<Vec<_>>(),
        &out.snapshot,
    );
    let frontend_ok = out.ok();
    let database = (!opts.library_units).then(|| llg::core::db::Db::from_slang(&out.snapshot));
    let module_graph = module_graph_from_slang(
        &out.snapshot,
        &source_texts,
        database.as_ref().and_then(|result| result.as_ref().ok()),
    );
    let (model, lint_diags, db_error) = match database {
        Some(Ok(db)) => {
            let model = DesignModel::from_db(&db);
            let lint_diags = lint::lint_with_config(&db, &model, lint_cfg);
            (model, lint_diags, None)
        }
        other => {
            let mut model = empty_design();
            model.modules = module_graph
                .definitions
                .iter()
                .map(|definition| ModuleDef {
                    name: definition.name.clone(),
                    file: definition.file.clone(),
                    line: definition.line,
                    col: definition.col,
                    end_line: definition.end_line,
                    end_col: definition.end_col,
                })
                .collect();
            (
                model,
                Vec::new(),
                other.and_then(Result::err).map(|error| error.to_string()),
            )
        }
    };
    let mut diagnostics = out.diagnostics;
    if let Some(error) = db_error {
        diagnostics.push(db_build_diagnostic(&error));
    }
    let outcome = if opts.library_units {
        AnalysisOutcome::Compile
    } else if !frontend_ok {
        outcome_from_diagnostics(&diagnostics)
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        AnalysisOutcome::Compile
    } else {
        AnalysisOutcome::Valid
    };
    let macro_table = macros::build_table(&opts.defines, &source_texts, None);
    let source_maps = FeatureSourceMaps::from_sources(&source_texts);
    let connections = ConnectionInputs {
        parse_decls: Some(declarations),
        unresolved_bindings,
        ..ConnectionInputs::default()
    };
    let mut analysis = Analysis::new_with_outcome_and_sources(
        outcome,
        diagnostics,
        model,
        tokens,
        lint_diags,
        bindings,
        connections,
        &source_maps,
    )
    .with_macros(macro_table)
    .with_decl_details(decl_details)
    .with_module_graph(module_graph)
    .with_configured_top(opts.top.clone());
    analysis.attach_frontend_diagnostics(frontend_diagnostics);
    analysis
}

/// Recover a bounded declaration-level workspace after full elaboration
/// exceeds a native export limit. Slang receives the same admitted buffers in
/// one cross-file compilation but treats every unit as a library unit, which
/// prevents inferred top hierarchies from recursively expanding.
fn analyze_library_units_after_limit(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
    failure: String,
) -> Analysis {
    let recovery_opts = CompileOpts {
        sources: opts.sources.clone(),
        top: None,
        defines: opts.defines.clone(),
        include_dirs: opts.include_dirs.clone(),
        library_units: true,
        limits: opts.limits,
        ..CompileOpts::default()
    };
    crate::llg_debug!(
        "event=slang.limit_recovery.begin root={} generation={} files={} error={}",
        root,
        generation,
        recovery_opts
            .sources
            .iter()
            .filter(|source| source.is_compilation_unit)
            .count(),
        bounded_log_text(&failure, 512)
    );
    let mut analysis = analyze_inner_slang(&recovery_opts, lint_cfg, root, generation, parent_id)
        .with_configured_top(opts.top.clone());
    if !analysis.has_feature_data() {
        crate::llg_debug!(
            "event=slang.limit_recovery.end outcome=error root={} generation={}",
            root,
            generation
        );
        return Analysis::fatal_preflight(format!(
            "{failure}. {}",
            crate::config::FRONTEND_LIMIT_GUIDANCE
        ));
    }
    analysis.outcome = AnalysisOutcome::Compile;
    analysis.diagnostics.push(Diag {
        severity: Severity::Warning,
        file: None,
        line: 0,
        col: 0,
        message: format!(
            "full workspace elaboration exceeded a frontend limit; serving bounded declaration-level navigation ({failure}). {}",
            crate::config::FRONTEND_LIMIT_GUIDANCE
        ),
    });
    analysis.lint.clear();
    crate::llg_debug!(
        "event=slang.limit_recovery.end outcome=ok root={} generation={} tokens={} declarations={} definitions={}",
        root,
        generation,
        analysis.tokens.iter().map(|file| file.nodes.len()).sum::<usize>(),
        analysis.index.decls.len(),
        analysis.module_graph.definitions.len()
    );
    analysis
}

/// Return an empty compile-outcome snapshot for backend failure recovery.
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

pub(super) fn empty_design() -> DesignModel {
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
pub(super) fn lint_settings_object(settings: &LSPAny) -> Option<&LSPObject> {
    let obj = settings.as_object()?;
    match obj.get("lint") {
        Some(LSPAny::Object(lint)) => Some(lint),
        _ if obj.contains_key("rules") || obj.contains_key("enabled") => Some(obj),
        _ => None,
    }
}

/// Map a settings `severity` string to a [`LintSeverity`] (`LintSeverity` has
/// no `FromStr` impl; `core::lint`'s own parser is private).
pub(super) fn parse_severity(value: &str) -> Option<LintSeverity> {
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
