//! The full record shape grid: every supported source into every supported
//! target for each record type, built with a member array of 16 cells (one
//! column) and of 3 cells (one leaf per cell), which must print the same.
//!
//! Ignored by default (hundreds of builds); the covering subset in
//! `sim_feature_completion/ki_record_shapes.rs` runs in the default suite.
//! Run it with
//! `scripts/run-tests.sh --test-work-dir /build --test sim_record_shape_grid --run-ignored only`.
//! The designs come from the tracked generator, which writes them to a
//! scratch directory.

use crate::sim_harness;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn generator_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/ki_record_shapes")
}

fn run(source: &Path, cells: &str, work: &Path) -> std::process::Output {
    let allow = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/lint/allow_mixed_assignments.toml");
    let directory = sim_harness::TempDir::new("record-shape-grid").expect("grid work directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .current_dir(directory.path())
        .arg("--config")
        .arg(&allow)
        .args(["-D", cells, "--top", "tb"])
        .arg(source)
        .env("LLG_RUNTIME_CACHE_DIR", work.join("runtime-cache"));
    sim_harness::run_command(&mut command, Duration::from_secs(300)).expect("grid model run")
}

#[test]
#[ignore = "builds hundreds of models; run on demand"]
fn record_shape_grid_prints_the_same_for_a_column_and_for_leaves() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let work = sim_harness::TempDir::new("record-shape-grid-sources").expect("grid directory");
    let generated = Command::new("python3")
        .arg(generator_dir().join("gen_record_shapes.py"))
        .arg("full")
        .arg(work.path())
        .output()
        .expect("record shape generator requires Python 3");
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let mut sources: Vec<PathBuf> = std::fs::read_dir(work.path())
        .expect("generated designs")
        .map(|entry| entry.expect("generated entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sv"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "the generator wrote no design");
    let threads = std::thread::available_parallelism()
        .map_or(2, |count| count.get() / 4)
        .max(1);
    let next = AtomicUsize::new(0);
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(source) = sources.get(index) else {
                    break;
                };
                let wide = run(source, "N=16", work.path());
                let narrow = run(source, "N=3", work.path());
                let name = source.file_name().unwrap().to_string_lossy().into_owned();
                if !wide.status.success() || !narrow.status.success() {
                    failures.lock().unwrap().push(format!(
                        "{name}: status {:?} at 16 cells, {:?} at 3: {}",
                        wide.status.code(),
                        narrow.status.code(),
                        String::from_utf8_lossy(&wide.stderr)
                    ));
                } else if wide.stdout != narrow.stdout {
                    failures
                        .lock()
                        .unwrap()
                        .push(format!("{name}: output differs between 16 and 3 cells"));
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
