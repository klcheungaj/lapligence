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
    pub(super) macros: macros::MacroTable,
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
    pub(super) fn with_macros(mut self, macros: macros::MacroTable) -> Analysis {
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
    pub(super) fn with_decl_details(mut self, mut details: tokens::DeclDetails) -> Analysis {
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
pub(super) fn append_synthetic_tokens(tokens: &mut Vec<FileTokens>, synthetic: &[VObjectInfo]) {
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

    pub(super) fn normalize_1based(&self, line: u32, col: u32, name: Option<&str>) -> (u32, u32) {
        (line, self.lsp_column(line, col, name))
    }

    fn normalize_0based(&self, line: u32, col: u32, name: Option<&str>) -> (u32, u32) {
        let (line1, col1) =
            self.normalize_1based(line.saturating_add(1), col.saturating_add(1), name);
        (line1.saturating_sub(1), col1.saturating_sub(1))
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

pub(super) fn collect_model_source_paths(model: &DesignModel, paths: &mut BTreeSet<String>) {
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

pub(super) fn collect_instance_source_paths(
    instances: &[InstanceModel],
    paths: &mut BTreeSet<String>,
) {
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

pub(super) fn feature_token_names(
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

pub(super) fn normalize_vobject_positions(
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

pub(super) fn normalize_one_based_positions(
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

pub(super) fn normalize_zero_based_positions(
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

pub(super) fn normalize_ref_bindings(
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

pub(super) fn normalize_connection_inputs(
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

pub(super) fn normalize_feature_positions(
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

pub(super) fn declaration_detail_name(detail: &str) -> Option<&str> {
    detail
        .split_whitespace()
        .last()
        .map(|name| name.trim_matches(|character: char| matches!(character, ',' | ';' | ')')))
        .filter(|name| !name.is_empty())
}

pub(super) fn normalize_decl_details(maps: &FeatureSourceMaps, details: &mut tokens::DeclDetails) {
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
pub(super) fn merged_ref_bindings(
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
pub(super) type ActualCandidatePos = (u32, u32);

/// A module body span in inclusive 0-based line coordinates.
///
/// `last0 == None` when the end position is unknown (unset `vpiEndLineNo`);
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

pub(super) fn first_position_at_or_after_line(
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

pub(super) fn first_position_after_line(
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
pub(super) fn select_parent_scope_position_sorted(
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

pub(super) fn outcome_from_pipeline(
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

pub(super) fn db_build_diagnostic(error: &str) -> Diag {
    Diag {
        severity: Severity::Error,
        file: None,
        line: 0,
        col: 0,
        message: format!("UHDM database build failed: {error}"),
    }
}

pub(super) fn token_node_count(tokens: &[FileTokens]) -> usize {
    tokens.iter().map(|file| file.nodes.len()).sum()
}

pub(super) fn token_cardinality(tokens: &[FileTokens]) -> usize {
    if crate::logging::enabled(crate::logging::Level::Debug) {
        token_node_count(tokens)
    } else {
        tokens.len()
    }
}

pub(super) const SURELOG_LOG_ARG_MAX: usize = 128;
pub(super) const SURELOG_LOG_ARGV_MAX: usize = 2_048;
pub(super) const SURELOG_LOG_ERROR_MAX: usize = 256;

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

pub(super) fn bounded_surelog_arg(arg: &str) -> String {
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

pub(super) fn surelog_argv_log_details(argv: &[String]) -> (String, String) {
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
pub(super) fn log_surelog_invocation(
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

pub(super) fn log_surelog_invocation_rejected(
    kind: &str,
    error: &impl std::fmt::Display,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) {
    if !crate::logging::enabled(crate::logging::Level::Debug) {
        return;
    }
    let root = bounded_log_text(root, SURELOG_LOG_ARG_MAX);
    let error = error.to_string();
    crate::llg_debug!(
        "event=surelog.invoke kind={} root={} generation={} parent_id={:?} outcome=rejected argv_count=0 argv_repr=[] argv_fingerprint=none error={}",
        kind,
        root,
        generation,
        parent_id,
        bounded_log_text(&error, SURELOG_LOG_ERROR_MAX),
    );
}

/// Serialises [`analyze`] calls: Surelog's global C++ singletons are not
/// thread-safe, and the stdout redirect below must not nest.
pub(super) static ANALYZE_LOCK: Mutex<()> = Mutex::new(());

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
    let scratch = analysis_scratch_dir();
    let _cwd = match ScratchCwd::enter(&scratch) {
        Ok(cwd) => cwd,
        Err(error) => {
            let message = format!(
                "cannot establish the Surelog analysis scratch directory {}: {error}",
                scratch.display()
            );
            analysis_span.complete("error", 1);
            return Analysis::fatal_preflight(message);
        }
    };
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
/// Entering the directory is fail-closed: callers must not invoke Surelog
/// unless this guard was constructed successfully, because Surelog writes
/// preprocessing artifacts into the process CWD.
pub(super) struct ScratchCwd {
    previous: Option<PathBuf>,
}

impl ScratchCwd {
    pub(super) fn enter(scratch: &Path) -> std::io::Result<Self> {
        let previous = std::env::current_dir()?;
        std::fs::create_dir_all(scratch)?;
        std::env::set_current_dir(scratch)?;
        Ok(Self {
            previous: Some(previous),
        })
    }
}

impl Drop for ScratchCwd {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            if let Err(error) = std::env::set_current_dir(&previous) {
                crate::llg_error!(
                    "event=surelog.restore_cwd outcome=error path={} error={}",
                    previous.display(),
                    error
                );
            }
        }
    }
}

pub(super) fn analyze_inner(
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
            let msg = msg.to_string();
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
    let has_uhdm = uhdm.is_some();
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
                            db.tops().len(),
                            db.flat_modules().len(),
                            db.packages().len(),
                            db.classes().len(),
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
                            Some(error.to_string()),
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
    let outcome = outcome_from_pipeline(&out, has_uhdm, has_design, db_built);
    // Every borrowed Design/VPI value has now been converted to owned Rust
    // data; their last uses are above, so the native session borrow ends here.

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
        has_uhdm,
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
