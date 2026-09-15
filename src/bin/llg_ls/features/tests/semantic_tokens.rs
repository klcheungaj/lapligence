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
