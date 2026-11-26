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
    /// What every token of [`Analysis::tokens`] names, captured from Slang's
    /// lexical semantic ids. Definition, hover, references and rename read
    /// these, as highlighting reads the tokens; nothing is resolved by name.
    pub bindings: tokens::Bindings,
    /// Rendered declaration snippets (`logic [3:0] val`, …) keyed by
    /// `(file, line1, col1)` of the declaration token, captured from semantic DB
    /// during the same walk as [`Analysis::bindings`].  Position-accurate
    /// even where several same-named declarations live in one module (inner-
    /// scope shadowing), so hover text describes exactly the object at that
    /// position instead of the first name match in the model.  Empty for
    /// parse-fallback analyses (the model supplies details there).
    pub decl_details: tokens::DeclDetails,
    /// Where the declarations that the outline lists end
    /// ([`tokens::DeclExtents`]). Empty for analyses that capture none.
    pub(crate) decl_extents: tokens::DeclExtents,
    /// Lint findings from the shared linter (`source: "llg-lint"` when
    /// published), produced alongside the Slang diagnostics.  1-based
    /// positions.  Empty when compilation failed or no semantic DB design was built.
    pub lint: Vec<LintDiag>,
    /// Design-wide elaboration errors the `llg` driver stops on
    /// ([`llg::core::design_checks`]), published as `source: "llg"` with the
    /// catalog id as code. They leave the outcome alone: the design elaborated
    /// and its navigation data is complete. Empty when no semantic DB was built.
    pub design: Vec<DesignDiag>,
    /// Source-level module definitions and instance edges retained while the
    /// Slang parse tree was alive.  The module explorer uses this graph for
    /// definitions that configured elaboration did not instantiate.
    pub(crate) module_graph: ModuleGraph,
    /// The effective `[compile] top` used for this analysis, retained so the
    /// explorer can apply the configured-top root union without rereading
    /// configuration during a request.
    pub(crate) configured_top: Option<String>,
    /// The macros of the compile, with the resolution of every written macro
    /// name. Empty for hand-built analyses.
    pub(super) macros: macros::MacroTable,
    /// Fingerprints of the texts the frontend read, by file name.
    sources: HashMap<String, SourceFingerprint>,
    /// Include search prefixes, include-only buffers and package- and
    /// definition-declaring units of the admitted project compile, shared
    /// with the isolated open-buffer token parse so an open buffer sees the
    /// headers (and their macros), packages and definitions its project
    /// compile saw. Empty for hand-built and preflight-failed analyses.
    pub(crate) include_context: Arc<IncludeContext>,
}

/// What identifies one source text: its length and a digest of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFingerprint {
    length: usize,
    digest: u64,
}

impl SourceFingerprint {
    pub fn of(text: &str) -> Self {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(text.as_bytes());
        Self {
            length: text.len(),
            digest: hasher.finish(),
        }
    }
}

/// The context one admitted project compile gives an isolated open-buffer
/// parse: its logical include search prefixes, in order, the admitted
/// include-only buffers by name and the compilation units that declare
/// packages or definitions ([`PackageContext`]). Texts are the admission snapshots
/// themselves (shared, not copied), so retaining them costs no more than the
/// committed analysis already pins.
#[derive(Debug, Default)]
pub struct IncludeContext {
    pub include_dirs: Vec<String>,
    pub includes: Vec<(String, Arc<String>)>,
    pub packages: PackageContext,
}

impl IncludeContext {
    /// Owned include buffers for one isolated compile request.
    pub fn sources(&self) -> Vec<compile::OwnedSource> {
        self.includes
            .iter()
            .map(|(name, text)| compile::OwnedSource::include(name.clone(), text.as_str()))
            .collect()
    }

    /// Whether an isolated parse of any buffer gets the same inputs from
    /// `other` as from `self`: the same include search prefixes, include
    /// buffers and package units, by name and text. Each commit snapshots
    /// texts anew, so equal texts in different allocations compare equal.
    pub fn same_inputs(&self, other: &IncludeContext) -> bool {
        self.include_dirs == other.include_dirs
            && self.includes.len() == other.includes.len()
            && self.includes.iter().zip(&other.includes).all(
                |((name, text), (other_name, other_text))| {
                    name == other_name && (Arc::ptr_eq(text, other_text) || text == other_text)
                },
            )
            && self.packages.same_units(&other.packages)
    }
}

impl Analysis {
    /// Assemble an [`Analysis`] from its parts, computing the symbol index.
    ///
    /// Public so tests can build an [`Analysis`] without running Slang; the
    /// production pipeline uses the same constructor inside [`analyze`].
    ///
    /// Passes empty bindings: hand-built analyses have no elaborated design
    /// behind them.
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
            tokens::Bindings::default(),
            None,
        )
    }

    /// Assemble an [`Analysis`] when the caller has observed the complete
    /// pipeline state and can therefore provide a more precise outcome than
    /// diagnostics alone allow.
    ///
    /// `bindings` are the identities captured from Slang's lexical table, and
    /// `declarations` the positions of its declaration tokens.
    pub fn new_with_outcome(
        outcome: AnalysisOutcome,
        diagnostics: Vec<Diag>,
        model: DesignModel,
        tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
        bindings: tokens::Bindings,
        declarations: Option<&ParseDeclPositions>,
    ) -> Analysis {
        Self::new_with_outcome_and_sources(
            outcome,
            diagnostics,
            model,
            tokens,
            lint,
            bindings,
            declarations,
            &FeatureSourceMaps::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_outcome_and_sources(
        outcome: AnalysisOutcome,
        diagnostics: Vec<Diag>,
        mut model: DesignModel,
        tokens: Vec<FileTokens>,
        lint: Vec<LintDiag>,
        bindings: tokens::Bindings,
        declarations: Option<&ParseDeclPositions>,
        source_maps: &FeatureSourceMaps,
    ) -> Analysis {
        normalize_model_positions(source_maps, &mut model);
        let index = SymbolIndex::from_parts(&model, &tokens, declarations);
        Analysis {
            outcome,
            diagnostics,
            frontend_diagnostics: Vec::new(),
            model,
            tokens,
            index,
            bindings,
            decl_details: HashMap::new(),
            decl_extents: HashMap::new(),
            lint,
            design: Vec::new(),
            module_graph: ModuleGraph::default(),
            configured_top: None,
            macros: macros::MacroTable::default(),
            sources: HashMap::new(),
            include_context: Arc::default(),
        }
    }

    /// Whether the tokens and bindings of `file` were captured from the text
    /// with this fingerprint, so they answer for a buffer that holds it.
    pub(crate) fn compiled_from(&self, file: &str, text: SourceFingerprint) -> bool {
        file_tokens(self, file)
            .and_then(|tokens| self.sources.get(&tokens.path))
            .is_some_and(|compiled| *compiled == text)
    }

    /// The length of the text that the frontend read as `file`, named exactly.
    pub(crate) fn compiled_length(&self, file: &Path) -> Option<usize> {
        Some(self.sources.get(file.to_str()?)?.length)
    }

    /// Attach the include context of the compile that produced this analysis.
    pub(crate) fn set_include_context(&mut self, context: IncludeContext) {
        self.include_context = Arc::new(context);
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

    /// Attach the declaration ends captured during the same Slang pass.
    fn with_decl_extents(mut self, extents: tokens::DeclExtents) -> Analysis {
        self.decl_extents = extents;
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
                logical: None,
                file: None,
                line: 0,
                col: 0,
                message: message.into(),
                id: None,
                option: None,
            }],
            empty_design(),
            Vec::new(),
            Vec::new(),
            tokens::Bindings::default(),
            None,
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
        logical: None,
        file: None,
        line: 0,
        col: 0,
        message: format!("semantic database build failed: {error}"),
        id: None,
        option: None,
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

/// Pipeline threads (`llg::stack`) the server runs at once: one project
/// analysis, which holds [`COMPILE_LOCK`], beside one isolated parse of an
/// open document.
pub(crate) const MAX_PIPELINES: usize = 2;

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
    analyze_with_config_context_parent(opts, lint_cfg, "-", 0, None)
        .unwrap_or_else(|cancelled| Analysis::fatal_preflight(cancelled.to_string()))
}

/// Analyze with root/generation metadata used only for lifecycle logging, and
/// an optional parent lifecycle ID supplied by the LSP job scheduler. Fails
/// once `opts.cancel` is cancelled.
pub(crate) fn analyze_with_config_context_parent(
    opts: &CompileOpts,
    lint_cfg: &LintConfig,
    root: &str,
    generation: u64,
    parent_id: Option<u64>,
) -> Result<Analysis, Cancelled> {
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
    let parent_id = Some(analysis_span.id());
    // Another root's analysis may have held the lock for long.
    let analysis = opts
        .cancel
        .check()
        .and_then(|()| {
            llg::stack::run_pipeline(|| {
                analyze_inner_slang(opts, lint_cfg, root, generation, parent_id)
            })
            .unwrap_or_else(|error| {
                crate::llg_error!("event=pipeline.start outcome=error error={error}");
                Ok(Analysis::fatal_preflight(error.to_string()))
            })
        })
        .inspect_err(|_| analysis_span.outcome("cancelled"))?;
    analysis_span.complete(
        match analysis.outcome {
            AnalysisOutcome::Valid => "ok",
            AnalysisOutcome::Fatal | AnalysisOutcome::Parse | AnalysisOutcome::Compile => "error",
        },
        analysis.diagnostics.len() + analysis.lint.len() + analysis.design.len(),
    );
    Ok(analysis)
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
) -> Result<Analysis, Cancelled> {
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
            let cancelled = error.kind() == compile::StartupErrorKind::Cancelled;
            crate::llg_debug!(
                "event=slang.compile.end outcome={} root={} generation={} elapsed_us={} error={}",
                if cancelled { "cancelled" } else { "error" },
                root,
                generation,
                started.elapsed().as_micros(),
                bounded_log_text(&error.to_string(), 512)
            );
            if cancelled {
                return Err(Cancelled);
            }
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
            return Ok(Analysis::fatal_preflight(message));
        }
    };
    crate::llg_debug!("event=slang.compile.end outcome={} root={} generation={} diagnostics={} semantic_nodes={} lexical_tokens={} elapsed_us={}", if out.ok() { "ok" } else { "error" }, root, generation, out.diagnostics.len(), out.snapshot.semantic_nodes.len(), out.snapshot.lexical_tokens.len(), started.elapsed().as_micros());
    let mut out = out;
    opts.cancel.check()?;
    let database = (!opts.library_units).then(|| {
        let _stage = Stage::new("db.import");
        llg::core::db::Db::from_slang_unless_cancelled(&out.snapshot, &opts.cancel)
    });
    opts.cancel.check()?;
    let protected_stage = Stage::new("ls.protected");
    let protected = ProtectedSource::new(&out.snapshot);
    let protected_view = (!protected.is_empty()).then(|| {
        let texts: HashMap<_, _> = out
            .snapshot
            .files
            .iter()
            .map(|file| (file.name.as_str(), file.text.as_str()))
            .collect();
        (
            protected.dispositions(),
            protected.hints(&texts),
            protected_lint_extents(&protected, &out.snapshot),
        )
    });
    drop(protected);
    let (mut dispositions, protected_hints, protected_extents) = protected_view.unwrap_or_default();
    // The frontend's unused-parameter warning for a parameter an FSM pragma
    // names as a state is wrong: the pragma uses it (IEEE 1800-2009 40.4.1).
    let fsm_states = database
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|db| db.fsm_state_parameter_warnings(&out.snapshot))
        .unwrap_or_default();
    if !fsm_states.is_empty() {
        if dispositions.is_empty() {
            dispositions = vec![Disposition::Keep; out.snapshot.diagnostics.len()];
        }
        for index in fsm_states {
            if let Some(disposition) = dispositions.get_mut(index) {
                *disposition = Disposition::Drop;
            }
        }
    }
    let mut hinted_errors = false;
    if !dispositions.is_empty() {
        out.retain_frontend_diagnostics(|index, _, projected| match dispositions[index] {
            Disposition::Keep => true,
            Disposition::Drop => false,
            Disposition::Hint(reason) => {
                hinted_errors |= matches!(
                    projected.severity,
                    Severity::Fatal | Severity::Syntax | Severity::Error
                );
                projected.severity = Severity::Info;
                projected.message = hint_message(&projected.message, reason);
                true
            }
        });
    }

    drop(protected_stage);
    let source_texts: Vec<_> = out
        .snapshot
        .files
        .iter()
        .map(|source| (source.name.as_str(), source.text.as_str()))
        .collect();
    let projection_stage = Stage::new("ls.tokens");
    let projection =
        tokens::project_slang_unless_cancelled(&out.snapshot, None, &opts.defines, &opts.cancel)?;
    drop(projection_stage);
    crate::llg_debug!(
        "event=slang.tokens.end root={} generation={} tokens={} elapsed_us={}",
        root,
        generation,
        out.snapshot.lexical_tokens.len(),
        started.elapsed().as_micros()
    );
    let diagnostics_stage = Stage::new("ls.diagnostics");
    let mut frontend_diagnostics = project_snapshot_diagnostics(
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
        &dispositions,
    );
    frontend_diagnostics.extend(protected_hints);
    // Errors that protected source explains no longer fail the frontend.
    let frontend_ok = out.ok()
        || (hinted_errors
            && !out.diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.severity,
                    Severity::Fatal | Severity::Syntax | Severity::Error
                )
            }));
    drop(diagnostics_stage);
    let graph_stage = Stage::new("ls.module_graph");
    let module_graph = module_graph_from_slang(
        &out.snapshot,
        &source_texts,
        database.as_ref().and_then(|result| result.as_ref().ok()),
        &opts.cancel,
    )?;
    drop(graph_stage);
    let (model, lint_diags, design_diags, db_error) = match database {
        Some(Ok(db)) => {
            // `compile.timescale` is the server's only default time scale.
            let design_diags: Vec<_> = design_checks::mixed_time_scale(&db, Reporter::Server)
                .into_iter()
                .collect();
            let model_stage = Stage::new("ls.model");
            let model = DesignModel::from_db(&db);
            drop(model_stage);
            let _lint_stage = Stage::new("lint");
            let lint_ctx = lint::LintCtx::new(&db, &model).cancelled_by(opts.cancel.clone());
            let mut lint_diags = LintRegistry::default_rules().lint(&lint_ctx, lint_cfg);
            opts.cancel.check()?;
            if !protected_extents.is_empty() {
                lint_diags.retain(|finding| !inside_extents(&protected_extents, finding));
            }
            (model, lint_diags, design_diags, None)
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
    let _index_stage = Stage::new("ls.index");
    opts.cancel.check()?;
    let source_maps = FeatureSourceMaps::from_sources(&source_texts);
    let mut analysis = Analysis::new_with_outcome_and_sources(
        outcome,
        diagnostics,
        model,
        projection.tokens,
        lint_diags,
        projection.bindings,
        Some(&projection.declarations),
        &source_maps,
    )
    .with_macros(projection.macros)
    .with_decl_details(projection.details)
    .with_decl_extents(projection.extents)
    .with_module_graph(module_graph)
    .with_configured_top(opts.top.clone());
    analysis.attach_frontend_diagnostics(frontend_diagnostics);
    analysis.design = design_diags;
    analysis.sources = out
        .snapshot
        .files
        .iter()
        .map(|file| (file.name.clone(), SourceFingerprint::of(&file.text)))
        .collect();
    Ok(analysis)
}

/// File name and 1-based inclusive line/column bounds of one protected
/// declaration.
type LintExtent = (String, (u32, u32), (u32, u32));

/// Line/column extents of the protected declarations of `snapshot`.
fn protected_lint_extents(
    protected: &ProtectedSource<'_>,
    snapshot: &llg::ffi::slang::Snapshot,
) -> Vec<LintExtent> {
    protected
        .protected_extents()
        .filter_map(|(file_id, start, end)| {
            let file = snapshot.files.iter().find(|file| file.id == file_id)?;
            let positions = tokens::SourcePositions::new(&file.text);
            Some((
                file.name.clone(),
                positions.position(start),
                positions.position(end),
            ))
        })
        .collect()
}

/// Whether `finding` lies in one of the protected `extents`: the hidden text
/// may use or drive what the declaration's plaintext declares.
fn inside_extents(extents: &[LintExtent], finding: &LintDiag) -> bool {
    let Some(file) = finding.file.as_deref() else {
        return false;
    };
    let position = (finding.line, finding.col);
    extents
        .iter()
        .any(|(name, start, end)| name == file && *start <= position && position <= *end)
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
) -> Result<Analysis, Cancelled> {
    let recovery_opts = CompileOpts {
        sources: opts.sources.clone(),
        top: None,
        defines: opts.defines.clone(),
        include_dirs: opts.include_dirs.clone(),
        library_units: true,
        editor: opts.editor,
        compilation_unit_mode: opts.compilation_unit_mode,
        limits: opts.limits,
        cancel: opts.cancel.clone(),
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
    let mut analysis = analyze_inner_slang(&recovery_opts, lint_cfg, root, generation, parent_id)?
        .with_configured_top(opts.top.clone());
    if !analysis.has_feature_data() {
        crate::llg_debug!(
            "event=slang.limit_recovery.end outcome=error root={} generation={}",
            root,
            generation
        );
        return Ok(Analysis::fatal_preflight(format!(
            "{failure}. {}",
            crate::config::FRONTEND_LIMIT_GUIDANCE
        )));
    }
    analysis.outcome = AnalysisOutcome::Compile;
    analysis.diagnostics.push(Diag {
        severity: Severity::Warning,
        logical: None,
        file: None,
        line: 0,
        col: 0,
        message: format!(
            "full workspace elaboration exceeded a frontend limit; serving bounded declaration-level navigation ({failure}). {}",
            crate::config::FRONTEND_LIMIT_GUIDANCE
        ),
        id: None,
        option: None,
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
    Ok(analysis)
}

/// Return an empty compile-outcome snapshot for backend failure recovery.
#[cfg(test)]
pub fn empty_analysis() -> Analysis {
    Analysis::new_with_outcome(
        AnalysisOutcome::Compile,
        Vec::new(),
        empty_design(),
        Vec::new(),
        Vec::new(),
        tokens::Bindings::default(),
        None,
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
/// its root>`, with a Windows drive or share encoded as leading directories
/// (see `ffi::platform::mirror_relative`).  `real` must be absolute (the LSP
/// only feeds `Url::to_file_path` results).  The mapping is a bijection over
/// any absolute path — workspace or external — so [`real_path`] always
/// recovers the original path, and a shadow never aliases its real file.
pub fn shadow_path(real: &Path, base: &Path) -> PathBuf {
    base.join(llg::ffi::platform::mirror_relative(real))
}

/// Reverse of [`shadow_path`]: recover the real absolute path from a shadow
/// path.  Returns `None` when `shadow` is not under `base`.
pub fn real_path(shadow: &Path, base: &Path) -> Option<PathBuf> {
    let relative = shadow.strip_prefix(base).ok()?;
    llg::ffi::platform::mirror_absolute(relative)
}

static STAGING_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Select the directory that holds the per-process shadow base
/// (`llg_ls --staging-dir`). Call before the first [`process_shadow_base`];
/// returns `false` when a root was already chosen.
pub fn set_staging_root(root: std::path::PathBuf) -> bool {
    STAGING_ROOT.set(root).is_ok()
}

/// `--staging-dir` when given, else the OS temp dir (`$TMPDIR`, `/tmp`, ...).
fn staging_root() -> std::path::PathBuf {
    STAGING_ROOT
        .get()
        .cloned()
        .unwrap_or_else(std::env::temp_dir)
}

/// The process-global private shadow base: `<staging root>/llg-<pid>-<rand>`,
/// where the staging root is `--staging-dir` or the OS temp dir.
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
        let root = staging_root();
        let mut last = root.join(format!("llg-{pid}"));
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
            last = root.join(format!("llg-{pid}-{rand}"));
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
