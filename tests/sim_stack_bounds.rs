//! Native stack bounds of generated models.
//!
//! Generated code passes packed results and operands through pointers, so a
//! generated function's native frame does not grow with its body length. The
//! frame-size probe compiles the emitted model with each available GCC/Clang
//! that supports `-fstack-usage` and reads the reported frame sizes; it skips
//! only when no such compiler exists. The public CLI runs execute the same
//! fixtures end to end against independent Python-derived values.
//!
//! Recursive subprograms run as stackless coroutines whose recursive calls
//! use the chain arena, so SystemVerilog recursion depth does not consume
//! native stack. The deep-recursion fixture runs through the public CLI and
//! its built model is then rerun under a small POSIX stack limit.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::Duration;

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

/// Independent model of `mix` in `deep_recursion.sv`: 64-bit wrapping
/// arithmetic over `n + 1` activations.
fn mix(n: u32, a: u64) -> u64 {
    // (addend or the input `a`, multiplier, shift of `a`) per statement pair.
    const STEPS: [(Option<u64>, u64, u32); 8] = [
        (None, 3, 1),
        (Some(5), 7, 2),
        (None, 11, 3),
        (Some(13), 17, 4),
        (None, 19, 5),
        (Some(23), 29, 6),
        (None, 31, 7),
        (Some(37), 41, 8),
    ];
    let mut b = a;
    for (addend, multiplier, shift) in STEPS {
        b = b.wrapping_add(addend.unwrap_or(a)) ^ b.wrapping_mul(multiplier);
        b = b.wrapping_sub(a >> shift);
    }
    if n == 0 {
        b
    } else {
        mix(n - 1, b).wrapping_add(1)
    }
}

fn deep_recursion_expected() -> String {
    format!(
        "mix={:016x}\neven=1 odd=1\ncalls=250 total={}\nlen=250 half=250.5\nlist={} doubled={}\nvif={}\n",
        mix(250, 0x0123_4567_89ab_cdef),
        (1..=250u32).sum::<u32>(),
        (0..=200u32).sum::<u32>(),
        2 * 200 + (0..200u32).sum::<u32>(),
        7 + 240,
    )
}

#[test]
fn deep_recursion_runs() {
    sim_cli::run_case(SUITE, "deep_recursion", &deep_recursion_expected(), "", &[]);
}

/// Stack limit for rerunning the deep-recursion model. With native C
/// recursion this fixture crashed at `ulimit -s 256`; with arena recursion
/// it runs in 64 KiB (see the stack-bounding evidence).
#[cfg(unix)]
const SMALL_STACK_KIB: &str = "256";

#[cfg(unix)]
#[test]
fn deep_recursion_runs_under_a_small_stack_limit() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let expected = deep_recursion_expected();
    for optimized in [true, false] {
        let directory =
            sim_harness::TempDir::new("stack-recursion").expect("create recursion directory");
        let mut build = Command::new(env!("CARGO_BIN_EXE_llg"));
        build
            .current_dir(directory.path())
            .args(["--top", "tb", "--out-dir", "out"]);
        if !optimized {
            build.arg("--no-opt");
        }
        build.arg(fixture("deep_recursion.sv"));
        let output = sim_harness::run_command(&mut build, Duration::from_secs(180))
            .expect("run llg on the deep-recursion fixture");
        assert!(
            output.status.success(),
            "llg failed (optimized={optimized}): {output:?}"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);

        let executable = directory.path().join("out/sim/tb/build/bin/sim");
        let output = sim_harness::run_command(
            Command::new("sh")
                .args([
                    "-c",
                    "ulimit -s \"$1\" && exec \"$2\"",
                    "llg-small-stack",
                    SMALL_STACK_KIB,
                ])
                .arg(&executable),
            Duration::from_secs(60),
        )
        .expect("rerun the deep-recursion model");
        assert!(
            output.status.success(),
            "model failed under ulimit -s {SMALL_STACK_KIB} (optimized={optimized}): {output:?}"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
}
