//! llg — Lapligence Verilog/SystemVerilog → C11 simulator driver.
//!
//! Usage:
//!
//! ```text
//! llg [generate options] [lint options] [wave options] [build options] [<file.sv>...] [-- <plusargs>...]
//! generate: --config <file>  --top <module>  --edition <v2001|sv2009>  --compilation-units <separate|merged>  --include-dir <path>  --define <NAME[=VALUE]>  --param-override <NAME=VALUE>  --define-system-task <prototype>  --libmap <file>  --libfile [<library>=]<file>  --library-order <library>[,<library>...]  --default-library <library>  --gen-only  --no-gen-only  --no-opt  --opt  --stop-policy <resume|exit>
//! lint:     --lint-only  --no-lint-only  --lint-json [<path>]  -Werror  -Wno-error
//! wave:     --wave <file.vcd|file.fst>  --wave-depth <N>  --no-wave
//! build:    --generator <backend>  --launcher <program>  --dpi-lib <path>...  --cc <program>  --cflags <flags>  --model-opt-level <O0|O1|O2|O3|Os>  --cmake <program>  --build-jobs <N>
//! output:   --out-dir <dir>  --runtime-cache <dir>
//! append:   --append-<list> <value>  (source, include-dir, define, param-override, define-system-task, libmap, libfile, library-order, dpi-lib, plusarg)
//! ```
//!
//! Configuration: the file named by `--config` (which must exist) supplies
//! defaults for the options above and the lint rule settings (`[lint]`);
//! `llg` never discovers `llg.toml` on its own. See `docs/config.md` and
//! `settings.rs` for the key list and the precedence (command line >
//! environment > config file > built-in default). A list option on the command
//! line (`-I`, `-D`, source files, `--`, ...) replaces the file's list; its
//! `--append-<list>` twin adds to the list instead, after any replacing values.
//! With no arguments the driver prints usage and exits 2.
//!
//! Every run lints the elaborated design (`core::lint`) before codegen: each
//! finding prints to stderr as `file:line:col: [SEVERITY] rule: message`
//! followed by a count line. Lint errors exit 1 before codegen; warnings do
//! not stop the run unless `-Werror` reports them as errors. `--lint-only`
//! stops after lint (exit 0 clean or warnings only, 1 on errors).
//!
//! `--lint-json` is a report-only mode (it implies `--lint-only`): it emits one
//! machine-readable JSON object (see `core::lint::diags_to_json`) instead of
//! the human-readable lines. The JSON goes to stdout, or to the file given as
//! `--lint-json <path>` (the token after the flag is the output path when it
//! does not start with `-`). Frontend diagnostics remain on stderr. Exit
//! codes: 0 clean, 1 on lint errors, 2 usage errors.
//!
//! `--wave <file>` makes the model dump every signal (or `--wave-depth N`
//! levels below each top) into a `.vcd` or `.fst` file from time 0, without
//! `$dumpfile`/`$dumpvars` in the design; the file replaces any `$dumpfile`
//! name and the design's `$dumpvars` selections are ignored.
//!
//! The frontend export is not budgeted: a simulator compile may use all
//! available memory (`LLG_MEMORY_LIMIT_MB` remains an optional process guard).
//!
//! Model build (CMake is the only supported model builder):
//!
//! - After C emission the driver writes the model plus stackless runtime sources
//!   into `<out-dir>/sim/<design>` (`--out-dir`, default `build`) and
//!   automatically configures + builds them with CMake
//!   (`sim::build::build_model_cmake_with_opts`). Each tool option wins over
//!   its environment fallback, which wins over the config file:
//!   `--cmake` > `$LLG_CMAKE` > `build.cmake` > `cmake`;
//!   `--cc` > `$LLG_CC` > `$CC` > `build.cc` > `cl` on Windows, `cc` elsewhere
//!   (`sim::build::DEFAULT_C_COMPILER`);
//!   `--cflags` > `$LLG_CFLAGS` > `build.cflags`.
//! - `--model-opt-level <O0|O1|O2|O3|Os>` selects model and runtime C
//!   optimization. Extra flags follow it and can override it. Release adds
//!   only NDEBUG. Source-only projects retain the selected level.
//! - The runtime archive cache is `--runtime-cache` >
//!   `$LLG_RUNTIME_CACHE_DIR` > `output.runtime_cache` >
//!   `<out-dir>/llg-runtime-cache`.
//! - `--build-jobs <N>` sets the `cmake --build --parallel` job count for the
//!   runtime archive and the model: `--build-jobs` >
//!   `$CMAKE_BUILD_PARALLEL_LEVEL` (positive integer) > `build.jobs` >
//!   available parallelism.
//! - `--generator <backend>` selects cmake's generator backend (`-G`,
//!   e.g. `Ninja`, `"Unix Makefiles"`); it overrides `$CMAKE_GENERATOR`,
//!   which overrides `build.generator`; without any, the library selects
//!   `Ninja` on every host.
//! - `--launcher <program>` selects `CMAKE_C_COMPILER_LAUNCHER` (for example,
//!   `ccache` or `sccache`): `--launcher` > `$LLG_C_LAUNCHER` >
//!   `build.launcher` > none.
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
use llg::core::lint::{LintDiag, LintSeverity};
use llg::sim;

mod cli;
mod settings;

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
    if no_arguments {
        eprintln!("{}", cli::USAGE);
        return Err(2);
    }
    let config = settings::load_config(cli.config_path.as_deref()).map_err(config_failure)?;
    let options = settings::resolve(cli, &settings::Env::from_process(), config.as_ref())
        .map_err(config_failure)?;
    if options.files.is_empty() {
        eprintln!(
            "llg: no source files given (name them on the command line or in a --config file's \
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
        lint_only,
        warnings_as_errors,
        lint_json_mode,
        lint_json_path,
        lint_config,
        wave,
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
        cli_build_options,
    } = options;
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
        limits: llg::ffi::slang::Limits::simulator(),
        ..Default::default()
    }) {
        Ok(out) => out,
        Err(compile::CompileError::Startup(e)) => {
            eprintln!("llg: compile failed to start: {e}");
            return 1;
        }
        Err(compile::CompileError::FrontendDiagnostics(diagnostics)) => {
            for d in &diagnostics {
                eprintln!(
                    "{:?}: {} {}",
                    d.severity,
                    d.location(),
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
            "{:?}: {} {}",
            d.severity,
            d.location(),
            llg::core::diagnostics::user_message(d)
        );
    }

    // 2. Lint gate: every run lints the owned db before codegen. Errors
    //    (and warnings under -Werror) stop the run.
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
    let lint_stage = llg::profile::Stage::new("lint");
    let model = llg::core::model::DesignModel::from_db(&codegen_db);
    let mut findings = llg::core::lint::lint_with_config(&codegen_db, &model, &lint_config);
    drop(model);
    drop(lint_stage);
    if warnings_as_errors {
        for finding in &mut findings {
            if finding.severity == LintSeverity::Warning {
                finding.severity = LintSeverity::Error;
            }
        }
    }
    let lint_errors = findings
        .iter()
        .filter(|d| d.severity == LintSeverity::Error)
        .count();
    if lint_json_mode {
        // Machine-readable report mode: one JSON object on stdout (or in a
        // file), no human-readable lint lines, and no codegen/simulation
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
        return i32::from(lint_errors > 0);
    }
    print_lint_findings(&findings, lint_only);
    if lint_errors > 0 {
        return 1;
    }
    if lint_only {
        return 0;
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
            waveform: wave,
            ..Default::default()
        },
    );
    let gen = match generated {
        Ok(g) => g,
        Err(e) if e.is_legacy_unsupported() => {
            for line in e.detail().lines() {
                eprintln!("error: {line}");
            }
            return 1;
        }
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
    // The cache follows --out-dir unless the flag, the environment or the
    // config file (resolved in `settings`) moves it.
    let runtime_cache_dir = runtime_cache.unwrap_or_else(|| out_root.join("llg-runtime-cache"));
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

/// Print lint findings to stderr as `file:line:col: [SEVERITY] rule: message`
/// followed by a count line. A clean design prints nothing unless the run
/// stops after lint, which confirms it with `lint: clean`.
fn print_lint_findings(findings: &[LintDiag], lint_only: bool) {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    for d in findings {
        let sev = match d.severity {
            LintSeverity::Error => {
                errors += 1;
                "ERROR"
            }
            LintSeverity::Warning => {
                warnings += 1;
                "WARNING"
            }
            LintSeverity::Info => "INFO",
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
            if let Some(logical) = &d.logical {
                loc.push_str(&format!(" (`line {}:{})", logical.file, logical.line));
            }
        }
        if !loc.is_empty() {
            loc.push_str(": ");
        }
        eprintln!("{loc}[{sev}] {}: {}", d.rule, d.message);
    }
    if !findings.is_empty() {
        eprintln!("lint: {errors} error(s), {warnings} warning(s)");
    } else if lint_only {
        eprintln!("lint: clean");
    }
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
}
