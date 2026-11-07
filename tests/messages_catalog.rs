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

/// Every runtime id is used by the runtime or the emitter, so the catalog
/// lists no message the model cannot print.
#[test]
fn every_runtime_id_is_used() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim");
    let mut text = String::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src/sim") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("c" | "h" | "rs")
            ) && path.file_name().and_then(|n| n.to_str()) != Some("llg_msg.h")
            {
                text.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    for entry in catalog::runtime_messages() {
        let name = format!("LLG_MSG_{}", entry.name);
        let used = text.match_indices(&name).any(|(i, _)| {
            !text[i + name.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        assert!(used, "{name} ({}) is never reported", entry.def.id());
    }
}
