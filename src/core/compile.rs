//! Slang-only compilation facade shared by the language server and simulator.
//!
//! The native compiler receives in-memory source buffers and returns an owned
//! snapshot. No native Slang object escapes the FFI call.

use crate::core::tokens::FileTokens;
use crate::ffi::secure_fs::{self, AdmittedTarget, FileIdentity, OpenedPath};
use crate::ffi::slang::{self, CompileOptions, CompileRequest, Define, Limits, ParameterOverride};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub use crate::ffi::slang::{
    CompilationUnitMode, Diagnostic as SlangDiagnostic, DiagnosticProvider, DiagnosticSeverity,
    DiagnosticSubsystem, LanguageEdition, Snapshot, Source, SourceRange,
};
mod editions;
mod library_configs;
mod library_mapping;
use editions::edition_diagnostics;
use library_mapping::{library_match_pattern, LibraryMapBuffers, LibrarySpecificity};

/// A source buffer owned by a compile request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSource {
    pub name: String,
    pub text: String,
    pub is_compilation_unit: bool,
    /// Parse an admitted map with Slang's library-map grammar.
    pub is_library_map: bool,
}

impl OwnedSource {
    pub fn compilation_unit(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            is_compilation_unit: true,
            is_library_map: false,
        }
    }

    pub fn include(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            is_compilation_unit: false,
            is_library_map: false,
        }
    }
}

/// An admitted source file that belongs to a named Verilog source library.
/// The source is copied into the native cache before elaboration; the library
/// name never authorizes a native filesystem read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySource {
    pub name: String,
    pub text: String,
    pub library: String,
    /// Parse this library source as a library map.
    pub is_library_map: bool,
}

/// An ordered include search directory for one source library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryIncludeDir {
    pub library: String,
    pub path: String,
}

impl LibrarySource {
    pub fn new(
        name: impl Into<String>,
        text: impl Into<String>,
        library: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            library: library.into(),
            is_library_map: false,
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
    /// Standard SystemVerilog prototypes for user-defined `$` tasks/functions
    /// made available to the elaborator and simulator plugin bridge.
    pub system_subroutines: Vec<String>,
    /// Check every definition as an uninstantiated library unit instead of
    /// recursively elaborating inferred top-level designs.
    pub library_units: bool,
    /// Library map files are admitted and expanded by Rust before Slang.
    pub library_map_files: Vec<String>,
    /// Already-admitted library map buffers. Their names provide the relative
    /// base for paths in `include` and `library` clauses.
    pub library_maps: Vec<OwnedSource>,
    /// Explicit library files, in `library=path` or `path` form.
    pub library_files: Vec<String>,
    /// Already-admitted named library sources.
    pub library_sources: Vec<LibrarySource>,
    /// Include directories from admitted library maps, scoped to their library.
    pub library_include_dirs: Vec<LibraryIncludeDir>,
    /// Default liblist search order used when a configuration has no local
    /// `liblist` clause.
    pub library_order: Vec<String>,
    /// Name of the default source library; defaults to `work`.
    pub default_library: Option<String>,
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
    /// Set when the owned edition profile rejected a form that the newer
    /// Slang grammar accepted. Such a rejection is not visible in
    /// `snapshot.has_errors()`, so it must be carried explicitly.
    owned_errors: bool,
}

// Keep path-based admission below the native bridge's hard source-byte cap.
// The C++ shim clamps its effective limit to this value; doing the same before
// opening a path prevents Rust from reading a larger caller-requested budget
// that the bridge can never accept.
const NATIVE_HARD_MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const NATIVE_HARD_MAX_SOURCES: u64 = 4_096;
const MAX_LIBRARY_MAP_WORK: u64 = 1_000_000;
const MAX_LIBRARY_PATTERN_COMPONENTS: usize = 512;
const MAX_LIBRARY_INCLUDE_DIRS: usize = 4_096;
const MAX_LIBRARY_PATH_BYTES: usize = 16 * 1024;
const LIBRARY_MAP_READ_CHUNK_BYTES: usize = 8 * 1024;
// Regular files normally satisfy one read per chunk. Keep a separate I/O
// iteration ceiling so a filesystem that returns one byte at a time cannot
// turn a bounded source into unbounded admission work while retaining the
// normal 128 MiB source limit.
const MAX_REGULAR_FILE_READ_ITERATIONS: u64 = 1_000_000;

#[derive(Debug)]
struct LibraryMapWorkBudget {
    used: u64,
    limit: u64,
    allocation_used: u64,
    allocation_limit: u64,
    path_resolution_reserved: bool,
}

impl LibraryMapWorkBudget {
    #[cfg(test)]
    fn new(limit: u64) -> Self {
        Self::with_allocation_limit(limit, limit)
    }

    fn with_allocation_limit(limit: u64, allocation_limit: u64) -> Self {
        Self {
            used: 0,
            limit,
            allocation_used: 0,
            allocation_limit,
            path_resolution_reserved: false,
        }
    }

    fn charge(&mut self, amount: u64, operation: &str) -> Result<(), StartupError> {
        let next = self
            .used
            .checked_add(amount)
            .ok_or_else(|| self.limit_error(operation))?;
        if next > self.limit {
            return Err(self.limit_error(operation));
        }
        self.used = next;
        Ok(())
    }

    fn charge_usize(&mut self, amount: usize, operation: &str) -> Result<(), StartupError> {
        let amount = u64::try_from(amount).map_err(|_| self.limit_error(operation))?;
        self.charge(amount, operation)
    }

    fn charge_allocation(&mut self, amount: u64, operation: &str) -> Result<(), StartupError> {
        let next = self
            .allocation_used
            .checked_add(amount)
            .ok_or_else(|| self.allocation_limit_error(operation))?;
        if next > self.allocation_limit {
            return Err(self.allocation_limit_error(operation));
        }
        self.allocation_used = next;
        Ok(())
    }

    fn charge_allocation_usize(
        &mut self,
        amount: usize,
        operation: &str,
    ) -> Result<(), StartupError> {
        let amount = u64::try_from(amount).map_err(|_| self.allocation_limit_error(operation))?;
        self.charge_allocation(amount, operation)
    }

    fn limit_error(&self, operation: &str) -> StartupError {
        StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map {operation} work budget exceeded after {} operations (limit {}); simplify wildcard patterns or reduce library-map candidates",
                self.used, self.limit
            ),
        )
    }

    fn allocation_limit_error(&self, operation: &str) -> StartupError {
        StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map {operation} allocation budget exceeded after {} bytes (limit {}); reduce library-map input size",
                self.allocation_used, self.allocation_limit
            ),
        )
    }
}

fn charge_library_map_text_allocation(
    work: &mut LibraryMapWorkBudget,
    text: &str,
    operation: &str,
) -> Result<(), StartupError> {
    work.charge_allocation_usize(text.len(), operation)
}

fn checked_library_path_bytes(path: &Path, operation: &str) -> Result<usize, StartupError> {
    let length = path.to_string_lossy().len();
    if length > MAX_LIBRARY_PATH_BYTES {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map {operation} path is {length} bytes; the limit is {MAX_LIBRARY_PATH_BYTES}; simplify the path"
            ),
        ));
    }
    Ok(length)
}

fn charge_library_path_bytes(
    work: &mut LibraryMapWorkBudget,
    path: &Path,
    operation: &str,
) -> Result<(), StartupError> {
    let length = checked_library_path_bytes(path, operation)?;
    work.charge_usize(length, operation)
}

fn charge_library_path_join(
    work: &mut LibraryMapWorkBudget,
    parent: &Path,
    child: &std::ffi::OsStr,
    operation: &str,
) -> Result<(), StartupError> {
    let parent_length = checked_library_path_bytes(parent, operation)?;
    let child_length = child.to_string_lossy().len();
    let length = parent_length
        .checked_add(1)
        .and_then(|length| length.checked_add(child_length))
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("library map {operation} path length overflows the configured limit"),
            )
        })?;
    if length > MAX_LIBRARY_PATH_BYTES {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map {operation} path is {length} bytes; the limit is {MAX_LIBRARY_PATH_BYTES}; simplify the path"
            ),
        ));
    }
    work.charge_usize(length, operation)
}

fn canonicalize_library_map_path(
    work: &mut LibraryMapWorkBudget,
    path: &Path,
    operation: &str,
) -> Result<AdmittedTarget, StartupError> {
    checked_library_path_bytes(path, operation)?;
    work.charge(1, operation)?;
    if work.path_resolution_reserved {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!("library map {operation} path resolution is already active"),
        ));
    }

    // Keep a transient reservation while the handle-derived target is
    // obtained; this keeps the reservation separate from cumulative map work
    // so every ordinary match does not spend MAX_LIBRARY_PATH_BYTES.
    work.path_resolution_reserved = true;
    let result = secure_fs::open_path(path).map(|opened| opened.admitted_target());
    work.path_resolution_reserved = false;
    let canonical = result.map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!(
                "cannot resolve library map path {}: {error}",
                path.display()
            ),
        )
    })?;
    charge_library_path_bytes(work, canonical.actual_path(), operation)?;
    Ok(canonical)
}

fn charge_key_comparison_work<I>(
    work: &mut LibraryMapWorkBudget,
    key_lengths: I,
    sort: bool,
    operation: &str,
) -> Result<(), StartupError>
where
    I: IntoIterator<Item = usize>,
{
    let mut count = 0_u64;
    let mut max_key_length = 0_u64;
    for key_length in key_lengths {
        work.charge(1, "library map ordering key scan")?;
        let key_length = u64::try_from(key_length)
            .map_err(|_| work.limit_error("library map ordering key scan"))?;
        work.charge(key_length, "library map ordering key scan")?;
        count = count
            .checked_add(1)
            .ok_or_else(|| work.limit_error("library map ordering key scan"))?;
        max_key_length = max_key_length.max(key_length);
    }

    let comparisons = if sort && count > 1 {
        let count_usize = usize::try_from(count)
            .map_err(|_| work.limit_error("library map ordering comparisons"))?;
        let levels = u64::from(usize::BITS - (count_usize - 1).leading_zeros());
        count
            .checked_mul(
                levels
                    .checked_add(1)
                    .ok_or_else(|| work.limit_error("library map ordering comparisons"))?,
            )
            .and_then(|value| value.checked_mul(2))
            .ok_or_else(|| work.limit_error("library map ordering comparisons"))?
    } else {
        count.saturating_sub(1)
    };
    let comparison_bytes = comparisons
        .checked_mul(
            max_key_length
                .checked_add(1)
                .ok_or_else(|| work.limit_error(operation))?,
        )
        .ok_or_else(|| work.limit_error(operation))?;
    work.charge(comparison_bytes, operation)
}

fn effective_source_byte_limit(limits: Limits) -> u64 {
    limits.max_source_bytes.min(NATIVE_HARD_MAX_SOURCE_BYTES)
}

fn effective_source_count_limit(limits: Limits) -> usize {
    usize::try_from(limits.max_sources.min(NATIVE_HARD_MAX_SOURCES)).unwrap_or(usize::MAX)
}

impl CompileOut {
    pub fn ok(&self) -> bool {
        !self.snapshot.has_errors() && !self.owned_errors
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
    preflight_library_sources(&opts.library_sources, opts.limits)?;
    let mut source_count = opts
        .sources
        .len()
        .checked_add(opts.library_sources.len())
        .and_then(|count| count.checked_add(opts.library_maps.len()))
        .and_then(|count| count.checked_add(opts.files.len()))
        .and_then(|count| count.checked_add(opts.library_files.len()))
        .ok_or_else(|| {
            StartupError::new(StartupErrorKind::InvalidArgument, "source count overflow")
        })?;
    if source_count > effective_source_count_limit(opts.limits) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    let mut owned = Vec::new();
    let mut library_owned = opts.library_sources.clone();
    let admitted_bytes = opts
        .sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)
        })
        .and_then(|total| {
            opts.library_sources
                .iter()
                .try_fold(total, |total, source| {
                    total
                        .checked_add(source.name.len() as u64)?
                        .checked_add(source.text.len() as u64)?
                        .checked_add(source.library.len() as u64)
                })
        })
        .and_then(|total| {
            opts.library_maps.iter().try_fold(total, |total, source| {
                total
                    .checked_add(source.name.len() as u64)?
                    .checked_add(source.text.len() as u64)
            })
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source byte count overflow",
            )
        })?;
    let mut remaining = effective_source_byte_limit(opts.limits)
        .checked_sub(admitted_bytes)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "admitted source bytes exceed the configured Slang limit",
            )
        })?;
    let mut identities = std::collections::HashSet::new();
    let mut admitted_targets = HashMap::new();
    for source in &opts.library_sources {
        if source.name.is_empty() || source.library.is_empty() {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                "library source names and library names must be nonempty",
            ));
        }
    }
    for path in &opts.files {
        let resolved = absolute_path(Path::new(path))?;
        let resolved_path = resolved.actual_path().to_path_buf();
        admitted_targets.insert(resolved_path.clone(), resolved.clone());
        if !identities.insert(resolved_path.clone()) {
            continue;
        }
        let name = resolved_path.to_string_lossy().into_owned();
        let content_limit = remaining.checked_sub(name.len() as u64).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let text = read_bounded_target(&name, &resolved, content_limit, "SystemVerilog source")?;
        remaining = remaining.saturating_sub(name.len() as u64 + text.len() as u64);
        owned.push(OwnedSource::compilation_unit(name, text));
    }
    for spec in &opts.library_files {
        let (library, path) = split_library_file_spec(spec, opts.default_library.as_deref())?;
        let resolved = absolute_path(Path::new(path))?;
        let resolved_path = resolved.actual_path().to_path_buf();
        admitted_targets.insert(resolved_path.clone(), resolved.clone());
        if !identities.insert(resolved_path.clone()) {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                format!(
                    "source is assigned more than once: {}",
                    resolved_path.display()
                ),
            ));
        }
        let name = resolved_path.to_string_lossy().into_owned();
        let name_bytes = u64::try_from(name.len()).map_err(|_| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let library_bytes = u64::try_from(library.len()).map_err(|_| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "library name exceeds the configured Slang byte limit",
            )
        })?;
        let metadata_bytes = name_bytes.checked_add(library_bytes).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "source metadata byte count overflow",
            )
        })?;
        let content_limit = remaining.checked_sub(metadata_bytes).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let text = read_bounded_target(&name, &resolved, content_limit, "SystemVerilog source")?;
        let text_bytes = u64::try_from(text.len()).map_err(|_| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        remaining = content_limit.checked_sub(text_bytes).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("source path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        library_owned.push(LibrarySource::new(name, text, library));
    }
    let mut library_include_dirs = opts.library_include_dirs.clone();
    let mut library_map_work = LibraryMapWorkBudget::with_allocation_limit(
        MAX_LIBRARY_MAP_WORK,
        effective_source_byte_limit(opts.limits),
    );
    let path_roots = owned;
    let mut owned = opts.sources.clone();
    owned.extend(path_roots.iter().cloned());
    let map_originals = if !opts.library_map_files.is_empty() || !opts.library_maps.is_empty() {
        let mut buffers =
            LibraryMapBuffers::new(&mut owned, &mut library_owned, &mut library_map_work)?;
        admit_library_maps_with_targets(
            opts,
            &mut library_include_dirs,
            &mut identities,
            &mut buffers,
            &mut source_count,
            &mut remaining,
            &mut library_map_work,
            &mut admitted_targets,
        )?;
        collect_in_memory_library_maps(
            &opts.library_maps,
            &opts.defines,
            opts.edition,
            &mut library_include_dirs,
            &mut buffers,
            source_count,
            effective_source_count_limit(opts.limits),
            &mut library_map_work,
        )?;
        buffers.finish(&mut remaining, &mut library_map_work)?
    } else {
        Vec::new()
    };
    let mut macros = macro_environment_from_defines(&opts.defines);
    let expansion_budget = MacroExpansionBudget::new(effective_source_byte_limit(opts.limits));
    for path_root in path_roots {
        let Some(root) = owned.iter().find(|source| *source == &path_root).cloned() else {
            continue;
        };
        if matches!(opts.compilation_unit_mode, CompilationUnitMode::Separate) {
            macros = macro_environment_from_defines(&opts.defines);
        }
        let root_target = admitted_targets.get(Path::new(&root.name)).cloned();
        let mut include_stack = vec![PathBuf::from(&root.name)];
        admit_macro_includes(
            &root.name,
            &root.text,
            opts,
            &opts.include_dirs,
            &mut macros,
            root_target.as_ref(),
            &mut admitted_targets,
            &mut identities,
            &mut owned,
            &mut source_count,
            &mut remaining,
            &mut include_stack,
            &expansion_budget,
            0,
        )?;
    }
    for root in library_owned.clone() {
        let mut library_macros = macro_environment_from_defines(&opts.defines);
        let mut include_dirs = opts.include_dirs.clone();
        include_dirs.extend(
            library_include_dirs
                .iter()
                .filter(|dir| dir.library == root.library)
                .map(|dir| dir.path.clone()),
        );
        let root_target = admitted_targets.get(Path::new(&root.name)).cloned();
        let mut include_stack = vec![PathBuf::from(&root.name)];
        admit_macro_includes(
            &root.name,
            &root.text,
            opts,
            &include_dirs,
            &mut library_macros,
            root_target.as_ref(),
            &mut admitted_targets,
            &mut identities,
            &mut owned,
            &mut source_count,
            &mut remaining,
            &mut include_stack,
            &expansion_budget,
            0,
        )?;
    }
    if owned.len().saturating_add(library_owned.len()) > effective_source_count_limit(opts.limits) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    compile_source_groups(
        &owned,
        &[],
        &library_owned,
        &library_include_dirs,
        map_originals,
        &mut library_map_work,
        opts,
    )
}

/// Compile exact source buffers without reading the filesystem.
pub fn compile_sources(
    sources: &[OwnedSource],
    opts: &CompileOpts,
) -> Result<CompileOut, StartupError> {
    preflight_options(opts)?;
    preflight_sources(sources, opts.limits)?;
    preflight_library_maps(&opts.library_maps, opts.limits)?;
    preflight_library_sources(&opts.library_sources, opts.limits)?;
    let mut source_count = sources
        .len()
        .checked_add(opts.library_maps.len())
        .and_then(|count| count.checked_add(opts.library_sources.len()))
        .ok_or_else(|| {
            StartupError::new(StartupErrorKind::InvalidArgument, "source count overflow")
        })?;
    let source_limit = effective_source_count_limit(opts.limits);
    if source_count > source_limit {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "source count exceeds the configured Slang limit",
        ));
    }
    let admitted_bytes = sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)
        })
        .and_then(|total| {
            opts.library_maps.iter().try_fold(total, |total, source| {
                total
                    .checked_add(source.name.len() as u64)?
                    .checked_add(source.text.len() as u64)
            })
        })
        .and_then(|total| {
            opts.library_sources
                .iter()
                .try_fold(total, |total, source| {
                    total
                        .checked_add(source.name.len() as u64)?
                        .checked_add(source.text.len() as u64)?
                        .checked_add(source.library.len() as u64)
                })
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "source byte count overflow",
            )
        })?;
    let mut remaining = effective_source_byte_limit(opts.limits)
        .checked_sub(admitted_bytes)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "admitted source bytes exceed the configured Slang limit",
            )
        })?;
    let mut owned = sources.to_vec();
    let mut library_owned = opts.library_sources.clone();
    let mut library_include_dirs = opts.library_include_dirs.clone();
    let mut library_map_work = LibraryMapWorkBudget::with_allocation_limit(
        MAX_LIBRARY_MAP_WORK,
        effective_source_byte_limit(opts.limits),
    );
    let map_originals = admit_in_memory_library_maps_with_dirs(
        &opts.library_maps,
        &opts.defines,
        opts.edition,
        &mut library_include_dirs,
        &mut owned,
        &mut library_owned,
        &mut source_count,
        &mut remaining,
        source_limit,
        &mut library_map_work,
    )?;
    compile_source_groups(
        &owned,
        &[],
        &library_owned,
        &library_include_dirs,
        map_originals,
        &mut library_map_work,
        opts,
    )
}

fn preflight_options(opts: &CompileOpts) -> Result<(), StartupError> {
    const MAX_OPTIONS: usize = 4_096;
    const MAX_OPTION_BYTES: u64 = 4 * 1024 * 1024;
    if opts.defines.len() > MAX_OPTIONS
        || opts.param_overrides.len() > MAX_OPTIONS
        || opts.include_dirs.len() > MAX_OPTIONS
        || opts.library_include_dirs.len() > MAX_OPTIONS
        || opts.system_subroutines.len() > MAX_OPTIONS
        || opts.library_map_files.len() > MAX_OPTIONS
        || opts.library_maps.len() > MAX_OPTIONS
        || opts.library_files.len() > MAX_OPTIONS
        || opts.library_sources.len() > MAX_OPTIONS
        || opts.library_order.len() > MAX_OPTIONS
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
        .chain(opts.library_include_dirs.iter().map(|entry| &entry.library))
        .chain(opts.library_include_dirs.iter().map(|entry| &entry.path))
        .chain(&opts.system_subroutines)
        .chain(&opts.library_map_files)
        .chain(&opts.library_files)
        .chain(&opts.library_order)
        .map(|value| value.len() as u64)
        .chain(opts.top.iter().map(|value| value.len() as u64))
        .chain(opts.default_library.iter().map(|value| value.len() as u64))
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
    library_sources: &[LibrarySource],
    library_include_dirs: &[LibraryIncludeDir],
    map_originals: Vec<OwnedSource>,
    map_work: &mut LibraryMapWorkBudget,
    opts: &CompileOpts,
) -> Result<CompileOut, StartupError> {
    let borrowed: Vec<_> = first
        .iter()
        .chain(second)
        .map(|source| Source {
            name: &source.name,
            text: &source.text,
            is_compilation_unit: source.is_compilation_unit,
            is_library_map: source.is_library_map,
        })
        .collect();
    let borrowed_libraries: Vec<_> = library_sources
        .iter()
        .map(|source| crate::ffi::slang::LibrarySource {
            name: &source.name,
            text: &source.text,
            library: &source.library,
            is_library_map: source.is_library_map,
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
        library_include_dirs: library_include_dirs
            .iter()
            .map(|entry| crate::ffi::slang::LibraryIncludeDir {
                library: entry.library.clone(),
                path: entry.path.clone(),
            })
            .collect(),
        parameter_overrides: opts
            .param_overrides
            .iter()
            .map(|value| parse_override(value))
            .collect::<Result<_, _>>()?,
        system_subroutines: opts.system_subroutines.clone(),
        library_units: opts.library_units,
        library_order: opts.library_order.clone(),
        default_library: opts.default_library.clone(),
        compilation_unit_mode: opts.compilation_unit_mode,
        edition: opts.edition,
        limits: opts.limits,
    };
    let mut snapshot = slang::compile(&CompileRequest {
        sources: &borrowed,
        library_sources: &borrowed_libraries,
        options: &options,
    })
    .map_err(startup_from_slang)?;
    library_configs::restore_source_text(&mut snapshot.files, map_originals, map_work)?;
    let mut diagnostics = project_diagnostics(&snapshot);
    let edition_diagnostics = edition_diagnostics(
        &snapshot,
        opts.edition,
        &opts.system_subroutines,
        opts.compilation_unit_mode,
        &borrowed,
    );
    let owned_errors = edition_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    diagnostics.extend(edition_diagnostics);
    Ok(CompileOut {
        diagnostics,
        snapshot,
        owned_errors,
    })
}

fn preflight_sources(sources: &[OwnedSource], limits: Limits) -> Result<(), StartupError> {
    if sources.len() > effective_source_count_limit(limits) {
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
    if bytes > effective_source_byte_limit(limits) {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "source bytes exceed the configured Slang limit",
        ));
    }
    Ok(())
}

fn preflight_library_sources(
    sources: &[LibrarySource],
    limits: Limits,
) -> Result<(), StartupError> {
    if sources.len() > effective_source_count_limit(limits) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "library source count exceeds the configured Slang limit",
        ));
    }
    let bytes = sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)?
                .checked_add(source.library.len() as u64)
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "library source byte count overflow",
            )
        })?;
    if bytes > effective_source_byte_limit(limits) {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library source bytes exceed the configured Slang limit",
        ));
    }
    Ok(())
}

fn preflight_library_maps(maps: &[OwnedSource], limits: Limits) -> Result<(), StartupError> {
    if maps.len() > effective_source_count_limit(limits) {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "library map count exceeds the configured Slang limit",
        ));
    }
    let bytes = maps
        .iter()
        .try_fold(0_u64, |total, source| {
            total
                .checked_add(source.name.len() as u64)?
                .checked_add(source.text.len() as u64)
        })
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                "library map byte count overflow",
            )
        })?;
    if bytes > effective_source_byte_limit(limits) {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library map bytes exceed the configured Slang limit",
        ));
    }
    Ok(())
}

fn read_bounded(path: &str, limit: u64) -> Result<String, StartupError> {
    let expected = secure_fs::open_path(Path::new(path)).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot resolve SystemVerilog source {path}: {error}"),
        )
    })?;
    read_bounded_target(
        path,
        &expected.admitted_target(),
        limit,
        "SystemVerilog source",
    )
}

fn read_bounded_at(
    path: &str,
    expected: &AdmittedTarget,
    limit: u64,
    kind: &str,
) -> Result<String, StartupError> {
    let (mut file, metadata) = open_regular_file_at(path, expected, kind)?;
    let bytes = read_regular_file_contents(
        &mut file,
        metadata.len(),
        limit,
        path,
        kind,
        None,
        "source read",
        "source final metadata",
    )?;
    String::from_utf8(bytes).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("{kind} {path} is not UTF-8: {error}"),
        )
    })
}

fn read_bounded_target(
    path: &str,
    expected: &AdmittedTarget,
    limit: u64,
    kind: &str,
) -> Result<String, StartupError> {
    read_bounded_at(path, expected, limit, kind)
}

#[cfg(test)]
fn read_bounded_library_map(
    path: &str,
    limit: u64,
    work: &mut LibraryMapWorkBudget,
) -> Result<String, StartupError> {
    let expected = secure_fs::open_path(Path::new(path)).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot resolve library map {path}: {error}"),
        )
    })?;
    read_bounded_library_file_at(
        path,
        &expected.admitted_target(),
        limit,
        work,
        "library map",
    )
}

fn read_bounded_library_file_at(
    path: &str,
    expected: &AdmittedTarget,
    limit: u64,
    work: &mut LibraryMapWorkBudget,
    kind: &str,
) -> Result<String, StartupError> {
    let (open_operation, metadata_operation, read_operation, final_metadata_operation) =
        if kind == "library map" {
            (
                "library map open",
                "library map metadata",
                "library map incremental read",
                "library map final metadata",
            )
        } else {
            (
                "library source open",
                "library source metadata",
                "library source incremental read",
                "library source final metadata",
            )
        };
    let (mut file, metadata) = open_regular_library_file_at(
        path,
        expected,
        kind,
        work,
        open_operation,
        metadata_operation,
    )?;
    let bytes = read_regular_file_contents(
        &mut file,
        metadata.len(),
        limit,
        path,
        kind,
        Some(work),
        read_operation,
        final_metadata_operation,
    )?;
    String::from_utf8(bytes).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("{kind} {path} is not UTF-8: {error}"),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn read_regular_file_contents(
    file: &mut OpenedPath,
    expected_size: u64,
    limit: u64,
    path: &str,
    kind: &str,
    mut work: Option<&mut LibraryMapWorkBudget>,
    read_operation: &str,
    final_metadata_operation: &str,
) -> Result<Vec<u8>, StartupError> {
    if expected_size > limit {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!("{kind} {path} exceeds the configured Slang byte limit"),
        ));
    }
    let capacity = usize::try_from(expected_size).map_err(|_| {
        StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!("{kind} {path} is too large to admit safely"),
        )
    })?;
    if let Some(budget) = work.as_deref_mut() {
        budget.charge_allocation(expected_size, &format!("{kind} text admission"))?;
    }
    let mut bytes = Vec::new();
    if capacity > 0 {
        bytes.try_reserve_exact(capacity).map_err(|_| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("{kind} {path} cannot reserve its bounded buffer"),
            )
        })?;
    }

    let mut total = 0_u64;
    let mut iterations = 0_u64;
    let mut chunk = [0_u8; LIBRARY_MAP_READ_CHUNK_BYTES];
    while total < expected_size {
        let request =
            usize::try_from((expected_size - total).min(LIBRARY_MAP_READ_CHUNK_BYTES as u64))
                .unwrap_or(LIBRARY_MAP_READ_CHUNK_BYTES);
        let count = loop {
            if iterations >= MAX_REGULAR_FILE_READ_ITERATIONS {
                return Err(StartupError::new(
                    StartupErrorKind::LimitExceeded,
                    format!("{kind} {path} read iterations exceed the admission limit"),
                ));
            }
            iterations += 1;
            if let Some(budget) = work.as_deref_mut() {
                // Charge every OS read attempt, including each retry after
                // EINTR.  A filesystem that repeatedly interrupts reads
                // therefore cannot bypass the map work ceiling.
                budget.charge(1, read_operation)?;
            }
            match file.read_bytes(&mut chunk[..request]) {
                Ok(count) => break count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(StartupError::new(
                        StartupErrorKind::Input,
                        format!("cannot read {kind} {path}: {error}"),
                    ));
                }
            }
        };
        if count == 0 {
            return Err(StartupError::new(
                StartupErrorKind::Input,
                format!("{kind} {path} changed or was truncated while reading"),
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
        total = total.checked_add(count as u64).ok_or_else(|| {
            StartupError::new(StartupErrorKind::LimitExceeded, "source size overflow")
        })?;
    }

    if total != expected_size || bytes.len() as u64 != expected_size {
        return Err(StartupError::new(
            StartupErrorKind::Input,
            format!("{kind} {path} changed or was truncated while reading"),
        ));
    }
    if let Some(budget) = work {
        budget.charge(1, final_metadata_operation)?;
    }
    let final_size = file.refresh_metadata().map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot inspect {kind} {path} after reading: {error}"),
        )
    })?;
    if !final_size.file_type().is_file() || final_size.len() != expected_size {
        return Err(StartupError::new(
            StartupErrorKind::Input,
            format!("{kind} {path} changed or was truncated while reading"),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
fn open_regular_file(
    path: &str,
    kind: &str,
) -> Result<(OpenedPath, std::fs::Metadata), StartupError> {
    let opened = secure_fs::open_path(Path::new(path)).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot open {kind} {path}: {error}"),
        )
    })?;
    if !opened.is_file() {
        return Err(StartupError::new(
            StartupErrorKind::Input,
            format!("{kind} {path} is not a regular file"),
        ));
    }
    let metadata = opened.metadata().clone();
    Ok((opened, metadata))
}

fn open_regular_file_at(
    path: &str,
    expected: &AdmittedTarget,
    kind: &str,
) -> Result<(OpenedPath, std::fs::Metadata), StartupError> {
    let file = secure_fs::open_regular_file_exact(Path::new(path), expected).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot open {kind} {path}: {error}"),
        )
    })?;
    let metadata = file.metadata().clone();
    Ok((file, metadata))
}

fn open_regular_library_file_at(
    path: &str,
    expected: &AdmittedTarget,
    kind: &str,
    work: &mut LibraryMapWorkBudget,
    open_operation: &str,
    metadata_operation: &str,
) -> Result<(OpenedPath, std::fs::Metadata), StartupError> {
    work.charge(1, open_operation)?;
    let (file, metadata) = open_regular_file_at(path, expected, kind)?;
    work.charge(1, metadata_operation)?;
    Ok((file, metadata))
}

#[derive(Debug, Clone)]
struct LibraryMapEntry {
    library: String,
    patterns: Vec<String>,
    include_dirs: Vec<String>,
}

#[derive(Debug, Clone)]
struct LibraryMapToken {
    value: String,
    quoted: bool,
    configuration: Option<std::ops::Range<usize>>,
    offset: usize,
}

impl LibraryMapToken {
    fn is(&self, value: &str) -> bool {
        self.value == value
    }
}

/// Admit library-map inputs before entering Slang. The native bridge remains
/// cache-only, so every file named by a map is read and assigned its library
/// identity here. Expansion is lexical, deterministic and limited to the
/// explicit map patterns.
#[cfg(test)]
fn admit_library_maps(
    opts: &CompileOpts,
    identities: &mut HashSet<PathBuf>,
    library_sources: &mut Vec<LibrarySource>,
    source_count: &mut usize,
    remaining: &mut u64,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    let mut library_include_dirs = Vec::new();
    let mut admitted_targets = HashMap::new();
    let mut sources = Vec::new();
    let mut buffers = LibraryMapBuffers::new(&mut sources, library_sources, work)?;
    admit_library_maps_with_targets(
        opts,
        &mut library_include_dirs,
        identities,
        &mut buffers,
        source_count,
        remaining,
        work,
        &mut admitted_targets,
    )?;
    buffers.finish(remaining, work).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn admit_library_maps_with_targets(
    opts: &CompileOpts,
    library_include_dirs: &mut Vec<LibraryIncludeDir>,
    identities: &mut HashSet<PathBuf>,
    buffers: &mut LibraryMapBuffers<'_>,
    source_count: &mut usize,
    remaining: &mut u64,
    work: &mut LibraryMapWorkBudget,
    admitted_targets: &mut HashMap<PathBuf, AdmittedTarget>,
) -> Result<(), StartupError> {
    let source_limit = effective_source_count_limit(opts.limits);
    if *source_count > source_limit {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library map expansion exceeds the configured source limit",
        ));
    }
    let mut pending = Vec::new();
    let mut seen_maps = HashSet::new();
    for path in &opts.library_map_files {
        work.charge(1, "map admission")?;
        let input_path = Path::new(path);
        charge_library_path_bytes(work, input_path, "library map path preparation")?;
        let resolved =
            canonicalize_library_map_path(work, input_path, "library map path resolution")?;
        if seen_maps.contains(resolved.actual_path()) {
            continue;
        }
        // Root map buffers can now become native configuration sources too.
        // Reserve their input slot before reading or projecting any text.
        charge_library_map_source(source_count, source_limit, "library map")?;
        work.charge(1, "library map name allocation")?;
        let name = resolved.actual_path().to_string_lossy().into_owned();
        admitted_targets.insert(resolved.actual_path().to_path_buf(), resolved.clone());
        let content_limit = remaining.checked_sub(name.len() as u64).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!("library map path {name} exceeds the configured Slang byte limit"),
            )
        })?;
        let text =
            read_bounded_library_file_at(&name, &resolved, content_limit, work, "library map")?;
        *remaining = remaining.saturating_sub(name.len() as u64 + text.len() as u64);
        work.charge_usize(name.len(), "library map identity admission")?;
        let resolved_path = resolved.actual_path().to_path_buf();
        if !seen_maps.contains(&resolved_path) {
            work.charge(1, "library map identity clone")?;
            seen_maps.insert(resolved_path.clone());
            pending.push((resolved_path, text, resolved));
        }
    }

    let mut parsed_maps = Vec::new();
    let mut cursor = 0;
    while cursor < pending.len() {
        let map_path_len = pending[cursor].0.to_string_lossy().len();
        work.charge(1, "library map name processing clone")?;
        work.charge_usize(map_path_len, "library map name processing clone")?;
        let text_len = pending[cursor].1.len();
        work.charge(1, "library map text processing clone")?;
        work.charge_allocation_usize(text_len, "library map text processing clone")?;
        let (map_path, text, map_target) = pending[cursor].clone();
        cursor += 1;
        work.charge(1, "map parsing")?;
        let expanded =
            preprocess_library_map(&text, &opts.defines, opts.edition, work).map_err(|error| {
                StartupError::new(error.kind(), format!("{}: {error}", map_path.display()))
            })?;
        let ParsedLibraryMap {
            includes,
            entries,
            configuration,
        } = parse_library_map(&expanded, work).map_err(|error| {
            StartupError::new(error.kind(), format!("{}: {error}", map_path.display()))
        })?;
        if configuration.is_some() {
            charge_library_map_text_allocation(work, &text, "native map source clone")?;
            buffers.retain_configuration(&map_path.to_string_lossy(), &text, text.clone(), work)?;
        }
        let base = map_path.parent().unwrap_or_else(|| Path::new("."));
        for include in includes {
            let paths = expand_library_pattern_admitted_under(
                base,
                &include,
                source_limit as u64,
                work,
                &map_target,
            )?;
            if paths.is_empty() {
                return Err(StartupError::new(
                    StartupErrorKind::Input,
                    format!("library map include `{include}` matched no files"),
                ));
            }
            for target in paths {
                let path = target.actual_path();
                let path_len = path.to_string_lossy().len();
                work.charge_usize(path_len, "library map identity admission")?;
                if seen_maps.contains(path) {
                    continue;
                }
                charge_library_map_source(source_count, source_limit, "library map")?;
                work.charge(1, "map source admission")?;
                work.charge(1, "library map identity clone")?;
                seen_maps.insert(path.to_path_buf());
                admitted_targets.insert(path.to_path_buf(), target.clone());
                work.charge(1, "library map name allocation")?;
                let name = path.to_string_lossy().into_owned();
                let content_limit = remaining.checked_sub(name.len() as u64).ok_or_else(|| {
                    StartupError::new(
                        StartupErrorKind::LimitExceeded,
                        format!("library map path {name} exceeds the configured Slang byte limit"),
                    )
                })?;
                let map_text = read_bounded_library_file_at(
                    &name,
                    &target,
                    content_limit,
                    work,
                    "library map",
                )?;
                *remaining = remaining.saturating_sub(name.len() as u64 + map_text.len() as u64);
                pending.push((path.to_path_buf(), map_text, target));
            }
        }
        work.charge_allocation_usize(
            std::mem::size_of::<(PathBuf, AdmittedTarget, Vec<LibraryMapEntry>)>(),
            "parsed filesystem map record",
        )?;
        parsed_maps.push((map_path, map_target, entries));
    }

    // All root/included configurations must be retained before patterns can
    // map those same files into libraries. Otherwise an earlier map rereads a
    // later map as raw HDL and charges its already-admitted input twice.
    for (map_path, map_target, entries) in parsed_maps {
        let base = map_path.parent().unwrap_or_else(|| Path::new("."));
        for entry in entries {
            for dir in &entry.include_dirs {
                for path in admit_library_include_dirs(base, dir, &map_target, work)? {
                    charge_library_include_dir_entry(
                        library_include_dirs.len(),
                        &entry.library,
                        &path,
                        work,
                    )?;
                    library_include_dirs.push(LibraryIncludeDir {
                        library: entry.library.clone(),
                        path,
                    });
                }
            }
            for pattern in entry.patterns {
                let specificity = LibrarySpecificity::of_pattern(&pattern);
                let match_pattern = library_match_pattern(&pattern, work)?;
                let paths = expand_library_pattern_admitted_under(
                    base,
                    &match_pattern,
                    source_limit as u64,
                    work,
                    &map_target,
                )?;
                if paths.is_empty() {
                    return Err(StartupError::new(
                        StartupErrorKind::Input,
                        format!(
                            "library `{}` pattern `{pattern}` matched no files",
                            entry.library
                        ),
                    ));
                }
                for target in paths {
                    let path = target.actual_path();
                    let path_len = path.to_string_lossy().len();
                    work.charge_usize(path_len, "library source identity admission")?;
                    let name = path.to_string_lossy();
                    let already_admitted =
                        buffers.offer(&name, &entry.library, specificity, work)?;
                    if already_admitted {
                        admitted_targets
                            .entry(path.to_path_buf())
                            .or_insert_with(|| target.clone());
                        // A repeated map match or an explicit library assignment
                        // reuses the admitted bytes. It never authorizes a reread.
                        continue;
                    }
                    if identities.contains(path) {
                        return Err(StartupError::new(
                            StartupErrorKind::Internal,
                            format!("admitted source has no retained buffer: {}", path.display()),
                        ));
                    }
                    charge_library_map_source(source_count, source_limit, "library source")?;
                    work.charge(1, "library source admission")?;
                    work.charge(1, "library source identity clone")?;
                    identities.insert(path.to_path_buf());
                    admitted_targets.insert(path.to_path_buf(), target.clone());
                    work.charge(1, "library source name allocation")?;
                    let name = path.to_string_lossy().into_owned();
                    let name_bytes = u64::try_from(name.len()).map_err(|_| {
                        StartupError::new(
                            StartupErrorKind::LimitExceeded,
                            format!("source path {name} exceeds the configured Slang byte limit"),
                        )
                    })?;
                    let content_limit = remaining.checked_sub(name_bytes).ok_or_else(|| {
                        StartupError::new(
                            StartupErrorKind::LimitExceeded,
                            format!("source path {name} exceeds the configured Slang byte limit"),
                        )
                    })?;
                    let source_text = read_bounded_library_file_at(
                        &name,
                        &target,
                        content_limit,
                        work,
                        "library source",
                    )?;
                    let text_bytes = u64::try_from(source_text.len()).map_err(|_| {
                        StartupError::new(
                            StartupErrorKind::LimitExceeded,
                            format!("source path {name} exceeds the configured Slang byte limit"),
                        )
                    })?;
                    *remaining = content_limit.checked_sub(text_bytes).ok_or_else(|| {
                        StartupError::new(
                            StartupErrorKind::LimitExceeded,
                            format!("source path {name} exceeds the configured Slang byte limit"),
                        )
                    })?;
                    buffers
                        .sources
                        .push(OwnedSource::compilation_unit(name, source_text));
                }
            }
        }
    }
    Ok(())
}

/// Admit map-backed library sources from buffers already supplied by the
/// caller. An in-memory map name is a logical document name, not a filesystem
/// path. Its parent is used as the explicit base for relative map patterns and
/// includes; matching never opens a path, which preserves the cache-only FFI
/// contract for exact-source compilation.
#[cfg(test)]
fn admit_in_memory_library_maps(
    maps: &[OwnedSource],
    sources: &mut Vec<OwnedSource>,
    library_sources: &mut Vec<LibrarySource>,
    source_count: &mut usize,
    remaining: &mut u64,
    source_limit: usize,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<OwnedSource>, StartupError> {
    admit_in_memory_library_maps_with_dirs(
        maps,
        &[],
        LanguageEdition::SystemVerilog2009,
        &mut Vec::new(),
        sources,
        library_sources,
        source_count,
        remaining,
        source_limit,
        work,
    )
}

#[allow(clippy::too_many_arguments)]
fn admit_in_memory_library_maps_with_dirs(
    maps: &[OwnedSource],
    defines: &[String],
    edition: LanguageEdition,
    library_include_dirs: &mut Vec<LibraryIncludeDir>,
    sources: &mut Vec<OwnedSource>,
    library_sources: &mut Vec<LibrarySource>,
    source_count: &mut usize,
    remaining: &mut u64,
    source_limit: usize,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<OwnedSource>, StartupError> {
    if maps.is_empty() {
        return Ok(Vec::new());
    }
    let mut buffers = LibraryMapBuffers::new(sources, library_sources, work)?;
    collect_in_memory_library_maps(
        maps,
        defines,
        edition,
        library_include_dirs,
        &mut buffers,
        *source_count,
        source_limit,
        work,
    )?;
    buffers.finish(remaining, work)
}

#[allow(clippy::too_many_arguments)]
fn collect_in_memory_library_maps(
    maps: &[OwnedSource],
    defines: &[String],
    edition: LanguageEdition,
    library_include_dirs: &mut Vec<LibraryIncludeDir>,
    buffers: &mut LibraryMapBuffers<'_>,
    source_count: usize,
    source_limit: usize,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    if maps.is_empty() {
        return Ok(());
    }
    if source_count > source_limit {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library map expansion exceeds the configured source limit",
        ));
    }

    let mut pending = Vec::new();
    let mut seen_maps = HashMap::new();
    for map in maps {
        work.charge(1, "in-memory map admission")?;
        if map.name.is_empty() {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                "in-memory library map names must be nonempty",
            ));
        }
        let key = logical_path_key(Path::new(&map.name), work, "in-memory map name")?;
        if let Some(retained) = seen_maps.get(&key) {
            work.charge_usize(map.text.len(), "duplicate map contents comparison")?;
            if *retained != map.text.as_str() {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    format!("conflicting in-memory library map contents: {}", map.name),
                ));
            }
        } else {
            work.charge(1, "in-memory map name clone")?;
            work.charge_usize(map.name.len(), "in-memory map name clone")?;
            work.charge(1, "in-memory map text clone")?;
            charge_library_map_text_allocation(work, &map.text, "in-memory map text clone")?;
            seen_maps.insert(key, map.text.as_str());
            pending.push((map.name.clone(), map.text.clone()));
        }
    }

    // Register every admitted map's configuration before any logical glob
    // resolves; a map can name itself or another admitted map as library source.
    let mut parsed = Vec::new();
    for (name, text) in &pending {
        let expanded = preprocess_library_map(text, defines, edition, work)
            .map_err(|error| StartupError::new(error.kind(), format!("{name}: {error}")))?;
        let mut map = parse_library_map(&expanded, work)
            .map_err(|error| StartupError::new(error.kind(), format!("{name}: {error}")))?;
        if map.configuration.take().is_some() {
            charge_library_map_text_allocation(work, text, "native map source clone")?;
            buffers.retain_configuration(name, text, text.clone(), work)?;
        }
        work.charge_allocation_usize(std::mem::size_of::<ParsedLibraryMap>(), "parsed map record")?;
        parsed.push(Some(map));
    }
    let mut cursor = 0;
    while cursor < pending.len() {
        let map_name_len = pending[cursor].0.len();
        work.charge(1, "in-memory map name processing clone")?;
        work.charge_usize(map_name_len, "in-memory map name processing clone")?;
        let map_name = pending[cursor].0.clone();
        cursor += 1;
        work.charge(1, "in-memory map parsing")?;
        let ParsedLibraryMap {
            includes, entries, ..
        } = parsed[cursor - 1].take().ok_or_else(|| {
            StartupError::new(StartupErrorKind::Internal, "missing parsed library map")
        })?;
        let base = logical_map_parent(&map_name);

        for include in includes {
            let mut matches = Vec::new();
            for candidate in maps {
                work.charge(1, "in-memory include candidate scan")?;
                if map_pattern_matches(&base, &include, Path::new(&candidate.name), work)? {
                    matches.push(candidate);
                }
            }
            charge_key_comparison_work(
                work,
                matches.iter().map(|candidate| candidate.name.len()),
                true,
                "in-memory include ordering comparisons",
            )?;
            matches.sort_by(|left, right| left.name.cmp(&right.name));
            if matches.is_empty() {
                return Err(StartupError::new(
                    StartupErrorKind::Input,
                    format!(
                        "in-memory library map include `{include}` matched no admitted buffers"
                    ),
                ));
            }
            for candidate in matches {
                work.charge(1, "in-memory map admission")?;
                let key = logical_path_key(Path::new(&candidate.name), work, "in-memory map name")?;
                if !seen_maps.contains_key(&key) {
                    return Err(StartupError::new(
                        StartupErrorKind::Internal,
                        "included logical map was not registered at intake",
                    ));
                }
            }
        }

        for entry in entries {
            for dir in &entry.include_dirs {
                for path in
                    logical_map_include_dirs(&base, dir, buffers.sources, buffers.libraries, work)?
                {
                    charge_library_include_dir_entry(
                        library_include_dirs.len(),
                        &entry.library,
                        &path,
                        work,
                    )?;
                    library_include_dirs.push(LibraryIncludeDir {
                        library: entry.library.clone(),
                        path,
                    });
                }
            }
            for pattern in entry.patterns {
                work.charge(1, "in-memory library pattern")?;
                let specificity = LibrarySpecificity::of_pattern(&pattern);
                let match_pattern = library_match_pattern(&pattern, work)?;
                let mut matches = Vec::new();
                for source in buffers.sources.iter() {
                    work.charge(1, "in-memory source candidate scan")?;
                    if map_pattern_matches(&base, &match_pattern, Path::new(&source.name), work)? {
                        charge_library_map_clone(work, &source.name, "in-memory match name")?;
                        matches.push(source.name.clone());
                    }
                }
                for source in buffers.libraries.iter() {
                    work.charge(1, "in-memory library candidate scan")?;
                    if map_pattern_matches(&base, &match_pattern, Path::new(&source.name), work)? {
                        charge_library_map_clone(work, &source.name, "in-memory match name")?;
                        matches.push(source.name.clone());
                    }
                }
                if matches.is_empty() {
                    return Err(StartupError::new(
                        StartupErrorKind::Input,
                        format!(
                            "in-memory library `{}` pattern `{pattern}` matched no admitted buffers",
                            entry.library
                        ),
                    ));
                }
                charge_key_comparison_work(
                    work,
                    matches.iter().map(String::len),
                    true,
                    "in-memory source ordering comparisons",
                )?;
                // Retain the existing deterministic in-memory admission order.
                // Assignment winners, unlike discovery order, are independent
                // of map declaration order. No source moves during collection.
                matches.sort_by(|left, right| right.cmp(left));
                for name in matches {
                    buffers.offer(&name, &entry.library, specificity, work)?;
                }
            }
        }
    }
    Ok(())
}

fn charge_library_source_metadata(remaining: &mut u64, library: &str) -> Result<(), StartupError> {
    let library_bytes = u64::try_from(library.len()).map_err(|_| {
        StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library name exceeds the configured Slang byte limit",
        )
    })?;
    *remaining = remaining.checked_sub(library_bytes).ok_or_else(|| {
        StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!("library name {library} exceeds the configured Slang byte limit"),
        )
    })?;
    Ok(())
}

fn logical_path_key(
    path: &Path,
    work: &mut LibraryMapWorkBudget,
    operation: &str,
) -> Result<Vec<String>, StartupError> {
    let values = logical_components_from_text(&path.to_string_lossy());
    work.charge_usize(values.len(), operation)?;
    let mut result: Vec<String> = Vec::new();
    for component in values {
        work.charge(1, operation)?;
        work.charge_usize(component.len(), operation)?;
        if component == "." {
            continue;
        }
        if component == ".." {
            if result.last().is_some_and(|part| {
                !part.is_empty() && part != ".." && !logical_prefix_component(part)
            }) {
                result.pop();
            } else {
                work.charge_usize(2, operation)?;
                result.push(component);
            }
            continue;
        }
        result.push(component);
    }
    Ok(result)
}

fn map_pattern_matches(
    base: &Path,
    pattern: &str,
    candidate: &Path,
    work: &mut LibraryMapWorkBudget,
) -> Result<bool, StartupError> {
    work.charge_usize(pattern.len(), "logical map pattern preparation")?;
    if pattern.contains('$') {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("environment expansion is not allowed in library map path `{pattern}`"),
        ));
    }
    let pattern = logical_map_pattern_key(base, pattern, work)?;
    let candidate = logical_path_key(candidate, work, "logical map candidate normalization")?;
    logical_path_pattern_matches(&pattern, &candidate, work)
}

#[derive(Debug)]
struct LogicalMapPatternComponent {
    value: String,
    wildcards: bool,
}

fn logical_map_pattern_key(
    base: &Path,
    pattern: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<LogicalMapPatternComponent>, StartupError> {
    let absolute = logical_text_is_absolute(pattern);
    let mut result = if absolute {
        Vec::new()
    } else {
        let base_components = logical_path_key(base, work, "logical map base normalization")?;
        work.charge_usize(base_components.len(), "logical map base component storage")?;
        base_components
            .into_iter()
            .map(|value| LogicalMapPatternComponent {
                value,
                wildcards: false,
            })
            .collect()
    };
    let components = logical_components_from_text(pattern);
    let component_count = components.len();
    work.charge_usize(component_count, "logical map pattern component storage")?;
    for component in components {
        work.charge(1, "logical map pattern normalization")?;
        work.charge_usize(component.len(), "logical map pattern normalization")?;
        if component == "." {
            continue;
        }
        if component.is_empty()
            && result.last().is_some_and(|part| {
                part.value.len() == 2
                    && part.value.as_bytes()[0].is_ascii_alphabetic()
                    && part.value.as_bytes()[1] == b':'
            })
        {
            // Keep the root marker after a Windows drive prefix.  Clearing
            // the result here would turn `C:\\root\\*.sv` into a path
            // relative to the drive rather than an absolute path.
            work.charge(1, "logical map pattern normalization")?;
            result.push(LogicalMapPatternComponent {
                value: component,
                wildcards: false,
            });
        } else if logical_prefix_component(&component) {
            result.clear();
            result.push(LogicalMapPatternComponent {
                value: component,
                wildcards: false,
            });
        } else if component == ".." {
            if result.last().is_some_and(|part| {
                !part.value.is_empty()
                    && part.value != ".."
                    && !logical_prefix_component(&part.value)
            }) {
                result.pop();
            } else {
                work.charge(1, "logical map pattern normalization")?;
                result.push(LogicalMapPatternComponent {
                    value: component,
                    wildcards: false,
                });
            }
        } else {
            work.charge(1, "logical map pattern normalization")?;
            result.push(LogicalMapPatternComponent {
                wildcards: text_has_wildcard(&component),
                value: component,
            });
        }
    }
    Ok(result)
}

fn logical_prefix_component(value: &str) -> bool {
    value.is_empty()
        || value.starts_with("//")
        || (value.len() == 2
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':')
}

fn logical_map_parent(name: &str) -> PathBuf {
    let Some(index) = name.rfind(['/', '\\']) else {
        return PathBuf::from(".");
    };
    if index == 0 {
        PathBuf::from(&name[..1])
    } else if index == 2 && name.as_bytes()[1] == b':' && name.as_bytes()[0].is_ascii_alphabetic() {
        // `Path` uses the host spelling, so preserve a Windows drive root
        // lexically even when the map is being matched on Unix.
        PathBuf::from(&name[..=index])
    } else {
        PathBuf::from(&name[..index])
    }
}

fn logical_text_is_absolute(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .first()
        .is_some_and(|byte| matches!(byte, b'/' | b'\\'))
        || (bytes.len() >= 3
            && bytes[1] == b':'
            && !bytes[2].is_ascii_whitespace()
            && matches!(bytes[2], b'/' | b'\\'))
}

fn logical_components_from_text(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let separator = |character: char| matches!(character, '/' | '\\');
    let separator_byte = |byte: u8| matches!(byte, b'/' | b'\\');
    let mut result = Vec::new();
    let mut start = 0;
    if bytes.len() >= 2 && separator_byte(bytes[0]) && separator_byte(bytes[1]) {
        let parts = text[2..]
            .split(separator)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() >= 2 {
            result.push(format!("//{}/{}", parts[0], parts[1]));
            let prefix_len = 2 + parts[0].len() + 1 + parts[1].len();
            start = prefix_len;
            while start < text.len() && separator_byte(bytes[start]) {
                start += 1;
            }
        } else {
            result.push("//".to_owned());
            start = 2;
        }
    } else if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        result.push(text[..2].to_owned());
        start = 2;
        if start < text.len() && separator_byte(bytes[start]) {
            result.push(String::new());
            while start < text.len() && separator_byte(bytes[start]) {
                start += 1;
            }
        }
    } else if bytes.first().is_some_and(|byte| separator_byte(*byte)) {
        result.push(String::new());
        while start < text.len() && separator_byte(bytes[start]) {
            start += 1;
        }
    }
    for part in text[start..]
        .split(separator)
        .filter(|part| !part.is_empty())
    {
        result.push(part.to_owned());
    }
    result
}

fn logical_path_pattern_matches(
    pattern: &[LogicalMapPatternComponent],
    candidate: &[String],
    work: &mut LibraryMapWorkBudget,
) -> Result<bool, StartupError> {
    let first_is_parent = if let Some(first) = pattern.first() {
        if first.wildcards {
            false
        } else {
            charge_logical_component_comparison(work, &first.value, "..")?
        }
    } else {
        false
    };
    if first_is_parent {
        return Ok(false);
    }

    // Greedy backtracking keeps only the current positions and the last `**`.
    // Every loop retry and every component comparison is charged at the point
    // where it happens, including the bytes inspected by literal equality.
    let mut pattern_index = 0;
    let mut candidate_index = 0;
    let mut star_index = None;
    let mut star_candidate = 0;
    while candidate_index < candidate.len() {
        work.charge(1, "logical path matching step")?;
        let recursive = if pattern_index < pattern.len() && pattern[pattern_index].wildcards {
            charge_logical_component_comparison(work, &pattern[pattern_index].value, "**")?
        } else {
            false
        };
        if pattern_index < pattern.len() && !recursive {
            let pattern_component = &pattern[pattern_index];
            let matched = if pattern_component.wildcards {
                wildcard_component_matches(
                    &candidate[candidate_index],
                    &pattern_component.value,
                    work,
                )?
            } else {
                charge_logical_value_comparison(
                    work,
                    &candidate[candidate_index],
                    &pattern_component.value,
                )?
            };
            if matched {
                pattern_index += 1;
                candidate_index += 1;
                continue;
            }
        } else if recursive {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_candidate = candidate_index;
            continue;
        }

        let Some(star) = star_index else {
            return Ok(false);
        };
        work.charge(1, "logical path matching backtracking")?;
        star_candidate += 1;
        candidate_index = star_candidate;
        pattern_index = star + 1;
    }
    while pattern_index < pattern.len() && pattern[pattern_index].wildcards {
        let recursive =
            charge_logical_component_comparison(work, &pattern[pattern_index].value, "**")?;
        if !recursive {
            break;
        }
        pattern_index += 1;
    }
    Ok(pattern_index == pattern.len())
}

fn charge_logical_component_comparison(
    work: &mut LibraryMapWorkBudget,
    value: &str,
    literal: &str,
) -> Result<bool, StartupError> {
    work.charge(1, "logical path component comparison")?;
    work.charge_usize(value.len(), "logical path component comparison")?;
    work.charge_usize(literal.len(), "logical path component comparison")?;
    Ok(value == literal)
}

fn charge_logical_value_comparison(
    work: &mut LibraryMapWorkBudget,
    value: &str,
    pattern: &str,
) -> Result<bool, StartupError> {
    work.charge(1, "logical path literal comparison")?;
    work.charge_usize(value.len(), "logical path literal comparison")?;
    work.charge_usize(pattern.len(), "logical path literal comparison")?;
    Ok(value == pattern)
}

fn wildcard_component_matches(
    value: &str,
    pattern: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<bool, StartupError> {
    // The usual greedy `*` matcher is constant-memory. A star remembers the
    // next value position to retry, so it never needs a byte-by-byte table.
    // Charge every loop iteration: a failed suffix can cause the greedy
    // matcher to replay that suffix for each later value position.
    let value = value.as_bytes();
    let pattern = pattern.as_bytes();
    let mut value_index = 0;
    let mut pattern_index = 0;
    let mut star_index = None;
    let mut star_value = 0;
    while value_index < value.len() {
        work.charge(1, "byte matching step")?;
        if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_value = value_index;
        } else if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if let Some(star) = star_index {
            star_value += 1;
            value_index = star_value;
            pattern_index = star + 1;
        } else {
            return Ok(false);
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        work.charge(1, "byte matching trailing-star step")?;
        pattern_index += 1;
    }
    Ok(pattern_index == pattern.len())
}

fn charge_library_map_source(
    source_count: &mut usize,
    source_limit: usize,
    kind: &str,
) -> Result<(), StartupError> {
    if *source_count >= source_limit {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!("library map expansion exceeds the configured {kind} source limit"),
        ));
    }
    *source_count += 1;
    Ok(())
}

#[derive(Debug)]
struct ParsedLibraryMap {
    includes: Vec<String>,
    entries: Vec<LibraryMapEntry>,
    configuration: Option<String>,
}

/// Expand only the map's lexical input for bounded path admission. Slang
/// independently preprocesses the original map when it owns configurations,
/// preserving the macro invocation's native source range.
fn preprocess_library_map(
    text: &str,
    defines: &[String],
    edition: LanguageEdition,
    work: &mut LibraryMapWorkBudget,
) -> Result<String, StartupError> {
    let mut macros = macro_environment_from_defines(defines);
    let budget = MacroExpansionBudget::new(work.allocation_limit);
    let mut conditions: Vec<ConditionalFrame> = Vec::new();
    let mut output = String::new();
    let mut block_comment = false;
    for (line_number, raw_line) in logical_preprocessor_lines(text) {
        let line = strip_preprocessor_comments(&raw_line, &mut block_comment);
        let trimmed = line.trim_start();
        let directive = if trimmed.starts_with('`') {
            preprocessor_directive(trimmed)
        } else {
            None
        };
        let active = conditions.last().is_none_or(|frame| frame.active);
        let mut emitted = false;
        let error = |message: &str| {
            StartupError::new(
                StartupErrorKind::InvalidArgument,
                format!("library map line {line_number}: {message}"),
            )
        };
        match directive {
            Some(("ifdef" | "ifndef", arguments)) => {
                let name = first_macro_identifier(arguments)
                    .ok_or_else(|| error("conditional requires a macro name"))?;
                let selected = macros.contains_key(name) == trimmed.starts_with("`ifdef");
                conditions.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: active && selected,
                    active: active && selected,
                    seen_else: false,
                });
            }
            Some(("elsif", arguments)) => {
                let name = first_macro_identifier(arguments)
                    .ok_or_else(|| error("`elsif requires a macro name"))?;
                let frame = conditions
                    .last_mut()
                    .ok_or_else(|| error("`elsif without `ifdef"))?;
                if frame.seen_else {
                    return Err(error("`elsif after `else"));
                }
                frame.active =
                    frame.parent_active && !frame.branch_taken && macros.contains_key(name);
                frame.branch_taken |= frame.active;
            }
            Some(("else", _)) => {
                let frame = conditions
                    .last_mut()
                    .ok_or_else(|| error("`else without `ifdef"))?;
                if frame.seen_else {
                    return Err(error("duplicate `else"));
                }
                frame.active = frame.parent_active && !frame.branch_taken;
                frame.branch_taken = true;
                frame.seen_else = true;
            }
            Some(("endif", _)) => {
                conditions
                    .pop()
                    .ok_or_else(|| error("`endif without `ifdef"))?;
            }
            Some(("define", arguments)) if active => {
                let arguments = arguments.trim_start();
                let name = first_macro_identifier(arguments)
                    .ok_or_else(|| error("`define requires a macro name"))?;
                if !arguments.starts_with(name) {
                    return Err(error("`define requires a valid macro name"));
                }
                let remainder = &arguments[name.len()..];
                if remainder.starts_with('(') && macro_parameters(remainder, 0).is_none() {
                    return Err(error("malformed function-like macro definition"));
                }
                if matches!(
                    name,
                    "define"
                        | "undef"
                        | "undefineall"
                        | "include"
                        | "ifdef"
                        | "ifndef"
                        | "elsif"
                        | "else"
                        | "endif"
                        | "resetall"
                        | "timescale"
                        | "default_nettype"
                        | "celldefine"
                        | "endcelldefine"
                        | "line"
                        | "begin_keywords"
                        | "end_keywords"
                        | "pragma"
                ) {
                    return Err(error("compiler directive names cannot be redefined"));
                }
                define_macro(arguments, &mut macros);
            }
            Some(("undef", arguments)) if active => {
                let name = first_macro_identifier(arguments)
                    .ok_or_else(|| error("`undef requires a macro name"))?;
                macros.remove(name);
            }
            Some(("undefineall", _)) if active => {
                if edition == LanguageEdition::Verilog2001 {
                    return Err(error("`undefineall requires SystemVerilog-2009"));
                }
                macros.clear();
            }
            Some(("include", _)) if active => {
                return Err(error(
                    "`include in a library map is unsupported; use a map include declaration",
                ));
            }
            _ if active => {
                let expanded = expand_macros(&raw_line, &macros, &budget)?;
                let mut comment = false;
                let visible = strip_preprocessor_comments(&expanded, &mut comment);
                if let Some((name, _)) = preprocessor_directive(&visible) {
                    let message = if macros.contains_key(name) {
                        format!("recursive or over-depth macro `{name}")
                    } else {
                        format!("undefined macro `{name}")
                    };
                    return Err(error(&message));
                }
                work.charge_usize(expanded.len() + 1, "expanded library map")?;
                work.charge_allocation_usize(expanded.len() + 1, "expanded library map")?;
                output.push_str(&expanded);
                emitted = true;
            }
            _ => {}
        }
        let newlines = if emitted {
            1
        } else {
            raw_line.bytes().filter(|byte| *byte == b'\n').count() + 1
        };
        work.charge_usize(newlines, "expanded library map lines")?;
        work.charge_allocation_usize(newlines, "expanded library map lines")?;
        for _ in 0..newlines {
            output.push('\n');
        }
    }
    if !conditions.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "library map has an unbalanced conditional directive",
        ));
    }
    Ok(output)
}

fn parse_library_map(
    text: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<ParsedLibraryMap, StartupError> {
    work.charge_usize(text.len(), "library map tokenization")?;
    let tokens = library_map_tokens(text, work)?;
    let mut includes = Vec::new();
    let mut entries = Vec::new();
    let mut configurations = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if let Some(range) = &tokens[index].configuration {
            work.charge_allocation_usize(
                std::mem::size_of::<std::ops::Range<usize>>(),
                "library map configuration range",
            )?;
            configurations.push(range.clone());
            index += 1;
            continue;
        }
        match tokens[index].value.as_str() {
            ";" => {
                index += 1;
                continue;
            }
            "include" => {
                let Some(path) = tokens.get(index + 1).filter(|token| !token.is(";")) else {
                    return Err(StartupError::new(
                        StartupErrorKind::InvalidArgument,
                        "library map include requires a path",
                    ));
                };
                charge_library_map_clone(work, &path.value, "library map include clone")?;
                includes.push(path.value.clone());
                index += 2;
            }
            "library" => {
                let Some(name) = tokens.get(index + 1).filter(|token| !token.is(";")) else {
                    return Err(StartupError::new(
                        StartupErrorKind::InvalidArgument,
                        "library declaration requires a name",
                    ));
                };
                let mut patterns = Vec::new();
                let mut include_dirs = Vec::new();
                let mut in_incdir = false;
                charge_library_map_clone(work, &name.value, "library map library-name clone")?;
                index += 2;
                while index < tokens.len() && !tokens[index].is(";") {
                    if tokens[index].is(",") {
                        index += 1;
                        continue;
                    }
                    let incdir_clause = !tokens[index].quoted
                        && (tokens[index].is("-incdir")
                            || (tokens[index].is("-")
                                && tokens
                                    .get(index + 1)
                                    .is_some_and(|token| token.is("incdir") && !token.quoted)));
                    if incdir_clause {
                        if in_incdir {
                            return Err(StartupError::new(
                                StartupErrorKind::InvalidArgument,
                                format!("library `{}` has repeated -incdir clause", name.value),
                            ));
                        }
                        in_incdir = true;
                        index += if tokens[index].is("-") { 2 } else { 1 };
                        continue;
                    }
                    charge_library_map_clone(
                        work,
                        &tokens[index].value,
                        "library map pattern clone",
                    )?;
                    if in_incdir {
                        include_dirs.push(tokens[index].value.clone());
                    } else {
                        patterns.push(tokens[index].value.clone());
                    }
                    index += 1;
                }
                if patterns.is_empty() {
                    return Err(StartupError::new(
                        StartupErrorKind::InvalidArgument,
                        format!("library `{}` has no source patterns", name.value),
                    ));
                }
                if in_incdir && include_dirs.is_empty() {
                    return Err(StartupError::new(
                        StartupErrorKind::InvalidArgument,
                        format!("library `{}` -incdir requires a directory", name.value),
                    ));
                }
                work.charge(1, "library map entry allocation")?;
                entries.push(LibraryMapEntry {
                    library: name.value.clone(),
                    patterns,
                    include_dirs,
                });
            }
            _ => {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    format!(
                        "library map line {}: unexpected token `{}`",
                        text[..tokens[index].offset]
                            .bytes()
                            .filter(|byte| *byte == b'\n')
                            .count()
                            + 1,
                        tokens[index].value
                    ),
                ));
            }
        }
        if !tokens.get(index).is_some_and(|token| token.is(";")) {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                "library map declaration requires a terminating semicolon",
            ));
        }
        index += 1;
    }
    let configuration = library_configs::project(text, &configurations, work)?;
    Ok(ParsedLibraryMap {
        includes,
        entries,
        configuration,
    })
}

fn charge_library_map_clone(
    work: &mut LibraryMapWorkBudget,
    value: &str,
    operation: &str,
) -> Result<(), StartupError> {
    work.charge(1, operation)?;
    work.charge_allocation_usize(value.len(), operation)
}

fn push_library_map_token(
    tokens: &mut Vec<LibraryMapToken>,
    value: &str,
    offset: usize,
    work: &mut LibraryMapWorkBudget,
    operation: &str,
) -> Result<(), StartupError> {
    charge_library_map_clone(work, value, operation)?;
    tokens.push(LibraryMapToken {
        value: value.to_owned(),
        quoted: false,
        configuration: None,
        offset,
    });
    Ok(())
}

fn starts_library_map_line_comment(bytes: &[u8], index: usize, path_context: bool) -> bool {
    if bytes.get(index) != Some(&b'/') || bytes.get(index + 1) != Some(&b'/') {
        return false;
    }
    if !path_context {
        return true;
    }
    let content_start = index + 2;
    if content_start >= bytes.len() || bytes[content_start].is_ascii_whitespace() {
        return true;
    }

    // A leading `//` is also a valid UNC path. Treat it as a comment only
    // when the following token has no path separator, which leaves ordinary
    // `// comment` forms unambiguous while preserving `//server/share`.
    let mut end = content_start;
    while end < bytes.len()
        && !bytes[end].is_ascii_whitespace()
        && !matches!(bytes[end], b';' | b',')
    {
        end += 1;
    }
    !bytes[content_start..end]
        .iter()
        .any(|byte| matches!(byte, b'/' | b'\\'))
}

fn library_map_tokens(
    text: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<LibraryMapToken>, StartupError> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let at_statement_start = tokens
            .last()
            .is_none_or(|token: &LibraryMapToken| token.is(";") || token.configuration.is_some());
        if at_statement_start
            && bytes[index..].starts_with(b"config")
            && bytes
                .get(index + 6)
                .is_none_or(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'_' | b'$'))
        {
            // Only delimit the unchanged configuration block. The ordinary
            // Slang parser still owns its grammar, names and binding rules.
            let end = library_configs::declaration_end(text, index + 6)?;
            push_library_map_token(
                &mut tokens,
                "config",
                index,
                work,
                "map configuration token",
            )?;
            if let Some(token) = tokens.last_mut() {
                token.configuration = Some(index..end);
            }
            index = end;
            continue;
        }
        let path_context = library_map_path_context(&tokens);
        if starts_library_map_line_comment(bytes, index, path_context) {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/'
            && bytes.get(index + 1) == Some(&b'*')
            && (!path_context || starts_library_map_block_comment(bytes, index))
        {
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            if index + 1 >= bytes.len() {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    "unterminated library map comment",
                ));
            }
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            index += 1;
            let content_start = index;
            let mut end = index;
            while end < bytes.len() {
                if bytes[end] == b'"' {
                    let mut backslashes = 0_usize;
                    let mut preceding = end;
                    while preceding > content_start && bytes[preceding - 1] == b'\\' {
                        backslashes += 1;
                        preceding -= 1;
                    }
                    if backslashes.is_multiple_of(2) {
                        break;
                    }
                }
                end += 1;
            }
            if end >= bytes.len() {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    "unterminated library map string",
                ));
            }
            charge_library_map_clone(
                work,
                &text[content_start..end],
                "library map quoted token allocation",
            )?;
            work.charge_allocation_usize(
                end - content_start,
                "library map quoted token allocation",
            )?;
            index = end + 1;
            let token = String::from_utf8(bytes[content_start..end].to_vec()).map_err(|_| {
                StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    "library map quoted string is not valid UTF-8",
                )
            })?;
            tokens.push(LibraryMapToken {
                value: token,
                quoted: true,
                configuration: None,
                offset: content_start - 1,
            });
            continue;
        }
        if matches!(bytes[index], b';' | b',') {
            let punctuation = if bytes[index] == b';' { ";" } else { "," };
            push_library_map_token(
                &mut tokens,
                punctuation,
                index,
                work,
                "library map punctuation token allocation",
            )?;
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b';' | b',')
        {
            index += 1;
        }
        push_library_map_token(
            &mut tokens,
            &text[start..index],
            start,
            work,
            "library map token allocation",
        )?;
    }
    Ok(tokens)
}

fn library_map_path_context(tokens: &[LibraryMapToken]) -> bool {
    let start = tokens
        .iter()
        .rposition(|token| token.is(";") || token.configuration.is_some())
        .map_or(0, |index| index + 1);
    let statement = &tokens[start..];
    match statement.first().map(|token| token.value.as_str()) {
        Some("include") => statement.len() == 1,
        Some("library") => {
            statement.len() == 2 || statement.last().is_some_and(|token| token.is(","))
        }
        _ => false,
    }
}

fn starts_library_map_block_comment(bytes: &[u8], index: usize) -> bool {
    let content_start = index + 2;
    // `/**/` is both an empty block comment and the first four bytes of an
    // absolute recursive pattern such as `/**/*.sv`.  A wildcard suffix
    // identifies the latter; a literal suffix keeps the ordinary comment
    // behavior even when it is adjacent to the comment terminator.
    if bytes.get(content_start) == Some(&b'*')
        && bytes.get(content_start + 1) == Some(&b'/')
        && bytes
            .get(content_start + 2)
            .is_some_and(|byte| matches!(byte, b'*' | b'?'))
    {
        return false;
    }
    if bytes
        .get(content_start)
        .is_some_and(u8::is_ascii_whitespace)
    {
        return true;
    }
    let mut cursor = content_start;
    while cursor + 1 < bytes.len() && !matches!(bytes[cursor], b';' | b',') {
        if bytes[cursor] == b'*' && bytes[cursor + 1] == b'/' {
            return true;
        }
        cursor += 1;
    }
    false
}

struct LibraryPatternBudget<'a> {
    work: &'a mut LibraryMapWorkBudget,
    matches: u64,
    max_matches: u64,
}

impl LibraryPatternBudget<'_> {
    fn visit(&mut self) -> Result<(), StartupError> {
        self.work.charge(1, "filesystem traversal")
    }

    fn match_path(&mut self) -> Result<(), StartupError> {
        if self.matches >= self.max_matches {
            return Err(StartupError::new(
                StartupErrorKind::LimitExceeded,
                "library map pattern matches exceed the source limit; reduce matching files or raise max_sources",
            ));
        }
        self.work.charge(1, "filesystem source matching")?;
        self.matches = self
            .matches
            .checked_add(1)
            .ok_or_else(|| self.work.limit_error("filesystem source matching"))?;
        Ok(())
    }
}

fn charge_library_pattern_entry_name(
    work: &mut LibraryMapWorkBudget,
    name: &std::ffi::OsStr,
) -> Result<(), StartupError> {
    let name_len = name.to_string_lossy().len();
    work.charge_usize(name_len, "filesystem directory entry name")
}

fn charge_library_pattern_child_path(
    work: &mut LibraryMapWorkBudget,
    parent: &Path,
    name: &std::ffi::OsStr,
) -> Result<(), StartupError> {
    charge_library_path_join(work, parent, name, "filesystem child path bytes")?;
    work.charge(1, "filesystem child path storage")?;
    work.charge(2, "filesystem path inspection")
}

fn charge_library_pattern_component(
    work: &mut LibraryMapWorkBudget,
    component: &std::ffi::OsStr,
    operation: &str,
) -> Result<(), StartupError> {
    work.charge_usize(component.to_string_lossy().len(), operation)
}

#[cfg(test)]
fn expand_library_pattern(
    base: &Path,
    pattern: &str,
    max_matches: u64,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<PathBuf>, StartupError> {
    Ok(
        expand_library_pattern_admitted(base, pattern, max_matches, work)?
            .into_iter()
            .map(|target| target.actual_path().to_path_buf())
            .collect(),
    )
}

#[cfg(test)]
fn expand_library_pattern_admitted(
    base: &Path,
    pattern: &str,
    max_matches: u64,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<AdmittedTarget>, StartupError> {
    expand_library_pattern_admitted_with_kind(base, pattern, max_matches, work, None, false)
}

fn expand_library_pattern_admitted_under(
    base: &Path,
    pattern: &str,
    max_matches: u64,
    work: &mut LibraryMapWorkBudget,
    map_target: &AdmittedTarget,
) -> Result<Vec<AdmittedTarget>, StartupError> {
    work.charge(2, "filesystem map anchor admission")?;
    let base_handle = secure_fs::open_parent_of_target(map_target).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot reopen library map base {}: {error}", base.display()),
        )
    })?;
    expand_library_pattern_admitted_with_kind(
        base,
        pattern,
        max_matches,
        work,
        Some(&base_handle),
        false,
    )
}

fn expand_library_include_pattern_admitted_under(
    base: &Path,
    pattern: &str,
    max_matches: u64,
    work: &mut LibraryMapWorkBudget,
    map_target: &AdmittedTarget,
) -> Result<Vec<AdmittedTarget>, StartupError> {
    work.charge(2, "filesystem map include anchor admission")?;
    let base_handle = secure_fs::open_parent_of_target(map_target).map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!("cannot reopen library map base {}: {error}", base.display()),
        )
    })?;
    expand_library_pattern_admitted_with_kind(
        base,
        pattern,
        max_matches,
        work,
        Some(&base_handle),
        true,
    )
}

fn expand_library_pattern_admitted_with_kind(
    base: &Path,
    pattern: &str,
    max_matches: u64,
    work: &mut LibraryMapWorkBudget,
    admitted_base: Option<&OpenedPath>,
    directories: bool,
) -> Result<Vec<AdmittedTarget>, StartupError> {
    work.charge_usize(pattern.len(), "filesystem pattern preparation")?;
    if pattern.contains('$') {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("environment expansion is not allowed in library map path `{pattern}`"),
        ));
    }
    if max_matches == 0 {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library map pattern matches exceed the source limit; reduce matching files or raise max_sources",
        ));
    }
    let pattern_path = Path::new(pattern);
    let component_count = pattern_path.components().count();
    if component_count > MAX_LIBRARY_PATTERN_COMPONENTS {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map pattern component depth exceeds the limit of {MAX_LIBRARY_PATTERN_COMPONENTS}; simplify the pattern"
            ),
        ));
    }
    work.charge_usize(component_count, "filesystem pattern component storage")?;
    let components = pattern_path.components().collect::<Vec<_>>();
    // Relative map paths are relative to the map file (V 13.2.1 / SV 33.3.1),
    // including leading `..` components. The descriptor-relative walk below
    // cannot climb above its anchor, so re-anchor at the ancestor directory,
    // which is opened with the same trust as an absolute pattern path.
    let leading = components
        .iter()
        .take_while(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        .count();
    let parents = components[..leading]
        .iter()
        .filter(|component| matches!(component, std::path::Component::ParentDir))
        .count();
    if !pattern_path.is_absolute() && parents > 0 {
        let mut anchor = admitted_base
            .map(|base| base.actual_path().to_path_buf())
            .unwrap_or_else(|| base.to_path_buf());
        for _ in 0..parents {
            if !anchor.pop() {
                return Err(StartupError::new(
                    StartupErrorKind::Input,
                    format!("library map path `{pattern}` climbs above its filesystem root"),
                ));
            }
        }
        let rest = components[leading..].iter().collect::<PathBuf>();
        let Some(rest) = rest.to_str().filter(|rest| !rest.is_empty()) else {
            return Ok(Vec::new());
        };
        charge_library_path_bytes(work, &anchor, "filesystem pattern anchor base bytes")?;
        work.charge(1, "filesystem pattern anchor admission")?;
        let anchor_handle = secure_fs::open_path(&anchor).map_err(|error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!("cannot open library map base {}: {error}", anchor.display()),
            )
        })?;
        return expand_library_pattern_admitted_with_kind(
            anchor_handle.actual_path(),
            rest,
            max_matches,
            work,
            Some(&anchor_handle),
            directories,
        );
    }
    let wildcard_index = components
        .iter()
        .position(|component| component_has_wildcard(component.as_os_str()));
    let owned_base_handle = if pattern_path.is_absolute() || admitted_base.is_some() {
        None
    } else {
        Some(secure_fs::open_path(base).map_err(|error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!("cannot open library map base {}: {error}", base.display()),
            )
        })?)
    };
    let base_handle = admitted_base.or(owned_base_handle.as_ref());
    // Candidates are opened beneath the handle's resolved path. A base spelled
    // through a symlink (macOS reports /var/... as /private/var/...) must use
    // that resolved spelling, as the absolute-anchor branch above does.
    let base = owned_base_handle
        .as_ref()
        .map_or(base, |handle| handle.actual_path());
    let Some(wildcard_index) = wildcard_index else {
        if pattern_path.is_absolute() {
            charge_library_path_bytes(work, pattern_path, "filesystem pattern path bytes")?;
            work.charge(1, "filesystem pattern path storage")?;
        } else {
            charge_library_path_join(
                work,
                base,
                pattern_path.as_os_str(),
                "filesystem pattern path bytes",
            )?;
            work.charge(1, "filesystem pattern path storage")?;
        }
        let candidate = if pattern_path.is_absolute() {
            PathBuf::from(pattern)
        } else {
            base.join(pattern)
        };
        work.charge(1, "filesystem path resolution")?;
        let opened = if pattern_path.is_absolute() {
            secure_fs::open_path(&candidate)
        } else {
            let base_handle = base_handle.ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Input,
                    format!("cannot open library map base {}", base.display()),
                )
            })?;
            secure_fs::open_path_under(&candidate, base_handle)
        };
        let opened = match opened {
            Ok(opened) => opened,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(StartupError::new(
                    StartupErrorKind::Input,
                    format!(
                        "cannot inspect library map path {}: {error}",
                        candidate.display()
                    ),
                ));
            }
        };
        work.charge(1, "filesystem source check")?;
        if !(if directories {
            opened.is_dir()
        } else {
            opened.is_file()
        }) {
            return Ok(Vec::new());
        }
        work.charge(1, "filesystem source matching")?;
        return Ok(vec![opened.admitted_target()]);
    };

    let mut anchor = if pattern_path.is_absolute() {
        work.charge(1, "filesystem pattern anchor storage")?;
        PathBuf::new()
    } else {
        charge_library_path_bytes(work, base, "filesystem pattern anchor base bytes")?;
        work.charge(1, "filesystem pattern anchor storage")?;
        PathBuf::from(base)
    };
    for component in &components[..wildcard_index] {
        charge_library_pattern_child_path(work, &anchor, component.as_os_str())?;
        anchor.push(component.as_os_str());
    }
    let rest_len = components.len() - wildcard_index;
    work.charge_usize(rest_len, "filesystem pattern suffix storage")?;
    let mut rest = Vec::with_capacity(rest_len);
    for component in &components[wildcard_index..] {
        charge_library_pattern_component(
            work,
            component.as_os_str(),
            "filesystem pattern suffix bytes",
        )?;
        work.charge(1, "filesystem pattern suffix allocation")?;
        rest.push(component.as_os_str().to_owned());
    }
    work.charge(1, "filesystem pattern anchor resolution")?;
    let anchor = if pattern_path.is_absolute() {
        secure_fs::open_path(&anchor)
    } else {
        if anchor == base {
            let base_handle = base_handle.ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Input,
                    format!("cannot open library map base {}", base.display()),
                )
            })?;
            Ok(base_handle.try_clone().map_err(|error| {
                StartupError::new(
                    StartupErrorKind::Input,
                    format!("cannot clone library map base {}: {error}", base.display()),
                )
            })?)
        } else {
            let base_handle = base_handle.ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Input,
                    format!("cannot open library map base {}", base.display()),
                )
            })?;
            secure_fs::open_path_under(&anchor, base_handle)
        }
    }
    .map_err(|error| {
        StartupError::new(
            StartupErrorKind::Input,
            format!(
                "cannot resolve library map pattern anchor {}: {error}",
                anchor.display()
            ),
        )
    })?;
    let mut matches = Vec::new();
    let mut budget = LibraryPatternBudget {
        work,
        matches: 0,
        max_matches,
    };
    let mut ancestors = Vec::new();
    walk_library_pattern(
        &anchor,
        &rest,
        &mut matches,
        &mut budget,
        0,
        &mut ancestors,
        directories,
    )?;
    charge_key_comparison_work(
        budget.work,
        matches.iter().map(|target: &AdmittedTarget| {
            target.actual_path().as_os_str().to_string_lossy().len()
        }),
        true,
        "filesystem match ordering comparisons",
    )?;
    matches.sort_by(|left, right| left.actual_path().cmp(right.actual_path()));
    charge_key_comparison_work(
        budget.work,
        matches.iter().map(|target: &AdmittedTarget| {
            target.actual_path().as_os_str().to_string_lossy().len()
        }),
        false,
        "filesystem match deduplication comparisons",
    )?;
    matches.dedup_by(|left, right| left.actual_path() == right.actual_path());
    Ok(matches)
}

fn walk_library_pattern(
    current: &OpenedPath,
    components: &[std::ffi::OsString],
    matches: &mut Vec<AdmittedTarget>,
    budget: &mut LibraryPatternBudget,
    depth: usize,
    ancestors: &mut Vec<FileIdentity>,
    directories: bool,
) -> Result<(), StartupError> {
    if depth > MAX_LIBRARY_PATTERN_COMPONENTS {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            format!(
                "library map pattern recursion depth exceeds the limit of {MAX_LIBRARY_PATTERN_COMPONENTS}; simplify the pattern"
            ),
        ));
    }
    let next_depth = depth
        .checked_add(1)
        .ok_or_else(|| budget.work.limit_error("filesystem pattern recursion"))?;
    budget.visit()?;
    budget.work.charge(2, "filesystem path inspection")?;
    let current_identity = current.identity();
    if ancestors.contains(&current_identity)
        && ancestors
            .last()
            .is_none_or(|last| *last != current_identity)
    {
        return Ok(());
    }
    budget.work.charge(1, "filesystem ancestor comparison")?;
    ancestors.push(current_identity);
    if components.is_empty() {
        budget.work.charge(1, "filesystem source check")?;
        let is_match = if directories {
            current.is_dir()
        } else {
            current.is_file()
        };
        if is_match {
            budget.match_path()?;
            budget.work.charge(1, "filesystem match storage")?;
            matches.push(current.admitted_target());
        }
        ancestors.pop();
        return Ok(());
    }
    let component = components[0].to_string_lossy();
    budget
        .work
        .charge_usize(component.len(), "filesystem pattern component")?;
    if component == "**" {
        walk_library_pattern(
            current,
            &components[1..],
            matches,
            budget,
            next_depth,
            ancestors,
            directories,
        )?;
        let mut children = Vec::new();
        budget.work.charge(1, "filesystem directory scan")?;
        if !current.is_dir() {
            ancestors.pop();
            return Ok(());
        }
        for entry in current.read_dir().map_err(|error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!(
                    "cannot read library map directory {}: {error}",
                    current.actual_path().display()
                ),
            )
        })? {
            budget.work.charge(1, "filesystem directory entry")?;
            let file_name = match entry {
                Ok(file_name) => file_name,
                Err(_) => continue,
            };
            charge_library_pattern_entry_name(budget.work, &file_name)?;
            charge_library_pattern_child_path(budget.work, current.actual_path(), &file_name)?;
            let Ok(child) = current.open_child(&file_name) else {
                continue;
            };
            budget.work.charge(1, "filesystem directory check")?;
            if child.is_dir() {
                budget
                    .work
                    .charge(1, "filesystem directory child storage")?;
                children.push((file_name, child));
            }
        }
        charge_key_comparison_work(
            budget.work,
            children
                .iter()
                .map(|(name, _)| name.to_string_lossy().len()),
            true,
            "filesystem directory ordering comparisons",
        )?;
        children.sort_by(|left, right| left.0.cmp(&right.0));
        for (_, child) in children {
            walk_library_pattern(
                &child,
                components,
                matches,
                budget,
                next_depth,
                ancestors,
                directories,
            )?;
        }
    } else if component_has_wildcard(components[0].as_os_str()) {
        let mut children = Vec::new();
        budget.work.charge(1, "filesystem directory scan")?;
        if !current.is_dir() {
            ancestors.pop();
            return Ok(());
        }
        for entry in current.read_dir().map_err(|error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!(
                    "cannot read library map directory {}: {error}",
                    current.actual_path().display()
                ),
            )
        })? {
            budget.work.charge(1, "filesystem directory entry")?;
            let file_name = match entry {
                Ok(file_name) => file_name,
                Err(_) => continue,
            };
            let name = file_name.to_string_lossy();
            budget
                .work
                .charge_usize(name.len(), "filesystem directory entry name")?;
            if wildcard_component_matches(&name, &component, budget.work)? {
                charge_library_pattern_child_path(budget.work, current.actual_path(), &file_name)?;
                let Ok(child) = current.open_child(&file_name) else {
                    continue;
                };
                budget.work.charge(1, "filesystem directory check")?;
                if components.len() > 1 && !child.is_dir() {
                    continue;
                }
                budget
                    .work
                    .charge(1, "filesystem directory child storage")?;
                children.push((file_name, child));
            }
        }
        charge_key_comparison_work(
            budget.work,
            children
                .iter()
                .map(|(name, _)| name.to_string_lossy().len()),
            true,
            "filesystem directory ordering comparisons",
        )?;
        children.sort_by(|left, right| left.0.cmp(&right.0));
        for (_, child) in children {
            walk_library_pattern(
                &child,
                &components[1..],
                matches,
                budget,
                next_depth,
                ancestors,
                directories,
            )?;
        }
    } else {
        budget
            .work
            .charge_usize(component.len(), "filesystem path descent")?;
        charge_library_pattern_child_path(
            budget.work,
            current.actual_path(),
            components[0].as_ref(),
        )?;
        if let Ok(child) = current.open_child(components[0].as_ref()) {
            walk_library_pattern(
                &child,
                &components[1..],
                matches,
                budget,
                next_depth,
                ancestors,
                directories,
            )?;
        }
    }
    ancestors.pop();
    Ok(())
}

fn component_has_wildcard(component: &std::ffi::OsStr) -> bool {
    let component = component.to_string_lossy();
    text_has_wildcard(&component)
}

fn text_has_wildcard(component: &str) -> bool {
    component.contains('*') || component.contains('?')
}

fn split_library_file_spec<'a>(
    value: &'a str,
    default_library: Option<&str>,
) -> Result<(String, &'a str), StartupError> {
    if let Some((library, path)) = value.split_once('=') {
        if library.is_empty() || path.is_empty() {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                "library file must use library=path with nonempty values",
            ));
        }
        return Ok((library.to_owned(), path));
    }
    Ok((default_library.unwrap_or("work").to_owned(), value))
}

fn absolute_path(path: &Path) -> Result<AdmittedTarget, StartupError> {
    secure_fs::open_path(path)
        .map(|opened| opened.admitted_target())
        .map_err(|error| {
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

fn charge_library_include_dir_entry(
    count: usize,
    library: &str,
    path: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    if count >= MAX_LIBRARY_INCLUDE_DIRS {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "library include directory count exceeds the native limit",
        ));
    }
    let bytes = std::mem::size_of::<LibraryIncludeDir>()
        .checked_add(library.len())
        .and_then(|bytes| bytes.checked_add(path.len()))
        .ok_or_else(|| work.limit_error("library include directory metadata"))?;
    work.charge_allocation_usize(bytes, "library include directory metadata")
}

fn admit_library_include_dirs(
    base: &Path,
    spec: &str,
    map_target: &AdmittedTarget,
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<String>, StartupError> {
    work.charge_usize(spec.len(), "library include directory path")?;
    if spec.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            "library -incdir requires a directory path",
        ));
    }
    let matches = expand_library_include_pattern_admitted_under(
        base,
        spec,
        MAX_LIBRARY_INCLUDE_DIRS as u64,
        work,
        map_target,
    )
    .map_err(|error| {
        StartupError::new(
            error.kind(),
            format!("library -incdir `{spec}` cannot be admitted: {error}"),
        )
    })?;
    if matches.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::Input,
            format!("library -incdir `{spec}` matched no directories"),
        ));
    }
    Ok(matches
        .into_iter()
        .map(|entry| entry.actual_path().to_string_lossy().into_owned())
        .collect())
}

fn logical_map_include_dirs(
    base: &Path,
    spec: &str,
    sources: &[OwnedSource],
    libraries: &[LibrarySource],
    work: &mut LibraryMapWorkBudget,
) -> Result<Vec<String>, StartupError> {
    if spec.is_empty() || spec.contains('$') {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("logical library -incdir requires a valid directory path: `{spec}`"),
        ));
    }
    let pattern = logical_map_pattern_key(base, spec, work)?;
    let mut directories = Vec::new();
    let mut seen = HashSet::new();
    for name in sources
        .iter()
        .map(|source| source.name.as_str())
        .chain(libraries.iter().map(|source| source.name.as_str()))
    {
        work.charge(1, "logical include directory candidate")?;
        let key = logical_path_key(Path::new(name), work, "logical include directory candidate")?;
        for end in 1..key.len() {
            work.charge(1, "logical include directory ancestor")?;
            if logical_path_pattern_matches(&pattern, &key[..end], work)? {
                let path = key[..end].join("/");
                work.charge_allocation_usize(path.len(), "logical include directory match")?;
                if seen.insert(path.clone()) {
                    if directories.len() >= MAX_LIBRARY_INCLUDE_DIRS {
                        return Err(StartupError::new(
                            StartupErrorKind::LimitExceeded,
                            "logical library -incdir matches exceed the include directory limit",
                        ));
                    }
                    directories.push(path);
                }
            }
        }
    }
    if directories.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::Input,
            format!("logical library -incdir `{spec}` matched no admitted directories"),
        ));
    }
    charge_key_comparison_work(
        work,
        directories.iter().map(String::len),
        true,
        "logical include directory ordering",
    )?;
    directories.sort();
    Ok(directories)
}

// Only the Unix-specific include replacement tests use this helper.
#[cfg(all(test, unix))]
fn resolve_include(
    including: &Path,
    target: &str,
    include_dirs: &[String],
) -> Option<AdmittedTarget> {
    resolve_include_checked(including, None, target, include_dirs)
}

fn resolve_include_checked(
    including: &Path,
    including_target: Option<&AdmittedTarget>,
    target: &str,
    include_dirs: &[String],
) -> Option<AdmittedTarget> {
    let target = Path::new(target);
    if let Some(expected) = including_target {
        let root = secure_fs::open_parent_of_target(expected).ok()?;
        if let Some(path) = resolve_include_under_root(&root, target) {
            return Some(path);
        }
    } else if let Some(parent) = including.parent() {
        if let Some(root) = secure_fs::open_path(parent)
            .ok()
            .filter(|root| root.is_dir())
        {
            if let Some(path) = resolve_include_under_root(&root, target) {
                return Some(path);
            }
        }
    }

    include_dirs
        .iter()
        .map(PathBuf::from)
        .find_map(|root_path| {
            let root = secure_fs::open_path(&root_path).ok()?;
            if !root.is_dir() {
                return None;
            }
            resolve_include_under_root(&root, target)
        })
}

fn resolve_include_under_root(root: &OpenedPath, target: &Path) -> Option<AdmittedTarget> {
    let candidate = if target.is_absolute() {
        if !target.starts_with(root.actual_path()) {
            return None;
        }
        target.to_path_buf()
    } else {
        root.actual_path().join(target)
    };
    let opened = secure_fs::open_path_under(&candidate, root).ok()?;
    opened.is_file().then(|| opened.admitted_target())
}

const MAX_INCLUDE_DISCOVERY_DEPTH: usize = 256;
const MAX_MACRO_EXPANSION_DEPTH: usize = 64;

struct MacroExpansionBudget {
    bytes: Cell<u64>,
    work: Cell<u64>,
}

impl MacroExpansionBudget {
    fn new(limit: u64) -> Self {
        Self {
            bytes: Cell::new(limit),
            work: Cell::new(limit),
        }
    }

    fn charge(cell: &Cell<u64>, amount: usize) -> Result<(), StartupError> {
        let amount = u64::try_from(amount).map_err(|_| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "macro include expansion exceeds the configured Slang byte limit",
            )
        })?;
        let remaining = cell.get().checked_sub(amount).ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                "macro include expansion exceeds the configured Slang byte limit",
            )
        })?;
        cell.set(remaining);
        Ok(())
    }

    fn push(&self, output: &mut String, character: char) -> Result<(), StartupError> {
        Self::charge(&self.bytes, character.len_utf8())?;
        output.push(character);
        Ok(())
    }

    fn push_str(&self, output: &mut String, value: &str) -> Result<(), StartupError> {
        Self::charge(&self.bytes, value.len())?;
        output.push_str(value);
        Ok(())
    }

    fn step(&self) -> Result<(), StartupError> {
        Self::charge(&self.work, 1)
    }
}

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
    seen_else: bool,
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

#[allow(clippy::too_many_arguments)]
fn admit_macro_includes(
    name: &str,
    text: &str,
    opts: &CompileOpts,
    include_dirs: &[String],
    macros: &mut MacroEnvironment,
    including_target: Option<&AdmittedTarget>,
    admitted_targets: &mut HashMap<PathBuf, AdmittedTarget>,
    identities: &mut HashSet<PathBuf>,
    owned: &mut Vec<OwnedSource>,
    source_count: &mut usize,
    remaining: &mut u64,
    include_stack: &mut Vec<PathBuf>,
    expansion_budget: &MacroExpansionBudget,
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
        let Some(path) =
            resolve_include_checked(including, including_target, &target, include_dirs)
        else {
            // Leave missing, malformed, and unauthorized targets for Slang so
            // its diagnostic retains the original directive and source range.
            return Ok(());
        };
        let path_name = path.actual_path().to_string_lossy().into_owned();
        admitted_targets.insert(path.actual_path().to_path_buf(), path.clone());
        if !identities.contains(path.actual_path()) {
            if *source_count >= effective_source_count_limit(opts.limits) {
                return Err(StartupError::new(
                    StartupErrorKind::LimitExceeded,
                    "include graph exceeds the configured Slang source limit",
                ));
            }
            *source_count += 1;
            let content_limit = remaining
                .checked_sub(path_name.len() as u64)
                .ok_or_else(|| {
                    StartupError::new(
                        StartupErrorKind::LimitExceeded,
                        format!("include path {path_name} exceeds the configured Slang byte limit"),
                    )
                })?;
            let child_text =
                read_bounded_at(&path_name, &path, content_limit, "SystemVerilog source")?;
            *remaining = remaining.saturating_sub(path_name.len() as u64 + child_text.len() as u64);
            identities.insert(path.actual_path().to_path_buf());
            owned.push(OwnedSource::include(path_name.clone(), child_text));
        }

        // A repeated include still executes its directives, but a cycle must
        // stop admission recursion and remain visible to Slang's diagnostics.
        if include_stack
            .iter()
            .any(|entry| entry == path.actual_path())
        {
            return Ok(());
        }
        let Some(child) = owned
            .iter()
            .find(|source| source.name == path_name)
            .cloned()
        else {
            return Ok(());
        };
        include_stack.push(path.actual_path().to_path_buf());
        admit_macro_includes(
            &child.name,
            &child.text,
            opts,
            include_dirs,
            macros,
            Some(&path),
            admitted_targets,
            identities,
            owned,
            source_count,
            remaining,
            include_stack,
            expansion_budget,
            depth + 1,
        )?;
        include_stack.pop();
        Ok(())
    };
    scan_preprocessor_includes(text, macros, expansion_budget, &mut admit)
}

/// Extract literal and macro-expanded includes without treating comments or
/// ordinary strings as directives. Macro expansion is only used to discover
/// bounded, authorized files; Slang remains the source of preprocessing
/// diagnostics and semantic macro identity.
#[cfg(test)]
fn literal_includes(source: &str) -> Vec<String> {
    let mut macros = MacroEnvironment::new();
    preprocessor_includes(source, &mut macros)
}

#[cfg(test)]
fn preprocessor_includes(source: &str, macros: &mut MacroEnvironment) -> Vec<String> {
    let mut includes = Vec::new();
    let expansion_budget = MacroExpansionBudget::new(u64::MAX);
    let mut collect = |target: String, _macros: &mut MacroEnvironment| {
        includes.push(target);
        Ok(())
    };
    scan_preprocessor_includes(source, macros, &expansion_budget, &mut collect)
        .expect("include collection cannot fail");
    includes
}

fn scan_preprocessor_includes<F>(
    source: &str,
    macros: &mut MacroEnvironment,
    expansion_budget: &MacroExpansionBudget,
    on_include: &mut F,
) -> Result<(), StartupError>
where
    F: FnMut(String, &mut MacroEnvironment) -> Result<(), StartupError>,
{
    let mut conditions = Vec::new();
    let mut block_comment = false;
    for (_, raw_line) in logical_preprocessor_lines(source) {
        let line = strip_preprocessor_comments(&raw_line, &mut block_comment);
        let Some((directive, arguments)) = preprocessor_directive(&line) else {
            continue;
        };
        match directive {
            "ifdef" | "ifndef" => {
                let parent_active = conditions
                    .last()
                    .is_none_or(|frame: &ConditionalFrame| frame.active);
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
                    seen_else: false,
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
            _ if !conditions.last().is_none_or(|frame| frame.active) => {}
            "define" => define_macro(arguments, macros),
            "undef" => {
                if let Some(name) = first_macro_identifier(arguments) {
                    macros.remove(name);
                }
            }
            "undefineall" => macros.clear(),
            "include" => {
                if let Some(target) = include_target(arguments, macros, expansion_budget)? {
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
        ) && conditions.last().is_none_or(|frame| frame.active)
        {
            if let Some(target) = expanded_include_target(&line, macros, expansion_budget)? {
                on_include(target, macros)?;
            }
        }
    }
    Ok(())
}

fn logical_preprocessor_lines(source: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;
    let mut continuing = false;
    for (index, raw) in source.split_inclusive('\n').enumerate() {
        if !continuing {
            start_line = index + 1;
        }
        let mut line = raw.strip_suffix('\n').unwrap_or(raw);
        if line.ends_with('\r') {
            line = &line[..line.len() - 1];
        }
        if let Some(stripped) = line.strip_suffix('\\') {
            current.push_str(stripped);
            current.push('\n');
            continuing = true;
        } else {
            current.push_str(line);
            lines.push((start_line, std::mem::take(&mut current)));
            continuing = false;
        }
    }
    if !current.is_empty() {
        lines.push((start_line, current));
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

fn include_target(
    arguments: &str,
    macros: &MacroEnvironment,
    budget: &MacroExpansionBudget,
) -> Result<Option<String>, StartupError> {
    let expanded = expand_macros(arguments, macros, budget)?;
    Ok(parsed_include_target(&expanded))
}

fn parsed_include_target(expanded: &str) -> Option<String> {
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

fn expanded_include_target(
    line: &str,
    macros: &MacroEnvironment,
    budget: &MacroExpansionBudget,
) -> Result<Option<String>, StartupError> {
    let expanded = expand_macros(line, macros, budget)?;
    let Some((directive, arguments)) = preprocessor_directive(&expanded) else {
        return Ok(None);
    };
    Ok((directive == "include")
        .then(|| parsed_include_target(arguments))
        .flatten())
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

fn expand_macros(
    text: &str,
    macros: &MacroEnvironment,
    budget: &MacroExpansionBudget,
) -> Result<String, StartupError> {
    let mut stack = Vec::new();
    expand_macro_text(text, macros, &mut stack, budget, 0)
}

fn expand_macro_text(
    text: &str,
    macros: &MacroEnvironment,
    stack: &mut Vec<String>,
    budget: &MacroExpansionBudget,
    depth: usize,
) -> Result<String, StartupError> {
    budget.step()?;
    if depth > MAX_MACRO_EXPANSION_DEPTH {
        let mut output = String::with_capacity(text.len());
        budget.push_str(&mut output, text)?;
        return Ok(output);
    }
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        budget.step()?;
        if bytes[index] == b'`' && index + 1 < bytes.len() && bytes[index + 1] == b'"' {
            budget.push(&mut output, '"')?;
            index += 2;
            continue;
        }
        if bytes[index] == b'`' && index + 1 < bytes.len() && bytes[index + 1] == b'`' {
            index += 2;
            continue;
        }
        if !bytes[index].is_ascii() {
            let character = text[index..].chars().next().ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Internal,
                    "invalid UTF-8 expansion boundary",
                )
            })?;
            budget.push(&mut output, character)?;
            index += character.len_utf8();
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
            budget.push(&mut output, character)?;
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
            budget.push_str(&mut output, name)?;
            continue;
        }
        let Some(definition) = macros.get(name) else {
            budget.push(&mut output, '`')?;
            budget.push_str(&mut output, name)?;
            continue;
        };
        if stack.iter().any(|active| active == name) {
            budget.push(&mut output, '`')?;
            budget.push_str(&mut output, name)?;
            continue;
        }
        let Some(parameters) = definition.parameters.as_ref() else {
            stack.push(name.to_owned());
            let expanded = expand_macro_text(&definition.body, macros, stack, budget, depth + 1);
            stack.pop();
            budget.push_str(&mut output, &expanded?)?;
            continue;
        };
        if index >= bytes.len() || bytes[index] != b'(' {
            budget.push(&mut output, '`')?;
            budget.push_str(&mut output, name)?;
            continue;
        }
        let Some((arguments, end)) = macro_call_arguments(text, index) else {
            budget.push(&mut output, '`')?;
            budget.push_str(&mut output, name)?;
            continue;
        };
        if arguments.len() != parameters.len() {
            budget.push(&mut output, '`')?;
            budget.push_str(&mut output, name)?;
            continue;
        }
        let expanded_arguments: Vec<_> = arguments
            .iter()
            .map(|argument| expand_macro_text(argument, macros, stack, budget, depth + 1))
            .collect::<Result<_, _>>()?;
        let substituted =
            substitute_macro_arguments(&definition.body, parameters, &expanded_arguments, budget)?;
        stack.push(name.to_owned());
        let expanded = expand_macro_text(&substituted, macros, stack, budget, depth + 1);
        stack.pop();
        budget.push_str(&mut output, &expanded?)?;
        index = end;
    }
    Ok(output)
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

fn substitute_macro_arguments(
    text: &str,
    parameters: &[String],
    arguments: &[String],
    budget: &MacroExpansionBudget,
) -> Result<String, StartupError> {
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        budget.step()?;
        if !bytes[index].is_ascii() {
            let character = text[index..].chars().next().ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Internal,
                    "invalid UTF-8 substitution boundary",
                )
            })?;
            budget.push(&mut output, character)?;
            index += character.len_utf8();
            continue;
        }
        let character = bytes[index] as char;
        if !is_macro_identifier_start(character) {
            budget.push(&mut output, character)?;
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
            budget.push_str(&mut output, &arguments[parameter])?;
        } else {
            budget.push_str(&mut output, name)?;
        }
    }
    Ok(output)
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
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(label: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "llg-compile-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

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
    fn macro_include_scan_preserves_utf8_paths() {
        let source = r#"
            `include "直接/头文件.svh"
            `define HEADER "目录/头文件.svh"
            `include `HEADER
        "#;
        assert_eq!(
            literal_includes(source),
            vec!["直接/头文件.svh".to_owned(), "目录/头文件.svh".to_owned()]
        );
    }

    #[test]
    fn macro_expansion_budget_stops_amplification() {
        let mut macros = MacroEnvironment::new();
        define_macro("DOUBLE(value) value value", &mut macros);
        let budget = MacroExpansionBudget::new(64);
        let error = expand_macros(
            "`DOUBLE(`DOUBLE(`DOUBLE(`DOUBLE(`DOUBLE(x)))))",
            &macros,
            &budget,
        )
        .expect_err("nested macro expansion must exhaust its bounded work budget");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("macro include expansion"));
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

    #[test]
    fn library_name_bytes_count_before_path_admission() {
        let path = temporary_path("library-name-budget");
        std::fs::write(&path, [0xff]).expect("write invalid source fixture");
        let path_name = path
            .canonicalize()
            .expect("canonical source fixture")
            .to_string_lossy()
            .into_owned();
        let source = OwnedSource::compilation_unit("top.sv", "module top; endmodule\n");
        let library_source = LibrarySource::new(
            "library.sv",
            "module library; endmodule\n",
            "L".repeat(path_name.len() + 2),
        );
        let admitted_without_library_name = source.name.len() as u64
            + source.text.len() as u64
            + library_source.name.len() as u64
            + library_source.text.len() as u64;
        let limits = Limits {
            max_source_bytes: admitted_without_library_name + path_name.len() as u64 + 1,
            ..Limits::default()
        };
        let result = compile(&CompileOpts {
            sources: vec![source],
            files: vec![path_name],
            library_sources: vec![library_source],
            limits,
            ..CompileOpts::default()
        });
        assert_eq!(
            result
                .expect_err("library name must consume the combined source budget")
                .kind(),
            StartupErrorKind::LimitExceeded
        );
        std::fs::remove_file(path).expect("remove invalid source fixture");
    }

    #[test]
    fn explicit_library_file_name_bytes_count_before_path_read() {
        let path = temporary_path("library-file-name-budget");
        std::fs::write(&path, [0xff]).expect("write invalid library source fixture");
        let path_name = path
            .canonicalize()
            .expect("canonical library source fixture")
            .to_string_lossy()
            .into_owned();
        let source = OwnedSource::compilation_unit("top.sv", "module top; endmodule\n");
        let library_name = "L".repeat(path_name.len() + 2);
        let limits = Limits {
            max_source_bytes: source.name.len() as u64
                + source.text.len() as u64
                + path_name.len() as u64
                + 1,
            ..Limits::default()
        };
        let result = compile(&CompileOpts {
            sources: vec![source],
            library_files: vec![format!("{library_name}={path_name}")],
            limits,
            ..CompileOpts::default()
        });
        assert_eq!(
            result
                .expect_err("explicit library names must consume the source budget")
                .kind(),
            StartupErrorKind::LimitExceeded
        );
        std::fs::remove_file(path).expect("remove invalid library source fixture");
    }

    #[test]
    fn map_library_name_bytes_count_before_expanded_source_read() {
        let root = temporary_path("library-map-name-budget");
        std::fs::create_dir_all(&root).expect("create library map budget root");
        let top = root.join("top.sv");
        let map = root.join("root.map");
        let source = root.join("mapped.sv");
        std::fs::write(&top, "module top; endmodule\n").expect("write top source");
        std::fs::write(&source, [0xff]).expect("write invalid mapped source");
        let source_name = source
            .canonicalize()
            .expect("canonical mapped source")
            .to_string_lossy()
            .into_owned();
        let library_name = "L".repeat(source_name.len() + 2);
        let map_text = format!("library {library_name} mapped.sv;\n");
        std::fs::write(&map, &map_text).expect("write library map");
        let top_name = top
            .canonicalize()
            .expect("canonical top source")
            .to_string_lossy()
            .into_owned();
        let map_name = map
            .canonicalize()
            .expect("canonical library map")
            .to_string_lossy()
            .into_owned();
        let limits = Limits {
            max_source_bytes: top_name.len() as u64
                + "module top; endmodule\n".len() as u64
                + map_name.len() as u64
                + map_text.len() as u64
                + source_name.len() as u64
                + 1,
            ..Limits::default()
        };
        let result = compile(&CompileOpts {
            files: vec![top_name],
            library_map_files: vec![map_name],
            limits,
            ..CompileOpts::default()
        });
        assert_eq!(
            result
                .expect_err("map library names must consume the source budget")
                .kind(),
            StartupErrorKind::LimitExceeded
        );
        std::fs::remove_dir_all(root).expect("remove library map budget root");
    }

    #[test]
    fn custom_source_budget_is_clamped_to_the_native_hard_limit() {
        assert_eq!(
            effective_source_byte_limit(Limits {
                max_source_bytes: NATIVE_HARD_MAX_SOURCE_BYTES.saturating_add(1),
                ..Limits::default()
            }),
            NATIVE_HARD_MAX_SOURCE_BYTES
        );
    }

    #[test]
    fn library_pattern_match_ceiling_is_checked_before_file_reads() {
        let root = temporary_path("library-pattern-limit");
        std::fs::create_dir_all(&root).expect("create library pattern root");
        std::fs::write(root.join("a.sv"), "module a; endmodule\n").expect("write first source");
        std::fs::write(root.join("b.sv"), "module b; endmodule\n").expect("write second source");
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let error = expand_library_pattern(&root, "*.sv", 1, &mut work)
            .expect_err("pattern exceeding its source ceiling must fail");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        std::fs::remove_dir_all(root).expect("remove library pattern root");
    }

    #[test]
    fn logical_map_matching_bounds_long_wildcard_and_name_work() {
        let mut work = LibraryMapWorkBudget::new(4_096);
        let pattern = format!("{}*", "a".repeat(4_096));
        let candidate = PathBuf::from("a".repeat(4_096));
        let error = map_pattern_matches(Path::new("."), &pattern, &candidate, &mut work)
            .expect_err("long logical wildcard work must be bounded");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("logical map"));
    }

    #[test]
    fn logical_map_byte_matching_charges_replayed_suffix_steps() {
        let value = "a".repeat(20_000);
        let pattern = format!("*{}b", "a".repeat(6_000));
        let mut work = LibraryMapWorkBudget::new(50_000);
        let error = wildcard_component_matches(&value, &pattern, &mut work)
            .expect_err("replayed wildcard suffix work must be bounded");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("byte matching step"));
    }

    #[test]
    fn logical_recursive_matching_charges_literal_backtracking_bytes() {
        let pattern = vec![
            LogicalMapPatternComponent {
                value: "**".to_owned(),
                wildcards: true,
            },
            LogicalMapPatternComponent {
                value: "literal".repeat(128),
                wildcards: false,
            },
        ];
        let candidate = vec!["candidate".repeat(128); 64];
        let mut work = LibraryMapWorkBudget::new(10_000);
        let error = logical_path_pattern_matches(&pattern, &candidate, &mut work)
            .expect_err("recursive literal backtracking must consume comparison bytes");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("literal comparison") || error.contains("component comparison"));
    }

    #[test]
    fn logical_map_star_precedes_literal_matching() {
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(wildcard_component_matches("*", "*?", &mut work)
            .expect("wildcard matching should succeed"));

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(
            map_pattern_matches(Path::new("."), "dir/*?", Path::new("dir/*"), &mut work,)
                .expect("logical map matching should succeed")
        );
    }

    #[test]
    fn library_map_token_work_is_charged_before_semicolon_amplification() {
        let text = ";".repeat(65);
        let mut work = LibraryMapWorkBudget::new(64);
        let error = parse_library_map(&text, &mut work)
            .expect_err("token amplification must be bounded before token allocation");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("tokenization"));
    }

    #[test]
    fn in_memory_library_map_text_is_charged_before_cloning() {
        let text = ";".repeat(65);
        let maps = [OwnedSource::include("root.map", text)];
        let mut sources = Vec::new();
        let mut library_sources = Vec::new();
        let mut source_count = maps.len();
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(64);
        let error = admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            128,
            &mut work,
        )
        .expect_err("in-memory map text must be bounded before cloning");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("in-memory map text clone"));
        assert!(sources.is_empty());
        assert!(library_sources.is_empty());
    }

    #[test]
    fn large_mapped_buffers_use_byte_admission_without_exhausting_structural_work() {
        let map_text = format!("/*{}*/\nlibrary L mapped.sv;\n", "m".repeat(384 * 1024));
        let source_text = format!("module mapped; /*{}*/ endmodule\n", "s".repeat(768 * 1024));
        let sources = [
            OwnedSource::compilation_unit("top.sv", "module top; endmodule\n"),
            OwnedSource::compilation_unit("mapped.sv", source_text),
        ];
        let opts = CompileOpts {
            library_maps: vec![OwnedSource::include("root.map", map_text)],
            limits: Limits {
                max_source_bytes: 2 * 1024 * 1024,
                ..Limits::default()
            },
            ..CompileOpts::default()
        };

        let result = compile_sources(&sources, &opts).expect(
            "mapped source and map content within the public source budget should be admitted",
        );
        assert!(!result.snapshot.has_errors());
    }

    #[test]
    fn large_mapped_buffers_still_enforce_the_public_source_byte_limit() {
        let map_text = format!("/*{}*/\nlibrary L mapped.sv;\n", "m".repeat(384 * 1024));
        let source_text = format!("module mapped; /*{}*/ endmodule\n", "s".repeat(768 * 1024));
        let source_bytes = "top.sv".len()
            + "module top; endmodule\n".len()
            + "mapped.sv".len()
            + source_text.len();
        let map_bytes = "root.map".len() + map_text.len();
        let sources = [
            OwnedSource::compilation_unit("top.sv", "module top; endmodule\n"),
            OwnedSource::compilation_unit("mapped.sv", source_text),
        ];
        let opts = CompileOpts {
            library_maps: vec![OwnedSource::include("root.map", map_text)],
            limits: Limits {
                max_source_bytes: (source_bytes + map_bytes - 1) as u64,
                ..Limits::default()
            },
            ..CompileOpts::default()
        };

        let error = compile_sources(&sources, &opts)
            .expect_err("mapped content above the public source budget must be rejected");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("source bytes"));
    }

    #[test]
    fn filesystem_library_map_text_is_charged_before_reading() {
        let path = temporary_path("library-map-text-budget");
        let text = ";".repeat(65);
        std::fs::write(&path, &text).expect("write map text budget fixture");
        let path_name = path.to_string_lossy().into_owned();
        let mut work = LibraryMapWorkBudget::new(64);
        let error = read_bounded_library_map(&path_name, 1_024, &mut work)
            .expect_err("filesystem map text must be bounded before reading");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("library map text admission"));
        std::fs::remove_file(path).expect("remove map text budget fixture");
    }

    #[cfg(unix)]
    #[test]
    fn non_regular_library_map_files_are_rejected_before_reading() {
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let error = read_bounded_library_map("/dev/zero", 64, &mut work)
            .expect_err("special library-map files must be rejected");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("not a regular file"));
        assert!(work.used < 64);
    }

    #[cfg(unix)]
    #[test]
    fn fifo_library_map_is_rejected_without_blocking() {
        use std::process::Command;

        let path = temporary_path("library-map-fifo");
        let status = Command::new("mkfifo").arg(&path).status();
        let Ok(status) = status else {
            return;
        };
        if !status.success() {
            return;
        }

        let path_name = path.to_string_lossy().into_owned();
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let error = read_bounded_library_map(&path_name, 64, &mut work)
            .expect_err("FIFO must be rejected without opening a blocking descriptor");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("not a regular file"));
        std::fs::remove_file(path).expect("remove library-map FIFO");
    }

    #[cfg(unix)]
    #[test]
    fn fifo_source_readers_are_rejected_without_blocking() {
        use std::process::Command;

        let path = temporary_path("source-fifo");
        let status = Command::new("mkfifo").arg(&path).status();
        let Ok(status) = status else {
            return;
        };
        if !status.success() {
            return;
        }

        let path_name = path.to_string_lossy().into_owned();
        let direct = read_bounded(&path_name, 64).expect_err("FIFO source must be rejected");
        assert_eq!(direct.kind(), StartupErrorKind::Input);
        assert!(direct.contains("not a regular file"));

        let parsed = parse_only(&path_name, &[]).expect_err("parse_only must reject FIFO input");
        assert_eq!(parsed.kind(), StartupErrorKind::Input);
        assert!(parsed.contains("not a regular file"));

        let positional = compile(&CompileOpts {
            files: vec![path_name.clone()],
            ..CompileOpts::default()
        })
        .expect_err("positional FIFO input must be rejected");
        assert_eq!(positional.kind(), StartupErrorKind::Input);
        assert!(positional.contains("not a regular file"));

        let library = compile(&CompileOpts {
            library_files: vec![format!("L={path_name}")],
            ..CompileOpts::default()
        })
        .expect_err("--libfile FIFO input must be rejected");
        assert_eq!(library.kind(), StartupErrorKind::Input);
        assert!(library.contains("not a regular file"));

        std::fs::remove_file(path).expect("remove source FIFO");
    }

    #[cfg(unix)]
    #[test]
    fn macro_include_reader_rejects_fifo_replacement_without_blocking() {
        use std::process::Command;

        let root = temporary_path("macro-include-fifo");
        std::fs::create_dir_all(&root).expect("create macro include root");
        let including = root.join("top.sv");
        let child = root.join("child.svh");
        std::fs::write(&child, "`define CHILD 1\n").expect("write include fixture");
        let resolved = resolve_include(&including, "child.svh", &[])
            .expect("regular include should resolve before the replacement race");
        std::fs::remove_file(&child).expect("remove include fixture");
        let status = Command::new("mkfifo").arg(&child).status();
        let Ok(status) = status else {
            std::fs::remove_dir_all(root).expect("remove macro include root");
            return;
        };
        if !status.success() {
            std::fs::remove_dir_all(root).expect("remove macro include root");
            return;
        }

        let path_name = resolved.actual_path().to_string_lossy().into_owned();
        let error = read_bounded_at(&path_name, &resolved, 64, "SystemVerilog source")
            .expect_err("macro include replacement must be rejected before blocking");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(
            error.contains("changed from admitted target") || error.contains("not a regular file")
        );
        std::fs::remove_dir_all(root).expect("remove macro include root");
    }

    #[test]
    fn regular_file_truncation_race_is_rejected() {
        let path = temporary_path("source-truncation-race");
        std::fs::write(&path, b"original").expect("write truncation fixture");
        let path_name = path.to_string_lossy().into_owned();
        let (mut file, metadata) =
            open_regular_file(&path_name, "test source").expect("open truncation fixture");
        std::fs::write(&path, b"x").expect("truncate source fixture");
        let error = read_regular_file_contents(
            &mut file,
            metadata.len(),
            64,
            &path_name,
            "test source",
            None,
            "test read",
            "test final metadata",
        )
        .expect_err("truncated regular files must not be admitted");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("changed or was truncated"));
        std::fs::remove_file(path).expect("remove truncation fixture");
    }

    #[test]
    fn regular_file_growth_after_open_is_rejected() {
        let path = temporary_path("source-growth-race");
        std::fs::write(&path, b"old").expect("write growth fixture");
        let path_name = path.to_string_lossy().into_owned();
        let (mut file, metadata) =
            open_regular_file(&path_name, "test source").expect("open growth fixture");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open growth fixture for append")
            .write_all(b"new")
            .expect("grow source fixture");
        let error = read_regular_file_contents(
            &mut file,
            metadata.len(),
            64,
            &path_name,
            "test source",
            None,
            "test read",
            "test final metadata",
        )
        .expect_err("grown regular files must not be admitted");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("changed or was truncated"));
        std::fs::remove_file(path).expect("remove growth fixture");
    }

    #[test]
    fn library_map_read_iterations_use_structural_budget() {
        let path = temporary_path("library-map-read-iterations");
        std::fs::write(&path, b"library L source.sv;\n").expect("write iteration fixture");
        let path_name = path.to_string_lossy().into_owned();
        let mut work = LibraryMapWorkBudget::with_allocation_limit(2, u64::MAX);
        let error = read_bounded_library_map(&path_name, 64, &mut work)
            .expect_err("each library-map read iteration must consume structural work");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("incremental read"));
        std::fs::remove_file(path).expect("remove iteration fixture");
    }

    #[test]
    fn library_map_parser_charges_large_ast_allocations() {
        let library = "L".repeat(8_000);
        let text = format!("library {library} tiny.sv;\n");
        let mut work = LibraryMapWorkBudget::with_allocation_limit(
            MAX_LIBRARY_MAP_WORK,
            text.len() as u64 + 4_000,
        );
        let error = parse_library_map(&text, &mut work)
            .expect_err("large library-name AST allocations must be bounded");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("library-name clone") || error.contains("allocation budget"));
    }

    #[test]
    fn filesystem_child_path_bytes_are_charged_before_join() {
        let parent = PathBuf::from("p".repeat(128));
        let name = std::ffi::OsString::from("n".repeat(128));
        let mut work = LibraryMapWorkBudget::new(200);
        charge_library_pattern_entry_name(&mut work, &name)
            .expect("entry name should fit before child path accounting");
        let error = charge_library_pattern_child_path(&mut work, &parent, &name)
            .expect_err("parent path bytes must be charged before joining the child");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("child path bytes"));
    }

    #[test]
    fn filesystem_nonmatching_entry_names_share_the_byte_budget() {
        let root = temporary_path("library-pattern-entry-names");
        std::fs::create_dir_all(&root).expect("create entry-name budget root");
        for index in 0..3 {
            let name = format!("nonmatching-{index}-{}.sv", "x".repeat(180));
            std::fs::write(root.join(name), "module source; endmodule\n")
                .expect("write long nonmatching source");
        }
        let mut work = LibraryMapWorkBudget::new(512);
        let error = expand_library_pattern(&root, "match-*.sv", 8, &mut work)
            .expect_err("long nonmatching entries must consume shared path-byte work");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("directory entry name"));
        std::fs::remove_dir_all(root).expect("remove entry-name budget root");
    }

    #[test]
    fn wildcard_scan_skips_matching_files_when_components_remain() {
        let root = temporary_path("library-pattern-file-branch");
        let directory = root.join("candidate");
        std::fs::create_dir_all(&directory).expect("create wildcard directory");
        let source = directory.join("source.sv");
        std::fs::write(&source, "module source; endmodule\n").expect("write wildcard source");
        std::fs::write(root.join("README"), "not a directory\n").expect("write README");

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let matches = expand_library_pattern(&root, "*/*.sv", 8, &mut work)
            .expect("ordinary files matched by an intermediate wildcard must be skipped");
        assert_eq!(
            matches,
            vec![source.canonicalize().expect("canonical wildcard source")]
        );
        std::fs::remove_dir_all(root).expect("remove wildcard file branch root");
    }

    #[test]
    fn filesystem_map_base_wildcards_are_literal() {
        let root = temporary_path("library-pattern-literal-base");
        let literal = root.join("a*b");
        let wildcard_match = root.join("axb");
        std::fs::create_dir_all(&literal).expect("create literal wildcard directory");
        std::fs::create_dir_all(&wildcard_match).expect("create wildcard sibling directory");
        let literal_source = literal.join("source.sv");
        let sibling_source = wildcard_match.join("source.sv");
        let map = literal.join("root.map");
        std::fs::write(&literal_source, "module literal_source; endmodule\n")
            .expect("write literal source");
        std::fs::write(&sibling_source, "module sibling_source; endmodule\n")
            .expect("write sibling source");
        std::fs::write(&map, "library L source.sv;\n").expect("write literal-base map");

        let mut identities = HashSet::new();
        let mut library_sources = Vec::new();
        let mut source_count = 0;
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_library_maps(
            &CompileOpts {
                library_map_files: vec![map
                    .canonicalize()
                    .expect("canonical literal-base map")
                    .to_string_lossy()
                    .into_owned()],
                ..CompileOpts::default()
            },
            &mut identities,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            &mut work,
        )
        .expect("literal wildcard base should admit its own source");
        assert_eq!(library_sources.len(), 1);
        assert_eq!(
            library_sources[0].name,
            literal_source
                .canonicalize()
                .expect("canonical literal source")
                .to_string_lossy()
        );
        assert_eq!(library_sources[0].library, "L");
        std::fs::remove_dir_all(root).expect("remove literal-base root");
    }

    #[test]
    fn in_memory_map_base_wildcards_are_literal() {
        let maps = [OwnedSource::include(
            "a*b/root.map",
            "library L source.sv;\n",
        )];
        let mut sources = vec![
            OwnedSource::compilation_unit("a*b/source.sv", ""),
            OwnedSource::compilation_unit("axb/source.sv", ""),
        ];
        let mut library_sources = Vec::new();
        let mut source_count = sources.len();
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            128,
            &mut work,
        )
        .expect("literal wildcard in-memory base should admit its own source");
        assert_eq!(
            library_sources
                .iter()
                .map(|source| source.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a*b/source.sv"]
        );
        assert_eq!(
            sources
                .iter()
                .map(|source| source.name.as_str())
                .collect::<Vec<_>>(),
            vec!["axb/source.sv"]
        );
    }

    #[test]
    fn canonical_path_resolution_uses_a_transient_reservation() {
        let root = temporary_path("library-pattern-canonical-reservation");
        std::fs::create_dir_all(&root).expect("create canonical reservation root");
        std::fs::write(root.join("source.sv"), "module source; endmodule\n")
            .expect("write canonical reservation source");
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_PATH_BYTES as u64);
        let matches = expand_library_pattern(&root, "source.sv", 8, &mut work)
            .expect("canonicalization reservation must not accumulate per match");
        assert_eq!(matches.len(), 1);
        assert!(work.used < MAX_LIBRARY_PATH_BYTES as u64);
        std::fs::remove_dir_all(root).expect("remove canonical reservation root");
    }

    #[test]
    fn filesystem_pattern_canonicalization_scales_past_transient_ceiling() {
        let root = temporary_path("library-pattern-many-matches");
        std::fs::create_dir_all(&root).expect("create many-match root");
        for index in 0..96 {
            std::fs::write(
                root.join(format!("source-{index}.sv")),
                "module source; endmodule\n",
            )
            .expect("write many-match source");
        }

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let matches = expand_library_pattern(&root, "*.sv", 128, &mut work)
            .expect("ordinary map expansion should not spend one path ceiling per match");
        assert_eq!(matches.len(), 96);

        std::fs::remove_dir_all(root).expect("remove many-match root");
    }

    #[test]
    fn in_memory_library_map_quoted_unicode_path_preserves_utf8() {
        let maps = [OwnedSource::include("root.map", "library L \"dir/é.sv\";")];
        let mut sources = vec![OwnedSource::compilation_unit("dir/é.sv", "")];
        let mut library_sources = Vec::new();
        let mut source_count = sources.len();
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            128,
            &mut work,
        )
        .expect("quoted UTF-8 in-memory map path should match");
        assert!(sources.is_empty());
        assert_eq!(library_sources.len(), 1);
        assert_eq!(library_sources[0].name, "dir/é.sv");
        assert_eq!(library_sources[0].library, "L");
    }

    #[test]
    fn library_map_lexer_preserves_unc_and_windows_path_spelling() {
        let text = r#"
            // this comment must remain a comment
            library unc //server/share/source.sv;
            library drive "C:\work\rtl\\core\"v1.sv";
            library quoted "//server/share/quoted.sv";
            library drive_unquoted C:\work\rtl\source.sv;
        "#;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse path spellings");
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].library, "unc");
        assert_eq!(entries[0].patterns, vec!["//server/share/source.sv"]);
        assert_eq!(entries[1].library, "drive");
        assert_eq!(entries[1].patterns, vec![r#"C:\work\rtl\\core\"v1.sv"#]);
        assert_eq!(entries[2].library, "quoted");
        assert_eq!(entries[2].patterns, vec!["//server/share/quoted.sv"]);
        assert_eq!(entries[3].library, "drive_unquoted");
        assert_eq!(entries[3].patterns, vec![r#"C:\work\rtl\source.sv"#]);
    }

    #[test]
    fn library_map_lexer_keeps_unambiguous_comments() {
        let text = "library L source.sv; // trailing comment\n// whole-line comment\nlibrary M other.sv;\n";
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse comments");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].patterns, vec!["source.sv"]);
        assert_eq!(entries[1].patterns, vec!["other.sv"]);
    }

    #[test]
    fn library_map_lexer_uses_path_context_for_unc_and_comment_paths() {
        let text = "library L source.sv; //comment/path\nlibrary M //server/share/source.sv;\n";
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse path context");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].patterns, vec!["source.sv"]);
        assert_eq!(entries[1].patterns, vec!["//server/share/source.sv"]);
    }

    #[test]
    fn library_map_lexer_keeps_block_comments_and_absolute_star_paths_distinct() {
        let text = "/* header comment */\nlibrary L /*/*.sv;\n";
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse absolute star path");
        assert_eq!(entries[0].patterns, vec!["/*/*.sv"]);
    }

    #[test]
    fn library_map_lexer_keeps_empty_comments_and_absolute_recursive_paths_distinct() {
        let text = "library L /**/*.sv;\nlibrary M /**/source.sv;\n";
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse recursive path and empty comment");
        assert_eq!(entries[0].patterns, vec!["/**/*.sv"]);
        assert_eq!(entries[1].patterns, vec!["source.sv"]);
    }

    #[test]
    fn quoted_incdir_token_remains_a_literal_pattern() {
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(r#"library L "-incdir";"#, &mut work).expect("quoted incdir path");
        assert_eq!(entries[0].patterns, vec!["-incdir"]);
    }

    #[test]
    fn line_comments_keep_unc_paths_only_in_path_context() {
        let text = "//server/share is a comment here\nlibrary L //server/share/source.sv;";
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let ParsedLibraryMap { entries, .. } =
            parse_library_map(text, &mut work).expect("parse UNC context");
        assert_eq!(entries[0].patterns, vec!["//server/share/source.sv"]);
    }

    #[test]
    fn equal_rank_map_library_comparison_is_budgeted() {
        // Explicit assignments now outrank every map pattern without a
        // comparison; an equal-rank repeat still compares both library names
        // before coalescing, and that comparison must consume both lengths.
        let library = "L".repeat(256);
        let maps = [OwnedSource::include(
            "root.map",
            format!("library {library} candidate.sv, candidate.sv;\n"),
        )];
        let comparison_failure = |limit| {
            let mut sources = vec![OwnedSource::compilation_unit("candidate.sv", "")];
            let mut library_sources = Vec::new();
            let mut source_count = 0;
            let mut remaining = u64::MAX;
            let mut work = LibraryMapWorkBudget::with_allocation_limit(limit, u64::MAX);
            admit_in_memory_library_maps(
                &maps,
                &mut sources,
                &mut library_sources,
                &mut source_count,
                &mut remaining,
                128,
                &mut work,
            )
            .err()
            .filter(|error| error.contains("map library comparison"))
        };
        let first = (0..MAX_LIBRARY_MAP_WORK)
            .find(|&limit| comparison_failure(limit).is_some())
            .expect("an equal-rank repeat must charge its library comparison");
        let error = comparison_failure(first).unwrap();
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        let length = library.len() as u64;
        for limit in [first + length - 1, first + length, first + 2 * length - 1] {
            assert!(
                comparison_failure(limit).is_some(),
                "limit {limit} must still fail inside the comparison"
            );
        }
        assert!(comparison_failure(first + 2 * length).is_none());
    }

    #[test]
    fn filesystem_map_parent_components_resolve_from_the_map_directory() {
        // V 13.2.1 / SV 33.3.1: relative map paths, including leading `..`,
        // are relative to the map file, for both library patterns and includes.
        let root = temporary_path("library-map-parent");
        let nested = root.join("maps").join("nested");
        std::fs::create_dir_all(&nested).expect("create nested map directory");
        std::fs::create_dir_all(root.join("rtl")).expect("create source directory");
        let source = root.join("rtl").join("cell.sv");
        std::fs::write(&source, "module mapped; endmodule\n").expect("write mapped source");
        std::fs::write(nested.join("child.map"), "library L ../../rtl/*.sv;\n")
            .expect("write child map");
        let map = root.join("maps").join("root.map");
        std::fs::write(&map, "include ./nested/../nested/child.map;\n").expect("write root map");
        let opts = CompileOpts {
            library_map_files: vec![map
                .canonicalize()
                .expect("canonical root map")
                .to_string_lossy()
                .into_owned()],
            ..CompileOpts::default()
        };
        let mut identities = HashSet::new();
        let mut library_sources = Vec::new();
        let mut source_count = 0;
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_library_maps(
            &opts,
            &mut identities,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            &mut work,
        )
        .expect("parent-relative map paths resolve from their map");
        assert_eq!(library_sources.len(), 1);
        assert_eq!(
            library_sources[0].name,
            source
                .canonicalize()
                .expect("canonical mapped source")
                .to_string_lossy()
        );
        assert_eq!(library_sources[0].library, "L");
        std::fs::remove_dir_all(root).expect("remove parent map root");
    }

    #[test]
    fn filesystem_library_map_quoted_unicode_path_preserves_utf8() {
        let root = temporary_path("library-map-unicode");
        std::fs::create_dir_all(&root).expect("create Unicode map root");
        let source = root.join("é.sv");
        let map = root.join("root.map");
        std::fs::write(&source, "module mapped; endmodule\n").expect("write Unicode source");
        std::fs::write(&map, "library L \"é.sv\";\n").expect("write Unicode map");
        let map_name = map
            .canonicalize()
            .expect("canonical Unicode map")
            .to_string_lossy()
            .into_owned();
        let opts = CompileOpts {
            library_map_files: vec![map_name],
            ..CompileOpts::default()
        };
        let mut identities = HashSet::new();
        let mut library_sources = Vec::new();
        let mut source_count = 0;
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_library_maps(
            &opts,
            &mut identities,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            &mut work,
        )
        .expect("quoted UTF-8 filesystem map path should match");
        assert_eq!(library_sources.len(), 1);
        assert_eq!(
            library_sources[0].name,
            source
                .canonicalize()
                .expect("canonical Unicode source")
                .to_string_lossy()
        );
        assert_eq!(library_sources[0].library, "L");
        std::fs::remove_dir_all(root).expect("remove Unicode map root");
    }

    #[test]
    fn library_map_ordering_charges_key_comparison_work() {
        let mut work = LibraryMapWorkBudget::new(128);
        let keys = vec![1_024_usize; 4];
        let error = charge_key_comparison_work(&mut work, keys, true, "test ordering comparisons")
            .expect_err("long ordering keys must consume comparison work");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("ordering key scan"));
    }

    #[test]
    fn in_memory_source_removal_follows_admission_charge() {
        let maps = [OwnedSource::include(
            "root.map",
            "library L candidate.sv;\n",
        )];
        let run = |limit| {
            let mut sources = vec![OwnedSource::compilation_unit(
                "candidate.sv",
                "module candidate; endmodule\n",
            )];
            let mut library_sources = Vec::new();
            let mut source_count = sources.len();
            let mut remaining = u64::MAX;
            let mut work = LibraryMapWorkBudget::new(limit);
            let result = admit_in_memory_library_maps(
                &maps,
                &mut sources,
                &mut library_sources,
                &mut source_count,
                &mut remaining,
                128,
                &mut work,
            );
            (result, sources, library_sources)
        };

        for limit in 0..MAX_LIBRARY_MAP_WORK {
            let (result, sources, library_sources) = run(limit);
            let Err(error) = result else {
                continue;
            };
            if error.contains("in-memory library source admission") {
                assert_eq!(sources.len(), 1);
                assert!(library_sources.is_empty());
                assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
                return;
            }
        }
        panic!("test budget did not stop before source removal");
    }

    #[test]
    fn in_memory_source_admission_is_deterministic_for_both_input_orders() {
        let admit = |names: &[&str]| {
            let maps = [OwnedSource::include(
                "root.map",
                "library L candidate/*.sv;",
            )];
            let mut sources = names
                .iter()
                .map(|name| OwnedSource::compilation_unit(*name, ""))
                .collect::<Vec<_>>();
            let mut library_sources = Vec::new();
            let mut source_count = sources.len();
            let mut remaining = u64::MAX;
            let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
            admit_in_memory_library_maps(
                &maps,
                &mut sources,
                &mut library_sources,
                &mut source_count,
                &mut remaining,
                128,
                &mut work,
            )
            .expect("library map source admission");
            (
                library_sources
                    .into_iter()
                    .map(|source| source.name)
                    .collect::<Vec<_>>(),
                sources
                    .into_iter()
                    .map(|source| source.name)
                    .collect::<Vec<_>>(),
            )
        };

        let first = admit(&["top.sv", "candidate/z.sv", "candidate/a.sv"]);
        let second = admit(&["candidate/z.sv", "candidate/a.sv", "top.sv"]);
        assert_eq!(first.0, vec!["candidate/z.sv", "candidate/a.sv"]);
        assert_eq!(first.1, vec!["top.sv"]);
        assert_eq!(second, first);
    }

    #[test]
    fn logical_map_patterns_and_candidate_scans_share_one_work_budget() {
        let mut work = LibraryMapWorkBudget::new(256);
        let mut exhausted = None;
        for index in 0..32 {
            let name = format!("candidate-{index}.sv");
            let result = map_pattern_matches(
                Path::new("."),
                &format!("candidate-{index}.s?"),
                Path::new(&name),
                &mut work,
            );
            if let Err(error) = result {
                exhausted = Some(error);
                break;
            }
        }
        let error = exhausted.expect("candidate scans must consume their shared work budget");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("work budget"));
    }

    #[test]
    fn filesystem_pattern_traversal_shares_work_across_patterns() {
        let root = temporary_path("library-pattern-shared-budget");
        std::fs::create_dir_all(&root).expect("create shared budget root");
        std::fs::write(root.join("source.sv"), "module source; endmodule\n")
            .expect("write shared budget source");

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        expand_library_pattern(&root, "*.sv", 8, &mut work)
            .expect("first pattern should fit the shared work budget");
        let consumed = work.used;
        work.limit = consumed.saturating_add("*.sv".len() as u64);
        let error = expand_library_pattern(&root, "*.sv", 8, &mut work)
            .expect_err("the second pattern must use the remaining shared budget");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("filesystem"));

        std::fs::remove_dir_all(root).expect("remove shared budget root");
    }

    #[test]
    fn filesystem_pattern_component_depth_is_bounded_before_recursion() {
        let root = temporary_path("library-pattern-depth");
        std::fs::create_dir_all(&root).expect("create deep pattern root");
        let pattern = format!("*/{}source.sv", "a/".repeat(20_000));
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let error = expand_library_pattern(&root, &pattern, 8, &mut work)
            .expect_err("deep literal suffix must hit the component-depth limit");
        assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
        assert!(error.contains("component depth"));
        std::fs::remove_dir_all(root).expect("remove deep pattern root");
    }

    #[test]
    fn logical_map_paths_preserve_leading_parent_components() {
        assert_eq!(
            logical_path_key(
                Path::new("../../x"),
                &mut LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK),
                "test path normalization",
            )
            .expect("logical path normalization"),
            vec!["..".to_owned(), "..".to_owned(), "x".to_owned()]
        );
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(
            !map_pattern_matches(Path::new("."), "../../x", Path::new("x"), &mut work)
                .expect("lexical map matching must not fail")
        );
    }

    #[test]
    fn logical_map_matching_preserves_drive_and_unc_prefixes() {
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new(r"C:\work"),
            "*.sv",
            Path::new(r"C:\work\source.sv"),
            &mut work,
        )
        .expect("drive path matching"));
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new(r"\\server\share"),
            "*.sv",
            Path::new(r"\\server\share\source.sv"),
            &mut work,
        )
        .expect("UNC path matching"));
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new("."),
            r"C:\work\*.sv",
            Path::new(r"C:\work\source.sv"),
            &mut work,
        )
        .expect("absolute drive pattern matching"));
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new("."),
            r"\\server\share\*.sv",
            Path::new(r"\\server\share\source.sv"),
            &mut work,
        )
        .expect("absolute UNC pattern matching"));
    }

    #[test]
    fn logical_map_parent_preserves_drive_and_unc_roots() {
        assert_eq!(logical_map_parent(r"C:\root.map"), PathBuf::from(r"C:\"));
        assert_eq!(
            logical_map_parent(r"\\server\share\root.map"),
            PathBuf::from(r"\\server\share")
        );

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new(r"C:\"),
            "*.sv",
            Path::new(r"C:\source.sv"),
            &mut work,
        )
        .expect("drive-root relative map pattern matching"));
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        assert!(map_pattern_matches(
            Path::new(r"\\server\share"),
            "*.sv",
            Path::new(r"\\server\share\source.sv"),
            &mut work,
        )
        .expect("UNC-root relative map pattern matching"));
    }

    #[cfg(unix)]
    #[test]
    fn opened_map_anchor_rejects_parent_replacement_before_child_open() {
        let root = temporary_path("secure-anchor-race");
        let moved = temporary_path("secure-anchor-moved");
        let external = temporary_path("secure-anchor-external");
        std::fs::create_dir_all(&root).expect("create secure anchor");
        std::fs::create_dir_all(&external).expect("create external anchor");
        std::fs::write(root.join("source.sv"), "module source; endmodule\n")
            .expect("write admitted source");
        std::fs::write(external.join("source.sv"), "module external; endmodule\n")
            .expect("write external source");
        let anchor = secure_fs::open_path(&root).expect("open stable anchor handle");
        std::fs::rename(&root, &moved).expect("move admitted anchor");
        std::fs::rename(&external, &root).expect("replace admitted anchor path");

        let error = anchor
            .open_child(std::ffi::OsStr::new("source.sv"))
            .expect_err("descriptor-relative child open must retain the original anchor");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);

        std::fs::remove_dir_all(moved).expect("remove moved admitted anchor");
        std::fs::remove_dir_all(root).expect("remove replacement anchor");
    }

    #[cfg(unix)]
    #[test]
    fn reopened_source_rejects_parent_replacement_before_read() {
        let root = temporary_path("secure-source-race");
        let moved = temporary_path("secure-source-moved");
        let external = temporary_path("secure-source-external");
        std::fs::create_dir_all(&root).expect("create source root");
        std::fs::create_dir_all(&external).expect("create external source root");
        let path = root.join("source.sv");
        std::fs::write(&path, "module source; endmodule\n").expect("write source");
        std::fs::write(external.join("source.sv"), "module external; endmodule\n")
            .expect("write external source");
        let expected = secure_fs::open_path(&path)
            .expect("capture source handle target")
            .admitted_target();
        std::fs::rename(&root, &moved).expect("move source root");
        std::fs::rename(&external, &root).expect("replace source root path");

        let path_name = path.to_string_lossy().into_owned();
        let error = open_regular_file_at(&path_name, &expected, "test source")
            .expect_err("reopened source must retain its admitted target");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("changed from admitted target"));

        std::fs::remove_dir_all(moved).expect("remove moved source root");
        std::fs::remove_dir_all(root).expect("remove replacement source root");
    }

    #[cfg(unix)]
    #[test]
    fn include_target_rejects_parent_replacement_before_read() {
        let root = temporary_path("secure-include-race");
        let moved = temporary_path("secure-include-moved");
        let external = temporary_path("secure-include-external");
        std::fs::create_dir_all(&root).expect("create include root");
        std::fs::create_dir_all(&external).expect("create external include root");
        let including = root.join("top.sv");
        let child = root.join("child.svh");
        std::fs::write(&including, "module top; endmodule\n").expect("write including source");
        std::fs::write(&child, "`define CHILD 1\n").expect("write admitted include");
        std::fs::write(external.join("child.svh"), "`define EXTERNAL 1\n")
            .expect("write external include");
        let expected = resolve_include(&including, "child.svh", &[])
            .expect("include should resolve before replacement");
        std::fs::rename(&root, &moved).expect("move admitted include root");
        std::fs::rename(&external, &root).expect("replace admitted include root");

        let path_name = expected.actual_path().to_string_lossy().into_owned();
        let error = read_bounded_at(&path_name, &expected, 64, "SystemVerilog source")
            .expect_err("include replacement must retain its admitted target");
        assert_eq!(error.kind(), StartupErrorKind::Input);
        assert!(error.contains("changed from admitted target"));

        std::fs::remove_dir_all(moved).expect("remove moved include root");
        std::fs::remove_dir_all(root).expect("remove replacement include root");
    }

    #[cfg(unix)]
    #[test]
    fn include_resolution_rejects_replaced_admitted_parent() {
        let root = temporary_path("secure-include-resolution-race");
        let moved = temporary_path("secure-include-resolution-moved");
        let external = temporary_path("secure-include-resolution-external");
        std::fs::create_dir_all(&root).expect("create include resolution root");
        std::fs::create_dir_all(&external).expect("create external resolution root");
        let including = root.join("top.sv");
        std::fs::write(&including, "module top; endmodule\n").expect("write including source");
        std::fs::write(root.join("child.svh"), "`define CHILD 1\n")
            .expect("write admitted include");
        std::fs::write(external.join("top.sv"), "module external; endmodule\n")
            .expect("write external including source");
        std::fs::write(external.join("child.svh"), "`define EXTERNAL 1\n")
            .expect("write external include");
        let expected = secure_fs::open_path(&including)
            .expect("open including source")
            .admitted_target();
        std::fs::rename(&root, &moved).expect("move admitted include root");
        std::fs::rename(&external, &root).expect("replace admitted include root");

        assert!(resolve_include_checked(&including, Some(&expected), "child.svh", &[]).is_none());

        std::fs::remove_dir_all(moved).expect("remove moved admitted include root");
        std::fs::remove_dir_all(root).expect("remove replacement include root");
    }

    #[test]
    fn unquoted_absolute_star_path_matches_in_memory_sources() {
        let maps = [OwnedSource::include("root.map", "library L /*/*.sv;")];
        let mut sources = vec![OwnedSource::compilation_unit("/rtl/source.sv", "")];
        let mut library_sources = Vec::new();
        let mut source_count = sources.len();
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            128,
            &mut work,
        )
        .expect("absolute star pattern should match an admitted source");
        assert!(sources.is_empty());
        assert_eq!(library_sources[0].name, "/rtl/source.sv");
    }

    #[test]
    fn unquoted_absolute_recursive_path_matches_in_memory_sources() {
        let maps = [OwnedSource::include("root.map", "library L /**/*.sv;")];
        let mut sources = vec![OwnedSource::compilation_unit("/rtl/nested/source.sv", "")];
        let mut library_sources = Vec::new();
        let mut source_count = sources.len();
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut library_sources,
            &mut source_count,
            &mut remaining,
            128,
            &mut work,
        )
        .expect("absolute recursive pattern should match an admitted source");
        assert!(sources.is_empty());
        assert_eq!(library_sources[0].name, "/rtl/nested/source.sv");
    }

    #[cfg(unix)]
    #[test]
    fn recursive_library_pattern_skips_symlink_directory_cycles() {
        use std::os::unix::fs::symlink;

        let root = temporary_path("library-pattern-cycle");
        let real = root.join("real");
        std::fs::create_dir_all(&real).expect("create recursive library root");
        let source = real.join("source.sv");
        std::fs::write(&source, "module source; endmodule\n").expect("write recursive source");
        symlink(&root, real.join("cycle")).expect("create recursive directory symlink");

        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let matches = expand_library_pattern(&root, "**/*.sv", 8, &mut work)
            .expect("recursive pattern should terminate at symlink directories");
        assert_eq!(
            matches,
            vec![source.canonicalize().expect("canonical source")]
        );
        std::fs::remove_dir_all(root).expect("remove recursive library root");
    }
}
