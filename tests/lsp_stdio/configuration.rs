//! Configuration.

use super::*;

#[test]
fn lsp_stdio_loads_per_root_lint_configuration() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let path_a = root_a.join("lint").join("per_root.sv");
    let path_b = root_b.join("lint").join("per_root.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize lint workspace");

    client
        .open(
            &path_a,
            &fs::read_to_string(&path_a).expect("read root A lint source"),
        )
        .expect("open root A lint source");
    client
        .open(
            &path_b,
            &fs::read_to_string(&path_b).expect("read root B lint source"),
        )
        .expect("open root B lint source");

    // root-a/llg.toml disables unused-signal, while root-b's promotes it to
    // an error.  The two open documents observe their own root configuration.
    let root_a_diagnostics = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&root_a_diagnostics, "unused-signal"));
    let root_b_diagnostics = wait_for_diagnostics(&mut client, &uri_b, |params| {
        lint_severity(params, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&root_b_diagnostics, "unused-signal"), Some(1));
    assert_no_shadow_uris(&root_b_diagnostics);
    client.shutdown();
}

#[test]
fn lsp_stdio_lints_standalone_files_without_a_workspace_folder() {
    let cwd_fixture = FixtureTree::new();
    let source_fixture = FixtureTree::new();
    let directory_a = source_fixture.base().join("standalone-a");
    let directory_b = source_fixture.base().join("standalone-b");
    fs::create_dir_all(&directory_a).expect("create first standalone directory");
    fs::create_dir_all(&directory_b).expect("create second standalone directory");
    let path_a = directory_a.join("bad.sv");
    let path_b = directory_b.join("lint.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    let broken = "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneBroken;\n";
    let lint_a = "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneLintA;\n  logic unused_a;\nendmodule\n";
    let lint_b = "// llg-lsp-fixture: standalone-b/lint.sv\nmodule StandaloneLintB;\n  logic unused_b;\nendmodule\n";
    let recovered =
        "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneRecovered; endmodule\n";

    let mut client = LspProcess::spawn(cwd_fixture.base());
    client
        .initialize(&[], default_init_options())
        .expect("initialize without workspace folders");

    client
        .open(&path_a, broken)
        .expect("open standalone syntax-error source");
    let syntax = wait_for_diagnostics(&mut client, &uri_a, has_severity_1);
    assert!(
        has_severity_1(&syntax),
        "standalone syntax error was not published"
    );

    client
        .change(&path_a, 2, lint_a)
        .expect("repair standalone syntax and introduce lint finding");
    let linted_a = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_severity_1(params) && has_lint_rule(params, "unused-signal")
    });
    assert!(has_lint_rule(&linted_a, "unused-signal"));

    client
        .open(&path_b, lint_b)
        .expect("open source in second standalone directory");
    let linted_b = wait_for_diagnostics(&mut client, &uri_b, |params| {
        has_lint_rule(params, "unused-signal")
    });
    assert!(has_lint_rule(&linted_b, "unused-signal"));

    client
        .change(&path_a, 3, recovered)
        .expect("remove standalone lint finding");
    let recovered_diagnostics = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_severity_1(params) && !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_severity_1(&recovered_diagnostics));
    assert!(!has_lint_rule(&recovered_diagnostics, "unused-signal"));
    client.shutdown();
}

#[test]
fn lsp_stdio_root_config_reload_keeps_discovery_filters() {
    // Editing the root `llg.toml` (lint-only change) must not reset source
    // discovery filters.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("discovery").join("lint_settings.sv");
    let text = format!(
        "{SOURCE_HEADER} root-a/discovery/lint_settings.sv\nmodule LintSettingsAllowed;\n  logic unused_lint_settings;\nendmodule\n"
    );
    fs::write(&path, &text).expect("write lint settings source");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize restrictive workspace");
    client
        .open(&path, &text)
        .expect("open lint settings source");
    let initial = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&initial, "unused-signal"));

    // Flip unused-signal on (as a warning) in the config; discovery filters
    // (which exclude `**/excluded/**`) must be retained.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"warning\"\n",
    )
    .expect("update root config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(lint_severity(&updated, "unused-signal"), Some(2));

    let excluded = client
        .request(
            "workspace/symbol",
            json!({ "query": "ExcludedShouldNotAppear" }),
        )
        .expect("query excluded symbol");
    assert!(
        names(&excluded)
            .iter()
            .all(|name| name != "ExcludedShouldNotAppear"),
        "config reload reset discovery filters: {excluded}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_root_config_watch_ignores_restrictive_source_include() {
    // The effective `llg.toml` is watched even when the root's discovery
    // filters are restrictive (the config controls discovery and must remain
    // observable).
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let source_dir = root_a.join("src");
    fs::create_dir_all(&source_dir).expect("create restrictive include directory");
    let source = source_dir.join("lint_watch.sv");
    let source_text = format!(
        "{SOURCE_HEADER} root-a/src/lint_watch.sv\nmodule RootLintWatch;\n  logic unused_root_lint_watch;\nendmodule\n"
    );
    fs::write(&source, &source_text).expect("write root lint watch source");
    let uri = file_uri(&source);
    // Restrictive config: only `src/**` is discoverable, `unused-signal` off.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"src/**\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = false\n",
    )
    .expect("write restrictive root config");
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize restrictive lint workspace");
    client
        .open(&source, &source_text)
        .expect("open root lint watch source");
    let initial = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&initial, "unused-signal"));

    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"src/**\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"warning\"\n",
    )
    .expect("change root lint config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send root config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(lint_severity(&updated, "unused-signal"), Some(2));
    client.shutdown();
}

#[test]
fn lsp_stdio_init_with_default_and_overridden_config_files() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");

    // Default init (no overrides): both roots load `<root>/llg.toml`.
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize with default config files");
    // root-a disables unused-signal; root-b promotes it to an error.
    let path_a = root_a.join("lint").join("per_root.sv");
    let path_b = root_b.join("lint").join("per_root.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    client
        .open(&path_a, &fs::read_to_string(&path_a).expect("read a"))
        .expect("open a");
    client
        .open(&path_b, &fs::read_to_string(&path_b).expect("read b"))
        .expect("open b");
    let diag_a = wait_for_diagnostics(&mut client, &uri_a, |p| !has_lint_rule(p, "unused-signal"));
    assert!(!has_lint_rule(&diag_a, "unused-signal"));
    let diag_b = wait_for_diagnostics(&mut client, &uri_b, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&diag_b, "unused-signal"), Some(1));
    client.shutdown();

    // Overridden init: point root-a at a custom config (in root-a) that
    // enables unused-signal as an error.
    let custom_config = root_a.join("custom.toml");
    fs::write(
        &custom_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"error\"\n",
    )
    .expect("write custom config");
    let mut client = LspProcess::spawn(&fixture.root);
    let options = init_options_with_config_files(&[(
        file_uri(&root_a).as_str(),
        custom_config.to_str().expect("path"),
    )]);
    client
        .initialize(&[("root-a", &root_a)], options)
        .expect("initialize with overridden config file");
    client
        .open(&path_a, &fs::read_to_string(&path_a).expect("read a"))
        .expect("open a");
    let overridden = wait_for_diagnostics(&mut client, &uri_a, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&overridden, "unused-signal"),
        Some(1),
        "config override must load the custom lint policy for root-a"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_config_reload_without_restart() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("lint").join("per_root.sv");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize config reload workspace");
    client
        .open(&path, &fs::read_to_string(&path).expect("read lint source"))
        .expect("open lint source");
    let initial = wait_for_diagnostics(&mut client, &uri, |p| !has_lint_rule(p, "unused-signal"));
    assert!(!has_lint_rule(&initial, "unused-signal"));

    // Reload with unused-signal as an error: no restart required.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"error\"\n",
    )
    .expect("write reloaded config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config reload event");
    let reloaded = wait_for_diagnostics(&mut client, &uri, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&reloaded, "unused-signal"), Some(1));

    // A malformed reload must retain the last-valid config and publish a
    // diagnostic against the TOML URI.
    fs::write(&config_path, "schema_version = 1\n[sources\n").expect("write malformed config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send malformed config event");
    let config_uri = file_uri(&config_path);
    wait_for_diagnostics(&mut client, &config_uri, |p| {
        p.get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|d| {
                d.iter().any(|diag| {
                    diag.get("source").and_then(Value::as_str) == Some("llg")
                        && diag.get("severity").and_then(Value::as_u64) == Some(1)
                })
            })
    });
    // The open file still gets its retained policy (unused-signal as error).
    // Identical payloads are not re-sent after every commit, so nudge the
    // buffer with an extra finding to force a fresh publication that proves
    // the malformed reload retained the last-valid lint policy.
    let retained_text = format!(
        "{}\nmodule RetainedProbe; endmodule\n",
        fs::read_to_string(&path).expect("read lint source")
    );
    client
        .change(&path, 2, &retained_text)
        .expect("nudge open buffer");
    let retained = wait_for_diagnostics(&mut client, &uri, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&retained, "unused-signal"),
        Some(1),
        "malformed reload must retain the last-valid lint policy"
    );
    client.shutdown();
}

/// An override config reached through `initializationOptions.llg.configFiles`
/// may carry ANY basename; editing it must reload that root without a restart
/// exactly like `<root>/llg.toml` does (review B).
#[test]
fn lsp_stdio_override_config_watch_reloads_without_restart() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let custom_config = root_a.join("custom.toml");
    let config_with_rule = |severity: &str| {
        format!(
            "schema_version = 1\n\
             [sources]\n\
             directories = [\".\"]\n\
             include = [\"**/*.v\", \"**/*.sv\"]\n\
             exclude = [\"**/excluded/**\"]\n\
             [lint]\n\
             enabled = true\n\
             [lint.rules.unused-signal]\n\
             enabled = true\n\
             severity = \"{severity}\"\n"
        )
    };
    fs::write(&custom_config, config_with_rule("error")).expect("write override config");

    let mut client = LspProcess::spawn(&fixture.root);
    let options = init_options_with_config_files(&[(
        file_uri(&root_a).as_str(),
        custom_config.to_str().expect("config path"),
    )]);
    client
        .initialize(&[("root-a", &root_a)], options)
        .expect("initialize override workspace");

    let path = root_a.join("lint").join("per_root.sv");
    let uri = file_uri(&path);
    client
        .open(&path, &fs::read_to_string(&path).expect("read lint source"))
        .expect("open lint source");
    let baseline = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&baseline, "unused-signal"),
        Some(1),
        "override config policy must be active initially: {baseline}"
    );

    // Modify the OVERRIDE config (not llg.toml): unused-signal demoted to a
    // warning.  The watch event classifies as `Other` by basename and must
    // still be routed to this root via its effective config path.
    fs::write(&custom_config, config_with_rule("warning")).expect("update override config");
    client
        .send_watch_event(&custom_config, 2)
        .expect("send override-config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(
        lint_severity(&updated, "unused-signal"),
        Some(2),
        "editing the override config must refresh diagnostics without restart: {updated}"
    );
    client.shutdown();
}

// ── llg/configChanged notification ─────────────────────────────────────────
//
// An effective llg.toml reload (e.g. a `[compile] defines` edit) must PUSH a
// `llg/configChanged` notification to the client: inactive-range dimming is
// pulled by the client per open buffer, so without the push it would stay
// stale indefinitely after editing defines (diagnostics republish themselves,
// they cannot carry the signal).  Client-side watcher behavior cannot be
// exercised over stdio; this pins the server half of the contract — emission
// on every state-changing reload, empty params, fresh data behind the signal,
// and NO emission when a reload parses to an identical config.

#[test]
fn lsp_stdio_config_hot_reload_pushes_config_changed_notification() {
    const SOURCE: &str = "\
module m;
`ifdef FEATURE
  logic on;
`else
  logic off;
`endif
endmodule
";
    const CONFIG_DEFINES_FEATURE: &str = "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [compile]\n\
         defines = [\"FEATURE\"]\n\
         [lint]\n\
         enabled = false\n";
    let (guard, ws) = rename_workspace("config-changed", &[("top.sv", SOURCE)]);
    let path = ws.join("top.sv");
    let uri = file_uri(&path);
    let config_path = ws.join(CONFIG_FILE);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("config-changed-ws", &ws)], default_init_options())
        .expect("initialize config-changed workspace");
    client.open(&path, SOURCE).expect("open source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Hot-reload `[compile] defines` through the watched llg.toml.
    fs::write(&config_path, CONFIG_DEFINES_FEATURE).expect("write config enabling FEATURE");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");

    // The server must PUSH the reload signal; the params object is empty by
    // contract (the client refetches everything it decorates).
    let notified = client
        .wait_for_notification_where("llg/configChanged", |params| {
            params.as_object().is_some_and(|object| object.is_empty())
        })
        .expect("llg/configChanged notification after defines hot reload");

    // The data behind the signal is already committed: the SAME open buffer
    // answers with the complementary ranges, no didChange/didSave involved.
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        if Instant::now() > deadline {
            panic!("inactive ranges never flipped after the configChanged push: {notified}");
        }
        thread::sleep(POLL_INTERVAL);
        let flipped = client
            .request_with_timeout(
                "llg/inactiveRanges",
                json!({ "uri": uri }),
                Duration::from_secs(5),
            )
            .expect("llg/inactiveRanges request during reload");
        if flipped.get("ranges") == Some(&json!([{ "startLine": 3, "endLine": 5 }])) {
            break;
        }
    }

    // A reload that parses to an IDENTICAL config changes no state and must
    // NOT re-notify.  Bounded absence window: any wrongful notification is
    // sent synchronously with the reload handling, well inside the window.
    let notifications_before = client.notifications.len();
    fs::write(&config_path, CONFIG_DEFINES_FEATURE).expect("rewrite identical config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send identical-config watch event");
    let quiet_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(message) = client.receive_until(quiet_deadline) {
        client
            .route_unsolicited(message)
            .expect("route messages while waiting out the identical reload");
    }
    assert!(
        !client.notifications[notifications_before..]
            .iter()
            .any(|message| message.get("method").and_then(Value::as_str)
                == Some("llg/configChanged")),
        "identical reload must not re-notify: {:?}",
        &client.notifications[notifications_before..]
    );

    client.shutdown();
    drop(guard);
}
