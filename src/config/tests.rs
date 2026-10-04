//! Schema, resolution and loading tests.

use super::*;
use std::io;
use std::path::{Path, PathBuf};

/// `base` joined with a `/`-separated relative path, one component at a time,
/// so the result has the native separators the resolver produces.
fn native(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn write_and_load(root: &Path, text: &str) -> ConfigLoad {
    let dir = root.join("proj");
    std::fs::create_dir_all(&dir).expect("create config root");
    let path = dir.join(CONFIG_FILE);
    std::fs::write(&path, text).expect("write config");
    load_config_file(&dir.join(CONFIG_FILE)).expect("load config")
}

#[test]
fn valid_config_resolves_paths_from_config_dir() {
    let root = std::env::temp_dir().join(format!("llg_cfg_valid_{}", std::process::id()));
    let dir = root.join("proj");
    std::fs::create_dir_all(dir.join("rtl")).expect("create rtl");
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
    assert_eq!(
        config
            .compile
            .param_overrides
            .get("DEPTH")
            .map(String::as_str),
        Some("1024")
    );
    let search = include_dirs(&config);
    assert!(search.contains(&dir.join("rtl")));
    assert!(search.contains(&dir.join("inc")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_config_uses_safe_defaults() {
    let root = std::env::temp_dir().join(format!("llg_cfg_missing_{}", std::process::id()));
    let dir = root.join("proj");
    std::fs::create_dir_all(&dir).expect("create dir");
    let load = load_config_file(&dir.join(CONFIG_FILE)).expect("load missing config");
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
    let root = std::env::temp_dir().join(format!("llg_cfg_analysis_budget_{}", std::process::id()));
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
    assert!(zero.errors.iter().any(
        |error| error.message.contains("max_file_bytes") && error.message.contains("positive")
    ));

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
    let root = std::env::temp_dir().join(format!("llg_cfg_invalid_utf8_{}", std::process::id()));
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
/// the Slang argument boundary; such entries are dropped with a warning.
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
fn default_excludes_cover_repository_outputs() {
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
    // built-in output exclusions are never lost.
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
    let load = load_config_file(&dir.join(CONFIG_FILE)).expect("load config");
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
            // Normalized with the host separator (`vendor\inc` on Windows).
            && warning
                .message
                .contains(&Path::new("vendor").join("inc").display().to_string())));
    let _ = std::fs::remove_dir_all(root);
}

fn base() -> PathBuf {
    std::env::temp_dir().join("llg_cfg_pure")
}

fn parse(text: &str) -> Result<ParsedConfig, ConfigError> {
    parse_config_detailed(&base(), text)
}

fn parse_err(text: &str) -> String {
    parse(text).expect_err("config must be rejected").message
}

#[test]
fn driver_keys_resolve_relative_to_the_config_directory() {
    let parsed = parse(
        r#"schema_version = 1
[sources]
files = ["rtl/top.sv", "../shared/pkg.sv"]
[compile]
edition = "2001"
compilation_units = "merged"
system_tasks = ["$my_task(input int)"]
[libraries]
map_files = ["libs/lib.map"]
files = ["lib_a=libs/a.sv", "libs/b.sv"]
order = ["lib_a", "work"]
default = "work"
[lint]
run = true
json = true
json_file = "out/lint.json"
[simulator]
stop_policy = "exit"
max_export_mib = 64
optimize = false
plusargs = ["+seed=1"]
[build]
gen_only = true
generator = "Ninja"
launcher = "ccache"
cc = "gcc"
cflags = ""
model_opt_level = "O2"
cmake = "cmake"
jobs = 3
dpi_libs = ["dpi/libx.so"]
[output]
out_dir = "out"
runtime_cache = "../cache"
"#,
    )
    .expect("valid driver keys");
    assert!(parsed.warnings.is_empty());
    let config = parsed.config;
    let base = base();
    assert_eq!(
        config.sources.files,
        vec![
            native(&base, "rtl/top.sv"),
            native(base.parent().unwrap(), "shared/pkg.sv")
        ]
    );
    assert!(!config.sources.directories_configured);
    assert_eq!(config.compile.edition, Some(LanguageEdition::Verilog2001));
    assert_eq!(
        config.compile.compilation_units,
        Some(CompilationUnitMode::Merged)
    );
    assert_eq!(config.compile.system_tasks, vec!["$my_task(input int)"]);
    assert_eq!(
        config.libraries.map_files,
        vec![native(&base, "libs/lib.map")]
    );
    assert_eq!(
        config.libraries.files,
        vec![
            format!("lib_a={}", native(&base, "libs/a.sv").display()),
            native(&base, "libs/b.sv").to_string_lossy().into_owned()
        ],
        "only the path part of library=path is resolved"
    );
    assert_eq!(config.libraries.order, vec!["lib_a", "work"]);
    assert_eq!(config.libraries.default.as_deref(), Some("work"));
    assert_eq!(config.lint_run.run, Some(true));
    assert_eq!(config.lint_run.json, Some(true));
    assert_eq!(
        config.lint_run.json_file,
        Some(native(&base, "out/lint.json"))
    );
    assert_eq!(config.simulator.stop_policy, Some(StopPolicy::Exit));
    assert_eq!(config.simulator.max_export_mib, Some(64));
    assert_eq!(config.simulator.optimize, Some(false));
    assert_eq!(config.simulator.plusargs, Some(vec!["+seed=1".to_owned()]));
    assert_eq!(config.build.gen_only, Some(true));
    assert_eq!(config.build.cc.as_deref(), Some("gcc"));
    assert_eq!(
        config.build.cflags.as_deref(),
        Some(""),
        "empty flags are kept"
    );
    assert_eq!(config.build.model_opt_level, Some(ModelOptLevel::O2));
    assert_eq!(config.build.jobs, Some(3));
    assert_eq!(config.build.dpi_libs, vec![native(&base, "dpi/libx.so")]);
    assert_eq!(config.output.out_dir, Some(base.join("out")));
    assert_eq!(
        config.output.runtime_cache,
        Some(base.parent().unwrap().join("cache"))
    );
}

#[test]
fn unset_driver_keys_stay_none_so_precedence_can_fall_through() {
    let config = parse("schema_version = 1\n").unwrap().config;
    assert_eq!(config.compile.edition, None);
    assert_eq!(config.simulator, SimulatorConfig::default());
    assert_eq!(config.build, BuildConfig::default());
    assert_eq!(config.output, OutputConfig::default());
    assert_eq!(config.lint_run, LintRunConfig::default());
    assert!(!config.sources.directories_configured);
    assert_eq!(config.sources.directories, vec![base()]);
}

#[test]
fn explicit_source_directories_are_recorded() {
    let config = parse("schema_version = 1\n[sources]\ndirectories = [\"rtl\"]\n")
        .unwrap()
        .config;
    assert!(config.sources.directories_configured);
    assert_eq!(config.sources.directories, vec![base().join("rtl")]);
}

#[test]
fn invalid_driver_values_name_their_key() {
    for (text, key) in [
        ("[compile]\nedition = \"2005\"\n", "compile.edition"),
        (
            "[compile]\ncompilation_units = \"split\"\n",
            "compile.compilation_units",
        ),
        ("[compile]\ntop = \"\"\n", "compile.top"),
        ("[compile]\nsystem_tasks = [\"\"]\n", "compile.system_tasks"),
        ("[libraries]\nfiles = [\"=x.sv\"]\n", "libraries.files"),
        ("[libraries]\nfiles = [\"lib=\"]\n", "libraries.files"),
        ("[libraries]\norder = [\"a,b\"]\n", "libraries.order"),
        ("[libraries]\nmap_files = [\"\"]\n", "libraries.map_files"),
        (
            "[simulator]\nstop_policy = \"halt\"\n",
            "simulator.stop_policy",
        ),
        (
            "[simulator]\nmax_export_mib = 0\n",
            "simulator.max_export_mib",
        ),
        (
            "[simulator]\nmax_export_mib = 16385\n",
            "simulator.max_export_mib",
        ),
        (
            "[build]\nmodel_opt_level = \"O4\"\n",
            "build.model_opt_level",
        ),
        ("[build]\njobs = 0\n", "build.jobs"),
        ("[build]\ncc = \"\"\n", "build.cc"),
        ("[output]\nout_dir = \"\"\n", "output.out_dir"),
        ("[lint]\njson_file = \"\"\n", "lint.json_file"),
    ] {
        let message = parse_err(&format!("schema_version = 1\n{text}"));
        assert!(message.contains(key), "{key}: {message}");
    }
}

#[test]
fn shape_errors_report_line_and_dotted_key() {
    let message = parse_err("schema_version = 1\n[build]\njobs = \"many\"\n");
    assert!(message.contains("line 3"), "{message}");
    assert!(message.contains("`build.jobs`"), "{message}");
    let message = parse_err("schema_version = 1\n[compile]\ntopp = \"x\"\n");
    assert!(message.contains("`compile.topp`"), "{message}");
    assert!(message.contains("unknown field"), "{message}");
    let message = parse_err("schema_version = 1\n[bogus]\n");
    assert!(message.contains("`bogus`"), "{message}");
    let message = parse_err("schema_version = 1\nextra = true\n");
    assert!(message.contains("`extra`"), "{message}");
}

#[test]
fn every_model_opt_level_name_is_accepted() {
    for level in ["O0", "O1", "O2", "O3", "Os"] {
        let config = parse(&format!(
            "schema_version = 1\n[build]\nmodel_opt_level = \"{level}\"\n"
        ))
        .unwrap()
        .config;
        assert_eq!(
            config.build.model_opt_level,
            ModelOptLevel::parse(level).ok()
        );
    }
}

#[test]
fn missing_file_is_reported_not_an_error() {
    let path = std::env::temp_dir()
        .join(format!("llg_cfg_missing_{}", std::process::id()))
        .join(CONFIG_FILE);
    let load = load_config_file(&path).expect("missing is not an io error");
    assert!(load.missing);
    assert!(load.config.is_none() && load.errors.is_empty());
}

#[test]
fn discover_sources_walks_every_directory_with_filters() {
    let root = std::env::temp_dir().join(format!("llg_cfg_discover_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["rtl/gen", "tb"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    for file in [
        "rtl/a.sv",
        "rtl/b.v",
        "rtl/defs.svh",
        "rtl/gen/g.sv",
        "tb/t.sv",
        "tb/readme.md",
    ] {
        std::fs::write(root.join(file), "").unwrap();
    }
    let parsed = parse_config_detailed(
        &root,
        "schema_version = 1\n[sources]\ndirectories = [\"rtl\", \"tb\"]\nexclude = [\"gen/**\"]\n",
    )
    .unwrap();
    let units = discover_sources(&parsed.config).unwrap();
    assert_eq!(
        units,
        vec![
            native(&root, "rtl/a.sv"),
            native(&root, "rtl/b.v"),
            native(&root, "tb/t.sv")
        ],
        "sorted, headers and excluded files omitted"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn stop_policy_names_round_trip() {
    for (name, policy) in [("resume", StopPolicy::Resume), ("exit", StopPolicy::Exit)] {
        assert_eq!(StopPolicy::parse(name), Ok(policy));
        assert_eq!(policy.env_value(), name);
    }
    assert!(StopPolicy::parse("stop").is_err());
}
