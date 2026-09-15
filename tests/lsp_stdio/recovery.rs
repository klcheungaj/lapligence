//! Recovery.

use super::*;

/// Diagnostics are published project-wide: a compilation unit that is NEVER
/// opened still receives its findings, and fixing it on disk refreshes its
/// publication through a watched-file event — all while only the other file
/// was ever opened.
#[test]
fn lsp_stdio_publishes_and_clears_diagnostics_for_never_opened_files() {
    let fixture = FixtureTree::new();
    let root = fixture.root("project-wide");
    let good = root.join("good.sv");
    let broken = root.join("broken.sv");
    // The fix removes the broken assignment; the file stays closed.  The
    // remaining diagnostics may still carry frontend warnings (e.g. the
    // missing-timescale notice), so the refresh is asserted as "the syntax
    // error is gone", not "the list is empty".
    let original = fs::read_to_string(&broken).expect("read broken source");
    assert!(original.contains("assign broken_signal = ;"));
    let fixed = original.replace("  assign broken_signal = ;\n", "");
    assert_ne!(fixed, original);

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("project-wide", &root)], default_init_options())
        .expect("initialize project-wide workspace");

    // Only file A (good.sv) is ever opened; B (broken.sv) stays closed.
    let good_text = fs::read_to_string(&good).expect("read good source");
    client.open(&good, &good_text).expect("open good.sv");

    // File B was never opened, yet its error reaches the client.
    let broken_uri = file_uri(&broken);
    let error_params = wait_for_diagnostics(&mut client, &broken_uri, has_error_or_warning);
    assert!(
        has_severity_1(&error_params),
        "never-opened file must publish its syntax error"
    );

    // The opened file keeps receiving its own publications alongside.
    wait_for_diagnostics(&mut client, &file_uri(&good), |_params| true);

    // Fixing B on disk + watched-file event refreshes B's diagnostics
    // without ever opening it.
    fs::write(&broken, fixed).expect("fix broken source on disk");
    client
        .send_watch_event(&broken, 2)
        .expect("watched change for fixed source");
    let refreshed = wait_for_diagnostics(&mut client, &broken_uri, |params| {
        !params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("Syntax error"))
                })
            })
    });
    assert!(
        !has_severity_1(&refreshed),
        "fixed never-opened file must lose its error: {refreshed}"
    );
    client.shutdown();
}

/// A project whose sources contain a frontend error still serves navigation
/// features from the partial analysis: Slang retains the surviving semantic
/// data when an include fails, so documentSymbol returns modules and hover resolves the
/// declaration even though the whole-project analysis never reaches strict
/// validity.  Diagnostics keep publishing regardless.
#[test]
fn lsp_stdio_error_project_still_serves_features() {
    let fixture = FixtureTree::new();
    let root = fixture.root("err-proj");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create err-proj root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write err-proj config");
    let clean = src.join("clean_mod.sv");
    // The task's named clean module: `module clean_mod(...)`.
    let clean_text = concat!(
        "// llg-lsp-fixture: err-proj/src/clean_mod.sv\n",
        "module clean_mod (\n",
        "  input logic clk\n",
        ");\n",
        "  logic value;\n",
        "endmodule\n"
    );
    fs::write(&clean, clean_text).expect("write clean source");
    // Guaranteed frontend error (Severity::Error, no syntax cascade): the
    // include target does not exist anywhere in the configured dirs.
    let broken = src.join("broken_include.sv");
    fs::write(
        &broken,
        concat!(
            "// llg-lsp-fixture: err-proj/src/broken_include.sv\n",
            "`include \"missing_defs.svh\"\n",
            "module broken_inc;\n",
            "  logic bi_signal;\n",
            "endmodule\n"
        ),
    )
    .expect("write include-broken source");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("err-proj", &root)], default_init_options())
        .expect("initialize err-proj workspace");

    // Premise: the project really is tainted — the broken unit publishes an
    // error while it is NEVER opened.
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // didOpen the clean file; features must be served for it despite the
    // project-wide error.
    client.open(&clean, clean_text).expect("open clean source");
    let clean_uri = file_uri(&clean);
    let symbols = wait_for_document_symbols(&mut client, &clean_uri, |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&symbols, "clean_mod");
    assert_no_shadow_uris(&symbols);
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": clean_uri },
                "position": position_at(clean_text, "module clean_mod", "module ".len())
            }),
        )
        .expect("hover on clean module declaration");
    assert!(
        !hover.is_null(),
        "hover on clean_mod must be served from the error project: {hover}"
    );
    assert!(hover.get("contents").is_some(), "hover shape: {hover}");
    client.shutdown();
}

/// A project with a syntax error lacks complete semantic data, but its lexical
/// snapshot survives: declaration-level features must serve
/// (parse-tree fallback) instead of leaving the root feature-less until the
/// file is fixed.  The opened clean file gets its module document symbol and
/// hover, and the workspace index finds declarations even inside the
/// unterminated broken unit; instance-level data stays unavailable.
#[test]
fn lsp_stdio_syntax_error_still_serves_declarations() {
    let fixture = FixtureTree::new();
    let root = fixture.root("decl-fallback");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create decl-fallback root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write decl-fallback config");
    let clean = src.join("clean_module.sv");
    let clean_text = concat!(
        "// llg-lsp-fixture: decl-fallback/src/clean_module.sv\n",
        "module clean_mod (\n",
        "  input logic clk\n",
        ");\n",
        "  logic value;\n",
        "endmodule\n"
    );
    fs::write(&clean, clean_text).expect("write clean source");
    // Unterminated module: a guaranteed syntax error that prevents a complete
    // semantic snapshot.
    let broken = src.join("broken.sv");
    fs::write(
        &broken,
        concat!(
            "// llg-lsp-fixture: decl-fallback/src/broken.sv\n",
            "module broken_unterminated;\n",
            "  logic bi_signal;\n"
        ),
    )
    .expect("write unterminated source");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("decl-fallback", &root)], default_init_options())
        .expect("initialize decl-fallback workspace");

    // Premise: the project really is syntax-broken — the unterminated module
    // publishes an error while it is NEVER opened.
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // didOpen the clean file: document symbols must include the declared
    // module despite the project-wide syntax failure.
    client.open(&clean, clean_text).expect("open clean source");
    let clean_uri = file_uri(&clean);
    let symbols = wait_for_document_symbols(&mut client, &clean_uri, |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&symbols, "clean_mod");
    assert_no_shadow_uris(&symbols);

    // The workspace index finds the clean module AND the broken unit's
    // declaration (the frontend's error recovery keeps its header).
    let workspace = wait_for_workspace_symbols(&mut client, "clean_mod", |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&workspace, "clean_mod");
    assert_no_shadow_uris(&workspace);
    let broken_symbols = wait_for_workspace_symbols(&mut client, "broken_unterminated", |result| {
        names(result)
            .iter()
            .any(|name| name == "broken_unterminated")
    });
    assert_has_name(&broken_symbols, "broken_unterminated");

    // Hover on the clean module declaration is served.
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": clean_uri },
                "position": position_at(clean_text, "module clean_mod", "module ".len())
            }),
        )
        .expect("hover on clean_mod under syntax failure");
    assert!(
        !hover.is_null(),
        "hover on clean_mod must be served by the parse-tree fallback: {hover}"
    );
    assert!(hover.get("contents").is_some(), "hover shape: {hover}");
    client.shutdown();
}

/// A Fatal analysis (include-isolation preflight failure discovered by the
/// initial scan) is feature-less: no snapshot ever existed, so
/// documentSymbol answers null/empty while the preflight diagnostic still
/// arrives.
#[test]
fn lsp_stdio_fatal_analysis_stays_featureless() {
    let fixture = FixtureTree::new();
    let root = fixture.root("fatal-root");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create fatal fixture root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write fatal root config");
    let top = src.join("top.sv");
    fs::write(
        &top,
        concat!(
            "// llg-lsp-fixture: fatal-root/src/top.sv\n",
            "`include \"../../outside_fatal.svh\"\n",
            "module FatalTop; endmodule\n"
        ),
    )
    .expect("write escaping source");
    let top_uri = file_uri(&top);

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("fatal-root", &root)], default_init_options())
        .expect("initialize fatal workspace");

    // The preflight failure is published against the compiled file even
    // though it was never opened.
    let fatal_params = wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| {
                            message.contains("escapes configured source/include directories")
                        })
                })
            })
    });
    assert!(
        has_severity_1(&fatal_params),
        "preflight escape must publish an error: {fatal_params}"
    );

    // No servable snapshot ever existed: features stay empty/null.
    let symbols = wait_for_document_symbols(&mut client, &top_uri, Value::is_null);
    assert!(
        symbols.is_null(),
        "documentSymbol must stay feature-less on a fatal-only root: {symbols}"
    );
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": top_uri },
                "position": { "line": 2, "character": 10 }
            }),
        )
        .expect("hover request during fatal state");
    assert!(
        hover.is_null(),
        "hover must stay feature-less on a fatal-only root: {hover}"
    );
    client.shutdown();
}

/// Fixing the broken file through a watched-file event upgrades the
/// analysis. While the syntax error stands, no complete semantic model exists,
/// so only DECLARATION-LEVEL data serves (the module name comes from the
/// parse-tree fallback); after the fix the analysis becomes strictly valid —
/// the error publication disappears and the recovered module keeps its
/// workspace-index entry with full elaborated data.
#[test]
fn lsp_stdio_fixing_broken_file_upgrades_analysis_to_valid() {
    let fixture = FixtureTree::new();
    let root = fixture.root("project-wide");
    let broken = root.join("broken.sv");
    let original = fs::read_to_string(&broken).expect("read broken source");
    let fixed = original.replace("  assign broken_signal = ;\n", "");
    assert_ne!(fixed, original, "fixture must contain the broken assign");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("project-wide", &root)], default_init_options())
        .expect("initialize project-wide workspace");
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // While the parse error stands the project still serves declaration-level
    // data: the parse-tree fallback surfaces the module name.
    let before = wait_for_workspace_symbols(&mut client, "ProjectWideBroken", |result| {
        names(result).iter().any(|name| name == "ProjectWideBroken")
    });
    assert_has_name(&before, "ProjectWideBroken");

    // Fix the broken file ON DISK and notify through the watched-file event.
    fs::write(&broken, &fixed).expect("fix broken source on disk");
    client
        .send_watch_event(&broken, 2)
        .expect("watched change for fixed source");
    let refreshed = wait_for_diagnostics(&mut client, &broken_uri, |params| {
        !params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("Syntax error"))
                })
            })
    });
    assert!(
        !has_severity_1(&refreshed),
        "fixed file must lose its error: {refreshed}"
    );

    // The re-analysis is valid now: the recovered module joins the index.
    let symbols = wait_for_workspace_symbols(&mut client, "ProjectWideBroken", |result| {
        names(result).iter().any(|name| name == "ProjectWideBroken")
    });
    assert_has_name(&symbols, "ProjectWideBroken");
    client.shutdown();
}

#[test]
fn lsp_stdio_shared_external_file_aggregates_labeled_findings() {
    // Two roots share one external include directory with different compile
    // defines and lint severities.  The shared file is analyzed under both
    // configurations; its aggregated publication shows identical findings
    // exactly once and conflicting findings labeled per root.
    let fixture = FixtureTree::new();
    let ext = fixture.root("agg-ext");
    fs::create_dir_all(&ext).expect("create shared external dir");
    let header = ext.join("agg_hdr.svh");
    // `AGG_FLAG_NAME` is defined differently per root, so the flagged signal
    // declaration occupies the SAME physical line under both configurations
    // while its name (and therefore the lint message) differs.
    let header_text = concat!(
        "// llg-lsp-fixture: agg-ext/agg_hdr.svh\n",
        "`ifndef AGG_HDR_SVH\n",
        "`define AGG_HDR_SVH\n",
        "module AggShared;\n",
        "  logic [3:0] agg_narrow;\n",
        "  logic [7:0] agg_wide;\n",
        "  assign agg_wide = agg_narrow;\n",
        "  logic unused_shared_signal;\n",
        "  logic `AGG_FLAG_NAME ;\n",
        "endmodule\n",
        "`endif\n",
    );
    fs::write(&header, header_text).expect("write shared header");

    let mut roots = Vec::new();
    for (name, define) in [
        ("agg-a", "AGG_FLAG_NAME=agg_flag_a"),
        ("agg-b", "AGG_FLAG_NAME=agg_flag_b"),
    ] {
        let root = fixture.root(name);
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create source dir");
        // agg-a promotes width-mismatch to a warning; agg-b keeps the Info
        // default.  Different defines rename the flag signal declared at the
        // same location in the shared header.
        let width_override = if name == "agg-a" {
            "[lint.rules.width-mismatch]\nseverity = \"warning\"\n"
        } else {
            ""
        };
        fs::write(
            root.join(CONFIG_FILE),
            format!(
                "schema_version = 1\n\
                 [sources]\n\
                 directories = [\".\"]\n\
                 include = [\"**/*.v\", \"**/*.sv\"]\n\
                 [compile]\n\
                 include_dirs = [\"../agg-ext\"]\n\
                 defines = [\"{define}\"]\n\
                 [lint]\n\
                 enabled = true\n\
                 {width_override}"
            ),
        )
        .expect("write aggregation config");
        let top = src.join("top.sv");
        let stem = name.replace('-', "_");
        let top_text = format!(
            "{SOURCE_HEADER} {name}/src/top.sv\n\
             `include \"../../agg-ext/agg_hdr.svh\"\n\
             module AggUser_{stem};\n\
             endmodule\n"
        );
        fs::write(&top, &top_text).expect("write aggregation top");
        roots.push((name, root, top));
    }

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("agg-a", &roots[0].1), ("agg-b", &roots[1].1)],
            default_init_options(),
        )
        .expect("initialize aggregation workspace");
    for (_, _, top) in &roots {
        client
            .open(top, &fs::read_to_string(top).expect("read top"))
            .expect("open aggregation top");
    }
    // Wait until both roots resolved the shared header as a dependency and
    // committed a valid analysis.
    for (_, _, top) in &roots {
        wait_for_diagnostics(&mut client, &file_uri(top), has_no_severity_1);
    }
    wait_for_workspace_symbols(&mut client, "AggUser_", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "AggUser_agg_a")
            && symbol_names.iter().any(|name| name == "AggUser_agg_b")
    });

    // The union for the never-opened shared file is published by the
    // aggregation slice once both roots have committed.  Ownership ties
    // resolve deterministically to the lowest normalized root path (agg-a),
    // so its copies stay unlabeled while agg-b's differing copies carry
    // [agg-b].
    let header_uri = file_uri(&header);
    let aggregated = client
        .wait_for_notification_where("textDocument/publishDiagnostics", |params| {
            params.get("uri").and_then(Value::as_str) == Some(header_uri.as_str())
                && params
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .is_some_and(|diagnostics| {
                        !diagnostics.is_empty()
                            && diagnostics.iter().any(|diagnostic| {
                                diagnostic
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .is_some_and(|message| message.contains("[agg-b]"))
                            })
                    })
        })
        .expect("aggregated publication for the shared file");
    let aggregated = aggregated.get("params").cloned().unwrap_or(Value::Null);

    let diagnostics = aggregated
        .get("diagnostics")
        .and_then(Value::as_array)
        .expect("aggregated diagnostics array");
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.get("message").and_then(Value::as_str))
        .collect();

    // Each distinct rule finding from both roots appears once, unlabeled.
    for expected in [
        "unused variable 'unused_shared_signal'",
        "signal `unused_shared_signal` in `AggShared` is never used",
    ] {
        assert_eq!(
            messages
                .iter()
                .filter(|message| **message == expected)
                .count(),
            1,
            "identical {expected:?} findings must be deduplicated: {messages:?}"
        );
    }

    // The same-location finding differs per root (define-renamed signal):
    // the owner copy stays unlabeled, the non-owner copy carries [agg-b].
    assert!(
        messages
            .iter()
            .any(|message| message.contains("agg_flag_a") && !message.contains('[')),
        "owner copy must stay unlabeled: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("agg_flag_b") && message.ends_with("[agg-b]")),
        "non-owner copy must carry the documented [root-name] label: {messages:?}"
    );

    // The width-mismatch pair fires identically in both roots except for the
    // per-root severity override: both copies survive at the same location.
    let width_findings = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.get("code").and_then(Value::as_str) == Some("width-mismatch")
        })
        .count();
    assert_eq!(
        width_findings, 2,
        "differing-severity findings must both survive with labels: {messages:?}"
    );
    client.shutdown();
}
