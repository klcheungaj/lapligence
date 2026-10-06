//! Separate-process regression coverage for deterministic generated sources.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::sim_harness;

const RUNS: usize = 3;
const FIXTURES: [&str; 3] = [
    "data_types_next/static_function_executable_assignments.sv",
    "syn021_tagged_union/struct_contexts.sv",
    "compact_names/names.sv",
];

fn snapshot_tree(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    fn visit(
        root: &Path,
        directory: &Path,
        files: &mut Vec<(PathBuf, Vec<u8>)>,
    ) -> Result<(), String> {
        let mut entries = std::fs::read_dir(directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read entry in {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("inspect {}: {error}", path.display()))?;
            if file_type.is_dir() {
                visit(root, &path, files)?;
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| format!("relativize {}: {error}", path.display()))?
                    .to_path_buf();
                let contents = std::fs::read(&path)
                    .map_err(|error| format!("read {}: {error}", path.display()))?;
                files.push((relative, contents));
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn assert_same_tree(
    expected: &[(PathBuf, Vec<u8>)],
    actual: &[(PathBuf, Vec<u8>)],
    fixture: &str,
    mode: &str,
    run: usize,
) {
    let expected_paths = expected.iter().map(|(path, _)| path).collect::<Vec<_>>();
    let actual_paths = actual.iter().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(
        actual_paths, expected_paths,
        "generated file set changed for {fixture} ({mode}) on run {run}"
    );
    for ((path, expected), (_, actual)) in expected.iter().zip(actual) {
        assert!(
            actual == expected,
            "generated file {} changed for {fixture} ({mode}) on run {run} ({} bytes versus {})",
            path.display(),
            actual.len(),
            expected.len()
        );
    }
}

#[test]
fn generated_sources_are_repeatable_across_processes() {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    for fixture in FIXTURES {
        for no_opt in [false, true] {
            let mode = if no_opt { "no-opt" } else { "default" };
            let directory = sim_harness::TempDir::new("generated-c-determinism")
                .expect("create deterministic-emission directory");
            let source = fixture_root.join(fixture);
            let lint = crate::sim_cli::expected_lint(&source);
            let mut expected: Option<Vec<(PathBuf, Vec<u8>)>> = None;
            for run in 1..=RUNS {
                let output_root = directory.path().join(format!("{mode}-{run}"));
                let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
                command
                    .args(["--top", "tb", "--gen-only", "--out-dir"])
                    .arg(&output_root);
                if no_opt {
                    command.arg("--no-opt");
                }
                // A fixture that deliberately violates a lint rule runs with
                // that rule disabled; `sim_cli` checks the default-lint errors.
                if let Some(lint) = lint {
                    command.args(lint.allow_args());
                }
                command.arg(&source);
                let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
                    .expect("run llg generation process");
                assert!(
                    output.status.success(),
                    "generation failed for {fixture} ({mode}) on run {run}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let snapshot = snapshot_tree(&output_root.join("sim/tb"))
                    .expect("snapshot generated source tree");
                assert!(!snapshot.is_empty(), "generated source tree is empty");
                let (_, symbols) = snapshot
                    .iter()
                    .find(|(path, _)| path == Path::new("model.symbols.tsv"))
                    .expect("generated source tree includes the symbol map");
                if fixture == "compact_names/names.sv" {
                    assert!(!symbols.is_empty(), "long source names must be mapped");
                }
                if let Some(first) = &expected {
                    assert_same_tree(first, &snapshot, fixture, mode, run);
                } else {
                    expected = Some(snapshot);
                }
            }
        }
    }
}
