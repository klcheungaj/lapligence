//! Raw serde shape of `llg.toml`. Values are type-checked here and validated
//! in [`super::resolve`]; every table rejects unknown fields.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub sources: RawSources,
    #[serde(default)]
    pub compile: RawCompile,
    #[serde(default)]
    pub libraries: RawLibraries,
    #[serde(default)]
    pub analysis: RawAnalysis,
    #[serde(default)]
    pub lint: RawLint,
    #[serde(default)]
    pub simulator: RawSimulator,
    #[serde(default)]
    pub waveform: RawWaveform,
    #[serde(default)]
    pub build: RawBuild,
    #[serde(default)]
    pub output: RawOutput,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSources {
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCompile {
    pub top: Option<String>,
    pub edition: Option<String>,
    pub compilation_units: Option<String>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub defines: Vec<String>,
    /// Raw parameter-override values; type-checked during resolution so a
    /// wrong type yields a precise error.
    #[serde(default)]
    pub param_overrides: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub system_tasks: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawLibraries {
    #[serde(default)]
    pub map_files: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub order: Vec<String>,
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAnalysis {
    pub max_file_bytes: Option<u64>,
    pub max_total_input_bytes: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawLint {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub rules: BTreeMap<String, RawRule>,
    pub only: Option<bool>,
    pub warnings_as_errors: Option<bool>,
    pub json: Option<bool>,
    pub json_file: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRule {
    pub enabled: Option<bool>,
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSimulator {
    pub stop_policy: Option<String>,
    pub optimize: Option<bool>,
    pub plusargs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawWaveform {
    pub file: Option<String>,
    pub depth: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBuild {
    pub gen_only: Option<bool>,
    pub generator: Option<String>,
    pub launcher: Option<String>,
    pub cc: Option<String>,
    pub cflags: Option<String>,
    pub model_opt_level: Option<String>,
    pub cmake: Option<String>,
    pub jobs: Option<u64>,
    #[serde(default)]
    pub dpi_libs: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawOutput {
    pub out_dir: Option<String>,
    pub runtime_cache: Option<String>,
}
