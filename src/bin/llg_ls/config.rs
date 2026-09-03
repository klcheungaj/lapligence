//! `llg.toml` v1 configuration parsing and validation.
//!
//! This module owns the editor-independent root configuration contract.  It
//! parses `llg.toml` with a real TOML parser, resolves every path from the
//! directory containing the config file, validates the result atomically, and
//! exposes the derived values the backend needs: normalized source/include
//! directories, source include/exclude filters, `CompileOpts` overrides and a
//! `LintConfig`.
//!
//! Contract (see the migration plan):
//!
//! * `schema_version = 1` is required; unknown versions and unknown v1 fields
//!   are errors so a misspelled key cannot silently change analysis.
//! * Relative paths resolve from the directory containing `llg.toml`;
//!   absolute paths are accepted.
//! * `sources.directories` defaults to `["."]` and may point outside the
//!   workspace.
//! * `sources.include`/`exclude` are root-relative globs evaluated against
//!   each source directory; exclude wins.
//! * Only `.v`/`.sv` files become compilation units.  `.vh`/`.svh` and
//!   arbitrary-extension includes enter analysis only through include
//!   resolution.
//! * Every source directory is automatically an include-search directory.
//! * `compile.include_dirs` adds search-only directories and may be external.
//! * `compile.defines`/`include_dirs` are converted to validated Surelog
//!   `-D`/`-I` arguments internally; no raw compiler-argument passthrough.
//! * `[compile.param_overrides]` maps top-level parameter names to string or
//!   integer values, converted to Surelog `-PNAME=VALUE` arguments; integers
//!   are normalized to their decimal string form.
//! * `[analysis]` bounds each unique input file at 1 MiB by default and the
//!   complete unique compilation-unit/include input set at 8 MiB.  Both
//!   limits must be positive when configured.
//! * Config reloads read at most [`MAX_CONFIG_BYTES`] plus one byte and reject
//!   oversized or non-UTF-8 files without replacing the last valid config.
//! * Structural errors (malformed TOML, unknown fields/versions, wrong
//!   types) reject the whole config atomically.  Individual malformed
//!   `defines` entries or `param_overrides` keys/values are dropped with a
//!   published warning instead: one bad entry never discards the rest of the
//!   configuration.  Values containing ASCII control characters are dropped
//!   the same way (they cannot survive the Surelog C-argument boundary).
//! * A missing config uses safe defaults: the config directory as the sole
//!   source directory, recursive `.v`/`.sv`, and built-in excludes
//!   (`slpp_all/**`, `.git/**`, `target/**`).
//! * Malformed or semantically invalid config is rejected atomically.  The
//!   backend publishes diagnostics against the TOML URI, retains the last
//!   valid configuration on reload, and uses safe defaults until a valid
//!   configuration has ever loaded.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::workspace::{self, SourceFileKind};
use llg::core::lint::{LintConfig, LintSeverity, RuleConfig};

/// The only supported `llg.toml` schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// The config file name loaded per root.
pub const CONFIG_FILE: &str = "llg.toml";

/// Built-in source directories excluded by default.
///
/// `slpp_all/**` is defense-in-depth: Surelog writes preprocessed-output
/// copies there, and a leaked directory must never enter source discovery or
/// watchers (the analysis CWD guard in `features` keeps it out of project
/// trees entirely).
pub const DEFAULT_EXCLUDE_GLOBS: [&str; 3] = ["slpp_all/**", ".git/**", "target/**"];

/// Recursive include globs used when a config does not name explicit
/// include patterns (same defaults as a missing config file).
pub const DEFAULT_INCLUDE_GLOBS: [&str; 2] = ["**/*.v", "**/*.sv"];

/// Conservative default maximum size of one unique analysis input in bytes.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Conservative default maximum size of all unique analysis inputs in bytes.
pub const DEFAULT_MAX_TOTAL_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Fixed maximum size of a configuration file read during reload.
///
/// Configuration is control-plane input, but it still arrives from a path
/// selected by the client.  Keep its read bounded independently of the
/// analysis input budgets so a malformed or unexpectedly large `llg.toml`
/// cannot consume memory before TOML validation runs.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// A validated, resolved `llg.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct LlgConfig {
    pub schema_version: u32,
    /// The directory that contains the config file; relative paths resolve
    /// against it.
    pub base_dir: PathBuf,
    pub sources: SourcesConfig,
    pub compile: CompileConfig,
    pub analysis: AnalysisConfig,
    pub lint: LintConfig,
}

/// Source discovery configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcesConfig {
    /// Normalized absolute source directories.  Each is also an include dir.
    pub directories: Vec<PathBuf>,
    /// Root-relative include globs (evaluated against each source dir).
    pub include: Vec<String>,
    /// Root-relative exclude globs; excludes win over includes.
    pub exclude: Vec<String>,
}

/// Compile configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileConfig {
    pub top: Option<String>,
    /// Normalized absolute include-search directories (may be external).
    pub include_dirs: Vec<PathBuf>,
    /// Validated preprocessor defines (`NAME` or `NAME=VALUE`), raw.
    pub defines: Vec<String>,
    /// Validated top-level parameter overrides, ordered by name.  These
    /// become Surelog `-PNAME=VALUE` arguments and apply to the top-level
    /// module instances of the analysis (the explicit `top` or Surelog's
    /// auto-detected tops).
    pub param_overrides: BTreeMap<String, String>,
}

/// Byte budgets applied to the unique root compilation units and literal
/// include dependencies admitted to an LSP analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisConfig {
    /// Maximum bytes in one unique input file.
    pub max_file_bytes: u64,
    /// Maximum bytes across all unique input files.
    pub max_total_input_bytes: u64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_total_input_bytes: DEFAULT_MAX_TOTAL_INPUT_BYTES,
        }
    }
}

/// An error describing why a `llg.toml` failed to parse/validate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Result of loading a root's config file from disk.
#[derive(Debug, Clone)]
pub struct ConfigLoad {
    /// The config file path that was read.
    pub path: PathBuf,
    /// `Some` when a valid config was produced, `None` when the file was
    /// missing or invalid.
    pub config: Option<LlgConfig>,
    /// Parse/validation errors (empty for a missing or valid file).
    pub errors: Vec<ConfigError>,
    /// Non-fatal load warnings: configured source/include directories that do
    /// not exist on disk, and malformed `defines`/`param_overrides` entries
    /// that were dropped instead of failing the load.  Reported once per load
    /// against the TOML URI; they never fail the load.
    pub warnings: Vec<ConfigError>,
    /// Whether the config file existed on disk.
    pub present: bool,
}

// ── Raw TOML shape ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema_version: u32,
    #[serde(default)]
    sources: RawSources,
    #[serde(default)]
    compile: RawCompile,
    #[serde(default)]
    analysis: RawAnalysis,
    #[serde(default)]
    lint: RawLint,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSources {
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCompile {
    top: Option<String>,
    #[serde(default)]
    include_dirs: Vec<String>,
    #[serde(default)]
    defines: Vec<String>,
    /// Raw parameter-override values; type-checked in
    /// [`resolve_param_overrides`] so a wrong type yields a precise error.
    #[serde(default)]
    param_overrides: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawAnalysis {
    #[serde(default)]
    max_file_bytes: Option<u64>,
    #[serde(default)]
    max_total_input_bytes: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLint {
    enabled: Option<bool>,
    #[serde(default)]
    rules: std::collections::BTreeMap<String, RawRule>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawRule {
    enabled: Option<bool>,
    severity: Option<String>,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Whether `path` is a compilation-unit source (`.v`/`.sv`).
pub fn is_compilation_unit(path: &Path) -> bool {
    SourceFileKind::from_path(path).is_some_and(|kind| {
        matches!(
            kind,
            SourceFileKind::Verilog | SourceFileKind::SystemVerilog
        )
    })
}

/// A successfully parsed config plus its non-fatal warnings.
///
/// `warnings` carries entry-level issues that were dropped instead of
/// rejecting the whole file (malformed `defines` entries, invalid
/// `param_overrides` keys/values).  Callers publish them against the TOML
/// URI; they never fail the load.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedConfig {
    pub config: LlgConfig,
    pub warnings: Vec<ConfigError>,
}

/// Parse `text` as a `llg.toml` relative to `base_dir` (the directory
/// containing the config file), returning soft warnings alongside.
/// On any structural error the whole config is rejected.
pub fn parse_config_detailed(base_dir: &Path, text: &str) -> Result<ParsedConfig, ConfigError> {
    let base_dir = workspace::normalize_absolute_path(base_dir)
        .ok_or_else(|| ConfigError::new("config base directory must be absolute"))?;

    let value: toml::Value = toml::from_str(text)
        .map_err(|error| ConfigError::new(format!("malformed TOML: {error}")))?;

    if !value.is_table() {
        return Err(ConfigError::new("llg.toml must be a TOML table"));
    }
    let raw: RawConfig = value
        .try_into()
        .map_err(|error| ConfigError::new(format!("invalid llg.toml: {error}")))?;

    if raw.schema_version != SCHEMA_VERSION {
        return Err(ConfigError::new(format!(
            "unsupported schema_version {} (expected {SCHEMA_VERSION})",
            raw.schema_version
        )));
    }

    validate_raw_shape()?;
    let mut warnings = Vec::new();
    let sources = resolve_sources(&base_dir, &raw.sources)?;
    let (compile, mut compile_warnings) = resolve_compile(&base_dir, &raw.compile)?;
    warnings.append(&mut compile_warnings);
    let analysis = resolve_analysis(&raw.analysis)?;
    let lint = translate_lint(&raw.lint)?;

    Ok(ParsedConfig {
        config: LlgConfig {
            schema_version: raw.schema_version,
            base_dir,
            sources,
            compile,
            analysis,
            lint,
        },
        warnings,
    })
}

/// Parse `text` as a `llg.toml` relative to `base_dir` (the directory
/// containing the config file).  On any error the whole config is rejected.
/// Soft entry-level warnings are available from [`parse_config_detailed`].
pub fn parse_config(base_dir: &Path, text: &str) -> Result<LlgConfig, ConfigError> {
    Ok(parse_config_detailed(base_dir, text)?.config)
}

/// Load and parse `<root>/llg.toml`.
///
/// A missing file produces a present `ConfigLoad` with `config: None` (callers
/// apply safe defaults).  A malformed file is rejected atomically with its
/// errors.  I/O errors other than "not found" propagate as `Err`.
pub fn load_config(root: &Path) -> io::Result<ConfigLoad> {
    let root = workspace::normalize_absolute_path(root).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "config root must be an absolute path",
        )
    })?;
    let path = root.join(CONFIG_FILE);
    load_config_file(&path)
}

/// Load and parse the config file at an explicit absolute path.
///
/// Relative paths inside the TOML resolve from the directory containing the
/// file.  A missing file produces `config: None` with no errors; a malformed
/// file is rejected atomically.  Oversized and non-UTF-8 files remain read
/// errors, matching the existing `io::Result` contract.  The file read is
/// bounded to `MAX_CONFIG_BYTES` plus one byte.
pub fn load_config_file(path: &Path) -> io::Result<ConfigLoad> {
    let path = workspace::normalize_absolute_path(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "config path must be an absolute path",
        )
    })?;
    match read_config_text(&path) {
        Ok(text) => {
            let base = path.parent().unwrap_or(Path::new("/"));
            let (config, errors, warnings) = match parse_config_detailed(base, &text) {
                Ok(parsed) => {
                    let mut warnings = parsed.warnings;
                    warnings.extend(missing_directory_warnings(&parsed.config));
                    (Some(parsed.config), Vec::new(), warnings)
                }
                Err(error) => (None, vec![error], Vec::new()),
            };
            Ok(ConfigLoad {
                path: path.clone(),
                config,
                errors,
                warnings,
                present: true,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ConfigLoad {
            path,
            config: None,
            errors: Vec::new(),
            warnings: Vec::new(),
            present: false,
        }),
        Err(error) => Err(error),
    }
}

/// Read a config file with a fixed upper bound and validate its UTF-8 before
/// handing it to the TOML parser.  The extra byte distinguishes an exactly
/// full file from an oversized one without ever retaining more than the
/// configured bound plus one byte.
fn read_config_text(path: &Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::with_capacity(MAX_CONFIG_BYTES as usize + 1);
    file.take(MAX_CONFIG_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("config file exceeds the maximum size of {MAX_CONFIG_BYTES} bytes"),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("config file is not valid UTF-8: {error}"),
        )
    })
}

/// One warning per configured source/include directory that does not exist on
/// disk.  Non-fatal: discovery simply finds nothing there until the directory
/// appears.
fn missing_directory_warnings(config: &LlgConfig) -> Vec<ConfigError> {
    let mut warnings = Vec::new();
    for dir in &config.sources.directories {
        if !dir.exists() {
            warnings.push(ConfigError::new(format!(
                "configured source directory does not exist: {}",
                dir.display()
            )));
        }
    }
    for dir in &config.compile.include_dirs {
        if !dir.exists() {
            warnings.push(ConfigError::new(format!(
                "configured include directory does not exist: {}",
                dir.display()
            )));
        }
    }
    warnings
}

/// Build the safe-default config used when no valid config has ever loaded.
///
/// The default treats the config root as the sole source directory, discovers
/// recursive `.v`/`.sv`, excludes `.git/**` and `target/**`, and leaves
/// compile + lint settings at their defaults.
pub fn default_config(root: &Path) -> LlgConfig {
    let root = workspace::normalize_absolute_path(root).unwrap_or_else(|| root.to_path_buf());
    LlgConfig {
        schema_version: SCHEMA_VERSION,
        base_dir: root.clone(),
        sources: SourcesConfig {
            directories: vec![root.clone()],
            include: DEFAULT_INCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
            exclude: DEFAULT_EXCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
        },
        compile: CompileConfig {
            top: None,
            include_dirs: Vec::new(),
            defines: Vec::new(),
            param_overrides: BTreeMap::new(),
        },
        analysis: AnalysisConfig::default(),
        lint: LintConfig::default(),
    }
}

/// All include-search directories for a config: every source directory plus
/// the explicit `compile.include_dirs`, normalized and deduplicated in order.
pub fn include_dirs(config: &LlgConfig) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut dirs = Vec::new();
    for dir in config
        .sources
        .directories
        .iter()
        .chain(&config.compile.include_dirs)
    {
        if seen.insert(dir.clone()) {
            dirs.push(dir.clone());
        }
    }
    dirs
}

/// Build a `CompileOpts` from a config and compiled file paths.
///
/// `shadow_base` is the private per-process shadow tree; all shadow mirrors
/// are emitted first in configured order, followed by all live directories in
/// that same order, so a staged header cannot be preempted by a live fallback
/// directory.  `defines`/`include_dirs`/`param_overrides` are converted to
/// validated Surelog `-D`/`-I`/`-P` arguments; there is no raw compiler-argument
/// passthrough.
pub fn compile_opts(
    config: &LlgConfig,
    files: Vec<String>,
    shadow_base: &Path,
) -> llg::core::compile::CompileOpts {
    compile_opts_with_include_dirs(config, files, shadow_base, true)
}

/// Build compile options for the LSP's admitted/staged input path.
///
/// This deliberately omits live source/include directories.  The LSP stages
/// every admitted literal include into the private shadow tree; omitting live
/// `-I` fallbacks also makes macro-generated or dynamic includes fail closed
/// instead of letting Surelog read an unmeasured project file.  The general
/// [`compile_opts`] path retains its live directories for dump/general flows
/// that intentionally need them.
pub fn compile_opts_isolated(
    config: &LlgConfig,
    files: Vec<String>,
    shadow_base: &Path,
) -> llg::core::compile::CompileOpts {
    compile_opts_with_include_dirs(config, files, shadow_base, false)
}

fn compile_opts_with_include_dirs(
    config: &LlgConfig,
    files: Vec<String>,
    shadow_base: &Path,
    include_live_dirs: bool,
) -> llg::core::compile::CompileOpts {
    let mut include_args = Vec::new();
    let search_dirs = include_dirs(config);
    for dir in &search_dirs {
        let shadow_dir = crate::features::shadow_path(&dir, shadow_base);
        include_args.push(format!("-I{}", shadow_dir.display()));
    }
    if include_live_dirs {
        for dir in &search_dirs {
            include_args.push(format!("-I{}", dir.display()));
        }
    }
    llg::core::compile::CompileOpts {
        files,
        top: config.compile.top.clone(),
        defines: config
            .compile
            .defines
            .iter()
            .map(|define| format!("-D{define}"))
            .collect(),
        param_overrides: config
            .compile
            .param_overrides
            .iter()
            .map(|(name, value)| format!("-P{name}={value}"))
            .collect(),
        include_dirs: include_args,
        ..Default::default()
    }
}

// ── Resolution helpers ───────────────────────────────────────────────────────

fn validate_raw_shape() -> Result<(), ConfigError> {
    // `deny_unknown_fields` already rejected unknown keys; this hook exists
    // for additional cross-field validation that serde cannot express.
    Ok(())
}

fn resolve_sources(base_dir: &Path, raw: &RawSources) -> Result<SourcesConfig, ConfigError> {
    // Validate patterns first so invalid globs reject the whole config.
    //
    // A present `[sources]` table WITHOUT `include` would otherwise yield an
    // empty glob list and silently discover nothing; fall back to the same
    // recursive `.v`/`.sv` defaults a missing config uses.  Exclude patterns
    // always sit ON TOP of the built-in excludes (defense-in-depth:
    // `slpp_all/**` must never enter discovery), so an explicit-empty
    // exclude means exactly "no extra excludes beyond the built-ins".
    let include = if raw.include.is_empty() {
        DEFAULT_INCLUDE_GLOBS
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect()
    } else {
        normalize_patterns("sources.include", &raw.include)?
    };
    let mut exclude = if raw.exclude.is_empty() {
        Vec::new()
    } else {
        normalize_patterns("sources.exclude", &raw.exclude)?
    };
    for pattern in DEFAULT_EXCLUDE_GLOBS {
        if !exclude.iter().any(|existing| existing == pattern) {
            exclude.push(pattern.to_owned());
        }
    }

    let directories = if raw.directories.is_empty() {
        vec![base_dir.to_path_buf()]
    } else {
        raw.directories
            .iter()
            .map(|entry| resolve_dir(base_dir, entry, "sources.directories"))
            .collect::<Result<Vec<_>, _>>()?
    };
    let directories = dedupe_paths(directories);

    Ok(SourcesConfig {
        directories,
        include,
        exclude,
    })
}

fn resolve_compile(
    base_dir: &Path,
    raw: &RawCompile,
) -> Result<(CompileConfig, Vec<ConfigError>), ConfigError> {
    let include_dirs = raw
        .include_dirs
        .iter()
        .map(|entry| resolve_dir(base_dir, entry, "compile.include_dirs"))
        .collect::<Result<Vec<_>, _>>()?;
    let include_dirs = dedupe_paths(include_dirs);
    let (defines, mut warnings) = validate_defines(&raw.defines);
    let (param_overrides, mut override_warnings) = resolve_param_overrides(raw)?;
    warnings.append(&mut override_warnings);
    Ok((
        CompileConfig {
            top: raw.top.clone(),
            include_dirs,
            defines,
            param_overrides,
        },
        warnings,
    ))
}

fn resolve_analysis(raw: &RawAnalysis) -> Result<AnalysisConfig, ConfigError> {
    let max_file_bytes = raw.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES);
    if max_file_bytes == 0 {
        return Err(ConfigError::new("analysis.max_file_bytes must be positive"));
    }
    let max_total_input_bytes = raw
        .max_total_input_bytes
        .unwrap_or(DEFAULT_MAX_TOTAL_INPUT_BYTES);
    if max_total_input_bytes == 0 {
        return Err(ConfigError::new(
            "analysis.max_total_input_bytes must be positive",
        ));
    }
    Ok(AnalysisConfig {
        max_file_bytes,
        max_total_input_bytes,
    })
}

fn resolve_dir(base_dir: &Path, entry: &str, field: &str) -> Result<PathBuf, ConfigError> {
    let raw_path = Path::new(entry);
    let absolute = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    };
    let normalized = workspace::normalize_absolute_path(&absolute)
        .ok_or_else(|| ConfigError::new(format!("{field} must resolve to an absolute path")))?;
    Ok(normalized)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn normalize_patterns(field: &str, patterns: &[String]) -> Result<Vec<String>, ConfigError> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for pattern in patterns {
        let normalized_pattern = workspace::normalize_relative_pattern(pattern)
            .ok_or_else(|| ConfigError::new(format!("{field}: invalid glob `{pattern}`")))?;
        if normalized_pattern.is_empty() {
            return Err(ConfigError::new(format!("{field}: glob must not be empty")));
        }
        if seen.insert(normalized_pattern.clone()) {
            normalized.push(normalized_pattern);
        }
    }
    Ok(normalized)
}

/// Values become C-string arguments at the Surelog FFI boundary
/// (`SessionBuilder::add_arg`), where ASCII control characters silently
/// vanish.  Entries carrying them are therefore soft-dropped up front with
/// the other fail-soft warnings.
fn has_control_char(value: &str) -> bool {
    value.chars().any(|ch| ch < '\u{20}' || ch == '\u{7f}')
}

/// Validate `compile.defines` entries.  Fail-soft: a malformed entry is
/// dropped with a warning instead of rejecting the whole config; the first
/// entry for a name wins and later duplicates are reported.
fn validate_defines(defines: &[String]) -> (Vec<String>, Vec<ConfigError>) {
    let mut warnings = Vec::new();
    let mut seen_names = BTreeSet::new();
    let mut validated = Vec::new();
    for define in defines {
        if define.is_empty() {
            warnings.push(ConfigError::new(
                "compile.defines: empty define entry dropped",
            ));
            continue;
        }
        let name = define.split('=').next().unwrap_or("");
        let valid_name = !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
        if !valid_name {
            warnings.push(ConfigError::new(format!(
                "compile.defines: invalid define `{define}` dropped"
            )));
            continue;
        }
        if let Some((_, value)) = define.split_once('=') {
            if has_control_char(value) {
                warnings.push(ConfigError::new(format!(
                    "compile.defines: define `{name}` dropped \
                     (value contains an ASCII control character)"
                )));
                continue;
            }
        }
        if !seen_names.insert(name.to_owned()) {
            warnings.push(ConfigError::new(format!(
                "compile.defines: duplicate define name `{name}` dropped (first entry wins)"
            )));
            continue;
        }
        validated.push(define.clone());
    }
    (validated, warnings)
}

/// Validate `[compile.param_overrides]`.  Keys must be SystemVerilog simple
/// identifiers; values must be TOML strings or integers (integers are
/// normalized to decimal strings).  A wrong value TYPE rejects the config
/// atomically (consistent with every other typed field); an invalid key or an
/// empty string value is fail-soft: dropped with a warning.
fn resolve_param_overrides(
    raw: &RawCompile,
) -> Result<(BTreeMap<String, String>, Vec<ConfigError>), ConfigError> {
    let mut warnings = Vec::new();
    let mut overrides = BTreeMap::new();
    for (name, value) in &raw.param_overrides {
        if !is_identifier(name) {
            warnings.push(ConfigError::new(format!(
                "compile.param_overrides.{name}: invalid parameter identifier, entry dropped"
            )));
            continue;
        }
        let normalized = match value {
            toml::Value::String(text) => text.clone(),
            toml::Value::Integer(int) => int.to_string(),
            other => {
                return Err(ConfigError::new(format!(
                    "compile.param_overrides.{name}: value must be a string or integer, got {}",
                    value_type_name(other)
                )));
            }
        };
        if normalized.is_empty() {
            warnings.push(ConfigError::new(format!(
                "compile.param_overrides.{name}: empty value dropped"
            )));
            continue;
        }
        if has_control_char(&normalized) {
            warnings.push(ConfigError::new(format!(
                "compile.param_overrides.{name}: value contains an ASCII control character, \
                 entry dropped"
            )));
            continue;
        }
        overrides.insert(name.clone(), normalized);
    }
    Ok((overrides, warnings))
}

/// A SystemVerilog simple identifier: letter or underscore first, then
/// letters, digits, underscores or `$`.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn value_type_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

fn translate_lint(raw: &RawLint) -> Result<LintConfig, ConfigError> {
    // Unknown rule ids must be rejected (deny_unknown_fields doctrine): a
    // misspelled `[lint.rules.<id>]` table would otherwise silently configure
    // nothing.
    for id in raw.rules.keys() {
        if !known_rule_ids().iter().any(|known| known == id) {
            return Err(ConfigError::new(format!(
                "lint.rules.{id}: unknown lint rule `{id}`"
            )));
        }
    }
    let mut config = LintConfig::default();
    if raw.enabled == Some(false) {
        // Global kill-switch: disable every rule (per-rule entries can
        // re-enable individual rules, mirroring the client settings path).
        for id in known_rule_ids() {
            config.set(
                id.to_owned(),
                RuleConfig {
                    enabled: false,
                    severity: config.severity(id),
                },
            );
        }
    }
    for (id, rule) in &raw.rules {
        let mut rule_config = config.get(id);
        if let Some(enabled) = rule.enabled {
            rule_config.enabled = enabled;
        }
        if let Some(severity) = &rule.severity {
            rule_config.severity = Some(parse_severity(severity)?);
        }
        config.set(id.clone(), rule_config);
    }
    Ok(config)
}

fn parse_severity(value: &str) -> Result<LintSeverity, ConfigError> {
    match value {
        "error" => Ok(LintSeverity::Error),
        "warning" => Ok(LintSeverity::Warning),
        "info" => Ok(LintSeverity::Info),
        _ => Err(ConfigError::new(format!(
            "lint.rules.*.severity must be error, warning or info, got `{value}`"
        ))),
    }
}

/// The known rule ids, for the global kill-switch.
fn known_rule_ids() -> Vec<&'static str> {
    llg::core::lint::LintRegistry::default_rules()
        .all()
        .iter()
        .map(|rule| rule.id())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_and_load(root: &Path, text: &str) -> ConfigLoad {
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create config root");
        let path = dir.join(CONFIG_FILE);
        std::fs::write(&path, text).expect("write config");
        load_config(&dir).expect("load config")
    }

    #[test]
    fn valid_config_resolves_paths_from_config_dir() {
        let root = std::env::temp_dir().join(format!("llg_cfg_valid_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir.join("rtl")).expect("create rtl");
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"rtl\", \"../shared\"]\n\
             include = [\"**/*.v\", \"**/*.sv\"]\n\
             exclude = [\"**/generated/**\"]\n\
             [compile]\n\
             top = \"top\"\n\
             include_dirs = [\"inc\", \"../vendor/inc\"]\n\
             defines = [\"SYNTHESIS\", \"WIDTH=8\"]\n\
             [compile.param_overrides]\n\
             DEPTH = 1024\n\
             [analysis]\n\
             max_file_bytes = 17\n\
             max_total_input_bytes = 91\n",
        );
        let config = load.config.expect("valid config");
        assert_eq!(config.sources.directories[0], dir.join("rtl"));
        assert_eq!(config.sources.directories[1], root.join("shared"));
        assert_eq!(config.compile.include_dirs[0], dir.join("inc"));
        assert_eq!(
            config.compile.include_dirs[1],
            root.join("vendor").join("inc")
        );
        assert_eq!(config.compile.defines, vec!["SYNTHESIS", "WIDTH=8"]);
        assert_eq!(config.compile.top.as_deref(), Some("top"));
        assert_eq!(config.analysis.max_file_bytes, 17);
        assert_eq!(config.analysis.max_total_input_bytes, 91);
        let opts = compile_opts(&config, vec!["top.sv".to_owned()], &root.join("shadow"));
        assert_eq!(opts.defines, vec!["-DSYNTHESIS", "-DWIDTH=8"]);
        assert_eq!(opts.param_overrides, vec!["-PDEPTH=1024"]);
        assert!(opts
            .include_dirs
            .iter()
            .any(|arg| arg == &format!("-I{}", dir.join("rtl").display())));
        assert!(opts
            .include_dirs
            .iter()
            .any(|arg| arg == &format!("-I{}", dir.join("inc").display())));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_config_uses_safe_defaults() {
        let root = std::env::temp_dir().join(format!("llg_cfg_missing_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create dir");
        let load = load_config(&dir).expect("load missing config");
        assert!(!load.present);
        assert!(load.config.is_none());
        assert!(load.errors.is_empty());
        let defaults = default_config(&dir);
        assert_eq!(defaults.sources.directories, vec![dir.clone()]);
        assert!(defaults.sources.exclude.iter().any(|p| p == "target/**"));
        assert!(defaults.sources.include.iter().any(|p| p == "**/*.sv"));
        assert!(defaults.compile.param_overrides.is_empty());
        assert!(defaults.compile.defines.is_empty());
        assert_eq!(defaults.analysis, AnalysisConfig::default());
        assert_eq!(defaults.analysis.max_file_bytes, 1024 * 1024);
        assert_eq!(defaults.analysis.max_total_input_bytes, 8 * 1024 * 1024);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn analysis_budgets_require_positive_integer_values_atomically() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_analysis_budget_{}", std::process::id()));
        let valid = write_and_load(
            &root,
            "schema_version = 1\n\
             [analysis]\n\
             max_file_bytes = 1\n\
             max_total_input_bytes = 2\n",
        );
        assert_eq!(
            valid.config.expect("positive budget config").analysis,
            AnalysisConfig {
                max_file_bytes: 1,
                max_total_input_bytes: 2,
            }
        );

        let zero = write_and_load(
            &root,
            "schema_version = 1\n[analysis]\nmax_file_bytes = 0\n",
        );
        assert!(zero.config.is_none());
        assert!(zero
            .errors
            .iter()
            .any(|error| error.message.contains("max_file_bytes")
                && error.message.contains("positive")));

        let zero_total = write_and_load(
            &root,
            "schema_version = 1\n[analysis]\nmax_total_input_bytes = 0\n",
        );
        assert!(zero_total.config.is_none());
        assert!(zero_total
            .errors
            .iter()
            .any(|error| error.message.contains("max_total_input_bytes")
                && error.message.contains("positive")));

        let negative = write_and_load(
            &root,
            "schema_version = 1\n[analysis]\nmax_total_input_bytes = -1\n",
        );
        assert!(negative.config.is_none());
        assert!(!negative.errors.is_empty());

        let wrong_type = write_and_load(
            &root,
            "schema_version = 1\n[analysis]\nmax_file_bytes = \"1 MiB\"\n",
        );
        assert!(wrong_type.config.is_none());
        assert!(!wrong_type.errors.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_fields_and_unknown_versions_are_errors() {
        let root = std::env::temp_dir().join(format!("llg_cfg_unknown_{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create dir");

        let bad_field = write_and_load(
            &root,
            "schema_version = 1\n[sources]\ndirectorie = [\".\"]\n",
        );
        assert!(bad_field.config.is_none());
        assert!(bad_field
            .errors
            .iter()
            .any(|e| e.message.contains("unknown")));

        let bad_version = write_and_load(&root, "schema_version = 2\n");
        assert!(bad_version.config.is_none());
        assert!(bad_version
            .errors
            .iter()
            .any(|e| e.message.contains("schema_version")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_toml_is_rejected_atomically() {
        let root = std::env::temp_dir().join(format!("llg_cfg_malformed_{}", std::process::id()));
        let load = write_and_load(&root, "schema_version = 1\n[sources\n");
        assert!(load.config.is_none());
        assert!(load.errors.iter().any(|e| e.message.contains("malformed")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn oversized_config_is_rejected_before_toml_parsing() {
        let root = std::env::temp_dir().join(format!("llg_cfg_oversized_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create config root");
        let path = dir.join(CONFIG_FILE);
        let bytes = vec![b'x'; MAX_CONFIG_BYTES as usize + 1];
        std::fs::write(&path, bytes).expect("write oversized config");

        let error = load_config_file(&path).expect_err("oversized config must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds the maximum size"));
        assert!(error.to_string().contains(&MAX_CONFIG_BYTES.to_string()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_utf8_config_is_reported_without_unbounded_decoding() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_invalid_utf8_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create config root");
        let path = dir.join(CONFIG_FILE);
        std::fs::write(&path, b"schema_version = 1\n\xff").expect("write invalid UTF-8 config");

        let error = load_config_file(&path).expect_err("invalid UTF-8 must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("not valid UTF-8"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_glob_is_atomic_but_bad_define_is_soft() {
        let root = std::env::temp_dir().join(format!("llg_cfg_badpat_{}", std::process::id()));
        let bad_pattern = write_and_load(
            &root,
            "schema_version = 1\n[sources]\ninclude = [\"../escape/**\"]\n",
        );
        assert!(bad_pattern.config.is_none());
        assert!(bad_pattern
            .errors
            .iter()
            .any(|e| e.message.contains("include")));

        // An invalid define only drops its own entry: the rest of the config
        // still loads and a warning names the dropped value.
        let bad_define = write_and_load(
            &root,
            "schema_version = 1\n\
             [compile]\n\
             defines = [\"-WIDTH=8\"]\n\
             [lint]\n\
             enabled = false\n",
        );
        let config = bad_define
            .config
            .expect("soft define failure must not reject the config");
        assert!(config.compile.defines.is_empty());
        assert!(!config.lint.is_enabled("unused-signal"));
        assert!(bad_define.errors.is_empty());
        assert!(bad_define
            .warnings
            .iter()
            .any(|w| w.message.contains("-WIDTH=8")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn param_overrides_accept_strings_and_integers() {
        let root = std::env::temp_dir().join(format!("llg_cfg_pov_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create dir");
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [compile]\n\
             top = \"soc_top\"\n\
             [compile.param_overrides]\n\
             DEPTH = \"1024\"\n\
             WIDTH = 8\n",
        );
        let config = load.config.expect("valid config");
        assert_eq!(config.compile.top.as_deref(), Some("soc_top"));
        assert_eq!(
            config.compile.param_overrides,
            BTreeMap::from([
                ("DEPTH".to_owned(), "1024".to_owned()),
                ("WIDTH".to_owned(), "8".to_owned()),
            ]),
            "integer values are normalized to decimal strings"
        );
        assert!(load.warnings.is_empty());
        let opts = compile_opts(&config, vec![], &root.join("shadow"));
        assert_eq!(
            opts.param_overrides,
            vec!["-PDEPTH=1024".to_owned(), "-PWIDTH=8".to_owned()],
            "overrides become Surelog -P arguments in deterministic name order"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_override_identifiers_and_empty_values_are_soft() {
        let root = std::env::temp_dir().join(format!("llg_cfg_povbad_{}", std::process::id()));
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [compile.param_overrides]\n\
             GOOD = \"1\"\n\
             \"2BAD\" = \"4\"\n\
             \"DASH-NAME\" = \"9\"\n\
             EMPTY = \"\"\n",
        );
        let config = load
            .config
            .expect("soft failures must not reject the config");
        assert_eq!(
            config.compile.param_overrides,
            BTreeMap::from([("GOOD".to_owned(), "1".to_owned())]),
            "only the valid entry survives"
        );
        assert_eq!(load.warnings.len(), 3, "one warning per dropped entry");
        assert!(load
            .warnings
            .iter()
            .any(|w| w.message.contains("2BAD") && w.message.contains("identifier")));
        assert!(load
            .warnings
            .iter()
            .any(|w| w.message.contains("EMPTY") && w.message.contains("empty")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wrong_override_value_type_rejects_the_config() {
        let root = std::env::temp_dir().join(format!("llg_cfg_povtype_{}", std::process::id()));
        let load = write_and_load(
            &root,
            "schema_version = 1\n[compile.param_overrides]\nDEPTH = [1024]\n",
        );
        assert!(
            load.config.is_none(),
            "a typed field with the wrong type rejects atomically"
        );
        assert!(load
            .errors
            .iter()
            .any(|e| e.message.contains("DEPTH") && e.message.contains("string or integer")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_define_names_warn_and_first_wins() {
        let root = std::env::temp_dir().join(format!("llg_cfg_dupdef_{}", std::process::id()));
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [compile]\n\
             defines = [\"WIDTH=8\", \"WIDTH=16\", \"FOO\", \"FOO\", \"BAR\"]\n",
        );
        let config = load.config.expect("valid config");
        assert_eq!(
            config.compile.defines,
            vec!["WIDTH=8".to_owned(), "FOO".to_owned(), "BAR".to_owned()],
            "first entry per name wins"
        );
        assert_eq!(load.warnings.len(), 2);
        assert!(load
            .warnings
            .iter()
            .all(|w| w.message.contains("duplicate define name")));
        let _ = std::fs::remove_dir_all(root);
    }

    /// Control characters in define/override values would vanish silently at
    /// the Surelog argument boundary; such entries are dropped with a warning.
    #[test]
    fn control_characters_in_values_are_dropped_with_a_warning() {
        let root = std::env::temp_dir().join(format!("llg_cfg_ctrl_{}", std::process::id()));
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [compile]\n\
             defines = [\"GOOD=8\", \"BADV=x\\u0001y\"]\n\
             [compile.param_overrides]\n\
             BADKEY = \"a\\u007f\"\n",
        );
        let config = load
            .config
            .expect("soft failures must not reject the config");
        assert_eq!(
            config.compile.defines,
            vec!["GOOD=8".to_owned()],
            "only the control-char-free define survives"
        );
        assert!(config.compile.param_overrides.is_empty());
        assert_eq!(load.warnings.len(), 2, "one warning per dropped entry");
        assert!(load
            .warnings
            .iter()
            .any(|w| w.message.contains("BADV") && w.message.contains("control character")));
        assert!(load
            .warnings
            .iter()
            .any(|w| w.message.contains("BADKEY") && w.message.contains("control character")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_dirs_are_implicit_include_dirs_and_dedupe() {
        let root = std::env::temp_dir().join(format!("llg_cfg_includes_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create dir");
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\".\", \"rtl\"]\n\
             [compile]\n\
             include_dirs = [\"rtl\"]\n",
        );
        let config = load.config.expect("config");
        let dirs = include_dirs(&config);
        assert_eq!(dirs, vec![dir.clone(), dir.join("rtl")]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compile_include_args_group_shadow_dirs_before_live_dirs() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_include_order_{}", std::process::id()));
        let dir = root.join("proj");
        for child in ["src_a", "src_b", "inc"] {
            std::fs::create_dir_all(dir.join(child)).expect("create include directory");
        }
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"src_a\", \"src_b\"]\n\
             [compile]\n\
             include_dirs = [\"inc\"]\n",
        );
        let config = load.config.expect("config");
        let shadow_base = dir.join("shadow");
        let search_dirs = include_dirs(&config);
        let expected = search_dirs
            .iter()
            .map(|directory| {
                format!(
                    "-I{}",
                    crate::features::shadow_path(directory, &shadow_base).display()
                )
            })
            .chain(
                search_dirs
                    .iter()
                    .map(|directory| format!("-I{}", directory.display())),
            )
            .collect::<Vec<_>>();

        let opts = compile_opts(&config, vec!["top.sv".to_owned()], &shadow_base);
        assert_eq!(opts.include_dirs, expected);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn isolated_compile_include_args_omit_live_dirs() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_isolated_include_{}", std::process::id()));
        let dir = root.join("proj");
        for child in ["src", "inc"] {
            std::fs::create_dir_all(dir.join(child)).expect("create include directory");
        }
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"src\"]\n\
             [compile]\n\
             include_dirs = [\"inc\"]\n",
        );
        let config = load.config.expect("config");
        let shadow_base = dir.join("shadow");
        let search_dirs = include_dirs(&config);
        let expected_shadow = search_dirs
            .iter()
            .map(|directory| {
                format!(
                    "-I{}",
                    crate::features::shadow_path(directory, &shadow_base).display()
                )
            })
            .collect::<Vec<_>>();

        let opts = compile_opts_isolated(&config, vec!["top.sv".to_owned()], &shadow_base);
        assert_eq!(opts.include_dirs, expected_shadow);
        for directory in search_dirs {
            assert!(!opts
                .include_dirs
                .contains(&format!("-I{}", directory.display())));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compilation_unit_extension_classification() {
        assert!(is_compilation_unit(Path::new("top.sv")));
        assert!(is_compilation_unit(Path::new("top.v")));
        assert!(!is_compilation_unit(Path::new("top.svh")));
        assert!(!is_compilation_unit(Path::new("top.vh")));
        assert!(!is_compilation_unit(Path::new("top.inc")));
    }

    #[test]
    fn unknown_lint_rule_ids_are_errors() {
        let root = std::env::temp_dir().join(format!("llg_cfg_badrule_{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create dir");

        let typo = write_and_load(
            &root,
            "schema_version = 1\n\
             [lint.rules.unused-signal-typo]\n\
             enabled = false\n",
        );
        assert!(
            typo.config.is_none(),
            "misspelled rule id must reject the config"
        );
        assert!(typo
            .errors
            .iter()
            .any(|e| e.message.contains("unknown lint rule `unused-signal-typo`")));

        let known = write_and_load(
            &root,
            "schema_version = 1\n\
             [lint.rules.unused-signal]\n\
             enabled = false\n",
        );
        let config = known.config.expect("known rule must load");
        assert!(!config.lint.is_enabled("unused-signal"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn default_excludes_cover_slpp_all() {
        assert!(DEFAULT_EXCLUDE_GLOBS.contains(&"slpp_all/**"));
        assert!(DEFAULT_EXCLUDE_GLOBS.contains(&".git/**"));
        assert!(DEFAULT_EXCLUDE_GLOBS.contains(&"target/**"));
    }

    #[test]
    fn empty_sources_table_falls_back_to_default_includes() {
        // A present `[sources]` table without `include` used to produce an
        // empty glob list and silently discover nothing.
        let root = std::env::temp_dir().join(format!("llg_cfg_emptyinc_{}", std::process::id()));
        let load = write_and_load(
            &root,
            "schema_version = 1\n[sources]\ndirectories = [\".\"]\n",
        );
        let config = load.config.expect("valid config");
        assert_eq!(
            config.sources.include,
            DEFAULT_INCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect::<Vec<_>>(),
            "empty include must fall back to the recursive .v/.sv defaults"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_and_empty_exclude_keep_builtin_defense() {
        let root = std::env::temp_dir().join(format!("llg_cfg_emptyexc_{}", std::process::id()));

        // No exclude at all: exactly the built-in excludes.
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\".\"]\n\
             include = [\"**/*.sv\"]\n",
        );
        let config = load.config.expect("valid config");
        assert_eq!(
            config.sources.exclude,
            DEFAULT_EXCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect::<Vec<_>>(),
            "empty exclude means no extra excludes beyond the built-ins"
        );

        // An explicit user list sits on top of the built-in excludes, so the
        // slpp_all defense is never lost.
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\".\"]\n\
             include = [\"**/*.sv\"]\n\
             exclude = [\"**/generated/**\"]\n",
        );
        let config = load.config.expect("valid config");
        assert!(
            config
                .sources
                .exclude
                .iter()
                .any(|pattern| pattern == "**/generated/**"),
            "user exclude must be kept: {:?}",
            config.sources.exclude
        );
        for builtin in DEFAULT_EXCLUDE_GLOBS {
            assert!(
                config
                    .sources
                    .exclude
                    .iter()
                    .any(|pattern| pattern == builtin),
                "built-in exclude `{builtin}` must always apply: {:?}",
                config.sources.exclude
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_configured_directories_produce_warnings_not_errors() {
        let root = std::env::temp_dir().join(format!("llg_cfg_missingdir_{}", std::process::id()));
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create config root");
        let path = dir.join(CONFIG_FILE);
        std::fs::write(
            &path,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"rtl\", \".\"]\n\
             [compile]\n\
             include_dirs = [\"vendor/inc\"]\n",
        )
        .expect("write config");
        let load = load_config(&dir).expect("load config");
        assert!(load.config.is_some(), "missing directories are non-fatal");
        assert!(load.errors.is_empty());
        assert_eq!(load.warnings.len(), 2, "one warning per missing directory");
        assert!(load
            .warnings
            .iter()
            .any(|warning| warning.message.contains("source directory")
                && warning.message.contains("rtl")));
        assert!(load
            .warnings
            .iter()
            .any(|warning| warning.message.contains("include directory")
                && warning.message.contains("vendor/inc")));
        let _ = std::fs::remove_dir_all(root);
    }
}
