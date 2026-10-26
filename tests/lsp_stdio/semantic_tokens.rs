//! Semantic tokens.

use super::*;

#[test]
fn lsp_stdio_semantic_tokens_use_current_open_buffer_and_cached_unopened_snapshot() {
    // Arrange
    let fixture = FixtureTree::new();
    let root = fixture.root("semantic-current");
    fs::create_dir_all(&root).expect("create semantic-token workspace");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n\n[compile]\ndefines = [\"SEMANTIC_ONLY=1\"]\n",
    )
    .expect("write semantic-token config");
    let opened = root.join("opened.sv");
    let unopened = root.join("unopened.sv");
    let other = root.join("other_unit.sv");
    let include = root.join("not_consumed.svh");
    let opened_disk = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\nmodule DiskOpened;\n  logic disk_signal;\nendmodule\n"
    );
    let unopened_snapshot = format!(
        "{SOURCE_HEADER} semantic-current/unopened.sv\nmodule CachedUnopened;\n  logic cached_signal;\nendmodule\n"
    );
    fs::write(&opened, &opened_disk).expect("write opened disk snapshot");
    fs::write(&unopened, &unopened_snapshot).expect("write unopened snapshot");
    fs::write(
        &other,
        format!(
            "{SOURCE_HEADER} semantic-current/other_unit.sv\nmodule UnrelatedProjectUnit; endmodule\n"
        ),
    )
    .expect("write unrelated project unit");
    fs::write(
        &include,
        format!(
            "{SOURCE_HEADER} semantic-current/not_consumed.svh\nmodule IncludedUnit; endmodule\n"
        ),
    )
    .expect("write include sentinel");
    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("semantic-current", &root)], default_init_options())
        .expect("initialize semantic-token workspace");

    // Change the closed file without a watched-file notification.  Its LSP
    // result must continue to come from the initialized project snapshot.
    let unopened_new_disk = format!(
        "{SOURCE_HEADER} semantic-current/unopened.sv\n\n\n\n\n\n\n\nmodule NewDiskOnly; endmodule\n"
    );
    fs::write(&unopened, unopened_new_disk).expect("replace unopened disk text");

    // Act
    let unopened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&unopened) } }),
        )
        .expect("request unopened semantic tokens");
    let opened_buffer = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\n`include \"not_consumed.svh\"\n\n\n\n\nmodule UnsavedOpened;\n  logic unsaved_signal;\nendmodule\n"
    );
    client
        .open(&opened, &opened_buffer)
        .expect("open unsaved semantic-token buffer");
    let opened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request opened semantic tokens");

    // Assert
    let unopened_positions = semantic_token_positions(&unopened_tokens);
    assert!(
        !unopened_positions.is_empty(),
        "cached unopened semantic tokens must be non-empty"
    );
    assert!(
        unopened_positions.contains(&(1, 0)),
        "unopened file did not retain the cached module position: {unopened_positions:?}"
    );
    assert!(
        unopened_positions.iter().all(|(line, _)| *line < 8),
        "unopened file was reparsed from changed disk text: {unopened_positions:?}"
    );

    let opened_positions = semantic_token_positions(&opened_tokens);
    assert!(
        !opened_positions.is_empty(),
        "opened semantic tokens must be non-empty"
    );
    assert!(
        opened_positions.contains(&(6, 0)),
        "opened file did not use the current in-memory module position: {opened_positions:?}"
    );

    // A syntax-broken current buffer must produce no semantic tokens.  In
    // particular, it must neither expose a partial parse-only stream nor
    // fall back to the last valid project snapshot.
    let syntax_buffer =
        format!("{SOURCE_HEADER} semantic-current/opened.sv\n\n\n\n\nmodule SyntaxOnly;\n");
    client
        .change(&opened, 2, &syntax_buffer)
        .expect("change opened buffer to syntax-broken source");
    let syntax_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request syntax-broken opened semantic tokens");
    assert!(
        semantic_token_positions(&syntax_tokens).is_empty(),
        "syntax-broken current buffer returned semantic tokens: {syntax_tokens}"
    );

    // A later complete revision must not be pinned to the cached empty result
    // from the broken text: the text hash changes and valid unsaved
    // highlighting becomes available again immediately.
    let recovered_buffer = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\n\nmodule RecoveredOpened;\n  logic recovered_signal;\nendmodule\n"
    );
    client
        .change(&opened, 3, &recovered_buffer)
        .expect("repair opened semantic-token buffer");
    let recovered_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request repaired opened semantic tokens");
    assert!(
        !semantic_token_positions(&recovered_tokens).is_empty(),
        "repaired current buffer did not recover semantic tokens: {recovered_tokens}"
    );

    // Act: replace the token-bearing open buffer with whitespace only.
    client
        .change(&opened, 4, " \n\t\n")
        .expect("change opened buffer to whitespace");
    let empty_opened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request whitespace-only opened semantic tokens");

    // Assert: a successful empty parse is authoritative, not a reason to
    // return the token-bearing cached project snapshot.
    assert!(
        semantic_token_positions(&empty_opened_tokens).is_empty(),
        "whitespace-only current buffer returned stale cached semantic tokens: {empty_opened_tokens}"
    );
    client.shutdown();
}

/// Parameters and localparams are constants (`variable` + `readonly`) at
/// declarations, uses and override actuals, built-in type and kind words are
/// `keyword` + `primitive` and directions `keyword` + `direction` (no other
/// modifier bits) through project and isolated unsaved-buffer analysis,
/// including references to missing children.
#[test]
fn lsp_stdio_semantic_colors_preserve_constants_and_types_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("semantic_colors.sv");
    let uri = file_uri(&path);
    let source = "module color_top #(parameter int WIDTH = 8)(input logic [WIDTH-1:0] data, output wire [WIDTH-1:0] result, inout tri shared);\nlocalparam int LIMIT = WIDTH + 1;\nwire [LIMIT-1:0] buffer;\nint memory [LIMIT];\nassign buffer = data + LIMIT;\nassign result = data;\ncolor_child #(.P(WIDTH), .Q(LIMIT)) child(.data(data));\nendmodule\n";
    fs::write(&path, source).expect("write semantic color fixture");
    fs::write(
        root.join("semantic_color_child.sv"),
        "module color_child #(parameter int P = 8, Q = 4)(input logic [P-1:0] data); endmodule\n",
    )
    .expect("write separate child definition");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);

    for opened in [false, true] {
        let buffer = if opened {
            source.replace("LIMIT", "CEILING")
        } else {
            source.to_owned()
        };
        if opened {
            client
                .open(&path, &buffer)
                .expect("open unsaved renamed constant");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({
                    "textDocument": { "uri": uri }
                }),
            )
            .expect("semantic color response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        for name in ["WIDTH", if opened { "CEILING" } else { "LIMIT" }] {
            let occurrences = buffer.match_indices(name).count();
            assert!(occurrences >= 4);
            for occurrence in 0..occurrences {
                let (line, col) = position_of(&buffer, name, occurrence);
                let row = row_at(&rows, line, col);
                assert_eq!(
                    row.token_type, "variable",
                    "opened={opened} {name}: {row:?}"
                );
                assert!(
                    row.modifiers.iter().any(|name| name == "readonly"),
                    "{row:?}"
                );
                assert_eq!(
                    row.modifiers.iter().any(|name| name == "declaration"),
                    occurrence == 0,
                    "{row:?}"
                );
                assert!(
                    !row.modifiers.iter().any(|name| name == "connectionLabel"),
                    "actual is not a label: {row:?}"
                );
            }
        }
        for (names, expected) in [
            (
                &["logic", "wire", "tri", "int", "parameter", "localparam"][..],
                "primitive",
            ),
            (&["input", "output", "inout"][..], "direction"),
        ] {
            for name in names {
                for occurrence in 0..buffer.match_indices(name).count() {
                    let (line, col) = position_of(&buffer, name, occurrence);
                    let row = row_at(&rows, line, col);
                    assert_eq!(row.token_type, "keyword", "{name}: {row:?}");
                    assert_eq!(row.modifiers, [expected], "{name}: {row:?}");
                }
            }
        }
        for name in ["module color_top", "assign buffer", "endmodule"] {
            let (line, col) = position_of(&buffer, name, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "keyword", "{name}");
            assert!(row.modifiers.is_empty(), "{name}: {row:?}");
        }
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    client.shutdown();
}

/// Enum items are `enumMember` + `readonly` at declarations (which add
/// `declaration`) and at references, `typedef` and `enum` are `keyword` +
/// `typeDefinition` and `logic` is `keyword` + `primitive` (`import`/`package`
/// stay plain `keyword`), through project and isolated unsaved-buffer analysis.
#[test]
fn lsp_stdio_semantic_tokens_mark_enum_members_readonly_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("enum_colors.sv");
    let uri = file_uri(&path);
    let source = "package enum_pkg;\n  typedef enum logic [1:0] { PKG_A, PKG_B } pkg_t;\n  enum { ANON_A, ANON_B = 5 } anon;\nendpackage\nmodule enum_top;\n  import enum_pkg::*;\n  typedef enum logic [1:0] { IDLE, RUN } state_t;\n  state_t st;\n  pkg_t pk;\n  initial begin\n    st = IDLE;\n    pk = enum_pkg::PKG_B;\n    if (pk == PKG_A) st = RUN;\n    enum_pkg::anon = ANON_B;\n  end\nendmodule\n";
    fs::write(&path, source).expect("write enum fixture");
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
            client.open(&path, source).expect("open enum buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("enum semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let has = |row: &SemanticRow, name: &str| row.modifiers.iter().any(|m| m == name);
        let declarations = [
            "PKG_A, ", "PKG_B }", "ANON_A,", "ANON_B =", "IDLE,", "RUN }",
        ];
        for needle in declarations {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(
                row.token_type, "enumMember",
                "opened={opened} {needle}: {row:?}"
            );
            assert!(has(row, "readonly"), "opened={opened} {needle}: {row:?}");
            assert!(has(row, "declaration"), "opened={opened} {needle}: {row:?}");
        }
        let references = ["IDLE;", "PKG_B;", "PKG_A)", "RUN;", "ANON_B;"];
        for needle in references {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(
                row.token_type, "enumMember",
                "opened={opened} {needle}: {row:?}"
            );
            assert!(has(row, "readonly"), "opened={opened} {needle}: {row:?}");
            assert!(
                !has(row, "declaration"),
                "opened={opened} {needle}: {row:?}"
            );
        }
        let (line, col) = position_of(source, "enum_pkg::PKG_B", 0);
        if let Some(qualifier) = rows
            .iter()
            .find(|row| row.line == line && row.character == col)
        {
            assert_ne!(
                qualifier.token_type, "enumMember",
                "opened={opened}: {qualifier:?}"
            );
        }
        for (needle, expected) in [
            ("typedef enum logic", "typeDefinition"),
            ("enum logic", "typeDefinition"),
            ("enum {", "typeDefinition"),
            ("logic [1:0] {", "primitive"),
        ] {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "keyword", "opened={opened} {needle}");
            assert_eq!(
                row.modifiers,
                [expected],
                "opened={opened} {needle}: {row:?}"
            );
        }
        for needle in ["import", "package"] {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "keyword", "opened={opened} {needle}");
            assert!(
                row.modifiers.is_empty(),
                "opened={opened} {needle}: {row:?}"
            );
        }
    }
    client.shutdown();
}

/// Every grouped word is `keyword` with exactly its group's llg modifier at
/// every occurrence: `typedef struct union enum` carry `typeDefinition`,
/// `input output inout ref` carry `direction`, and built-in data and net types
/// plus kind and qualifier words carry `primitive`. The legend keeps only
/// standard token types and appends the three modifiers and `controlFlow`
/// after `connectionLabel`. `module`/`always` are `keyword` without modifier
/// bits, and a use of the user typedef name keeps its own `type` token, through
/// project and isolated unsaved-buffer analysis.
#[test]
fn lsp_stdio_semantic_tokens_send_grouped_words_as_keywords_with_llg_modifiers() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("keyword_words.sv");
    let uri = file_uri(&path);
    let source = "module all_words #(parameter int P = 1)(input logic i, output wire o, inout tri t);\n  localparam int L = 2;\n  specparam S = 3;\n  bit b; reg r; byte by; shortint si; longint li; integer ig; time tm;\n  shortreal sr; real re; realtime rt; string sg; chandle ch; event ev;\n  logic signed [1:0] sn; logic unsigned [1:0] us;\n  genvar gv;\n  supply0 s0; supply1 s1; triand ta; trior tr; trireg tg; tri0 t0; tri1 t1;\n  uwire uw; wand wa; wor wo;\n  typedef struct packed { bit f; } st_t;\n  typedef union packed { bit f; } un_t;\n  typedef enum { E0 } en_t;\n  st_t user_s;\n  const int ci = 1;\n  var logic vl;\n  static int sv;\n  function automatic void fa(ref int x); endfunction\n  always @* case (sv) 0: user_s = '{default: 0}; endcase\n  assign o = i;\nendmodule\n";
    let type_definition = ["typedef", "struct", "union", "enum"];
    let direction = ["input", "output", "inout", "ref"];
    let primitive = [
        "parameter",
        "localparam",
        "specparam",
        "const",
        "var",
        "bit",
        "logic",
        "reg",
        "byte",
        "shortint",
        "int",
        "longint",
        "integer",
        "time",
        "shortreal",
        "real",
        "realtime",
        "string",
        "chandle",
        "event",
        "void",
        "signed",
        "unsigned",
        "genvar",
        "supply0",
        "supply1",
        "tri",
        "triand",
        "trior",
        "trireg",
        "tri0",
        "tri1",
        "uwire",
        "wire",
        "wand",
        "wor",
        "default",
        "automatic",
        "static",
    ];
    fs::write(&path, source).expect("write keyword-word fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    assert!(
        !types
            .iter()
            .any(|t| t == "primitive" || t == "declarationKeyword"),
        "{types:?}"
    );
    assert_eq!(
        types.last().map(String::as_str),
        Some("operator"),
        "{types:?}"
    );
    let connection_label = modifiers
        .iter()
        .position(|m| m == "connectionLabel")
        .expect("connectionLabel in legend");
    assert_eq!(
        modifiers[connection_label + 1..],
        ["typeDefinition", "direction", "primitive", "controlFlow"],
        "{modifiers:?}"
    );
    wait_for_diagnostics(&mut client, &uri, |_| true);

    for opened in [false, true] {
        if opened {
            client
                .open(&path, source)
                .expect("open keyword-word buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("keyword-word semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        for (words, expected) in [
            (&type_definition[..], "typeDefinition"),
            (&direction[..], "direction"),
            (&primitive[..], "primitive"),
        ] {
            for word in words {
                let positions = word_positions(source, word);
                assert!(!positions.is_empty(), "sample lacks {word}");
                for (line, col) in positions {
                    let row = row_at(&rows, line, col);
                    assert_eq!(row.token_type, "keyword", "opened={opened} {word}: {row:?}");
                    assert_eq!(row.modifiers, [expected], "opened={opened} {word}: {row:?}");
                }
            }
        }
        for needle in ["module", "always"] {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "keyword", "opened={opened} {needle}");
            assert!(
                row.modifiers.is_empty(),
                "opened={opened} {needle}: {row:?}"
            );
        }
        let (line, col) = position_of(source, "st_t user_s;", 0);
        let row = row_at(&rows, line, col);
        assert_eq!(row.token_type, "type", "opened={opened}: {row:?}");
        assert!(
            !row.modifiers.iter().any(|m| m == "defaultLibrary"),
            "opened={opened}: {row:?}"
        );
    }
    client.shutdown();
}

/// A name with no definition gets no token (the editor leaves it uncolored)
/// while declared names keep `variable`, through project and isolated
/// unsaved-buffer analysis.
#[test]
fn lsp_stdio_semantic_tokens_skip_unresolved_names_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("unresolved.sv");
    let uri = file_uri(&path);
    let source = "module unresolved_top;\n  logic [3:0] data, result;\n  assign result = data + ghost_name;\n  always_comb data = result;\nendmodule\n";
    fs::write(&path, source).expect("write unresolved fixture");
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
            client.open(&path, source).expect("open unresolved buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("unresolved semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let (line, col) = position_of(source, "ghost_name", 0);
        assert!(
            rows.iter()
                .all(|row| !(row.line == line && row.character == col)),
            "opened={opened}: undeclared name got a token: {rows:?}"
        );
        for (needle, occurrence) in [("data", 2), ("result", 2)] {
            let (line, col) = position_of(source, needle, occurrence);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "variable", "opened={opened} {needle}");
        }
        let (line, col) = position_of(source, "data", 0);
        assert!(
            row_at(&rows, line, col)
                .modifiers
                .iter()
                .any(|m| m == "declaration"),
            "opened={opened}: declarations keep their token"
        );
    }
    client.shutdown();
}

/// One undeclared name leaves only itself uncolored: the other names of its
/// invalid block, package qualifiers, hierarchical path steps, `disable`
/// labels and system names keep their kinds, through project and isolated
/// unsaved-buffer analysis.
#[test]
fn lsp_stdio_semantic_tokens_keep_resolved_names_of_erroneous_blocks_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("resolved_names.sv");
    let uri = file_uri(&path);
    let source = "package pk; int pv; function int pf(int a); return a; endfunction endpackage\n\
interface ifc; logic sig; endinterface\n\
module sub(input logic i, output logic o); assign o = i; endmodule\n\
module names_top;\n\
  logic [3:0] v, w;\n\
  ifc bus();\n\
  sub u0(.i(v[0]), .o(w[0]));\n\
  for (genvar g = 0; g < 2; g++) begin : gb logic q; end\n\
  initial begin : bad\n\
    v = ghost + w;\n\
    w = pk::pf(v) + pk::pv + gb[0].q + u0.o + bus.sig + $bits(v);\n\
    disable bad;\n\
  end\n\
endmodule\n";
    fs::write(&path, source).expect("write resolved-names fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);

    let expected: [(&str, usize, u64, &str, &[&str]); 15] = [
        ("v = ghost", 0, 0, "variable", &[]),
        ("+ w;", 0, 2, "variable", &[]),
        ("pk::pf", 0, 0, "namespace", &[]),
        ("pk::pf", 0, 4, "function", &[]),
        ("pf(v)", 0, 3, "variable", &[]),
        ("pk::pv", 0, 0, "namespace", &[]),
        ("pk::pv", 0, 4, "variable", &[]),
        ("gb[0].q", 0, 0, "namespace", &[]),
        ("gb[0].q", 0, 6, "variable", &[]),
        ("u0.o", 0, 0, "variable", &[]),
        ("u0.o", 0, 3, "parameter", &["readonly"]),
        ("bus.sig", 0, 0, "variable", &[]),
        ("bus.sig", 0, 4, "variable", &[]),
        ("$bits", 0, 0, "function", &["defaultLibrary"]),
        ("disable bad", 0, 8, "namespace", &[]),
    ];
    for opened in [false, true] {
        if opened {
            client
                .open(&path, source)
                .expect("open resolved-names buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("resolved-names semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let (line, col) = position_of(source, "ghost", 0);
        assert!(
            rows.iter()
                .all(|row| !(row.line == line && row.character == col)),
            "opened={opened}: undeclared name got a token: {rows:?}"
        );
        for (needle, occurrence, offset, token_type, token_modifiers) in expected {
            let (line, col) = position_of(source, needle, occurrence);
            let row = row_at(&rows, line, col + offset);
            assert_eq!(
                row.token_type, token_type,
                "opened={opened} {needle}+{offset}"
            );
            assert_eq!(
                row.modifiers, token_modifiers,
                "opened={opened} {needle}+{offset}"
            );
        }
    }
    client.shutdown();
}

/// Connection labels carry `connectionLabel` and the type of the child
/// declaration they name (`parameter` for ports, `variable` + `readonly` for
/// parameters); actuals retain their own symbol classification in both
/// cached-project and isolated open-buffer responses.
#[test]
fn lsp_stdio_semantic_tokens_mark_connection_labels_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let labels_path = root_a.join("bindings").join("labels").join("top.sv");
    let labels_text = fs::read_to_string(&labels_path).expect("read label_top fixture");
    let labels_uri = file_uri(&labels_path);

    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize connection-label workspace");
    let legend = initialize
        .get("capabilities")
        .and_then(|capabilities| capabilities.get("semanticTokensProvider"))
        .and_then(|provider| provider.get("legend"))
        .cloned()
        .expect("semanticTokensProvider.legend");
    let legend_types = legend_names(&legend, "tokenTypes");
    let legend_modifiers = legend_names(&legend, "tokenModifiers");
    assert!(
        legend_modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        "legend must advertise the connectionLabel modifier: {legend_modifiers:?}"
    );

    wait_for_diagnostics(&mut client, &labels_uri, has_no_severity_1);

    // ── Path 1: unopened document → cached project-index tokens ──────────
    let cached = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": labels_uri } }),
        )
        .expect("cached semantic tokens request");
    let rows = semantic_token_rows(&cached, &legend_types, &legend_modifiers);
    assert!(!rows.is_empty(), "cached token stream must be non-empty");

    // `label_child u_child(.clk(wa));` — the label token starts at the
    // identifier AFTER the dot, the actual at the identifier after the paren.
    let (clk_line, clk_dot_col) = position_of(&labels_text, ".clk", 0);
    let clk_row = row_at(&rows, clk_line, clk_dot_col + 1);
    assert_eq!(clk_row.token_type, "parameter", "clk row: {clk_row:?}");
    assert!(
        clk_row
            .modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        ".clk label must carry connectionLabel: {clk_row:?}"
    );
    let (wa_line, wa_paren_col) = position_of(&labels_text, "(wa)", 0);
    let wa_row = row_at(&rows, wa_line, wa_paren_col + 1);
    assert_eq!(wa_row.token_type, "variable", "wa row: {wa_row:?}");
    assert!(
        !wa_row
            .modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        "connected signal must NOT carry connectionLabel: {wa_row:?}"
    );

    // ── Path 2: open buffer → isolated parse-only tokens ─────────────────
    // The buffer exercises single-line AND multi-line instantiations with
    // named PORT connections and named PARAMETER overrides.
    let iso_path = root_a.join("iso_labels.sv");
    fs::write(&iso_path, format!("{SOURCE_HEADER} placeholder\n")).expect("seed iso file");
    let iso_uri = file_uri(&iso_path);
    let iso_buffer = format!(
        "{SOURCE_HEADER} root-a/iso_labels.sv\n\
module iso_top;\n  logic wa;\n  logic [7:0] tq;\n\n\
  p_pchild #(.W(4), .D(wa)) u_iso (.clk(wa), .q(tq));\n\n\
  p_pchild #(\n    .W(8),\n    .D(1)\n  ) u_ml (\n    .clk(wa),\n    .q(tq)\n  );\n\
endmodule\n"
    );
    client
        .open(&iso_path, &iso_buffer)
        .expect("open iso buffer");
    let isolated = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": iso_uri } }),
        )
        .expect("isolated semantic tokens request");
    let iso_rows = semantic_token_rows(&isolated, &legend_types, &legend_modifiers);
    assert!(
        !iso_rows.is_empty(),
        "isolated token stream must be non-empty"
    );

    let expect_label = |needle: &str, occurrence: usize, want_type: &str| -> Vec<String> {
        let (line, dot_col) = position_of(&iso_buffer, needle, occurrence);
        let row = row_at(&iso_rows, line, dot_col + 1);
        assert_eq!(row.token_type, want_type, "{needle} row: {row:?}");
        assert!(
            row.modifiers.iter().any(|name| name == "connectionLabel"),
            "{needle} label must carry connectionLabel: {row:?}"
        );
        row.modifiers.clone()
    };
    let expect_plain = |text: &str, needle: &str, occurrence: usize, offset: usize| {
        let (line, col) = position_of(text, needle, occurrence);
        let row = row_at(&iso_rows, line, col + offset as u64);
        assert_eq!(row.token_type, "variable", "{needle} row: {row:?}");
        assert!(
            !row.modifiers.iter().any(|name| name == "connectionLabel"),
            "connected signal at {needle}+{offset} must NOT carry connectionLabel: {row:?}"
        );
    };

    // Single-line instantiation: param override labels …
    let w_mods = expect_label(".W", 0, "variable");
    assert!(
        w_mods.iter().any(|name| name == "readonly"),
        "override label keeps its readonly base modifier: {w_mods:?}"
    );
    expect_label(".D", 0, "variable");
    // … port labels …
    expect_label(".clk", 0, "parameter");
    expect_label(".q", 0, "parameter");
    // … and the connected signals stay plain.
    expect_plain(&iso_buffer, ".D(wa)", 0, 3);
    expect_plain(&iso_buffer, ".clk(wa)", 0, 5);
    expect_plain(&iso_buffer, ".q(tq)", 0, 3);

    // Multi-line instantiation: identical marking on continuation lines.
    expect_label(".W", 1, "variable");
    expect_label(".D", 1, "variable");
    expect_label(".clk", 1, "parameter");
    expect_label(".q", 1, "parameter");
    expect_plain(&iso_buffer, ".clk(wa)", 1, 5);
    expect_plain(&iso_buffer, ".q(tq)", 1, 3);

    client.shutdown();
}

/// `default` labelling a case, case-inside, case-generate, property-case or
/// randsequence-case item is `keyword.controlFlow`; every other `default`
/// (assignment-pattern keys, `default clocking`, `default disable iff`,
/// default bins) is `keyword.primitive`, through project and isolated
/// unsaved-buffer analysis.
#[test]
fn lsp_stdio_semantic_tokens_split_control_and_declaration_defaults_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("default_words.sv");
    let uri = file_uri(&path);
    let source = "module default_words;\n\
  typedef struct packed { logic [3:0] a; logic [3:0] b; } st_t;\n\
  logic [7:0] q; st_t s; int arr [4]; logic clk;\n\
  default clocking cb @(posedge clk); endclocking\n\
  default disable iff (clk);\n\
  covergroup cg @(posedge clk); coverpoint q { bins lo = {0}; bins other = default; } endgroup\n\
  case (1) 0: begin : c0 logic z; end default: begin : cd logic z; end endcase\n\
  always @(posedge clk) begin\n\
    case (q) 8'd0: q = 1; default: q = 0; endcase\n\
    case (q) inside [0:3]: q = 2; default q = 3; endcase\n\
    s = '{default: 0}; s = '{a: 1, default: 0}; arr = '{default: 5};\n\
  end\n\
  property p; case (q) 0: clk; default: 1; endcase endproperty\n\
  initial randsequence (main) main : case (q) 0: ra; default: rb; endcase; ra : {}; rb : {}; endsequence\n\
endmodule\n";
    // Zero-based lines whose `default` labels a case item.
    let control_lines = [6, 8, 9, 12, 13];
    fs::write(&path, source).expect("write default-word fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);

    let positions = word_positions(source, "default");
    assert_eq!(positions.len(), 11, "sample defaults");
    for opened in [false, true] {
        if opened {
            client
                .open(&path, source)
                .expect("open default-word buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("default-word semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        for &(line, col) in &positions {
            let row = row_at(&rows, line, col);
            let expected: &[&str] = if control_lines.contains(&line) {
                &["controlFlow"]
            } else {
                &["primitive"]
            };
            assert_eq!(row.token_type, "keyword", "opened={opened} {row:?}");
            assert_eq!(row.modifiers, expected, "opened={opened} {row:?}");
        }
        for (needle, expected) in [
            ("logic [3:0] a", &["primitive"][..]),
            ("case (q)", &["controlFlow"]),
        ] {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "keyword", "opened={opened} {needle}");
            assert_eq!(row.modifiers, expected, "opened={opened} {needle}");
        }
    }
    client.shutdown();
}

/// Raw `(token type index, modifier bitset)` of the token starting at
/// zero-based `line`/`character`, without legend name resolution.
fn raw_token_at(result: &Value, line: u64, character: u64) -> (u64, u64) {
    let data = result["data"].as_array().expect("semantic token data");
    let (mut at_line, mut at_character) = (0, 0);
    for token in data.as_chunks::<5>().0 {
        let delta_line = token[0].as_u64().unwrap();
        let delta_start = token[1].as_u64().unwrap();
        if delta_line == 0 {
            at_character += delta_start;
        } else {
            at_line += delta_line;
            at_character = delta_start;
        }
        if (at_line, at_character) == (line, character) {
            return (token[3].as_u64().unwrap(), token[4].as_u64().unwrap());
        }
    }
    panic!("no semantic token at {line}:{character}");
}

/// Control-flow words are `keyword` (index 15) with exactly the
/// `controlFlow` bit (modifier 14) in both serving paths, while `disable iff`,
/// block delimiters, declaration keywords and the other grouped words keep
/// their own bits.
#[test]
fn lsp_stdio_semantic_tokens_mark_control_flow_keywords_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("control_words.sv");
    let uri = file_uri(&path);
    let source = "module control_words(input logic clk, input logic rst);\n\
  typedef logic [3:0] q_t;\n\
  q_t q; int arr [2]; event ev;\n\
  for (genvar i = 0; i < 2; i++) begin : g\n\
    if (i == 0) begin : t end else begin : e end\n\
  end\n\
  case (1) 1: begin : c end default: begin : d end endcase\n\
  property p; @(posedge clk iff rst) disable iff (rst) if (q == 0) q == 1 else q == 2; endproperty\n\
  function automatic int f(int n); return n; endfunction\n\
  always @(posedge clk) begin : blk\n\
    unique if (q == 0) q <= 1; else if (q inside {[1:2]}) q <= 2; else q <= 3;\n\
    unique0 if (q == 1) q <= 0;\n\
    priority if (q == 2) q <= 0;\n\
    unique case (q) 0: q <= 1; default: q <= 2; endcase\n\
    unique0 casez (q) 4'b1???: q <= 0; endcase\n\
    priority casex (q) 4'b0xxx: q <= 1; endcase\n\
  end\n\
  initial begin\n\
    randcase 1: arr[0] = 0; endcase\n\
    for (int k = 0; k < 2; k++) begin if (k == 1) break; else continue; end\n\
    foreach (arr[k]) arr[k] = k;\n\
    while (arr[0] > 0) arr[0]--;\n\
    do arr[0]++; while (arr[0] < 2);\n\
    repeat (2) @(posedge clk);\n\
    wait (rst);\n\
    wait fork;\n\
    wait_order (ev) else arr[0] = 0;\n\
    fork : fk join_any\n\
    disable fk;\n\
    disable fork;\n\
    -> ev;\n\
    forever begin break; end\n\
  end\n\
endmodule\n";
    let control_words = [
        "if",
        "else",
        "case",
        "casex",
        "casez",
        "randcase",
        "endcase",
        "for",
        "foreach",
        "while",
        "do",
        "repeat",
        "forever",
        "return",
        "break",
        "continue",
        "wait",
        "wait_order",
        "unique",
        "unique0",
        "priority",
        "disable",
        "default",
    ];
    let disable_iff = position_of(source, "disable iff", 0);
    fs::write(&path, source).expect("write control-word fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    assert_eq!(types[15], "keyword", "{types:?}");
    assert_eq!(modifiers[14], "controlFlow", "{modifiers:?}");
    assert_eq!(modifiers.len(), 15, "{modifiers:?}");
    let control = 1 << 14;
    wait_for_diagnostics(&mut client, &uri, |_| true);

    for opened in [false, true] {
        if opened {
            client
                .open(&path, source)
                .expect("open control-word buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("control-word semantic token response");
        for word in control_words {
            let positions = word_positions(source, word);
            assert!(!positions.is_empty(), "sample lacks {word}");
            for (line, col) in positions {
                let want = if (line, col) == disable_iff {
                    0
                } else {
                    control
                };
                assert_eq!(
                    raw_token_at(&result, line, col),
                    (15, want),
                    "opened={opened} {word} at {line}:{col}"
                );
            }
        }
        for (word, bits) in [
            ("module", 0),
            ("always", 0),
            ("begin", 0),
            ("end", 0),
            ("fork", 0),
            ("join_any", 0),
            ("iff", 0),
            ("inside", 0),
            ("typedef", 1 << 11),
            ("input", 1 << 12),
            ("logic", 1 << 13),
        ] {
            for (line, col) in word_positions(source, word) {
                assert_eq!(
                    raw_token_at(&result, line, col),
                    (15, bits),
                    "opened={opened} {word} at {line}:{col}"
                );
            }
        }
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let (line, col) = position_of(source, "fk;", 0);
        let row = row_at(&rows, line, col);
        assert_eq!(row.token_type, "namespace", "opened={opened} {row:?}");
        assert!(row.modifiers.is_empty(), "opened={opened} {row:?}");
        let (line, col) = position_of(source, "-> ev", 0);
        assert_eq!(
            row_at(&rows, line, col).token_type,
            "operator",
            "opened={opened}"
        );
    }
    client.shutdown();
}

/// Value parameters are `variable` + `readonly`, type parameters
/// `typeParameter`, port labels `parameter` and parameter labels `variable` +
/// `readonly` (labels with `connectionLabel`) in both serving paths. Compiler
/// directives are `keyword` (conditional ones with `controlFlow`) in the
/// project stream; the isolated open-buffer parse masks directive lines, so
/// they have no token there.
#[test]
fn lsp_stdio_semantic_tokens_type_parameters_labels_and_directives_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("analogue_types.sv");
    let uri = file_uri(&path);
    let source = "`ifndef ANALOGUE_TYPES\n\
`define ANALOGUE_TYPES\n\
`endif\n\
`timescale 1ns/1ps\n\
module analogue_child #(parameter int P = 1, parameter type T = logic [1:0])(input T a, output logic y);\n\
  T t_local;\n\
  assign t_local = a;\n\
  assign y = t_local[0] ^ P[0];\n\
endmodule\n\
module analogue_top;\n\
  localparam int L = 2;\n\
  logic [1:0] v; logic o;\n\
  analogue_child #(.P(L)) u (.a(v), .y(o));\n\
endmodule\n";
    fs::write(&path, source).expect("write analogue-type fixture");
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
                .open(&path, source)
                .expect("open analogue-type buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("analogue-type semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        let expect = |needle: &str, offset: u64, token_type: &str, want: &[&str]| {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col + offset);
            assert_eq!(
                row.token_type, token_type,
                "opened={opened} {needle}: {row:?}"
            );
            assert_eq!(row.modifiers, want, "opened={opened} {needle}: {row:?}");
        };
        expect("P = 1", 0, "variable", &["declaration", "readonly"]);
        expect("T = logic", 0, "typeParameter", &["declaration"]);
        expect("T a", 0, "typeParameter", &[]);
        expect("T t_local", 0, "typeParameter", &[]);
        expect("P[0]", 0, "variable", &["readonly"]);
        expect("L = 2", 0, "variable", &["declaration", "readonly"]);
        expect(".P(L)", 1, "variable", &["readonly", "connectionLabel"]);
        expect(".P(L)", 3, "variable", &["readonly"]);
        expect(".a(v)", 1, "parameter", &["connectionLabel"]);
        expect(".y(o)", 1, "parameter", &["connectionLabel"]);
        for (needle, want) in [
            ("`ifndef", &["controlFlow"][..]),
            ("`define", &[]),
            ("`endif", &["controlFlow"]),
            ("`timescale", &[]),
        ] {
            let (line, col) = position_of(source, needle, 0);
            if opened {
                assert!(
                    !rows
                        .iter()
                        .any(|row| row.line == line && row.character == col),
                    "masked directive {needle} has a token: {rows:?}"
                );
            } else {
                expect(needle, 0, "keyword", want);
            }
        }
    }
    client.shutdown();
}

/// Named, labeled and generate block names are `namespace` (`declaration` at
/// the opening name) at declarations, end labels and references, and
/// definition requests from end labels and references reach the opening name;
/// instance names and block members keep their kinds. Both serving paths.
#[test]
fn lsp_stdio_semantic_tokens_send_block_names_as_namespaces_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root = fixture.root("root-a");
    let path = root.join("block_names.sv");
    let uri = file_uri(&path);
    let source = "module blk_sub(output logic o); assign o = 1'b0; endmodule\n\
module block_names;\n\
  logic q;\n\
  blk_sub u0(.o());\n\
  for (genvar i = 0; i < 2; i++) begin : g\n\
    logic x;\n\
  end : g\n\
  if (1) begin : gi logic y; end\n\
  initial begin : blk\n\
    fork : f\n\
      q = g[0].x;\n\
    join : f\n\
    lbl: begin\n\
      q = block_names.g[1].x;\n\
      q = gi.y | u0.o;\n\
    end : lbl\n\
    l2: assert (q == 0);\n\
    disable lbl;\n\
    disable block_names.blk;\n\
  end : blk\n\
endmodule\n";
    fs::write(&path, source).expect("write block-name fixture");
    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);

    // (needle, occurrence, offset of the name inside the needle).
    let declarations: [(&str, usize, u64); 6] = [
        ("begin : g", 0, 8),
        ("begin : gi", 0, 8),
        ("begin : blk", 0, 8),
        ("fork : f", 0, 7),
        ("lbl: begin", 0, 0),
        ("l2:", 0, 0),
    ];
    // (needle, occurrence, offset, declaration index).
    let uses: [(&str, usize, u64, usize); 9] = [
        ("end : g", 0, 6, 0),
        ("join : f", 0, 7, 3),
        ("end : lbl", 0, 6, 4),
        ("end : blk", 0, 6, 2),
        ("g[0].x", 0, 0, 0),
        (".g[1].x", 0, 1, 0),
        ("gi.y", 0, 0, 1),
        ("disable lbl", 0, 8, 4),
        (".blk;", 0, 1, 2),
    ];
    let at = |(needle, occurrence, offset): (&str, usize, u64)| {
        let (line, col) = position_of(source, needle, occurrence);
        (line, col + offset)
    };
    for opened in [false, true] {
        if opened {
            client.open(&path, source).expect("open block-name buffer");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("block-name semantic token response");
        let rows = semantic_token_rows(&result, &types, &modifiers);
        for site in declarations {
            let (line, col) = at(site);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "namespace", "opened={opened} {site:?}");
            assert_eq!(row.modifiers, ["declaration"], "opened={opened} {site:?}");
        }
        for (needle, occurrence, offset, declaration) in uses {
            let (line, col) = at((needle, occurrence, offset));
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, "namespace", "opened={opened} {needle}");
            assert!(
                row.modifiers.is_empty(),
                "opened={opened} {needle}: {row:?}"
            );
            let (decl_line, decl_col) = at(declarations[declaration]);
            let definition = client
                .request(
                    "textDocument/definition",
                    json!({
                        "textDocument": { "uri": uri },
                        "position": { "line": line, "character": col }
                    }),
                )
                .expect("block-name definition");
            let target = if definition.is_array() {
                definition[0].clone()
            } else {
                definition.clone()
            };
            assert_eq!(
                target.get("uri").and_then(Value::as_str),
                Some(uri.as_str()),
                "opened={opened} {needle}: {definition}"
            );
            assert_eq!(
                start_key(&target["range"]["start"]),
                (decl_line, decl_col),
                "opened={opened} {needle}: {definition}"
            );
            let name = source.lines().nth(decl_line as usize).unwrap()[decl_col as usize..]
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap();
            let hover = param_hover_markup(
                &mut client,
                &uri,
                &json!({ "line": line, "character": col }),
            );
            assert!(
                hover.contains(&format!("scope {name}")),
                "opened={opened} {needle}: {hover}"
            );
        }
        for (needle, token_type) in [
            ("u0(.o", "variable"),
            ("u0.o", "variable"),
            ("x;\n", "variable"),
            ("module block_names", "keyword"),
        ] {
            let (line, col) = position_of(source, needle, 0);
            let row = row_at(&rows, line, col);
            assert_eq!(row.token_type, token_type, "opened={opened} {needle}");
        }
    }
    client.shutdown();
}
