//! Drift checks for the message catalog (`llg::core::report::catalog`):
//! `docs/messages.md` is generated from it, and the catalog covers every lint
//! rule, by-design rejection family and runtime message id.

use std::path::Path;

use llg::core::report::catalog;

fn repo_file(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| panic!("{}: {error}", full.display()))
}

#[test]
fn messages_doc_matches_the_catalog() {
    let expected = catalog::markdown();
    let actual = repo_file("docs/messages.md").replace("\r\n", "\n");
    assert!(
        actual == expected,
        "docs/messages.md is stale; regenerate it with \
         `cargo test --test general messages_catalog::regenerate -- --ignored`"
    );
}

/// Rewrite `docs/messages.md` from the catalog.
#[test]
#[ignore]
fn regenerate() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/messages.md");
    std::fs::write(&path, catalog::markdown()).expect("write docs/messages.md");
}

#[test]
fn every_lint_rule_has_one_catalog_entry() {
    let registry = llg::core::lint::LintRegistry::default_rules();
    let rules: Vec<&str> = registry.all().iter().map(|rule| rule.id()).collect();
    let catalog: Vec<&str> = catalog::LINT_RULES.iter().map(|entry| entry.rule).collect();
    assert_eq!(rules, catalog, "LINT_RULES must list the registry in order");
}

#[test]
fn every_legacy_family_has_one_catalog_entry() {
    let labels: Vec<&str> = llg::sim::legacy_unsupported::LegacyFamily::ALL
        .iter()
        .map(|family| family.label())
        .collect();
    let catalog: Vec<&str> = catalog::LEGACY_FAMILIES.iter().map(|(_, l)| *l).collect();
    assert_eq!(labels, catalog);
}
