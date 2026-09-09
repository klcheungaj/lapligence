//! Integration coverage for request-local Slang parsing and lexical capture.

use llg::core::{compile, tokens};

#[test]
fn parse_source_collects_only_the_admitted_buffer() {
    let source_text = concat!(
        "// llg-test-fixture: tests/parse_only.rs/opened.sv\r\n",
        "`include \"not_admitted.svh\"\r\n",
        "\r\n",
        "module ParsedInMemory;\r\n",
        "  logic local_signal;\r\n",
        "endmodule\r\n",
    );

    let parsed = compile::parse_source(
        "/virtual/project/opened.sv",
        source_text,
        &["SEMANTIC_ONLY=1".to_owned()],
    )
    .expect("parse admitted source");

    assert_eq!(parsed.tokens.len(), 1);
    let file = &parsed.tokens[0];
    assert_eq!(file.path, "/virtual/project/opened.sv");
    assert!(file.nodes.iter().any(|node| {
        node.name.as_deref() == Some("ParsedInMemory") && node.line == 4 && node.col == 8
    }));
    assert!(file.nodes.iter().any(|node| {
        node.name.as_deref() == Some("module")
            && node.kind == tokens::TOKEN_SLANG_KEYWORD
            && node.line == 4
            && node.col == 1
    }));
    assert!(file
        .nodes
        .iter()
        .all(|node| node.name.as_deref() != Some("not_admitted")));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("not_admitted.svh")
            || diagnostic.message.to_ascii_lowercase().contains("include")
    }));
}

#[test]
fn parse_source_classifies_connection_labels_and_actuals_separately() {
    let source_text = concat!(
        "// llg-test-fixture: tests/parse_only.rs/labels.sv\n",
        "module labels;\n",
        "  logic wa;\n",
        "  logic [7:0] tq;\n",
        "\n",
        "  child #(.W(4), .D(wa)) u_iso (.clk(wa), .q(tq));\n",
        "\n",
        "  child #(\n",
        "    .W(8),\n",
        "    .D(1)\n",
        "  ) u_ml (\n",
        "    .clk(wa),\n",
        "    .q(tq)\n",
        "  );\n",
        "endmodule\n",
    );

    let parsed =
        compile::parse_source("labels.sv", source_text, &[]).expect("parse connection labels");
    let file = parsed.tokens.first().expect("one parsed file");
    let type_at = |line: u32, col: u32| {
        file.nodes
            .iter()
            .find(|node| node.line == line && node.col == col)
            .unwrap_or_else(|| panic!("no token at {line}:{col}: {:?}", file.nodes))
            .kind
    };

    for (line, column) in [(6, 12), (6, 19), (9, 6), (10, 6)] {
        assert_eq!(
            type_at(line, column),
            tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL,
            "parameter connection label at {line}:{column}"
        );
    }
    for (line, column) in [(6, 34), (6, 44), (12, 6), (13, 6)] {
        assert_eq!(
            type_at(line, column),
            tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL,
            "port connection label at {line}:{column}"
        );
    }
    for (line, column) in [(6, 21), (6, 38), (6, 46), (12, 10), (13, 8)] {
        assert_ne!(
            type_at(line, column),
            tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL,
            "actual expression at {line}:{column}"
        );
        assert_ne!(
            type_at(line, column),
            tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL,
            "actual expression at {line}:{column}"
        );
    }
}
