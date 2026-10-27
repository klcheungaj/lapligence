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

const BUILTIN_SOURCE: &str = include_str!("../fixtures/semantic_tokens/builtin_functions.sv");

/// Built-in calls in recovered syntax and after an unresolved receiver.
const BUILTIN_ERRORS_SOURCE: &str = "module bif_err;\n\
  int n;\n\
  initial begin\n\
    n = $clog2(4) + ghost.size();\n\
    $display(\"%0d\", n);\n\
    broken (\n\
  end\n\
endmodule\n";

const LIB: &[&str] = &["defaultLibrary"];

const BUILTIN_CHECKS: &[Check] = &[
    // ST-40: `$name` in every context.
    ("ST-40", "$clog2(n)", 0, 0, "function", LIB),
    ("ST-40", "$clog2(16)", 0, 0, "function", LIB),
    ("ST-40", "$bits(int)", 0, 0, "function", LIB),
    ("ST-40", "$countones(x)", 0, 0, "function", LIB),
    ("ST-40", "$clog2(D))", 0, 0, "function", LIB),
    ("ST-40", "$clog2(D)-1", 0, 0, "function", LIB),
    ("ST-40", "$ceil", 0, 0, "function", LIB),
    ("ST-40", "$floor", 0, 0, "function", LIB),
    ("ST-40", "$ln", 0, 0, "function", LIB),
    ("ST-40", "$sqrt", 0, 0, "function", LIB),
    ("ST-40", "$bits(addr)", 0, 0, "function", LIB),
    ("ST-40", "$signed", 0, 0, "function", LIB),
    ("ST-40", "$unsigned", 0, 0, "function", LIB),
    ("ST-40", "$clog2(D) > 2", 0, 0, "function", LIB),
    ("ST-40", "$display", 0, 0, "function", LIB),
    ("ST-40", "$size", 0, 0, "function", LIB),
    ("ST-40", "$countones(v)", 0, 0, "function", LIB),
    ("ST-40", "$bif_unknown", 0, 0, "function", LIB),
    ("ST-40", "`BIF_LOG2(8)", 0, 0, "macro", NONE),
    // ST-41: built-in methods.
    ("ST-41", "q.size()", 0, 2, "function", LIB),
    ("ST-41", "srandom", 0, 0, "function", LIB),
    ("ST-41", "randomize() with", 0, 0, "function", LIB),
    ("ST-41", "qq.size() - 1", 0, 3, "function", LIB),
    ("ST-41", "qq.size;", 0, 3, "function", LIB),
    ("ST-41", "push_back", 0, 0, "function", LIB),
    ("ST-41", "unique()", 0, 0, "function", LIB),
    ("ST-41", "and()", 0, 0, "function", LIB),
    ("ST-41", "sum() with", 0, 0, "function", LIB),
    ("ST-41", "len()", 0, 0, "function", LIB),
    ("ST-41", "next().name", 0, 0, "function", LIB),
    ("ST-41", "name()", 0, 0, "function", LIB),
    ("ST-41", "put(1)", 0, 0, "function", LIB),
    ("ST-41", "self()", 0, 0, "function", LIB),
    ("ST-41", "randomize(n)", 0, 0, "function", LIB),
    // ST-42: the std package qualifier and built-in class names.
    ("ST-42", "std::", 0, 0, "namespace", NONE),
    ("ST-42", "process::", 0, 0, "class", NONE),
    // ST-43: user subroutines keep `function` without `defaultLibrary`.
    ("ST-43", "pf(int", 0, 0, "function", DECL),
    ("ST-43", "pf(3)", 0, 0, "function", NONE),
    ("ST-43", "size();\n    return", 0, 0, "function", DECL),
    ("ST-43", "obj.size()", 0, 4, "function", NONE),
];

/// ST-40 to ST-43 over the wire in both serving paths, with an unresolved
/// receiver (ST-43) and recovered syntax (ST-40) in a second file.
#[test]
fn st_40_to_st_43_builtin_functions_and_methods_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("st_builtins.sv");
    let errors_path = root.join("st_builtin_errors.sv");
    fs::write(&path, BUILTIN_SOURCE).expect("write built-in fixture");
    fs::write(&errors_path, BUILTIN_ERRORS_SOURCE).expect("write built-in error fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    let uri = file_uri(&path);
    let errors_uri = file_uri(&errors_path);
    wait_for_diagnostics(&mut client, &uri, |_| true);
    wait_for_diagnostics(&mut client, &errors_uri, |_| true);
    for (file, uri, source, checks) in [
        (&path, &uri, BUILTIN_SOURCE, BUILTIN_CHECKS),
        (
            &errors_path,
            &errors_uri,
            BUILTIN_ERRORS_SOURCE,
            &[
                ("ST-40", "$clog2", 0, 0, "function", LIB),
                ("ST-40", "$display", 0, 0, "function", LIB),
                ("ST-40", "n = $clog2", 0, 0, "variable", NONE),
            ][..],
        ),
    ] {
        let mut streams = Vec::new();
        for opened in [false, true] {
            if opened {
                client.open(file, source).expect("open built-in buffer");
            }
            let result = client
                .request(
                    "textDocument/semanticTokens/full",
                    json!({ "textDocument": { "uri": uri } }),
                )
                .expect("built-in semantic token response");
            let rows = semantic_token_rows(&result, &types, &modifiers);
            let label = if opened { "open buffer" } else { "project" };
            for check in checks {
                assert_check(&rows, source, check, label);
            }
            if source == BUILTIN_ERRORS_SOURCE {
                let (line, col) = position_of(source, "ghost.size", 0);
                assert!(
                    !rows.iter().any(|row| row.line == line
                        && (row.character == col || row.character == col + 6)),
                    "ST-43 {label}: unresolved receiver or its method has a token"
                );
            }
            // ST-21: nothing of the `BIF_LOG2` body is painted over its use.
            if source == BUILTIN_SOURCE {
                let (line, col) = position_of(source, "`BIF_LOG2(8)", 0);
                assert!(
                    rows.iter().all(|row| row.line != line
                        || row.character <= col
                        || row.character >= col + 9),
                    "ST-21 {label}: expansion token over the use site"
                );
            }
            streams.push(result["data"].clone());
        }
        assert_eq!(streams[0], streams[1], "ST-24: serving paths differ");
    }
    client.shutdown();
}

const MACRO_SOURCE: &str = include_str!("../fixtures/semantic_tokens/macros_and_class_scope.sv");

const PRIM: &[&str] = &["primitive"];
const STATIC: &[&str] = &["static"];

const MACRO_CHECKS: &[Check] = &[
    // ST-60: the name a `define declares and its use sites are `macro`.
    ("ST-60", "`define A 32", 0, 8, "macro", DECL),
    ("ST-60", "`define A 32", 0, 0, "keyword", CONTROL),
    ("ST-60", "`A + `ABC", 0, 0, "macro", NONE),
    ("ST-60", "`A + `ABC", 0, 5, "macro", NONE),
    // ST-61: the net type of `default_nettype is a primitive keyword.
    ("ST-61", "`default_nettype none", 0, 0, "keyword", CONTROL),
    ("ST-61", "`default_nettype none", 0, 17, "keyword", PRIM),
    ("ST-61", "`default_nettype wire", 0, 17, "keyword", PRIM),
    // ST-62: the macro name of a conditional directive or `undef.
    ("ST-62", "`ifdef XXX", 0, 0, "keyword", CONTROL),
    ("ST-62", "`ifdef XXX", 0, 7, "macro", NONE),
    ("ST-62", "`elsif ZZZ", 0, 7, "macro", NONE),
    ("ST-62", "`ifndef YYY", 0, 8, "macro", NONE),
    ("ST-62", "`undef N", 0, 0, "keyword", CONTROL),
    ("ST-62", "`undef N", 0, 7, "macro", NONE),
    // ST-63: formal arguments are parameters, declared in the formal list.
    ("ST-63", "ABC(x,y)", 0, 4, "parameter", DECL),
    ("ST-63", "ABC(x,y)", 0, 6, "parameter", DECL),
    ("ST-63", "((x)+(y))", 0, 2, "parameter", NONE),
    ("ST-63", "((x)+(y))", 0, 6, "parameter", NONE),
    ("ST-63", "WR(v)", 0, 3, "parameter", DECL),
    ("ST-63", "$size(v)", 0, 6, "parameter", NONE),
    ("ST-63", "write_dr(v)", 0, 9, "parameter", NONE),
    ("ST-63", "`ABC(e, e)", 0, 5, "parameter", NONE),
    // ST-64: a `define body is highlighted like source, its names from the
    // expansions.
    ("ST-64", "cnt = cnt + 1; if", 0, 0, "variable", NONE),
    ("ST-64", "cnt = cnt + 1; if", 0, 6, "variable", NONE),
    ("ST-64", "cnt = cnt + 1; if", 0, 10, "operator", NONE),
    ("ST-64", "cnt = cnt + 1; if", 0, 12, "number", NONE),
    ("ST-64", "if (cnt > `A)", 0, 0, "keyword", CONTROL),
    ("ST-64", "if (cnt > `A)", 0, 10, "macro", NONE),
    ("ST-64", "`ABC(e, e)", 0, 0, "macro", NONE),
    ("ST-64", "SAME_KIND hits", 0, 10, "variable", NONE),
    ("ST-64", "\"body text\"", 0, 0, "string", NONE),
    ("ST-64", "never_seen + 1", 0, 13, "number", NONE),
    ("ST-64", "$size(v)", 0, 0, "function", LIB),
    // ST-65: class-scope names, also inside a `define body (ST-64).
    ("ST-65", "Util2#($size(v))", 0, 0, "class", NONE),
    ("ST-65", "write_dr(v)", 0, 0, "function", STATIC),
    ("ST-65", "Util2#($size(in))", 0, 0, "class", NONE),
    ("ST-65", "Util2#($size(in))", 0, 7, "function", LIB),
    ("ST-65", "Util2#($size(in))", 0, 13, "variable", NONE),
    ("ST-65", "write_dr(in)", 0, 0, "function", STATIC),
    ("ST-65", "write_dr(in)", 0, 9, "variable", NONE),
    ("ST-65", "`WR(in)", 0, 4, "variable", NONE),
    (
        "ST-65",
        "write_dr(input",
        0,
        0,
        "function",
        &["declaration", "static"],
    ),
    (
        "ST-65",
        "width();",
        0,
        0,
        "function",
        &["declaration", "static"],
    ),
    ("ST-65", "inst_width", 0, 0, "function", DECL),
    ("ST-65", "C::f()", 0, 0, "class", NONE),
    ("ST-65", "C::f()", 0, 3, "function", STATIC),
    ("ST-65", "Util2#(2)::width", 0, 0, "class", NONE),
    ("ST-65", "Util2#(2)::width", 0, 11, "function", STATIC),
    ("ST-65", "Util2#(4)::T", 0, 0, "class", NONE),
    ("ST-65", "Util2#(4)::T", 0, 11, "type", NONE),
    ("ST-65", "C::Inner::depth", 0, 0, "class", NONE),
    ("ST-65", "C::Inner::depth", 0, 3, "class", NONE),
    ("ST-65", "C::Inner::depth", 0, 10, "function", STATIC),
    // ST-23 (unchanged by ST-66): a delay is an operator and a number.
    ("ST-66", "#10", 0, 0, "operator", NONE),
    ("ST-66", "#10", 0, 1, "number", NONE),
    ("ST-66", "#1ns", 0, 1, "number", NONE),
];

/// ST-60 to ST-66 over the wire in both serving paths: macro names,
/// directive operands, macro formals and bodies, class-scope names, and the
/// empty document-color answer that keeps VS Code from offering a color
/// picker beside a delay.
#[test]
fn st_60_to_st_66_macros_directives_class_scope_and_delays_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("st_macros.sv");
    fs::write(&path, MACRO_SOURCE).expect("write macro fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    // ST-66: the server is a document-color provider.
    assert_eq!(
        initialize["capabilities"]["colorProvider"],
        json!(true),
        "ST-66 {initialize}"
    );
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    let uri = file_uri(&path);
    wait_for_diagnostics(&mut client, &uri, |_| true);
    let mut streams = Vec::new();
    for opened in [false, true] {
        if opened {
            client.open(&path, MACRO_SOURCE).expect("open macro buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("macro semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let label = if opened { "open buffer" } else { "project" };
        for check in MACRO_CHECKS {
            assert_check(&rows, MACRO_SOURCE, check, label);
        }
        // ST-64: a body name whose expansions disagree (a variable in one
        // module, a parameter in the other) and one of a macro never
        // expanded get no token.
        for needle in ["mixed\n", "never_seen"] {
            let (line, col) = position_of(MACRO_SOURCE, needle, 0);
            assert!(
                !rows
                    .iter()
                    .any(|row| row.line == line && row.character == col),
                "ST-64 {label}: {needle:?} has a token"
            );
        }
        // ST-66: an empty list, never null.
        let colors = client
            .request(
                "textDocument/documentColor",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("document color response");
        assert_eq!(colors, json!([]), "ST-66 {label}");
        streams.push(result["data"].clone());
    }
    assert_eq!(streams[0], streams[1], "ST-24: serving paths differ");
    client.shutdown();
}

/// ST-64: body names highlight like source but are macro text, not
/// references of the design: renaming the variable leaves the `define body
/// alone.
#[test]
fn st_64_macro_body_names_are_not_renamed_with_the_design_name() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("st_macros.sv");
    fs::write(&path, MACRO_SOURCE).expect("write macro fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let uri = file_uri(&path);
    wait_for_diagnostics(&mut client, &uri, |_| true);
    let (line, col) = position_of(MACRO_SOURCE, "cnt;", 0);
    let rename = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": col },
                "newName": "renamed",
            }),
        )
        .expect("rename response");
    let edits = rename["changes"][uri.as_str()]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!edits.is_empty(), "ST-64: rename found nothing: {rename}");
    let (body_line, _) = position_of(MACRO_SOURCE, "cnt = cnt + 1; if", 0);
    assert!(
        edits
            .iter()
            .all(|edit| edit["range"]["start"]["line"] != json!(body_line)),
        "ST-64: rename edits the `define body: {rename}"
    );
    client.shutdown();
}
