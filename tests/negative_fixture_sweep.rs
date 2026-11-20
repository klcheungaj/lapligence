//! Every negative simulation fixture is refused before a C model exists, in
//! both optimizer modes. Negative fixtures are the `neg_*` / `reject_*` files
//! under `tests/fixtures/sim` and the fixtures [`MANIFEST`] lists as `refused`
//! because a named test asserts their compile failure under another name. A
//! fixture that generates C is accepted silently unless [`MANIFEST`] lists it
//! with the reason the stop happens elsewhere: under the strict 2001 profile
//! (checked here), or at link or run time, as a named test asserts. The
//! manifest stays minimal: a `v2001`, `runtime` or `link` row whose fixture is
//! now refused under the default options fails as stale.

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
    /// Refused under the default options, like a `neg_*` fixture; only for
    /// fixtures outside that naming.
    Refused,
    /// Refused by the strict 2001 profile; the sweep rechecks it there.
    Verilog2001,
    /// Refused outside in-process generation; the row's test asserts the stop.
    Elsewhere { kind: String },
}

/// A manifest row: the stop and, where one is required, the
/// `<suite path>::<test fn>` that asserts it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    stop: Stop,
    test: Option<String>,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Named as a negative fixture (`neg_*` / `reject_*`).
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

/// The named negative fixtures and every other fixture a manifest row lists.
fn negative_fixtures(rows: &BTreeMap<String, Row>) -> Vec<PathBuf> {
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
    for name in rows.keys() {
        let path = root().join("tests/fixtures/sim").join(name);
        if path.is_file() && !is_negative(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

/// Manifest rows keyed by the fixture path relative to `tests/fixtures/sim`.
fn manifest() -> BTreeMap<String, Row> {
    let text = std::fs::read_to_string(root().join(MANIFEST)).expect("read the sweep manifest");
    let mut rows = BTreeMap::new();
    for (number, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let row = match fields.as_slice() {
            [_, "v2001"] => Row {
                stop: Stop::Verilog2001,
                test: None,
            },
            [_, "v2001", test] => Row {
                stop: Stop::Verilog2001,
                test: Some((*test).to_owned()),
            },
            [_, "refused", test] => Row {
                stop: Stop::Refused,
                test: Some((*test).to_owned()),
            },
            [_, kind @ ("runtime" | "link"), test] => Row {
                stop: Stop::Elsewhere {
                    kind: (*kind).to_owned(),
                },
                test: Some((*test).to_owned()),
            },
            _ => panic!("{MANIFEST}:{}: malformed row `{line}`", number + 1),
        };
        assert!(
            rows.insert(fields[0].to_owned(), row).is_none(),
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
    // The checked compile of simulation: frontend errors and protected
    // envelopes refuse the design.
    let Ok(compiled) = compile::compile_checked(&CompileOpts {
        files: vec![path.to_string_lossy().into_owned()],
        edition,
        ..Default::default()
    }) else {
        return Vec::new();
    };
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
    for (index, path) in negative_fixtures(&rows).into_iter().enumerate() {
        if index % SHARDS != shard {
            continue;
        }
        checked += 1;
        let name = relative(&path);
        let accepted = generating_modes(&path, LanguageEdition::SystemVerilog2009);
        match (rows.get(&name).map(|row| &row.stop), accepted.is_empty()) {
            (None | Some(Stop::Refused), true) => {}
            (None, false) => failures.push(format!(
                "{name}: generated C in {accepted:?} without a {MANIFEST} row"
            )),
            (Some(Stop::Refused), false) => failures.push(format!(
                "{name}: listed as refused but generated C in {accepted:?}"
            )),
            (Some(_), true) => failures.push(format!(
                "{name}: refused under the default options; its {MANIFEST} row is stale"
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

/// Each manifest row names an existing fixture. A `refused` row names a
/// fixture outside the negative naming, and each row that stops outside
/// generation or lists such a fixture names a test function, in the named
/// suite file, that runs that fixture.
#[test]
fn manifest_rows_name_fixtures_and_tests() {
    let mut failures = Vec::new();
    for (name, row) in manifest() {
        let path = root().join("tests/fixtures/sim").join(&name);
        if !path.is_file() {
            failures.push(format!("{name}: no such fixture"));
            continue;
        }
        let named = is_negative(&path);
        if named && row.stop == Stop::Refused {
            failures.push(format!(
                "{name}: a `neg_*` / `reject_*` fixture is swept without a `refused` row"
            ));
            continue;
        }
        let kind = match &row.stop {
            Stop::Refused => "refused",
            Stop::Verilog2001 => "v2001",
            Stop::Elsewhere { kind } => kind.as_str(),
        };
        let Some(test) = row.test else {
            if !named {
                failures.push(format!(
                    "{name}: a fixture outside the negative naming needs the test asserting its stop"
                ));
            }
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
