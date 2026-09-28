use std::path::{Path, PathBuf};

use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::sim;

#[path = "support/generated_c_lint.rs"]
mod generated_c_lint;

const SHARDS: usize = 6;

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

fn lint_fixture_shard(shard: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let mut generated = 0usize;
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
        for (mode, options) in [
            ("default", sim::opt::OptConfig::default()),
            ("no-opt", sim::opt::OptConfig::none()),
        ] {
            let Ok(model) = sim::codegen::generate_from_db_with_opts(&database, &options) else {
                continue;
            };
            generated += 1;
            if let Err(errors) = generated_c_lint::lint_generated_coroutine_c(&model.model_c) {
                panic!(
                    "{} ({mode}) failed generated coroutine C lint:\n{}",
                    path.display(),
                    errors.join("\n")
                );
            }
        }
    }
    assert!(generated > 0, "fixture shard {shard} generated no C models");
}

fn run_shard(shard: usize) {
    std::thread::Builder::new()
        .name(format!("generated-c-lint-{shard}"))
        .stack_size(32 * 1024 * 1024)
        .spawn(move || lint_fixture_shard(shard))
        .expect("spawn generated-C lint worker")
        .join()
        .expect("generated-C lint worker panicked");
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

#[test]
fn fixture_sweep_4() {
    run_shard(4);
}

#[test]
fn fixture_sweep_5() {
    run_shard(5);
}
