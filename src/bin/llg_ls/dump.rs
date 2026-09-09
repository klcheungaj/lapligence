//! dump — offline `llg --dump-tokens <PATH>` diagnostic mode.
//!
//! Runs the SAME analysis pipeline the LSP serves features from (config load,
//! `.v`/`.sv` discovery, longest-root ownership, `config::compile_opts`,
//! [`features::analyze_with_config`]) without staging any buffers, then prints
//! one deterministic line per indexed token occurrence: location, name, raw
//! lexical kind, declaration/reference classification, semantic-token legend
//! classification and the elaboration binding target.  The output is meant to
//! be diffed or pasted verbatim when navigation correctness on a real project
//! must be checked away from an editor.
//!
//! The routine runs BEFORE the LSP runtime starts (see `main.rs`) and writes
//! plain lines to stdout — the stdout-is-LSP-framing rule only applies while
//! serving.  Output is LF-only, uncolored, timestamp-free and sorted by
//! `(relative file, line0, col0)`, so identical inputs produce byte-identical
//! dumps.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tower_lsp::lsp_types::SemanticToken;

use crate::config::{self, LlgConfig};
use crate::features::{self, Analysis, AnalysisOutcome};
use crate::semantic_tokens;
use crate::workspace::{self, RootDescriptor};
use llg::core::tokens;

// ── Entry point ───────────────────────────────────────────────────────────────

/// Inspect the process arguments: when the first argument is `--dump-tokens`,
/// run the dump for the following path and return the process exit code.
/// `None` when the arguments do not select dump mode (normal LSP serving).
pub fn run_from_env() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--dump-tokens") {
        return None;
    }
    Some(match args.get(1) {
        Some(raw) => run(Path::new(raw)),
        None => {
            eprintln!("usage: llg --dump-tokens <PATH>");
            2
        }
    })
}

/// A resolved dump target: the analysis root, its effective config and — for
/// single-file invocations — the one file whose rows survive filtering.
struct Resolved {
    root: PathBuf,
    config: LlgConfig,
    filter: Option<PathBuf>,
}

/// Run the dump for `target` (a directory or a file) and return the exit code
/// (0 on success, 2 on usage/path errors).  Diagnostics go to stderr; the
/// report itself goes to stdout.
pub fn run(target: &Path) -> i32 {
    let resolved = match resolve_target(target) {
        Ok(resolved) => resolved,
        Err(message) => {
            eprintln!("llg --dump-tokens: {message}");
            return 2;
        }
    };
    let Resolved {
        root,
        config,
        filter,
    } = resolved;

    let descriptor = RootDescriptor::from_absolute(&root)
        .expect("dump root is normalized")
        .with_config(Some(Arc::new(config.clone())));

    // Same discovery + ownership shape as `Backend::make_jobs`: units under
    // the configured source dirs, kept only when this root structurally owns
    // them.
    let mut units = match workspace::discover_units(&descriptor) {
        Ok(units) => units,
        Err(error) => {
            eprintln!(
                "llg --dump-tokens: discovery failed for {}: {error}",
                root.display()
            );
            Vec::new()
        }
    };
    units.retain(|file| {
        workspace::owning_root_unfiltered(file, std::slice::from_ref(&descriptor))
            .is_some_and(|owner| owner.root == descriptor.root)
    });
    let files: Vec<String> = units
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();

    // No buffers are staged: the compile runs on the discovered on-disk files.
    // The shadow include dirs that precede the real ones are empty, exactly
    // like a server pass over a workspace with no open editors.
    let opts = config::compile_opts(&config, files.clone(), &features::process_shadow_base());
    let analysis = features::analyze_with_config(&opts, &config.lint);

    let candidates: Vec<String> = filter
        .as_ref()
        .map(|path| vec![path.to_string_lossy().into_owned()])
        .unwrap_or_default();
    let rows = collect_rows_for(&analysis, &root, &candidates);
    print_report(files.len(), &rows, &analysis);

    // Nothing user-owned lives under the shadow base in dump mode; remove the
    // private tree used for admitted source mirrors.
    features::cleanup_process_shadow();
    0
}

fn resolve_target(target: &Path) -> Result<Resolved, String> {
    let metadata = std::fs::metadata(target)
        .map_err(|error| format!("cannot access {}: {error}", target.display()))?;
    if metadata.is_dir() {
        let root = normalize(target).ok_or("dump root must resolve to an absolute path")?;
        Ok(Resolved {
            config: load_root_config(&root),
            root,
            filter: None,
        })
    } else if metadata.is_file() {
        let file = normalize(target).ok_or("dump target must resolve to an absolute path")?;
        let root = find_config_root(&file);
        Ok(Resolved {
            config: load_root_config(&root),
            root,
            filter: Some(file),
        })
    } else {
        Err(format!(
            "not a regular file or directory: {}",
            target.display()
        ))
    }
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    workspace::normalize_absolute_path(&absolute)
}

/// Directory holding `file`, or its parent when no ancestor ships a config.
fn find_config_root(file: &Path) -> PathBuf {
    let fallback = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    let mut dir = fallback.clone();
    loop {
        if dir.join(config::CONFIG_FILE).is_file() {
            return dir;
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => return fallback,
        }
    }
}

/// Load `<root>/llg.toml` with the server's semantics: missing configs use
/// safe defaults; malformed ones warn on stderr and fall back to defaults too
/// (the server retains last-valid state, which a fresh CLI process has none
/// of).
fn load_root_config(root: &Path) -> LlgConfig {
    match config::load_config_file(&root.join(config::CONFIG_FILE)) {
        Ok(load) => match load.config {
            Some(config) => config,
            None => {
                for error in &load.errors {
                    eprintln!(
                        "llg --dump-tokens: invalid {}: {}",
                        load.path.display(),
                        error.message
                    );
                }
                config::default_config(root)
            }
        },
        Err(error) => {
            eprintln!(
                "llg --dump-tokens: cannot read {}: {error}",
                root.join(config::CONFIG_FILE).display()
            );
            config::default_config(root)
        }
    }
}

// ── Row collection ────────────────────────────────────────────────────────────

/// One output row before display-path resolution.
pub(crate) struct OutRow {
    /// Absolute token file path (filtering + relativization input).
    abs_file: String,
    line: u32,
    col: u32,
    end_col: u32,
    name: String,
    token_kind: String,
    is_decl: bool,
    sym: Option<(String, Vec<String>)>,
    bind: Option<BindOut>,
    /// Display form of `abs_file` (root-relative when possible).
    disp_file: String,
}

struct BindOut {
    abs_file: String,
    line: u32,
    col: u32,
    name: String,
    kind: String,
    via_label: bool,
    via_connection: bool,
    /// Display form of `abs_file`.
    disp_file: String,
}

/// One decoded (absolute-position) semantic token from a file's stream.
struct SemTok {
    line: u32,
    col: u32,
    len: u32,
    ty: u32,
    mods: u32,
}

fn decode_semantic_tokens(data: &[SemanticToken]) -> Vec<SemTok> {
    let mut out = Vec::with_capacity(data.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for token in data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        out.push(SemTok {
            line,
            col,
            len: token.length,
            ty: token.token_type,
            mods: token.token_modifiers_bitset,
        });
    }
    out
}

/// The `sym=` field components for a position: `(legend type name,
/// modifier names in legend/bit order)`, or `None` when no semantic token
/// covers it.
///
/// An exact start match wins; otherwise the covering token (same line,
/// `col` inside `[start, start+len)`) is used.
fn sym_field(
    legend_types: &[String],
    legend_modifiers: &[String],
    tokens: &[SemTok],
    line: u32,
    col: u32,
) -> Option<(String, Vec<String>)> {
    let exact = tokens.iter().find(|t| t.line == line && t.col == col);
    let covered = exact.or_else(|| {
        tokens
            .iter()
            .find(|t| t.line == line && col >= t.col && col < t.col.saturating_add(t.len))
    })?;
    let ty = legend_types.get(covered.ty as usize)?;
    let modifiers: Vec<String> = legend_modifiers
        .iter()
        .enumerate()
        .filter(|(bit, _)| covered.mods & (1 << bit) != 0)
        .map(|(_, name)| name.clone())
        .collect();
    Some((ty.clone(), modifiers))
}

/// Binding lookup mirroring `features::definition_at`: exact key position
/// first, then the containing identifier's start column via the index.
fn binding_at<'a>(
    analysis: &'a Analysis,
    file: &str,
    line: u32,
    col: u32,
) -> Option<&'a llg::core::tokens::DeclTarget> {
    let bindings = &analysis.ref_bindings;
    if let Some(target) = bindings.get(&(file.to_owned(), line, col)) {
        return Some(target);
    }
    if let Some(entry) = analysis.index.entry_at(file, line, col) {
        if let Some(target) = bindings.get(&(file.to_owned(), line, entry.col)) {
            return Some(target);
        }
    }
    None
}

/// Collect the dump rows for one analysis, relativizing display paths against
/// `display_root`.
///
/// When `filter_candidates` is non-empty only rows whose token file matches
/// one of the candidates survive; the LSP `llg/dumpTokens` handler passes the
/// shadow-first candidate set of the requested document (open buffers compile
/// under shadow paths, so the analysis' internal file strings may be shadow
/// paths that appear verbatim among the candidates).  An empty slice keeps
/// every row — the CLI directory-dump shape.
pub(crate) fn collect_rows_for(
    analysis: &Analysis,
    display_root: &Path,
    filter_candidates: &[String],
) -> Vec<OutRow> {
    // Position → DECL/REF classification from the symbol index.  Both vectors
    // are position-deduplicated, so each key maps to exactly one class.
    let mut classified: HashMap<(&str, u32, u32), bool> = HashMap::new();
    for entry in analysis.index.decls.iter().chain(&analysis.index.refs) {
        classified.insert((entry.file.as_str(), entry.line, entry.col), entry.is_decl);
    }

    let legend = semantic_tokens::legend();
    let legend_types: Vec<String> = legend
        .token_types
        .iter()
        .map(|ty| ty.as_str().to_owned())
        .collect();
    let legend_modifiers: Vec<String> = legend
        .token_modifiers
        .iter()
        .map(|modifier| modifier.as_str().to_owned())
        .collect();

    // Decoded semantic-token streams per file (computed once per file).
    let mut sem_by_file: HashMap<String, Vec<SemTok>> = HashMap::new();
    for ft in &analysis.tokens {
        let data = features::semantic_tokens_for(analysis, &ft.path).data;
        sem_by_file.insert(ft.path.clone(), decode_semantic_tokens(&data));
    }

    let filters: Option<(Vec<PathBuf>, HashSet<&str>)> = if filter_candidates.is_empty() {
        None
    } else {
        Some((
            filter_candidates
                .iter()
                .filter_map(|candidate| normalize(Path::new(candidate)))
                .collect(),
            filter_candidates.iter().map(String::as_str).collect(),
        ))
    };

    let mut rows: Vec<OutRow> = Vec::new();
    for ft in &analysis.tokens {
        if let Some((normalized_filters, raw_filters)) = &filters {
            let matches = match normalize(Path::new(&ft.path)) {
                Some(normalized) => normalized_filters.contains(&normalized),
                None => raw_filters.contains(ft.path.as_str()),
            };
            if !matches {
                continue;
            }
        }
        let sem = sem_by_file.get(&ft.path);
        for node in &ft.nodes {
            let Some(name) = node.name.as_deref().filter(|name| !name.is_empty()) else {
                continue;
            };
            if node.line == 0 || node.col == 0 {
                continue;
            }
            let line0 = node.line - 1;
            let col0 = node.col - 1;
            let Some(&is_decl) = classified.get(&(ft.path.as_str(), line0, col0)) else {
                continue; // keywords/macros etc.: not indexed as DECL or REF
            };
            let sym =
                sem.and_then(|toks| sym_field(&legend_types, &legend_modifiers, toks, line0, col0));
            let bind = binding_at(analysis, &ft.path, line0, col0).map(|target| BindOut {
                abs_file: target.file.clone(),
                line: target.line0,
                col: target.col0,
                name: target.name.clone(),
                kind: target.kind.clone(),
                via_label: target.via_label,
                via_connection: target.via_connection,
                disp_file: String::new(),
            });
            rows.push(OutRow {
                abs_file: ft.path.clone(),
                line: line0,
                col: col0,
                end_col: col0.saturating_add(name.chars().count() as u32),
                name: name.to_owned(),
                token_kind: token_kind_name(node.kind),
                is_decl,
                sym,
                bind,
                disp_file: String::new(),
            });
        }
    }

    // Sort by (relative file, line, col), then resolve the display paths so
    // printing never re-derives them.
    rows.sort_by(|left, right| {
        display_path(display_root, &left.abs_file)
            .cmp(&display_path(display_root, &right.abs_file))
            .then(left.line.cmp(&right.line))
            .then(left.col.cmp(&right.col))
    });
    for row in &mut rows {
        row.disp_file = display_path(display_root, &row.abs_file);
        if let Some(bind) = &mut row.bind {
            bind.disp_file = display_path(display_root, &bind.abs_file);
        }
    }
    rows
}

/// Display form of an analysis path: root-relative when the file lives under
/// the root, otherwise the absolute path printed as-is.
///
/// Staged-buffer (shadow-tree) paths are mapped back to their real locations
/// first (pure string reversal over the process shadow base) so an LSP dump of
/// an open document shows workspace-relative rows.  The CLI never stages
/// buffers, so its analysis paths are never under the shadow base and this
/// mapping is a no-op there.
fn display_path(root: &Path, path: &str) -> String {
    let shadow_base = features::process_shadow_base();
    let real = features::real_path(Path::new(path), &shadow_base);
    let resolved = real.as_deref().unwrap_or_else(|| Path::new(path));
    if let Some(relative) = workspace::root_relative_path(root, resolved) {
        return relative.to_string_lossy().replace('\\', "/");
    }
    match normalize(resolved) {
        Some(absolute) => absolute.to_string_lossy().replace('\\', "/"),
        None => resolved.to_string_lossy().into_owned(),
    }
}

fn outcome_label(outcome: AnalysisOutcome) -> &'static str {
    match outcome {
        AnalysisOutcome::Valid => "valid",
        AnalysisOutcome::Fatal => "fatal",
        AnalysisOutcome::Parse => "parse",
        AnalysisOutcome::Compile => "compile",
    }
}

/// The complete report as display lines: the header (built from
/// `files_label`), one formatted line per row, and the trailing `# analysis:`
/// summary.  The CLI prints every line; the LSP `llg/dumpTokens` handler
/// serves only the rows plus the summary (see `lsp.rs`).
pub(crate) fn format_rows(files_label: &str, rows: &[OutRow], analysis: &Analysis) -> Vec<String> {
    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(format!("# llg token dump {files_label}"));
    for row in rows {
        let parts = LineParts {
            file: &row.disp_file,
            line: row.line,
            col: row.col,
            end_col: row.end_col,
            name: &row.name,
            token_kind: &row.token_kind,
            is_decl: row.is_decl,
            sym: row
                .sym
                .as_ref()
                .map(|(ty, mods)| (ty.as_str(), mods.as_slice())),
            bind: row.bind.as_ref().map(|bind| BindParts {
                file: &bind.disp_file,
                line: bind.line,
                col: bind.col,
                name: &bind.name,
                kind: &bind.kind,
                via_label: bind.via_label,
                via_connection: bind.via_connection,
            }),
        };
        lines.push(format_line(&parts));
    }
    lines.push(format!(
        "# analysis: outcome={} modules={} bindings={}",
        outcome_label(analysis.outcome),
        analysis.model.modules.len(),
        analysis.ref_bindings.len()
    ));
    lines
}

fn print_report(files: usize, rows: &[OutRow], analysis: &Analysis) {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in format_rows(&format!("root=. files={files}"), rows, analysis) {
        let _ = writeln!(out, "{line}");
    }
}

// ── Line formatter ────────────────────────────────────────────────────────────

/// Fully-resolved display fields of one dump line (unit-testable pure shape).
pub(crate) struct LineParts<'a> {
    pub file: &'a str,
    pub line: u32,
    pub col: u32,
    pub end_col: u32,
    pub name: &'a str,
    pub token_kind: &'a str,
    pub is_decl: bool,
    /// Legend type name plus modifier names; `None` renders `sym=-`.
    pub sym: Option<(&'a str, &'a [String])>,
    pub bind: Option<BindParts<'a>>,
}

pub(crate) struct BindParts<'a> {
    pub file: &'a str,
    pub line: u32,
    pub col: u32,
    pub name: &'a str,
    pub kind: &'a str,
    pub via_label: bool,
    pub via_connection: bool,
}

/// Render one report line in the stable machine-parseable format:
///
/// ```text
/// <relfile>:<line0>:<col0>-<col1>\t<name>\tkind=<TokenKind>\tDECL|REF\tsym=<type>[/<mods>]\tbind=<relfile>:<line0>:<col0>[<name>,<kind>]|\t[via=label][\tvia=connection]
/// ```
///
/// `bind=-` marks an unbound occurrence; `via=label` / `via=connection` are
/// appended as extra tab-separated fields when the binding came from the
/// port-connection folds (label side / connected-signal side).
pub(crate) fn format_line(parts: &LineParts) -> String {
    let classification = if parts.is_decl { "DECL" } else { "REF" };
    let sym = match parts.sym {
        Some((ty, [])) => (*ty).to_owned(),
        Some((ty, modifiers)) => format!("{ty}/{}", modifiers.join("+")),
        None => "-".to_owned(),
    };
    let mut line = format!(
        "{file}:{line}:{col}-{end}\t{name}\tkind={token_kind}\t{classification}\tsym={sym}\tbind=",
        file = parts.file,
        line = parts.line,
        col = parts.col,
        end = parts.end_col,
        name = parts.name,
        token_kind = parts.token_kind,
    );
    match &parts.bind {
        Some(bind) => {
            line.push_str(&format!(
                "{}:{}:{}[{},{}]",
                bind.file, bind.line, bind.col, bind.name, bind.kind
            ));
            if bind.via_label {
                line.push_str("\tvia=label");
            }
            if bind.via_connection {
                line.push_str("\tvia=connection");
            }
        }
        None => line.push('-'),
    }
    line
}

// ── Lexical kind names ───────────────────────────────────────────────────────

fn token_kind_name(kind: i32) -> String {
    let (base, declaration) = tokens::token_base_kind(kind);
    const NAMES: &[(i32, &str)] = &[
        (tokens::TOKEN_SLANG_MODULE, "module"),
        (tokens::TOKEN_SLANG_INTERFACE, "interface"),
        (tokens::TOKEN_SLANG_PROGRAM, "program"),
        (tokens::TOKEN_SLANG_PACKAGE, "package"),
        (tokens::TOKEN_SLANG_CLASS, "class"),
        (tokens::TOKEN_SLANG_ENUM_MEMBER, "enum-member"),
        (tokens::TOKEN_SLANG_TYPE_ALIAS, "type-alias"),
        (tokens::TOKEN_SLANG_PARAMETER, "parameter"),
        (tokens::TOKEN_SLANG_PORT, "port"),
        (tokens::TOKEN_SLANG_VARIABLE, "variable"),
        (tokens::TOKEN_SLANG_NET, "net"),
        (tokens::TOKEN_SLANG_FUNCTION, "function"),
        (tokens::TOKEN_SLANG_TASK, "task"),
        (tokens::TOKEN_SLANG_IDENTIFIER, "identifier"),
        (
            tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL,
            "port-connection-label",
        ),
        (
            tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL,
            "parameter-connection-label",
        ),
    ];
    for (value, name) in NAMES {
        if *value == base {
            return if declaration {
                format!("{name}-declaration")
            } else {
                (*name).to_owned()
            };
        }
    }
    format!("token-{kind}")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn formats_reference_line_with_sym_and_binding() {
        let modifiers = mods(&["readonly"]);
        let bind_file = "m_a.sv".to_owned();
        let parts = LineParts {
            file: "m_a.sv",
            line: 1,
            col: 62,
            end_col: 65,
            name: "clk",
            token_kind: "identifier",
            is_decl: false,
            sym: Some(("parameter", &modifiers)),
            bind: Some(BindParts {
                file: &bind_file,
                line: 0,
                col: 23,
                name: "clk",
                kind: "port",
                via_label: false,
                via_connection: false,
            }),
        };
        assert_eq!(
            format_line(&parts),
            "m_a.sv:1:62-65\tclk\tkind=identifier\tREF\tsym=parameter/readonly\tbind=m_a.sv:0:23[clk,port]"
        );
    }

    #[test]
    fn formats_decl_line_without_modifiers_or_binding() {
        let parts = LineParts {
            file: "tb.sv",
            line: 2,
            col: 8,
            end_col: 10,
            name: "wa",
            token_kind: "variable",
            is_decl: true,
            sym: Some(("variable", &[])),
            bind: None,
        };
        assert_eq!(
            format_line(&parts),
            "tb.sv:2:8-10\twa\tkind=variable\tDECL\tsym=variable\tbind=-"
        );
    }

    #[test]
    fn formats_port_label_binding_with_via_field() {
        let bind_file = "child.sv".to_owned();
        let parts = LineParts {
            file: "top.sv",
            line: 5,
            col: 12,
            end_col: 15,
            name: "clk",
            token_kind: "function",
            is_decl: false,
            sym: None,
            bind: Some(BindParts {
                file: &bind_file,
                line: 3,
                col: 12,
                name: "clk",
                kind: "port",
                via_label: true,
                via_connection: false,
            }),
        };
        assert_eq!(
            format_line(&parts),
            "top.sv:5:12-15\tclk\tkind=function\tREF\tsym=-\tbind=child.sv:3:12[clk,port]\tvia=label"
        );
    }

    #[test]
    fn formats_connection_actual_binding_with_via_field() {
        // The connected signal (ACTUAL) of a named port connection binds to
        // the child module's port declaration, tagged `via=connection`
        // (argument→parameter navigation).
        let bind_file = "child.sv".to_owned();
        let parts = LineParts {
            file: "top.sv",
            line: 5,
            col: 16,
            end_col: 18,
            name: "wa",
            token_kind: "variable",
            is_decl: false,
            sym: Some(("variable", &[])),
            bind: Some(BindParts {
                file: &bind_file,
                line: 3,
                col: 12,
                name: "clk",
                kind: "port",
                via_label: false,
                via_connection: true,
            }),
        };
        assert_eq!(
            format_line(&parts),
            "top.sv:5:16-18\twa\tkind=variable\tREF\tsym=variable\tbind=child.sv:3:12[clk,port]\tvia=connection"
        );
    }

    #[test]
    fn sym_field_prefers_exact_start_then_coverage() {
        let types = vec!["variable".to_owned(), "keyword".to_owned()];
        let all_modifiers = vec!["declaration".to_owned(), "readonly".to_owned()];
        let tokens = vec![SemTok {
            line: 4,
            col: 7,
            len: 3,
            ty: 0,
            mods: 1 << 0,
        }];
        // Exact start: type + declaration modifier.
        assert_eq!(
            sym_field(&types, &all_modifiers, &tokens, 4, 7),
            Some(("variable".to_owned(), vec!["declaration".to_owned()]))
        );
        // Covered mid-token falls back to the same semantic token.
        assert_eq!(
            sym_field(&types, &all_modifiers, &tokens, 4, 9),
            Some(("variable".to_owned(), vec!["declaration".to_owned()]))
        );
        // Outside every token: none.
        assert!(sym_field(&types, &all_modifiers, &tokens, 4, 11).is_none());
        assert!(sym_field(&types, &all_modifiers, &tokens, 5, 7).is_none());
    }

    #[test]
    fn decode_walks_delta_encoding() {
        let make = |delta_line: u32, delta_start: u32, length: u32| SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: 2,
            token_modifiers_bitset: 0,
        };
        let decoded = decode_semantic_tokens(&[make(0, 5, 6), make(0, 14, 3), make(2, 1, 4)]);
        let positions: Vec<(u32, u32)> = decoded.iter().map(|t| (t.line, t.col)).collect();
        assert_eq!(positions, vec![(0, 5), (0, 19), (2, 1)]);
        assert_eq!(decoded[2].len, 4);
    }

    #[test]
    fn token_kind_names_cover_declarations_and_unknown_values() {
        assert_eq!(
            token_kind_name(tokens::TOKEN_SLANG_IDENTIFIER),
            "identifier"
        );
        assert_eq!(
            token_kind_name(tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET),
            "port-declaration"
        );
        assert_eq!(token_kind_name(999_999), "token-999999");
    }
}
