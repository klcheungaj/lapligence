//! Pure projections from committed analyses to LSP response payloads.

use super::*;

/// Failure to produce semantic tokens for an isolated open document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDocumentTokensError {
    message: String,
}

impl OpenDocumentTokensError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for OpenDocumentTokensError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OpenDocumentTokensError {}

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
            message: llg::core::diagnostics::user_message(d),
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

pub(super) fn empty_semantic_tokens() -> SemanticTokens {
    SemanticTokens {
        result_id: None,
        data: Vec::new(),
    }
}

pub(super) fn semantic_file_matches(left: &str, right: &str) -> bool {
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
) -> Result<SemanticTokens, OpenDocumentTokensError> {
    semantic_tokens_for_open_document_with_parent(file, defines, None, None)
}

/// Parent-aware variant used by an LSP semantic-token request.  Direct
/// callers retain the wrapper above and intentionally produce a no-parent
/// parse trace.
pub(crate) fn semantic_tokens_for_open_document_with_parent(
    file: &str,
    defines: &[String],
    source: Option<&str>,
    parent_id: Option<u64>,
) -> Result<SemanticTokens, OpenDocumentTokensError> {
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
    let scratch = analysis_scratch_dir();
    let _cwd = ScratchCwd::enter(&scratch).map_err(|error| {
        OpenDocumentTokensError::new(format!(
            "cannot establish the Surelog parse-only scratch directory {}: {error}",
            scratch.display()
        ))
    })?;
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
    let mut parsed = match compile::parse_only(file, defines) {
        Ok(parsed) => parsed,
        Err(error) => {
            let error = error.to_string();
            crate::llg_debug!(
                "event=surelog.parse_only.return outcome=error file={} parent_id={:?} elapsed_us={} error={}",
                file,
                parent_id,
                parse_started.elapsed().as_micros(),
                bounded_log_text(&error, SURELOG_LOG_ERROR_MAX)
            );
            parse_span.outcome("error");
            return Err(OpenDocumentTokensError::new(error));
        }
    };
    if let Some(source) = source.filter(|source| !source.is_ascii()) {
        let maps = FeatureSourceMaps::from_source(file, source);
        for file_tokens in &mut parsed.tokens {
            for node in &mut file_tokens.nodes {
                if matches!(
                    node.vpi_type,
                    tokens::TOKEN_GENVAR_DECL | tokens::TOKEN_GENVAR_REF
                ) || node.vpi_type == llg::core::vobject_types::VObjectTypeShifted::paGENVAR
                {
                    normalize_vobject_positions(&maps, Some(file), std::slice::from_mut(node));
                }
            }
        }
    }
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
pub(super) fn param_elab_value<'a>(
    a: &'a Analysis,
    file: &str,
    line0: u32,
    name: &str,
) -> Option<&'a Val> {
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
pub(super) fn inst_param_values<'m>(
    tops: &'m [InstanceModel],
    def_name: &str,
    name: &str,
) -> Vec<&'m Val> {
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
pub(super) fn unique_value(values: Vec<&Val>) -> Option<&Val> {
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
pub(super) fn with_elab_value(detail: String, value: &Val) -> String {
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
pub(super) fn macro_hover_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Hover> {
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
pub(super) fn build_macro_hover(
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
pub(super) fn hover_for_target(
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

pub(super) fn hover_fallback(a: &Analysis, file: &str, line: u32, col: u32) -> Option<Hover> {
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

pub(super) fn hover_detail(a: &Analysis, file: &str, name: &str) -> Option<String> {
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

pub(super) fn format_port(p: &PortModel) -> String {
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

pub(super) fn format_signal(s: &SignalModel) -> String {
    if s.kind == "array" {
        format!("array {} {}", s.ty.render(), s.name)
    } else {
        format!("{} {}", s.ty.render(), s.name)
    }
}

pub(super) fn format_param(p: &ParamModel) -> String {
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
pub(super) fn format_enum_const(ec: &EnumConstDef) -> String {
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
pub(super) fn format_class_field(f: &ClassFieldDef) -> String {
    format!("{} {}", f.ty.render(), clean_name(&f.name))
}

/// Hover text for a class definition: the declaration plus its methods and
/// fields, e.g. `class Counter` followed by `function int get()` and
/// `int count` entries.
pub(super) fn format_class(c: &ClassDef) -> String {
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
pub(super) fn format_func_arg(a: &FuncArgDef) -> String {
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
pub(super) fn func_signature(f: &FuncDef) -> String {
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
pub(super) fn func_from_decl<'m>(model: &'m DesignModel, decl: &SymEntry) -> Option<&'m FuncDef> {
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

pub(super) fn def_site_line(a: &Analysis, def_name: &str) -> Option<(String, u32)> {
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
pub(super) fn ref_target_location(t: &DeclTarget) -> Location {
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

pub(super) fn definition_fallback(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
) -> Option<Location> {
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
            return locations;
        }
    }
    references_fallback_with_options(a, file, line, col, include_declaration)
}

/// The binding target for the query position: exact key first, then the
/// containing identifier's start column (cursor normalization, matching
/// [`definition_at`]).
pub(super) fn binding_target_at<'a>(
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
pub(super) fn shadow_aware_reference_locations(
    a: &Analysis,
    e: &SymEntry,
    heads: &HashSet<(String, u32, u32)>,
    include_declaration: bool,
) -> Vec<Location> {
    let mut out: Vec<SymEntry> = Vec::new();
    let mut seen: HashSet<(String, u32, u32)> = HashSet::new();

    if include_declaration {
        for (file, line, col) in heads {
            if let Some(declaration) = a
                .index
                .entry_at(file, *line, *col)
                .filter(|entry| entry.is_decl)
            {
                seen.insert((file.clone(), *line, *col));
                out.push(declaration.clone());
            }
        }
    }

    // True declaration sites per the UHDM capture: positions recorded as
    // declared objects behave like declarations even when the multi-view
    // classification left them REF-shaped.
    let is_decl_position = |key: &(String, u32, u32)| a.decl_details.contains_key(key);

    let targets_head =
        |target: &DeclTarget| heads.contains(&(target.file.clone(), target.line0, target.col0));
    let genvar_head = heads.iter().any(|(file, line, col)| {
        a.ref_bindings
            .get(&(file.clone(), *line, *col))
            .is_some_and(|target| target.kind == "genvar")
    });

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
            None if genvar_head => false,
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
        if heads.contains(&key) {
            continue;
        }
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

pub(super) fn references_fallback_with_options(
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
pub(super) fn symbol_info(d: &SymEntry) -> SymbolInformation {
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
pub(super) fn sym_kind_to_lsp(kind: SymKind) -> SymbolKind {
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
pub(super) fn func_symbol(f: &FuncDef) -> DocumentSymbol {
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
pub(super) fn module_symbol(a: &Analysis, file: &str, m: &ModuleDef) -> DocumentSymbol {
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
pub(super) fn package_symbol(p: &PackageDef) -> DocumentSymbol {
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
pub(super) fn class_symbol(a: &Analysis, file: &str, c: &ClassDef) -> DocumentSymbol {
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
pub(super) fn class_children(
    a: &Analysis,
    file: &str,
    c: &ClassDef,
) -> Option<Vec<DocumentSymbol>> {
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
pub(super) fn class_children_fallback(c: &ClassDef, file: &str) -> Option<Vec<DocumentSymbol>> {
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
pub(super) fn module_children(
    a: &Analysis,
    file: &str,
    def_name: &str,
) -> Option<Vec<DocumentSymbol>> {
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
pub(super) fn is_sv_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Type words that mean "nothing usable was captured" — rendering them as a
/// detail would be a wrong guess.
pub(super) const UNKNOWN_TYPE_WORDS: &[&str] = &["var", "other", "unknown"];

/// Strips the trailing identifier (the declared NAME) from a declaration
/// snippet (`input logic [7:0] q` → `input logic [7:0]`), leaving the
/// source-declared type text.  `None` when nothing safe can be said:
/// single-token snippets, non-identifier tails, or degenerate type words all
/// degrade instead of guessing.
pub(super) fn strip_trailing_name(snippet: &str) -> Option<String> {
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
pub(super) fn param_colon_type(detail: &str) -> Option<String> {
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
pub(super) fn param_type_detail(
    model_detail: Option<&str>,
    decl_snippet: Option<&str>,
) -> Option<String> {
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
pub(super) fn child_type_detail(a: &Analysis, d: &SymEntry) -> Option<String> {
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
pub(super) fn module_instance_children(
    a: &Analysis,
    file: &str,
    def_scope: &str,
) -> Vec<DocumentSymbol> {
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
pub(super) fn instance_child_symbol(d: &SymEntry, type_text: Option<&str>) -> DocumentSymbol {
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
pub(super) fn child_symbol_from_entry(d: &SymEntry) -> DocumentSymbol {
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
pub(super) fn module_children_fallback(
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
pub(super) fn port_type_only(p: &PortModel) -> Option<String> {
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
pub(super) fn signal_type_only(s: &SignalModel) -> Option<String> {
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
pub(super) fn param_type_only(p: &ParamModel) -> Option<String> {
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
pub(super) fn child_symbol(
    a: &Analysis,
    file: &str,
    name: &str,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
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
pub(super) fn completion_kind_for(kind: SymKind) -> CompletionItemKind {
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

pub(super) const KEYWORDS: &[&str] = &[
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
pub(super) fn prefix_before_cursor(line: &str, col: u32) -> String {
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
pub(super) fn package_scope_prefix(line: &str, col: u32) -> Option<(String, String)> {
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
pub(super) fn lsp_name_len(name: &str) -> u32 {
    name.encode_utf16().count() as u32
}

/// Convert a 0-based LSP UTF-16 column into a UTF-8 byte offset at a char
/// boundary.  A position in the middle of a supplementary character is
/// rounded to that character's start, which is the only safe slice boundary.
pub(super) fn utf16_byte_offset(line: &str, col: u32) -> usize {
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
pub(super) fn clean_name(name: &str) -> &str {
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
pub(super) fn builtin_file(file: &str) -> bool {
    Path::new(file).file_name().and_then(|n| n.to_str()) == Some("builtin.sv")
}

/// The token list for `file`, by exact path with a filename fallback.
pub(super) fn file_tokens<'a>(a: &'a Analysis, file: &str) -> Option<&'a FileTokens> {
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
pub(super) fn token_at<'a>(
    a: &'a Analysis,
    file: &str,
    line: u32,
    col: u32,
) -> Option<&'a VObjectInfo> {
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
pub(super) fn all_instances(insts: &[InstanceModel]) -> Vec<&InstanceModel> {
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
pub(super) fn skip_self_label(a: &Analysis, file: &str, line: u32, col: u32) -> Option<(u32, u32)> {
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
pub(super) fn nearest_declaration(
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
pub(super) fn is_declaration_vpi_type(t: i32) -> bool {
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
            | llg::core::tokens::TOKEN_GENVAR_DECL
    )
}

// ── Location construction ─────────────────────────────────────────────────────

pub(super) fn location(file: &str, line1: u32, col1: u32, len: usize) -> Location {
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
pub(super) fn entry_location(e: &SymEntry) -> Location {
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

pub(super) fn module_def_location(a: &Analysis, m: &ModuleDef) -> Option<Location> {
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

pub(super) fn package_location(p: &PackageDef) -> Option<Location> {
    let file = p.file.as_deref()?;
    let len = lsp_name_len(clean_name(&p.name)) as usize;
    Some(location(file, p.line, p.col, len))
}

pub(super) fn instance_location(i: &InstanceModel) -> Option<Location> {
    let file = i.file.as_deref()?;
    Some(location(
        file,
        i.line,
        i.col,
        lsp_name_len(&i.name) as usize,
    ))
}
