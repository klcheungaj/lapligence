//! Unknown modules: one diagnostic, a black-box instance, continued analysis.

use super::*;

const LEAF: &str = concat!(
    "// llg-lsp-fixture: generated/black-box/leaf.sv\n",
    "module leaf(input logic a, output logic y);\n",
    "  assign y = a;\n",
    "endmodule\n",
);

// The unknown `ghost` instance sits between two known siblings, and a
// continuous assignment reads through it: Slang replaces that assignment with
// an invalid expression, which used to fail the whole owned import.
const MID: &str = concat!(
    "// llg-lsp-fixture: generated/black-box/mid.sv\n",
    "module mid(input logic i, output logic o, output logic probe);\n",
    "  logic t0, t1;\n",
    "  leaf u_before(.a(i), .y(t0));\n",
    "  ghost #(.W(4)) u_ghost(.x(t0), .y(t1));\n",
    "  leaf u_after(.a(t1), .y(o));\n",
    "  assign probe = u_ghost.sig;\n",
    "endmodule\n",
);

const TOP: &str = concat!(
    "// llg-lsp-fixture: generated/black-box/top.sv\n",
    "module top(input logic clk, output logic o, output logic probe);\n",
    "  mid u_mid(.i(clk), .o(o), .probe(probe));\n",
    "endmodule\n",
);

fn black_box_workspace(label: &str) -> (TempDirCleanup, PathBuf) {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "llg-lsp-black-box-{label}-{}-{id}",
        std::process::id()
    ));
    let root = base.join("workspace");
    fs::create_dir_all(&root).expect("create black-box workspace");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n[sources]\ndirectories = [\".\"]\n",
    )
    .expect("write black-box config");
    (TempDirCleanup(base), root)
}

fn diagnostics_of(params: &Value) -> Vec<Value> {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn is_unknown_module(diagnostic: &Value) -> bool {
    diagnostic
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| code.ends_with(".UnknownModule"))
}

fn request_at(client: &mut LspProcess, method: &str, uri: &str, at: (u64, u64)) -> Value {
    client
        .request(
            method,
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": at.0, "character": at.1 }
            }),
        )
        .unwrap_or_else(|error| panic!("{method} at {at:?} failed: {error}"))
}

fn definition_start(client: &mut LspProcess, uri: &str, at: (u64, u64)) -> (String, Value) {
    let response = request_at(client, "textDocument/definition", uri, at);
    single_location(&response, "definition")
}

fn hover_text(client: &mut LspProcess, uri: &str, at: (u64, u64)) -> String {
    request_at(client, "textDocument/hover", uri, at)
        .pointer("/contents/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn find_named<'a>(nodes: &'a [Value], key: &str, name: &str) -> &'a Value {
    nodes
        .iter()
        .find(|node| node.get(key).and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("no {key}={name} in {nodes:?}"))
}

/// A multi-file workspace where `mid` instantiates a module defined nowhere.
/// The server publishes exactly one error on the module-name range, keeps the
/// elaborated hierarchy above, beside and below the unknown instance, serves
/// navigation through it and shows it in the explorer as a declaration-only
/// child with no definition.
#[test]
fn lsp_stdio_unknown_module_is_a_black_box_and_analysis_continues() {
    let (_cleanup, root) = black_box_workspace("hierarchy");
    let leaf_path = root.join("leaf.sv");
    let mid_path = root.join("mid.sv");
    let top_path = root.join("top.sv");
    fs::write(&leaf_path, LEAF).expect("write leaf.sv");
    fs::write(&mid_path, MID).expect("write mid.sv");
    fs::write(&top_path, TOP).expect("write top.sv");
    let (leaf_uri, mid_uri) = (file_uri(&leaf_path), file_uri(&mid_path));

    let mut client = LspProcess::spawn(&root);
    client
        .initialize(&[("black-box", &root)], default_init_options())
        .expect("initialize black-box workspace");
    client.open(&mid_path, MID).expect("open mid.sv");
    let published = wait_for_diagnostics(&mut client, &mid_uri, |params| {
        diagnostics_of(params).iter().any(is_unknown_module)
    });
    assert_no_shadow_uris(&published);
    let diagnostics = diagnostics_of(&published);
    let errors: Vec<&Value> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "exactly the unknown-module error: {errors:?}"
    );
    let error = errors[0];
    assert!(is_unknown_module(error), "{error}");
    assert_eq!(
        error.get("source").and_then(Value::as_str),
        Some("slang-compiler")
    );
    assert!(error["message"]
        .as_str()
        .is_some_and(|message| message.contains("'ghost'")));
    let (line, character) = position_of(MID, "ghost", 0);
    assert_eq!(
        error["range"]["start"],
        json!({ "line": line, "character": character })
    );
    assert_eq!(
        error["range"]["end"],
        json!({ "line": line, "character": character + "ghost".len() as u64 })
    );
    // The black box can drive or read what it connects; the analysis is not
    // degraded to a database failure.
    for diagnostic in &diagnostics {
        let message = diagnostic["message"].as_str().unwrap_or_default();
        assert!(
            !message.contains("semantic database build failed"),
            "{diagnostic}"
        );
        let lint = diagnostic.get("source").and_then(Value::as_str) == Some("llg-lint");
        assert!(
            !(lint && (message.contains("`t0`") || message.contains("`t1`"))),
            "black-box connections are not undriven/unused: {diagnostic}"
        );
    }

    // Known siblings before and after the unknown instance stay elaborated.
    let label = position_of(MID, ".a(t1)", 0);
    let (uri, start) = definition_start(&mut client, &mid_uri, (label.0, label.1 + 1));
    assert_eq!(uri, leaf_uri, "u_after's label reaches the child port");
    let (line, character) = position_of(LEAF, "a, output", 0);
    assert_eq!(start, json!({ "line": line, "character": character }));
    let after_actual = position_of(MID, "(t1), .y(o)", 0);
    let (uri, start) =
        definition_start(&mut client, &mid_uri, (after_actual.0, after_actual.1 + 1));
    assert_eq!(uri, mid_uri);
    let (line, character) = position_of(MID, "t1;", 0);
    assert_eq!(start, json!({ "line": line, "character": character }));
    // The black box's own connection actuals still bind to the parent scope.
    let ghost_actual = position_of(MID, "(t0), .y(t1)", 0);
    let (uri, start) =
        definition_start(&mut client, &mid_uri, (ghost_actual.0, ghost_actual.1 + 1));
    assert_eq!(uri, mid_uri);
    let (line, character) = position_of(MID, "t0,", 0);
    assert_eq!(start, json!({ "line": line, "character": character }));
    // The instance name is a symbol described by its written module name;
    // the module name and the port labels have no target and do not fail.
    let ghost_instance = position_of(MID, "u_ghost(", 0);
    assert_eq!(
        hover_text(&mut client, &mid_uri, ghost_instance),
        "```systemverilog\nghost u_ghost\n```"
    );
    for at in [
        position_of(MID, "ghost #", 0),
        position_of(MID, ".x(t0)", 0),
        position_of(MID, ".W(4)", 0),
    ] {
        let at = (at.0, at.1 + 1);
        assert!(request_at(&mut client, "textDocument/definition", &mid_uri, at).is_null());
        let _ = request_at(&mut client, "textDocument/hover", &mid_uri, at);
    }

    // Document symbols keep every instance; known siblings carry their type.
    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": mid_uri } }),
        )
        .expect("mid.sv document symbols");
    let children = find_named(symbols.as_array().expect("symbol list"), "name", "mid")["children"]
        .as_array()
        .expect("mid children")
        .clone();
    for name in ["u_before", "u_after"] {
        let child = find_named(&children, "name", name);
        assert_eq!(child["detail"], json!("leaf"), "{child}");
        assert_eq!(child["kind"], json!(19), "{child}");
    }
    assert_eq!(
        find_named(&children, "name", "u_ghost")["detail"],
        json!("ghost")
    );

    // Explorer: the elaborated hierarchy continues through `mid`, and the
    // black box is a declaration-only child with no definition.
    let snapshot = client
        .request("llg/moduleExplorer", json!({}))
        .expect("module explorer request");
    assert_no_shadow_uris(&snapshot);
    let roots = snapshot["roots"]
        .as_array()
        .expect("explorer roots")
        .clone();
    let top = find_named(&roots, "moduleType", "top");
    assert_eq!(top["contentSource"], json!("elaborated"));
    let mid = find_named(
        top["children"].as_array().expect("top children"),
        "instanceName",
        "u_mid",
    );
    assert_eq!(mid["contentSource"], json!("elaborated"));
    let mid_children = mid["children"].as_array().expect("mid children").clone();
    for name in ["u_before", "u_after"] {
        let child = find_named(&mid_children, "instanceName", name);
        assert_eq!(child["contentSource"], json!("elaborated"), "{child}");
        assert_eq!(child["moduleType"], json!("leaf"), "{child}");
    }
    let ghost = find_named(&mid_children, "instanceName", "u_ghost");
    assert_eq!(ghost["moduleType"], json!("ghost"));
    assert_eq!(ghost["contentSource"], json!("declaration"));
    assert!(ghost["definitionId"].is_null(), "{ghost}");
    assert!(ghost["children"]
        .as_array()
        .is_none_or(|children| children.is_empty()));
    client.shutdown();
}

/// Introducing an unknown module into a valid open buffer publishes the fresh
/// analysis instead of keeping the last-good snapshot: code written after the
/// unknown instance is immediately navigable, and removing the instance
/// clears the error.
#[test]
fn lsp_stdio_unknown_module_edit_publishes_fresh_analysis() {
    let (_cleanup, root) = black_box_workspace("fresh");
    let path = root.join("single.sv");
    let valid = concat!(
        "// llg-lsp-fixture: generated/black-box/single.sv\n",
        "module single(input logic clk, output logic o);\n",
        "  assign o = clk;\n",
        "endmodule\n",
    );
    let unknown = concat!(
        "// llg-lsp-fixture: generated/black-box/single.sv\n",
        "module single(input logic clk, output logic o);\n",
        "  missing_mod u_m(.a(clk));\n",
        "  assign o = u_m.sig;\n",
        "  logic fresh;\n",
        "  assign fresh = clk;\n",
        "endmodule\n",
    );
    fs::write(&path, valid).expect("write single.sv");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&root);
    client
        .initialize(&[("black-box-fresh", &root)], default_init_options())
        .expect("initialize fresh-analysis workspace");
    client.open(&path, valid).expect("open single.sv");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    client
        .change(&path, 2, unknown)
        .expect("introduce an unknown module");
    let published = wait_for_diagnostics(&mut client, &uri, |params| {
        diagnostics_of(params).iter().any(is_unknown_module)
    });
    let errors: Vec<Value> = diagnostics_of(&published)
        .into_iter()
        .filter(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    let (line, character) = position_of(unknown, "missing_mod", 0);
    assert_eq!(
        errors[0]["range"]["start"],
        json!({ "line": line, "character": character })
    );
    // `fresh` exists only in the new buffer: its reference resolves only if
    // the new analysis was committed.
    let (uri_found, start) =
        definition_start(&mut client, &uri, position_of(unknown, "fresh = clk", 0));
    assert_eq!(uri_found, uri);
    let (line, character) = position_of(unknown, "fresh;", 0);
    assert_eq!(start, json!({ "line": line, "character": character }));
    let snapshot = client
        .request("llg/moduleExplorer", json!({}))
        .expect("module explorer request");
    let roots = snapshot["roots"]
        .as_array()
        .expect("explorer roots")
        .clone();
    let single = find_named(&roots, "moduleType", "single");
    assert_eq!(single["contentSource"], json!("elaborated"));
    let child = find_named(
        single["children"].as_array().expect("single children"),
        "instanceName",
        "u_m",
    );
    assert_eq!(child["moduleType"], json!("missing_mod"));

    client
        .change(&path, 3, valid)
        .expect("remove the unknown module");
    wait_for_diagnostics(&mut client, &uri, |params| {
        !diagnostics_of(params).iter().any(is_unknown_module)
    });
    client.shutdown();
}
