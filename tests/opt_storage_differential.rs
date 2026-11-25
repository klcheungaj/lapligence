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
//!   A fixture with DPI-C imports is linked with the C sources beside it; only
//!   one whose imports no such source defines is not run, and every other
//!   build failure fails the sweep.
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

/// The C sources beside a fixture that define its DPI-C imports, by the names
/// fixtures give them.
fn companions(fixture: &Path) -> Vec<PathBuf> {
    let stem = fixture
        .file_stem()
        .expect("a fixture has a file name")
        .to_string_lossy();
    ["", "_dpi"]
        .iter()
        .map(|suffix| fixture.with_file_name(format!("{stem}{suffix}.c")))
        .filter(|path| path.is_file())
        .collect()
}

/// Build one generated model, with the companions of its fixture when it does
/// not link without them. `None`: the fixture imports DPI-C functions that no
/// companion defines, so it has no executable of its own. Any other build
/// failure fails the test.
fn build_model(temp: &Path, model_c: &str, fixture: &Path, label: &str) -> Option<PathBuf> {
    use sim::build::{build_model_cmake_with_opts, BuildError, CmakeBuildOpts};

    let dir = temp.join(label);
    let mut opts = CmakeBuildOpts::default();
    loop {
        let error = match build_model_cmake_with_opts(&dir, &[("model.c", model_c)], &opts) {
            Ok(executable) => return Some(executable),
            Err(error) => error,
        };
        let imports =
            matches!(&error, BuildError::MissingDpiSymbol { imports, .. } if !imports.is_empty());
        let sources = companions(fixture);
        assert!(
            imports && opts.dpi_libraries.is_empty(),
            "{}, {label}: {error}",
            fixture.display()
        );
        if sources.is_empty() {
            return None;
        }
        for (index, source) in sources.iter().enumerate() {
            // Building a model replaces the contents of its directory.
            let stem = format!("{label}{index}");
            let archive = c_compiler::static_archive(source, temp, &stem, &[])
                .unwrap_or_else(|error| panic!("{}, {label}: {error}", fixture.display()));
            opts.dpi_libraries.push(archive);
        }
    }
}

/// Run one model; the observation includes the exit status and both output
/// streams.
fn run_model(dir: &Path, executable: &Path) -> String {
    let mut command = sim_harness::command_without_core_dumps(executable);
    match sim_harness::run_command(command.current_dir(dir), RUN_TIMEOUT) {
        Ok(output) => format!(
            "status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => format!("run error: {error}"),
    }
}

/// Whether the two forms were run and compared.
fn compare_runs(fixture: &Path, pruned: &str, kept: &str) -> bool {
    let temp = sim_harness::TempDir::new("storage-diff").expect("differential build directory");
    let [pruned, kept] = [("pruned", pruned), ("kept", kept)].map(|(label, model_c)| {
        build_model(temp.path(), model_c, fixture, label)
            .map(|executable| run_model(&temp.path().join(label), &executable))
    });
    assert_eq!(
        pruned,
        kept,
        "{}: unused-storage pruning changed the run",
        fixture.display()
    );
    kept.is_some()
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
    let (mut compared, mut unlinked) = (0usize, 0usize);
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
        // A fixture that does not link alone leaves its turn to the next.
        if execute && (execute_all || differing % EXEC_STRIDE == 0) {
            if !compare_runs(&path, &pruned.model_c, &kept.model_c) {
                unlinked += 1;
                continue;
            }
            compared += 1;
        }
        differing += 1;
    }
    println!(
        "storage differential shard {shard}: {generated} generated, {compared} runs compared, \
         {unlinked} without their DPI-C functions"
    );
    assert!(
        generated > 0,
        "storage differential shard {shard} generated no models"
    );
    assert!(
        !execute || compared > 0,
        "storage differential shard {shard} compared no runs"
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
