//! `llg.toml` schema shared by the `llg` simulator driver and the `llg_ls`
//! language server.
//!
//! One file drives both tools. This module owns the editor- and
//! process-independent contract: parsing, validation, path resolution,
//! source discovery and the defaults. The language server keeps its
//! LSP-specific parts (watching, reload, `initializationOptions`) and the
//! driver keeps command-line handling and precedence; neither re-parses TOML.
//!
//! Contract:
//!
//! * `schema_version = 1` is required; unknown versions and unknown fields
//!   are errors so a misspelled key cannot silently change a tool's behavior.
//! * Every key is accepted by both tools. A key one tool does not use is
//!   validated and then ignored there (see `docs/config.md` for the per-key
//!   applicability).
//! * Relative paths resolve from the directory containing the config file;
//!   absolute paths are accepted. Program names (`build.cc`, `build.cmake`,
//!   `build.launcher`, `build.generator`) and flag strings are not paths.
//! * `sources.directories` defaults to `["."]` for the language server and may
//!   point outside the workspace; [`SourcesConfig::directories_configured`]
//!   records whether the file named them, which is what makes the driver
//!   discover sources there.
//! * `sources.include`/`exclude` are directory-relative globs evaluated against
//!   each source directory; exclude wins. Only `.v`/`.sv` files become
//!   compilation units.
//! * `[compile.param_overrides]` maps top-level parameter names to string or
//!   integer values, converted to `NAME=VALUE`.
//! * `[analysis]` bounds each unique language-server input file (1 MiB by
//!   default) and the complete input set (8 MiB).
//! * Structural errors (malformed TOML, unknown fields/versions, wrong types,
//!   out-of-range values) reject the whole config atomically. Individual
//!   malformed `defines` entries or `param_overrides` keys/values are dropped
//!   with a warning instead, so one bad entry never discards the rest.
//! * Reads are bounded by [`MAX_CONFIG_BYTES`].

mod load;
pub mod paths;
mod resolve;
mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::compile::{CompilationUnitMode, LanguageEdition};
use crate::core::lint::LintConfig;
use crate::sim::build::ModelOptLevel;

pub use load::{load_config_file, parse_config_detailed, ConfigLoad, ParsedConfig};
pub use paths::is_compilation_unit;
pub use resolve::discover_sources;

/// The only supported `llg.toml` schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// The config file name loaded per root, and discovered in the current
/// directory by the `llg` driver.
pub const CONFIG_FILE: &str = "llg.toml";

/// Built-in source directories excluded by default.
pub const DEFAULT_EXCLUDE_GLOBS: [&str; 2] = [".git/**", "target/**"];

/// Recursive include globs used when a config does not name explicit include
/// patterns (same defaults as a missing config file).
pub const DEFAULT_INCLUDE_GLOBS: [&str; 2] = ["**/*.v", "**/*.sv"];

/// Conservative default maximum size of one unique analysis input in bytes.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Conservative default maximum size of all unique analysis inputs in bytes.
pub const DEFAULT_MAX_TOTAL_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Fixed maximum size of a configuration file read.
///
/// Configuration is control-plane input, but it still arrives from a path
/// selected by the client or command line. Keep its read bounded independently
/// of the analysis input budgets so a malformed or unexpectedly large
/// `llg.toml` cannot consume memory before TOML validation runs.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// How the generated model reacts to `$stop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopPolicy {
    Resume,
    Exit,
}

impl StopPolicy {
    /// Parse the name used by `llg --stop-policy` and `simulator.stop_policy`.
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "resume" => Ok(Self::Resume),
            "exit" => Ok(Self::Exit),
            _ => Err("expected resume or exit"),
        }
    }

    /// The `LLG_STOP_POLICY` value read by the generated model.
    pub const fn env_value(self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::Exit => "exit",
        }
    }
}

/// A validated, resolved `llg.toml`.
///
/// Optional fields are `None` when the file did not set them, so the driver
/// can apply command-line > environment > config > built-in precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct LlgConfig {
    pub schema_version: u32,
    /// The directory that contains the config file; relative paths resolve
    /// against it.
    pub base_dir: PathBuf,
    pub sources: SourcesConfig,
    pub compile: CompileConfig,
    pub libraries: LibrariesConfig,
    pub analysis: AnalysisConfig,
    pub lint: LintConfig,
    pub lint_run: LintRunConfig,
    pub simulator: SimulatorConfig,
    pub waveform: WaveformConfig,
    pub build: BuildConfig,
    pub output: OutputConfig,
}

/// Source discovery configuration.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourcesConfig {
    /// Normalized absolute source directories (the config directory when the
    /// file names none). Each is also an include-search directory.
    pub directories: Vec<PathBuf>,
    /// Whether the file named `directories` explicitly. The driver discovers
    /// sources only from explicit directories.
    pub directories_configured: bool,
    /// Directory-relative include globs.
    pub include: Vec<String>,
    /// Directory-relative exclude globs; excludes win over includes.
    pub exclude: Vec<String>,
    /// Explicit source files (normalized absolute). Driver only.
    pub files: Vec<PathBuf>,
}

/// Compile configuration.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompileConfig {
    pub top: Option<String>,
    /// Language edition. Driver only.
    pub edition: Option<LanguageEdition>,
    /// Compilation-unit grouping. Driver only.
    pub compilation_units: Option<CompilationUnitMode>,
    /// Normalized absolute include-search directories (may be external).
    pub include_dirs: Vec<PathBuf>,
    /// Validated preprocessor defines (`NAME` or `NAME=VALUE`), raw.
    pub defines: Vec<String>,
    /// Validated top-level parameter overrides, ordered by name. They become
    /// `NAME=VALUE` overrides applying to the top-level module instances.
    pub param_overrides: BTreeMap<String, String>,
    /// VPI system task/function prototypes. Driver only.
    pub system_tasks: Vec<String>,
}

/// Library configuration. Driver only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LibrariesConfig {
    /// Library map files (normalized absolute).
    pub map_files: Vec<PathBuf>,
    /// Library files in `[library=]path` form with the path made absolute.
    pub files: Vec<String>,
    /// Default library search order.
    pub order: Vec<String>,
    /// Name of the default source library.
    pub default: Option<String>,
}

/// Byte budgets applied to the unique root compilation units and literal
/// include dependencies admitted to an LSP analysis. Language server only.
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

/// Driver-side lint run selection (`[lint] only/warnings_as_errors/json/json_file`).
/// The rule settings themselves live in [`LlgConfig::lint`] and are shared.
/// The driver always lints; these choose what follows and how findings gate.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LintRunConfig {
    /// Lint without emitting or running a model (`--lint-only`).
    pub only: Option<bool>,
    /// Treat lint warnings as errors (`-Werror`).
    pub warnings_as_errors: Option<bool>,
    pub json: Option<bool>,
    /// Normalized absolute JSON report file.
    pub json_file: Option<PathBuf>,
}

/// Simulator behavior. Driver only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimulatorConfig {
    pub stop_policy: Option<StopPolicy>,
    pub optimize: Option<bool>,
    /// Arguments passed to the generated simulator.
    pub plusargs: Option<Vec<String>>,
}

/// Waveform dumping requested without `$dumpfile`/`$dumpvars`. Driver only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WaveformConfig {
    /// Normalized absolute `.vcd` or `.fst` output file.
    pub file: Option<PathBuf>,
    /// Hierarchy levels below each top instance; 0 dumps every level.
    pub depth: Option<u32>,
}

/// Model build tooling. Driver only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BuildConfig {
    pub gen_only: Option<bool>,
    pub generator: Option<String>,
    pub launcher: Option<String>,
    pub cc: Option<String>,
    pub cflags: Option<String>,
    pub model_opt_level: Option<ModelOptLevel>,
    pub cmake: Option<String>,
    pub jobs: Option<usize>,
    /// Explicit DPI-C libraries (normalized absolute).
    pub dpi_libs: Vec<PathBuf>,
}

/// Output locations. Driver only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OutputConfig {
    pub out_dir: Option<PathBuf>,
    pub runtime_cache: Option<PathBuf>,
}

/// An error describing why a `llg.toml` failed to parse/validate. The message
/// names the offending key; callers add the file path.
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

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Build the safe-default config used when no valid config has ever loaded.
///
/// The default treats `root` as the sole source directory, discovers
/// recursive `.v`/`.sv`, excludes `.git/**` and `target/**`, and leaves every
/// other setting at its default.
pub fn default_config(root: &Path) -> LlgConfig {
    let root = paths::normalize_absolute_path(root).unwrap_or_else(|| root.to_path_buf());
    LlgConfig {
        schema_version: SCHEMA_VERSION,
        base_dir: root.clone(),
        sources: SourcesConfig {
            directories: vec![root],
            directories_configured: false,
            include: DEFAULT_INCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
            exclude: DEFAULT_EXCLUDE_GLOBS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
            files: Vec::new(),
        },
        compile: CompileConfig::default(),
        libraries: LibrariesConfig::default(),
        analysis: AnalysisConfig::default(),
        lint: LintConfig::default(),
        lint_run: LintRunConfig::default(),
        simulator: SimulatorConfig::default(),
        waveform: WaveformConfig::default(),
        build: BuildConfig::default(),
        output: OutputConfig::default(),
    }
}

/// Validate a `--top`/`compile.top` value: a module name only, without a
/// `library.` prefix or a `:config` suffix.
pub fn validate_top_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("the top module name must not be empty".to_owned());
    }
    if name.contains(['.', ':']) || name.chars().any(char::is_whitespace) {
        return Err(format!(
            "`{name}` is not a module name (give the module name only, without a library or `:config`)"
        ));
    }
    Ok(())
}

/// Whether `path` names a waveform file `llg` can write (`.vcd` or `.fst`,
/// any case), the formats `$dumpfile` accepts.
pub fn is_waveform_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("vcd") || extension.eq_ignore_ascii_case("fst")
        })
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

#[cfg(test)]
mod tests;
