//! Workspace.

use super::*;

#[test]
fn lsp_stdio_discovers_multi_root_workspace_and_handles_watchers() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let root_c = fixture.root("root-c");
    let mut client = LspProcess::spawn(&fixture.root);

    // No didOpen is sent before this request: discovery must index `.v`/`.sv`
    // compilation units from both workspace folders on disk.  `.vh`/`.svh`
    // files are include-only and must NOT appear as standalone symbols.
    let initialize = client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize multi-root workspace");
    // Specific providers must be present, not just an arbitrary capabilities
    // object (mirrors the Node E2E capability contract).
    let capabilities = initialize
        .get("capabilities")
        .and_then(Value::as_object)
        .expect("initialize response must carry a capabilities object")
        .clone();
    for flag in [
        "hoverProvider",
        "definitionProvider",
        "referencesProvider",
        "documentSymbolProvider",
        "workspaceSymbolProvider",
        "completionProvider",
        "semanticTokensProvider",
    ] {
        assert!(
            capabilities.get(flag).is_some_and(|value| !value.is_null()),
            "capability {flag} must be advertised: {capabilities:?}"
        );
    }
    let trigger_characters = capabilities
        .get("completionProvider")
        .and_then(|provider| provider.get("triggerCharacters"))
        .and_then(Value::as_array)
        .expect("completionProvider.triggerCharacters")
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    for trigger in [".", ":", "`"] {
        assert!(
            trigger_characters
                .iter()
                .any(|candidate| candidate == trigger),
            "completion trigger character `{trigger}` must be advertised: {trigger_characters:?}"
        );
    }
    let legend_token_types = capabilities
        .get("semanticTokensProvider")
        .and_then(|provider| provider.get("legend"))
        .and_then(|legend| legend.get("tokenTypes"))
        .and_then(Value::as_array)
        .expect("semanticTokensProvider.legend.tokenTypes");
    assert!(
        !legend_token_types.is_empty(),
        "semantic token legend must list token types"
    );
    let full_sync = match capabilities.get("textDocumentSync") {
        Some(Value::Number(number)) => number.as_i64() == Some(1),
        Some(Value::Object(object)) => object.get("change").and_then(Value::as_i64) == Some(1),
        _ => false,
    };
    assert!(full_sync, "full text sync must be advertised");
    assert_eq!(
        capabilities
            .get("workspace")
            .and_then(|workspace| workspace.get("workspaceFolders"))
            .and_then(|folders| folders.get("supported"))
            .and_then(Value::as_bool),
        Some(true),
        "workspace folder support must be advertised"
    );

    let registration = client
        .wait_for_server_request("client/registerCapability")
        .expect("server must dynamically register watched-file support");
    assert_eq!(
        registration.get("method").and_then(Value::as_str),
        Some("client/registerCapability")
    );
    let registration_text = registration.to_string();
    assert!(
        registration_text.contains("workspace/didChangeWatchedFiles"),
        "watched-file registration missing: {registration}"
    );
    // Watchers cover the effective config and `.v`/`.sv` units; headers and
    // arbitrary-extension includes are covered only once resolved as deps.
    assert!(
        registration_text.contains("llg.toml"),
        "watch registration does not cover the config file: {registration}"
    );
    assert!(
        registration_text.contains("**/*.v") && registration_text.contains("**/*.sv"),
        "watch registration does not cover .v/.sv units: {registration}"
    );

    let discovery = wait_for_workspace_symbols(&mut client, "Discovery", |result| {
        let names = names(result);
        ["DiscoveryV", "DiscoverySv"]
            .iter()
            .all(|expected| names.iter().any(|name| name == expected))
            && names.iter().all(|name| {
                name != "ExcludedShouldNotAppear" && name != "DiscoveryVh" && name != "DiscoverySvh"
            })
    });
    assert_no_shadow_uris(&discovery);

    let merged = wait_for_workspace_symbols(&mut client, "RootBDiscovery", |result| {
        names(result).iter().any(|name| name == "RootBDiscovery")
    });
    assert_no_shadow_uris(&merged);

    let top_path = root_b.join("ports").join("top.sv");
    let top_text = fs::read_to_string(&top_path).expect("read multiline-port fixture");
    let document_symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": file_uri(&top_path) } }),
        )
        .expect("document symbols request");
    assert_has_name(&document_symbols, "port_top");
    assert_no_shadow_uris(&document_symbols);

    let semantic_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&top_path) } }),
        )
        .expect("semantic tokens request");
    let token_data = semantic_tokens
        .get("data")
        .and_then(Value::as_array)
        .expect("semantic token result data");
    assert!(!token_data.is_empty(), "semantic token result is empty");
    assert_eq!(
        token_data.len() % 5,
        0,
        "semantic token data is not 5-tuples"
    );

    // The `.data` label is deliberately separated from its actual connection
    // expression.  The result must point to the child port in the real source.
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".data", 1)
            }),
        )
        .expect("multiline named-port definition request");
    assert_no_shadow_uris(&definition);
    let definition_uri = definition
        .get("uri")
        .and_then(Value::as_str)
        .expect("definition location URI");
    assert_eq!(
        definition_uri,
        file_uri(&root_b.join("ports").join("child.sv")),
        "named port definition must resolve to the child declaration"
    );

    // Add and remove an independent root through the standard workspace
    // notification.  The existing roots must remain indexed after removal.
    client
        .send_workspace_folder_change(&[("root-c", &root_c)], &[])
        .expect("add workspace folder");
    let added = wait_for_workspace_symbols(&mut client, "FolderAdded", |result| {
        names(result).iter().any(|name| name == "FolderAdded")
    });
    assert_no_shadow_uris(&added);
    client
        .send_workspace_folder_change(&[], &[("root-c", &root_c)])
        .expect("remove workspace folder");
    wait_for_workspace_symbols(&mut client, "FolderAdded", |result| {
        names(result).iter().all(|name| name != "FolderAdded")
    });
    let roots_after_remove = wait_for_workspace_symbols(&mut client, "RootBDiscovery", |result| {
        names(result).iter().any(|name| name == "RootBDiscovery")
    });
    assert_no_shadow_uris(&roots_after_remove);

    // Exercise create/change/delete events without opening the watched file.
    let watched_dir = root_a.join("watched");
    fs::create_dir_all(&watched_dir).expect("create watched fixture directory");
    let watched_path = watched_dir.join("dynamic.sv");
    let created_text =
        format!("{SOURCE_HEADER} root-a/watched/dynamic.sv\nmodule WatchedCreated; endmodule\n");
    fs::write(&watched_path, &created_text).expect("create watched source");
    client
        .send_watch_event(&watched_path, 1)
        .expect("send watched create event");
    wait_for_workspace_symbols(&mut client, "WatchedCreated", |result| {
        names(result).iter().any(|name| name == "WatchedCreated")
    });

    let changed_text =
        format!("{SOURCE_HEADER} root-a/watched/dynamic.sv\nmodule WatchedChanged; endmodule\n");
    fs::write(&watched_path, &changed_text).expect("change watched source");
    client
        .send_watch_event(&watched_path, 2)
        .expect("send watched change event");
    wait_for_workspace_symbols(&mut client, "WatchedChanged", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "WatchedChanged")
            && symbol_names.iter().all(|name| name != "WatchedCreated")
    });

    fs::remove_file(&watched_path).expect("delete watched source");
    client
        .send_watch_event(&watched_path, 3)
        .expect("send watched delete event");
    wait_for_workspace_symbols(&mut client, "WatchedChanged", |result| {
        names(result).iter().all(|name| name != "WatchedChanged")
    });
    client.shutdown();
}

#[test]
fn lsp_stdio_keeps_navigation_snapshot_during_syntax_error_and_recovers() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let invalid = valid.replace(
        "assign snapshot_signal = 1'b0;",
        "assign snapshot_signal = ;",
    );
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize snapshot workspace");

    client.open(&path, &valid).expect("open snapshot source");
    let valid_diagnostics = wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    assert!(!has_lint_rule(&valid_diagnostics, "unused-signal"));

    let symbol_request = json!({ "textDocument": { "uri": uri } });
    let before_symbols = client
        .request("textDocument/documentSymbol", symbol_request.clone())
        .expect("document symbols before syntax error");
    assert_has_name(&before_symbols, "SnapshotTop");
    assert_no_shadow_uris(&before_symbols);

    let before_definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "assign snapshot_signal", "assign ".len())
            }),
        )
        .expect("definition before syntax error");
    assert_no_shadow_uris(&before_definition);
    assert_eq!(
        before_definition.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "baseline navigation must use the real URI"
    );

    let declaration_position = position_at(&valid, "snapshot_signal", 0);
    let references_with_declaration = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": declaration_position,
                "context": { "includeDeclaration": true }
            }),
        )
        .expect("references including declaration");
    let references_without_declaration = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": declaration_position,
                "context": { "includeDeclaration": false }
            }),
        )
        .expect("references excluding declaration");
    assert!(
        has_location_start(&references_with_declaration, &declaration_position),
        "declaration missing when includeDeclaration=true: {references_with_declaration}"
    );
    assert!(
        !has_location_start(&references_without_declaration, &declaration_position),
        "declaration returned when includeDeclaration=false: {references_without_declaration}"
    );
    assert_eq!(
        references_with_declaration.as_array().map(Vec::len),
        references_without_declaration
            .as_array()
            .map(|locations| locations.len() + 1),
        "includeDeclaration should only add the declaration"
    );

    client
        .change(&path, 2, &invalid)
        .expect("introduce syntax error");
    let syntax_diagnostics = wait_for_diagnostics(&mut client, &uri, has_severity_1);
    assert!(
        syntax_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))),
        "syntax error did not publish diagnostics"
    );

    // A failed recompile must not destroy the last good feature snapshot.
    let stale_symbols = client
        .request("textDocument/documentSymbol", symbol_request)
        .expect("document symbols during syntax error");
    assert_has_name(&stale_symbols, "SnapshotTop");
    assert_no_shadow_uris(&stale_symbols);
    let stale_definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "assign snapshot_signal", "assign ".len())
            }),
        )
        .expect("definition during syntax error");
    assert_no_shadow_uris(&stale_definition);
    assert_eq!(
        stale_definition.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "stale navigation must remain mapped to the real URI"
    );

    client
        .change(&path, 3, &valid)
        .expect("recover snapshot source");
    let recovered_diagnostics = wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    assert!(
        recovered_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .all(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) != Some(1))),
        "diagnostics did not clear after recovery"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_rejects_include_escape_and_keeps_last_good_navigation() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize include workspace");
    client.open(&path, &valid).expect("open include source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let relative_escape = valid.replace(
        "module SnapshotTop;",
        "`include \"../../outside.svh\"\nmodule SnapshotTop;",
    );
    client
        .change(&path, 2, &relative_escape)
        .expect("introduce relative include escape");
    let relative_diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("severity").and_then(Value::as_u64) == Some(1)
                        && diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| {
                                message.contains("escapes configured source/include directories")
                            })
                })
            })
    });
    assert!(
        relative_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("../../outside.svh"))
            })),
        "relative include failure was not published: {relative_diagnostics}"
    );
    let stale_symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        )
        .expect("navigation during include failure");
    assert_has_name(&stale_symbols, "SnapshotTop");

    client
        .change(&path, 3, &valid)
        .expect("recover after relative include failure");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let absolute_escape = valid.replace(
        "module SnapshotTop;",
        &format!(
            "`include \"{}\"\nmodule SnapshotTop;",
            fixture.root("outside.svh").display()
        ),
    );
    client
        .change(&path, 4, &absolute_escape)
        .expect("introduce absolute include escape");
    let absolute_diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("severity").and_then(Value::as_u64) == Some(1)
                        && diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| {
                                message.contains("escapes configured source/include directories")
                            })
                })
            })
    });
    assert!(
        absolute_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("outside.svh"))
            })),
        "absolute include failure was not published: {absolute_diagnostics}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_revalidates_nested_root_candidates_after_ownership() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nested = root_a.join("nested");
    fs::create_dir_all(&nested).expect("create nested workspace root");
    let nested_config = nested.join(CONFIG_FILE);
    fs::write(
        &nested_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.sv\", \"**/*.v\"]\n\
         exclude = [\"**/owned.sv\"]\n",
    )
    .expect("write nested workspace config");
    let nested_source = nested.join("owned.sv");
    fs::write(
        &nested_source,
        "// llg-lsp-fixture: root-a/nested/owned.sv\nmodule NestedOwnership; endmodule\n",
    )
    .expect("write nested workspace source");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("nested", &nested)],
            default_init_options(),
        )
        .expect("initialize nested workspace");

    // The nested root structurally owns `owned.sv` (longest prefix) but its
    // discovery excludes it, so it must not be indexed by either root yet.
    wait_for_workspace_symbols(&mut client, "NestedOwnership", |result| {
        names(result).iter().all(|name| name != "NestedOwnership")
    });

    client
        .send_workspace_folder_change(&[], &[("nested", &nested)])
        .expect("remove nested workspace root");
    let parent_owned = wait_for_workspace_symbols(&mut client, "NestedOwnership", |result| {
        names(result).iter().any(|name| name == "NestedOwnership")
    });
    assert_no_shadow_uris(&parent_owned);
    client.shutdown();
}

#[test]
fn lsp_stdio_cross_root_include_under_configured_dir_is_allowed() {
    // The "cannot cross workspace root" blanket rule is removed: an include
    // that stays under a configured source directory is allowed even when it
    // points into another workspace root's tree.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nested = root_a.join("nested");
    let nested_include_dir = nested.join("include");
    fs::create_dir_all(&nested_include_dir).expect("create nested workspace root");
    let nested_config = nested.join(CONFIG_FILE);
    fs::write(
        &nested_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.sv\", \"**/*.v\", \"**/*.svh\"]\n",
    )
    .expect("write nested workspace config");
    let nested_include = nested_include_dir.join("shared_defs.svh");
    fs::write(
        &nested_include,
        "// llg-lsp-fixture: root-a/nested/include/shared_defs.svh\n`define NESTED_SHARED\n",
    )
    .expect("write nested include");
    let outer_dir = root_a.join("outer");
    fs::create_dir_all(&outer_dir).expect("create outer source directory");
    let outer = outer_dir.join("nested_include.sv");
    let outer_text = "// llg-lsp-fixture: root-a/outer/nested_include.sv\n`include \"../nested/include/shared_defs.svh\"\nmodule OuterInclude;\n  logic [NESTED_SHARED-1:0] v;\nendmodule\n";
    fs::write(&outer, outer_text).expect("write outer source");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("nested", &nested)],
            default_init_options(),
        )
        .expect("initialize nested include workspace");
    let outer_uri = file_uri(&outer);
    client.open(&outer, outer_text).expect("open outer source");
    let diagnostics = wait_for_diagnostics(&mut client, &outer_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                !diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("escapes configured"))
                })
            })
    });
    assert!(
        diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| !diagnostics.iter().any(|d| d
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|m| m.contains("escapes configured")))),
        "configured-dir include was rejected: {diagnostics}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_unsaved_top_resolves_unopened_disk_include() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let include_dir = root_a.join("unsaved");
    fs::create_dir_all(&include_dir).expect("create unsaved include directory");
    let top = include_dir.join("top.sv");
    let include = include_dir.join("defs.inc");
    let disk_top = "// llg-lsp-fixture: root-a/unsaved/top.sv\nmodule DiskTop; endmodule\n";
    let open_top = "// llg-lsp-fixture: root-a/unsaved/top.sv\n`include \"defs.inc\"\nmodule UnsavedIncludeTop;\n  logic [HEADER_WIDTH-1:0] value;\nendmodule\n";
    fs::write(&top, disk_top).expect("write on-disk top");
    fs::write(
        &include,
        "// llg-lsp-fixture: root-a/unsaved/defs.inc\n`define HEADER_WIDTH 8\n",
    )
    .expect("write unopened include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize unsaved include workspace");
    let top_uri = file_uri(&top);
    client.open(&top, open_top).expect("open unsaved top");
    let diagnostics = wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);
    assert!(
        diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .all(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) != Some(1))),
        "unopened include caused a compile error: {diagnostics}"
    );
    let symbols = wait_for_workspace_symbols(&mut client, "UnsavedIncludeTop", |result| {
        names(result).iter().any(|name| name == "UnsavedIncludeTop")
            && names(result).iter().all(|name| name != "DiskTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_open_unsaved_inc_overrides_disk_include_for_open_top() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let include_dir = root_a.join("unsaved_inc");
    fs::create_dir_all(&include_dir).expect("create unsaved include directory");
    let top = include_dir.join("top.sv");
    let include = include_dir.join("defs.inc");
    let disk_top = format!(
        "{SOURCE_HEADER} root-a/unsaved_inc/top.sv\n`include \"defs.inc\"\n`ifdef UNSAVED_INCLUDE\nmodule UnsavedIncTop;\n`else\nmodule DiskIncTop;\n`endif\nendmodule\n"
    );
    let open_top = disk_top.clone();
    let disk_include =
        format!("{SOURCE_HEADER} root-a/unsaved_inc/defs.inc\n`define DISK_INCLUDE\n");
    let open_include =
        format!("{SOURCE_HEADER} root-a/unsaved_inc/defs.inc\n`define UNSAVED_INCLUDE\n");
    fs::write(&top, &disk_top).expect("write on-disk include top");
    fs::write(&include, &disk_include).expect("write on-disk include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize unsaved include workspace");
    client.open(&top, &open_top).expect("open include top");
    client
        .open(&include, &open_include)
        .expect("open unsaved include buffer");

    let symbols = wait_for_workspace_symbols(&mut client, "IncTop", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "UnsavedIncTop")
            && symbol_names.iter().all(|name| name != "DiskIncTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_open_unsaved_excluded_svh_overrides_disk_include_for_open_top() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let top_dir = root_a.join("unsaved_excluded");
    let include_dir = root_a.join("excluded");
    fs::create_dir_all(&top_dir).expect("create excluded include top directory");
    fs::create_dir_all(&include_dir).expect("create excluded include directory");
    let top = top_dir.join("top.sv");
    let include = include_dir.join("open_defs.svh");
    let disk_top = format!(
        "{SOURCE_HEADER} root-a/unsaved_excluded/top.sv\n`include \"../excluded/open_defs.svh\"\n`ifdef UNSAVED_EXCLUDED_INCLUDE\nmodule UnsavedExcludedSvhTop;\n`else\nmodule DiskExcludedSvhTop;\n`endif\nendmodule\n"
    );
    let open_top = disk_top.clone();
    let disk_include =
        format!("{SOURCE_HEADER} root-a/excluded/open_defs.svh\n`define DISK_EXCLUDED_INCLUDE\n");
    let open_include = format!(
        "{SOURCE_HEADER} root-a/excluded/open_defs.svh\n`define UNSAVED_EXCLUDED_INCLUDE\n"
    );
    fs::write(&top, &disk_top).expect("write excluded include top");
    fs::write(&include, &disk_include).expect("write excluded include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize excluded include workspace");
    client
        .open(&top, &open_top)
        .expect("open excluded include top");
    client
        .open(&include, &open_include)
        .expect("open unsaved excluded include buffer");

    let symbols = wait_for_workspace_symbols(&mut client, "ExcludedSvhTop", |result| {
        let symbol_names = names(result);
        symbol_names
            .iter()
            .any(|name| name == "UnsavedExcludedSvhTop")
            && symbol_names.iter().all(|name| name != "DiskExcludedSvhTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}
