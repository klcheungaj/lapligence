//! Effective driver settings: the explicit `llg.toml` and the precedence
//! between the command line, the environment, the config file and built-in
//! defaults.
//!
//! Precedence, highest first: command line, environment (`$LLG_CC`,
//! `$LLG_CFLAGS`, `$LLG_C_LAUNCHER`, ...), `llg.toml`, built-in default. [`layered`] is the one
//! place that expresses this order; every option that has an environment
//! variable goes through it. `llg` reads a config file only when `--config`
//! names it; it never discovers `llg.toml` in the current directory.
//!
//! A scalar from a higher layer replaces the lower one. A list option given on
//! the command line replaces the config list (several occurrences accumulate
//! among themselves); its `--append-<list>` twin adds to the list instead, after
//! the replacing values when both are given. Source files on the command line
//! replace `sources.files` and the directory discovery, and `--` replaces the
//! configured plusargs even when nothing follows it. Only the config supplies
//! lists (no list option has an environment variable). Duplicates: a later
//! `NAME[=VALUE]` define or `NAME=VALUE` parameter override replaces an earlier
//! one for the same `NAME`; any other list keeps the first occurrence of an
//! identical entry, and source files are compared by canonical path. Plusargs
//! are never deduplicated.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use llg::config::{self, LlgConfig, StopPolicy};
use llg::core::compile;
use llg::sim;

use crate::cli::Cli;

/// Default output root: models go to `build/sim/<design>` and the runtime
/// cache to `build/llg-runtime-cache`, both under the current directory.
pub(crate) const DEFAULT_OUT_DIR: &str = "build";

/// Resolved options for one driver run.
#[derive(Debug)]
pub(crate) struct DriverOptions {
    pub top: Option<String>,
    pub edition: compile::LanguageEdition,
    pub compilation_unit_mode: compile::CompilationUnitMode,
    pub include_dirs: Vec<String>,
    pub defines: Vec<String>,
    pub param_overrides: Vec<String>,
    pub system_subroutines: Vec<String>,
    pub library_map_files: Vec<String>,
    pub library_files: Vec<String>,
    pub library_order: Vec<String>,
    pub default_library: Option<String>,
    pub files: Vec<String>,
    pub runtime_args: Vec<String>,
    /// Stop after lint (`--lint-only`, implied by a JSON report).
    pub lint_only: bool,
    /// Lint warnings stop the run like errors (`-Werror`).
    pub warnings_as_errors: bool,
    pub lint_json_mode: bool,
    pub lint_json_path: Option<PathBuf>,
    /// Rule settings from the `--config` file's `[lint]`, or the defaults.
    pub lint_config: llg::core::lint::LintConfig,
    /// Waveform dumping from time 0 (`--wave`, `[waveform]`).
    pub wave: Option<sim::codegen::WaveformOptions>,
    pub generator: Option<String>,
    pub dpi_libraries: Vec<PathBuf>,
    pub launcher: Option<String>,
    pub cc: Option<String>,
    pub cflags: Option<String>,
    pub model_opt_level: sim::build::ModelOptLevel,
    pub cmake: Option<String>,
    pub build_jobs: Option<usize>,
    pub out_dir: PathBuf,
    pub runtime_cache: Option<PathBuf>,
    pub gen_only: bool,
    pub no_opt: bool,
    pub stop_policy: StopPolicy,
    /// Whether the command line itself named a build-tool option, so
    /// `--gen-only` can warn that it is ignored. Config values do not warn.
    pub cli_build_options: bool,
}

/// Environment fallbacks for the options that have them. Read once, so the
/// precedence code never touches the process environment.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Env {
    /// `$LLG_CC`, else `$CC`; an empty value counts as unset.
    pub cc: Option<String>,
    /// `$LLG_CFLAGS`; an empty value is kept because it means "no flags".
    pub cflags: Option<String>,
    /// `$LLG_CMAKE`; empty counts as unset.
    pub cmake: Option<String>,
    /// `$CMAKE_GENERATOR`; empty counts as unset.
    pub generator: Option<String>,
    /// `$LLG_C_LAUNCHER`; empty counts as unset.
    pub launcher: Option<String>,
    /// `$CMAKE_BUILD_PARALLEL_LEVEL` when it is a positive integer.
    pub build_jobs: Option<usize>,
    /// `$LLG_RUNTIME_CACHE_DIR`; empty counts as unset.
    pub runtime_cache: Option<PathBuf>,
}

impl Env {
    pub(crate) fn from_process() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub(crate) fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let non_empty = |name: &str| get(name).filter(|value| !value.is_empty());
        Self {
            cc: non_empty("LLG_CC").or_else(|| non_empty("CC")),
            cflags: get("LLG_CFLAGS"),
            cmake: non_empty("LLG_CMAKE"),
            generator: non_empty("CMAKE_GENERATOR"),
            launcher: non_empty(sim::build::C_LAUNCHER_ENV).map(|value| value.trim().to_owned()),
            build_jobs: get(sim::build::BUILD_PARALLEL_LEVEL_ENV)
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|jobs| *jobs > 0),
            runtime_cache: non_empty(sim::build::RUNTIME_CACHE_DIR_ENV).map(PathBuf::from),
        }
    }
}

/// The precedence rule for a scalar option: command line, then environment,
/// then config file. `None` means the built-in default applies.
fn layered<T>(cli: Option<T>, env: Option<T>, config: Option<T>) -> Option<T> {
    cli.or(env).or(config)
}

/// A config problem that stops the driver, with the message to print.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SettingsError(pub String);

/// Load the config file named by `--config` (relative to the current
/// directory), which must exist. Without `--config` no file is read: `llg`
/// does not discover `llg.toml`. Warnings (dropped entries, missing
/// directories) are printed to stderr.
pub(crate) fn load_config(explicit: Option<&Path>) -> Result<Option<LlgConfig>, SettingsError> {
    let Some(explicit) = explicit else {
        return Ok(None);
    };
    let cwd = std::env::current_dir().map_err(|error| {
        SettingsError(format!("cannot determine the current directory: {error}"))
    })?;
    load_config_in(&cwd, explicit).map(Some)
}

fn load_config_in(cwd: &Path, explicit: &Path) -> Result<LlgConfig, SettingsError> {
    let path = cwd.join(explicit);
    let load = config::load_config_file(&path).map_err(|error| {
        SettingsError(format!("cannot read config {}: {error}", path.display()))
    })?;
    if load.missing {
        return Err(SettingsError(format!(
            "config file {} does not exist",
            path.display()
        )));
    }
    for warning in &load.warnings {
        eprintln!("llg: warning: {}: {}", load.path.display(), warning.message);
    }
    load.config.ok_or_else(|| {
        SettingsError(
            load.errors
                .iter()
                .map(|error| format!("invalid config {}: {}", load.path.display(), error.message))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A list option: the command-line replacement when one was given, else the
/// configured entries, followed by the `--append-<list>` entries.
fn listed<T>(
    config: impl IntoIterator<Item = T>,
    replace: Option<Vec<T>>,
    append: Vec<T>,
) -> Vec<T> {
    let mut merged: Vec<T> = replace.unwrap_or_else(|| config.into_iter().collect());
    merged.extend(append);
    merged
}

/// A repeatable option's values, or `None` when it was not given.
fn given<T>(values: Vec<T>) -> Option<Vec<T>> {
    (!values.is_empty()).then_some(values)
}

/// Drop repeated entries, keeping the first of each `key`.
fn dedup_first<T, K: Ord>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

/// Drop earlier entries that a later entry with the same `key` replaces.
fn dedup_last<T, K: Ord>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = BTreeSet::new();
    let mut kept: Vec<T> = items
        .into_iter()
        .rev()
        .filter(|item| seen.insert(key(item)))
        .collect();
    kept.reverse();
    kept
}

/// The `NAME` of a `NAME`, `NAME=VALUE` define or parameter override.
fn entry_name(entry: &str) -> String {
    entry.split('=').next().unwrap_or(entry).to_owned()
}

/// Identity of a source file for duplicate detection: its canonical path when
/// it exists, else the text as written.
fn source_identity(file: &str) -> PathBuf {
    std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file))
}

/// Apply the precedence rules. `config` is `None` when `--config` was not
/// given. Source discovery (when the config names directories) touches the
/// filesystem.
pub(crate) fn resolve(
    cli: Cli,
    env: &Env,
    config: Option<&LlgConfig>,
) -> Result<DriverOptions, SettingsError> {
    let empty = config::default_config(Path::new(if cfg!(windows) { "C:\\" } else { "/" }));
    let config = config.unwrap_or(&empty);
    let append = cli.append;

    // Sources: files named on the command line replace the config's explicit
    // files and the discovery under its explicit directories; otherwise the
    // config supplies them. `--append-source` files follow either way.
    let replaced_sources = given(cli.files);
    let mut config_files: Vec<String> = Vec::new();
    let mut source_dirs: Vec<String> = Vec::new();
    if replaced_sources.is_none() {
        config_files.extend(config.sources.files.iter().map(|path| path_string(path)));
        if config.sources.directories_configured {
            let discovered = config::discover_sources(config).map_err(|error| {
                SettingsError(format!(
                    "cannot discover sources from {}: {error}",
                    config::CONFIG_FILE
                ))
            })?;
            config_files.extend(discovered.iter().map(|path| path_string(path)));
            source_dirs = config
                .sources
                .directories
                .iter()
                .map(|path| path_string(path))
                .collect();
        }
    }
    let files = dedup_first(
        listed(config_files, replaced_sources, append.files),
        |file| source_identity(file),
    );

    // Source directories are include-search directories, as in the language
    // server; they follow the sources, not the include-directory list.
    let include_dirs = dedup_first(
        source_dirs
            .into_iter()
            .chain(listed(
                config
                    .compile
                    .include_dirs
                    .iter()
                    .map(|path| path_string(path)),
                given(cli.include_dirs),
                append.include_dirs,
            ))
            .collect(),
        String::clone,
    );

    let defines = dedup_last(
        listed(
            config.compile.defines.iter().cloned(),
            given(cli.defines),
            append.defines,
        ),
        |entry| entry_name(entry),
    );
    let param_overrides = dedup_last(
        listed(
            config
                .compile
                .param_overrides
                .iter()
                .map(|(name, value)| format!("{name}={value}")),
            given(cli.param_overrides),
            append.param_overrides,
        ),
        |entry| entry_name(entry),
    );
    let system_subroutines = dedup_first(
        listed(
            config.compile.system_tasks.iter().cloned(),
            given(cli.system_subroutines),
            append.system_subroutines,
        ),
        String::clone,
    );
    let library_map_files = dedup_first(
        listed(
            config
                .libraries
                .map_files
                .iter()
                .map(|path| path_string(path)),
            given(cli.library_map_files),
            append.library_map_files,
        ),
        String::clone,
    );
    let library_files = dedup_first(
        listed(
            config.libraries.files.iter().cloned(),
            given(cli.library_files),
            append.library_files,
        ),
        String::clone,
    );
    let library_order = dedup_first(
        listed(
            config.libraries.order.iter().cloned(),
            given(cli.library_order),
            append.library_order,
        ),
        String::clone,
    );
    let dpi_libraries = dedup_first(
        listed(
            config.build.dpi_libs.iter().cloned(),
            given(cli.dpi_libraries),
            append.dpi_libraries,
        ),
        PathBuf::clone,
    );
    // `--` replaces the configured plusargs even when empty; repeats are
    // meaningful, so nothing is deduplicated.
    let runtime_args = listed(
        config.simulator.plusargs.iter().flatten().cloned(),
        cli.runtime_args,
        append.runtime_args,
    );

    // `--no-lint-only` also cancels a configured JSON report.
    let lint_json_mode = cli.lint_json_mode.unwrap_or(
        config
            .lint_run
            .json
            .unwrap_or(config.lint_run.json_file.is_some()),
    );
    // A command-line `--lint-json` chooses its own destination, stdout when it
    // names none; the file's `json_file` applies only to the file's own mode.
    let lint_json_path = if cli.lint_json_mode.is_some() {
        cli.lint_json_path
    } else {
        config.lint_run.json_file.clone()
    };
    // A JSON report replaces the run, so it implies lint-only.
    let lint_only = lint_json_mode || cli.lint_only.or(config.lint_run.only).unwrap_or(false);
    let warnings_as_errors = cli
        .warnings_as_errors
        .or(config.lint_run.warnings_as_errors)
        .unwrap_or(false);

    // `--wave` replaces the configured file and `--no-wave` cancels it; the
    // depth layers independently but needs a file.
    let wave_file = match cli.wave_file {
        Some(file) => file,
        None => config.waveform.file.clone(),
    };
    let wave_depth = cli.wave_depth.or(config.waveform.depth);
    let wave = match wave_file {
        Some(file) => Some(sim::codegen::WaveformOptions {
            file: file.into_os_string().into_string().map_err(|file| {
                SettingsError(format!(
                    "waveform file {} is not valid UTF-8",
                    PathBuf::from(file).display()
                ))
            })?,
            depth: wave_depth.unwrap_or(0),
        }),
        None if cli.wave_depth.is_some() => {
            return Err(SettingsError(
                "--wave-depth requires --wave <file> or a [waveform] file".to_owned(),
            ));
        }
        None => None,
    };

    let cli_build_options = cli.generator.is_some()
        || cli.launcher.is_some()
        || cli.cc.is_some()
        || cli.cflags.is_some()
        || cli.cmake.is_some()
        || cli.build_jobs.is_some()
        || cli.runtime_cache.is_some();

    Ok(DriverOptions {
        top: cli.top.or_else(|| config.compile.top.clone()),
        edition: cli.edition.or(config.compile.edition).unwrap_or_default(),
        compilation_unit_mode: cli
            .compilation_unit_mode
            .or(config.compile.compilation_units)
            .unwrap_or_default(),
        include_dirs,
        defines,
        param_overrides,
        system_subroutines,
        library_map_files,
        library_files,
        library_order,
        default_library: cli
            .default_library
            .or_else(|| config.libraries.default.clone()),
        files,
        runtime_args,
        lint_only,
        warnings_as_errors,
        lint_json_mode,
        lint_json_path,
        lint_config: config.lint.clone(),
        wave,
        generator: layered(
            cli.generator,
            env.generator.clone(),
            config.build.generator.clone(),
        ),
        dpi_libraries,
        launcher: layered(
            cli.launcher,
            env.launcher.clone(),
            config.build.launcher.clone(),
        ),
        cc: layered(cli.cc, env.cc.clone(), config.build.cc.clone()),
        cflags: layered(cli.cflags, env.cflags.clone(), config.build.cflags.clone()),
        model_opt_level: cli
            .model_opt_level
            .or(config.build.model_opt_level)
            .unwrap_or_default(),
        cmake: layered(cli.cmake, env.cmake.clone(), config.build.cmake.clone()),
        build_jobs: layered(cli.build_jobs, env.build_jobs, config.build.jobs),
        out_dir: cli
            .out_dir
            .or_else(|| config.output.out_dir.clone())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT_DIR)),
        runtime_cache: layered(
            cli.runtime_cache,
            env.runtime_cache.clone(),
            config.output.runtime_cache.clone(),
        ),
        gen_only: cli.gen_only.or(config.build.gen_only).unwrap_or(false),
        no_opt: !cli.optimize.or(config.simulator.optimize).unwrap_or(true),
        stop_policy: cli
            .stop_policy
            .or(config.simulator.stop_policy)
            .unwrap_or(StopPolicy::Resume),
        cli_build_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;

    fn cli(args: &[&str]) -> Cli {
        parse_args(args.iter().map(|arg| (*arg).to_owned()).collect()).expect("valid arguments")
    }

    fn resolve_no_env(
        cli: Cli,
        config: Option<&LlgConfig>,
    ) -> Result<DriverOptions, SettingsError> {
        resolve(cli, &Env::default(), config)
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("llg_settings_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn config(dir: &Path, text: &str) -> LlgConfig {
        config::parse_config_detailed(dir, text)
            .expect("valid config")
            .config
    }

    const FULL: &str = r#"schema_version = 1
[compile]
top = "cfg_top"
edition = "v2001"
compilation_units = "merged"
include_dirs = ["inc"]
defines = ["CFG_A", "CFG_B=2"]
system_tasks = ["$cfg_task()"]
[compile.param_overrides]
DEPTH = 8
[libraries]
map_files = ["lib.map"]
files = ["lib=l.sv"]
order = ["lib"]
default = "cfglib"
[simulator]
stop_policy = "exit"
optimize = false
plusargs = ["+cfg"]
[waveform]
file = "cfg.fst"
depth = 3
[lint]
only = true
warnings_as_errors = true
[build]
generator = "Ninja"
cc = "cfgcc"
cflags = "-DCFG"
model_opt_level = "O1"
jobs = 2
dpi_libs = ["x.so"]
[output]
out_dir = "cfgout"
runtime_cache = "cfgcache"
[sources]
files = ["a.sv"]
"#;

    #[test]
    fn defaults_without_a_config_match_the_historic_driver() {
        let options = resolve_no_env(cli(&["d.sv"]), None).unwrap();
        assert_eq!(options.files, ["d.sv"]);
        assert_eq!(options.top, None);
        assert_eq!(options.out_dir, PathBuf::from(DEFAULT_OUT_DIR));
        assert_eq!(options.stop_policy, StopPolicy::Resume);
        assert!(!options.no_opt && !options.gen_only);
        assert!(!options.lint_only && !options.warnings_as_errors && !options.lint_json_mode);
        assert_eq!(options.wave, None);
        assert!(options.include_dirs.is_empty() && options.runtime_args.is_empty());
    }

    #[test]
    fn config_values_apply_when_the_command_line_is_silent() {
        let dir = temp("silent");
        let options = resolve_no_env(cli(&[]), Some(&config(&dir, FULL))).unwrap();
        assert_eq!(options.top.as_deref(), Some("cfg_top"));
        assert_eq!(options.edition, compile::LanguageEdition::Verilog2001);
        assert_eq!(
            options.compilation_unit_mode,
            compile::CompilationUnitMode::Merged
        );
        assert_eq!(options.include_dirs, [path_string(&dir.join("inc"))]);
        assert_eq!(options.defines, ["CFG_A", "CFG_B=2"]);
        assert_eq!(options.param_overrides, ["DEPTH=8"]);
        assert_eq!(options.system_subroutines, ["$cfg_task()"]);
        assert_eq!(
            options.library_map_files,
            [path_string(&dir.join("lib.map"))]
        );
        assert_eq!(options.library_order, ["lib"]);
        assert_eq!(options.default_library.as_deref(), Some("cfglib"));
        assert_eq!(options.files, [path_string(&dir.join("a.sv"))]);
        assert_eq!(options.runtime_args, ["+cfg"]);
        assert_eq!(options.stop_policy, StopPolicy::Exit);
        assert_eq!(
            options.wave,
            Some(sim::codegen::WaveformOptions {
                file: path_string(&dir.join("cfg.fst")),
                depth: 3,
            })
        );
        assert!(options.lint_only && options.warnings_as_errors);
        assert!(options.no_opt);
        assert_eq!(options.generator.as_deref(), Some("Ninja"));
        assert_eq!(options.cc.as_deref(), Some("cfgcc"));
        assert_eq!(options.cflags.as_deref(), Some("-DCFG"));
        assert_eq!(options.model_opt_level, sim::build::ModelOptLevel::O1);
        assert_eq!(options.build_jobs, Some(2));
        assert_eq!(options.dpi_libraries, [dir.join("x.so")]);
        assert_eq!(options.out_dir, dir.join("cfgout"));
        assert_eq!(options.runtime_cache, Some(dir.join("cfgcache")));
        assert!(!options.cli_build_options, "config values do not warn");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn command_line_scalars_replace_config_scalars() {
        let dir = temp("scalars");
        let options = resolve_no_env(
            cli(&[
                "--top",
                "cli_top",
                "--edition",
                "sv2009",
                "--compilation-units",
                "separate",
                "--default-library",
                "clilib",
                "--stop-policy",
                "resume",
                "--wave",
                "cli.vcd",
                "--no-lint-only",
                "-Wno-error",
                "--opt",
                "--generator",
                "Make",
                "--cc",
                "clicc",
                "--cflags",
                "",
                "--model-opt-level",
                "Os",
                "--cmake",
                "clicmake",
                "--build-jobs",
                "5",
                "--out-dir",
                "cliout",
                "--runtime-cache",
                "clicache",
                "--launcher",
                "ccache",
                "--no-gen-only",
                "x.sv",
            ]),
            Some(&config(&dir, FULL)),
        )
        .unwrap();
        assert_eq!(options.top.as_deref(), Some("cli_top"));
        assert_eq!(options.edition, compile::LanguageEdition::SystemVerilog2009);
        assert_eq!(
            options.compilation_unit_mode,
            compile::CompilationUnitMode::Separate
        );
        assert_eq!(options.default_library.as_deref(), Some("clilib"));
        assert_eq!(options.stop_policy, StopPolicy::Resume);
        assert_eq!(
            options.wave,
            Some(sim::codegen::WaveformOptions {
                file: "cli.vcd".to_owned(),
                depth: 3,
            }),
            "--wave replaces the file; the configured depth still applies"
        );
        assert!(!options.lint_only && !options.warnings_as_errors);
        assert!(!options.no_opt, "--opt overrides optimize = false");
        assert_eq!(options.generator.as_deref(), Some("Make"));
        assert_eq!(options.cc.as_deref(), Some("clicc"));
        assert_eq!(
            options.cflags.as_deref(),
            Some(""),
            "empty CLI flags clear config flags"
        );
        assert_eq!(options.model_opt_level, sim::build::ModelOptLevel::Os);
        assert_eq!(options.cmake.as_deref(), Some("clicmake"));
        assert_eq!(options.build_jobs, Some(5));
        assert_eq!(options.out_dir, PathBuf::from("cliout"));
        assert_eq!(options.runtime_cache, Some(PathBuf::from("clicache")));
        assert!(options.cli_build_options);
        assert!(!options.gen_only);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn list_options_on_the_command_line_replace_the_config_lists() {
        let dir = temp("replace");
        let options = resolve_no_env(
            cli(&[
                "-I",
                "cli_inc",
                "-D",
                "CLI_ONLY",
                "--param-override",
                "WIDTH=3",
                "--define-system-task",
                "$cli()",
                "--libmap",
                "cli.map",
                "--libfile",
                "cl=c.sv",
                "-L",
                "a,b",
                "--dpi-lib",
                "cli.so",
                "x.sv",
                "--",
                "+cli",
            ]),
            Some(&config(&dir, FULL)),
        )
        .unwrap();
        assert_eq!(options.include_dirs, ["cli_inc"]);
        assert_eq!(options.defines, ["CLI_ONLY"]);
        assert_eq!(options.param_overrides, ["WIDTH=3"]);
        assert_eq!(options.system_subroutines, ["$cli()"]);
        assert_eq!(options.library_map_files, ["cli.map"]);
        assert_eq!(options.library_files, ["cl=c.sv"]);
        assert_eq!(options.library_order, ["a", "b"]);
        assert_eq!(options.dpi_libraries, [PathBuf::from("cli.so")]);
        assert_eq!(options.runtime_args, ["+cli"]);
        assert_eq!(
            options.files,
            ["x.sv"],
            "command-line files replace sources.files"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn repeated_replace_options_accumulate_among_themselves() {
        let dir = temp("replace_many");
        let options = resolve_no_env(
            cli(&[
                "-D", "A", "-D", "B=2", "-I", "i1", "-I", "i2", "x.sv", "y.sv",
            ]),
            Some(&config(&dir, FULL)),
        )
        .unwrap();
        assert_eq!(options.defines, ["A", "B=2"]);
        assert_eq!(options.include_dirs, ["i1", "i2"]);
        assert_eq!(options.files, ["x.sv", "y.sv"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn append_options_extend_the_config_lists() {
        let dir = temp("append");
        let options = resolve_no_env(
            cli(&[
                "--append-include-dir",
                "cli_inc",
                "--append-define",
                "CLI_ONLY",
                "--append-param-override",
                "WIDTH=3",
                "--append-define-system-task",
                "$cli()",
                "--append-libmap",
                "cli.map",
                "--append-libfile",
                "cl=c.sv",
                "--append-library-order",
                "a,b",
                "--append-dpi-lib",
                "cli.so",
                "--append-source",
                "x.sv",
                "--append-plusarg",
                "+cli",
            ]),
            Some(&config(&dir, FULL)),
        )
        .unwrap();
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("inc")), "cli_inc".to_owned()]
        );
        assert_eq!(options.defines, ["CFG_A", "CFG_B=2", "CLI_ONLY"]);
        assert_eq!(options.param_overrides, ["DEPTH=8", "WIDTH=3"]);
        assert_eq!(options.system_subroutines, ["$cfg_task()", "$cli()"]);
        assert_eq!(
            options.library_map_files,
            [path_string(&dir.join("lib.map")), "cli.map".to_owned()]
        );
        assert_eq!(
            options.library_files,
            [
                format!("lib={}", path_string(&dir.join("l.sv"))),
                "cl=c.sv".to_owned()
            ]
        );
        assert_eq!(options.library_order, ["lib", "a", "b"]);
        assert_eq!(
            options.dpi_libraries,
            [dir.join("x.so"), PathBuf::from("cli.so")]
        );
        assert_eq!(options.runtime_args, ["+cfg", "+cli"]);
        assert_eq!(
            options.files,
            [path_string(&dir.join("a.sv")), "x.sv".to_owned()],
            "appended sources follow sources.files"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn append_without_a_config_is_the_whole_list() {
        let options = resolve_no_env(
            cli(&[
                "--append-define",
                "A",
                "--append-source",
                "x.sv",
                "--append-plusarg",
                "+p",
            ]),
            None,
        )
        .unwrap();
        assert_eq!(options.defines, ["A"]);
        assert_eq!(options.files, ["x.sv"]);
        assert_eq!(options.runtime_args, ["+p"]);
    }

    #[test]
    fn replace_values_are_the_base_and_append_values_follow() {
        let dir = temp("mixed");
        let cfg = config(&dir, FULL);
        let options = resolve_no_env(
            cli(&[
                "--append-define",
                "APP_FIRST",
                "-D",
                "REP",
                "--append-define",
                "APP2",
                "-I",
                "rep_inc",
                "--append-include-dir",
                "app_inc",
                "-L",
                "r",
                "--append-library-order",
                "p",
                "r2.sv",
                "--append-source",
                "a2.sv",
                "--append-plusarg",
                "+app",
                "--",
                "+rep",
            ]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            options.defines,
            ["REP", "APP_FIRST", "APP2"],
            "the config list is replaced; the order of the two kinds on the command line does not matter"
        );
        assert_eq!(options.include_dirs, ["rep_inc", "app_inc"]);
        assert_eq!(options.library_order, ["r", "p"]);
        assert_eq!(options.files, ["r2.sv", "a2.sv"]);
        assert_eq!(options.runtime_args, ["+rep", "+app"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn each_option_changes_only_its_own_list() {
        let dir = temp("own_list");
        let cfg = config(&dir, FULL);
        let lists = |options: &DriverOptions| {
            [
                format!("{:?}", options.include_dirs),
                format!("{:?}", options.defines),
                format!("{:?}", options.param_overrides),
                format!("{:?}", options.system_subroutines),
                format!("{:?}", options.library_map_files),
                format!("{:?}", options.library_files),
                format!("{:?}", options.library_order),
                format!("{:?}", options.dpi_libraries),
                format!("{:?}", options.runtime_args),
                format!("{:?}", options.files),
            ]
        };
        let baseline = lists(&resolve_no_env(cli(&[]), Some(&cfg)).unwrap());
        let cases: [(usize, &[&str], &[&str]); 10] = [
            (0, &["-I", "cli_inc"], &["--append-include-dir", "cli_inc"]),
            (1, &["-D", "CLI_ONLY"], &["--append-define", "CLI_ONLY"]),
            (
                2,
                &["-G", "WIDTH=3"],
                &["--append-param-override", "WIDTH=3"],
            ),
            (
                3,
                &["--define-system-task", "$cli()"],
                &["--append-define-system-task", "$cli()"],
            ),
            (4, &["--libmap", "cli.map"], &["--append-libmap", "cli.map"]),
            (
                5,
                &["--libfile", "cl=c.sv"],
                &["--append-libfile", "cl=c.sv"],
            ),
            (6, &["-L", "a,b"], &["--append-library-order", "a,b"]),
            (7, &["--dpi-lib", "cli.so"], &["--append-dpi-lib", "cli.so"]),
            (8, &["--", "+cli"], &["--append-plusarg", "+cli"]),
            (9, &["x.sv"], &["--append-source", "x.sv"]),
        ];
        for (index, replace, append) in cases {
            for args in [replace, append] {
                let options = lists(&resolve_no_env(cli(args), Some(&cfg)).unwrap());
                let differing: Vec<usize> = (0..options.len())
                    .filter(|position| options[*position] != baseline[*position])
                    .collect();
                assert_eq!(differing, [index], "{args:?} changes only its own list");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn append_options_require_a_value() {
        for flag in [
            "--append-source",
            "--append-include-dir",
            "--append-define",
            "--append-param-override",
            "--append-define-system-task",
            "--append-libmap",
            "--append-libfile",
            "--append-library-order",
            "--append-dpi-lib",
            "--append-plusarg",
        ] {
            assert_eq!(parse_args(vec![flag.to_owned()]).unwrap_err(), 2, "{flag}");
            assert_eq!(
                parse_args(vec![flag.to_owned(), String::new()]).unwrap_err(),
                2,
                "{flag} with an empty value"
            );
        }
        for bad in ["NAME", "=1", "N="] {
            assert_eq!(
                parse_args(vec!["--append-param-override".to_owned(), bad.to_owned()]).unwrap_err(),
                2,
                "{bad}"
            );
        }
    }

    #[test]
    fn later_defines_and_parameter_overrides_replace_earlier_ones_by_name() {
        let dir = temp("dupes");
        let cfg = config(&dir, FULL);
        let options = resolve_no_env(
            cli(&[
                "-D", "X=1", "-D", "X=2", "-D", "NEW", "-D", "NEW=1", "-G", "DEPTH=2", "-G",
                "DEPTH=3",
            ]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.defines, ["X=2", "NEW=1"]);
        assert_eq!(options.param_overrides, ["DEPTH=3"]);
        let options = resolve_no_env(
            cli(&[
                "--append-define",
                "CFG_B=9",
                "--append-define",
                "NEW",
                "--append-define",
                "NEW=1",
                "--append-param-override",
                "DEPTH=2",
                "--append-param-override",
                "DEPTH=3",
            ]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            options.defines,
            ["CFG_A", "CFG_B=9", "NEW=1"],
            "an appended define replaces the config define of the same name"
        );
        assert_eq!(options.param_overrides, ["DEPTH=3"]);
        // Other lists keep the first occurrence of an identical entry.
        let options = resolve_no_env(
            cli(&[
                "--append-include-dir",
                "cli_inc",
                "--append-include-dir",
                "cli_inc",
                "--append-library-order",
                "lib,x,lib",
                "--append-libfile",
                "cl=c.sv",
                "--append-libfile",
                "cl=c.sv",
            ]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("inc")), "cli_inc".to_owned()]
        );
        assert_eq!(options.library_order, ["lib", "x"]);
        assert_eq!(
            options.library_files,
            [
                format!("lib={}", path_string(&dir.join("l.sv"))),
                "cl=c.sv".to_owned()
            ]
        );
        // Plusargs are not deduplicated.
        let options = resolve_no_env(cli(&["--", "+a", "+a"]), Some(&cfg)).unwrap();
        assert_eq!(options.runtime_args, ["+a", "+a"]);
        let options = resolve_no_env(
            cli(&["--append-plusarg", "+cfg", "--append-plusarg", "+cfg"]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.runtime_args, ["+cfg", "+cfg", "+cfg"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_source_appended_and_in_the_config_is_listed_once() {
        let dir = temp("same_source");
        let file = dir.join("a.sv");
        std::fs::write(&file, "").unwrap();
        let cfg = config(&dir, "schema_version = 1\n[sources]\nfiles = [\"a.sv\"]\n");
        let named = path_string(&file);
        let options = resolve_no_env(
            cli(&["--append-source", named.as_str(), "--append-source", "b.sv"]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.files, [named, "b.sv".to_owned()]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_plusarg_marker_replaces_the_config_plusargs_even_when_empty() {
        let dir = temp("plusargs");
        let cfg = config(&dir, FULL);
        let options = resolve_no_env(cli(&["x.sv", "--"]), Some(&cfg)).unwrap();
        assert!(options.runtime_args.is_empty());
        let options = resolve_no_env(cli(&["x.sv"]), Some(&cfg)).unwrap();
        assert_eq!(options.runtime_args, ["+cfg"], "no marker keeps the config");
        let options = resolve_no_env(cli(&["--append-plusarg", "+app", "--"]), Some(&cfg)).unwrap();
        assert_eq!(
            options.runtime_args,
            ["+app"],
            "empty replacement plus append"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn config_directories_supply_sources_and_include_dirs_until_replaced() {
        let dir = temp("discover");
        std::fs::create_dir_all(dir.join("rtl")).unwrap();
        std::fs::write(dir.join("rtl/b.sv"), "").unwrap();
        std::fs::write(dir.join("rtl/a.v"), "").unwrap();
        std::fs::write(dir.join("rtl/defs.svh"), "").unwrap();
        let text = "schema_version = 1\n[sources]\ndirectories = [\"rtl\"]\nfiles = [\"extra.sv\"]\n[compile]\ninclude_dirs = [\"inc\"]\n";
        let cfg = config(&dir, text);
        let discovered = [
            path_string(&dir.join("extra.sv")),
            path_string(&dir.join("rtl").join("a.v")),
            path_string(&dir.join("rtl").join("b.sv")),
        ];
        let options = resolve_no_env(cli(&[]), Some(&cfg)).unwrap();
        assert_eq!(options.files, discovered);
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("rtl")), path_string(&dir.join("inc"))]
        );
        // Appending sources and include directories keeps the discovery.
        let options = resolve_no_env(
            cli(&["--append-source", "only.sv", "--append-include-dir", "mine"]),
            Some(&cfg),
        )
        .unwrap();
        let mut appended_files = discovered.to_vec();
        appended_files.push("only.sv".to_owned());
        assert_eq!(options.files, appended_files);
        assert_eq!(
            options.include_dirs,
            [
                path_string(&dir.join("rtl")),
                path_string(&dir.join("inc")),
                "mine".to_owned()
            ]
        );
        // Named sources replace the discovery and the source directories as
        // include directories; `-I` replaces only the explicit include list.
        let options = resolve_no_env(cli(&["only.sv", "-I", "mine"]), Some(&cfg)).unwrap();
        assert_eq!(options.files, ["only.sv"]);
        assert_eq!(options.include_dirs, ["mine"]);
        let options = resolve_no_env(cli(&["only.sv"]), Some(&cfg)).unwrap();
        assert_eq!(options.include_dirs, [path_string(&dir.join("inc"))]);
        let options = resolve_no_env(cli(&["-I", "mine"]), Some(&cfg)).unwrap();
        assert_eq!(options.files, discovered);
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("rtl")), "mine".to_owned()]
        );
        // Replacement and appended sources together.
        let options =
            resolve_no_env(cli(&["only.sv", "--append-source", "more.sv"]), Some(&cfg)).unwrap();
        assert_eq!(options.files, ["only.sv", "more.sv"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lint_selection_follows_the_precedence_rules() {
        let dir = temp("lint");
        let on = config(&dir, "schema_version = 1\n[lint]\njson_file = \"r.json\"\n");
        let options = resolve_no_env(cli(&["x.sv"]), Some(&on)).unwrap();
        assert!(options.lint_only && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(dir.join("r.json")));
        let options = resolve_no_env(cli(&["--no-lint-only", "x.sv"]), Some(&on)).unwrap();
        assert!(!options.lint_only && !options.lint_json_mode);
        let options = resolve_no_env(cli(&["x.sv", "--lint-json"]), Some(&on)).unwrap();
        assert!(options.lint_json_mode);
        assert_eq!(
            options.lint_json_path, None,
            "the command line picks stdout"
        );
        let off = config(&dir, "schema_version = 1\n");
        let options =
            resolve_no_env(cli(&["--lint-json", "out.json", "x.sv"]), Some(&off)).unwrap();
        assert!(options.lint_only && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(PathBuf::from("out.json")));
        let options = resolve_no_env(cli(&["--lint-only", "x.sv"]), Some(&off)).unwrap();
        assert!(options.lint_only && !options.lint_json_mode);
        let options = resolve_no_env(cli(&["-Werror", "x.sv"]), Some(&off)).unwrap();
        assert!(options.warnings_as_errors && !options.lint_only);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn waveform_options_follow_the_precedence_rules() {
        let dir = temp("wave");
        let on = config(
            &dir,
            "schema_version = 1\n[waveform]\nfile = \"w/dump.vcd\"\n",
        );
        let options = resolve_no_env(cli(&["x.sv"]), Some(&on)).unwrap();
        let wave = options.wave.expect("configured waveform");
        assert_eq!(wave.file, path_string(&dir.join("w/dump.vcd")));
        assert_eq!(wave.depth, 0, "the default depth dumps every level");
        let options = resolve_no_env(cli(&["--no-wave", "x.sv"]), Some(&on)).unwrap();
        assert_eq!(options.wave, None);
        let options = resolve_no_env(cli(&["--wave-depth", "2", "x.sv"]), Some(&on)).unwrap();
        assert_eq!(options.wave.map(|wave| wave.depth), Some(2));
        let error = resolve_no_env(cli(&["--wave-depth", "2", "x.sv"]), None)
            .expect_err("a depth needs a file");
        assert!(error.0.contains("--wave-depth requires"), "{error:?}");
        for rejected in [
            &["--wave", "dump.txt", "x.sv"][..],
            &["--wave"][..],
            &["--wave", "d.vcd", "--wave-depth", "-1", "x.sv"][..],
        ] {
            assert_eq!(
                parse_args(rejected.iter().map(|arg| (*arg).to_owned()).collect()).err(),
                Some(2),
                "{rejected:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn top_accepts_a_module_name_only() {
        assert_eq!(cli(&["--top", "tb", "x.sv"]).top.as_deref(), Some("tb"));
        for rejected in ["work.tb", "cfg:config", "a b", ""] {
            assert_eq!(
                parse_args(vec![
                    "--top".to_owned(),
                    rejected.to_owned(),
                    "x.sv".to_owned()
                ])
                .err(),
                Some(2),
                "{rejected}"
            );
        }
    }

    #[test]
    fn removed_and_unknown_options_are_usage_errors() {
        for removed in [
            "--lint",
            "--no-lint",
            "--lint-config",
            "--max-export-mib",
            "-Wall",
            "--bogus",
        ] {
            assert_eq!(
                parse_args(vec![removed.to_owned(), "x.sv".to_owned()]).err(),
                Some(2),
                "{removed}"
            );
        }
    }

    #[test]
    fn config_lint_rules_are_the_base_of_the_lint_settings() {
        let dir = temp("rules");
        let cfg = config(
            &dir,
            "schema_version = 1\n[lint.rules.unused-signal]\nenabled = false\n",
        );
        let options = resolve_no_env(cli(&["x.sv"]), Some(&cfg)).unwrap();
        assert!(!options.lint_config.is_enabled("unused-signal"));
        assert!(options.lint_config.is_enabled("width-mismatch"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_config_is_read_without_an_explicit_path() {
        let dir = temp("no_discovery");
        std::fs::write(
            dir.join(config::CONFIG_FILE),
            "schema_version = 1\n[compile]\ntop = \"ignored\"\n",
        )
        .unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let loaded = load_config(None);
        std::env::set_current_dir(previous).unwrap();
        assert_eq!(loaded.unwrap().map(|config| config.compile.top), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn explicit_config_must_exist_and_resolves_relative_to_the_current_directory() {
        let dir = temp("explicit");
        let error = load_config_in(&dir, Path::new("missing.toml")).unwrap_err();
        assert!(error.0.contains("missing.toml") && error.0.contains("does not exist"));
        std::fs::write(
            dir.join("custom.toml"),
            "schema_version = 1\n[compile]\ntop = \"t\"\n",
        )
        .unwrap();
        let loaded = load_config_in(&dir, Path::new("custom.toml")).unwrap();
        assert_eq!(loaded.compile.top.as_deref(), Some("t"));
        std::fs::write(
            dir.join("bad.toml"),
            "schema_version = 1\n[compile]\nbogus = 1\n",
        )
        .unwrap();
        let error = load_config_in(&dir, Path::new("bad.toml")).unwrap_err();
        assert!(
            error.0.contains("bad.toml") && error.0.contains("compile.bogus"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    const ENV_CONFIG: &str = r#"schema_version = 1
[build]
generator = "CfgGen"
launcher = "cfglauncher"
cc = "cfgcc"
cmake = "cfgcmake"
cflags = "-DCFG"
jobs = 2
[output]
runtime_cache = "cfgcache"
"#;

    fn env_all() -> Env {
        Env {
            cc: Some("envcc".to_owned()),
            cflags: Some("-DENV".to_owned()),
            cmake: Some("envcmake".to_owned()),
            generator: Some("EnvGen".to_owned()),
            launcher: Some("envlauncher".to_owned()),
            build_jobs: Some(3),
            runtime_cache: Some(PathBuf::from("envcache")),
        }
    }

    /// The env-backed options of a run: cc, cflags, cmake, generator,
    /// launcher, jobs, runtime cache.
    fn env_backed(options: &DriverOptions) -> [String; 7] {
        [
            format!("{:?}", options.cc),
            format!("{:?}", options.cflags),
            format!("{:?}", options.cmake),
            format!("{:?}", options.generator),
            format!("{:?}", options.launcher),
            format!("{:?}", options.build_jobs),
            format!("{:?}", options.runtime_cache),
        ]
    }

    #[test]
    fn environment_ranks_between_the_command_line_and_the_config() {
        let dir = temp("env_order");
        let cfg = config(&dir, ENV_CONFIG);
        let env = env_all();

        // Default: neither environment nor config.
        let options = resolve(cli(&["x.sv"]), &Env::default(), None).unwrap();
        assert_eq!(
            env_backed(&options),
            ["None", "None", "None", "None", "None", "None", "None"].map(str::to_owned)
        );

        // Config only.
        let options = resolve(cli(&["x.sv"]), &Env::default(), Some(&cfg)).unwrap();
        assert_eq!(options.cc.as_deref(), Some("cfgcc"));
        assert_eq!(options.cflags.as_deref(), Some("-DCFG"));
        assert_eq!(options.cmake.as_deref(), Some("cfgcmake"));
        assert_eq!(options.generator.as_deref(), Some("CfgGen"));
        assert_eq!(options.launcher.as_deref(), Some("cfglauncher"));
        assert_eq!(options.build_jobs, Some(2));
        assert_eq!(options.runtime_cache, Some(dir.join("cfgcache")));

        // Environment beats config.
        let options = resolve(cli(&["x.sv"]), &env, Some(&cfg)).unwrap();
        assert_eq!(options.cc.as_deref(), Some("envcc"));
        assert_eq!(options.cflags.as_deref(), Some("-DENV"));
        assert_eq!(options.cmake.as_deref(), Some("envcmake"));
        assert_eq!(options.generator.as_deref(), Some("EnvGen"));
        assert_eq!(options.launcher.as_deref(), Some("envlauncher"));
        assert_eq!(options.build_jobs, Some(3));
        assert_eq!(options.runtime_cache, Some(PathBuf::from("envcache")));
        // ... and also applies without a config file.
        let options = resolve(cli(&["x.sv"]), &env, None).unwrap();
        assert_eq!(options.cc.as_deref(), Some("envcc"));

        // Command line beats environment and config.
        let options = resolve(
            cli(&[
                "--cc",
                "clicc",
                "--cflags",
                "",
                "--cmake",
                "clicmake",
                "--generator",
                "CliGen",
                "--launcher",
                "clilauncher",
                "--build-jobs",
                "4",
                "--runtime-cache",
                "clicache",
                "x.sv",
            ]),
            &env,
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.cc.as_deref(), Some("clicc"));
        assert_eq!(options.cflags.as_deref(), Some(""));
        assert_eq!(options.cmake.as_deref(), Some("clicmake"));
        assert_eq!(options.generator.as_deref(), Some("CliGen"));
        assert_eq!(options.launcher.as_deref(), Some("clilauncher"));
        assert_eq!(options.build_jobs, Some(4));
        assert_eq!(options.runtime_cache, Some(PathBuf::from("clicache")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn each_environment_option_individually_sits_between_cli_and_config() {
        let dir = temp("env_each");
        let cfg = config(&dir, ENV_CONFIG);
        let env = env_all();
        let with_config =
            env_backed(&resolve(cli(&["x.sv"]), &Env::default(), Some(&cfg)).unwrap());
        let with_env = env_backed(&resolve(cli(&["x.sv"]), &env, Some(&cfg)).unwrap());
        let options: [(&str, &str); 7] = [
            ("--cc", "clicc"),
            ("--cflags", "-DCLI"),
            ("--cmake", "clicmake"),
            ("--generator", "CliGen"),
            ("--launcher", "clilauncher"),
            ("--build-jobs", "9"),
            ("--runtime-cache", "clicache"),
        ];
        for (index, (flag, value)) in options.iter().enumerate() {
            let resolved =
                env_backed(&resolve(cli(&[flag, value, "x.sv"]), &env, Some(&cfg)).unwrap());
            for (position, entry) in resolved.iter().enumerate() {
                if position == index {
                    assert!(
                        entry.contains(value) || entry.contains('9'),
                        "{flag}: {entry}"
                    );
                } else {
                    assert_eq!(entry, &with_env[position], "{flag} leaves the others alone");
                }
            }
            assert_ne!(resolved[index], with_env[index]);
            assert_ne!(
                with_env[index], with_config[index],
                "env beats config for {flag}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn environment_parsing_follows_the_historic_fallbacks() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            Env::from_lookup(move |name| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            })
        };
        assert_eq!(vars(&[]), Env::default());
        let env = vars(&[("CC", "gcc")]);
        assert_eq!(env.cc.as_deref(), Some("gcc"));
        let env = vars(&[("CC", "gcc"), ("LLG_CC", "clang")]);
        assert_eq!(env.cc.as_deref(), Some("clang"), "$LLG_CC beats $CC");
        let env = vars(&[("LLG_CC", ""), ("CC", "gcc")]);
        assert_eq!(env.cc.as_deref(), Some("gcc"), "an empty $LLG_CC is unset");
        let env = vars(&[("LLG_CFLAGS", "")]);
        assert_eq!(env.cflags.as_deref(), Some(""), "empty flags are a value");
        let env = vars(&[
            ("LLG_CMAKE", ""),
            ("CMAKE_GENERATOR", ""),
            ("LLG_C_LAUNCHER", ""),
        ]);
        assert_eq!((env.cmake, env.generator, env.launcher), (None, None, None));
        let env = vars(&[("LLG_C_LAUNCHER", " ccache ")]);
        assert_eq!(env.launcher.as_deref(), Some("ccache"));
        for (text, jobs) in [("6", Some(6)), (" 4 ", Some(4)), ("0", None), ("x", None)] {
            let env = Env::from_lookup(|name| {
                (name == "CMAKE_BUILD_PARALLEL_LEVEL").then(|| text.to_owned())
            });
            assert_eq!(env.build_jobs, jobs, "{text:?}");
        }
        let env = vars(&[("LLG_RUNTIME_CACHE_DIR", "/rc")]);
        assert_eq!(env.runtime_cache, Some(PathBuf::from("/rc")));
    }

    #[test]
    fn non_environment_options_do_not_read_the_environment() {
        // The model optimization level and the stop policy have no
        // environment fallback: command line, config, default only.
        let dir = temp("no_env_options");
        let cfg = config(
            &dir,
            "schema_version = 1\n[build]\nmodel_opt_level = \"O1\"\n",
        );
        let options = resolve(cli(&["x.sv"]), &env_all(), Some(&cfg)).unwrap();
        assert_eq!(options.model_opt_level, sim::build::ModelOptLevel::O1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parameter_override_arguments_are_validated() {
        for bad in ["NAME", "=1", "1X=2", "A-B=1", "N="] {
            assert_eq!(
                parse_args(vec!["--param-override".to_owned(), bad.to_owned()]).unwrap_err(),
                2,
                "{bad}"
            );
        }
        assert!(parse_args(vec!["-G".to_owned(), "W=8".to_owned()]).is_ok());
    }
}
