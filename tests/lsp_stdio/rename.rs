//! Rename.

use super::*;

// ── textDocument/prepareRename + textDocument/rename ────────────────────────
//
// Rename reuses the find-references machinery, so the acceptance surface is:
// the edit set equals the reference set (declaration included), each edit
// replaces ONLY the identifier span, and non-renamable positions (keywords,
// instance names) answer null instead of an edit.  These tests use
// self-contained generated workspaces so the shared fixture tree stays
// untouched.

/// Owns a generated rename-workspace base directory, removed on drop.
pub(super) struct TempDirCleanup(pub(super) PathBuf);

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Create a single-root workspace under a fresh temp base with the given
/// source files; returns (cleanup guard, workspace dir).
pub(super) fn rename_workspace(dir_name: &str, files: &[(&str, &str)]) -> (TempDirCleanup, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-rename-{}-{dir_name}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(&ws).expect("create rename workspace");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = false\n",
    )
    .expect("write rename workspace llg.toml");
    for (file, text) in files {
        fs::write(ws.join(file), text).expect("write rename workspace source");
    }
    (TempDirCleanup(base), ws)
}

/// The `newText`-annotated edit list one URI contributes to a WorkspaceEdit.
pub(super) fn edits_for(result: &Value, uri: &str) -> Vec<(Value, Value, String)> {
    let edits = result
        .get("changes")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("workspace edit must carry changes: {result}"))
        .get(uri)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no edits for {uri}: {result}"));
    edits
        .iter()
        .map(|edit| {
            let range = edit.get("range").cloned().expect("edit range");
            let start = range.get("start").cloned().expect("range start");
            let end = range.get("end").cloned().expect("range end");
            let new_text = edit
                .get("newText")
                .and_then(Value::as_str)
                .expect("edit newText")
                .to_owned();
            (start, end, new_text)
        })
        .collect()
}

pub(super) fn start_key(start: &Value) -> (u64, u64) {
    (
        start.get("line").and_then(Value::as_u64).expect("line"),
        start
            .get("character")
            .and_then(Value::as_u64)
            .expect("character"),
    )
}

/// Scenario (1): renaming a net referenced across two files produces edits in
/// BOTH uris — the child module's port declaration + its in-module use in one
/// file, and the `.clk` named-connection label in the instantiating file.
#[test]
fn lsp_stdio_rename_net_across_files() {
    let child_text = "module ren_child(input logic clk, output logic q);\n\
                      \x20 assign q = clk;\n\
                      endmodule\n";
    let top_text = "module ren_top;\n\
                    \x20 logic wa;\n\
                    \x20 logic t_q;\n\n\
                    \x20 ren_child u0(.clk(wa), .q(t_q));\n\n\
                    \x20 always #5 wa = ~wa;\n\
                    endmodule\n";
    let (_cleanup, ws) = rename_workspace(
        "cross-file",
        &[("ren_child.sv", child_text), ("ren_top.sv", top_text)],
    );
    let child_path = ws.join("ren_child.sv");
    let top_path = ws.join("ren_top.sv");
    let child_uri = file_uri(&child_path);
    let top_uri = file_uri(&top_path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize rename workspace");
    client.open(&top_path, top_text).expect("open ren_top");
    wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);

    // prepareRename at the in-module use of `clk` (`assign q = clk;`).
    let clk_use = position_at(child_text, "= clk", 2);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": child_uri },
                "position": position_at(child_text, "= clk", 4)
            }),
        )
        .expect("prepareRename at clk use");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("clk"),
        "prepareRename must offer the current name: {prepared}"
    );
    assert_eq!(
        prepared.get("range").and_then(|range| range.get("start")),
        Some(&clk_use),
        "range start must be the identifier start: {prepared}"
    );

    // The rename itself: declaration + use in ren_child.sv, label in ren_top.sv.
    let new_name = "clk_in";
    let result = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": child_uri },
                "position": position_at(child_text, "= clk", 4),
                "newName": new_name
            }),
        )
        .expect("rename request for clk");
    assert_no_shadow_uris(&result);
    let changes = result
        .get("changes")
        .and_then(Value::as_object)
        .expect("workspace edit changes");
    let uris: Vec<&String> = changes.keys().collect();
    assert_eq!(
        uris.len(),
        2,
        "edits must span exactly both files: {changes:?}"
    );

    // ren_child.sv: port declaration + in-module use, 3 chars each.
    let child_edits = edits_for(&result, &child_uri);
    let expected_decl = position_at(child_text, "input logic clk", 12);
    assert_eq!(child_edits.len(), 2, "child edits: {child_edits:?}");
    assert_eq!(start_key(&child_edits[0].0), start_key(&expected_decl));
    assert_eq!(start_key(&child_edits[1].0), start_key(&clk_use));
    for (start, end, text) in &child_edits {
        assert_eq!(text, new_name);
        assert_eq!(
            end.get("character").and_then(Value::as_u64),
            Some(start.get("character").and_then(Value::as_u64).expect("c") + 3),
            "only the `clk` identifier may be replaced: {start}..{end}"
        );
    }

    // ren_top.sv: exactly the `.clk` connection-label occurrence.
    let top_edits = edits_for(&result, &top_uri);
    let expected_label = position_at(top_text, ".clk", 1);
    assert_eq!(top_edits.len(), 1, "top edits: {top_edits:?}");
    assert_eq!(start_key(&top_edits[0].0), start_key(&expected_label));
    assert_eq!(top_edits[0].2, new_name);
    client.shutdown();
}

/// Scenario (2): prepareRename answers a range+placeholder on a parameter and
/// NULL on a keyword.
#[test]
fn lsp_stdio_prepare_rename_parameter_and_keyword() {
    let text = "module param_mod #(parameter W = 8)(\n\
                \x20 input logic [W-1:0] d,\n\
                \x20 output logic [7:0] q\n\
                );\n\
                \x20 assign q = d;\n\
                endmodule\n\
                \n\
                module tb_params;\n\
                \x20 logic [7:0] d;\n\
                \x20 logic [7:0] q;\n\
                \x20 param_mod #(.W(8)) u0(.d(d), .q(q));\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("param", &[("param_mod.sv", text)]);
    let path = ws.join("param_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize parameter rename workspace");
    client.open(&path, text).expect("open param_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Parameter declaration: placeholder W, single-character range.
    let w_decl = position_at(text, "parameter W = 8", 10);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": w_decl
            }),
        )
        .expect("prepareRename on parameter");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("W")
    );
    assert_eq!(
        prepared.get("range").and_then(|range| range.get("start")),
        Some(&w_decl)
    );

    // Keyword position (`module`): not renamable → null.
    let keyword = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(text, "module param_mod", 2)
            }),
        )
        .expect("prepareRename on keyword");
    assert!(
        keyword.is_null(),
        "a keyword must not be renamable: {keyword}"
    );
    client.shutdown();
}

/// Scenario (3): an illegal new name is rejected with an invalidParams error;
/// nothing renames silently.
#[test]
fn lsp_stdio_rename_rejects_invalid_names() {
    let text = "module inv_mod;\n\
                \x20 logic sig1;\n\
                \x20 assign sig1 = 1'b0;\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("invalid-name", &[("inv_mod.sv", text)]);
    let path = ws.join("inv_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize invalid-name workspace");
    client.open(&path, text).expect("open inv_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let sig_decl = position_at(text, "logic sig1", 6);
    for bad in ["1abc", "has space", "a-b", "module"] {
        let error = client
            .request(
                "textDocument/rename",
                json!({
                    "textDocument": { "uri": uri },
                    "position": sig_decl,
                    "newName": bad
                }),
            )
            .err()
            .unwrap_or_else(|| panic!("`{bad}` must be rejected"));
        assert!(
            error.contains("-32602") && error.contains(bad),
            "invalid name must yield invalid_params naming the offender: {error}"
        );
    }

    // A valid rename still works afterwards.
    let ok = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": sig_decl,
                "newName": "sig2"
            }),
        )
        .expect("valid rename after rejections");
    let edits = edits_for(&ok, &uri);
    assert_eq!(edits.len(), 2, "decl + use: {edits:?}");
    client.shutdown();
}

/// Scenario (4): prefix-collision safety — renaming `data` never touches
/// `data_out`, even though every occurrence shares a prefix and lives in the
/// same module.
#[test]
fn lsp_stdio_rename_is_prefix_collision_safe() {
    let text = "module data_mod;\n\
                \x20 logic data;\n\
                \x20 logic data_out;\n\n\
                \x20 assign data_out = ~data;\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("prefix", &[("data_mod.sv", text)]);
    let path = ws.join("data_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize prefix workspace");
    client.open(&path, text).expect("open data_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let data_decl = position_at(text, "logic data;", 6);
    let data_use = position_at(text, "~data;", 1);
    let result = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": data_decl,
                "newName": "din"
            }),
        )
        .expect("rename request for data");
    let edits = edits_for(&result, &uri);
    assert_eq!(
        edits.len(),
        2,
        "exactly the two `data` occurrences may be edited: {edits:?}"
    );
    assert_eq!(start_key(&edits[0].0), start_key(&data_decl));
    assert_eq!(start_key(&edits[1].0), start_key(&data_use));
    for (start, end, new_text) in &edits {
        assert_eq!(new_text, "din");
        assert_eq!(
            end.get("character").and_then(Value::as_u64),
            Some(start.get("character").and_then(Value::as_u64).expect("c") + 4),
            "only the 4-char `data` identifier may be replaced: {start}..{end}"
        );
    }

    // The same request from the `data_out` side stays on its own family.
    let data_out_decl = position_at(text, "logic data_out;", 6);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": data_out_decl
            }),
        )
        .expect("prepareRename on data_out");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("data_out"),
        "{prepared}"
    );
    client.shutdown();
}
