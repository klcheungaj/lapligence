//! Slang-only compilation facade shared by the language server and simulator.
//!
//! The native compiler receives in-memory source buffers and returns an owned
//! snapshot. No native Slang object escapes the FFI call.

use crate::core::tokens::FileTokens;
use crate::ffi::slang::{self, CompileOptions, CompileRequest, Define, Limits, ParameterOverride};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub use crate::ffi::slang::{
    Diagnostic as SlangDiagnostic, DiagnosticProvider, DiagnosticSeverity, DiagnosticSubsystem,
    Snapshot, Source, SourceRange,
};

/// A source buffer owned by a compile request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSource {
    pub name: String,
    pub text: String,
    pub is_compilation_unit: bool,
}

impl OwnedSource {
    pub fn compilation_unit(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            is_compilation_unit: true,
        }
    }

    pub fn include(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            is_compilation_unit: false,
        }
    }
}

/// Options controlling one Slang compilation and elaboration.
#[derive(Debug, Clone, Default)]
pub struct CompileOpts {
    /// Path-based compilation units for CLI and test callers. Each path is
    /// read once before entering Slang. The LSP uses [`sources`] exclusively.
    pub files: Vec<String>,
    /// Already-admitted compilation units and include buffers.
    pub sources: Vec<OwnedSource>,
    pub top: Option<String>,
    /// Definitions in `NAME` or `NAME=VALUE` form.
    pub defines: Vec<String>,
    /// Top-level overrides in `NAME=VALUE` form.
    pub param_overrides: Vec<String>,
    /// Logical search prefixes. In-memory callers must also supply include
    /// contents in [`sources`](Self::sources); path mode admits literal
    /// includes through bounded Rust reads before entering Slang.
    pub include_dirs: Vec<String>,
    pub limits: Limits,
}

/// Frontend-neutral diagnostic class retained by Rust consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fatal,
    Syntax,
    Error,
    Warning,
    Note,
    Info,
}

/// Compact diagnostic projection. Positions are one-based UTF-16; zero means
/// unknown. The complete named Slang diagnostic remains in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub severity: Severity,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StartupErrorKind {
    InvalidArgument,
    Input,
    Frontend,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupError {
    kind: StartupErrorKind,
    message: String,
}

impl StartupError {
    fn new(kind: StartupErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn kind(&self) -> StartupErrorKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn contains(&self, pattern: &str) -> bool {
        self.message.contains(pattern)
    }
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for StartupError {}

/// Owned result of one Slang compilation.
#[derive(Debug)]
pub struct CompileOut {
    pub diagnostics: Vec<Diag>,
    pub snapshot: Snapshot,
}

impl CompileOut {
    pub fn ok(&self) -> bool {
        !self.snapshot.has_errors()
    }
}

#[derive(Debug)]
pub enum CompileError {
    Startup(StartupError),
    FrontendDiagnostics(Vec<Diag>),
}

impl CompileError {
    pub fn startup_message(&self) -> Option<&str> {
        match self {
            Self::Startup(error) => Some(error.message()),
            Self::FrontendDiagnostics(_) => None,
        }
    }
    pub fn diagnostics(&self) -> Option<&[Diag]> {
        match self {
            Self::Startup(_) => None,
            Self::FrontendDiagnostics(diagnostics) => Some(diagnostics),
        }
    }
    pub fn into_diagnostics(self) -> Option<Vec<Diag>> {
        match self {
            Self::Startup(_) => None,
            Self::FrontendDiagnostics(diagnostics) => Some(diagnostics),
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Startup(error) => error.fmt(f),
            Self::FrontendDiagnostics(diagnostics) => write!(
                f,
                "Slang reported {} blocking frontend diagnostic(s)",
                diagnostics
                    .iter()
                    .filter(|d| matches!(
                        d.severity,
                        Severity::Fatal | Severity::Syntax | Severity::Error
                    ))
                    .count()
            ),
        }
    }
}
impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Startup(error) => Some(error),
            Self::FrontendDiagnostics(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseOnlyOut {
    pub diagnostics: Vec<Diag>,
    pub tokens: Vec<FileTokens>,
    pub parsed_token_count: usize,
    pub supplemented_token_count: usize,
}

/// Compile path-based and already-admitted in-memory sources with Slang.
pub fn compile(opts: &CompileOpts) -> Result<CompileOut, StartupError> {
    preflight_options(opts)?;
    preflight_sources(&opts.sources, opts.limits)?;
    let source_count = opts
        .sources
        .len()
        .checked_add(opts.files.len())
        .ok_or_else(|| {
            StartupError::new(StartupErrorKind::InvalidArgument, "source count overflow")
        })?;
    if source_count > usize::try_from(opts.limits.max_sources).unwrap_or(usize::MAX) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    let mut owned = Vec::new();
    let admitted_bytes = opts
        .sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source byte count overflow",
            )
        })?;
    let mut remaining = opts.limits.max_source_bytes - admitted_bytes;
    let mut identities = std::collections::HashSet::new();
    for path in &opts.files {
        let resolved = absolute_path(Path::new(path))?;
        if !identities.insert(resolved.clone()) {
            continue;
        }
        let name = resolved.to_string_lossy().into_owned();
        let content_limit = remaining.checked_sub(name.len() as u64).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let text = read_bounded(&name, content_limit)?;
        remaining = remaining.saturating_sub(name.len() as u64 + text.len() as u64);
        owned.push(OwnedSource::compilation_unit(name, text));
    }
    let mut cursor = 0;
    while cursor < owned.len() {
        let including = PathBuf::from(&owned[cursor].name);
        let targets = literal_includes(&owned[cursor].text);
        for target in targets {
            let Some(path) = resolve_include(&including, &target, &opts.include_dirs) else {
                continue;
            };
            if !identities.insert(path.clone()) {
                continue;
            }
            if opts.sources.len().saturating_add(owned.len())
                >= usize::try_from(opts.limits.max_sources).unwrap_or(usize::MAX)
            {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    "literal include graph exceeds the configured Slang source limit",
                ));
            }
            let name = path.to_string_lossy().into_owned();
            let content_limit = remaining.checked_sub(name.len() as u64).ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    format!("include path {name} exceeds the configured Slang byte limit"),
                )
            })?;
            let text = read_bounded(&name, content_limit)?;
            remaining = remaining.saturating_sub(name.len() as u64 + text.len() as u64);
            owned.push(OwnedSource::include(name, text));
        }
        cursor += 1;
    }
    if opts.sources.len().saturating_add(owned.len())
        > usize::try_from(opts.limits.max_sources).unwrap_or(usize::MAX)
    {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    compile_source_groups(&opts.sources, &owned, opts)
}

/// Compile exact source buffers without reading the filesystem.
pub fn compile_sources(
    sources: &[OwnedSource],
    opts: &CompileOpts,
) -> Result<CompileOut, StartupError> {
    preflight_options(opts)?;
    preflight_sources(sources, opts.limits)?;
    compile_source_groups(sources, &[], opts)
}

fn preflight_options(opts: &CompileOpts) -> Result<(), StartupError> {
    const MAX_OPTIONS: usize = 4_096;
    const MAX_OPTION_BYTES: u64 = 4 * 1024 * 1024;
    if opts.defines.len() > MAX_OPTIONS
        || opts.param_overrides.len() > MAX_OPTIONS
        || opts.include_dirs.len() > MAX_OPTIONS
    {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "frontend option count exceeds the Slang ABI limit",
        ));
    }
    let bytes = opts
        .defines
        .iter()
        .chain(&opts.param_overrides)
        .chain(&opts.include_dirs)
        .map(|value| value.len() as u64)
        .chain(opts.top.iter().map(|value| value.len() as u64))
        .try_fold(0_u64, u64::checked_add)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "frontend option byte count overflow",
            )
        })?;
    if bytes > MAX_OPTION_BYTES {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "frontend options exceed the Slang ABI byte limit",
        ));
    }
    Ok(())
}

fn compile_source_groups(
    first: &[OwnedSource],
    second: &[OwnedSource],
    opts: &CompileOpts,
) -> Result<CompileOut, StartupError> {
    let borrowed: Vec<_> = first
        .iter()
        .chain(second)
        .map(|source| Source {
            name: &source.name,
            text: &source.text,
            is_compilation_unit: source.is_compilation_unit,
        })
        .collect();
    let options = CompileOptions {
        defines: opts
            .defines
            .iter()
            .map(|value| parse_define(value))
            .collect(),
        top_modules: opts.top.iter().cloned().collect(),
        include_dirs: opts
            .include_dirs
            .iter()
            .map(|dir| normalize_include_dir(dir))
            .collect(),
        parameter_overrides: opts
            .param_overrides
            .iter()
            .map(|value| parse_override(value))
            .collect::<Result<_, _>>()?,
        limits: opts.limits,
    };
    let snapshot = slang::compile(&CompileRequest {
        sources: &borrowed,
        options: &options,
    })
    .map_err(startup_from_slang)?;
    let diagnostics = project_diagnostics(&snapshot);
    Ok(CompileOut {
        diagnostics,
        snapshot,
    })
}

fn preflight_sources(sources: &[OwnedSource], limits: Limits) -> Result<(), StartupError> {
    if sources.len() > usize::try_from(limits.max_sources).unwrap_or(usize::MAX) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    let bytes = sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source byte count overflow",
            )
        })?;
    if bytes > limits.max_source_bytes {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source bytes exceed the configured Slang limit",
        ));
    }
    Ok(())
}

fn read_bounded(path: &str, limit: u64) -> Result<String, StartupError> {
    let file = std::fs::File::open(path).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot open SystemVerilog source {path}: {error}"),
        )
    })?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!("cannot read SystemVerilog source {path}: {error}"),
            )
        })?;
    if bytes.len() as u64 > limit {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("source {path} exceeds the configured Slang byte limit"),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("SystemVerilog source {path} is not UTF-8: {error}"),
        )
    })
}

fn absolute_path(path: &Path) -> Result<PathBuf, StartupError> {
    path.canonicalize().map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!(
                "cannot resolve SystemVerilog source {}: {error}",
                path.display()
            ),
        )
    })
}

fn normalize_include_dir(value: &str) -> String {
    let path = Path::new(value);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path).to_string_lossy().into_owned())
        .unwrap_or_else(|_| value.to_owned())
}

fn resolve_include(including: &Path, target: &str, include_dirs: &[String]) -> Option<PathBuf> {
    let target = Path::new(target);
    let mut candidates = Vec::with_capacity(include_dirs.len().saturating_add(1));
    if target.is_absolute() {
        candidates.push(target.to_path_buf());
    } else {
        if let Some(parent) = including.parent() {
            candidates.push(parent.join(target));
        }
        candidates.extend(include_dirs.iter().map(|dir| Path::new(dir).join(target)));
    }
    candidates.into_iter().find_map(|candidate| {
        candidate
            .is_file()
            .then(|| candidate.canonicalize().ok())
            .flatten()
    })
}

/// Extract literal quoted includes without treating comments or ordinary
/// strings as directives. Dynamic and macro-generated targets remain for
/// Slang to diagnose because no bounded file can be admitted for them.
fn literal_includes(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut targets = Vec::new();
    let mut index = 0;
    let mut block_comment = false;
    while index < bytes.len() {
        if block_comment {
            if bytes[index..].starts_with(b"*/") {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = index.saturating_add(2);
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if bytes[index..].starts_with(b"`include") {
            let after = index + b"`include".len();
            if after == bytes.len()
                || !matches!(bytes[after], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$')
            {
                let mut start = after;
                while start < bytes.len() && bytes[start].is_ascii_whitespace() {
                    start += 1;
                }
                if start < bytes.len() && bytes[start] == b'"' {
                    let mut end = start + 1;
                    while end < bytes.len() && bytes[end] != b'"' && bytes[end] != b'\n' {
                        end += 1;
                    }
                    if end < bytes.len() && bytes[end] == b'"' {
                        targets.push(String::from_utf8_lossy(&bytes[start + 1..end]).into_owned());
                        index = end + 1;
                        continue;
                    }
                }
            }
        }
        index += 1;
    }
    targets
}

pub fn compile_checked(opts: &CompileOpts) -> Result<CompileOut, CompileError> {
    checked(compile(opts).map_err(CompileError::Startup)?)
}

pub fn compile_sources_checked(
    sources: &[OwnedSource],
    opts: &CompileOpts,
) -> Result<CompileOut, CompileError> {
    checked(compile_sources(sources, opts).map_err(CompileError::Startup)?)
}

fn checked(out: CompileOut) -> Result<CompileOut, CompileError> {
    if out.ok() {
        Ok(out)
    } else {
        Err(CompileError::FrontendDiagnostics(out.diagnostics))
    }
}

pub fn parse_only(file: &str, defines: &[String]) -> Result<ParseOnlyOut, StartupError> {
    let limits = Limits::default();
    let content_limit = limits
        .max_source_bytes
        .checked_sub(file.len() as u64)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source path exceeds the configured Slang byte limit",
            )
        })?;
    let text = read_bounded(file, content_limit)?;
    parse_source(file, &text, defines)
}

/// Compile one supplied buffer in isolation without a filesystem read.
pub fn parse_source(
    name: &str,
    text: &str,
    defines: &[String],
) -> Result<ParseOnlyOut, StartupError> {
    let limits = Limits::default();
    let source_bytes = (name.len() as u64)
        .checked_add(text.len() as u64)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source byte count overflow",
            )
        })?;
    let define_bytes = defines
        .iter()
        .try_fold(0_u64, |total, define| {
            total.checked_add(define.len() as u64)
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "define byte count overflow",
            )
        })?;
    if source_bytes > limits.max_source_bytes
        || defines.len() > 4_096
        || define_bytes > 4 * 1024 * 1024
    {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "isolated source or defines exceed the Slang ABI limit",
        ));
    }
    let sources = vec![OwnedSource::compilation_unit(name, text)];
    let opts = CompileOpts {
        defines: defines.to_vec(),
        limits,
        ..CompileOpts::default()
    };
    let out = compile_sources(&sources, &opts)?;
    let source_texts = [(name, text)];
    let tokens = crate::core::tokens::from_slang_snapshot(&out.snapshot, &source_texts);
    let parsed_token_count = tokens.iter().map(|file| file.nodes.len()).sum();
    Ok(ParseOnlyOut {
        diagnostics: out.diagnostics,
        tokens,
        parsed_token_count,
        supplemented_token_count: 0,
    })
}

fn parse_define(value: &str) -> Define {
    let (name, value) = value
        .split_once('=')
        .map_or((value, None), |(name, value)| {
            (name, Some(value.to_owned()))
        });
    Define {
        name: name.to_owned(),
        value,
    }
}

fn parse_override(value: &str) -> Result<ParameterOverride, StartupError> {
    let Some((name, value)) = value.split_once('=') else {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("parameter override must have NAME=VALUE form: {value:?}"),
        ));
    };
    if name.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "parameter override name cannot be empty",
        ));
    }
    Ok(ParameterOverride {
        name: name.to_owned(),
        value: value.to_owned(),
    })
}

fn startup_from_slang(error: slang::SlangError) -> StartupError {
    use slang::SlangErrorKind;
    let kind = match error.kind() {
        SlangErrorKind::InvalidArgument | SlangErrorKind::LimitExceeded => {
            StartupErrorKind::InvalidArgument
        }
        SlangErrorKind::Frontend => StartupErrorKind::Frontend,
        SlangErrorKind::Internal | SlangErrorKind::InvalidNativeData => StartupErrorKind::Internal,
    };
    StartupError::new(kind, error.to_string())
}

fn project_diagnostics(snapshot: &Snapshot) -> Vec<Diag> {
    let files: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.id, file.name.as_str()))
        .collect();
    let texts: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|source| (source.name.as_str(), source.text.as_str()))
        .collect();
    snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity != DiagnosticSeverity::Ignored)
        .map(|diagnostic| {
            let (file, line, col) = diagnostic.primary.map_or((None, 0, 0), |range| {
                let file = files.get(&range.file_id).copied();
                let (line, col) = file
                    .and_then(|file| texts.get(file).copied())
                    .map(|text| one_based_utf16_position(text, range.start))
                    .unwrap_or((0, 0));
                (file.map(str::to_owned), line, col)
            });
            let severity = match diagnostic.severity {
                DiagnosticSeverity::Ignored => Severity::Info,
                DiagnosticSeverity::Note => Severity::Note,
                DiagnosticSeverity::Warning => Severity::Warning,
                DiagnosticSeverity::Error
                    if matches!(
                        diagnostic.subsystem,
                        DiagnosticSubsystem::Lexer
                            | DiagnosticSubsystem::Numeric
                            | DiagnosticSubsystem::Preprocessor
                            | DiagnosticSubsystem::Parser
                    ) =>
                {
                    Severity::Syntax
                }
                DiagnosticSeverity::Error => Severity::Error,
                DiagnosticSeverity::Fatal => Severity::Fatal,
            };
            Diag {
                severity,
                file,
                line,
                col,
                message: diagnostic.message.clone(),
            }
        })
        .collect()
}

fn one_based_utf16_position(text: &str, offset: u64) -> (u32, u32) {
    let requested = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(text.len());
    let mut end = requested;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut line = 1_u32;
    let mut column = 1_u32;
    let mut chars = text[..end].chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                line = line.saturating_add(1);
                column = 1;
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            '\n' => {
                line = line.saturating_add(1);
                column = 1;
            }
            _ => column = column.saturating_add(character.len_utf16() as u32),
        }
    }
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_include_scan_ignores_comments_and_strings() {
        let source = r#"
            // `include "commented.svh"
            /* `include "blocked.svh" */
            string message = "`include \"ordinary.svh\"";
            `include "first.svh"
            `include_next "not-an-include.svh"
            `include "second.svh"
        "#;
        assert_eq!(
            literal_includes(source),
            vec!["first.svh".to_owned(), "second.svh".to_owned()]
        );
    }

    #[test]
    fn utf16_positions_handle_crlf_and_non_ascii_text() {
        let text = "a😀\r\nb";
        assert_eq!(one_based_utf16_position(text, "a😀".len() as u64), (1, 4));
        assert_eq!(
            one_based_utf16_position(text, "a😀\r\n".len() as u64),
            (2, 1)
        );
    }
}
