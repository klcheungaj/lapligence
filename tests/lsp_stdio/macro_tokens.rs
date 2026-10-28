//! Semantic tokens around text macros, in both serving paths.
//
// Contract:
// * the project stream (an unopened file) and the isolated open-buffer parse
//   see the same macros: in-source `` `define``s, the headers the project
//   compile admitted (through `[compile] include_dirs`) and `[compile]
//   defines`. The server compiles every file as its own compilation unit, so
//   a macro defined only in another project file is unknown in both paths.
// * a macro use site `` `NAME `` is one `keyword` token; no token of the
//   expansion is painted over it. Text written in the actual arguments keeps
//   its own kind at its written position, also through nested macros.
// * directives are `keyword` (conditional ones with `controlFlow`); inactive
//   `ifdef` branches carry no token.
// * an unknown macro is a Slang diagnostic and the rest of the buffer is
//   still tokenized (a syntax error degrades the stream per token).

use super::*;

const MACRO_TOKENS_TOML: &str = "\
schema_version = 1

[sources]
directories = [\".\"]
include = [\"**/*.v\", \"**/*.sv\"]

[lint]
enabled = false

[compile]
include_dirs = [\"inc\"]
defines = [\"CFG_WIDTH=4\"]
";

const MACRO_TOKENS_HEADER: &str = "\
`define HDR_ADD(a, b) ((a) + (b))
`define HDR_ONE 1
";

const MACRO_TOKENS_OTHER: &str = "\
`define OTHER_W 3
module macro_other_unit;
endmodule
";

const MACRO_TOKENS_TOP: &str = "\
`include \"defs.svh\"
`define LOCAL_W 8
`define SUM(x, y) ((x) + (y))
`define NESTED(z) `SUM(z, `LOCAL_W)
module macro_top(input logic [`LOCAL_W-1:0] a, output logic [7:0] y);
  localparam int CW = `CFG_WIDTH;
  localparam int HW = `HDR_ONE;
  logic [7:0] s1, s2, s3;
  assign s1 = `SUM(a, 1);
  assign s2 = `NESTED(a);
  assign s3 = `HDR_ADD(a, CW) + HW;
`ifdef CFG_WIDTH
  assign y = s1 ^ s2 ^ s3;
`else
  assign y = broken (;
`endif
`ifdef NOT_DEFINED
  wire inactive_net;
`endif
endmodule
";

const MACRO_TOKENS_USER: &str = "\
module macro_user;
  localparam int OW = `OTHER_W;
  logic after_unknown;
  assign after_unknown = 1'b0;
endmodule
";

struct MacroTokensWorkspace {
    _guard: TempDirCleanup,
    ws: PathBuf,
    top: PathBuf,
    user: PathBuf,
}

fn macro_tokens_workspace(tag: &str) -> MacroTokensWorkspace {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-macro-tokens-{}-{tag}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(ws.join("inc")).expect("create macro-token workspace");
    fs::write(ws.join(CONFIG_FILE), MACRO_TOKENS_TOML).expect("write llg.toml");
    fs::write(ws.join("inc").join("defs.svh"), MACRO_TOKENS_HEADER).expect("write header");
    fs::write(ws.join("a_other.sv"), MACRO_TOKENS_OTHER).expect("write other unit");
    let top = ws.join("macro_top.sv");
    fs::write(&top, MACRO_TOKENS_TOP).expect("write macro_top.sv");
    let user = ws.join("macro_user.sv");
    fs::write(&user, MACRO_TOKENS_USER).expect("write macro_user.sv");
    MacroTokensWorkspace {
        _guard: TempDirCleanup(base),
        ws,
        top,
        user,
    }
}

fn token_rows(
    client: &mut LspProcess,
    path: &Path,
    types: &[String],
    modifiers: &[String],
) -> Vec<SemanticRow> {
    let result = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(path) } }),
        )
        .expect("semantic token response");
    semantic_token_rows(&result, types, modifiers)
}

/// The type and modifiers of the token at `needle` plus `offset` columns,
/// asserting it is the only token starting there.
fn sole_token(
    rows: &[SemanticRow],
    source: &str,
    needle: &str,
    occurrence: usize,
    offset: u64,
) -> (String, Vec<String>) {
    let (line, character) = position_of(source, needle, occurrence);
    let at: Vec<_> = rows
        .iter()
        .filter(|row| row.line == line && row.character == character + offset)
        .collect();
    assert_eq!(at.len(), 1, "{needle}+{offset}: {rows:?}");
    (at[0].token_type.clone(), at[0].modifiers.clone())
}

fn no_token_on_line(rows: &[SemanticRow], source: &str, needle: &str) {
    let (line, _) = position_of(source, needle, 0);
    assert!(
        rows.iter().all(|row| row.line != line),
        "line of {needle} has tokens: {rows:?}"
    );
}

/// Use sites of in-source, header and configured macros are `keyword`;
/// arguments written in `` `SUM(a, 1)``, a nested `` `NESTED(a)`` and a header
/// macro keep their own kinds; directives are keywords and inactive branches
/// carry no token. The project stream and the isolated open-buffer parse
/// agree token for token.
#[test]
fn lsp_stdio_macro_use_sites_and_arguments_match_in_both_serving_paths() {
    let workspace = macro_tokens_workspace("use-sites");
    let source = MACRO_TOKENS_TOP;
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("macro-tokens", &workspace.ws)], default_init_options())
        .expect("initialize macro-token workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &file_uri(&workspace.top), has_no_severity_1);

    let mut streams = Vec::new();
    for opened in [false, true] {
        if opened {
            client
                .open(&workspace.top, source)
                .expect("open macro_top.sv");
        }
        let rows = token_rows(&mut client, &workspace.top, &types, &modifiers);
        let token = |needle: &str, occurrence: usize, offset: u64| {
            sole_token(&rows, source, needle, occurrence, offset)
        };
        let macro_type = ("keyword".to_owned(), Vec::<String>::new());
        for (needle, occurrence) in [
            ("`LOCAL_W-1", 0),
            ("`CFG_WIDTH;", 0),
            ("`HDR_ONE;", 0),
            ("`SUM(a, 1)", 0),
            ("`NESTED(a)", 0),
            ("`HDR_ADD(a, CW)", 0),
            // A macro usage kept unexpanded inside a macro body.
            ("`SUM(z", 0),
            ("`LOCAL_W)", 0),
        ] {
            assert_eq!(
                token(needle, occurrence, 0),
                macro_type,
                "opened={opened} {needle}"
            );
        }
        let port = ("parameter".to_owned(), vec!["readonly".to_owned()]);
        assert_eq!(token("`SUM(a, 1)", 0, 5), port, "opened={opened}");
        assert_eq!(
            token("`SUM(a, 1)", 0, 8),
            ("number".to_owned(), Vec::new()),
            "opened={opened}"
        );
        assert_eq!(token("`NESTED(a)", 0, 8), port, "opened={opened}");
        assert_eq!(token("`HDR_ADD(a, CW)", 0, 9), port, "opened={opened}");
        assert_eq!(
            token("`HDR_ADD(a, CW)", 0, 12),
            ("variable".to_owned(), vec!["readonly".to_owned()]),
            "opened={opened}"
        );
        assert_eq!(
            token("HW;", 0, 0),
            ("variable".to_owned(), vec!["readonly".to_owned()]),
            "opened={opened}"
        );
        let keyword = |bits: &[&str]| {
            (
                "keyword".to_owned(),
                bits.iter().map(|bit| (*bit).to_owned()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            token("`include", 0, 0),
            keyword(&["controlFlow"]),
            "opened={opened}"
        );
        assert_eq!(
            token("`define", 0, 0),
            keyword(&["controlFlow"]),
            "opened={opened}"
        );
        assert_eq!(
            token("`ifdef CFG", 0, 0),
            keyword(&["controlFlow"]),
            "opened={opened}"
        );
        assert_eq!(token("y = s1", 0, 0), port, "opened={opened}");
        no_token_on_line(&rows, source, "broken (");
        no_token_on_line(&rows, source, "inactive_net");
        let lines = source.lines().count() as u64;
        assert!(
            rows.iter().all(|row| row.line < lines),
            "opened={opened}: header tokens leaked: {rows:?}"
        );
        streams.push(
            rows.iter()
                .map(|row| {
                    (
                        row.line,
                        row.character,
                        row.token_type.clone(),
                        row.modifiers.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        streams[0], streams[1],
        "project and isolated streams differ"
    );
    client.shutdown();
}

/// A macro defined only in another project file is unknown (separate
/// compilation units): the use site keeps its `keyword` token, Slang reports the
/// unknown macro, and the rest of the buffer is still tokenized in both
/// serving paths.
#[test]
fn lsp_stdio_unknown_macro_keeps_the_rest_of_the_buffer_in_both_serving_paths() {
    let workspace = macro_tokens_workspace("unknown");
    let source = MACRO_TOKENS_USER;
    let uri = file_uri(&workspace.user);
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("macro-tokens", &workspace.ws)], default_init_options())
        .expect("initialize macro-token workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    let diagnostics = wait_for_diagnostics(&mut client, &uri, has_severity_1);
    let unknown = diagnostics["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("unknown macro"))
        })
        .unwrap_or_else(|| panic!("no unknown-macro diagnostic: {diagnostics}"));
    let (line, character) = position_of(source, "`OTHER_W", 0);
    assert_eq!(
        (
            unknown["range"]["start"]["line"].as_u64(),
            unknown["range"]["start"]["character"].as_u64()
        ),
        (Some(line), Some(character)),
        "{unknown}"
    );

    for opened in [false, true] {
        if opened {
            client
                .open(&workspace.user, source)
                .expect("open macro_user.sv");
        }
        let rows = token_rows(&mut client, &workspace.user, &types, &modifiers);
        assert_eq!(
            sole_token(&rows, source, "`OTHER_W", 0, 0),
            ("keyword".to_owned(), Vec::new()),
            "opened={opened}"
        );
        assert_eq!(
            sole_token(&rows, source, "macro_user", 0, 0),
            ("class".to_owned(), vec!["declaration".to_owned()]),
            "opened={opened}"
        );
        assert_eq!(
            sole_token(&rows, source, "after_unknown;", 0, 0),
            ("variable".to_owned(), vec!["declaration".to_owned()]),
            "opened={opened}"
        );
        assert_eq!(
            sole_token(&rows, source, "after_unknown =", 0, 0),
            ("variable".to_owned(), Vec::new()),
            "opened={opened}"
        );
        assert_eq!(
            sole_token(&rows, source, "endmodule", 0, 0),
            ("keyword".to_owned(), Vec::new()),
            "opened={opened}"
        );
    }
    client.shutdown();
}

/// An unsaved buffer is preprocessed with the include context of the last
/// committed analysis: a macro added by the edit and a header macro both
/// expand before any project re-analysis of the edit is committed.
#[test]
fn lsp_stdio_unsaved_macro_edits_use_the_committed_include_context() {
    let workspace = macro_tokens_workspace("unsaved");
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("macro-tokens", &workspace.ws)], default_init_options())
        .expect("initialize macro-token workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &file_uri(&workspace.top), has_no_severity_1);
    client
        .open(&workspace.top, MACRO_TOKENS_TOP)
        .expect("open macro_top.sv");

    // The edit adds a macro and a use of it below the header macro use.
    let edited = MACRO_TOKENS_TOP.replace(
        "  logic [7:0] s1, s2, s3;\n",
        "  logic [7:0] s1, s2, s3;\n`define FRESH(v) (v)\n  wire [7:0] fresh = `FRESH(s1) + `HDR_ONE;\n",
    );
    client
        .change(&workspace.top, 2, &edited)
        .expect("edit macro_top.sv");
    let rows = token_rows(&mut client, &workspace.top, &types, &modifiers);
    assert_eq!(
        sole_token(&rows, &edited, "`FRESH(s1)", 0, 0),
        ("keyword".to_owned(), Vec::new())
    );
    assert_eq!(
        sole_token(&rows, &edited, "`FRESH(s1)", 0, 7),
        ("variable".to_owned(), Vec::new())
    );
    assert_eq!(
        sole_token(&rows, &edited, "`HDR_ONE;", 1, 0),
        ("keyword".to_owned(), Vec::new())
    );
    assert_eq!(
        sole_token(&rows, &edited, "fresh =", 0, 0),
        ("variable".to_owned(), vec!["declaration".to_owned()])
    );
    client.shutdown();
}
