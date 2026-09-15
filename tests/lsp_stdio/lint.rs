//! Lint.

use super::*;

// ── Built-in lint rules: type / careless-mistake checks ─────────────────────
//
// End-to-end acceptance for the rules added alongside `unused-signal`:
// `implicit-net`, `case-default-missing` and `comparison-width-mismatch`.
// The fixture root is the brand-new `lint-rules/` directory (not one of the
// pinned roots A/B/C); its single source fires exactly those three rules and
// no other.

#[test]
fn lsp_stdio_publishes_new_lint_rules_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize lint-rules workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read careless fixture"),
        )
        .expect("open careless fixture");

    // All three new rules publish as llg-lint diagnostics with their default
    // warning severity (LSP severity 2).
    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        has_lint_rule(params, "implicit-net")
            && has_lint_rule(params, "case-default-missing")
            && has_lint_rule(params, "comparison-width-mismatch")
    });
    assert_eq!(lint_severity(&diagnostics, "implicit-net"), Some(2));
    assert_eq!(lint_severity(&diagnostics, "case-default-missing"), Some(2));
    assert_eq!(
        lint_severity(&diagnostics, "comparison-width-mismatch"),
        Some(2)
    );
    assert_no_shadow_uris(&diagnostics);

    // Disabling one rule through the watched root `llg.toml` removes exactly
    // that rule's findings; the other two keep publishing.
    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.case-default-missing]\n\
         enabled = false\n",
    )
    .expect("update lint-rules config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "case-default-missing")
            && has_lint_rule(params, "implicit-net")
            && has_lint_rule(params, "comparison-width-mismatch")
    });
    assert!(!has_lint_rule(&updated, "case-default-missing"));
    assert_eq!(lint_severity(&updated, "implicit-net"), Some(2));
    assert_eq!(
        lint_severity(&updated, "comparison-width-mismatch"),
        Some(2)
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_publishes_expanded_careless_mistake_rules_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless_more.sv");
    let uri = file_uri(&path);
    let rule_ids = [
        "undriven-signal",
        "incomplete-sensitivity-list",
        "out-of-range-select",
        "xz-logical-equality",
        "duplicate-case-item",
    ];

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize expanded lint-rules workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read expanded careless fixture"),
        )
        .expect("open expanded careless fixture");

    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        rule_ids.iter().all(|rule| has_lint_rule(params, rule))
    });
    for rule in rule_ids {
        assert_eq!(
            lint_severity(&diagnostics, rule),
            Some(2),
            "{rule} should publish as a warning: {diagnostics:?}"
        );
    }
    assert_no_shadow_uris(&diagnostics);

    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.out-of-range-select]\n\
         enabled = false\n",
    )
    .expect("disable out-of-range-select");
    client
        .send_watch_event(&config_path, 2)
        .expect("send expanded lint config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "out-of-range-select")
            && rule_ids
                .iter()
                .filter(|rule| **rule != "out-of-range-select")
                .all(|rule| has_lint_rule(params, rule))
    });
    assert!(!has_lint_rule(&updated, "out-of-range-select"));
    for rule in rule_ids
        .iter()
        .filter(|rule| **rule != "out-of-range-select")
    {
        assert_eq!(lint_severity(&updated, rule), Some(2));
    }
    client.shutdown();
}

#[test]
fn lsp_stdio_publishes_control_lint_batch_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless_control.sv");
    let uri = file_uri(&path);
    let rule_ids = [
        "empty-implicit-sensitivity",
        "assignment-in-condition",
        "casex-statement",
    ];

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize control lint workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read control lint fixture"),
        )
        .expect("open control lint fixture");

    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        rule_ids.iter().all(|rule| has_lint_rule(params, rule))
    });
    assert_eq!(
        diagnostics.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "diagnostics must retain the real workspace URI: {diagnostics:?}"
    );
    for rule in rule_ids {
        assert_eq!(
            lint_severity(&diagnostics, rule),
            Some(2),
            "{rule} should publish as a warning: {diagnostics:?}"
        );
    }
    assert_no_shadow_uris(&diagnostics);

    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.empty-implicit-sensitivity]\n\
         enabled = false\n\
         [lint.rules.casex-statement]\n\
         severity = \"error\"\n",
    )
    .expect("update control lint config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send control lint config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "empty-implicit-sensitivity")
            && has_lint_rule(params, "assignment-in-condition")
            && lint_severity(params, "casex-statement") == Some(1)
    });
    assert!(!has_lint_rule(&updated, "empty-implicit-sensitivity"));
    assert_eq!(lint_severity(&updated, "assignment-in-condition"), Some(2));
    assert_eq!(lint_severity(&updated, "casex-statement"), Some(1));
    assert_no_shadow_uris(&updated);
    client.shutdown();
}
