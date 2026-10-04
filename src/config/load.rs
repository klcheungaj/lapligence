//! Reading and parsing `llg.toml`, including error enrichment with the
//! offending key and line.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::paths;
use super::resolve::resolve;
use super::schema::RawConfig;
use super::{ConfigError, LlgConfig, MAX_CONFIG_BYTES};

/// Result of loading a config file from disk.
#[derive(Debug, Clone)]
pub struct ConfigLoad {
    /// The config file path that was read.
    pub path: PathBuf,
    /// Whether the file does not exist. Callers decide whether that is an
    /// error: the language server and the driver's default discovery treat it
    /// as "use defaults", an explicit driver `--config` does not.
    pub missing: bool,
    /// `Some` when a valid config was produced, `None` when the file was
    /// missing or invalid.
    pub config: Option<LlgConfig>,
    /// Parse/validation errors (empty for a missing or valid file).
    pub errors: Vec<ConfigError>,
    /// Non-fatal load warnings: configured source/include directories that do
    /// not exist on disk, and malformed `defines`/`param_overrides` entries
    /// that were dropped instead of failing the load. They never fail the load.
    pub warnings: Vec<ConfigError>,
}

/// A successfully parsed config plus its non-fatal warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedConfig {
    pub config: LlgConfig,
    pub warnings: Vec<ConfigError>,
}

/// Parse `text` as a `llg.toml` relative to `base_dir` (the directory
/// containing the config file), returning soft warnings alongside.
/// On any structural error the whole config is rejected.
pub fn parse_config_detailed(base_dir: &Path, text: &str) -> Result<ParsedConfig, ConfigError> {
    let base_dir = paths::normalize_absolute_path(base_dir)
        .ok_or_else(|| ConfigError::new("config base directory must be absolute"))?;

    let value: toml::Value = toml::from_str(text)
        .map_err(|error| ConfigError::new(format!("malformed TOML: {error}")))?;
    if !value.is_table() {
        return Err(ConfigError::new("llg.toml must be a TOML table"));
    }
    let raw: RawConfig = toml::from_str(text).map_err(|error| shape_error(text, &error))?;
    let (config, warnings) = resolve(base_dir, &raw)?;
    Ok(ParsedConfig { config, warnings })
}

/// Load and parse the config file at an explicit absolute path.
///
/// Relative paths inside the TOML resolve from the directory containing the
/// file. A missing file produces `missing: true` with no errors; a malformed
/// file is rejected atomically. Oversized and non-UTF-8 files remain read
/// errors. The file read is bounded to `MAX_CONFIG_BYTES` plus one byte.
pub fn load_config_file(path: &Path) -> io::Result<ConfigLoad> {
    let path = paths::normalize_absolute_path(path).ok_or_else(|| {
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
                path,
                missing: false,
                config,
                errors,
                warnings,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ConfigLoad {
            path,
            missing: true,
            config: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        }),
        Err(error) => Err(error),
    }
}

/// Read a config file with a fixed upper bound and validate its UTF-8 before
/// handing it to the TOML parser. The extra byte distinguishes an exactly
/// full file from an oversized one without ever retaining more than the
/// configured bound plus one byte.
fn read_config_text(path: &Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
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
/// disk. Non-fatal: discovery simply finds nothing there until the directory
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

/// Format a shape error (unknown field, wrong type) with its line and dotted
/// key, so the message identifies the key without the caller re-reading the
/// file.
fn shape_error(text: &str, error: &toml::de::Error) -> ConfigError {
    let Some(span) = error.span() else {
        return ConfigError::new(format!("invalid llg.toml: {}", error.message()));
    };
    let offset = span.start.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_number = text[..line_start].matches('\n').count() + 1;
    let line_end = text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index);
    let line = &text[line_start..line_end];
    let location = match key_path(text, line_start, line) {
        Some(key) => format!("line {line_number}, key `{key}`"),
        None => format!("line {line_number}"),
    };
    ConfigError::new(format!(
        "invalid llg.toml at {location}: {}",
        error.message()
    ))
}

/// Dotted key of the `key = value` line (or `[table]` header) beginning at
/// `line_start`, qualified by the nearest preceding table header.
fn key_path(text: &str, line_start: usize, line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(header) = table_header(trimmed) {
        return Some(header);
    }
    let key = trimmed.split_once('=')?.0.trim();
    if key.is_empty() {
        return None;
    }
    let table = text[..line_start]
        .lines()
        .rev()
        .find_map(|previous| table_header(previous.trim()));
    Some(match table {
        Some(table) => format!("{table}.{key}"),
        None => key.to_owned(),
    })
}

fn table_header(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?;
    let inner = inner.strip_prefix('[').unwrap_or(inner);
    let end = inner.find(']')?;
    Some(inner[..end].trim().to_owned())
}
