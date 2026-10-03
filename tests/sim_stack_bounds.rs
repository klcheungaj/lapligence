//! Native stack bounds of generated models.
//!
//! Generated code passes packed results and operands through pointers, so a
//! generated function's native frame does not grow with its body length. The
//! frame-size probe compiles the emitted model with each available GCC/Clang
//! that supports `-fstack-usage` and reads the reported frame sizes; it skips
//! only when no such compiler exists. The public CLI runs execute the same
//! fixtures end to end against independent Python-derived values.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::sim;

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "stack_bounds";

/// Fixed bound for a generated function with two packed locals and two
/// packed formals, independent of its body length. Measured frames are about
/// 0.7-1 KiB on x86-64 for GCC 14/Clang 19 at -O0/-O3 (persistence evidence);
/// the bound leaves room for other compilers and targets.
const LONG_BODY_FRAME_BOUND: u64 = 2048;
/// Allowed difference between the 128-statement and one-statement bodies.
/// Before destination passing the difference was about 160 bytes per statement.
const BODY_LENGTH_SLACK: u64 = 96;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/stack_bounds")
        .join(name)
}

fn generate(path: &Path, options: &sim::opt::OptConfig) -> String {
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![path.to_string_lossy().into_owned()],
        ..Default::default()
    })
    .unwrap_or_else(|error| panic!("compile {}: {error:?}", path.display()));
    let database = Db::from_slang(&compiled.snapshot)
        .unwrap_or_else(|error| panic!("import {}: {error:?}", path.display()));
    sim::codegen::generate_from_db_with_opts(&database, options)
        .unwrap_or_else(|error| panic!("generate {}: {error:?}", path.display()))
        .model_c
}

/// Whether `compiler` exists and accepts `-fstack-usage`.
fn stack_usage_compiler(compiler: &str, scratch: &Path) -> bool {
    let probe = scratch.join(format!("{compiler}-probe.c"));
    if std::fs::write(&probe, "int llg_probe(int x) { return x + 1; }\n").is_err() {
        return false;
    }
    Command::new(compiler)
        .current_dir(scratch)
        .args(["-c", "-fstack-usage", "-o"])
        .arg(scratch.join(format!("{compiler}-probe.o")))
        .arg(&probe)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Compile `model` and return `(function, frame bytes)` rows from `.su`.
fn frame_sizes(compiler: &str, level: &str, model: &str, scratch: &Path) -> Vec<(String, u64)> {
    let source = scratch.join("model.c");
    std::fs::write(&source, model).expect("write generated model");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(compiler)
        .current_dir(scratch)
        // Inlining would fold the measured functions into their caller.
        .args([
            "-std=c11",
            level,
            "-DNDEBUG",
            "-fno-inline",
            "-fstack-usage",
            "-w",
            "-c",
        ])
        .arg(format!("-I{}", root.join("src/sim/rt").display()))
        .arg(format!(
            "-I{}",
            root.join("vendor/slang/external/ieee1800").display()
        ))
        .arg("-o")
        .arg(scratch.join("model.o"))
        .arg(&source)
        .output()
        .unwrap_or_else(|error| panic!("start {compiler}: {error}"));
    assert!(
        output.status.success(),
        "{compiler} {level} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let usage = std::fs::read_to_string(scratch.join("model.su")).expect("read stack usage");
    usage
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let location = fields.next()?;
            let bytes = fields.next()?.parse().ok()?;
            Some((location.rsplit(':').next()?.to_owned(), bytes))
        })
        .collect()
}

fn frame_of(rows: &[(String, u64)], function: &str) -> u64 {
    rows.iter()
        .filter(|(name, _)| name == function || name.starts_with(&format!("{function}.")))
        .map(|(_, bytes)| *bytes)
        .max()
        .unwrap_or_else(|| panic!("no stack usage for {function}: {rows:?}"))
}

#[test]
fn long_function_body_runs() {
    sim_cli::run_case(
        SUITE,
        "long_function_body",
        "short=fffffff8fffffff8fffffff900000009\n\
         long=0000001f0000000b0000000700000079\n",
        "",
        &[],
    );
}

#[test]
fn long_function_body_frame_is_bounded() {
    let scratch = sim_harness::TempDir::new("stack-frames").expect("create scratch directory");
    let compilers = ["gcc", "clang"]
        .into_iter()
        .filter(|compiler| stack_usage_compiler(compiler, scratch.path()))
        .collect::<Vec<_>>();
    if compilers.is_empty() {
        eprintln!("SKIP: no GCC/Clang with -fstack-usage available");
        return;
    }
    let path = fixture("long_function_body.sv");
    for (mode, options) in [
        ("default", sim::opt::OptConfig::default()),
        ("no-opt", sim::opt::OptConfig::none()),
    ] {
        let model = generate(&path, &options);
        for compiler in &compilers {
            for level in ["-O0", "-O3"] {
                let rows = frame_sizes(compiler, level, &model, scratch.path());
                let short = frame_of(&rows, "fn_tb_body_short");
                let long = frame_of(&rows, "fn_tb_body_long");
                eprintln!("{compiler} {level} ({mode}): body_short {short} B, body_long {long} B");
                assert!(
                    long <= LONG_BODY_FRAME_BOUND,
                    "{compiler} {level} ({mode}): body_long frame {long} B exceeds {LONG_BODY_FRAME_BOUND} B"
                );
                assert!(
                    long <= short + BODY_LENGTH_SLACK,
                    "{compiler} {level} ({mode}): body_long frame {long} B grows over body_short {short} B"
                );
            }
        }
    }
}
