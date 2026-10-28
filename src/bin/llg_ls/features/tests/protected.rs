//! Protected (encrypted) envelopes are opaque regions: no errors from the
//! hidden text, the plaintext header stays a normal definition.
//!
//! Envelopes are synthetic: valid protect pragmas around pseudo-random
//! base64 payloads (no real keys or ciphertext).

use super::*;

/// A decryption envelope (IEEE 1800-2009 34.3.1 output form) with a key
/// block and a data block of `bytes` pseudo-random bytes.
fn envelope(seed: u64, bytes: usize) -> String {
    let key = base64_block(seed, 128);
    let data = base64_block(seed ^ 0x5eed, bytes);
    format!(
        "`pragma protect begin_protected\n\
         `pragma protect encrypt_agent=\"synthetic\", encrypt_agent_info=\"test only\"\n\
         `pragma protect key_keyowner=\"none\", key_keyname=\"none\", key_method=\"rsa\"\n\
         `pragma protect encoding=(enctype=\"base64\", line_length=64, bytes={})\n\
         `pragma protect key_block\n{key}\
         `pragma protect data_method=\"aes128-cbc\"\n\
         `pragma protect encoding=(enctype=\"base64\", line_length=64, bytes={})\n\
         `pragma protect data_block\n{data}\
         `pragma protect end_protected\n",
        encoded_bytes(128),
        encoded_bytes(bytes)
    )
}

/// Bytes that `base64_block(_, bytes)` decodes to (whole 3-byte groups).
fn encoded_bytes(bytes: usize) -> usize {
    bytes.div_ceil(3) * 3
}

/// Pseudo-random base64 text, 64 characters per line.
fn base64_block(seed: u64, bytes: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut state = seed;
    let mut text = String::new();
    for index in 0..encoded_bytes(bytes) / 3 * 4 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        text.push(ALPHABET[(state >> 58) as usize] as char);
        if index % 64 == 63 {
            text.push('\n');
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn analyze_sources(sources: &[(&'static str, &str)]) -> Analysis {
    analyze(&CompileOpts {
        sources: sources
            .iter()
            .map(|(name, text)| compile::OwnedSource::compilation_unit(hp(name), *text))
            .collect(),
        ..Default::default()
    })
}

fn severities(a: &Analysis, file: &'static str) -> Vec<(Option<DiagnosticSeverity>, String)> {
    lsp_diagnostics(a)
        .remove(hp(file))
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| (diagnostic.severity, diagnostic.message))
        .collect()
}

/// Every published diagnostic of `file` is a hint.
fn assert_only_hints(a: &Analysis, file: &'static str) {
    let diagnostics = severities(a, file);
    assert!(
        diagnostics
            .iter()
            .all(|(severity, _)| *severity == Some(DiagnosticSeverity::HINT)),
        "{file}: {diagnostics:?}"
    );
}

fn envelope_hints(a: &Analysis, file: &'static str) -> usize {
    lsp_diagnostics(a)
        .remove(hp(file))
        .unwrap_or_default()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String(PROTECTED_ENVELOPE_CODE.to_owned()))
        })
        .count()
}

fn body_protected() -> String {
    format!(
        "module body_prot #(parameter int W = 8) (\n\
         \x20 input  logic         clk,\n\
         \x20 input  logic [W-1:0] d,\n\
         \x20 output logic [W-1:0] q\n\
         );\n\
         {}endmodule\n",
        envelope(1, 96)
    )
}

const BODY_TOP: &str = "module top;\n\
  logic clk;\n\
  logic [7:0] d, q;\n\
  body_prot #(.W(8)) u0 (.clk(clk), .d(d), .q(q));\n\
  initial $display(\"%h %h\", u0.inner, top.u0.sub.leaf);\n\
endmodule\n";

/// A module whose body is protected and whose header is plaintext is an
/// ordinary indexed definition: no error or warning anywhere, its ports and
/// parameters serve hover, navigation and symbols, and hierarchical names
/// into its instances are hints, not "could not resolve" errors.
#[test]
fn protected_body_keeps_the_plaintext_header_without_errors() {
    let _guards = analysis_guards();
    let body = body_protected();
    let a = analyze_sources(&[("/virtual/body.sv", &body), ("/virtual/top.sv", BODY_TOP)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    assert_only_hints(&a, "/virtual/body.sv");
    assert_eq!(envelope_hints(&a, "/virtual/body.sv"), 1);
    // The skipped-block warnings of the frontend and the unused/undriven
    // findings inside the protected definition are gone; findings about the
    // plaintext top stay.
    assert!(
        a.lint
            .iter()
            .all(|finding| finding.file.as_deref() != Some(hp("/virtual/body.sv"))),
        "{:?}",
        a.lint
    );
    let top = severities(&a, "/virtual/top.sv");
    assert!(
        !top.iter()
            .any(|(severity, _)| *severity == Some(DiagnosticSeverity::ERROR)),
        "{top:?}"
    );
    let hidden: Vec<_> = top
        .iter()
        .filter(|(_, message)| message.contains("could not resolve hierarchical path"))
        .collect();
    assert_eq!(hidden.len(), 2, "{top:?}");
    assert!(
        hidden.iter().all(|(severity, message)| {
            *severity == Some(DiagnosticSeverity::HINT)
                && message.contains("continues into protected source")
        }),
        "{hidden:?}"
    );

    let module = a
        .model
        .modules
        .iter()
        .find(|module| module.name == "body_prot")
        .expect("protected module is a definition");
    assert_eq!(module.file.as_deref(), Some(hp("/virtual/body.sv")));
    let symbols = document_symbols(&a, hp("/virtual/body.sv"));
    assert!(
        symbols.iter().any(|symbol| symbol.name == "body_prot"),
        "{symbols:?}"
    );
    let workspace = workspace_symbols(&a, "body_prot");
    assert!(
        workspace
            .iter()
            .any(|symbol| symbol.location.uri.path().ends_with("body.sv")),
        "{workspace:?}"
    );
    // `.clk` and `.W` at the instantiation resolve to the header.
    let (line, col) = pos_of(BODY_TOP, ".clk", 0);
    let target = definition_at(&a, hp("/virtual/top.sv"), line, col + 1)
        .expect("definition of the .clk label");
    assert!(target.uri.path().ends_with("body.sv"), "{target:?}");
    assert_eq!(target.range.start, Position::new(1, 23), "{target:?}");
    let (line, col) = pos_of(BODY_TOP, ".W", 0);
    let target = definition_at(&a, hp("/virtual/top.sv"), line, col + 1).expect("definition of .W");
    assert_eq!(target.range.start, Position::new(0, 33), "{target:?}");
    let (line, col) = pos_of(BODY_TOP, ".q", 0);
    let hover = hover_at(&a, hp("/virtual/top.sv"), line, col + 1).expect("hover on .q");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("output"), "{}", markup.value);
    let (line, col) = pos_of(BODY_TOP, "body_prot", 0);
    let target = definition_at(&a, hp("/virtual/top.sv"), line, col + 1)
        .expect("definition of the module name");
    assert_eq!(target.range.start, Position::new(0, 7), "{target:?}");
    let completion = completion_at(&a, hp("/virtual/top.sv"), line, col + 4, "  body");
    assert!(
        completion.iter().any(|item| item.label == "body_prot"),
        "{completion:?}"
    );
}

/// The usual vendor layout: a plaintext header followed by an envelope that
/// also holds the end keyword. The missing `endmodule` is a hint and the
/// module keeps its ports.
#[test]
fn protected_end_keyword_is_a_hint_and_the_module_keeps_its_ports() {
    let _guards = analysis_guards();
    let header = format!(
        "module hdr_prot (input logic a, output logic b);\n{}",
        envelope(2, 60)
    );
    let top = "module top;\n  logic a, b;\n  hdr_prot u2 (.a(a), .b(b));\n  initial $display(u2.y);\nendmodule\n";
    let a = analyze_sources(&[("/virtual/hdr.sv", &header), ("/virtual/top.sv", top)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    assert_only_hints(&a, "/virtual/hdr.sv");
    let top_diagnostics = severities(&a, "/virtual/top.sv");
    assert!(
        top_diagnostics.iter().any(|(severity, message)| {
            *severity == Some(DiagnosticSeverity::HINT)
                && message.contains("hierarchical path name 'y'")
        }) && !top_diagnostics
            .iter()
            .any(|(severity, _)| *severity == Some(DiagnosticSeverity::ERROR)),
        "{top_diagnostics:?}"
    );
    let hdr = severities(&a, "/virtual/hdr.sv");
    assert!(
        hdr.iter().any(|(_, message)| message
            .starts_with("expected 'endmodule' (the end of this declaration is inside protected")),
        "{hdr:?}"
    );
    let (line, col) = pos_of(top, ".b", 0);
    let target = definition_at(&a, hp("/virtual/top.sv"), line, col + 1)
        .expect("definition of the .b label");
    assert!(target.uri.path().ends_with("hdr.sv"), "{target:?}");
    assert!(
        a.model
            .modules
            .iter()
            .any(|module| module.name == "hdr_prot"),
        "{:?}",
        a.model.modules
    );
}

/// An envelope outside every declaration may define whole modules, so an
/// instantiation of an unknown module becomes a hint instead of an error.
#[test]
fn whole_module_envelope_makes_unknown_instantiations_opaque() {
    let _guards = analysis_guards();
    let whole = envelope(3, 200);
    let top = "module top;\n  logic [7:0] d, q;\n  whole_prot #(.W(8)) u1 (.a(d), .b(q));\n  initial $display(\"%h\", u1.x);\nendmodule\n";
    let a = analyze_sources(&[("/virtual/whole.sv", &whole), ("/virtual/top.sv", top)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    let hints = lsp_diagnostics(&a)
        .remove(hp("/virtual/whole.sv"))
        .unwrap_or_default();
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert_eq!(hints[0].severity, Some(DiagnosticSeverity::HINT));
    assert!(
        hints[0]
            .message
            .contains("definitions inside it are unknown"),
        "{hints:?}"
    );
    assert_eq!(hints[0].range.start, Position::new(0, 0));
    let top_diagnostics = severities(&a, "/virtual/top.sv");
    let unknown: Vec<_> = top_diagnostics
        .iter()
        .filter(|(_, message)| message.contains("unknown module 'whole_prot'"))
        .collect();
    assert_eq!(unknown.len(), 1, "{top_diagnostics:?}");
    assert_eq!(unknown[0].0, Some(DiagnosticSeverity::HINT));
    assert!(
        !top_diagnostics
            .iter()
            .any(|(severity, _)| *severity == Some(DiagnosticSeverity::ERROR)),
        "{top_diagnostics:?}"
    );
}

/// Envelopes explain only what their hidden text can cause: an unknown
/// module without an envelope outside declarations, a missing member of a
/// plaintext instance and a port the protected header does not declare stay
/// errors.
#[test]
fn protected_source_keeps_unrelated_errors() {
    let _guards = analysis_guards();
    let body = body_protected();
    let top = "module plain(input logic a);\nendmodule\n\
module top;\n\
  logic clk;\n\
  logic [7:0] d, q;\n\
  body_prot u0 (.clk(clk), .d(d), .q(q), .nope(clk));\n\
  plain p0 (.a(clk));\n\
  missing_mod m0 ();\n\
  initial $display(\"%h\", p0.inner);\n\
endmodule\n";
    let a = analyze_sources(&[("/virtual/body.sv", &body), ("/virtual/top.sv", top)]);
    assert!(!a.is_valid());
    let errors: Vec<_> = severities(&a, "/virtual/top.sv")
        .into_iter()
        .filter(|(severity, _)| *severity == Some(DiagnosticSeverity::ERROR))
        .map(|(_, message)| message)
        .collect();
    for expected in [
        "nope",
        "unknown module 'missing_mod'",
        "could not resolve hierarchical path name 'inner'",
    ] {
        assert!(
            errors.iter().any(|message| message.contains(expected)),
            "missing {expected:?} in {errors:?}"
        );
    }
    assert_only_hints(&a, "/virtual/body.sv");
}

/// The encoded blocks are one opaque region: no semantic token inside them.
#[test]
fn encoded_blocks_have_no_semantic_tokens() {
    let _guards = analysis_guards();
    let body = body_protected();
    let a = analyze_sources(&[("/virtual/body.sv", &body), ("/virtual/top.sv", BODY_TOP)]);
    let block_lines: Vec<u32> = body
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            *line != "endmodule"
                && !line.is_empty()
                && line
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
        })
        .map(|(index, _)| index as u32)
        .collect();
    assert!(block_lines.len() >= 4, "{body}");
    let tokens = semantic_tokens_for(&a, hp("/virtual/body.sv"));
    let mut line = 0;
    let mut covered = Vec::new();
    for token in &tokens.data {
        line += token.delta_line;
        covered.push(line);
    }
    assert!(
        covered.contains(&0),
        "the plaintext header keeps its tokens"
    );
    assert!(
        block_lines.iter().all(|line| !covered.contains(line)),
        "tokens inside encoded blocks: {covered:?} vs {block_lines:?}"
    );
}

/// The Verilog-XL `protected form, an envelope without its closing
/// directive and an encryption envelope (plaintext) parse without errors.
#[test]
fn legacy_truncated_and_encryption_envelopes_parse_without_errors() {
    let _guards = analysis_guards();
    let legacy = format!(
        "module legacy_prot(a, b);\n  input a;\n  output b;\n`protected\n{}`endprotected\nendmodule\n",
        base64_block(4, 90)
    );
    let full = envelope(5, 120);
    let cut = full
        .find("`pragma protect end_protected")
        .expect("closing directive");
    let truncated = format!(
        "module cut_prot(input logic a, output logic b);\n{}",
        &full[..cut]
    );
    let encrypt = "module enc_env(input logic a, output logic b);\n\
`pragma protect begin\n\
  assign b = a;\n\
`pragma protect end\n\
endmodule\n";
    let top = "module top;\n  logic a, b, c, d, e, f;\n  legacy_prot l0 (a, b);\n  cut_prot c0 (.a(c), .b(d));\n  enc_env e0 (.a(e), .b(f));\nendmodule\n";
    let a = analyze_sources(&[
        ("/virtual/legacy.v", &legacy),
        ("/virtual/cut.sv", &truncated),
        ("/virtual/enc.sv", encrypt),
        ("/virtual/top.sv", top),
    ]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    assert_only_hints(&a, "/virtual/legacy.v");
    assert_eq!(envelope_hints(&a, "/virtual/legacy.v"), 1);
    assert_only_hints(&a, "/virtual/cut.sv");
    let cut_hints = severities(&a, "/virtual/cut.sv");
    assert!(
        cut_hints
            .iter()
            .any(|(_, message)| message.contains("not closed before the end of the file")),
        "{cut_hints:?}"
    );
    // Plaintext inside an encryption envelope is ordinary, checked source.
    assert_eq!(envelope_hints(&a, "/virtual/enc.sv"), 0);
    assert!(severities(&a, "/virtual/enc.sv").is_empty());
    for module in ["legacy_prot", "cut_prot", "enc_env"] {
        assert!(
            a.model
                .modules
                .iter()
                .any(|candidate| candidate.name == module),
            "{module}: {:?}",
            a.model.modules
        );
    }
}
