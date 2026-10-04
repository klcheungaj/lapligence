//! Effective driver settings: the explicit `llg.toml` and the precedence
//! between the command line, the environment, the config file and built-in
//! defaults.
//!
//! Precedence, highest first: command line, environment (`$LLG_CC`,
//! `$LLG_CFLAGS`, ...), `llg.toml`, built-in default. [`layered`] is the one
//! place that expresses this order; every option that has an environment
//! variable goes through it. `llg` reads a config file only when `--config`
//! names it; it never discovers `llg.toml` in the current directory.
//!
//! A scalar from a higher layer replaces the lower one. A repeatable option
//! appends: the config list comes first and the command-line values follow;
//! `--clear <list>` drops the config list before they apply. Only the config
//! supplies lists (no list option has an environment variable). Duplicates:
//! a later `NAME[=VALUE]` define or `NAME=VALUE` parameter override replaces an
//! earlier one for the same `NAME`; any other list keeps the first occurrence
//! of an identical entry, and source files are compared by canonical path.
//! Plusargs after `--` are never deduplicated.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use llg::config::{self, LlgConfig, StopPolicy};
use llg::core::compile;
use llg::ffi::slang::SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES;
use llg::sim;

use crate::cli::{Cli, ListKey, MIB};

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
    pub lint_mode: bool,
    pub lint_json_mode: bool,
    pub lint_json_path: Option<PathBuf>,
    /// Legacy `llg-lint.toml` named on the command line; replaces the rule
    /// settings in `lint_config`.
    pub lint_config_path: Option<PathBuf>,
    /// Rule settings from `llg.toml`, or the defaults.
    pub lint_config: llg::core::lint::LintConfig,
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
    pub max_export_bytes: u64,
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

/// The configured entries of a list unless `--clear` named it, followed by the
/// command-line entries.
fn appended<T>(config: impl IntoIterator<Item = T>, cli: Vec<T>, cleared: bool) -> Vec<T> {
    let mut merged: Vec<T> = if cleared {
        Vec::new()
    } else {
        config.into_iter().collect()
    };
    merged.extend(cli);
    merged
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
    let cleared = |key: ListKey| cli.clear.contains(&key);

    // Sources: the config's explicit files and everything discovered under its
    // explicit directories, then the command-line files.
    let mut config_files: Vec<String> = Vec::new();
    let mut source_dirs: Vec<String> = Vec::new();
    if !cleared(ListKey::Sources) {
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
    let files = dedup_first(appended(config_files, cli.files, false), |file| {
        source_identity(file)
    });

    // Source directories are include-search directories, as in the language
    // server; they follow `--clear sources`, not `--clear include-dirs`.
    let include_dirs = dedup_first(
        appended(
            source_dirs.into_iter().chain(
                config
                    .compile
                    .include_dirs
                    .iter()
                    .filter(|_| !cleared(ListKey::IncludeDirs))
                    .map(|path| path_string(path)),
            ),
            cli.include_dirs,
            false,
        ),
        String::clone,
    );

    let defines = dedup_last(
        appended(
            config.compile.defines.iter().cloned(),
            cli.defines,
            cleared(ListKey::Defines),
        ),
        |entry| entry_name(entry),
    );
    let param_overrides = dedup_last(
        appended(
            config
                .compile
                .param_overrides
                .iter()
                .map(|(name, value)| format!("{name}={value}")),
            cli.param_overrides,
            cleared(ListKey::ParamOverrides),
        ),
        |entry| entry_name(entry),
    );
    let system_subroutines = dedup_first(
        appended(
            config.compile.system_tasks.iter().cloned(),
            cli.system_subroutines,
            cleared(ListKey::SystemTasks),
        ),
        String::clone,
    );
    let library_map_files = dedup_first(
        appended(
            config
                .libraries
                .map_files
                .iter()
                .map(|path| path_string(path)),
            cli.library_map_files,
            cleared(ListKey::LibMaps),
        ),
        String::clone,
    );
    let library_files = dedup_first(
        appended(
            config.libraries.files.iter().cloned(),
            cli.library_files,
            cleared(ListKey::LibFiles),
        ),
        String::clone,
    );
    let library_order = dedup_first(
        appended(
            config.libraries.order.iter().cloned(),
            cli.library_order,
            cleared(ListKey::LibraryOrder),
        ),
        String::clone,
    );
    let dpi_libraries = dedup_first(
        appended(
            config.build.dpi_libs.iter().cloned(),
            cli.dpi_libraries,
            cleared(ListKey::DpiLibs),
        ),
        PathBuf::clone,
    );
    // Plusargs after `--` follow the configured ones; repeats are meaningful.
    let runtime_args = appended(
        config.simulator.plusargs.iter().flatten().cloned(),
        cli.runtime_args.unwrap_or_default(),
        cleared(ListKey::Plusargs),
    );

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
    // `--lint-json` implies lint mode.
    let lint_mode = lint_json_mode || cli.lint_mode.or(config.lint_run.run).unwrap_or(false);

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
        lint_mode,
        lint_json_mode,
        lint_json_path,
        lint_config_path: cli.lint_config_path,
        lint_config: config.lint.clone(),
        generator: layered(
            cli.generator,
            env.generator.clone(),
            config.build.generator.clone(),
        ),
        dpi_libraries,
        launcher: cli.launcher.or_else(|| config.build.launcher.clone()),
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
        max_export_bytes: cli
            .max_export_bytes
            .or(config.simulator.max_export_mib.map(|mib| mib * MIB))
            .unwrap_or(SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES),
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
edition = "2001"
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
max_export_mib = 7
optimize = false
plusargs = ["+cfg"]
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
        assert_eq!(options.max_export_bytes, SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES);
        assert!(!options.no_opt && !options.gen_only && !options.lint_mode);
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
        assert_eq!(options.max_export_bytes, 7 * MIB);
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
                "2009",
                "--compilation-units",
                "separate",
                "--default-library",
                "clilib",
                "--stop-policy",
                "resume",
                "--max-export-mib",
                "9",
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
        assert_eq!(options.max_export_bytes, 9 * MIB);
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
    fn repeatable_command_line_options_append_to_the_config_lists() {
        let dir = temp("lists");
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
            "command-line files follow sources.files"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_discards_only_the_named_config_list() {
        let dir = temp("clear");
        let cfg = config(&dir, FULL);
        let cases: [(&str, &[&str]); 10] = [
            ("include-dirs", &["-I", "cli_inc"]),
            ("defines", &["-D", "CLI_ONLY"]),
            ("param-overrides", &["-G", "WIDTH=3"]),
            ("system-tasks", &["--define-system-task", "$cli()"]),
            ("libmaps", &["--libmap", "cli.map"]),
            ("libfiles", &["--libfile", "cl=c.sv"]),
            ("library-order", &["-L", "a,b"]),
            ("dpi-libs", &["--dpi-lib", "cli.so"]),
            ("plusargs", &["--", "+cli"]),
            ("sources", &["x.sv"]),
        ];
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
        for (list, extra) in cases {
            let mut args = vec!["--clear", list];
            args.extend(extra);
            let options = lists(&resolve_no_env(cli(&args), Some(&cfg)).unwrap());
            let differing = options
                .iter()
                .zip(&baseline)
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(differing, 1, "--clear {list} changes only its own list");
        }
        let options =
            resolve_no_env(cli(&["--clear", "defines", "-D", "ONLY"]), Some(&cfg)).unwrap();
        assert_eq!(options.defines, ["ONLY"]);
        let options = resolve_no_env(cli(&["--clear", "defines"]), Some(&cfg)).unwrap();
        assert!(options.defines.is_empty(), "a clear alone empties the list");
        let options = resolve_no_env(cli(&["--clear", "plusargs"]), Some(&cfg)).unwrap();
        assert!(options.runtime_args.is_empty());
        let options = resolve_no_env(
            cli(&["-D", "LATE", "--clear", "defines,include-dirs"]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.defines, ["LATE"], "position does not matter");
        assert!(options.include_dirs.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_requires_a_known_list_name() {
        for args in [
            &["--clear"][..],
            &["--clear", "nope"],
            &["--clear", "defines,nope"],
        ] {
            let args = args.iter().map(|arg| (*arg).to_owned()).collect();
            assert_eq!(parse_args(args).unwrap_err(), 2);
        }
    }

    #[test]
    fn later_defines_and_parameter_overrides_replace_earlier_ones_by_name() {
        let dir = temp("dupes");
        let cfg = config(&dir, FULL);
        let options = resolve_no_env(
            cli(&[
                "-D", "CFG_B=9", "-D", "NEW", "-D", "NEW=1", "-G", "DEPTH=2", "-G", "DEPTH=3",
            ]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.defines, ["CFG_A", "CFG_B=9", "NEW=1"]);
        assert_eq!(options.param_overrides, ["DEPTH=3"]);
        // Other lists keep the first occurrence of an identical entry.
        let options = resolve_no_env(
            cli(&[
                "-I",
                "cli_inc",
                "-I",
                "cli_inc",
                "-L",
                "lib,x,lib",
                "--libfile",
                "cl=c.sv",
                "--libfile",
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
        let options = resolve_no_env(cli(&["--", "+cfg", "+cfg"]), Some(&cfg)).unwrap();
        assert_eq!(options.runtime_args, ["+cfg", "+cfg", "+cfg"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_source_named_on_the_command_line_and_in_the_config_is_listed_once() {
        let dir = temp("same_source");
        let file = dir.join("a.sv");
        std::fs::write(&file, "").unwrap();
        let cfg = config(&dir, "schema_version = 1\n[sources]\nfiles = [\"a.sv\"]\n");
        let named = path_string(&file);
        let options = resolve_no_env(cli(&[named.as_str(), "b.sv"]), Some(&cfg)).unwrap();
        assert_eq!(options.files, [named, "b.sv".to_owned()]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_plusarg_marker_alone_leaves_the_config_plusargs() {
        let dir = temp("plusargs");
        let options = resolve_no_env(cli(&["x.sv", "--"]), Some(&config(&dir, FULL))).unwrap();
        assert_eq!(options.runtime_args, ["+cfg"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn config_directories_supply_sources_and_include_dirs_and_files_append() {
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
        let options = resolve_no_env(cli(&["only.sv", "-I", "mine"]), Some(&cfg)).unwrap();
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
        let options = resolve_no_env(
            cli(&["--clear", "sources", "only.sv", "-I", "mine"]),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.files, ["only.sv"]);
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("inc")), "mine".to_owned()],
            "clearing sources also drops the source directories as include dirs"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lint_selection_follows_the_precedence_rules() {
        let dir = temp("lint");
        let on = config(
            &dir,
            "schema_version = 1\n[lint]\nrun = true\njson_file = \"r.json\"\n",
        );
        let options = resolve_no_env(cli(&["x.sv"]), Some(&on)).unwrap();
        assert!(options.lint_mode && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(dir.join("r.json")));
        let options = resolve_no_env(cli(&["--no-lint", "x.sv"]), Some(&on)).unwrap();
        assert!(!options.lint_mode && !options.lint_json_mode);
        let options = resolve_no_env(cli(&["x.sv", "--lint-json"]), Some(&on)).unwrap();
        assert!(options.lint_json_mode);
        assert_eq!(
            options.lint_json_path, None,
            "the command line picks stdout"
        );
        let off = config(&dir, "schema_version = 1\n");
        let options =
            resolve_no_env(cli(&["--lint-json", "out.json", "x.sv"]), Some(&off)).unwrap();
        assert!(options.lint_mode && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(PathBuf::from("out.json")));
        let options = resolve_no_env(cli(&["--lint", "x.sv"]), Some(&off)).unwrap();
        assert!(options.lint_mode && !options.lint_json_mode);
        let _ = std::fs::remove_dir_all(dir);
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
            build_jobs: Some(3),
            runtime_cache: Some(PathBuf::from("envcache")),
        }
    }

    /// The env-backed options of a run: cc, cflags, cmake, generator, jobs,
    /// runtime cache.
    fn env_backed(options: &DriverOptions) -> [String; 6] {
        [
            format!("{:?}", options.cc),
            format!("{:?}", options.cflags),
            format!("{:?}", options.cmake),
            format!("{:?}", options.generator),
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
            ["None", "None", "None", "None", "None", "None"].map(str::to_owned)
        );

        // Config only.
        let options = resolve(cli(&["x.sv"]), &Env::default(), Some(&cfg)).unwrap();
        assert_eq!(options.cc.as_deref(), Some("cfgcc"));
        assert_eq!(options.cflags.as_deref(), Some("-DCFG"));
        assert_eq!(options.cmake.as_deref(), Some("cfgcmake"));
        assert_eq!(options.generator.as_deref(), Some("CfgGen"));
        assert_eq!(options.build_jobs, Some(2));
        assert_eq!(options.runtime_cache, Some(dir.join("cfgcache")));

        // Environment beats config.
        let options = resolve(cli(&["x.sv"]), &env, Some(&cfg)).unwrap();
        assert_eq!(options.cc.as_deref(), Some("envcc"));
        assert_eq!(options.cflags.as_deref(), Some("-DENV"));
        assert_eq!(options.cmake.as_deref(), Some("envcmake"));
        assert_eq!(options.generator.as_deref(), Some("EnvGen"));
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
        let options: [(&str, &str); 6] = [
            ("--cc", "clicc"),
            ("--cflags", "-DCLI"),
            ("--cmake", "clicmake"),
            ("--generator", "CliGen"),
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
        let env = vars(&[("LLG_CMAKE", ""), ("CMAKE_GENERATOR", "")]);
        assert_eq!((env.cmake, env.generator), (None, None));
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
        // Launcher, model optimization level and the stop policy have no
        // environment fallback: command line, config, default only.
        let dir = temp("no_env_options");
        let cfg = config(
            &dir,
            "schema_version = 1\n[build]\nlauncher = \"cfglauncher\"\nmodel_opt_level = \"O1\"\n",
        );
        let options = resolve(cli(&["x.sv"]), &env_all(), Some(&cfg)).unwrap();
        assert_eq!(options.launcher.as_deref(), Some("cfglauncher"));
        assert_eq!(options.model_opt_level, sim::build::ModelOptLevel::O1);
        let options = resolve(
            cli(&["--launcher", "ccache", "x.sv"]),
            &env_all(),
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(options.launcher.as_deref(), Some("ccache"));
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
