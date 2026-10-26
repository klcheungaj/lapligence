//! Semantic tokens.

use super::*;

#[test]
fn semantic_tokens_require_exact_source_identity() {
    let a = sample_analysis();
    assert!(semantic_tokens_for(&a, hp("/symlink/top.sv"))
        .data
        .is_empty());
    assert!(!semantic_tokens_for(&a, hp("/x/top.sv")).data.is_empty());
}

#[test]
fn semantic_tokens_are_empty_for_a_file_with_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        logical: None,
        file: Some(hp("/x/top.sv").to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, hp("/x/top.sv"));

    // Assert
    assert!(tokens.data.is_empty());
}

#[test]
fn semantic_tokens_remain_available_when_another_file_has_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        logical: None,
        file: Some(hp("/other/top.sv").to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, hp("/x/top.sv"));

    // Assert
    assert!(!tokens.data.is_empty());
}

/// Regression: the semantic-token stream includes the `module` keyword as
/// well as the declaration identifier.
#[test]
fn semantic_tokens_cover_the_module_declaration_keyword() {
    use tower_lsp::lsp_types::SemanticTokenType;

    let _guards = analysis_guards();
    let fixture = resolved_temp_dir(&format!("llg_modkw_{}", std::process::id()));
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

/// Module and typedef names reach clients with the token types C/C++
/// servers use, grouped keyword words are `keyword` with exactly one llg
/// modifier (`typeDefinition`, `direction` or `primitive`), and other keywords
/// stay `keyword` without modifier bits, through the committed-analysis path.
#[test]
fn semantic_tokens_color_module_typedef_and_parameter_words_like_cpp() {
    let _guards = analysis_guards();
    // Path-mode compile names sources by their resolved path (macOS `/var` is
    // `/private/var`), so the request must use the resolved spelling too.
    let fixture = resolved_temp_dir(&format!("llg_cpptypes_{}", std::process::id()));
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

    let raw = semantic_tokens_for(&analysis, &sv.to_string_lossy());
    let decoded = decode_named(&raw);
    let lines: Vec<&str> = source.lines().collect();
    let find = |line: usize, word: &str, nth: usize| {
        let col = lines[line].match_indices(word).nth(nth).unwrap().0 as u32;
        decoded
            .iter()
            .position(|token| token.0 == line as u32 && token.1 == col)
            .unwrap_or_else(|| panic!("no token for {word} on line {line}: {decoded:?}"))
    };
    let type_of = |line: usize, word: &str, nth: usize| decoded[find(line, word, nth)].3.clone();
    let modifiers_of = |line: usize, word: &str, nth: usize| {
        raw.data[find(line, word, nth)].token_modifiers_bitset
    };
    let default_library = crate::semantic_tokens::legend()
        .token_modifiers
        .iter()
        .position(|m| *m == tower_lsp::lsp_types::SemanticTokenModifier::DEFAULT_LIBRARY)
        .expect("defaultLibrary in legend") as u32;
    for (line, word, nth, expected) in [
        (3, "leaf", 0, "class"),
        (1, "byte_t", 0, "type"),
        (3, "byte_t", 0, "type"),
        (5, "top", 0, "class"),
        (5, "module", 0, "keyword"),
        (7, "nib_t", 0, "type"),
        (8, "nib_t", 0, "type"),
        (9, "leaf", 0, "class"),
        (10, "leaf", 0, "class"),
        (10, "nib_t", 0, "type"),
        (11, "defparam", 0, "keyword"),
    ] {
        assert_eq!(type_of(line, word, nth), expected, "{word} on line {line}");
    }
    let modifier_bit = |name: &str| {
        let index = crate::semantic_tokens::legend()
            .token_modifiers
            .iter()
            .position(|m| m.as_str() == name)
            .unwrap_or_else(|| panic!("{name} in legend"));
        1u32 << index
    };
    for (line, word, modifier) in [
        (1, "typedef", "typeDefinition"),
        (1, "logic", "primitive"),
        (3, "parameter", "primitive"),
        (3, "input", "direction"),
        (6, "localparam", "primitive"),
        (6, "int", "primitive"),
        (3, "int", "primitive"),
    ] {
        assert_eq!(type_of(line, word, 0), "keyword", "{word} on line {line}");
        assert_eq!(
            modifiers_of(line, word, 0),
            modifier_bit(modifier),
            "{word} on line {line}"
        );
    }
    for (line, word) in [(5, "module"), (11, "defparam")] {
        assert_eq!(modifiers_of(line, word, 0), 0, "{word} on line {line}");
    }
    for (line, word) in [(1, "byte_t"), (3, "byte_t"), (7, "nib_t"), (8, "nib_t")] {
        assert_eq!(
            modifiers_of(line, word, 0) & (1 << default_library),
            0,
            "user typedef {word} on line {line}"
        );
    }

    let _ = std::fs::remove_dir_all(fixture);
    cleanup_process_shadow();
}

/// Control-flow keywords carry `controlFlow` through the committed-analysis
/// path: words that are control flow everywhere by text, and `unique`,
/// `priority`, `disable` and a case item `default` where the capture marks
/// them, while `disable iff`, block delimiters and declaration keywords keep
/// their own bits.
#[test]
fn semantic_tokens_mark_control_flow_keywords_in_committed_analysis() {
    let _guards = analysis_guards();
    let fixture = resolved_temp_dir(&format!("llg_controlflow_{}", std::process::id()));
    let sv = fixture.join("control_flow.sv");
    let source = "module top(input logic clk, input logic rst);\n  logic [3:0] q;\n  property p; @(posedge clk) disable iff (rst) q == 0; endproperty\n  always @(posedge clk) begin : blk\n    unique case (q) 0: q <= 1; default: q <= 2; endcase\n    priority if (q == 1) q <= 0; else q <= 3;\n    for (int i = 0; i < 2; i++) if (i == 1) break;\n  end\n  initial begin wait (rst); disable blk; end\nendmodule\n";
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
    let raw = semantic_tokens_for(&analysis, &sv.to_string_lossy());
    let decoded = decode_named(&raw);
    let lines: Vec<&str> = source.lines().collect();
    let token = |line: usize, word: &str| {
        let col = lines[line].find(word).unwrap() as u32;
        let index = decoded
            .iter()
            .position(|token| token.0 == line as u32 && token.1 == col)
            .unwrap_or_else(|| panic!("no token for {word} on line {line}: {decoded:?}"));
        (
            decoded[index].3.clone(),
            raw.data[index].token_modifiers_bitset,
        )
    };
    let bit = |name: &str| {
        let index = crate::semantic_tokens::legend()
            .token_modifiers
            .iter()
            .position(|m| m.as_str() == name)
            .unwrap_or_else(|| panic!("{name} in legend"));
        1u32 << index
    };
    let control = bit("controlFlow");
    for (line, word, bits) in [
        (4, "unique", control),
        (4, "case", control),
        (4, "default", control),
        (4, "endcase", control),
        (5, "priority", control),
        (5, "if", control),
        (5, "else", control),
        (6, "for", control),
        (6, "break", control),
        (8, "wait", control),
        (8, "disable", control),
        (2, "disable", 0),
        (2, "iff", 0),
        (3, "always", 0),
        (3, "begin", 0),
        (0, "module", 0),
        (0, "input", bit("direction")),
        (1, "logic", bit("primitive")),
    ] {
        assert_eq!(
            token(line, word),
            ("keyword".to_owned(), bits),
            "{word} on line {line}"
        );
    }

    let _ = std::fs::remove_dir_all(fixture);
    cleanup_process_shadow();
}
