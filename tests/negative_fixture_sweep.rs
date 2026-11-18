//! Every negative simulation fixture (`neg_*` / `reject_*` under
//! `tests/fixtures/sim`) is refused before a C model exists, in both
//! optimizer modes. A fixture that generates C is accepted silently unless
//! [`MANIFEST`] lists it with the reason the stop happens elsewhere: under the
//! strict 2001 profile (checked here), or at link or run time, as a named
//! test asserts. The manifest stays minimal: a row whose fixture is now
//! refused under the default options fails as stale.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::ffi::slang::LanguageEdition;
use llg::sim;

const SHARDS: usize = 4;
const MANIFEST: &str = "tests/fixtures/sim/negative_sweep.tsv";

/// Where a manifest row's stop happens.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stop {
    /// Refused by the strict 2001 profile; the sweep rechecks it there.
    Verilog2001,
    /// Refused outside in-process generation; `test` asserts the stop.
    Elsewhere { kind: String, test: String },
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn is_negative(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("sv" | "v")
    ) && (stem.starts_with("neg_") || stem.starts_with("reject_"))
}

fn negative_fixtures() -> Vec<PathBuf> {
    fn visit(path: &Path, paths: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, paths);
            } else if is_negative(&path) {
                paths.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    visit(&root().join("tests/fixtures/sim"), &mut paths);
    paths.sort();
    paths
}

/// Manifest rows keyed by the fixture path relative to `tests/fixtures/sim`.
fn manifest() -> BTreeMap<String, Stop> {
    let text = std::fs::read_to_string(root().join(MANIFEST)).expect("read the sweep manifest");
    let mut rows = BTreeMap::new();
    for (number, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let stop = match fields.as_slice() {
            [_, "v2001"] => Stop::Verilog2001,
            [_, kind @ ("runtime" | "link"), test] => Stop::Elsewhere {
                kind: (*kind).to_owned(),
                test: (*test).to_owned(),
            },
            _ => panic!("{MANIFEST}:{}: malformed row `{line}`", number + 1),
        };
        assert!(
            rows.insert(fields[0].to_owned(), stop).is_none(),
            "{MANIFEST}:{}: duplicate row for {}",
            number + 1,
            fields[0]
        );
    }
    rows
}

fn relative(path: &Path) -> String {
    path.strip_prefix(root().join("tests/fixtures/sim"))
        .expect("fixture below tests/fixtures/sim")
        .to_string_lossy()
        .replace('\\', "/")
}

/// The optimizer modes in which `path` reaches a generated C model.
fn generating_modes(path: &Path, edition: LanguageEdition) -> Vec<&'static str> {
    let Ok(compiled) = compile::compile(&CompileOpts {
        files: vec![path.to_string_lossy().into_owned()],
        edition,
        ..Default::default()
    }) else {
        return Vec::new();
    };
    if !compiled.ok() {
        return Vec::new();
    }
    let Ok(database) = Db::from_slang(&compiled.snapshot) else {
        return Vec::new();
    };
    [
        ("default", sim::opt::OptConfig::default()),
        ("no-opt", sim::opt::OptConfig::none()),
    ]
    .into_iter()
    .filter(|(_, options)| sim::codegen::generate_from_db_with_opts(&database, options).is_ok())
    .map(|(mode, _)| mode)
    .collect()
}

fn sweep_shard(shard: usize) {
    let rows = manifest();
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for (index, path) in negative_fixtures().into_iter().enumerate() {
        if index % SHARDS != shard {
            continue;
        }
        checked += 1;
        let name = relative(&path);
        let accepted = generating_modes(&path, LanguageEdition::SystemVerilog2009);
        match (rows.get(&name), accepted.is_empty()) {
            (None, true) => {}
            (None, false) => failures.push(format!(
                "{name}: generated C in {accepted:?} without a {MANIFEST} row"
            )),
            (Some(_), true) => failures.push(format!(
                "{name}: refused under the default options; remove its stale {MANIFEST} row"
            )),
            (Some(Stop::Verilog2001), false) => {
                let accepted = generating_modes(&path, LanguageEdition::Verilog2001);
                if !accepted.is_empty() {
                    failures.push(format!(
                        "{name}: generated C in {accepted:?} under the strict 2001 profile"
                    ));
                }
            }
            (Some(Stop::Elsewhere { .. }), false) => {}
        }
    }
    assert!(
        checked > 0,
        "negative sweep shard {shard} checked no fixture"
    );
    assert!(
        failures.is_empty(),
        "negative fixtures accepted silently:\n{}",
        failures.join("\n")
    );
}

fn run_shard(shard: usize) {
    std::thread::Builder::new()
        .name(format!("negative-sweep-{shard}"))
        .stack_size(32 * 1024 * 1024)
        .spawn(move || sweep_shard(shard))
        .expect("spawn negative sweep worker")
        .join()
        .expect("negative sweep worker panicked");
}

#[test]
fn negative_sweep_0() {
    run_shard(0);
}

#[test]
fn negative_sweep_1() {
    run_shard(1);
}

#[test]
fn negative_sweep_2() {
    run_shard(2);
}

#[test]
fn negative_sweep_3() {
    run_shard(3);
}

/// Each manifest row names an existing negative fixture, and each row that
/// stops outside generation names a test function, in the named suite file,
/// that runs that fixture.
#[test]
fn manifest_rows_name_fixtures_and_tests() {
    let mut failures = Vec::new();
    for (name, stop) in manifest() {
        let path = root().join("tests/fixtures/sim").join(&name);
        if !path.is_file() || !is_negative(&path) {
            failures.push(format!("{name}: no such negative fixture"));
            continue;
        }
        let Stop::Elsewhere { kind, test } = stop else {
            continue;
        };
        let Some((suite, function)) = test.split_once("::") else {
            failures.push(format!(
                "{name}: {kind} test `{test}` is not <suite path>::<fn>"
            ));
            continue;
        };
        let source = std::fs::read_to_string(root().join("tests").join(format!("{suite}.rs")))
            .unwrap_or_default();
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        if !source.contains(&format!("fn {function}(")) || !source.contains(stem) {
            failures.push(format!(
                "{name}: tests/{suite}.rs has no `fn {function}` running `{stem}`"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
