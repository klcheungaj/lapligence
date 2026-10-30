//! Same-named definitions in several project files, in both serving paths.
//
// Contract (`docs/config.md#lsp-duplicate-definitions`):
// * two files that both declare `module dup_top` (variants of one design)
//   each navigate within their own body: definition, hover, references and
//   rename of a name the body declares stay in that file, also for uses
//   written before the declaration (an error the frontend reports);
// * the frontend elaborates one of the definitions and warns on the other
//   one's name (`DuplicateDefinition`); both bodies keep their declaration
//   symbols and semantic tokens;
// * the project stream (an unopened file) and the isolated open-buffer parse
//   agree token for token, with one or both files open.

use super::*;

const FIRST: &str = "\
module dup_top (
  input  logic clk,
  output logic ok,
  output logic bad
);
assign ok = ok_r;
assign bad = bad_a || bad_b;

logic ok_r;
logic bad_a;
logic bad_b;

dup_leaf u_leaf (
  .clk(clk),
  .ok(ok_r),
  .flag(bad_a)
);
assign bad_b = !ok_r;
endmodule

module dup_leaf (input logic clk, output logic ok, output logic flag);
  logic st;
  always_ff @(posedge clk) st <= !st;
  assign ok = st;
  assign flag = 1'b0;
endmodule
";

// The second variant shifts every line and changes some expressions.
const SECOND: &str = "\
// variant with a registered error flag
module dup_top (
  input  logic clk,
  output logic ok,
  output logic bad
);
assign ok = ok_r;
assign bad = bad_a || bad_b;
logic ok_r;
logic bad_a;
logic bad_b;
dup_leaf u_leaf (
  .clk(clk),
  .ok(ok_r),
  .flag(bad_a)
);
always_ff @(posedge clk) bad_b <= ok_r;
endmodule

module dup_leaf (input logic clk, output logic ok, output logic flag);
  logic st;
  always_ff @(posedge clk) st <= !st;
  assign ok = st;
  assign flag = st;
endmodule
";

/// Names the `dup_top` bodies declare; every occurrence is in that body.
const NAMES: &[&str] = &["ok_r", "bad_a", "bad_b"];

struct DupWorkspace {
    _guard: TempDirCleanup,
    ws: PathBuf,
    files: [(PathBuf, &'static str); 2],
}

fn dup_workspace(tag: &str) -> DupWorkspace {
    let (guard, ws) = rename_workspace(
        &format!("dup-def-{tag}"),
        &[("dup_first.sv", FIRST), ("dup_second.sv", SECOND)],
    );
    DupWorkspace {
        files: [
            (ws.join("dup_first.sv"), FIRST),
            (ws.join("dup_second.sv"), SECOND),
        ],
        _guard: guard,
        ws,
    }
}

fn start(position: (u64, u64)) -> Value {
    json!({ "line": position.0, "character": position.1 })
}

fn locations(value: &Value) -> Vec<(String, (u64, u64))> {
    let mut out: Vec<_> = value
        .as_array()
        .unwrap_or_else(|| panic!("location list expected: {value}"))
        .iter()
        .map(|location| {
            (
                location["uri"].as_str().expect("location uri").to_owned(),
                start_key(&location["range"]["start"]),
            )
        })
        .collect();
    out.sort();
    out
}

fn rows_for(
    client: &mut LspProcess,
    path: &Path,
    types: &[String],
    modifiers: &[String],
) -> Vec<(u64, u64, String, Vec<String>)> {
    let result = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(path) } }),
        )
        .expect("semantic token response");
    semantic_token_rows(&result, types, modifiers)
        .into_iter()
        .map(|row| (row.line, row.character, row.token_type, row.modifiers))
        .collect()
}

/// Definition, hover, references and rename of every occurrence of every
/// name in `path` stay within `path` and reach its own declaration.
fn check_navigation(client: &mut LspProcess, path: &Path, source: &str, what: &str) {
    let uri = file_uri(path);
    for name in NAMES {
        let declaration = {
            let (line, character) = position_of(source, &format!("logic {name};"), 0);
            (line, character + 6)
        };
        let occurrences = word_positions(source, name);
        let expected: Vec<_> = occurrences
            .iter()
            .map(|position| (uri.clone(), *position))
            .collect();
        for &position in &occurrences {
            let at = format!("{what} {name}@{position:?}");
            let params = json!({ "textDocument": { "uri": uri }, "position": start(position) });
            let definition = client
                .request("textDocument/definition", params.clone())
                .expect("definition response");
            assert_eq!(
                single_location(&definition, &at),
                (uri.clone(), start(declaration)),
                "{at}: definition"
            );
            let hover = client
                .request("textDocument/hover", params.clone())
                .expect("hover response");
            assert!(
                hover.to_string().contains(name),
                "{at}: hover must name {name}: {hover}"
            );
            let mut with_declaration = params.clone();
            with_declaration["context"] = json!({ "includeDeclaration": true });
            let references = client
                .request("textDocument/references", with_declaration)
                .expect("references response");
            assert_eq!(locations(&references), expected, "{at}: references");
        }
        let rename = client
            .request(
                "textDocument/rename",
                json!({
                    "textDocument": { "uri": uri },
                    "position": start(declaration),
                    "newName": "renamed_sig"
                }),
            )
            .expect("rename response");
        let changes = rename["changes"]
            .as_object()
            .unwrap_or_else(|| panic!("{what} {name}: rename changes: {rename}"));
        assert_eq!(
            changes.keys().collect::<Vec<_>>(),
            [&uri],
            "{what} {name}: rename touches only its own file"
        );
        let mut edited: Vec<_> = edits_for(&rename, &uri)
            .iter()
            .map(|(start, _, _)| start_key(start))
            .collect();
        edited.sort();
        assert_eq!(edited, occurrences, "{what} {name}: rename edits");
    }
    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        )
        .expect("document symbols");
    for name in NAMES {
        let (line, character) = position_of(source, &format!("logic {name};"), 0);
        assert!(
            symbol_starts(&symbols, name).contains(&(line, character + 6)),
            "{what}: symbol {name} must sit on its declaration: {symbols}"
        );
    }
}

/// `selectionRange` starts of every document symbol called `name`.
fn symbol_starts(value: &Value, name: &str) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if item["name"].as_str() == Some(name) {
                out.push(start_key(&item["selectionRange"]["start"]));
            }
            out.extend(symbol_starts(&item["children"], name));
        }
    }
    out
}

fn is_duplicate_definition(diagnostic: &Value) -> bool {
    diagnostic["code"]
        .as_str()
        .is_some_and(|code| code.ends_with(".DuplicateDefinition"))
}

/// Each file's `dup_top` body navigates within itself, in the project
/// stream and with one or both files open, and the two token streams agree.
#[test]
fn lsp_stdio_same_named_definitions_navigate_within_their_own_file() {
    let workspace = dup_workspace("navigate");
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("dup-def", &workspace.ws)], default_init_options())
        .expect("initialize duplicate-definition workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");

    // The frontend warns on the name of one of the two definitions.
    let uris: Vec<String> = workspace
        .files
        .iter()
        .map(|(path, _)| file_uri(path))
        .collect();
    let warned = client
        .wait_for_notification_where("textDocument/publishDiagnostics", |params| {
            params["uri"]
                .as_str()
                .is_some_and(|uri| uris.iter().any(|known| known == uri))
                && params["diagnostics"]
                    .as_array()
                    .is_some_and(|all| all.iter().any(is_duplicate_definition))
        })
        .expect("duplicate-definition warning");
    let warned = &warned["params"];
    let (path, source) = workspace
        .files
        .iter()
        .find(|(path, _)| warned["uri"].as_str() == Some(file_uri(path).as_str()))
        .expect("warned file");
    let duplicate = warned["diagnostics"]
        .as_array()
        .and_then(|all| {
            all.iter().find(|diagnostic| {
                is_duplicate_definition(diagnostic)
                    && diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("'dup_top'"))
            })
        })
        .expect("dup_top duplicate diagnostic");
    let (line, character) = position_of(source, "module dup_top", 0);
    assert_eq!(
        duplicate["range"]["start"],
        start((line, character + 7)),
        "{}: {duplicate}",
        path.display()
    );
    // The related location names the other definition.
    let (other_path, other_source) = workspace
        .files
        .iter()
        .find(|(other, _)| other != path)
        .expect("other file");
    let (line, character) = position_of(other_source, "module dup_top", 0);
    let related = duplicate["relatedInformation"]
        .as_array()
        .unwrap_or_else(|| panic!("related information: {duplicate}"));
    assert!(
        related.iter().any(|entry| {
            entry["location"]["uri"].as_str() == Some(file_uri(other_path).as_str())
                && entry["location"]["range"]["start"] == start((line, character + 7))
        }),
        "{duplicate}"
    );

    let mut project = Vec::new();
    for (path, source) in &workspace.files {
        project.push(rows_for(&mut client, path, &types, &modifiers));
        check_navigation(&mut client, path, source, "project");
    }
    for (index, (path, source)) in workspace.files.iter().enumerate() {
        client
            .open(path, source)
            .expect("open duplicate definition");
        for (other, (path, source)) in workspace.files.iter().enumerate() {
            let what = format!("opened 0..={index}, file {other}");
            let rows = rows_for(&mut client, path, &types, &modifiers);
            assert_eq!(rows, project[other], "{what}: token streams differ");
            check_navigation(&mut client, path, source, &what);
        }
    }

    // A bound use is a variable token in each body.
    for ((_, source), rows) in workspace.files.iter().zip(&project) {
        let (line, character) = position_of(source, ".ok(ok_r)", 0);
        let row = rows
            .iter()
            .find(|row| row.0 == line && row.1 == character + 4)
            .unwrap_or_else(|| panic!("no token on the ok_r actual: {rows:?}"));
        assert_eq!(row.2, "variable", "{row:?}");
    }
    client.shutdown();
}
