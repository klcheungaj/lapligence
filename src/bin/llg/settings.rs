//! Effective driver settings: `llg.toml` discovery and the precedence between
//! the command line, the config file and the environment/built-in defaults.
//!
//! Precedence, highest first: command line, `llg.toml`, environment
//! fallbacks (`$LLG_CC`, `$LLG_CFLAGS`, ...), built-in defaults. The file
//! supplies defaults for command-line options, so everything the command line
//! can say the file can say, ranking above the environment exactly as the
//! option would. A scalar given on the command line replaces the file's value.
//! A repeatable option given on the command line (`-I`, `-D`, `--libmap`, ...)
//! replaces the whole list from the file; it never appends. Files named on the
//! command line replace `sources.files` and `sources.directories`.

use std::path::{Path, PathBuf};

use llg::config::{self, LlgConfig, StopPolicy};
use llg::core::compile;
use llg::ffi::slang::SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES;
use llg::sim;

use crate::cli::{Cli, MIB};

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

/// A config problem that stops the driver, with the message to print.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SettingsError(pub String);

/// Load the effective `llg.toml`: the explicit `--config` file (which must
/// exist) or `llg.toml` in the current directory when present. There is no
/// search through parent directories: the driver runs relative to the
/// directory it is started in. Returns `None` when no file applies. Warnings
/// (dropped entries, missing directories) are printed to stderr.
pub(crate) fn load_config(explicit: Option<&Path>) -> Result<Option<LlgConfig>, SettingsError> {
    let cwd = std::env::current_dir().map_err(|error| {
        SettingsError(format!("cannot determine the current directory: {error}"))
    })?;
    load_config_in(&cwd, explicit)
}

fn load_config_in(cwd: &Path, explicit: Option<&Path>) -> Result<Option<LlgConfig>, SettingsError> {
    let (path, is_explicit) = match explicit {
        Some(path) => (cwd.join(path), true),
        None => (cwd.join(config::CONFIG_FILE), false),
    };
    let load = config::load_config_file(&path).map_err(|error| {
        SettingsError(format!("cannot read config {}: {error}", path.display()))
    })?;
    if load.missing {
        return if is_explicit {
            Err(SettingsError(format!(
                "config file {} does not exist",
                path.display()
            )))
        } else {
            Ok(None)
        };
    }
    for warning in &load.warnings {
        eprintln!("llg: warning: {}: {}", load.path.display(), warning.message);
    }
    match load.config {
        Some(config) => Ok(Some(config)),
        None => Err(SettingsError(
            load.errors
                .iter()
                .map(|error| format!("invalid config {}: {}", load.path.display(), error.message))
                .collect::<Vec<_>>()
                .join("\n"),
        )),
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Apply the precedence rules. `config` is `None` when no file applies.
/// Source discovery (when the config names directories and the command line
/// names no files) touches the filesystem.
pub(crate) fn resolve(
    cli: Cli,
    config: Option<&LlgConfig>,
) -> Result<DriverOptions, SettingsError> {
    let empty = config::default_config(Path::new(if cfg!(windows) { "C:\\" } else { "/" }));
    let config = config.unwrap_or(&empty);

    // Sources: the command line wins as a whole; otherwise the file's explicit
    // files plus everything discovered under its explicit directories.
    let use_config_sources = cli.files.is_empty();
    let mut files = cli.files;
    let mut source_dirs: Vec<String> = Vec::new();
    if use_config_sources {
        files.extend(config.sources.files.iter().map(|path| path_string(path)));
        if config.sources.directories_configured {
            let discovered = config::discover_sources(config).map_err(|error| {
                SettingsError(format!(
                    "cannot discover sources from {}: {error}",
                    config::CONFIG_FILE
                ))
            })?;
            files.extend(discovered.iter().map(|path| path_string(path)));
            source_dirs = config
                .sources
                .directories
                .iter()
                .map(|path| path_string(path))
                .collect();
        }
        let mut seen = std::collections::BTreeSet::new();
        files.retain(|file| seen.insert(file.clone()));
    }

    // Source directories are include-search directories, as in the language
    // server; `-I` replaces only the explicit include list.
    let mut include_dirs = source_dirs;
    let listed: Vec<String> = if cli.include_dirs.is_empty() {
        config
            .compile
            .include_dirs
            .iter()
            .map(|path| path_string(path))
            .collect()
    } else {
        cli.include_dirs
    };
    for dir in listed {
        if !include_dirs.contains(&dir) {
            include_dirs.push(dir);
        }
    }

    let param_overrides = if cli.param_overrides.is_empty() {
        config
            .compile
            .param_overrides
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect()
    } else {
        cli.param_overrides
    };

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

    let list_or = |cli: Vec<String>, config: &[String]| {
        if cli.is_empty() {
            config.to_vec()
        } else {
            cli
        }
    };
    let paths_or = |cli: Vec<String>, config: &[PathBuf]| {
        if cli.is_empty() {
            config.iter().map(|path| path_string(path)).collect()
        } else {
            cli
        }
    };

    Ok(DriverOptions {
        top: cli.top.or_else(|| config.compile.top.clone()),
        edition: cli.edition.or(config.compile.edition).unwrap_or_default(),
        compilation_unit_mode: cli
            .compilation_unit_mode
            .or(config.compile.compilation_units)
            .unwrap_or_default(),
        include_dirs,
        defines: list_or(cli.defines, &config.compile.defines),
        param_overrides,
        system_subroutines: list_or(cli.system_subroutines, &config.compile.system_tasks),
        library_map_files: paths_or(cli.library_map_files, &config.libraries.map_files),
        library_files: list_or(cli.library_files, &config.libraries.files),
        library_order: list_or(cli.library_order, &config.libraries.order),
        default_library: cli
            .default_library
            .or_else(|| config.libraries.default.clone()),
        files,
        runtime_args: cli
            .runtime_args
            .or_else(|| config.simulator.plusargs.clone())
            .unwrap_or_default(),
        lint_mode,
        lint_json_mode,
        lint_json_path,
        lint_config_path: cli.lint_config_path,
        lint_config: config.lint.clone(),
        generator: cli.generator.or_else(|| config.build.generator.clone()),
        dpi_libraries: if cli.dpi_libraries.is_empty() {
            config.build.dpi_libs.clone()
        } else {
            cli.dpi_libraries
        },
        launcher: cli.launcher.or_else(|| config.build.launcher.clone()),
        cc: cli.cc.or_else(|| config.build.cc.clone()),
        cflags: cli.cflags.or_else(|| config.build.cflags.clone()),
        model_opt_level: cli
            .model_opt_level
            .or(config.build.model_opt_level)
            .unwrap_or_default(),
        cmake: cli.cmake.or_else(|| config.build.cmake.clone()),
        build_jobs: cli.build_jobs.or(config.build.jobs),
        out_dir: cli
            .out_dir
            .or_else(|| config.output.out_dir.clone())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT_DIR)),
        runtime_cache: cli
            .runtime_cache
            .or_else(|| config.output.runtime_cache.clone()),
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
        let options = resolve(cli(&["d.sv"]), None).unwrap();
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
        let options = resolve(cli(&[]), Some(&config(&dir, FULL))).unwrap();
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
        let options = resolve(
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
    fn repeatable_command_line_options_replace_the_whole_config_list() {
        let dir = temp("lists");
        let options = resolve(
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
    fn an_empty_plusarg_marker_clears_config_plusargs() {
        let dir = temp("plusargs");
        let options = resolve(cli(&["x.sv", "--"]), Some(&config(&dir, FULL))).unwrap();
        assert!(options.runtime_args.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn config_directories_supply_sources_and_include_dirs_unless_files_are_given() {
        let dir = temp("discover");
        std::fs::create_dir_all(dir.join("rtl")).unwrap();
        std::fs::write(dir.join("rtl/b.sv"), "").unwrap();
        std::fs::write(dir.join("rtl/a.v"), "").unwrap();
        std::fs::write(dir.join("rtl/defs.svh"), "").unwrap();
        let text = "schema_version = 1\n[sources]\ndirectories = [\"rtl\"]\nfiles = [\"extra.sv\"]\n[compile]\ninclude_dirs = [\"inc\"]\n";
        let cfg = config(&dir, text);
        let options = resolve(cli(&[]), Some(&cfg)).unwrap();
        assert_eq!(
            options.files,
            [
                path_string(&dir.join("extra.sv")),
                path_string(&dir.join("rtl/a.v")),
                path_string(&dir.join("rtl/b.sv")),
            ]
        );
        assert_eq!(
            options.include_dirs,
            [path_string(&dir.join("rtl")), path_string(&dir.join("inc"))]
        );
        let options = resolve(cli(&["only.sv", "-I", "mine"]), Some(&cfg)).unwrap();
        assert_eq!(options.files, ["only.sv"]);
        assert_eq!(
            options.include_dirs,
            ["mine"],
            "no source directories without config sources"
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
        let options = resolve(cli(&["x.sv"]), Some(&on)).unwrap();
        assert!(options.lint_mode && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(dir.join("r.json")));
        let options = resolve(cli(&["--no-lint", "x.sv"]), Some(&on)).unwrap();
        assert!(!options.lint_mode && !options.lint_json_mode);
        let options = resolve(cli(&["x.sv", "--lint-json"]), Some(&on)).unwrap();
        assert!(options.lint_json_mode);
        assert_eq!(
            options.lint_json_path, None,
            "the command line picks stdout"
        );
        let off = config(&dir, "schema_version = 1\n");
        let options = resolve(cli(&["--lint-json", "out.json", "x.sv"]), Some(&off)).unwrap();
        assert!(options.lint_mode && options.lint_json_mode);
        assert_eq!(options.lint_json_path, Some(PathBuf::from("out.json")));
        let options = resolve(cli(&["--lint", "x.sv"]), Some(&off)).unwrap();
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
        let options = resolve(cli(&["x.sv"]), Some(&cfg)).unwrap();
        assert!(!options.lint_config.is_enabled("unused-signal"));
        assert!(options.lint_config.is_enabled("width-mismatch"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn discovery_uses_only_the_current_directory() {
        let dir = temp("load");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(dir.join(config::CONFIG_FILE), "schema_version = 1\n").unwrap();
        assert!(load_config_in(&dir, None).unwrap().is_some());
        assert!(
            load_config_in(&nested, None).unwrap().is_none(),
            "no search through parent directories"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn explicit_config_must_exist_and_resolves_relative_to_the_current_directory() {
        let dir = temp("explicit");
        let error = load_config_in(&dir, Some(Path::new("missing.toml"))).unwrap_err();
        assert!(error.0.contains("missing.toml") && error.0.contains("does not exist"));
        std::fs::write(
            dir.join("custom.toml"),
            "schema_version = 1\n[compile]\ntop = \"t\"\n",
        )
        .unwrap();
        let loaded = load_config_in(&dir, Some(Path::new("custom.toml")))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.compile.top.as_deref(), Some("t"));
        std::fs::write(
            dir.join("bad.toml"),
            "schema_version = 1\n[compile]\nbogus = 1\n",
        )
        .unwrap();
        let error = load_config_in(&dir, Some(Path::new("bad.toml"))).unwrap_err();
        assert!(
            error.0.contains("bad.toml") && error.0.contains("compile.bogus"),
            "{}",
            error.0
        );
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
