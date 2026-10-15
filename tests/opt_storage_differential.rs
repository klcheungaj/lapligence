//! Unused-storage differential over every simulator fixture.
//!
//! Each fixture is generated twice from one owned DB: with the default
//! optimizer and with the same passes minus `unused_storage`. The pass may
//! only drop declarations, so:
//!
//! - when the model without the pass generates, the model with it must too
//!   (an omitted signal that generated code still names is an emission error);
//! - when the two models differ, the optimized one must still compile;
//! - a deterministic sample of the differing fixtures is built and run in both
//!   forms, and the runs must match. `LLG_STORAGE_DIFF_EXEC=all` runs every
//!   differing fixture instead (about 70 today, roughly a minute on 8 cores).
//!
//! Identical models need no run. Most fixtures use all of their variables, so
//! the pass omits storage in only a few percent of them; the targeted
//! regression cases live in `fixtures/sim/opt_storage`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::sim;

use crate::c_compiler;
use crate::sim_harness;

const SHARDS: usize = 4;
/// Every `EXEC_STRIDE`-th differing fixture of a shard is also built and run.
const EXEC_STRIDE: usize = 8;
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

fn fixture_paths(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, paths: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, paths);
            } else if matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("sv" | "v")
            ) {
                paths.push(path);
            }
        }
    }

    let mut paths = Vec::new();
    visit(root, &mut paths);
    paths.sort();
    paths
}

fn syntax_check(compiler: &str, model: &str, fixture: &Path) {
    use std::io::Write;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = sim::value_backend::ValueConfig::default();
    let mut child = Command::new(compiler)
        .args(["-x", "c", "-std=c11", "-fsyntax-only", "-w", "-"])
        .arg(format!("-I{}", root.join("src/sim/rt").display()))
        .arg(format!("-DLLG_SV4_USE_GMP={}", config.backend.selector()))
        .arg(format!(
            "-DLLG_SV4_GMP_KERNELS={}",
            config.kernel.selector()
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("start {compiler}: {error}"));
    child
        .stdin
        .as_mut()
        .expect("compiler stdin")
        .write_all(model.as_bytes())
        .unwrap_or_else(|error| panic!("write {} to {compiler}: {error}", fixture.display()));
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("wait for {compiler}: {error}"));
    assert!(
        output.status.success(),
        "{}: the model with unused-storage pruning does not compile:\n{}",
        fixture.display(),
        c_compiler::compiler_diagnostics(&output)
    );
}

/// Build and run one generated model; the observation includes the exit
/// status and both output streams.
fn run_model(dir: &Path, model_c: &str) -> String {
    let executable = match sim::build::build_model_cmake(dir, &[("model.c", model_c)]) {
        Ok(executable) => executable,
        Err(error) => return format!("build error: {error}"),
    };
    match sim_harness::run_command(Command::new(&executable).current_dir(dir), RUN_TIMEOUT) {
        Ok(output) => format!(
            "status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => format!("run error: {error}"),
    }
}

fn compare_runs(fixture: &Path, pruned: &str, kept: &str) {
    let temp = sim_harness::TempDir::new("storage-diff").expect("differential build directory");
    let pruned_run = run_model(&temp.path().join("pruned"), pruned);
    let kept_run = run_model(&temp.path().join("kept"), kept);
    assert_eq!(
        pruned_run,
        kept_run,
        "{}: unused-storage pruning changed the run",
        fixture.display()
    );
}

fn sweep_shard(shard: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let kept_config = sim::opt::OptConfig {
        unused_storage: false,
        ..sim::opt::OptConfig::default()
    };
    let compiler = c_compiler::host_c_compiler();
    let syntax = !c_compiler::is_msvc(&compiler) && c_compiler::c_compiler_available(&compiler);
    let execute_all = std::env::var("LLG_STORAGE_DIFF_EXEC").is_ok_and(|value| value == "all");
    let execute = sim::build::cmake_available();
    let (mut generated, mut differing) = (0usize, 0usize);
    for (index, path) in fixture_paths(&root).into_iter().enumerate() {
        if index % SHARDS != shard {
            continue;
        }
        let Ok(compiled) = compile::compile(&CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            ..Default::default()
        }) else {
            continue;
        };
        if !compiled.ok() {
            continue;
        }
        let Ok(database) = Db::from_slang(&compiled.snapshot) else {
            continue;
        };
        let Ok(kept) = sim::codegen::generate_from_db_with_opts(&database, &kept_config) else {
            continue;
        };
        let pruned =
            sim::codegen::generate_from_db_with_opts(&database, &sim::opt::OptConfig::default())
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: generation fails only with unused-storage pruning: {error}",
                        path.display()
                    )
                });
        generated += 1;
        if pruned.model_c == kept.model_c {
            continue;
        }
        if syntax {
            syntax_check(&compiler, &pruned.model_c, &path);
        }
        if execute && (execute_all || differing % EXEC_STRIDE == 0) {
            compare_runs(&path, &pruned.model_c, &kept.model_c);
        }
        differing += 1;
    }
    assert!(
        generated > 0,
        "storage differential shard {shard} generated no models"
    );
}

fn run_shard(shard: usize) {
    std::thread::Builder::new()
        .name(format!("storage-differential-{shard}"))
        .stack_size(32 * 1024 * 1024)
        .spawn(move || sweep_shard(shard))
        .expect("spawn storage differential worker")
        .join()
        .expect("storage differential worker panicked");
}

#[test]
fn fixture_sweep_0() {
    run_shard(0);
}

#[test]
fn fixture_sweep_1() {
    run_shard(1);
}

#[test]
fn fixture_sweep_2() {
    run_shard(2);
}

#[test]
fn fixture_sweep_3() {
    run_shard(3);
}
