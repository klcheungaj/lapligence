//! llg — Lapligence Verilog/SystemVerilog → C11 simulator driver.
//!
//! Usage:
//!
//! ```text
//! llg [generate options] [build options] [<file.sv>...] [-- <plusargs>...]
//! generate: --config <file>  --top <module[:config]>  --edition <2001|2009>  --compilation-units <separate|merged>  --include-dir <path>  --define <NAME[=VALUE]>  --param-override <NAME=VALUE>  --define-system-task <prototype>  --libmap <file>  --libfile [<library>=]<file>  --library-order <library>[,<library>...]  --default-library <library>  --lint  --no-lint  --lint-json [<path>]  --lint-config <file>  --gen-only  --no-gen-only  --no-opt  --opt  --stop-policy <resume|exit>  --max-export-mib <MiB>
//! build:    --generator <backend>  --launcher <program>  --dpi-lib <path>...  --cc <program>  --cflags <flags>  --model-opt-level <O0|O1|O2|O3|Os>  --cmake <program>  --build-jobs <N>
//! output:   --out-dir <dir>  --runtime-cache <dir>
//! ```
//!
//! Configuration: `llg.toml` in the current directory (or the file named by
//! `--config`, which must exist) supplies defaults for the options above; see
//! `docs/config.md` and `settings.rs` for the key list and the precedence
//! (command line > config file > environment > built-in default; a repeatable
//! option on the command line replaces the file's whole list; files named on
//! the command line replace the file's sources). With no arguments and no
//! `llg.toml` the driver prints usage and exits 2.
//!
//! `--lint` runs the shared linter (`core::lint`) over the compiled design
//! after elaboration and before codegen: each finding prints to stderr as
//! `file:line:col: [SEVERITY] rule: message`, and any lint error aborts with
//! exit code 1 before codegen.  `--lint-config <path>` reads a `llg-lint.toml`
//! file that enables/disables rules and overrides severities for the lint pass
//! (missing or malformed files abort with exit code 1).  Without `--lint` the
//! driver behaves exactly as before.
//!
//! `--lint-json` implies lint mode but is a report-only mode: it emits one
//! machine-readable JSON object (see `core::lint::diags_to_json`) instead of
//! the human-readable lines, then exits without running codegen or the
//! simulation.  The JSON goes to stdout, or to the file given as
//! `--lint-json <path>` (the token after the flag is the output path when it
//! does not start with `-`).  The human-readable lint lines are suppressed;
//! Frontend diagnostics remain on stderr. When both `--lint` and `--lint-json`
//! are given, `--lint-json` wins.  Exit codes: 0 clean, 1 on lint errors, 2
//! usage errors.
//!
//! `--max-export-mib <MiB>` sets the frontend export budget: the bytes of
//! semantic records Slang capture may export for the whole elaborated design
//! (default `SIMULATOR_DEFAULT_MAX_OUTPUT_BYTES`, 4096 MiB; at most the native
//! ceiling of 16384 MiB). The export grows linearly with the design, about
//! 15 KiB per small `always` process; it is a finite guard because the driver
//! has no other memory limit unless `LLG_MEMORY_LIMIT_MB` is set. A design that
//! exhausts it fails with an error naming the limit and this option.
//!
//! Model build (CMake is the only supported model builder):
//!
//! - After C emission the driver writes the model plus stackless runtime sources
//!   into `<out-dir>/sim/<design>` (`--out-dir`, default `build`) and
//!   automatically configures + builds them with CMake
//!   (`sim::build::build_model_cmake_with_opts`). Each tool option wins over
//!   its environment fallback: `--cmake` > `$LLG_CMAKE` > `cmake`;
//!   `--cc` > `$LLG_CC` > `$CC` > `cc`; `--cflags` > `$LLG_CFLAGS`.
//! - `--model-opt-level <O0|O1|O2|O3|Os>` selects model and runtime C
//!   optimization. Extra flags follow it and can override it. Release adds
//!   only NDEBUG. Source-only projects retain the selected level.
//! - The runtime archive cache is `--runtime-cache` >
//!   `$LLG_RUNTIME_CACHE_DIR` > `<out-dir>/llg-runtime-cache`.
//! - `--build-jobs <N>` sets the `cmake --build --parallel` job count for the
//!   runtime archive and the model: `--build-jobs` >
//!   `$CMAKE_BUILD_PARALLEL_LEVEL` (positive integer) > available parallelism.
//! - `--generator <backend>` selects cmake's generator backend (`-G`,
//!   e.g. `Ninja`, `"Unix Makefiles"`); it overrides `$CMAKE_GENERATOR`.
//! - `--launcher <program>` selects `CMAKE_C_COMPILER_LAUNCHER` (for example,
//!   `ccache` or `sccache`): `--launcher` > `$LLG_C_LAUNCHER` > none.
//!   Tool invocation options are ignored with a warning under `--gen-only`.
//! - `--gen-only` stops after emitting the model + runtime +
//!   `CMakeLists.txt` into `<out-dir>/sim/<design>` (prints the directory,
//!   exits 0) without configuring/building/running.
//!
//! Flow: compile + elaborate with Slang (via `core::compile`), lower the
//! owned semantic database to C11 (`sim::codegen::generate`), write the model plus the
//! runtime into `<out-dir>/sim/<design>`, build through CMake
//! (unless `--gen-only`), and run the resulting simulator (stdout inherits;
//! the exit code is the simulator's).

use std::process::Command;

use llg::core::compile;
use llg::ffi::slang::NATIVE_HARD_MAX_OUTPUT_BYTES;
use llg::sim;

mod cli;
mod settings;

use cli::MIB;
use settings::{DriverOptions, SettingsError};

fn main() -> std::process::ExitCode {
    let code = match start(std::env::args().skip(1).collect()) {
        Ok(options) => {
            let memory_report = llg::memory_limit::install();
            let _memory_guard = memory_report.guard;
            run(options)
        }
        Err(code) => code,
    };
    std::process::ExitCode::from(code as u8)
}

/// Parse the command line, apply `llg.toml` and return the effective options,
/// or the exit code when the driver should stop (help, usage or config error).
fn start(args: Vec<String>) -> Result<DriverOptions, i32> {
    let no_arguments = args.is_empty();
    let cli = cli::parse_args(args)?;
    let config = settings::load_config(cli.config_path.as_deref()).map_err(config_failure)?;
    if no_arguments && config.is_none() {
        eprintln!("{}", cli::USAGE);
        return Err(2);
    }
    let options = settings::resolve(cli, config.as_ref()).map_err(config_failure)?;
    if options.files.is_empty() {
        eprintln!(
            "llg: no source files given (name them on the command line or in llg.toml \
             `sources.files`/`sources.directories`)"
        );
        return Err(2);
    }
    Ok(options)
}

fn config_failure(error: SettingsError) -> i32 {
    for line in error.0.lines() {
        eprintln!("llg: {line}");
    }
    1
}

/// Explain how to raise an exhausted frontend export budget. The native
/// bridge names the exhausted budget; record-count ceilings are fixed, so only
/// the byte budget is adjustable from the command line.
fn export_limit_hint(error: &compile::StartupError, max_export_bytes: u64) -> Option<String> {
    if error.kind() != compile::StartupErrorKind::LimitExceeded {
        return None;
    }
    let ceiling = NATIVE_HARD_MAX_OUTPUT_BYTES / MIB;
    let current = max_export_bytes / MIB;
    if error.contains("export byte limit") {
        Some(if current < ceiling {
            format!(
                "the elaborated design exceeds the {current} MiB frontend export budget; \
                 raise it with --max-export-mib <MiB> (at most {ceiling})"
            )
        } else {
            format!(
                "the elaborated design exceeds the native {ceiling} MiB frontend export ceiling"
            )
        })
    } else if [
        "semantic node limit",
        "semantic edge limit",
        "constant limit",
    ]
    .iter()
    .any(|limit| error.contains(limit))
    {
        Some(
            "the elaborated design exceeds a native frontend record-count ceiling, \
             which --max-export-mib cannot raise"
                .to_owned(),
        )
    } else {
        None
    }
}

fn run(options: DriverOptions) -> i32 {
    let DriverOptions {
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
        lint_config: config_lint,
        generator,
        dpi_libraries,
        launcher,
        cc,
        cflags,
        model_opt_level,
        cmake,
        build_jobs,
        out_dir: out_root,
        runtime_cache,
        gen_only,
        no_opt,
        stop_policy,
        max_export_bytes,
        cli_build_options,
    } = options;
    // 0. Lint rule settings: the `llg.toml` `[lint]` rules, replaced entirely
    //    by an explicit `--lint-config` file. Read + parse before compiling so
    //    a missing or malformed file aborts fast and with a clear message.
    let mut lint_config = config_lint;
    if let Some(path) = &lint_config_path {
        lint_config = llg::core::lint::LintConfig::new();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("llg: cannot read lint config {}: {e}", path.display());
                return 1;
            }
        };
        if let Err(errs) = lint_config.parse_toml(&text) {
            for e in &errs {
                eprintln!("llg: lint config: {e}");
            }
            eprintln!("llg: aborting due to lint config errors");
            return 1;
        }
    }

    let _generation_stage = llg::profile::Stage::new("generation");
    let frontend_stage = llg::profile::Stage::new("frontend");
    // 1. Slang parse, compile and elaborate into an owned snapshot.
    let out = match compile::compile_checked(&compile::CompileOpts {
        files,
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
        limits: llg::ffi::slang::Limits::simulator(max_export_bytes),
        ..Default::default()
    }) {
        Ok(out) => out,
        Err(compile::CompileError::Startup(e)) => {
            eprintln!("llg: compile failed to start: {e}");
            if let Some(hint) = export_limit_hint(&e, max_export_bytes) {
                eprintln!("llg: {hint}");
            }
            return 1;
        }
        Err(compile::CompileError::FrontendDiagnostics(diagnostics)) => {
            for d in &diagnostics {
                eprintln!(
                    "{:?}: {}:{}:{} {}",
                    d.severity,
                    d.file.as_deref().unwrap_or(""),
                    d.line,
                    d.col,
                    llg::core::diagnostics::user_message(d)
                );
            }
            eprintln!("llg: Slang reported errors; aborting");
            return 1;
        }
    };
    drop(frontend_stage);
    for d in &out.diagnostics {
        eprintln!(
            "{:?}: {}:{}:{} {}",
            d.severity,
            d.file.as_deref().unwrap_or(""),
            d.line,
            d.col,
            llg::core::diagnostics::user_message(d)
        );
    }

    // 2. Lint gate (--lint mode): build the owned db + model, print findings,
    //    and abort on lint errors before codegen.
    let db_stage = llg::profile::Stage::new("db.import");
    let codegen_db = match llg::core::db::Db::from_slang(&out.snapshot) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("llg: db build failed: {e}");
            return 1;
        }
    };
    drop(db_stage);
    drop(out);
    if lint_mode {
        let model = llg::core::model::DesignModel::from_db(&codegen_db);
        let findings = llg::core::lint::lint_with_config(&codegen_db, &model, &lint_config);

        if lint_json_mode {
            // Machine-readable report mode: one JSON object on stdout (or in
            // a file), no human-readable lint lines, and no codegen/simulation
            // afterwards — the report is the entire stdout output.
            let json = llg::core::lint::diags_to_json(&findings);
            match &lint_json_path {
                Some(path) => {
                    if let Err(e) = std::fs::write(path, format!("{json}\n")) {
                        eprintln!("llg: cannot write lint JSON {}: {e}", path.display());
                        return 1;
                    }
                }
                None => println!("{json}"),
            }
            let errors = findings
                .iter()
                .filter(|d| d.severity == llg::core::lint::LintSeverity::Error)
                .count();
            if errors > 0 {
                return 1;
            }
            return 0;
        } else {
            let mut errors = 0usize;
            let mut warnings = 0usize;
            for d in &findings {
                let sev = match d.severity {
                    llg::core::lint::LintSeverity::Error => {
                        errors += 1;
                        "ERROR"
                    }
                    llg::core::lint::LintSeverity::Warning => {
                        warnings += 1;
                        "WARNING"
                    }
                    llg::core::lint::LintSeverity::Info => "INFO",
                };
                let mut loc = String::new();
                if let Some(f) = &d.file {
                    loc.push_str(f);
                    if d.line > 0 {
                        loc.push_str(&format!(":{}", d.line));
                        if d.col > 0 {
                            loc.push_str(&format!(":{}", d.col));
                        }
                    }
                }
                if !loc.is_empty() {
                    loc.push_str(": ");
                }
                eprintln!("{loc}[{sev}] {}: {}", d.rule, d.message);
            }
            if findings.is_empty() {
                eprintln!("lint: clean");
            } else {
                eprintln!("lint: {errors} error(s), {warnings} warning(s)");
            }
            if errors > 0 {
                return 1;
            }
        }
    }

    // 3. Reuse the validated owned semantic database.
    let optimization = if no_opt {
        sim::opt::OptConfig::none()
    } else {
        sim::opt::OptConfig::default()
    };
    let value_config = match sim::value_backend::ValueConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("llg: {error}");
            return 1;
        }
    };
    let generated = sim::codegen::generate_from_owned_db_with_codegen_options(
        codegen_db,
        &sim::codegen::CodegenOptions {
            optimization,
            value_config,
            ..Default::default()
        },
    );
    let gen = match generated {
        Ok(g) => g,
        Err(e) => {
            eprintln!("llg: codegen error: {e}");
            return 1;
        }
    };
    for w in &gen.warnings {
        eprintln!("llg: warning: {w}");
    }

    // 4. Write sources (+ CMakeLists.txt).  With --gen-only, stop here: the
    //    emitted directory is the output, nothing is configured or run.
    let out_dir = out_root.join("sim").join(gen_name(&gen));
    let model = gen.sources();
    if gen_only {
        if cli_build_options {
            eprintln!("llg: warning: build options ignored with --gen-only");
        }
        let opts = sim::build::CmakeBuildOpts {
            dpi_libraries,
            model_opt_level,
            value_config,
            gmp_root: None,
            ..Default::default()
        };
        if let Err(e) = sim::build::generate_model_sources_with_opts(&out_dir, &model, &opts) {
            eprintln!("llg: {e}");
            return 1;
        }
        println!("{}", out_dir.display());
        return 0;
    }

    // 5. Build the model with CMake (the only supported builder).
    // The cache follows --out-dir unless the flag or environment moves it.
    let runtime_cache_dir = runtime_cache
        .or_else(sim::build::runtime_cache_dir_from_env)
        .unwrap_or_else(|| out_root.join("llg-runtime-cache"));
    let opts = sim::build::CmakeBuildOpts {
        generator,
        dpi_libraries,
        launcher,
        runtime_cache_dir: Some(runtime_cache_dir),
        cc,
        cflags,
        model_opt_level,
        cmake,
        build_jobs,
        value_config,
        gmp_root: None,
    };
    let exe = match sim::build::build_model_cmake_with_opts(&out_dir, &model, &opts) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("llg: {e}");
            return 1;
        }
    };

    // 6. Run the simulator; propagate its exit code. The generated model
    // reads this explicit policy without inheriting an ambient setting from
    // the driver's parent process.
    let status = match Command::new(&exe)
        .args(&runtime_args)
        .env("LLG_STOP_POLICY", stop_policy.env_value())
        .status()
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("llg: failed to run {}: {e}", exe.display());
            return 1;
        }
    };
    model_exit_code(status.code())
}

/// The driver status for a model run. A model that exits normally with a
/// status in the portable 0-255 range reports it unchanged. Termination by a
/// signal (`None` on Unix) or any status outside that range becomes 1: a
/// Windows crash status such as 0xC0000409, which `abort()` produces through
/// the C runtime's fast-fail, would otherwise be truncated to an unrelated
/// low byte (9) by `ExitCode`.
fn model_exit_code(code: Option<i32>) -> i32 {
    code.filter(|code| u8::try_from(*code).is_ok()).unwrap_or(1)
}

/// Directory name for the generated model (the design name, sanitized).
fn gen_name(gen: &sim::codegen::GeneratedModel) -> String {
    // GeneratedModel.design_name carries the same string the model header
    // comment embeds; sanitize identically so directory names stay
    // byte-compatible with previous releases.
    gen.design_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;

    #[test]
    fn model_exit_codes_keep_byte_statuses_and_map_crashes_to_one() {
        assert_eq!(model_exit_code(Some(0)), 0);
        assert_eq!(model_exit_code(Some(1)), 1);
        assert_eq!(model_exit_code(Some(255)), 255);
        assert_eq!(model_exit_code(None), 1);
        assert_eq!(model_exit_code(Some(-1)), 1);
        assert_eq!(model_exit_code(Some(256)), 1);
        // STATUS_STACK_BUFFER_OVERRUN from abort() on Windows.
        assert_eq!(model_exit_code(Some(0xC000_0409_u32 as i32)), 1);
    }

    #[test]
    fn export_budget_defaults_to_the_simulator_policy() {
        let options = parse_args(vec!["design.sv".to_owned()]).unwrap();
        assert_eq!(options.max_export_bytes, None);
        let options = parse_args(vec![
            "--max-export-mib".to_owned(),
            "1".to_owned(),
            "design.sv".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.max_export_bytes, Some(MIB));
    }

    #[test]
    fn export_failure_hint_names_the_option_and_native_ceiling() {
        let error = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "tb.sv",
                "module tb; endmodule",
            )],
            &compile::CompileOpts {
                limits: llg::ffi::slang::Limits::simulator(1),
                ..Default::default()
            },
        )
        .expect_err("the export cannot fit one byte");
        let compile::CompileError::Startup(error) = error else {
            panic!("expected startup limit failure");
        };
        let hint = export_limit_hint(&error, MIB).expect("adjustable export hint");
        assert!(hint.contains("1 MiB frontend export budget"));
        assert!(hint.contains("--max-export-mib <MiB> (at most 16384)"));
        let hint = export_limit_hint(&error, NATIVE_HARD_MAX_OUTPUT_BYTES)
            .expect("native export ceiling hint");
        assert!(hint.contains("native 16384 MiB frontend export ceiling"));
    }
}
