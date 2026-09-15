//! Dependencies.

use super::*;

#[test]
fn lsp_stdio_arbitrary_extension_include_dep_changes() {
    // A resolved include dependency of arbitrary extension is watched: editing
    // it re-analyzes and the change reaches the published output (the `.mem`
    // file defines a macro consumed by `top.sv`, flipping its lint findings).
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let dir = root_a.join("dyninc");
    fs::create_dir_all(&dir).expect("create include directory");
    let top = dir.join("top.sv");
    let header = dir.join("config.mem");
    let top_text = concat!(
        "// llg-lsp-fixture: root-a/dyninc/top.sv\n",
        "`include \"config.mem\"\n",
        "module DynIncTop;\n",
        "  logic used_dyn_inc;\n",
        "  logic unused_dyn_inc;\n",
        "  assign used_dyn_inc = 1'b1;\n",
        "`ifdef DYN_EXTRA_SIGNAL\n",
        "  logic unused_dyn_extra;\n",
        "`endif\n",
        "endmodule\n",
    );
    fs::write(&top, top_text).expect("write top");
    fs::write(&header, "// llg-lsp-fixture: root-a/dyninc/config.mem\n").expect("write header");

    // Enable unused-signal so the macro flip is observable in diagnostics.
    fs::write(
        root_a.join(CONFIG_FILE),
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
    .expect("write lint-enabled config");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize include-dep workspace");
    let top_uri = file_uri(&top);
    client
        .open(&top, &fs::read_to_string(&top).expect("read top"))
        .expect("open top");

    // Baseline: only the unconditional unused signal is reported.
    wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("unused_dyn_inc"))
                }) && diagnostics.iter().all(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_none_or(|message| !message.contains("unused_dyn_extra"))
                })
            })
    });

    // Change the `.mem` include so it defines a macro consumed by top; the
    // resolved include dependency is watched, so this must flip the output.
    fs::write(
        &header,
        "// llg-lsp-fixture: root-a/dyninc/config.mem\n`define DYN_EXTRA_SIGNAL\n",
    )
    .expect("change include dependency");
    client
        .send_watch_event(&header, 2)
        .expect("send include-dep watch event");
    let flipped = wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("unused_dyn_extra"))
                })
            })
    });
    assert!(
        flipped
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("unused_dyn_extra"))
            })),
        "editing the arbitrary-extension dep did not affect diagnostics: {flipped}"
    );

    let symbols = wait_for_workspace_symbols(&mut client, "DynIncTop", |result| {
        names(result).iter().any(|name| name == "DynIncTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_dep_change_refreshes_every_dependent_root() {
    // A header living under root-b's tree is included by BOTH roots (allowed
    // because it sits under a configured include directory of each root).
    // Editing it must refresh BOTH roots' published diagnostics even though
    // the header classifies as an arbitrary-extension (`Other`) file: the
    // header defines a macro that each top consumes, so the flip is visible
    // in every dependent root's own output.
    let fixture = FixtureTree::new();
    let ext = fixture.root("ext-shared");
    fs::create_dir_all(&ext).expect("create external include dir");
    let header = ext.join("cross_hdr.svh");
    let header_text =
        "// llg-lsp-fixture: ext-shared/cross_hdr.svh\n// shared cross-root macro header\n";
    fs::write(&header, header_text).expect("write cross-root header");

    let mut roots = Vec::new();
    for name in ["cross-a", "cross-b"] {
        let root = fixture.root(name);
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create source dir");
        // The external dir is a configured include directory of both roots,
        // so the include target is authorized for either of them.
        fs::write(
            root.join(CONFIG_FILE),
            "schema_version = 1\n\
                 [sources]\n\
                 directories = [\".\"]\n\
                 include = [\"**/*.v\", \"**/*.sv\"]\n\
                 [compile]\n\
                 include_dirs = [\"../ext-shared\"]\n\
                 [lint]\n\
                 enabled = true\n",
        )
        .expect("write cross-root config");
        let top = src.join("top.sv");
        let stem = name.replace('-', "_");
        let top_text = format!(
            "{SOURCE_HEADER} {name}/src/top.sv\n\
             `include \"../../ext-shared/cross_hdr.svh\"\n\
             module CrossUser_{stem};\n\
               logic used_{stem};\n\
               logic unused_base_{stem};\n\
               assign used_{stem} = 1'b1;\n\
             `ifdef CROSS_EXTRA_UNUSED\n\
               logic unused_extra_{stem};\n\
             `endif\n\
             endmodule\n"
        );
        fs::write(&top, &top_text).expect("write cross-root top");
        roots.push((name, root, top));
    }

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("cross-a", &roots[0].1), ("cross-b", &roots[1].1)],
            default_init_options(),
        )
        .expect("initialize cross-root workspace");

    // Baseline on both roots: only the unconditional unused signal shows up.
    for (_, _, top) in &roots {
        client
            .open(top, &fs::read_to_string(top).expect("read top"))
            .expect("open cross-root top");
        wait_for_diagnostics(&mut client, &file_uri(top), |params| {
            params
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| message.contains("unused_base_"))
                    }) && diagnostics.iter().all(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_none_or(|message| !message.contains("unused_extra_"))
                    })
                })
        });
    }

    // Edit the shared header; the dep is tracked by both roots, so the watch
    // event must refresh BOTH roots' diagnostics.
    fs::write(
        &header,
        format!("{header_text}`define CROSS_EXTRA_UNUSED\n"),
    )
    .expect("edit cross-root header");
    client
        .send_watch_event(&header, 2)
        .expect("send cross-root dep watch event");
    for (name, _, top) in &roots {
        let extra = format!("unused_extra_{}", name.replace('-', "_"));
        let flipped = wait_for_diagnostics(&mut client, &file_uri(top), |params| {
            params
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| message.contains(&extra))
                    })
                })
        });
        assert!(
            flipped
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains(extra.as_str()))
                })),
            "dependent root was not refreshed by the dep change: {flipped}"
        );
    }
    client.shutdown();
}

/// Watchers are re-registered with the SAME registration id once a later
/// valid commit resolves new include dependencies (digest change), so the
/// exact dep path becomes watched (review G).
#[test]
fn lsp_stdio_watchers_reregister_after_dep_resolution_changes_digest() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let dir = root_a.join("dynreg");
    fs::create_dir_all(&dir).expect("create dynamic registration directory");
    let top = dir.join("top.sv");
    let initial_text = format!(
        "{SOURCE_HEADER} root-a/dynreg/top.sv\nmodule DynRegTop;\n  logic used_dyn_reg;\nendmodule\n"
    );
    fs::write(&top, &initial_text).expect("write reregistration top");
    let dep = dir.join("dep.inc");
    fs::write(
        &dep,
        format!("{SOURCE_HEADER} root-a/dynreg/dep.inc\n`define DYN_REG_EXTRA\n"),
    )
    .expect("write reregistration include dep");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize watcher-reregistration workspace");

    let is_watch_registration = |message: &Value| {
        message
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .is_some_and(|registrations| {
                registrations.iter().any(|registration| {
                    registration.get("method").and_then(Value::as_str)
                        == Some("workspace/didChangeWatchedFiles")
                        && registration.get("id").and_then(Value::as_str)
                            == Some("llg-watched-files")
                })
            })
    };
    let watchers_of = |message: &Value| -> Vec<String> {
        message
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|registrations| {
                registrations.iter().find_map(|registration| {
                    registration
                        .get("registerOptions")
                        .and_then(|options| options.get("watchers"))
                        .and_then(Value::as_array)
                        .map(|watchers| {
                            watchers
                                .iter()
                                .filter_map(|watcher| {
                                    watcher.get("globPattern").and_then(Value::as_str)
                                })
                                .map(str::to_owned)
                                .collect::<Vec<_>>()
                        })
                })
            })
            .unwrap_or_default()
    };

    // First registration: covers the effective config and *.v/*.sv globs but
    // not the not-yet-resolved include dependency.
    let first = client
        .wait_for_server_request_where(|message| is_watch_registration(message))
        .expect("first watched-file registration");
    let first_watchers = watchers_of(&first);
    assert!(
        !first_watchers
            .iter()
            .any(|pattern| pattern.ends_with("dep.inc")),
        "precondition: dep must be unwatched before resolution: {first_watchers:?}"
    );

    // A later valid commit resolves the include dependency; the digest
    // changes, forcing a SECOND registration that reuses the same id and now
    // watches the exact dep path.
    let resolved_text = format!(
        "{SOURCE_HEADER} root-a/dynreg/top.sv\n\
         `include \"dep.inc\"\n\
         module DynRegTop;\n\
           logic used_dyn_reg;\n\
         `ifdef DYN_REG_EXTRA\n\
           logic unused_dyn_reg_extra;\n\
         `endif\n\
         endmodule\n"
    );
    client
        .change(&top, 2, &resolved_text)
        .expect("resolve include dependency via didChange");
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    loop {
        let count = client
            .all_server_requests
            .iter()
            .filter(|message| is_watch_registration(message))
            .count();
        if count >= 2 {
            break;
        }
        let message = client
            .receive_until(deadline)
            .expect("second watched-file registration never arrived after dep resolution");
        client.route_unsolicited(message).expect("route message");
    }
    let registrations: Vec<Value> = client
        .all_server_requests
        .iter()
        .filter(|message| is_watch_registration(message))
        .cloned()
        .collect();
    let second = &registrations[1];
    assert_eq!(
        second
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|regs| regs.first())
            .and_then(|registration| registration.get("id"))
            .and_then(Value::as_str),
        first
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|regs| regs.first())
            .and_then(|registration| registration.get("id"))
            .and_then(Value::as_str),
        "re-registration must reuse the SAME registration id"
    );
    let second_watchers = watchers_of(second);
    assert!(
        second_watchers
            .iter()
            .any(|pattern| pattern.ends_with("dep.inc")),
        "re-registered watchers must cover the newly resolved dep: {second_watchers:?}"
    );
    assert_ne!(
        first_watchers, second_watchers,
        "the watcher set must have changed to trigger re-registration"
    );
    client.shutdown();
}
