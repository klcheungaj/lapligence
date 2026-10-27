//! The semantic-token requirements of `docs/semantic_tokens.md`, one check per
//! rule ID (`ST-nn`), over the wire in both serving paths: the committed
//! project stream of an unopened file and the open-buffer stream.

use super::*;

const SOURCE: &str = include_str!("../fixtures/semantic_tokens/requirements.sv");
/// The package `requirements.sv` imports from another project file (ST-28,
/// ST-29).
const PKG_SOURCE: &str = include_str!("../fixtures/semantic_tokens/requirements_pkg.sv");

/// Write the fixture and its package file into `root`; returns the fixture
/// path.
fn write_requirements(root: &Path) -> PathBuf {
    let path = root.join("st_requirements.sv");
    fs::write(&path, SOURCE).expect("write requirements fixture");
    fs::write(root.join("st_requirements_pkg.sv"), PKG_SOURCE).expect("write requirements package");
    path
}

/// Errors and an unresolved name for ST-22 and ST-26, after the clean module.
const ERRORS_SOURCE: &str = "module req_errors(input logic a, output logic b);\n\
  logic kept;\n\
  assign b = a | ghost_name;\n\
  always_comb kept = a;\n\
  broken (\n\
endmodule\n";

/// `(rule, needle, occurrence, offset into the needle, type, modifiers)`.
type Check = (
    &'static str,
    &'static str,
    usize,
    u64,
    &'static str,
    &'static [&'static str],
);

const CONTROL: &[&str] = &["controlFlow"];
const DECL: &[&str] = &["declaration"];
const RO: &[&str] = &["readonly"];
const NONE: &[&str] = &[];

const CHECKS: &[Check] = &[
    // ST-02: every other keyword is a plain `keyword`.
    ("ST-02", "module req_top", 0, 0, "keyword", NONE),
    ("ST-02", "endmodule", 0, 0, "keyword", NONE),
    ("ST-02", "begin : blk", 0, 0, "keyword", NONE),
    ("ST-02", "end\n  final", 0, 0, "keyword", NONE),
    ("ST-02", "function void", 0, 0, "keyword", NONE),
    ("ST-02", "class req_cls", 0, 0, "keyword", NONE),
    ("ST-02", "iff rst", 0, 0, "keyword", NONE),
    ("ST-02", "inside", 0, 0, "keyword", NONE),
    ("ST-02", "property p_req", 0, 0, "keyword", NONE),
    ("ST-02", "posedge clk iff", 0, 0, "keyword", NONE),
    // ST-03: control-flow statement words.
    ("ST-03", "case (q)", 0, 0, "keyword", CONTROL),
    ("ST-03", "endcase", 0, 0, "keyword", CONTROL),
    ("ST-03", "if (q inside", 0, 0, "keyword", CONTROL),
    ("ST-03", "else q <= 3", 0, 0, "keyword", CONTROL),
    ("ST-03", "for (int k", 0, 0, "keyword", CONTROL),
    ("ST-03", "break;", 0, 0, "keyword", CONTROL),
    ("ST-03", "return n", 0, 0, "keyword", CONTROL),
    // ST-04: positional control flow, and the positions that stay plain.
    ("ST-04", "unique case", 0, 0, "keyword", CONTROL),
    ("ST-04", "default: q", 0, 0, "keyword", CONTROL),
    ("ST-04", "disable blk", 0, 0, "keyword", CONTROL),
    ("ST-04", "disable iff", 0, 0, "keyword", NONE),
    // ST-05: process and continuous-assignment keywords.
    ("ST-05", "always_ff", 0, 0, "keyword", CONTROL),
    ("ST-05", "initial begin", 0, 0, "keyword", CONTROL),
    ("ST-05", "final $display", 0, 0, "keyword", CONTROL),
    ("ST-05", "assign y", 0, 0, "keyword", CONTROL),
    ("ST-05", "assign w", 0, 0, "keyword", CONTROL),
    ("ST-05", "alias w1", 0, 0, "keyword", CONTROL),
    ("ST-05", "force v", 0, 0, "keyword", CONTROL),
    ("ST-05", "release v", 0, 0, "keyword", CONTROL),
    ("ST-05", "always v", 0, 0, "keyword", NONE),
    // ST-06: every compiler directive, conditional or not.
    ("ST-06", "`define REQ_W", 0, 0, "keyword", CONTROL),
    ("ST-06", "`define REQ_ADD", 0, 0, "keyword", CONTROL),
    ("ST-06", "`ifdef", 0, 0, "keyword", CONTROL),
    ("ST-06", "`endif", 0, 0, "keyword", CONTROL),
    ("ST-06", "`timescale", 0, 0, "keyword", CONTROL),
    ("ST-06", "`default_nettype", 0, 0, "keyword", CONTROL),
    // ST-07, ST-08, ST-09: grouped keyword modifiers.
    (
        "ST-07",
        "typedef struct packed",
        0,
        0,
        "keyword",
        &["typeDefinition"],
    ),
    (
        "ST-07",
        "struct packed",
        0,
        0,
        "keyword",
        &["typeDefinition"],
    ),
    (
        "ST-07",
        "union packed",
        0,
        0,
        "keyword",
        &["typeDefinition"],
    ),
    ("ST-07", "enum logic", 0, 0, "keyword", &["typeDefinition"]),
    (
        "ST-08",
        "input logic clk, input logic rst",
        0,
        0,
        "keyword",
        &["direction"],
    ),
    ("ST-08", "output logic y", 0, 0, "keyword", &["direction"]),
    ("ST-08", "ref int", 0, 0, "keyword", &["direction"]),
    ("ST-09", "logic [3:0] lo", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "wire w1", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "int q", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "string msg", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "parameter int W", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "localparam", 0, 0, "keyword", &["primitive"]),
    ("ST-09", "automatic int", 0, 0, "keyword", &["primitive"]),
    // ST-10: design units.
    ("ST-10", "req_top(", 0, 0, "class", DECL),
    ("ST-10", "req_child #(.W", 0, 0, "class", NONE),
    ("ST-10", "req_cls;", 0, 0, "class", DECL),
    ("ST-10", "req_if;", 0, 0, "interface", DECL),
    // ST-11: packages, qualifiers and blocks.
    ("ST-11", "req_pkg;", 0, 0, "namespace", DECL),
    ("ST-11", "req_pkg::*", 0, 0, "namespace", NONE),
    ("ST-11", "req_pkg::pv", 0, 0, "namespace", NONE),
    ("ST-11", "blk\n", 0, 0, "namespace", DECL),
    ("ST-11", "blk;", 0, 0, "namespace", NONE),
    // ST-12: typedef names.
    ("ST-12", "pair_t;", 0, 0, "type", DECL),
    ("ST-12", "pair_t pv", 0, 0, "type", NONE),
    ("ST-12", "s_t s;", 0, 0, "type", NONE),
    // ST-13: enum items.
    (
        "ST-13",
        "IDLE,",
        0,
        0,
        "enumMember",
        &["declaration", "readonly"],
    ),
    ("ST-13", "IDLE;", 0, 0, "enumMember", RO),
    // ST-14, ST-15: value and type parameters.
    (
        "ST-14",
        "W = 1",
        0,
        0,
        "variable",
        &["declaration", "readonly"],
    ),
    (
        "ST-14",
        "L = `REQ_W",
        0,
        0,
        "variable",
        &["declaration", "readonly"],
    ),
    ("ST-14", "#(L)", 0, 2, "variable", RO),
    ("ST-15", "T = logic", 0, 0, "typeParameter", DECL),
    // ST-16: ports.
    (
        "ST-16",
        "clk, output",
        0,
        0,
        "parameter",
        &["declaration", "readonly"],
    ),
    ("ST-16", "y = clk", 0, 0, "parameter", RO),
    // ST-17: variables, nets, instances.
    ("ST-17", "s;", 0, 0, "variable", DECL),
    ("ST-17", "w1, w2", 0, 0, "variable", DECL),
    ("ST-17", "u0 (", 0, 0, "variable", DECL),
    ("ST-17", "s.a", 0, 0, "variable", NONE),
    ("ST-17", "pv.lo", 0, 0, "variable", NONE),
    ("ST-17", "n + n", 0, 0, "variable", NONE),
    // ST-18: struct and union members and class properties.
    ("ST-18", "lo;", 0, 0, "property", DECL),
    ("ST-18", "hi;", 0, 0, "property", DECL),
    ("ST-18", "a; struct", 0, 0, "property", DECL),
    ("ST-18", "x; } inner", 0, 0, "property", DECL),
    ("ST-18", "inner; }", 0, 0, "property", DECL),
    ("ST-18", "u0; logic", 0, 0, "property", DECL),
    ("ST-18", "u1; }", 0, 0, "property", DECL),
    ("ST-18", "prop; function", 0, 0, "property", DECL),
    ("ST-18", "prop = 1", 0, 0, "property", NONE),
    ("ST-18", "s.a", 0, 2, "property", NONE),
    ("ST-18", "s.inner.x", 0, 2, "property", NONE),
    ("ST-18", "s.inner.x", 0, 8, "property", NONE),
    ("ST-18", "'{a:", 0, 2, "property", NONE),
    ("ST-18", "inner: '{x", 0, 0, "property", NONE),
    ("ST-18", "'{x: 3", 0, 2, "property", NONE),
    ("ST-18", "pv.lo", 0, 3, "property", NONE),
    ("ST-18", "un.u1", 0, 3, "property", NONE),
    // ST-19: subroutines.
    ("ST-19", "twice(ref", 0, 0, "function", DECL),
    ("ST-19", "twice(r)", 0, 0, "function", NONE),
    ("ST-19", "set();", 0, 0, "function", DECL),
    ("ST-19", "$display", 0, 0, "function", &["defaultLibrary"]),
    // ST-20: connection labels against their actuals.
    (
        "ST-20",
        ".W(L)",
        0,
        1,
        "function",
        &["readonly", "connectionLabel"],
    ),
    ("ST-20", ".W(L)", 0, 3, "variable", RO),
    (
        "ST-20",
        ".T(logic",
        0,
        1,
        "function",
        &["readonly", "connectionLabel"],
    ),
    ("ST-20", ".clk(v)", 0, 1, "function", &["connectionLabel"]),
    ("ST-20", ".clk(v)", 0, 5, "variable", NONE),
    ("ST-20", ".y, .b", 0, 1, "function", &["connectionLabel"]),
    ("ST-20", ".b(bi)", 0, 1, "function", &["connectionLabel"]),
    ("ST-20", ".b(bi)", 0, 3, "variable", NONE),
    ("ST-20", "(v, z, bi)", 0, 1, "variable", NONE),
    // ST-21: macros.
    ("ST-21", "REQ_W 4", 0, 0, "macro", DECL),
    ("ST-21", "REQ_ADD(a, b)", 0, 0, "macro", DECL),
    ("ST-21", "`REQ_W;", 0, 0, "macro", NONE),
    ("ST-21", "`REQ_ADD(v", 0, 0, "macro", NONE),
    ("ST-21", "`REQ_ADD(v, rst)", 0, 9, "variable", NONE),
    ("ST-21", "`REQ_ADD(v, rst)", 0, 12, "parameter", RO),
    // ST-23: literals and operators.
    ("ST-23", "\"hi\"", 0, 0, "string", NONE),
    ("ST-23", "4'h1", 0, 0, "number", NONE),
    ("ST-23", "-> ev", 0, 0, "operator", NONE),
    // ST-28: import item names are references to the package and member.
    ("ST-28", "req_ext_pkg::EXT_L;", 0, 0, "namespace", NONE),
    ("ST-28", "req_ext_pkg::EXT_L;", 0, 13, "variable", RO),
    ("ST-28", "req_ext_pkg::ext_e;", 0, 13, "type", NONE),
    ("ST-28", "req_ext_pkg::EXT_ON;", 0, 13, "enumMember", RO),
    ("ST-28", "req_ext_pkg::ext_inc;", 0, 13, "function", NONE),
    // ST-29: items of a package in another file, through a header import,
    // explicit import items and a qualified name.
    ("ST-29", "req_ext_pkg::*", 0, 0, "namespace", NONE),
    ("ST-29", "EXT_W) (", 0, 0, "variable", RO),
    ("ST-29", "ext_t ei", 0, 0, "type", NONE),
    ("ST-29", "EXT_L-1:0] eo", 0, 0, "variable", RO),
    ("ST-29", "EXT_L + ext_inc", 0, 0, "variable", RO),
    ("ST-29", "ext_inc(PW)", 0, 0, "function", NONE),
    ("ST-29", "req_ext_pkg::EXT_W;", 0, 0, "namespace", NONE),
    ("ST-29", "req_ext_pkg::EXT_W;", 0, 13, "variable", RO),
    ("ST-29", "e2 = EXT_ON", 0, 5, "enumMember", RO),
];

fn assert_check(rows: &[SemanticRow], source: &str, check: &Check, path: &str) {
    let (rule, needle, occurrence, offset, token_type, modifiers) = *check;
    let (line, col) = position_of(source, needle, occurrence);
    let row = rows
        .iter()
        .find(|row| row.line == line && row.character == col + offset)
        .unwrap_or_else(|| panic!("{rule} {path}: no token at {needle:?}+{offset}"));
    let got: Vec<&str> = row.modifiers.iter().map(String::as_str).collect();
    assert_eq!(
        (row.token_type.as_str(), got.as_slice()),
        (token_type, modifiers),
        "{rule} {path}: {needle:?}+{offset}"
    );
}

/// ST-01 (legend), ST-02 to ST-21, ST-23, ST-28 (classification), ST-24
/// (both serving paths agree), ST-25 (an open buffer that uses macros is
/// fully tokenized), ST-27 (inactive branches carry no token) and ST-29 (a
/// package in another project file, also in the open buffer).
#[test]
fn st_semantic_token_requirements_hold_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = write_requirements(&root);
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    // ST-01: standard types only, llg modifiers appended at stable indices.
    assert_eq!(types[15], "keyword", "ST-01 {types:?}");
    assert_eq!(
        &modifiers[10..],
        [
            "connectionLabel",
            "typeDefinition",
            "direction",
            "primitive",
            "controlFlow"
        ],
        "ST-01"
    );
    wait_for_diagnostics(&mut client, &uri, |_| true);

    let mut streams = Vec::new();
    for opened in [false, true] {
        if opened {
            client
                .open(&path, SOURCE)
                .expect("open requirements buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("requirements semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let label = if opened { "open buffer" } else { "project" };
        for check in CHECKS {
            assert_check(&rows, SOURCE, check, label);
        }
        // ST-27: the inactive `ifdef` branch carries no token.
        let (line, _) = position_of(SOURCE, "req_inactive_net", 0);
        assert!(
            rows.iter().all(|row| row.line != line),
            "ST-27 {label}: inactive branch tokens"
        );
        streams.push(result["data"].clone());
    }
    // ST-24, ST-25: the open buffer (which uses macros) matches the project
    // stream token for token.
    assert_eq!(streams[0], streams[1], "ST-24/ST-25: serving paths differ");
    client.shutdown();
}

/// ST-18: navigation from a member reference and a pattern key reaches the
/// member declaration.
#[test]
fn st_19_member_references_and_pattern_keys_navigate_to_the_member() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = write_requirements(&root);
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    wait_for_diagnostics(&mut client, &uri, |_| true);
    let declaration = position_of(SOURCE, "a; struct", 0);
    for (needle, offset) in [("s.a", 2), ("'{a:", 2)] {
        let (line, col) = position_of(SOURCE, needle, 0);
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": col + offset }
                }),
            )
            .expect("member definition response");
        let (target, start) = single_location(&response, "member definition");
        assert_eq!(target, uri, "ST-18 {needle}");
        assert_eq!(
            (start["line"].as_u64(), start["character"].as_u64()),
            (Some(declaration.0), Some(declaration.1)),
            "ST-18 {needle}: {response}"
        );
    }
    client.shutdown();
}

/// ST-28, ST-29: definition requests on import item names and on uses of a
/// package in another project file reach the package declarations, and the
/// import item names are not document symbols.
#[test]
fn st_28_and_st_29_import_items_and_package_uses_navigate_to_the_package() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = write_requirements(&root);
    let uri = file_uri(&path);
    let package_uri = file_uri(&root.join("st_requirements_pkg.sv"));
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    wait_for_diagnostics(&mut client, &uri, |_| true);
    for (needle, offset, declaration, declaration_offset) in [
        ("req_ext_pkg::*", 0, "package req_ext_pkg", 8),
        ("req_ext_pkg::EXT_L;", 13, "EXT_L = 2", 0),
        ("req_ext_pkg::EXT_ON;", 13, "EXT_ON }", 0),
        ("req_ext_pkg::ext_inc;", 0, "package req_ext_pkg", 8),
        ("req_ext_pkg::ext_inc;", 13, "ext_inc(int", 0),
        ("EXT_W) (", 0, "EXT_W = 8", 0),
        ("e2 = EXT_ON", 5, "EXT_ON }", 0),
    ] {
        let (line, col) = position_of(SOURCE, needle, 0);
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": col + offset }
                }),
            )
            .expect("package definition response");
        let (target, start) = single_location(&response, "package definition");
        let expected = position_of(PKG_SOURCE, declaration, 0);
        assert_eq!(target, package_uri, "ST-28/ST-29 {needle}+{offset}");
        assert_eq!(
            (start["line"].as_u64(), start["character"].as_u64()),
            (Some(expected.0), Some(expected.1 + declaration_offset)),
            "ST-28/ST-29 {needle}+{offset}: {response}"
        );
    }
    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        )
        .expect("document symbols");
    let names = names(&symbols);
    for imported in ["EXT_L", "ext_e", "EXT_ON", "ext_inc", "req_ext_pkg"] {
        assert!(
            !names.iter().any(|name| name == imported),
            "ST-28: {imported} listed as a fixture symbol: {names:?}"
        );
    }
    client.shutdown();
}

/// ST-22 (an unresolved name gets no token) and ST-26 (a syntax error keeps
/// the tokens Slang recovered) in both serving paths.
#[test]
fn st_23_and_st_27_unresolved_names_and_syntax_errors_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("st_errors.sv");
    let uri = file_uri(&path);
    fs::write(&path, ERRORS_SOURCE).expect("write error fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);
    for opened in [false, true] {
        if opened {
            client
                .open(&path, ERRORS_SOURCE)
                .expect("open error buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("error semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let label = if opened { "open buffer" } else { "project" };
        let (line, col) = position_of(ERRORS_SOURCE, "ghost_name", 0);
        assert!(
            !rows
                .iter()
                .any(|row| row.line == line && row.character == col),
            "ST-22 {label}: unresolved name has a token"
        );
        for check in [
            ("ST-26", "module req_errors", 0, 0, "keyword", NONE),
            ("ST-26", "kept;", 0, 0, "variable", DECL),
            ("ST-26", "always_comb", 0, 0, "keyword", CONTROL),
            ("ST-26", "assign b", 0, 0, "keyword", CONTROL),
        ] {
            assert_check(&rows, ERRORS_SOURCE, &check, label);
        }
    }
    client.shutdown();
}
