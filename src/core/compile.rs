//! Slang-only compilation facade shared by the language server and simulator.
//!
//! The native compiler receives in-memory source buffers and returns an owned
//! snapshot. No native Slang object escapes the FFI call.

use crate::core::tokens::FileTokens;
use crate::ffi::slang::{self, CompileOptions, CompileRequest, Define, Limits, ParameterOverride};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

pub use crate::ffi::slang::{
    CompilationUnitMode, Diagnostic as SlangDiagnostic, DiagnosticProvider, DiagnosticSeverity,
    DiagnosticSubsystem, LanguageEdition, Snapshot, Source, SourceRange,
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
    /// Complete language policy for the compilation. The default is
    /// SystemVerilog-2009; `` `begin_keywords `` remains lexical only.
    pub edition: LanguageEdition,
    /// Definitions in `NAME` or `NAME=VALUE` form.
    pub defines: Vec<String>,
    /// Top-level overrides in `NAME=VALUE` form.
    pub param_overrides: Vec<String>,
    /// Logical search prefixes. In-memory callers must also supply include
    /// contents in [`sources`](Self::sources); path mode admits literal and
    /// bounded macro-expanded includes through Rust reads before entering
    /// cache-only Slang.
    pub include_dirs: Vec<String>,
    /// Check every definition as an uninstantiated library unit instead of
    /// recursively elaborating inferred top-level designs.
    pub library_units: bool,
    /// Select whether admitted compilation-unit buffers share preprocessing and
    /// `$unit` scope. The default keeps each buffer independent.
    pub compilation_unit_mode: CompilationUnitMode,
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
    LimitExceeded,
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
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let text = read_bounded(&name, content_limit)?;
        remaining = remaining.saturating_sub(name.len() as u64 + text.len() as u64);
        owned.push(OwnedSource::compilation_unit(name, text));
    }
    let root_count = owned.len();
    let mut macros = macro_environment_from_defines(&opts.defines);
    for root_index in 0..root_count {
        if matches!(opts.compilation_unit_mode, CompilationUnitMode::Separate) {
            macros = macro_environment_from_defines(&opts.defines);
        }
        let root = owned[root_index].clone();
        let mut include_stack = vec![PathBuf::from(&root.name)];
        admit_macro_includes(
            &root.name,
            &root.text,
            opts,
            &mut macros,
            &mut identities,
            &mut owned,
            &mut remaining,
            &mut include_stack,
            0,
        )?;
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
        library_units: opts.library_units,
        compilation_unit_mode: opts.compilation_unit_mode,
        edition: opts.edition,
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
            StartupErrorKind::LimitExceeded,
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
            StartupErrorKind::LimitExceeded,
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
    let mut roots = Vec::with_capacity(include_dirs.len().saturating_add(1));
    if let Some(parent) = including.parent() {
        roots.push(parent.to_path_buf());
    }
    roots.extend(include_dirs.iter().map(|dir| PathBuf::from(dir)));
    let roots: Vec<_> = roots
        .into_iter()
        .filter_map(|root| root.canonicalize().ok())
        .filter(|root| root.is_dir())
        .collect();
    if roots.is_empty() {
        return None;
    }
    let candidates = if target.is_absolute() {
        roots
            .iter()
            .map(|root| (target.to_path_buf(), root))
            .collect::<Vec<_>>()
    } else {
        roots
            .iter()
            .map(|root| (root.join(target), root))
            .collect::<Vec<_>>()
    };
    candidates.into_iter().find_map(|(candidate, root)| {
        let canonical = candidate.canonicalize().ok()?;
        if canonical.is_file() && canonical.starts_with(root) {
            Some(canonical)
        } else {
            None
        }
    })
}

const MAX_INCLUDE_DISCOVERY_DEPTH: usize = 256;
const MAX_MACRO_EXPANSION_DEPTH: usize = 64;

#[derive(Clone, Debug)]
struct MacroDefinition {
    parameters: Option<Vec<String>>,
    body: String,
}

type MacroEnvironment = HashMap<String, MacroDefinition>;

#[derive(Clone, Copy, Debug)]
struct ConditionalFrame {
    parent_active: bool,
    branch_taken: bool,
    active: bool,
}

fn macro_environment_from_defines(defines: &[String]) -> MacroEnvironment {
    defines
        .iter()
        .map(|value| {
            let define = parse_define(value);
            (
                define.name,
                MacroDefinition {
                    parameters: None,
                    body: define.value.unwrap_or_default(),
                },
            )
        })
        .collect()
}

fn admit_macro_includes(
    name: &str,
    text: &str,
    opts: &CompileOpts,
    macros: &mut MacroEnvironment,
    identities: &mut HashSet<PathBuf>,
    owned: &mut Vec<OwnedSource>,
    remaining: &mut u64,
    include_stack: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<(), StartupError> {
    if depth > MAX_INCLUDE_DISCOVERY_DEPTH {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "include expansion depth exceeds the configured admission limit",
        ));
    }
    let including = Path::new(name);
    let mut admit = |target: String, macros: &mut MacroEnvironment| -> Result<(), StartupError> {
        let Some(path) = resolve_include(including, &target, &opts.include_dirs) else {
            // Leave missing, malformed, and unauthorized targets for Slang so
            // its diagnostic retains the original directive and source range.
            return Ok(());
        };
        let path_name = path.to_string_lossy().into_owned();
        if !identities.contains(&path) {
            if opts.sources.len().saturating_add(owned.len())
                >= usize::try_from(opts.limits.max_sources).unwrap_or(usize::MAX)
            {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    "include graph exceeds the configured Slang source limit",
                ));
            }
            let content_limit = remaining
                .checked_sub(path_name.len() as u64)
                .ok_or_else(|| {
                    StartupError::new(
                        StartupErrorKind::LimitExceeded,
                        format!("include path {path_name} exceeds the configured Slang byte limit"),
                    )
                })?;
            let child_text = read_bounded(&path_name, content_limit)?;
            *remaining = remaining.saturating_sub(path_name.len() as u64 + child_text.len() as u64);
            identities.insert(path.clone());
            owned.push(OwnedSource::include(path_name.clone(), child_text));
        }

        // A repeated include still executes its directives, but a cycle must
        // stop admission recursion and remain visible to Slang's diagnostics.
        if include_stack.iter().any(|entry| entry == &path) {
            return Ok(());
        }
        let Some(child) = owned
            .iter()
            .find(|source| source.name == path_name)
            .cloned()
        else {
            return Ok(());
        };
        include_stack.push(path.clone());
        admit_macro_includes(
            &child.name,
            &child.text,
            opts,
            macros,
            identities,
            owned,
            remaining,
            include_stack,
            depth + 1,
        )?;
        include_stack.pop();
        Ok(())
    };
    scan_preprocessor_includes(text, macros, &mut admit)
}

/// Extract literal and macro-expanded includes without treating comments or
/// ordinary strings as directives. Macro expansion is only used to discover
/// bounded, authorized files; Slang remains the source of preprocessing
/// diagnostics and semantic macro identity.
fn literal_includes(source: &str) -> Vec<String> {
    let mut macros = MacroEnvironment::new();
    preprocessor_includes(source, &mut macros)
}

fn preprocessor_includes(source: &str, macros: &mut MacroEnvironment) -> Vec<String> {
    let mut includes = Vec::new();
    let mut collect = |target: String, _macros: &mut MacroEnvironment| {
        includes.push(target);
        Ok(())
    };
    scan_preprocessor_includes(source, macros, &mut collect)
        .expect("include collection cannot fail");
    includes
}

fn scan_preprocessor_includes<F>(
    source: &str,
    macros: &mut MacroEnvironment,
    on_include: &mut F,
) -> Result<(), StartupError>
where
    F: FnMut(String, &mut MacroEnvironment) -> Result<(), StartupError>,
{
    let mut conditions = Vec::new();
    let mut block_comment = false;
    for raw_line in logical_preprocessor_lines(source) {
        let line = strip_preprocessor_comments(&raw_line, &mut block_comment);
        let Some((directive, arguments)) = preprocessor_directive(&line) else {
            continue;
        };
        match directive {
            "ifdef" | "ifndef" => {
                let parent_active = conditions
                    .last()
                    .map_or(true, |frame: &ConditionalFrame| frame.active);
                let name = first_macro_identifier(arguments);
                let defined = name.is_some_and(|name| macros.contains_key(name));
                let condition = if directive == "ifdef" {
                    defined
                } else {
                    !defined
                };
                conditions.push(ConditionalFrame {
                    parent_active,
                    branch_taken: parent_active && condition,
                    active: parent_active && condition,
                });
            }
            "elsif" => {
                if let Some(frame) = conditions.last_mut() {
                    if !frame.parent_active || frame.branch_taken {
                        frame.active = false;
                    } else {
                        let condition = first_macro_identifier(arguments)
                            .is_some_and(|name| macros.contains_key(name));
                        frame.active = condition;
                        frame.branch_taken = condition;
                    }
                }
            }
            "else" => {
                if let Some(frame) = conditions.last_mut() {
                    frame.active = frame.parent_active && !frame.branch_taken;
                    frame.branch_taken = true;
                }
            }
            "endif" => {
                conditions.pop();
            }
            _ if !conditions.last().map_or(true, |frame| frame.active) => {}
            "define" => define_macro(arguments, macros),
            "undef" => {
                if let Some(name) = first_macro_identifier(arguments) {
                    macros.remove(name);
                }
            }
            "undefineall" => macros.clear(),
            "include" => {
                if let Some(target) = include_target(arguments, macros) {
                    on_include(target, macros)?;
                }
            }
            _ => {}
        }
        if !matches!(
            directive,
            "ifdef"
                | "ifndef"
                | "elsif"
                | "else"
                | "endif"
                | "define"
                | "undef"
                | "undefineall"
                | "include"
        ) && conditions.last().map_or(true, |frame| frame.active)
        {
            if let Some(target) = expanded_include_target(&line, macros) {
                on_include(target, macros)?;
            }
        }
    }
    Ok(())
}

fn logical_preprocessor_lines(source: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw in source.split_inclusive('\n') {
        let mut line = raw.strip_suffix('\n').unwrap_or(raw);
        if line.ends_with('\r') {
            line = &line[..line.len() - 1];
        }
        if line.ends_with('\\') {
            current.push_str(&line[..line.len() - 1]);
        } else {
            current.push_str(line);
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn strip_preprocessor_comments(line: &str, block_comment: &mut bool) -> String {
    let mut result = String::with_capacity(line.len());
    let mut string = false;
    let mut escaped = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if *block_comment {
            if character == '*' && characters.peek() == Some(&'/') {
                characters.next();
                *block_comment = false;
            }
            continue;
        }
        if !string && character == '/' {
            match characters.peek().copied() {
                Some('/') => break,
                Some('*') => {
                    characters.next();
                    *block_comment = true;
                    continue;
                }
                _ => {}
            }
        }
        result.push(character);
        if character == '"' && !escaped {
            string = !string;
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    result
}

fn preprocessor_directive(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut string = false;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' && (index == 0 || bytes[index - 1] != b'\\') {
            string = !string;
        } else if !string && bytes[index] == b'`' {
            break;
        }
        index += 1;
    }
    if index == bytes.len() {
        return None;
    }
    let rest = &line[index + 1..];
    let end = rest
        .find(|character: char| !is_macro_identifier_continue(character))
        .unwrap_or(rest.len());
    Some((&rest[..end], rest[end..].trim_start()))
}

fn is_macro_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_' || character == '$'
}

fn is_macro_identifier_continue(character: char) -> bool {
    is_macro_identifier_start(character) || character.is_ascii_digit()
}

fn first_macro_identifier(text: &str) -> Option<&str> {
    let start = text
        .char_indices()
        .find(|(_, character)| is_macro_identifier_start(*character))
        .map(|(index, _)| index)?;
    let end = text[start..]
        .char_indices()
        .find(|(_, character)| !is_macro_identifier_continue(*character))
        .map_or(text.len(), |(index, _)| start + index);
    Some(&text[start..end])
}

fn define_macro(arguments: &str, macros: &mut MacroEnvironment) {
    let Some(name) = first_macro_identifier(arguments) else {
        return;
    };
    let name_start = name.as_ptr() as usize - arguments.as_ptr() as usize;
    let name_end = name_start + name.len();
    let mut cursor = name_end;
    let bytes = arguments.as_bytes();
    if cursor < bytes.len() && bytes[cursor] == b'(' {
        let Some((parameters, body_start)) = macro_parameters(arguments, cursor) else {
            return;
        };
        macros.insert(
            name.to_owned(),
            MacroDefinition {
                parameters: Some(parameters),
                body: arguments[body_start..].trim_start().to_owned(),
            },
        );
    } else {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        macros.insert(
            name.to_owned(),
            MacroDefinition {
                parameters: None,
                body: arguments[cursor..].to_owned(),
            },
        );
    }
}

fn macro_parameters(text: &str, open: usize) -> Option<(Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = open + 1;
    let mut parameters = Vec::new();
    loop {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return None;
        }
        if bytes[cursor] == b')' {
            return Some((parameters, cursor + 1));
        }
        let start = cursor;
        while cursor < bytes.len() && is_macro_identifier_continue(bytes[cursor] as char) {
            cursor += 1;
        }
        if start == cursor {
            return None;
        }
        parameters.push(text[start..cursor].to_owned());
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor] == b',' {
            cursor += 1;
            continue;
        }
        if cursor < bytes.len() && bytes[cursor] == b')' {
            return Some((parameters, cursor + 1));
        }
        return None;
    }
}

fn include_target(arguments: &str, macros: &MacroEnvironment) -> Option<String> {
    let expanded = expand_macros(arguments, macros);
    let trimmed = expanded.trim_start();
    let (closing, start) = match trimmed.as_bytes().first().copied()? {
        b'"' => (b'"', 1),
        b'<' => (b'>', 1),
        _ => return None,
    };
    let bytes = trimmed.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        if closing == b'"' && bytes[cursor] == b'\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if bytes[cursor] == closing {
            let raw = &trimmed[start..cursor];
            if closing == b'"' {
                return Some(unescape_include_name(raw));
            }
            return Some(raw.to_owned());
        }
        if bytes[cursor] == b'\n' || bytes[cursor] == b'\r' {
            return None;
        }
        cursor += 1;
    }
    None
}

fn expanded_include_target(line: &str, macros: &MacroEnvironment) -> Option<String> {
    let expanded = expand_macros(line, macros);
    let (directive, arguments) = preprocessor_directive(&expanded)?;
    (directive == "include").then(|| include_target(arguments, macros))?
}

fn unescape_include_name(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        output.push('\\');
    }
    output
}

fn expand_macros(text: &str, macros: &MacroEnvironment) -> String {
    let mut stack = Vec::new();
    expand_macro_text(text, macros, &mut stack, 0)
}

fn expand_macro_text(
    text: &str,
    macros: &MacroEnvironment,
    stack: &mut Vec<String>,
    depth: usize,
) -> String {
    if depth > MAX_MACRO_EXPANSION_DEPTH {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'`' && index + 1 < bytes.len() && bytes[index + 1] == b'"' {
            output.push('"');
            index += 2;
            continue;
        }
        if bytes[index] == b'`' && index + 1 < bytes.len() && bytes[index + 1] == b'`' {
            index += 2;
            continue;
        }
        let invoked = if bytes[index] == b'`'
            && index + 1 < bytes.len()
            && is_macro_identifier_start(bytes[index + 1] as char)
        {
            index += 1;
            true
        } else {
            false
        };
        let character = bytes[index] as char;
        if !is_macro_identifier_start(character) {
            output.push(character);
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() && is_macro_identifier_continue(bytes[index] as char) {
            index += 1;
        }
        let name = &text[start..index];
        if !invoked {
            output.push_str(name);
            continue;
        }
        let Some(definition) = macros.get(name) else {
            output.push('`');
            output.push_str(name);
            continue;
        };
        if stack.iter().any(|active| active == name) {
            output.push('`');
            output.push_str(name);
            continue;
        }
        let Some(parameters) = definition.parameters.as_ref() else {
            stack.push(name.to_owned());
            output.push_str(&expand_macro_text(
                &definition.body,
                macros,
                stack,
                depth + 1,
            ));
            stack.pop();
            continue;
        };
        if index >= bytes.len() || bytes[index] != b'(' {
            output.push('`');
            output.push_str(name);
            continue;
        }
        let Some((arguments, end)) = macro_call_arguments(text, index) else {
            output.push('`');
            output.push_str(name);
            continue;
        };
        if arguments.len() != parameters.len() {
            output.push('`');
            output.push_str(name);
            continue;
        }
        let expanded_arguments: Vec<_> = arguments
            .iter()
            .map(|argument| expand_macro_text(argument, macros, stack, depth + 1))
            .collect();
        let substituted =
            substitute_macro_arguments(&definition.body, parameters, &expanded_arguments);
        stack.push(name.to_owned());
        output.push_str(&expand_macro_text(&substituted, macros, stack, depth + 1));
        stack.pop();
        index = end;
    }
    output
}

fn macro_call_arguments(text: &str, open: usize) -> Option<(Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = open + 1;
    let mut depth = 1_u32;
    let mut start = cursor;
    let mut arguments = Vec::new();
    let mut string = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if string {
            if byte == b'"' && bytes.get(cursor.wrapping_sub(1)) != Some(&b'\\') {
                string = false;
            }
        } else if byte == b'"' {
            string = true;
        } else if byte == b'(' {
            depth += 1;
        } else if byte == b')' {
            depth -= 1;
            if depth == 0 {
                if cursor > start || !arguments.is_empty() {
                    arguments.push(text[start..cursor].to_owned());
                }
                return Some((arguments, cursor + 1));
            }
        } else if byte == b',' && depth == 1 {
            arguments.push(text[start..cursor].to_owned());
            start = cursor + 1;
        }
        cursor += 1;
    }
    None
}

fn substitute_macro_arguments(text: &str, parameters: &[String], arguments: &[String]) -> String {
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        let character = bytes[index] as char;
        if !is_macro_identifier_start(character) {
            output.push(character);
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() && is_macro_identifier_continue(bytes[index] as char) {
            index += 1;
        }
        let name = &text[start..index];
        if let Some(parameter) = parameters.iter().position(|parameter| parameter == name) {
            output.push_str(&arguments[parameter]);
        } else {
            output.push_str(name);
        }
    }
    output
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
                StartupErrorKind::LimitExceeded,
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
    if source_bytes > limits.max_source_bytes {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "isolated source bytes exceed the configured Slang limit",
        ));
    }
    if defines.len() > 4_096 || define_bytes > 4 * 1024 * 1024 {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "isolated defines exceed the Slang ABI limit",
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
        SlangErrorKind::InvalidArgument => StartupErrorKind::InvalidArgument,
        SlangErrorKind::LimitExceeded => StartupErrorKind::LimitExceeded,
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
    fn macro_include_scan_expands_nested_and_function_like_names() {
        let source = r#"
            `define QUOTE(name) `"name`"
            `define LEAF nested.svh
            `define HEADER `QUOTE(`LEAF)
            `ifdef ENABLE_HEADER
              `include `HEADER
            `else
              `include "disabled.svh"
            `endif
        "#;
        let mut macros = macro_environment_from_defines(&["ENABLE_HEADER".to_owned()]);
        assert_eq!(
            preprocessor_includes(source, &mut macros),
            vec!["nested.svh".to_owned()]
        );
    }

    #[test]
    fn macro_include_scan_tracks_redefinitions_and_ignores_inactive_branches() {
        let source = r#"
            `define HEADER "first.svh"
            `ifdef USE_SECOND
              `undef HEADER
              `define HEADER "second.svh"
            `endif
            `include `HEADER
        "#;
        let mut macros = macro_environment_from_defines(&["USE_SECOND".to_owned()]);
        assert_eq!(
            preprocessor_includes(source, &mut macros),
            vec!["second.svh".to_owned()]
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
