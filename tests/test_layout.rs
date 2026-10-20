//! Integration-test layout guard.
//!
//! Cargo `autotests` is off: every `tests/*.rs` suite is a module of one
//! grouped test binary (or one of the few standalone `[[test]]` targets), so a
//! new file that is not registered would silently never compile or run. This
//! suite fails with the exact `mod` line to add instead.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Group root that registers a suite named `stem` (see `tests/readme.md`).
fn expected_root(stem: &str) -> &'static str {
    if stem.starts_with("sim_syn") {
        return "sim_syn";
    }
    match stem
        .strip_prefix("sim_")
        .and_then(|rest| rest.chars().next())
    {
        Some('a'..='m') => "sim_a_m",
        Some('n'..='z') => "sim_n_z",
        _ if stem.starts_with("runtime_") => "runtime",
        _ => "general",
    }
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `[[test]]` targets as `name -> path` from the package manifest.
fn test_targets(manifest: &str) -> BTreeMap<String, String> {
    let mut targets = BTreeMap::new();
    let mut in_test = false;
    let mut name = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_test = line == "[[test]]";
            name = None;
            continue;
        }
        if !in_test {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "name" => name = Some(value),
            "path" => {
                if let Some(name) = name.take() {
                    targets.insert(name, value);
                }
            }
            _ => {}
        }
    }
    targets
}

/// Top-level `mod name;` declarations without a `#[path]` attribute.
fn registered_modules(root_source: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut previous_was_path = false;
    for line in root_source.lines() {
        if line.starts_with("#[path") {
            previous_was_path = true;
            continue;
        }
        if let Some(name) = line
            .strip_prefix("mod ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            if !previous_was_path && name != "support" {
                modules.push(name.to_string());
            }
        }
        previous_was_path = false;
    }
    modules
}

fn suite_files(tests: &Path) -> Vec<String> {
    let mut stems: Vec<String> = fs::read_dir(tests)
        .expect("read tests directory")
        .filter_map(|entry| {
            let path = entry.expect("tests directory entry").path();
            (path.extension().is_some_and(|ext| ext == "rs"))
                .then(|| path.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect();
    stems.sort();
    stems
}

#[test]
fn every_suite_file_is_compiled_exactly_once() {
    let root = manifest_dir();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(
        manifest.contains("autotests = false"),
        "Cargo.toml must keep `autotests = false`; tests are grouped into binaries"
    );
    let targets = test_targets(&manifest);
    let mut owner: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, path) in &targets {
        let source = fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("read test target {path}: {error}"));
        owner
            .entry(name.clone())
            .or_default()
            .push(format!("[[test]] {name}"));
        for module in registered_modules(&source) {
            owner.entry(module).or_default().push(name.clone());
        }
    }

    let mut errors = Vec::new();
    for stem in suite_files(&root.join("tests")) {
        match owner.get(&stem).map(Vec::as_slice) {
            None => errors.push(format!(
                "tests/{stem}.rs is not compiled: add `mod {stem};` to tests/{}.rs",
                expected_root(&stem)
            )),
            Some([single]) if single.starts_with("[[test]]") => {}
            Some([group]) if group == expected_root(&stem) => {}
            Some([group]) => errors.push(format!(
                "tests/{stem}.rs is registered in tests/{group}.rs; move it to tests/{}.rs",
                expected_root(&stem)
            )),
            Some(many) => errors.push(format!("tests/{stem}.rs is compiled by {many:?}")),
        }
    }
    for (module, roots) in &owner {
        if !root.join("tests").join(format!("{module}.rs")).is_file() {
            errors.push(format!("{roots:?} register missing tests/{module}.rs"));
        }
    }
    assert!(
        errors.is_empty(),
        "test layout errors:\n{}",
        errors.join("\n")
    );
}

#[test]
fn group_rule_matches_documented_examples() {
    assert_eq!(expected_root("sim_syn038_ledger"), "sim_syn");
    assert_eq!(expected_root("sim_force"), "sim_a_m");
    assert_eq!(expected_root("sim_wait"), "sim_n_z");
    assert_eq!(expected_root("runtime_values"), "runtime");
    assert_eq!(expected_root("lsp_stdio"), "general");
}

/// Suites of the `general` binary that the Windows on ARM language-server lane
/// (`[profile.ci-lsp]` in `.config/nextest.toml`) selects.
fn ci_lsp_general_suites(nextest_config: &str) -> Vec<String> {
    let config: toml::Value = nextest_config.parse().expect("parse .config/nextest.toml");
    let filter = config["profile"]["ci-lsp"]["default-filter"]
        .as_str()
        .expect("[profile.ci-lsp] default-filter");
    let marker = "binary_id(=llg::general) and test(/^(";
    let start = filter
        .find(marker)
        .expect("ci-lsp filter selects `general` suites")
        + marker.len();
    let names = filter[start..]
        .split_once(")::/)")
        .expect("ci-lsp `general` suite list ends with `)::/)`")
        .0;
    names.split('|').map(str::to_string).collect()
}

/// Windows on ARM supports `llg_ls` only, so the suites its CI lane runs must
/// exist in `general` and must not execute the simulator, a C compiler or the
/// `llg` driver (which that platform does not support).
#[test]
fn ci_lsp_profile_selects_only_registered_language_server_suites() {
    let root = manifest_dir();
    let tests = root.join("tests");
    let config = fs::read_to_string(root.join(".config/nextest.toml")).expect("read nextest.toml");
    let general = fs::read_to_string(tests.join("general.rs")).expect("read general.rs");
    let registered = registered_modules(&general);
    let suites = ci_lsp_general_suites(&config);
    assert!(
        suites.iter().any(|suite| suite == "lsp_stdio"),
        "the language-server lane must run the LSP wire suite: {suites:?}"
    );
    let forbidden = [
        "sim_cli",
        "llg::sim",
        "c_compiler",
        "CARGO_BIN_EXE_llg)",
        "CARGO_BIN_EXE_llg\"",
    ];
    let mut errors = Vec::new();
    for suite in &suites {
        if !registered.contains(suite) {
            errors.push(format!(
                "ci-lsp names `{suite}`, which general.rs does not register"
            ));
            continue;
        }
        let mut sources = vec![tests.join(format!("{suite}.rs"))];
        let nested = tests.join(suite);
        if nested.is_dir() {
            sources.extend(
                fs::read_dir(&nested)
                    .expect("read suite directory")
                    .map(|entry| entry.expect("suite directory entry").path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "rs")),
            );
        }
        for path in sources {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for needle in forbidden {
                if text.contains(needle) {
                    errors.push(format!(
                        "{} uses `{needle}`; the ci-lsp lane must not run simulator suites",
                        path.display()
                    ));
                }
            }
        }
    }
    assert!(errors.is_empty(), "ci-lsp errors:\n{}", errors.join("\n"));
}

#[test]
fn ci_lsp_suite_list_is_parsed_from_the_filter() {
    let config = r#"
[profile.ci-lsp]
default-filter = "binary(=llg) or (binary_id(=llg::general) and test(/^(a|b_c)::/))"
"#;
    assert_eq!(ci_lsp_general_suites(config), ["a", "b_c"]);
}
