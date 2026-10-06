//! Validation and path resolution of a raw `llg.toml` into [`LlgConfig`].

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use super::paths::{self, DiscoveryFilters};
use super::schema::{
    RawAnalysis, RawBuild, RawCompile, RawConfig, RawLibraries, RawLint, RawOutput, RawSimulator,
    RawSources, RawWaveform,
};
use super::{
    AnalysisConfig, BuildConfig, CompileConfig, ConfigError, LibrariesConfig, LintRunConfig,
    LlgConfig, OutputConfig, SimulatorConfig, SourcesConfig, StopPolicy, WaveformConfig,
    DEFAULT_EXCLUDE_GLOBS, DEFAULT_INCLUDE_GLOBS, DEFAULT_MAX_FILE_BYTES,
    DEFAULT_MAX_TOTAL_INPUT_BYTES, SCHEMA_VERSION,
};
use crate::core::compile::{CompilationUnitMode, LanguageEdition};
use crate::core::lint::{LintConfig, LintSeverity, RuleConfig};
use crate::sim::build::ModelOptLevel;

/// Resolve `raw` against `base_dir`. Returns the config and the non-fatal
/// warnings (dropped entries).
pub(super) fn resolve(
    base_dir: PathBuf,
    raw: &RawConfig,
) -> Result<(LlgConfig, Vec<ConfigError>), ConfigError> {
    if raw.schema_version != SCHEMA_VERSION {
        return Err(ConfigError::new(format!(
            "schema_version: unsupported value {} (expected {SCHEMA_VERSION})",
            raw.schema_version
        )));
    }
    let sources = resolve_sources(&base_dir, &raw.sources)?;
    let (compile, warnings) = resolve_compile(&base_dir, &raw.compile)?;
    let libraries = resolve_libraries(&base_dir, &raw.libraries)?;
    let analysis = resolve_analysis(&raw.analysis)?;
    let lint = translate_lint(&raw.lint)?;
    let lint_run = resolve_lint_run(&base_dir, &raw.lint)?;
    let simulator = resolve_simulator(&raw.simulator)?;
    let waveform = resolve_waveform(&base_dir, &raw.waveform)?;
    let build = resolve_build(&base_dir, &raw.build)?;
    let output = resolve_output(&base_dir, &raw.output)?;
    Ok((
        LlgConfig {
            schema_version: raw.schema_version,
            base_dir,
            sources,
            compile,
            libraries,
            analysis,
            lint,
            lint_run,
            simulator,
            waveform,
            build,
            output,
        },
        warnings,
    ))
}

/// Discover the `.v`/`.sv` compilation units below every configured source
/// directory, applying the include/exclude filters. Deduplicated and sorted by
/// normalized path.
pub fn discover_sources(config: &LlgConfig) -> io::Result<Vec<PathBuf>> {
    let filters = DiscoveryFilters::new(&config.sources.include, &config.sources.exclude);
    let mut units = BTreeMap::new();
    for dir in &config.sources.directories {
        for file in paths::discover_files(dir, &filters)? {
            if paths::is_compilation_unit(&file.path) {
                units.entry(file.path).or_insert(());
            }
        }
    }
    Ok(units.into_keys().collect())
}

fn resolve_sources(base_dir: &Path, raw: &RawSources) -> Result<SourcesConfig, ConfigError> {
    // A present `[sources]` table WITHOUT `include` would otherwise yield an
    // empty glob list and silently discover nothing; fall back to the same
    // recursive `.v`/`.sv` defaults a missing config uses.  Exclude patterns
    // always sit ON TOP of the built-in excludes (defense-in-depth:
    // built-in exclusions must never be lost), so an explicit-empty
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

    let directories_configured = !raw.directories.is_empty();
    let directories = if directories_configured {
        resolve_paths(base_dir, &raw.directories, "sources.directories")?
    } else {
        vec![base_dir.to_path_buf()]
    };
    let files = resolve_paths(base_dir, &raw.files, "sources.files")?;

    Ok(SourcesConfig {
        directories,
        directories_configured,
        include,
        exclude,
        files,
    })
}

fn resolve_compile(
    base_dir: &Path,
    raw: &RawCompile,
) -> Result<(CompileConfig, Vec<ConfigError>), ConfigError> {
    let include_dirs = resolve_paths(base_dir, &raw.include_dirs, "compile.include_dirs")?;
    let top = non_empty("compile.top", raw.top.as_deref())?;
    if let Some(top) = &top {
        super::validate_top_name(top)
            .map_err(|error| ConfigError::new(format!("compile.top: {error}")))?;
    }
    let edition = raw
        .edition
        .as_deref()
        .map(|value| {
            value
                .parse::<LanguageEdition>()
                .map_err(|error| ConfigError::new(format!("compile.edition: {error}")))
        })
        .transpose()?;
    let compilation_units = raw
        .compilation_units
        .as_deref()
        .map(|value| {
            value
                .parse::<CompilationUnitMode>()
                .map_err(|error| ConfigError::new(format!("compile.compilation_units: {error}")))
        })
        .transpose()?;
    let (defines, mut warnings) = validate_defines(&raw.defines);
    let (param_overrides, mut override_warnings) = resolve_param_overrides(raw)?;
    warnings.append(&mut override_warnings);
    for entry in &raw.system_tasks {
        if entry.is_empty() {
            return Err(ConfigError::new(
                "compile.system_tasks: prototype must not be empty",
            ));
        }
    }
    Ok((
        CompileConfig {
            top,
            edition,
            compilation_units,
            include_dirs,
            defines,
            param_overrides,
            system_tasks: raw.system_tasks.clone(),
        },
        warnings,
    ))
}

fn resolve_libraries(base_dir: &Path, raw: &RawLibraries) -> Result<LibrariesConfig, ConfigError> {
    let map_files = resolve_paths(base_dir, &raw.map_files, "libraries.map_files")?;
    // `library=path` or `path`; only the path part is relative to the config.
    let mut files = Vec::with_capacity(raw.files.len());
    for entry in &raw.files {
        let (library, path) = match entry.split_once('=') {
            Some((library, path)) => (Some(library), path),
            None => (None, entry.as_str()),
        };
        if library.is_some_and(str::is_empty) || path.is_empty() {
            return Err(ConfigError::new(format!(
                "libraries.files: `{entry}` must use `library=path` or `path` with nonempty values"
            )));
        }
        let path = resolve_path(base_dir, path, "libraries.files")?;
        files.push(match library {
            Some(library) => format!("{library}={}", path.display()),
            None => path.to_string_lossy().into_owned(),
        });
    }
    for name in &raw.order {
        if name.is_empty() || name.contains(',') {
            return Err(ConfigError::new(format!(
                "libraries.order: `{name}` must be a nonempty library name without commas"
            )));
        }
    }
    Ok(LibrariesConfig {
        map_files,
        files,
        order: raw.order.clone(),
        default: non_empty("libraries.default", raw.default.as_deref())?,
    })
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

fn resolve_lint_run(base_dir: &Path, raw: &RawLint) -> Result<LintRunConfig, ConfigError> {
    let json_file = raw
        .json_file
        .as_deref()
        .map(|entry| resolve_path(base_dir, entry, "lint.json_file"))
        .transpose()?;
    Ok(LintRunConfig {
        only: raw.only,
        warnings_as_errors: raw.warnings_as_errors,
        json: raw.json,
        json_file,
    })
}

fn resolve_simulator(raw: &RawSimulator) -> Result<SimulatorConfig, ConfigError> {
    let stop_policy = raw
        .stop_policy
        .as_deref()
        .map(|value| {
            StopPolicy::parse(value)
                .map_err(|error| ConfigError::new(format!("simulator.stop_policy: {error}")))
        })
        .transpose()?;
    Ok(SimulatorConfig {
        stop_policy,
        optimize: raw.optimize,
        plusargs: raw.plusargs.clone(),
    })
}

fn resolve_waveform(base_dir: &Path, raw: &RawWaveform) -> Result<WaveformConfig, ConfigError> {
    let file = raw
        .file
        .as_deref()
        .map(|entry| resolve_path(base_dir, entry, "waveform.file"))
        .transpose()?;
    if let Some(file) = &file {
        if !super::is_waveform_file(file) {
            return Err(ConfigError::new(format!(
                "waveform.file: `{}` must end in .vcd or .fst",
                file.display()
            )));
        }
    }
    let depth = raw
        .depth
        .map(|depth| {
            u32::try_from(depth).map_err(|_| {
                ConfigError::new(format!(
                    "waveform.depth: {depth} is out of range (expected 0 to {})",
                    u32::MAX
                ))
            })
        })
        .transpose()?;
    if depth.is_some() && file.is_none() {
        return Err(ConfigError::new("waveform.depth requires waveform.file"));
    }
    Ok(WaveformConfig { file, depth })
}

fn resolve_build(base_dir: &Path, raw: &RawBuild) -> Result<BuildConfig, ConfigError> {
    let model_opt_level = raw
        .model_opt_level
        .as_deref()
        .map(|value| {
            ModelOptLevel::parse(value)
                .map_err(|error| ConfigError::new(format!("build.model_opt_level: {error}")))
        })
        .transpose()?;
    let jobs = match raw.jobs {
        None => None,
        Some(0) => return Err(ConfigError::new("build.jobs must be a positive integer")),
        Some(value) => Some(
            usize::try_from(value)
                .map_err(|_| ConfigError::new(format!("build.jobs: {value} is out of range")))?,
        ),
    };
    Ok(BuildConfig {
        gen_only: raw.gen_only,
        generator: non_empty("build.generator", raw.generator.as_deref())?,
        launcher: non_empty("build.launcher", raw.launcher.as_deref())?,
        cc: non_empty("build.cc", raw.cc.as_deref())?,
        // Empty flags are meaningful: they clear an inherited $LLG_CFLAGS.
        cflags: raw.cflags.clone(),
        model_opt_level,
        cmake: non_empty("build.cmake", raw.cmake.as_deref())?,
        jobs,
        dpi_libs: resolve_paths(base_dir, &raw.dpi_libs, "build.dpi_libs")?,
    })
}

fn resolve_output(base_dir: &Path, raw: &RawOutput) -> Result<OutputConfig, ConfigError> {
    Ok(OutputConfig {
        out_dir: raw
            .out_dir
            .as_deref()
            .map(|entry| resolve_path(base_dir, entry, "output.out_dir"))
            .transpose()?,
        runtime_cache: raw
            .runtime_cache
            .as_deref()
            .map(|entry| resolve_path(base_dir, entry, "output.runtime_cache"))
            .transpose()?,
    })
}

fn non_empty(field: &str, value: Option<&str>) -> Result<Option<String>, ConfigError> {
    match value {
        Some("") => Err(ConfigError::new(format!("{field} must not be empty"))),
        other => Ok(other.map(str::to_owned)),
    }
}

fn resolve_path(base_dir: &Path, entry: &str, field: &str) -> Result<PathBuf, ConfigError> {
    if entry.is_empty() {
        return Err(ConfigError::new(format!("{field}: path must not be empty")));
    }
    let raw_path = Path::new(entry);
    let absolute = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    };
    paths::normalize_absolute_path(&absolute)
        .ok_or_else(|| ConfigError::new(format!("{field} must resolve to an absolute path")))
}

/// Resolve a list of paths, dropping later duplicates.
fn resolve_paths(
    base_dir: &Path,
    entries: &[String],
    field: &str,
) -> Result<Vec<PathBuf>, ConfigError> {
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for entry in entries {
        let path = resolve_path(base_dir, entry, field)?;
        if seen.insert(path.clone()) {
            resolved.push(path);
        }
    }
    Ok(resolved)
}

fn normalize_patterns(field: &str, patterns: &[String]) -> Result<Vec<String>, ConfigError> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for pattern in patterns {
        let normalized_pattern = paths::normalize_relative_pattern(pattern)
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

/// Values become C-string arguments at the Slang FFI boundary
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
    crate::core::lint::LintRegistry::default_rules()
        .all()
        .iter()
        .map(|rule| rule.id())
        .collect()
}
