//! Command-line parsing for the `llg` driver.
//!
//! The parser records only what the command line said (`None`/empty means
//! "not given"), so `settings` can apply the documented precedence: command
//! line, then `llg.toml`, then environment fallbacks and built-in defaults.

use std::path::PathBuf;

use llg::config::StopPolicy;
use llg::core::compile;
use llg::ffi::slang::{NATIVE_HARD_MAX_OUTPUT_BYTES, SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES};
use llg::sim;

pub(crate) const MIB: u64 = 1024 * 1024;

/// Usage printed when `llg` is run without arguments and without a
/// discovered `llg.toml`.
pub(crate) const USAGE: &str = "usage: llg [generate options] [build options] <file.sv>... [-- <plusargs>...]
generate: --config <file>  --top <module[:config]>  --edition <2001|2009>  --compilation-units <separate|merged>  --include-dir <path>  --define <NAME[=VALUE]>  --param-override <NAME=VALUE>  --define-system-task <prototype>  --libmap <file>  --libfile [<library>=]<file>  --library-order <library>[,<library>...]  --default-library <library>  --lint  --no-lint  --lint-json [<path>]  --lint-config <file>  --gen-only  --no-gen-only  --no-opt  --opt  --max-export-mib <MiB>
build:    --generator <backend>  --launcher <program>  --dpi-lib <path>...  --cc <program>  --cflags <flags>  --model-opt-level <O0|O1|O2|O3|Os>  --cmake <program>  --build-jobs <N>
output:   --out-dir <dir>  --runtime-cache <dir>
stop:     --stop-policy <resume|exit>  # `$stop` handling (default: resume)
config:   llg.toml in the current directory is read when present (see docs/config.md)";

/// The command line as given. Options absent from it stay `None`/empty.
#[derive(Debug, Default)]
pub(crate) struct Cli {
    pub config_path: Option<PathBuf>,
    pub top: Option<String>,
    pub edition: Option<compile::LanguageEdition>,
    pub compilation_unit_mode: Option<compile::CompilationUnitMode>,
    pub include_dirs: Vec<String>,
    pub defines: Vec<String>,
    pub param_overrides: Vec<String>,
    pub system_subroutines: Vec<String>,
    pub library_map_files: Vec<String>,
    pub library_files: Vec<String>,
    pub library_order: Vec<String>,
    pub default_library: Option<String>,
    pub files: Vec<String>,
    /// `Some` once `--` appeared, even with no arguments after it.
    pub runtime_args: Option<Vec<String>>,
    pub lint_mode: Option<bool>,
    pub lint_json_mode: Option<bool>,
    pub lint_json_path: Option<PathBuf>,
    pub lint_config_path: Option<PathBuf>,
    pub generator: Option<String>,
    pub dpi_libraries: Vec<PathBuf>,
    pub launcher: Option<String>,
    pub cc: Option<String>,
    pub cflags: Option<String>,
    pub model_opt_level: Option<sim::build::ModelOptLevel>,
    pub cmake: Option<String>,
    pub build_jobs: Option<usize>,
    pub out_dir: Option<PathBuf>,
    pub runtime_cache: Option<PathBuf>,
    pub gen_only: Option<bool>,
    pub optimize: Option<bool>,
    pub stop_policy: Option<StopPolicy>,
    pub max_export_bytes: Option<u64>,
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<Cli, i32> {
    let mut top: Option<String> = None;
    let mut edition: Option<compile::LanguageEdition> = None;
    let mut compilation_unit_mode: Option<compile::CompilationUnitMode> = None;
    let mut include_dirs: Vec<String> = Vec::new();
    let mut defines: Vec<String> = Vec::new();
    let mut system_subroutines: Vec<String> = Vec::new();
    let mut library_map_files: Vec<String> = Vec::new();
    let mut library_files: Vec<String> = Vec::new();
    let mut library_order: Vec<String> = Vec::new();
    let mut default_library: Option<String> = None;
    let mut param_overrides: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut runtime_args: Option<Vec<String>> = None;
    let mut config_path: Option<PathBuf> = None;
    let mut lint_mode: Option<bool> = None;
    let mut lint_json_mode: Option<bool> = None;
    let mut lint_json_path: Option<PathBuf> = None;
    let mut lint_config_path: Option<PathBuf> = None;
    let mut generator: Option<String> = None;
    let mut dpi_libraries: Vec<PathBuf> = Vec::new();
    let mut launcher: Option<String> = None;
    let mut cc: Option<String> = None;
    let mut cflags: Option<String> = None;
    let mut model_opt_level: Option<sim::build::ModelOptLevel> = None;
    let mut cmake: Option<String> = None;
    let mut build_jobs: Option<usize> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut runtime_cache: Option<PathBuf> = None;
    let mut gen_only: Option<bool> = None;
    let mut optimize: Option<bool> = None;
    let mut stop_policy: Option<StopPolicy> = None;
    let mut max_export_bytes: Option<u64> = None;
    let mut it = args.into_iter().peekable();
    while let Some(a) = it.next() {
        if a == "--" {
            runtime_args = Some(it.collect());
            break;
        }
        match a.as_str() {
            "--help" | "-h" => {
                println!(
                    "Lapligence Verilog/SystemVerilog simulator

Usage: llg [OPTIONS] [<file.sv>...] [-- <plusargs>...]

Options and sources may also come from llg.toml (docs/config.md): ./llg.toml is
read when present, or the file given with --config. Command-line values
override the file; a repeatable option given on the command line replaces the
whole list from the file. Files named on the command line replace the file's
sources.

Options:
  -h, --help                 Print help and exit
  -V, --version              Print the package version and exit
      --config <file>        Read this llg.toml instead of ./llg.toml
                              (an explicit file that is missing is an error)
      --top <module[:config]> Select the top module or configured design
      --edition <2001|2009> Select the language edition (default: 2009)
      --compilation-units <separate|merged>
                              Select compilation-unit grouping (default: separate)
  -I, --include-dir <path>   Add an include-search directory
  -D, --define <NAME[=VALUE]> Define a preprocessor macro
  -G, --param-override <NAME=VALUE>
                              Override a top-level parameter (repeatable)
      --define-system-task <prototype>
                              Define a VPI system task/function prototype
      --libmap <file>        Admit a library map file (repeatable)
      -v, --libfile <[library=]file>
                              Admit a source file into a named library (repeatable)
      -L, --library-order <library>[,<library>...]
                              Set the default configuration library search order
      --default-library <name>
                              Name the default source library (default: work)
      --lint                 Run lint before simulation
      --no-lint              Do not lint (overrides llg.toml)
      --lint-json [<path>]   Report lint as JSON and exit
      --lint-config <file>   Load lint configuration (replaces the llg.toml [lint] rules)
      --gen-only             Emit C model sources without building
      --no-gen-only          Build and run (overrides llg.toml gen_only)
      --no-opt               Disable simulator optimization passes
      --opt                  Enable simulator optimization passes (overrides llg.toml)
      --stop-policy <resume|exit>
                              Handle `$stop` by resuming (default) or exiting
      --max-export-mib <MiB> Frontend export budget for the elaborated design
                              (default: {export_default}, at most {export_ceiling})
      --                    Pass remaining arguments to the generated simulator
      --generator <backend>  Select the CMake generator
      --launcher <program>   Select the CMake C compiler launcher
      --dpi-lib <path>       Link one explicit DPI-C library (repeatable)
      --cc <program>         C compiler for the model (default: $LLG_CC, $CC, cc)
      --cflags <flags>       Extra C compiler flags (default: $LLG_CFLAGS)
                              Appended after the model optimization level
      --model-opt-level <O0|O1|O2|O3|Os>
                              Model/runtime C optimization (default: {model_opt_default})
                              MSVC: O0=/Od, O1/Os=/O1, O2/O3=/O2
      --cmake <program>      CMake program (default: $LLG_CMAKE, cmake)
      --build-jobs <N>       Parallel compile jobs (default: $CMAKE_BUILD_PARALLEL_LEVEL,
                              available CPUs)
      --out-dir <dir>        Output root; the model goes to <dir>/sim/<design>
                              (default: build)
      --runtime-cache <dir>  Runtime archive cache (default: $LLG_RUNTIME_CACHE_DIR,
                              <out-dir>/llg-runtime-cache)",
                    model_opt_default = sim::build::DEFAULT_MODEL_OPT_LEVEL
                        .gnu_flag()
                        .trim_start_matches('-'),
                    export_default = SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES / MIB,
                    export_ceiling = NATIVE_HARD_MAX_OUTPUT_BYTES / MIB,
                );
                return Err(0);
            }
            "--version" | "-V" => {
                println!("llg {}", env!("CARGO_PKG_VERSION"));
                return Err(0);
            }
            "--top" | "-top" => top = it.next(),
            "--edition" => match it.next() {
                Some(value) => match value.parse() {
                    Ok(value) => edition = Some(value),
                    Err(error) => {
                        eprintln!("llg: {error}");
                        return Err(2);
                    }
                },
                None => {
                    eprintln!("llg: --edition requires 2001 or 2009");
                    return Err(2);
                }
            },
            "--compilation-units" | "--compilation-unit-mode" => match it.next() {
                Some(value) => match value.parse() {
                    Ok(value) => compilation_unit_mode = Some(value),
                    Err(error) => {
                        eprintln!("llg: {error}");
                        return Err(2);
                    }
                },
                None => {
                    eprintln!("llg: --compilation-units requires separate or merged");
                    return Err(2);
                }
            },
            "--include-dir" | "-I" => match it.next() {
                Some(path) if !path.is_empty() => include_dirs.push(path),
                _ => {
                    eprintln!("llg: --include-dir requires a path");
                    return Err(2);
                }
            },
            "--define" | "-D" => match it.next() {
                Some(define) if !define.is_empty() => defines.push(define),
                _ => {
                    eprintln!("llg: --define requires NAME or NAME=VALUE");
                    return Err(2);
                }
            },
            "--define-system-task" => match it.next() {
                Some(prototype) if !prototype.is_empty() => system_subroutines.push(prototype),
                _ => {
                    eprintln!("llg: --define-system-task requires a prototype");
                    return Err(2);
                }
            },
            "--libmap" | "--library-map" => match it.next() {
                Some(path) if !path.is_empty() => library_map_files.push(path),
                _ => {
                    eprintln!("llg: --libmap requires a file path");
                    return Err(2);
                }
            },
            "--libfile" | "-v" => match it.next() {
                Some(path) if !path.is_empty() => library_files.push(path),
                _ => {
                    eprintln!("llg: --libfile requires [library=]file");
                    return Err(2);
                }
            },
            "--library-order" | "-L" => match it.next() {
                Some(value) if !value.is_empty() => {
                    let names = value
                        .split(',')
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    if names.is_empty() {
                        eprintln!("llg: --library-order requires a library name");
                        return Err(2);
                    }
                    library_order.extend(names);
                }
                _ => {
                    eprintln!("llg: --library-order requires a library name");
                    return Err(2);
                }
            },
            "--default-library" | "--defaultLibName" => match it.next() {
                Some(value) if !value.is_empty() => default_library = Some(value),
                _ => {
                    eprintln!("llg: --default-library requires a library name");
                    return Err(2);
                }
            },
            "--config" => match it.next() {
                Some(path) if !path.is_empty() => config_path = Some(PathBuf::from(path)),
                _ => {
                    eprintln!("llg: --config requires a file path");
                    return Err(2);
                }
            },
            "--param-override" | "-G" => match it.next() {
                Some(entry) if valid_param_override(&entry) => param_overrides.push(entry),
                _ => {
                    eprintln!("llg: --param-override requires NAME=VALUE with an identifier NAME");
                    return Err(2);
                }
            },
            "--generator" | "-generator" => match it.next() {
                Some(g) => generator = Some(g),
                None => {
                    eprintln!("llg: --generator requires a backend name");
                    return Err(2);
                }
            },
            "--dpi-lib" => match it.next() {
                Some(path) if !path.is_empty() => dpi_libraries.push(PathBuf::from(path)),
                _ => {
                    eprintln!("llg: --dpi-lib requires a library path");
                    return Err(2);
                }
            },
            "--launcher" => match it.next() {
                Some(value) if !value.is_empty() => launcher = Some(value),
                _ => {
                    eprintln!("llg: --launcher requires a program name");
                    return Err(2);
                }
            },
            "--cc" => match it.next() {
                Some(value) if !value.is_empty() => cc = Some(value),
                _ => {
                    eprintln!("llg: --cc requires a compiler program");
                    return Err(2);
                }
            },
            // Empty flags are meaningful: they clear an inherited $LLG_CFLAGS.
            "--cflags" => match it.next() {
                Some(value) => cflags = Some(value),
                None => {
                    eprintln!("llg: --cflags requires a flag string");
                    return Err(2);
                }
            },
            "--cmake" => match it.next() {
                Some(value) if !value.is_empty() => cmake = Some(value),
                _ => {
                    eprintln!("llg: --cmake requires a program");
                    return Err(2);
                }
            },
            "--build-jobs" => match it.next().map(|value| value.parse::<usize>()) {
                Some(Ok(value)) if value > 0 => build_jobs = Some(value),
                _ => {
                    eprintln!("llg: --build-jobs requires a positive integer");
                    return Err(2);
                }
            },
            "--model-opt-level" => match it.next().as_deref().map(sim::build::ModelOptLevel::parse)
            {
                Some(Ok(value)) => model_opt_level = Some(value),
                _ => {
                    eprintln!("llg: --model-opt-level requires O0, O1, O2, O3 or Os");
                    return Err(2);
                }
            },
            "--out-dir" => match it.next() {
                Some(value) if !value.is_empty() => out_dir = Some(PathBuf::from(value)),
                _ => {
                    eprintln!("llg: --out-dir requires a directory");
                    return Err(2);
                }
            },
            "--runtime-cache" => match it.next() {
                Some(value) if !value.is_empty() => runtime_cache = Some(PathBuf::from(value)),
                _ => {
                    eprintln!("llg: --runtime-cache requires a directory");
                    return Err(2);
                }
            },
            "--max-export-mib" => match it.next().map(|value| value.parse::<u64>()) {
                Some(Ok(mib)) if (1..=NATIVE_HARD_MAX_OUTPUT_BYTES / MIB).contains(&mib) => {
                    max_export_bytes = Some(mib * MIB);
                }
                _ => {
                    eprintln!(
                        "llg: --max-export-mib requires an integer from 1 to {}",
                        NATIVE_HARD_MAX_OUTPUT_BYTES / MIB
                    );
                    return Err(2);
                }
            },
            "--gen-only" | "-gen-only" => gen_only = Some(true),
            "--no-gen-only" => gen_only = Some(false),
            "--no-opt" => optimize = Some(false),
            "--opt" => optimize = Some(true),
            "--stop-policy" => match it.next() {
                Some(value) => match StopPolicy::parse(&value) {
                    Ok(policy) => stop_policy = Some(policy),
                    Err(error) => {
                        eprintln!("llg: --stop-policy {error}");
                        return Err(2);
                    }
                },
                None => {
                    eprintln!("llg: --stop-policy requires resume or exit");
                    return Err(2);
                }
            },
            "--lint" | "-lint" => lint_mode = Some(true),
            "--no-lint" => {
                lint_mode = Some(false);
                lint_json_mode = Some(false);
            }
            "--lint-json" | "-lint-json" => {
                lint_mode = Some(true);
                lint_json_mode = Some(true);
                // The optional output path is the next token when it does not
                // start with `-`; otherwise the JSON goes to stdout.
                if let Some(next) = it.peek() {
                    if !next.starts_with('-') {
                        lint_json_path = it.next().map(PathBuf::from);
                    }
                }
            }
            "--lint-config" => match it.next() {
                Some(p) => lint_config_path = Some(PathBuf::from(p)),
                None => {
                    eprintln!("llg: --lint-config requires a file path");
                    return Err(2);
                }
            },
            _ => files.push(a),
        }
    }
    Ok(Cli {
        config_path,
        top,
        edition,
        compilation_unit_mode,
        include_dirs,
        defines,
        param_overrides,
        system_subroutines,
        library_map_files,
        library_files,
        library_order,
        default_library,
        files,
        runtime_args,
        lint_mode,
        lint_json_mode,
        lint_json_path,
        lint_config_path,
        generator,
        dpi_libraries,
        launcher,
        cc,
        cflags,
        model_opt_level,
        cmake,
        build_jobs,
        out_dir,
        runtime_cache,
        gen_only,
        optimize,
        stop_policy,
        max_export_bytes,
    })
}

/// `NAME=VALUE` with a SystemVerilog identifier name and a nonempty value.
fn valid_param_override(entry: &str) -> bool {
    let Some((name, value)) = entry.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    let valid_name = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    valid_name && !value.is_empty()
}
