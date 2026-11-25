//! Pure projections from committed analyses to LSP response payloads.

use super::*;
use crate::position_map::is_word;
use llg::core::report::Severity as ReportSeverity;
use tower_lsp::lsp_types::DiagnosticRelatedInformation;

/// Failure to produce semantic tokens for an isolated open document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDocumentTokensError {
    message: String,
    /// The parse was stopped: no statement about the buffer.
    pub(crate) cancelled: bool,
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
/// frontend diagnostics to a caller-provided source path.
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
    let rich_frontend: HashSet<_> = a
        .frontend_diagnostics
        .iter()
        .map(|(path, diagnostic)| (path.as_str(), diagnostic.message.as_str()))
        .collect();
    for d in &a.diagnostics {
        let Some(path) = d.file.as_deref().or(fallback_file.as_deref()) else {
            continue;
        };
        if rich_frontend.contains(&(path, d.message.as_str())) {
            continue;
        }
        let severity = match d.severity {
            Severity::Fatal | Severity::Syntax | Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Note => DiagnosticSeverity::INFORMATION,
            // Lint `Info` publishes as INFORMATION too: one word, one level.
            Severity::Info => DiagnosticSeverity::INFORMATION,
        };
        // Core positions are 1-based; 0 means unknown → report at (0,0).
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
            source: Some("slang".to_owned()),
            message: llg::core::diagnostics::user_message(d),
            related_information: d
                .logical
                .as_ref()
                .and_then(|logical| {
                    super::slang::mapped_origin(path, range, &logical.file, logical.line)
                })
                .map(|origin| vec![origin]),
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
            related_information: d
                .logical
                .as_ref()
                .and_then(|logical| {
                    super::slang::mapped_origin(path, range, &logical.file, logical.line)
                })
                .map(|origin| vec![origin]),
            tags: None,
            data: None,
        });
    }
    for d in &a.design {
        let Some(path) = d.location.file.as_deref() else {
            continue;
        };
        let related_information: Vec<_> = d
            .related
            .iter()
            .filter_map(|related| {
                Some(DiagnosticRelatedInformation {
                    location: Location::new(
                        Url::from_file_path(related.location.file.as_deref()?).ok()?,
                        point_range(related.location.line, related.location.col),
                    ),
                    message: related.message.clone(),
                })
            })
            .collect();
        out.entry(path.to_owned()).or_default().push(LspDiagnostic {
            range: point_range(d.location.line, d.location.col),
            severity: Some(match d.def.severity {
                ReportSeverity::Fatal | ReportSeverity::Error => DiagnosticSeverity::ERROR,
                ReportSeverity::Warning => DiagnosticSeverity::WARNING,
                ReportSeverity::Info => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(NumberOrString::String(d.def.id())),
            code_description: None,
            source: Some("llg".to_owned()),
            message: d.text.clone(),
            related_information: (!related_information.is_empty()).then_some(related_information),
            tags: None,
            data: None,
        });
    }
    for (path, diagnostic) in &a.frontend_diagnostics {
        out.entry(path.clone())
            .or_default()
            .push(diagnostic.clone());
    }
    out
}

/// The empty LSP range at a 1-based core position; 0 (unknown) maps to the
/// start of the file.
fn point_range(line: u32, col: u32) -> Range {
    let (line, col) = if line == 0 {
        (0, 0)
    } else {
        (line - 1, col.saturating_sub(1))
    };
    Range::new(Position::new(line, col), Position::new(line, col))
}

// ── Semantic tokens ───────────────────────────────────────────────────────────

/// Encode the semantic tokens for `file` from the cached token lists.
///
/// Source identity is exact. A same-named file elsewhere in the workspace
/// must neither provide tokens nor suppress this file's tokens. A file with
/// syntax errors keeps the tokens of everything the frontend recovered:
/// missing and skipped tokens are never captured, so an error degrades the
/// stream per token, not per file.
pub fn semantic_tokens_for(a: &Analysis, file: &str) -> SemanticTokens {
    match file_tokens(a, file) {
        Some(ft) => semantic_tokens::encode(&ft.nodes),
        None => empty_semantic_tokens(),
    }
}

/// The tokens of [`semantic_tokens_for`] at their `current` places in an edited
/// text, without those that have none.
pub(crate) fn placed_semantic_tokens_for(
    a: &Analysis,
    file: &str,
    current: impl Fn(Range) -> Option<Range>,
) -> SemanticTokens {
    let Some(tokens) = file_tokens(a, file) else {
        return empty_semantic_tokens();
    };
    semantic_tokens::encode_placed(&tokens.nodes, |token| {
        Some(current(token_range(token))?.start)
    })
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
    match (
        llg::ffi::platform::canonicalize(left),
        llg::ffi::platform::canonicalize(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Names of one edited document that completion offers besides those of the
/// analysis. The ones declared after them are not offered before a save.
const MAX_DECLARED_NAMES: usize = 4096;

/// A name that an edited document declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeclaredName {
    pub(crate) name: String,
    pub(crate) kind: SymKind,
}

/// The identifiers of one open document from its isolated parse: the tokens
/// highlighting sends and the bindings go-to-definition follows.
#[derive(Debug)]
pub struct DocumentIdentifiers {
    nodes: Vec<TokenInfo>,
    bindings: tokens::Bindings,
}

impl DocumentIdentifiers {
    pub(crate) fn new(nodes: Vec<TokenInfo>, bindings: tokens::Bindings) -> Self {
        Self { nodes, bindings }
    }

    pub fn semantic_tokens(&self) -> SemanticTokens {
        semantic_tokens::encode(&self.nodes)
    }

    pub fn token_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn definitions_at(&self, line: u32, col: u32) -> Vec<NamedLocation> {
        definitions_in(&self.nodes, &self.bindings, line, col)
    }

    /// The names that the document declares, in the order of the text.
    pub(crate) fn declared_names(&self) -> Vec<DeclaredName> {
        let mut seen = HashSet::new();
        let mut names = Vec::new();
        for token in &self.nodes {
            let class = tokens::token_class(token.kind);
            if !class.declaration || class.macro_body || class.undeclared {
                continue;
            }
            let Some((kind, _)) = classify_token(token.kind, true) else {
                continue;
            };
            let Some(name) = token.name.as_deref().map(clean_name) else {
                continue;
            };
            if !name.is_empty() && seen.insert(name) {
                names.push(DeclaredName {
                    name: name.to_owned(),
                    kind,
                });
                if names.len() == MAX_DECLARED_NAMES {
                    break;
                }
            }
        }
        names
    }

    /// The tokens and their bindings.
    #[cfg(test)]
    pub fn parts(&self) -> (&[TokenInfo], &tokens::Bindings) {
        (&self.nodes, &self.bindings)
    }
}

/// Parse one open document in isolation.
///
/// This path deliberately does not build or update an [`Analysis`]. Slang
/// receives the admitted in-memory buffer, preprocessed with the configured
/// defines and the include context of the root's committed project compile,
/// so macros from the buffer itself, its admitted headers and the
/// configuration expand exactly as in the project stream. The committed units
/// that declare the packages and definitions the buffer names join the parse
/// as library units ([`PackageContext::sources_for`]), so imported and
/// scope-qualified package items, instances, interface ports and hierarchical
/// names resolve as in the project stream. Frontend diagnostics do not make
/// this return an error and do not empty the table: the tokens the frontend
/// recovered stay (missing and skipped tokens are never captured), so a syntax
/// error or an unknown macro degrades it per token. `cancel` stops the parse.
pub(crate) fn open_document_identifiers(
    file: &str,
    defines: &[String],
    source: &str,
    includes: &IncludeContext,
    cancel: &CancelToken,
) -> Result<DocumentIdentifiers, OpenDocumentTokensError> {
    let parsed = compile::parse_source_in_context(
        file,
        source,
        defines,
        &includes.include_dirs,
        includes.sources(),
        includes.packages.sources_for(file, source),
        cancel,
    )
    .map_err(|error| {
        let mut message = error.to_string();
        if error.kind() == compile::StartupErrorKind::LimitExceeded {
            crate::llg_error!(
                "event=semantic_tokens.resource_limit file={} error={} advice={}",
                crate::logging::bounded_field(file),
                crate::logging::bounded_field(&message),
                crate::config::TOKEN_PARSE_LIMIT_GUIDANCE
            );
            message = format!("{message}. {}", crate::config::TOKEN_PARSE_LIMIT_GUIDANCE);
        }
        OpenDocumentTokensError {
            message,
            cancelled: error.kind() == compile::StartupErrorKind::Cancelled,
        }
    })?;
    let blocking_diagnostics = parsed
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Fatal | Severity::Syntax))
        .count();
    if blocking_diagnostics != 0 {
        crate::llg_debug!(
            "isolated semantic token parse recovered file={file} syntax_diagnostics={blocking_diagnostics}"
        );
    }
    let nodes = parsed
        .tokens
        .into_iter()
        .find(|tokens| semantic_file_matches(&tokens.path, file))
        .map(|tokens| tokens.nodes)
        .unwrap_or_default();
    Ok(DocumentIdentifiers::new(nodes, parsed.bindings))
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

fn normalize_inline_elab_value(detail: String) -> String {
    match detail.rsplit_once(" = ") {
        Some((declaration, value)) if !declaration.is_empty() && !value.is_empty() => {
            format!("{declaration}\nvalue = {value}")
        }
        _ => detail,
    }
}

/// The hover of a name in the text an analysis compiled.
#[derive(Debug, Clone)]
pub(crate) struct HoverText {
    /// The name.
    range: Range,
    blocks: Vec<HoverBlock>,
}

#[derive(Debug, Clone)]
enum HoverBlock {
    /// SystemVerilog, and where it stands when the hover says so.
    Code {
        detail: String,
        defined_at: Option<DeclTarget>,
    },
    Message(String),
}

impl HoverText {
    fn code(range: Range, detail: String, defined_at: Option<DeclTarget>) -> Self {
        Self {
            range,
            blocks: vec![HoverBlock::Code { detail, defined_at }],
        }
    }

    /// The analysed range of the name.
    pub(crate) fn range(&self) -> Range {
        self.range
    }

    /// The hover over `range` of the current text, in which `current_line`
    /// finds the 0-based line of a definition.
    pub(crate) fn render(
        &self,
        range: Range,
        mut current_line: impl FnMut(&DeclTarget) -> Option<u32>,
    ) -> Hover {
        let blocks: Vec<String> = self
            .blocks
            .iter()
            .map(|block| match block {
                HoverBlock::Message(message) => message.clone(),
                HoverBlock::Code { detail, defined_at } => {
                    let place = defined_at
                        .as_ref()
                        .and_then(|target| Some((target, current_line(target)?)));
                    let place = place.map_or_else(String::new, |(target, line)| {
                        // Open buffers compile under shadow paths; display the real
                        // project path so the site is recognizable to the user.
                        let display = real_path(Path::new(&target.file), &process_shadow_base())
                            .unwrap_or_else(|| PathBuf::from(&target.file));
                        format!("\n\ndefined at {}:{}", display.display(), line + 1)
                    });
                    format!("```systemverilog\n{detail}{place}\n```")
                }
            })
            .collect();
        Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: blocks.join("\n\n"),
            }),
            range: Some(range),
        }
    }
}

/// The hover of the symbol at the 0-based `(line, col)` position, if any.
///
/// A macro name is described from the analysis' macro table. Every other name
/// follows its definition ([`definitions_at`]): the hover describes the one
/// declaration it is bound to, or the declaration the cursor is on, as a
/// SystemVerilog code block (e.g. `input logic [7:0] count`). A name with
/// several definitions has no hover. Parameter/localparam hovers additionally
/// show the elaborated value from the committed model as a `value = <const>`
/// line (omitted when unresolved/ambiguous). Function/task declarations
/// additionally show the instance scope and the `automatic`/`static` storage
/// class from the model's per-instance clone.
pub(crate) fn hover_text_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<HoverText> {
    if let Some(hover) = macro_hover_at(a, file, line, col) {
        return Some(hover);
    }
    let token = identifier_at(a, file, line, col)?;
    let range = token_range(token);
    let target = match a.bindings.definition(token)? {
        tokens::Definition::Targets([target]) => target,
        tokens::Definition::Targets(_) => return None,
        tokens::Definition::Itself => return hover_declaration(a, token, range),
    };
    hover_for_target(a, target, range)
}

/// The place of an instance, which its hover names.
fn instance_site(entry: &SymEntry) -> Option<DeclTarget> {
    (entry.kind == SymKind::Instance).then(|| DeclTarget {
        name: entry.name.clone(),
        kind: kind_label(entry.kind).to_owned(),
        file: entry.file.clone(),
        line0: entry.line,
        col0: entry.col,
        via_label: false,
        via_connection: false,
    })
}

/// Hover of the declaration the cursor is on.
fn hover_declaration(a: &Analysis, token: &TokenInfo, range: Range) -> Option<HoverText> {
    let entry = a
        .index
        .decl_at(&token.file, range.start.line, range.start.character);
    let mut detail = entry.and_then(|entry| entry.detail.clone()).or_else(|| {
        a.decl_details
            .get(&(token.file.clone(), token.line, token.col))
            .filter(|text| !text.is_empty())
            .cloned()
    })?;
    if let Some(entry) = entry {
        if entry.kind == SymKind::Param {
            detail = match param_elab_value(a, &entry.file, entry.line, &entry.name) {
                Some(value) => with_elab_value(detail, value),
                None => normalize_inline_elab_value(detail),
            };
        }
        if let Some(function) = func_from_decl(&a.model, entry) {
            detail = format!(
                "{}\n\nscope: {}\n{}",
                func_signature(function),
                function.scope,
                if function.automatic {
                    "automatic"
                } else {
                    "static"
                }
            );
        }
    }
    let site = entry.and_then(instance_site);
    Some(HoverText::code(range, detail, site))
}

/// The range of a token's name.
pub(crate) fn token_range(token: &TokenInfo) -> Range {
    let start = Position::new(token.line.saturating_sub(1), token.col.saturating_sub(1));
    Range::new(
        start,
        Position::new(
            token.end_line.saturating_sub(1),
            token.end_col.saturating_sub(1),
        ),
    )
}

/// Hover of a written macro name, rendered from the committed macro table
/// ([`macros::MacroTable`]).
///
/// House style mirrors parameter hovers: a fenced SystemVerilog block with
/// one short detail line — `macro WIDTH = 8`, `macro MAX(a, b) = …`, or
/// `macro ENABLE` for a body-less macro — plus a `defined at <file>:<line>`
/// line for source-origin definitions (config defines name no site; the
/// shared-file hover annotation already labels config-derived text), one
/// block per definition the name stands for. A use of a macro that is not
/// defined renders an explicit message naming the configuration source
/// instead of any value; the operand of a conditional directive or `` `undef ``
/// that is not defined, and a macro of the language, have no hover.
fn macro_hover_at(a: &Analysis, file: &str, line: u32, col: u32) -> Option<HoverText> {
    let site = a.macros.site_at(file, line, col)?;
    let mut blocks: Vec<HoverBlock> = a
        .macros
        .definitions(site)
        .map(|definition| HoverBlock::Code {
            detail: macro_definition_detail(definition),
            defined_at: definition.target(),
        })
        .collect();
    if blocks.is_empty() {
        if site.kind != macros::MacroSiteKind::Use || site.predefined {
            return None;
        }
        let message = undefined_macro_hover(&site.name, a.macros.config_note());
        blocks.push(HoverBlock::Message(message));
    }
    let range = Range::new(
        Position::new(line, site.col_start0),
        Position::new(line, site.col_end0),
    );
    Some(HoverText { range, blocks })
}

fn macro_definition_detail(definition: &macros::MacroDefinition) -> String {
    let mut detail = format!("macro {}", definition.name);
    if let Some(args) = &definition.args {
        detail.push_str(&format!("({})", args.join(", ")));
    }
    if !definition.body.is_empty() {
        detail.push_str(&format!(" = {}", definition.body));
    }
    detail
}

fn undefined_macro_hover(name: &str, config_note: Option<&str>) -> String {
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

/// Hover for the declaration a name is bound to.
///
/// Detail precedence: the indexed declaration entry at the target position
/// (carries model data for functions/classes/instances), then the
/// position-accurate snippet captured during the semantic walk
/// ([`Analysis::decl_details`]). Parameter targets additionally gain the
/// elaborated-value line from the committed model (omitted when
/// unresolved/ambiguous). `None` when no detail can be produced. `range` is
/// the clicked name.
fn hover_for_target(a: &Analysis, target: &DeclTarget, range: Range) -> Option<HoverText> {
    let entry = a.index.decl_at(&target.file, target.line0, target.col0);
    let detail = entry.and_then(|entry| entry.detail.clone()).or_else(|| {
        a.decl_details
            .get(&(target.file.clone(), target.line0 + 1, target.col0 + 1))
            .filter(|text| !text.is_empty())
            .cloned()
    })?;
    // Parameter targets (elaboration-bound refs, override labels included)
    // gain the elaborated-value line from the committed model.
    let detail = if target.kind == "parameter" {
        match param_elab_value(a, &target.file, target.line0, &target.name) {
            Some(v) => with_elab_value(detail, v),
            // The indexed model detail can already carry Slang's elaborated
            // constant even when definition-span lookup cannot identify a
            // unique instance (for example a top-level localparam).
            None => normalize_inline_elab_value(detail),
        }
    } else {
        detail
    };
    let site = entry.and_then(instance_site);
    Some(HoverText::code(range, detail, site))
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
    let instance_function = all_instances(&model.top_instances)
        .into_iter()
        .find_map(|inst| {
            if inst.full_name != want_scope && clean_name(&inst.full_name) != want_scope {
                return None;
            }
            inst.funcs
                .iter()
                .find(|f| f.name == decl.name && f.line == want_line)
        });
    instance_function.or_else(|| {
        model.classes.iter().find_map(|class| {
            if clean_name(&class.name) != clean_name(want_scope) {
                return None;
            }
            class
                .methods
                .iter()
                .find(|method| method.name == decl.name && method.line == want_line)
        })
    })
}

// ── Definition ────────────────────────────────────────────────────────────────

/// A location in the text an analysis compiled, and the identifier that text
/// has there (`None` for an included file, whose location is its start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedLocation {
    pub name: Option<String>,
    pub location: Location,
}

/// The definitions of the name at the 0-based position: the targets its token
/// is bound to, or the token itself when it declares the name. Highlighting
/// encodes the same tokens ([`tokens::TokenRule`]), so a name has a definition
/// exactly when it has a semantic token, apart from the exceptions listed
/// there. Several targets, for a name several syntax trees or macro expansions
/// resolve differently, are all returned.
pub fn definitions_at(a: &Analysis, file: &str, line: u32, col: u32) -> Vec<NamedLocation> {
    match file_tokens(a, file) {
        Some(file_tokens) => definitions_in(&file_tokens.nodes, &a.bindings, line, col),
        None => Vec::new(),
    }
}

fn definitions_in(
    nodes: &[TokenInfo],
    bindings: &tokens::Bindings,
    line: u32,
    col: u32,
) -> Vec<NamedLocation> {
    let Some(token) = tokens::token_at(nodes, line, col) else {
        return Vec::new();
    };
    match bindings.definition(token) {
        Some(tokens::Definition::Targets(targets)) => targets.iter().map(target_location).collect(),
        Some(tokens::Definition::Itself) => vec![NamedLocation {
            name: token.name.clone(),
            location: Location {
                uri: file_uri(&token.file),
                range: token_range(token),
            },
        }],
        None => Vec::new(),
    }
}

fn file_uri(file: &str) -> Url {
    Url::from_file_path(file)
        .unwrap_or_else(|_| Url::parse("untitled:llg").expect("static URL is valid"))
}

/// LSP location for a bound declaration target (already 0-based), spanning
/// the target name with the same identifier-width convention as
/// [`entry_location`]; an included file is its start.
pub(crate) fn target_location(t: &DeclTarget) -> NamedLocation {
    let start = Position::new(t.line0, t.col0);
    let name = (t.kind != tokens::TARGET_FILE).then(|| t.name.clone());
    let range = name_range(start, name.as_deref().unwrap_or_default());
    NamedLocation {
        name,
        location: Location {
            uri: file_uri(&t.file),
            range,
        },
    }
}

/// The range of the identifier `name` at `start`.
pub(crate) fn name_range(start: Position, name: &str) -> Range {
    let end = Position::new(start.line, start.character + lsp_name_len(name));
    Range::new(start, end)
}

/// Whether `line` has the identifier `name` at the UTF-16 column `start`: plain,
/// escaped (5.6.1) or as the use of a macro.
pub(crate) fn spells_identifier(line: &str, start: u32, name: &str) -> bool {
    let name = name.strip_prefix(['\\', '`']).unwrap_or(name);
    let (before, text) = line.split_at(utf16_byte_offset(line, start));
    let (text, inside) = match text.strip_prefix('`') {
        Some(text) => (text, false),
        None => {
            let prefixed = before.ends_with(|last| is_word(last) || last == '\\')
                || before.ends_with('`') && !before.ends_with("``");
            (text, prefixed)
        }
    };
    match text.strip_prefix('\\') {
        // White space ends it.
        Some(text) => text
            .strip_prefix(name)
            .is_some_and(|after| after.chars().next().is_none_or(char::is_whitespace)),
        None => text
            .strip_prefix(name)
            .is_some_and(|after| !inside && !after.starts_with(is_word)),
    }
}

// ── References ────────────────────────────────────────────────────────────────

/// The declaration and every reference of the identifier at the 0-based
/// position, deduplicated by position.
#[cfg_attr(not(test), allow(dead_code))]
pub fn references_at(a: &Analysis, file: &str, line: u32, col: u32) -> Vec<Location> {
    references_at_with_options(a, file, line, col, true)
}

/// The token at the 0-based position.
pub(crate) fn identifier_at<'a>(
    a: &'a Analysis,
    file: &str,
    line: u32,
    col: u32,
) -> Option<&'a TokenInfo> {
    tokens::token_at(&file_tokens(a, file)?.nodes, line, col)
}

/// The declaration that the identifier at the 0-based position names: the one
/// its reference token is bound to, or itself. Names in `` `define `` text,
/// macro names, include paths and names with several definitions are no
/// references of the design and have none.
pub(crate) fn declaration_at(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
) -> Option<tokens::TokenKey> {
    let token = identifier_at(a, file, line, col)?;
    let key = (
        token.file.clone(),
        token.line.checked_sub(1)?,
        token.col.checked_sub(1)?,
    );
    if let Some(target) = a.bindings.references.get(&key) {
        return Some((target.file.clone(), target.line0, target.col0));
    }
    let class = tokens::token_class(token.kind);
    (class.rule() == tokens::TokenRule::Declaration
        && !class.macro_body
        && class.kind != tokens::TOKEN_SLANG_MACRO)
        .then_some(key)
}

/// All occurrences of the identifier at the 0-based position, honoring the
/// LSP `ReferenceContext.includeDeclaration` option: every reference bound to
/// the declaration it names ([`declaration_at`]).
pub fn references_at_with_options(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
    include_declaration: bool,
) -> Vec<Location> {
    let Some((head_file, head_line, head_col)) = declaration_at(a, file, line, col) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if include_declaration {
        let declaration = file_tokens(a, &head_file)
            .and_then(|tokens| tokens::token_at(&tokens.nodes, head_line, head_col));
        if let Some(declaration) = declaration {
            out.push(Location {
                uri: file_uri(&head_file),
                range: token_range(declaration),
            });
        }
    }
    let mut references: Vec<_> = a
        .bindings
        .references
        .iter()
        .filter(|(key, target)| {
            (target.line0, target.col0) == (head_line, head_col)
                && target.file == head_file
                && (&key.0, key.1, key.2) != (&head_file, head_line, head_col)
        })
        .collect();
    references.sort_by(|left, right| left.0.cmp(right.0));
    out.extend(
        references
            .into_iter()
            .map(|((file, line, col), target)| Location {
                uri: file_uri(file),
                range: Range::new(
                    Position::new(*line, *col),
                    Position::new(*line, *col + lsp_name_len(&target.name)),
                ),
            }),
    );
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
/// position-accurate semantic DB declaration snippet ([`Analysis::decl_details`])
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
            let decl_rank = i32::from(!tokens::token_class(n.kind).declaration);
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
/// signals, parameters — kind mapped from the symbol kind), the `unsaved` names
/// that the document declares and the analysis has not seen, and a curated list
/// of SystemVerilog keywords.
pub fn completion_at(
    a: &Analysis,
    unsaved: &[DeclaredName],
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
    let declared = a.index.decls.iter().map(|entry| (&entry.name, entry.kind));
    let unsaved = unsaved.iter().map(|unsaved| (&unsaved.name, unsaved.kind));
    for (name, kind) in declared.chain(unsaved) {
        if seen.insert(name.clone()) {
            items.push(CompletionItem {
                label: name.clone(),
                kind: Some(completion_kind_for(kind)),
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
pub(crate) fn utf16_byte_offset(line: &str, col: u32) -> usize {
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

/// Strip a Slang library prefix (`lib@name` → `name`) from a design name.
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

/// `true` when `file` is Slang's virtual builtin file (`<cwd>/builtin.sv`).
///
/// Slang parses the builtin classes (mailbox/process/semaphore) from a
/// string and reports them under this path, which never exists on disk;
/// declarations there are skipped so goto-definition cannot dead-end.
pub(super) fn builtin_file(file: &str) -> bool {
    Path::new(file).file_name().and_then(|n| n.to_str()) == Some("builtin.sv")
}

/// The token list for `file`: the file itself, however its path is spelled.
pub(super) fn file_tokens<'a>(a: &'a Analysis, file: &str) -> Option<&'a FileTokens> {
    if let Some(tokens) = a.tokens.iter().find(|tokens| tokens.path == file) {
        return Some(tokens);
    }
    let file = llg::ffi::platform::canonicalize(file).ok()?;
    a.tokens.iter().find(|tokens| {
        llg::ffi::platform::canonicalize(&tokens.path).is_ok_and(|path| path == file)
    })
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

// ── Location construction ─────────────────────────────────────────────────────

/// LSP location for an index entry (already 0-based).
pub(super) fn entry_location(e: &SymEntry) -> Location {
    let len = lsp_name_len(&e.name);
    Location {
        uri: file_uri(&e.file),
        range: Range::new(
            Position::new(e.line, e.col),
            Position::new(e.line, e.col + len),
        ),
    }
}
