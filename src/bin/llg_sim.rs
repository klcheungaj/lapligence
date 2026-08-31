//! llg_sim — Lapligence Verilog/SystemVerilog → C11 simulator driver.
//!
//! Usage:
//!
//! ```text
//! llg_sim [generate options] [build options] <file.sv>...
//! generate: --top <module>  --lint  --lint-json [<path>]  --lint-config <file>  --gen-only
//! build:    --generator <backend>        # cmake -G backend (Ninja, "Unix Makefiles", ...)
//! ```
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
//! Surelog's own stderr output remains.  When both `--lint` and `--lint-json`
//! are given, `--lint-json` wins.  Exit codes: 0 clean, 1 on lint errors, 2
//! usage errors.
//!
//! Model build (CMake is the only supported model builder):
//!
//! - After C emission the driver writes the model plus runtime/libaco sources
//!   into `target/sim/<design>` and automatically configures + builds them
//!   with CMake (`sim::build::build_model_cmake_with_opts`; `$LLG_CMAKE` or
//!   `cmake`; compiler from `$LLG_CC` / `$CC` / `cc`; extra flags via
//!   `$LLG_CFLAGS`).
//! - `--generator <backend>` selects cmake's generator backend (`-G`,
//!   e.g. `Ninja`, `"Unix Makefiles"`); it overrides `$CMAKE_GENERATOR`.
//!   Ignored with a warning when combined with `--gen-only`.
//! - `--gen-only` stops after emitting the model + runtime +
//!   `CMakeLists.txt` into `target/sim/<design>` (prints the directory,
//!   exits 0) without configuring/building/running.
//!
//! Flow: compile + elaborate with Surelog (via `core::compile`), lower the
//! elaborated UHDM to C11 (`sim::codegen::generate`), write the model plus the
//! runtime and libaco into `target/sim/<design>`, build through CMake
//! (unless `--gen-only`), and run the resulting simulator (stdout inherits;
//! the exit code is the simulator's).

use std::path::PathBuf;
use std::process::Command;

use llg::core::compile;
use llg::sim;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage: llg_sim [generate options] [build options] <file.sv>...\n\
             generate: --top <module>  --lint  --lint-json [<path>]  --lint-config <file>  --gen-only\n\
             build:    --generator <backend>        # cmake -G backend (Ninja, \"Unix Makefiles\", ...)"
        );
        std::process::exit(2);
    }

    let mut top: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut lint_mode = false;
    let mut lint_json_mode = false;
    let mut lint_json_path: Option<PathBuf> = None;
    let mut lint_config_path: Option<PathBuf> = None;
    let mut generator: Option<String> = None;
    let mut gen_only = false;
    let mut it = args.into_iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--top" | "-top" => top = it.next(),
            "--generator" | "-generator" => match it.next() {
                Some(g) => generator = Some(g),
                None => {
                    eprintln!("llg_sim: --generator requires a backend name");
                    std::process::exit(2);
                }
            },
            "--gen-only" | "-gen-only" => gen_only = true,
            "--lint" | "-lint" => lint_mode = true,
            "--lint-json" | "-lint-json" => {
                lint_mode = true;
                lint_json_mode = true;
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
                    eprintln!("llg_sim: --lint-config requires a file path");
                    std::process::exit(2);
                }
            },
            _ => files.push(a),
        }
    }
    if files.is_empty() {
        eprintln!("llg_sim: no source files given");
        std::process::exit(2);
    }

    // 0. Optional lint config: read + parse before compiling so a missing or
    //    malformed file aborts fast and with a clear message.
    let mut lint_config = llg::core::lint::LintConfig::new();
    if let Some(path) = &lint_config_path {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("llg_sim: cannot read lint config {}: {e}", path.display());
                std::process::exit(1);
            }
        };
        if let Err(errs) = lint_config.parse_toml(&text) {
            for e in &errs {
                eprintln!("llg_sim: lint config: {e}");
            }
            eprintln!("llg_sim: aborting due to lint config errors");
            std::process::exit(1);
        }
    }

    // 1. Surelog parse + compile + elaborate + -elabuhdm.
    let out = match compile::compile(&compile::CompileOpts {
        files,
        top,
        ..Default::default()
    }) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("llg_sim: compile failed to start: {e}");
            std::process::exit(1);
        }
    };
    for d in &out.diagnostics {
        eprintln!(
            "{:?}: {}:{}:{} {}",
            d.severity,
            d.file.as_deref().unwrap_or(""),
            d.line,
            d.col,
            d.message
        );
    }
    if !out.ok() {
        eprintln!("llg_sim: surelog reported errors; aborting");
        std::process::exit(1);
    }

    // 2. Lint gate (--lint mode): build the owned db + model, print findings,
    //    and abort on lint errors before codegen.
    let design = match out.uhdm_design() {
        Some(d) => d,
        None => {
            eprintln!("llg_sim: no elaborated UHDM design");
            std::process::exit(1);
        }
    };
    if lint_mode {
        let db = match llg::core::db::Db::build(design) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("lint: db build failed: {e}");
                std::process::exit(1);
            }
        };
        let model = llg::core::model::DesignModel::from_db(&db);
        let findings = llg::core::lint::lint_with_config(&db, &model, &lint_config);

        if lint_json_mode {
            // Machine-readable report mode: one JSON object on stdout (or in
            // a file), no human-readable lint lines, and no codegen/simulation
            // afterwards — the report is the entire stdout output.
            let json = llg::core::lint::diags_to_json(&findings);
            match &lint_json_path {
                Some(path) => {
                    if let Err(e) = std::fs::write(path, format!("{json}\n")) {
                        eprintln!("llg_sim: cannot write lint JSON {}: {e}", path.display());
                        std::process::exit(1);
                    }
                }
                None => println!("{json}"),
            }
            let errors = findings
                .iter()
                .filter(|d| d.severity == llg::core::lint::LintSeverity::Error)
                .count();
            if errors > 0 {
                std::process::exit(1);
            }
            std::process::exit(0);
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
                std::process::exit(1);
            }
        }
    }

    // 3. Codegen.
    let gen = match sim::codegen::generate(design) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("llg_sim: codegen error: {e}");
            std::process::exit(1);
        }
    };
    for w in &gen.warnings {
        eprintln!("llg_sim: warning: {w}");
    }

    // 4. Write sources (+ CMakeLists.txt).  With --gen-only, stop here: the
    //    emitted directory is the output, nothing is configured or run.
    let out_dir = PathBuf::from("target/sim").join(&gen_name(&gen));
    let model = [("model.c", gen.model_c.as_str())];
    if gen_only {
        if generator.is_some() {
            eprintln!("llg_sim: warning: --generator ignored with --gen-only");
        }
        if let Err(e) = sim::build::generate_model_sources(&out_dir, &model) {
            eprintln!("llg_sim: {e}");
            std::process::exit(1);
        }
        println!("{}", out_dir.display());
        std::process::exit(0);
    }

    // 5. Build the model with CMake (the only supported builder).
    let opts = sim::build::CmakeBuildOpts { generator };
    let exe = match sim::build::build_model_cmake_with_opts(&out_dir, &model, &opts) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("llg_sim: {e}");
            std::process::exit(1);
        }
    };

    // 6. Run the simulator; propagate its exit code.
    let status = match Command::new(&exe).status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("llg_sim: failed to run {}: {e}", exe.display());
            std::process::exit(1);
        }
    };
    std::process::exit(status.code().unwrap_or(1));
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
