//! Requests about documents with unsaved edits: the last analysis answers at
//! the positions that the user sees (`docs/config.md`, "LSP analysis
//! snapshots").

use super::analysis_triggers::Session;
use super::rename::start_key;
use super::*;

const TOP: &str = "\
`define WIDTH 8
module top(input logic clk);
  logic [`WIDTH-1:0] first, second;
  sub u_sub(.clk(clk), .din(first), .dout(second));
  function int f(int v);
    return v;
  endfunction
  function int g(bit v);
    return v;
  endfunction
endmodule
";

const SUB: &str = "\
module sub(input logic clk, input logic [7:0] din, output logic [7:0] dout);
  always_ff @(posedge clk) dout <= din;
endmodule
";

/// [`TOP`] and [`SUB`] with lines above and between the old ones.
fn edited() -> (String, String) {
    let top = TOP
        .replacen("`define", "// A note.\n\n`define", 1)
        .replacen("  sub u_sub", "  logic fresh;\n  sub u_sub", 1);
    (top, format!("// A note.\n{SUB}"))
}

impl Session {
    fn at(&self, file: &str, text: &str, needle: &str, occurrence: usize) -> Value {
        let (line, character) = position_of(text, needle, occurrence);
        json!({
            "textDocument": { "uri": self.uri(file) },
            "position": { "line": line, "character": character }
        })
    }

    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.client
            .request(method, params)
            .unwrap_or_else(|error| panic!("{method}: {error}"))
    }

    /// The hover's text, and where it begins.
    fn hover(
        &mut self,
        file: &str,
        text: &str,
        needle: &str,
        occurrence: usize,
    ) -> (String, Value) {
        let at = self.at(file, text, needle, occurrence);
        let hover = self.ask("textDocument/hover", at);
        let value = hover.pointer("/contents/value").and_then(Value::as_str);
        let value = value.unwrap_or_default().to_owned();
        (value, hover["range"]["start"].clone())
    }

    /// The places in `file` of the references of a name.
    fn references(
        &mut self,
        (file, text): (&str, &str),
        needle: &str,
        occurrence: usize,
        of: &str,
    ) -> Vec<(u64, u64)> {
        let mut params = self.at(file, text, needle, occurrence);
        params["context"] = json!({ "includeDeclaration": true });
        let references = self.ask("textDocument/references", params);
        let uri = self.uri(of);
        let mut places: Vec<(u64, u64)> = references
            .as_array()
            .expect("references")
            .iter()
            .filter(|location| location["uri"] == uri.as_str())
            .map(|location| start_key(&location["range"]["start"]))
            .collect();
        places.sort_unstable();
        places
    }

    fn stale_rename(&mut self, at: &Value) -> bool {
        let mut params = at.clone();
        params["newName"] = json!("renamed");
        match self.client.request("textDocument/rename", params) {
            Ok(_) => false,
            Err(error) => {
                assert!(error.contains("-32801"), "{error}");
                true
            }
        }
    }
}

fn places(text: &str, needles: &[(&str, usize)]) -> Vec<(u64, u64)> {
    let mut places: Vec<(u64, u64)> = needles
        .iter()
        .map(|(needle, occurrence)| position_of(text, needle, *occurrence))
        .collect();
    places.sort_unstable();
    places
}

/// Every symbol below `symbols`, with where its name begins and its range.
fn flatten(symbols: &Value, found: &mut Vec<(String, (u64, u64), Value)>) {
    for symbol in symbols.as_array().into_iter().flatten() {
        found.push((
            symbol["name"].as_str().expect("a name").to_owned(),
            start_key(&symbol["selectionRange"]["start"]),
            symbol["range"].clone(),
        ));
        flatten(&symbol["children"], found);
    }
}

#[test]
fn lsp_stdio_requests_answer_at_the_positions_of_the_unsaved_text() {
    let mut session = Session::ready("unsaved", &[("top.sv", TOP), ("sub.sv", SUB)]);
    session.open("top.sv", TOP);
    session.open("sub.sv", SUB);
    let before = session.hover("top.sv", TOP, "first", 0).0;
    assert!(before.contains("logic[7:0] first"), "{before}");
    let (top, sub) = edited();
    assert_eq!(session.change("top.sv", &top), "admitted");
    assert_eq!(session.change("sub.sv", &sub), "admitted");

    // Hover.
    for (needle, occurrence) in [("first", 0), ("first", 1)] {
        let (line, character) = position_of(&top, needle, occurrence);
        assert_eq!(
            session.hover("top.sv", &top, needle, occurrence),
            (
                before.clone(),
                json!({ "line": line, "character": character })
            )
        );
    }
    let macro_hover = session.hover("top.sv", &top, "WIDTH-1", 0).0;
    assert!(
        macro_hover.contains("macro WIDTH = 8") && macro_hover.contains("top.sv:3"),
        "{macro_hover}"
    );
    let instance = session.hover("top.sv", &top, "u_sub", 0).0;
    assert!(instance.contains("sub"), "{instance}");
    // Text that no analysis has read.
    assert_eq!(
        session.hover("top.sv", &top, "fresh", 0),
        (String::new(), Value::Null)
    );

    // References within a document, to others and from others.
    assert_eq!(
        session.references(("top.sv", &top), "first", 0, "top.sv"),
        places(&top, &[("first", 0), ("first)", 0)])
    );
    let din = [("din,", 0), ("din;", 0)];
    assert_eq!(
        session.references(("top.sv", &top), "din(", 0, "sub.sv"),
        places(&sub, &din)
    );
    assert_eq!(
        session.references(("sub.sv", &sub), "din;", 0, "top.sv"),
        places(&top, &[("din(", 0)])
    );

    // Definitions in another edited document.
    let params = session.at("top.sv", &top, "sub u_sub", 0);
    let definition = session.ask("textDocument/definition", params);
    let (line, character) = position_of(&sub, "sub(", 0);
    assert_eq!(definition["uri"], session.uri("sub.sv").as_str());
    assert_eq!(
        definition["range"]["start"],
        json!({ "line": line, "character": character })
    );

    // Symbols.
    let document = session.document("top.sv");
    let answer = session.ask("textDocument/documentSymbol", document);
    let mut symbols = Vec::new();
    flatten(&answer, &mut symbols);
    let module = symbols
        .iter()
        .find(|symbol| symbol.0 == "top")
        .expect("top");
    assert_eq!(module.1, position_of(&top, "top(", 0));
    assert_eq!(start_key(&module.2["start"]), module.1, "{answer}");
    for (name, needle) in [
        ("first", "first,"),
        ("second", "second;"),
        ("u_sub", "u_sub"),
    ] {
        let found = symbols.iter().find(|symbol| symbol.0 == name);
        let found = found.unwrap_or_else(|| panic!("{name} in {answer}"));
        assert_eq!(found.1, position_of(&top, needle, 0), "{name}");
    }
    let answer = session.ask("workspace/symbol", json!({ "query": "u_sub" }));
    assert_eq!(
        start_key(&answer[0]["location"]["range"]["start"]),
        position_of(&top, "u_sub", 0),
        "{answer}"
    );

    // The module explorer and the token dump.
    let explorer = session.ask("llg/moduleExplorer", json!({}));
    let module = explorer["modules"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|module| module["name"] == "sub")
        .expect("sub");
    assert_eq!(
        module["range"]["startLine"],
        position_of(&sub, "sub(", 0).0,
        "{module}"
    );
    let port = &module["ports"][1];
    assert_eq!(
        (
            port["name"].as_str(),
            port["location"]["range"]["startLine"].as_u64()
        ),
        (Some("din"), Some(position_of(&sub, "din,", 0).0)),
        "{module}"
    );
    let dump = session.tokens("top.sv");
    let (line, character) = position_of(&top, "din(", 0);
    let target = position_of(&sub, "din,", 0);
    let row = format!("top.sv:{line}:{character}-");
    let row = dump
        .iter()
        .find(|found| found.starts_with(&row))
        .unwrap_or_else(|| panic!("{row} in {dump:#?}"));
    assert!(
        row.contains(&format!("bind=sub.sv:{}:{}[din,", target.0, target.1)),
        "{row}"
    );

    // Completion offers what the unsaved text declares.
    let document = session.document("top.sv");
    session.ask("textDocument/semanticTokens/full", document);
    let params = session.at("top.sv", &top, "  sub u_sub", 0);
    let labels = session.ask("textDocument/completion", params);
    let offers = |label: &str| {
        labels
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["label"] == label))
    };
    assert!(offers("fresh") && offers("first"), "{labels}");

    // A rename would edit text that no analysis has read.
    let at = session.at("top.sv", &top, "first", 0);
    assert!(session.stale_rename(&at));
    session.write("top.sv", &top);
    session.write("sub.sv", &sub);
    session.save("sub.sv");
    session.save("top.sv");
    let deadline = Instant::now() + POLL_TIMEOUT;
    while session.stale_rename(&at) {
        assert!(Instant::now() < deadline, "no analysis after the save");
        thread::sleep(Duration::from_millis(20));
    }
    assert_ne!(
        session.hover("top.sv", &top, "fresh", 0),
        (String::new(), Value::Null)
    );
    session.client.shutdown();
}

/// Equal names at equal positions are not the same declaration.
#[test]
fn lsp_stdio_moved_text_is_not_answered_for_what_was_there() {
    let mut session = Session::ready("unsaved-moves", &[("top.sv", TOP), ("sub.sv", SUB)]);
    session.open("top.sv", TOP);
    // The lines of `f` are where those of `g` were.
    let moved = TOP.replacen("  function int f", "\n\n\n  function int f", 1);
    session.change("top.sv", &moved);
    assert_eq!(
        position_of(&moved, "return v", 0),
        position_of(TOP, "return v", 1)
    );
    let hover = session.hover("top.sv", &moved, "v;", 0).0;
    assert!(hover.contains("int v"), "{hover}");
    let hover = session.hover("top.sv", &moved, "v;", 1).0;
    assert!(hover.contains("bit v"), "{hover}");
    assert_eq!(
        session.references(("top.sv", &moved), "v);", 0, "top.sv"),
        places(&moved, &[("v);", 0), ("v;", 0)])
    );

    // An edited name has no facts, and neither has one that lost its place.
    let renamed = moved.replacen("(int v)", "(int value)", 1);
    session.change("top.sv", &renamed);
    assert_eq!(
        session.hover("top.sv", &renamed, "value", 0),
        (String::new(), Value::Null)
    );
    assert_eq!(
        session.references(("top.sv", &renamed), "v;", 0, "top.sv"),
        places(&renamed, &[("v;", 0)])
    );
    session.wait_logged(
        &["event=positions.dropped request=textDocument/references"],
        1,
    );

    // Undone, every place is the analysed one again.
    session.change("top.sv", TOP);
    assert_eq!(
        session.references(("top.sv", TOP), "v;", 1, "top.sv"),
        places(TOP, &[("v);", 1), ("v;", 1)])
    );
    let at = session.at("top.sv", TOP, "first", 0);
    assert!(!session.stale_rename(&at));
    session.client.shutdown();
}

/// Cached answers have analysed positions: an edit above the name leaves them.
#[test]
fn lsp_stdio_cached_answers_survive_an_edit() {
    let mut session = Session::ready("unsaved-cache", &[("top.sv", TOP), ("sub.sv", SUB)]);
    session.open("top.sv", TOP);
    let hits = |session: &mut Session| {
        session
            .logged(&["name=textDocument/hover ", "outcome=cache-hit"])
            .len()
    };
    let before = session.hover("top.sv", TOP, "first", 1).0;
    assert_eq!(hits(&mut session), 0);
    let (top, _) = edited();
    session.change("top.sv", &top);
    assert_eq!(session.hover("top.sv", &top, "first", 1).0, before);
    session.wait_logged(&["name=textDocument/hover ", "outcome=cache-hit"], 1);
    assert_eq!(hits(&mut session), 1);
    session.client.shutdown();
}

/// An edit stops the isolated parse of the text it replaced.
#[test]
fn lsp_stdio_an_edit_cancels_the_parse_of_the_older_text() {
    let text = super::analysis_triggers::slow_module();
    let mut session = Session::ready("unsaved-parse", &[("top.sv", "module top; endmodule\n")]);
    session.open("top.sv", "module top; endmodule\n");
    let cancelled = "event=semantic_tokens.open_parse.end outcome=cancelled";
    for attempt in 1..=20 {
        let parses = session
            .logged(&["event=semantic_tokens.parse_lock.end"])
            .len();
        session.change("top.sv", &format!("{text}// {attempt}\n"));
        let document = session.document("top.sv");
        session
            .client
            .send_request("textDocument/semanticTokens/full", document)
            .expect("semantic tokens");
        session.wait_logged(&["event=semantic_tokens.parse_lock.end"], parses + 1);
        session.change("top.sv", "module top;\n  logic last;\nendmodule\n");
        session.wait_logged(&["event=semantic_tokens.open_parse.end"], parses + 1);
        if !session.logged(&[cancelled]).is_empty() {
            break;
        }
        assert!(attempt < 20, "no parse was met halfway");
    }
    let document = session.document("top.sv");
    let tokens = session.ask("textDocument/semanticTokens/full", document);
    let last = position_of("module top;\n  logic last;\nendmodule\n", "last", 0);
    assert!(
        semantic_token_positions(&tokens).contains(&last),
        "{tokens}"
    );
    session.client.shutdown();
}

/// An escaped identifier (IEEE 1800-2009 5.6.1) is ranged from its backslash to
/// the white space after it, also at the places of an unsaved edit.
#[test]
fn lsp_stdio_escaped_identifiers_are_ranged_whole() {
    const ESCAPED: &str = "\
module \\esc!m (input logic \\a+b , output logic q);
  assign q = \\a+b ;
endmodule
module top;
  logic \\a+b , q;
  \\esc!m  u (.\\a+b (\\a+b ), .q(q));
endmodule
";
    let mut session = Session::ready("unsaved-escaped", &[("top.sv", ESCAPED)]);
    session.open("top.sv", ESCAPED);
    for text in [ESCAPED.to_owned(), format!("// A note.\n\n{ESCAPED}")] {
        session.change("top.sv", &text);
        let lines: Vec<&str> = text.lines().collect();
        let spelled = |range: &Value| -> String {
            let line = range["start"]["line"].as_u64().expect("line") as usize;
            let start = range["start"]["character"].as_u64().expect("start") as usize;
            let end = range["end"]["character"].as_u64().expect("end") as usize;
            assert_eq!(range["end"]["line"], range["start"]["line"], "{range}");
            lines[line][start..end].to_owned()
        };
        let mut params = session.at("top.sv", &text, "a+b ;", 0);
        params["context"] = json!({ "includeDeclaration": true });
        let references = session.ask("textDocument/references", params);
        let found: Vec<String> = references
            .as_array()
            .expect("references")
            .iter()
            .map(|location| spelled(&location["range"]))
            .collect();
        assert_eq!(found, ["\\a+b", "\\a+b", "\\a+b"], "{references}");
        let at = session.at("top.sv", &text, "esc!m  u", 0);
        let definition = session.ask("textDocument/definition", at);
        assert_eq!(spelled(&definition["range"]), "\\esc!m", "{definition}");
        let symbols = session.ask("textDocument/documentSymbol", session.document("top.sv"));
        let module = symbols
            .as_array()
            .expect("symbols")
            .iter()
            .find(|symbol| symbol["name"] == "esc!m")
            .expect("the escaped module");
        assert_eq!(spelled(&module["selectionRange"]), "\\esc!m", "{module}");
        let explorer = session.ask("llg/moduleExplorer", json!({}));
        let explored = |range: &Value| {
            spelled(&json!({
                "start": { "line": range["startLine"], "character": range["startCharacter"] },
                "end": { "line": range["endLine"], "character": range["endCharacter"] },
            }))
        };
        let instance = explorer["roots"]
            .as_array()
            .expect("roots")
            .iter()
            .flat_map(|root| root["children"].as_array().into_iter().flatten())
            .find(|child| child["instanceName"] == "u")
            .expect("the instance");
        assert_eq!(explored(&instance["range"]), "\\esc!m", "{instance}");
        let module = explorer["modules"]
            .as_array()
            .expect("modules")
            .iter()
            .find(|module| module["name"] == "esc!m")
            .expect("the escaped module");
        assert_eq!(explored(&module["range"]), "\\esc!m", "{module}");
    }
    session.client.shutdown();
}

/// An edit moves the places that the module explorer shows: the client is told
/// once a burst of edits pauses, and the explorer then answers at the new
/// places.
#[test]
fn lsp_stdio_edits_tell_explorer_clients_once_they_pause() {
    let mut session = Session::ready("unsaved-explorer", &[("top.sv", TOP), ("sub.sv", SUB)]);
    session.open("top.sv", TOP);
    let moved = "event=module_explorer.moved";
    assert!(session.logged(&[moved]).is_empty());
    for lines in 1..=3 {
        let text = format!("{}{TOP}", "// A note.\n".repeat(lines));
        assert_eq!(session.change("top.sv", &text), "admitted");
    }
    session.wait_logged(&[moved], 1);
    // Longer than the server's delay: no other notice follows.
    thread::sleep(Duration::from_millis(900));
    assert_eq!(session.logged(&[moved]).len(), 1);
    let query = json!({ "query": "no such symbol" });
    session.ask("workspace/symbol", query);
    let notices = session
        .client
        .notifications
        .iter()
        .filter(|message| message["method"] == "llg/moduleExplorerChanged")
        .count();
    assert!(notices >= 1, "no llg/moduleExplorerChanged");
    let explorer = session.ask("llg/moduleExplorer", json!({}));
    let top = explorer["modules"]
        .as_array()
        .expect("modules")
        .iter()
        .find(|module| module["name"] == "top")
        .expect("top");
    assert_eq!(top["range"]["startLine"], json!(4), "{top}");
    session.client.shutdown();
}

/// The outline of an edited document lists the design units that only its
/// unsaved text declares, at their places, and no declaration whose listing
/// depends on a scope that the isolated parse does not tell.
#[test]
fn lsp_stdio_the_outline_lists_unsaved_design_units() {
    let mut session = Session::ready("unsaved-outline", &[("top.sv", TOP), ("sub.sv", SUB)]);
    session.open("top.sv", TOP);
    let text = format!("// A note.\n{TOP}package fresh_pkg;\n  int fresh_var;\nendpackage\n");
    assert_eq!(session.change("top.sv", &text), "admitted");
    let answer = session.ask("textDocument/documentSymbol", session.document("top.sv"));
    let mut symbols = Vec::new();
    flatten(&answer, &mut symbols);
    let found = symbols.iter().find(|symbol| symbol.0 == "fresh_pkg");
    let found = found.unwrap_or_else(|| panic!("fresh_pkg in {answer}"));
    assert_eq!(found.1, position_of(&text, "fresh_pkg", 0), "{answer}");
    let top = symbols
        .iter()
        .find(|symbol| symbol.0 == "top")
        .expect("top");
    assert_eq!(top.1, position_of(&text, "top(", 0), "{answer}");
    assert!(
        symbols.iter().all(|symbol| symbol.0 != "fresh_var"),
        "{answer}"
    );
    session.client.shutdown();
}
