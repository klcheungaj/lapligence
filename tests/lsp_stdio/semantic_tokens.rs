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

/// Constant references and type words keep their classes through project and
/// isolated unsaved-buffer analysis, including references to missing children.
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
                    row.token_type, "property",
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
        for name in ["input", "logic", "output", "wire", "inout", "tri", "int"] {
            for occurrence in 0..buffer.match_indices(name).count() {
                let (line, col) = position_of(&buffer, name, occurrence);
                assert_eq!(row_at(&rows, line, col).token_type, "type", "{name}");
            }
        }
        for name in ["module color_top", "assign buffer", "endmodule"] {
            let (line, col) = position_of(&buffer, name, 0);
            assert_eq!(row_at(&rows, line, col).token_type, "keyword", "{name}");
        }
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    client.shutdown();
}

/// Connection labels carry `connectionLabel`; actuals retain their own symbol
/// classification in both cached-project and isolated open-buffer responses.
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
    assert_eq!(clk_row.token_type, "function", "clk row: {clk_row:?}");
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
    let w_mods = expect_label(".W", 0, "property");
    assert!(
        w_mods.iter().any(|name| name == "readonly"),
        "override label keeps its readonly base modifier: {w_mods:?}"
    );
    expect_label(".D", 0, "property");
    // … port labels …
    expect_label(".clk", 0, "function");
    expect_label(".q", 0, "function");
    // … and the connected signals stay plain.
    expect_plain(&iso_buffer, ".D(wa)", 0, 3);
    expect_plain(&iso_buffer, ".clk(wa)", 0, 5);
    expect_plain(&iso_buffer, ".q(tq)", 0, 3);

    // Multi-line instantiation: identical marking on continuation lines.
    expect_label(".W", 1, "property");
    expect_label(".D", 1, "property");
    expect_label(".clk", 1, "function");
    expect_label(".q", 1, "function");
    expect_plain(&iso_buffer, ".clk(wa)", 1, 5);
    expect_plain(&iso_buffer, ".q(tq)", 1, 3);

    client.shutdown();
}
