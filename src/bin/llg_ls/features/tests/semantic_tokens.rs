//! Semantic tokens.

use super::*;

#[test]
fn semantic_tokens_require_exact_source_identity() {
    let a = sample_analysis();
    assert!(semantic_tokens_for(&a, "/symlink/top.sv").data.is_empty());
    assert!(!semantic_tokens_for(&a, "/x/top.sv").data.is_empty());
}

#[test]
fn semantic_tokens_are_empty_for_a_file_with_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

    // Assert
    assert!(tokens.data.is_empty());
}

#[test]
fn semantic_tokens_remain_available_when_another_file_has_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        file: Some("/other/top.sv".to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

    // Assert
    assert!(!tokens.data.is_empty());
}

/// Regression: the semantic-token stream includes the `module` keyword as
/// well as the declaration identifier.
#[test]
fn semantic_tokens_cover_the_module_declaration_keyword() {
    use tower_lsp::lsp_types::SemanticTokenType;

    let _guards = analysis_guards();
    let fixture = std::env::temp_dir().join(format!("llg_modkw_{}", std::process::id()));
    let rtl = fixture.join("rtl");
    std::fs::create_dir_all(&rtl).expect("create fixture tree");
    let sv = rtl.join("modkw.sv");
    std::fs::write(
        &sv,
        "module mod_kw(input logic clk);\n  wire w;\nendmodule\n",
    )
    .expect("write design");

    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let analysis = analyze_with_config(&opts, &LintConfig::default());
    assert!(
        analysis.is_valid(),
        "analysis failed: {:?}",
        analysis.diagnostics
    );

    let legend = crate::semantic_tokens::legend();
    let keyword_index = legend
        .token_types
        .iter()
        .position(|t| *t == SemanticTokenType::KEYWORD)
        .expect("keyword type in legend") as u32;

    // Decode the delta-encoded stream back to absolute (line, col, len).
    let tokens = semantic_tokens_for(&analysis, &sv.to_string_lossy());
    let mut line = 0u32;
    let mut col = 0u32;
    let mut keywords: Vec<(u32, u32, u32)> = Vec::new();
    for token in &tokens.data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        if token.token_type == keyword_index {
            keywords.push((line, col, token.length));
        }
    }
    assert!(
        keywords.contains(&(0, 0, "module".len() as u32)),
        "expected a keyword token over `module` at 0:0, got {keywords:?}"
    );
    assert!(
        keywords.contains(&(2, 0, "endmodule".len() as u32)),
        "expected a keyword token over `endmodule` at 2:0, got {keywords:?}"
    );

    cleanup_process_shadow();
    let _ = std::fs::remove_dir_all(fixture);
}

/// Decode a delta-encoded stream into `(line, col, length, legend name)`.
fn decode_named(tokens: &SemanticTokens) -> Vec<(u32, u32, u32, String)> {
    let legend = crate::semantic_tokens::legend();
    let (mut line, mut col) = (0u32, 0u32);
    tokens
        .data
        .iter()
        .map(|token| {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            let name = legend.token_types[token.token_type as usize]
                .as_str()
                .to_owned();
            (line, col, token.length, name)
        })
        .collect()
}

/// Module and typedef names, and the parameter keywords, reach clients with
/// the token types C/C++ servers use, through the committed-analysis path.
#[test]
fn semantic_tokens_color_module_typedef_and_parameter_words_like_cpp() {
    let _guards = analysis_guards();
    let fixture = std::env::temp_dir().join(format!("llg_cpptypes_{}", std::process::id()));
    std::fs::create_dir_all(&fixture).expect("create fixture tree");
    let sv = fixture.join("cpp_types.sv");
    let source = "package pkg;\n  typedef logic [7:0] byte_t;\nendpackage\nmodule leaf #(parameter int W = 1)(input pkg::byte_t a);\nendmodule\nmodule top;\n  localparam int L = 2;\n  typedef logic [3:0] nib_t;\n  nib_t n;\n  leaf u0 (.a(8'd0));\n  leaf #(.W(L)) u1 (.a(nib_t'(1)));\n  defparam u0.W = 3;\nendmodule\n";
    std::fs::write(&sv, source).expect("write design");
    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let analysis = analyze_with_config(&opts, &LintConfig::default());
    assert!(
        analysis.is_valid(),
        "analysis failed: {:?}",
        analysis.diagnostics
    );

    let decoded = decode_named(&semantic_tokens_for(&analysis, &sv.to_string_lossy()));
    let lines: Vec<&str> = source.lines().collect();
    let type_of = |line: usize, word: &str, nth: usize| {
        let col = lines[line].match_indices(word).nth(nth).unwrap().0 as u32;
        decoded
            .iter()
            .find(|token| token.0 == line as u32 && token.1 == col)
            .unwrap_or_else(|| panic!("no token for {word} on line {line}: {decoded:?}"))
            .3
            .clone()
    };
    for (line, word, nth, expected) in [
        (3, "leaf", 0, "class"),
        (3, "parameter", 0, "type"),
        (1, "byte_t", 0, "type"),
        (3, "byte_t", 0, "type"),
        (5, "top", 0, "class"),
        (5, "module", 0, "keyword"),
        (6, "localparam", 0, "type"),
        (7, "nib_t", 0, "type"),
        (8, "nib_t", 0, "type"),
        (9, "leaf", 0, "class"),
        (10, "leaf", 0, "class"),
        (10, "nib_t", 0, "type"),
        (11, "defparam", 0, "keyword"),
    ] {
        assert_eq!(type_of(line, word, nth), expected, "{word} on line {line}");
    }

    let _ = std::fs::remove_dir_all(fixture);
    cleanup_process_shadow();
}
