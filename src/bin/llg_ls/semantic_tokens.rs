//! SystemVerilog semantic-token legend and Slang lexical-token encoding.

use llg::core::tokens::*;
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

const TT_NAMESPACE: u32 = 0;
const TT_TYPE: u32 = 1;
const TT_CLASS: u32 = 2;
const TT_ENUM: u32 = 3;
const TT_INTERFACE: u32 = 4;
const TT_STRUCT: u32 = 5;
const TT_PARAMETER: u32 = 7;
const TT_VARIABLE: u32 = 8;
const TT_PROPERTY: u32 = 9;
const TT_ENUM_MEMBER: u32 = 10;
const TT_FUNCTION: u32 = 12;
const TT_METHOD: u32 = 13;
const TT_MACRO: u32 = 14;
const TT_KEYWORD: u32 = 15;
const TT_STRING: u32 = 18;
const TT_NUMBER: u32 = 19;
const TT_OPERATOR: u32 = 21;
const TM_DECLARATION: u32 = 0;
const TM_READONLY: u32 = 2;
const TM_CONNECTION_LABEL: u32 = 10;
const TM_CONNECTION_LABEL_NAME: SemanticTokenModifier =
    SemanticTokenModifier::new("connectionLabel");

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::CLASS,
            SemanticTokenType::ENUM,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::STRUCT,
            SemanticTokenType::TYPE_PARAMETER,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::EVENT,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
            SemanticTokenType::MACRO,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::MODIFIER,
            SemanticTokenType::COMMENT,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::REGEXP,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFINITION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::STATIC,
            SemanticTokenModifier::DEPRECATED,
            SemanticTokenModifier::ABSTRACT,
            SemanticTokenModifier::ASYNC,
            SemanticTokenModifier::MODIFICATION,
            SemanticTokenModifier::DOCUMENTATION,
            SemanticTokenModifier::DEFAULT_LIBRARY,
            TM_CONNECTION_LABEL_NAME,
        ],
    }
}

pub fn encode(nodes: &[TokenInfo]) -> SemanticTokens {
    let mut items = Vec::with_capacity(nodes.len());
    for node in nodes {
        if node.line == 0 {
            continue;
        }
        let Some((mut token_type, modifiers)) = token_type_for(node.kind) else {
            continue;
        };
        if token_type == TT_KEYWORD && node.name.as_deref().is_some_and(is_type_keyword) {
            token_type = TT_TYPE;
        }
        let length = node
            .name
            .as_ref()
            .filter(|name| !name.is_empty())
            .map(|name| name.encode_utf16().count() as u32)
            .or_else(|| {
                (node.end_line == node.line && node.end_col >= node.col)
                    .then(|| node.end_col - node.col)
            });
        let Some(length) = length.filter(|length| *length != 0) else {
            continue;
        };
        items.push((
            node.line - 1,
            node.col.saturating_sub(1),
            length,
            token_type,
            modifiers,
        ));
    }
    items.sort_by_key(|item| (item.0, item.1));
    items.dedup_by_key(|item| (item.0, item.1));
    let mut previous_line = 0;
    let mut previous_col = 0;
    let data = items
        .into_iter()
        .map(|(line, col, length, token_type, token_modifiers_bitset)| {
            let delta_line = line - previous_line;
            let delta_start = if delta_line == 0 {
                col - previous_col
            } else {
                col
            };
            previous_line = line;
            previous_col = col;
            SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset,
            }
        })
        .collect();
    SemanticTokens {
        result_id: None,
        data,
    }
}

fn is_type_keyword(name: &str) -> bool {
    // Match the extension grammar's built-in, net, and port-direction types.
    matches!(
        name,
        "bit"
            | "logic"
            | "reg"
            | "byte"
            | "shortint"
            | "int"
            | "longint"
            | "integer"
            | "time"
            | "genvar"
            | "shortreal"
            | "real"
            | "realtime"
            | "supply0"
            | "supply1"
            | "tri"
            | "triand"
            | "trior"
            | "trireg"
            | "tri0"
            | "tri1"
            | "uwire"
            | "wire"
            | "wand"
            | "wor"
            | "var"
            | "void"
            | "signed"
            | "unsigned"
            | "string"
            | "const"
            | "chandle"
            | "event"
            | "struct"
            | "union"
            | "enum"
            | "input"
            | "output"
            | "inout"
            | "ref"
            | "parameter"
            | "localparam"
            | "specparam"
    )
}

fn token_type_for(kind: i32) -> Option<(u32, u32)> {
    let (kind, slang_declaration) = token_base_kind(kind);
    let declaration = slang_declaration || matches!(kind, TOKEN_GENVAR_DECL);
    let readonly = matches!(kind, TOKEN_SLANG_PARAMETER | TOKEN_SLANG_PORT);
    let connection = matches!(
        kind,
        TOKEN_SLANG_PORT_CONNECTION_LABEL | TOKEN_SLANG_PARAMETER_CONNECTION_LABEL
    );
    let token_type = match kind {
        TOKEN_SLANG_MODULE | TOKEN_SLANG_PROGRAM => TT_CLASS,
        TOKEN_SLANG_PACKAGE => TT_NAMESPACE,
        TOKEN_SLANG_INTERFACE => TT_INTERFACE,
        TOKEN_SLANG_CLASS => TT_CLASS,
        TOKEN_SLANG_STRUCT | TOKEN_SLANG_UNION => TT_STRUCT,
        TOKEN_SLANG_ENUM => TT_ENUM,
        TOKEN_SLANG_ENUM_MEMBER => TT_ENUM_MEMBER,
        TOKEN_SLANG_TYPE_ALIAS => TT_TYPE,
        TOKEN_SLANG_PARAMETER => TT_PROPERTY,
        TOKEN_SLANG_PORT => TT_PARAMETER,
        TOKEN_SLANG_VARIABLE
        | TOKEN_SLANG_NET
        | TOKEN_SLANG_IDENTIFIER
        | TOKEN_GENVAR_DECL
        | TOKEN_GENVAR_REF => TT_VARIABLE,
        TOKEN_SLANG_FUNCTION | TOKEN_SLANG_TASK => TT_FUNCTION,
        TOKEN_SLANG_METHOD => TT_METHOD,
        TOKEN_SLANG_MACRO => TT_MACRO,
        TOKEN_SLANG_KEYWORD => TT_KEYWORD,
        TOKEN_SLANG_STRING => TT_STRING,
        TOKEN_SLANG_NUMBER => TT_NUMBER,
        TOKEN_SLANG_OPERATOR => TT_OPERATOR,
        TOKEN_SLANG_PORT_CONNECTION_LABEL => TT_FUNCTION,
        TOKEN_SLANG_PARAMETER_CONNECTION_LABEL => TT_PROPERTY,
        _ => return None,
    };
    let mut modifiers = 0;
    if declaration {
        modifiers |= 1 << TM_DECLARATION;
    }
    if readonly || kind == TOKEN_SLANG_PARAMETER_CONNECTION_LABEL {
        modifiers |= 1 << TM_READONLY;
    }
    if connection {
        modifiers |= 1 << TM_CONNECTION_LABEL;
    }
    Some((token_type, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use llg::core::compile::{self, CompileOpts, OwnedSource};

    /// Absolute zero-based line, column, text and token type of one encoded token.
    type Decoded = (u32, u32, String, u32);

    /// Decode every encoded token.
    fn decode(source: &str, nodes: &[TokenInfo]) -> Vec<Decoded> {
        let lines: Vec<&str> = source.lines().collect();
        let (mut line, mut col) = (0u32, 0u32);
        encode(nodes)
            .data
            .iter()
            .map(|token| {
                line += token.delta_line;
                col = if token.delta_line == 0 {
                    col + token.delta_start
                } else {
                    token.delta_start
                };
                let text =
                    lines[line as usize][col as usize..(col + token.length) as usize].to_owned();
                (line, col, text, token.token_type)
            })
            .collect()
    }

    /// Token type of the `nth` whole-word occurrence of `word` on zero-based `line`.
    fn type_at(decoded: &[Decoded], source: &str, line: usize, word: &str, nth: usize) -> u32 {
        let text = source.lines().nth(line).unwrap();
        let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
        let bytes = text.as_bytes();
        let col = text
            .match_indices(word)
            .filter(|(at, _)| {
                (*at == 0 || !is_word(bytes[at - 1]))
                    && bytes.get(at + word.len()).is_none_or(|b| !is_word(*b))
            })
            .nth(nth)
            .unwrap_or_else(|| panic!("no occurrence {nth} of {word} on line {line}: {text}"))
            .0 as u32;
        decoded
            .iter()
            .find(|token| token.0 == line as u32 && token.1 == col)
            .unwrap_or_else(|| panic!("no token for {word} at {line}:{col}; stream {decoded:?}"))
            .3
    }

    /// Token streams of one source under every capture profile that serves tokens.
    fn profiles(source: &str) -> Vec<(String, Vec<Decoded>)> {
        let mut out = Vec::new();
        let parsed = compile::parse_source("/virtual/top.sv", source, &[]).unwrap();
        out.push((
            "isolated".to_owned(),
            decode(source, &parsed.tokens[0].nodes),
        ));
        for library_units in [false, true] {
            let compiled = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit("/virtual/top.sv", source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&compiled.snapshot, &[("/virtual/top.sv", source)]);
            out.push((
                format!("library_units={library_units}"),
                decode(source, &files[0].nodes),
            ));
        }
        out
    }

    #[test]
    fn parameter_references_keep_readonly_highlighting_in_both_capture_profiles() {
        let source = "package sizes; parameter int DEPTH = 4; endpackage\nmodule top #(parameter int WIDTH = 8)(input logic [WIDTH-1:0] data);\nlocalparam int LIMIT = WIDTH + 1;\nwire [LIMIT-1:0] result;\nint memory [sizes::DEPTH];\nassign result = data + LIMIT;\nendmodule";
        for library_units in [false, true] {
            let out = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit("/virtual/top.sv", source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[("/virtual/top.sv", source)]);
            for node in &files[0].nodes {
                if matches!(node.name.as_deref(), Some("WIDTH" | "LIMIT" | "DEPTH")) {
                    let (kind, modifiers) = token_type_for(node.kind).unwrap();
                    assert_eq!(kind, TT_PROPERTY, "library={library_units} {node:?}");
                    assert_ne!(modifiers & (1 << TM_READONLY), 0, "{node:?}");
                }
            }
        }
    }

    #[test]
    fn type_and_direction_keywords_use_type_highlighting() {
        let source = "module top(input logic signed [3:0] a, output wire b, inout tri c);\ninteger count; real value; string label;\nassign b = a[0];\nendmodule";
        let parsed = compile::parse_source("/virtual/top.sv", source, &[]).unwrap();
        let nodes = &parsed.tokens[0].nodes;
        let encoded = encode(nodes);
        assert_eq!(encoded.data.len(), nodes.len());
        for (node, token) in nodes.iter().zip(&encoded.data) {
            match node.name.as_deref().unwrap_or_default() {
                "input" | "logic" | "signed" | "output" | "wire" | "inout" | "tri" | "integer"
                | "real" | "string" => assert_eq!(token.token_type, TT_TYPE, "{node:?}"),
                "module" | "endmodule" | "assign" => {
                    assert_eq!(token.token_type, TT_KEYWORD, "{node:?}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn parameter_highlighting_does_not_leak_into_shadowing_arguments() {
        let source = "module top; parameter int WIDTH = 8;\nfunction automatic int f(input int WIDTH); return WIDTH + 1; endfunction\nendmodule";
        for library_units in [false, true] {
            let out = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit("/virtual/top.sv", source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[("/virtual/top.sv", source)]);
            let arguments: Vec<_> = files[0]
                .nodes
                .iter()
                .filter(|node| node.line == 2 && node.name.as_deref() == Some("WIDTH"))
                .collect();
            assert_eq!(arguments.len(), 2);
            for node in arguments {
                let (kind, modifiers) = token_type_for(node.kind).unwrap();
                assert_ne!(kind, TT_PROPERTY, "library={library_units} {node:?}");
                assert_eq!(modifiers & (1 << TM_READONLY), 0, "{node:?}");
            }
        }
    }

    #[test]
    fn isolated_parameter_actuals_keep_readonly_highlighting_without_the_child_module() {
        let source = "module top; parameter int WIDTH = 8; localparam int LIMIT = 4;\nmissing #(.WIDTH(WIDTH), .LIMIT(LIMIT)) child(.data(LIMIT));\nendmodule";
        let parsed = compile::parse_source("/virtual/top.sv", source, &[]).unwrap();
        let parameters: Vec<_> = parsed.tokens[0]
            .nodes
            .iter()
            .filter(|node| matches!(node.name.as_deref(), Some("WIDTH" | "LIMIT")))
            .collect();
        assert_eq!(parameters.len(), 7);
        for node in parameters {
            let (kind, modifiers) = token_type_for(node.kind).unwrap();
            assert_eq!(kind, TT_PROPERTY, "{node:?}");
            assert_ne!(modifiers & (1 << TM_READONLY), 0, "{node:?}");
        }
    }

    #[test]
    fn missing_module_actuals_use_their_generate_scope() {
        let source = "module top; parameter int WIDTH = 8;\nif (1) begin : nested int WIDTH; missing #(.P(WIDTH)) child(); end\nendmodule";
        for library_units in [false, true] {
            let out = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit("/virtual/top.sv", source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[("/virtual/top.sv", source)]);
            let locals: Vec<_> = files[0]
                .nodes
                .iter()
                .filter(|node| node.line == 2 && node.name.as_deref() == Some("WIDTH"))
                .collect();
            assert_eq!(locals.len(), 2);
            for node in locals {
                let (kind, modifiers) = token_type_for(node.kind).unwrap();
                assert_eq!(kind, TT_VARIABLE, "library={library_units} {node:?}");
                assert_eq!(modifiers & (1 << TM_READONLY), 0, "{node:?}");
            }
        }
    }

    #[test]
    fn module_declaration_and_instantiation_names_use_class_highlighting() {
        let source = "module leaf #(parameter int W = 1)(input logic a);\nendmodule\nprogram prg;\nendprogram\ninterface bus_if;\nendinterface\nmodule top;\n  leaf u0 (.a(1'b0));\n  leaf #(.W(2)) u1 (.a(1'b0));\n  leaf #(2) u3 (.a(1'b0));\n  if (1) begin : g\n    leaf u2 (.a(1'b0));\n  end\n  bus_if b ();\n  prg p0 ();\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            assert_eq!(at(0, "leaf", 0), TT_CLASS, "{profile}");
            assert_eq!(at(2, "prg", 0), TT_CLASS, "{profile}");
            assert_eq!(at(4, "bus_if", 0), TT_INTERFACE, "{profile}");
            assert_eq!(at(6, "top", 0), TT_CLASS, "{profile}");
            for line in [7, 8, 9, 11] {
                assert_eq!(at(line, "leaf", 0), TT_CLASS, "{profile} line {line}");
            }
            assert_eq!(at(13, "bus_if", 0), TT_INTERFACE, "{profile}");
            assert_eq!(at(14, "prg", 0), TT_CLASS, "{profile}");
            for (line, word) in [
                (0, "module"),
                (1, "endmodule"),
                (2, "program"),
                (4, "interface"),
                (6, "module"),
                (10, "if"),
                (10, "begin"),
            ] {
                assert_eq!(at(line, word, 0), TT_KEYWORD, "{profile} {word}");
            }
        }
    }

    #[test]
    fn instantiated_module_names_without_a_visible_definition_use_class_highlighting() {
        let source = "module top;\n  missing_mod u0 ();\n  missing_mod #(.P(1)) u1 ();\n  if (1) begin : g\n    missing_mod u2 ();\n  end\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            for line in [1, 2, 4] {
                assert_eq!(
                    type_at(&tokens, source, line, "missing_mod", 0),
                    TT_CLASS,
                    "{profile} line {line}"
                );
            }
        }
    }

    #[test]
    fn typedef_declarations_and_uses_use_type_highlighting() {
        let source = "package pkg;\n  typedef logic [7:0] byte_t;\nendpackage\nclass C;\n  typedef int cint_t;\n  cint_t m;\nendclass\nmodule leaf #(parameter pkg::byte_t W = 1)(input pkg::byte_t a);\nendmodule\nmodule top;\n  typedef logic [7:0] byte_t;\n  typedef byte_t alias_t;\n  typedef struct packed { byte_t hi; logic lo; } pair_t;\n  typedef enum logic [1:0] { E0, E1 } mode_t;\n  localparam byte_t P = 1;\n  parameter type T = byte_t;\n  byte_t x;\n  pkg::byte_t y;\n  C::cint_t z;\n  pair_t pr;\n  mode_t md;\n  alias_t al [2];\n  function byte_t f(byte_t v);\n    return byte_t'(v);\n  endfunction\n  task t(input byte_t v);\n  endtask\n  localparam int N = $bits(byte_t);\n  localparam byte_t Q = pkg::byte_t'(3);\n  localparam bit EQ = type(byte_t) == type(x);\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            let cases: &[(usize, &str, usize)] = &[
                (1, "byte_t", 0),
                (4, "cint_t", 0),
                (5, "cint_t", 0),
                (7, "byte_t", 0),
                (7, "byte_t", 1),
                (10, "byte_t", 0),
                (11, "alias_t", 0),
                (11, "byte_t", 0),
                (12, "byte_t", 0),
                (12, "pair_t", 0),
                (13, "mode_t", 0),
                (14, "byte_t", 0),
                (15, "byte_t", 0),
                (16, "byte_t", 0),
                (17, "byte_t", 0),
                (18, "cint_t", 0),
                (19, "pair_t", 0),
                (20, "mode_t", 0),
                (21, "alias_t", 0),
                (22, "byte_t", 0),
                (22, "byte_t", 1),
                (23, "byte_t", 0),
                (25, "byte_t", 0),
                (27, "byte_t", 0),
                (28, "byte_t", 0),
                (28, "byte_t", 1),
                (29, "byte_t", 0),
            ];
            for &(line, word, nth) in cases {
                assert_eq!(
                    at(line, word, nth),
                    TT_TYPE,
                    "{profile} {word}#{nth} on line {line}"
                );
            }
        }
    }

    #[test]
    fn objects_declared_with_a_typedef_keep_variable_highlighting() {
        let source = "module top;\n  typedef logic [7:0] byte_t;\n  typedef enum logic [1:0] { E0, E1 } mode_t;\n  byte_t x;\n  mode_t md;\n  initial begin x = 1; md = E1; end\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            assert_eq!(at(3, "x", 0), TT_VARIABLE, "{profile}");
            assert_eq!(at(4, "md", 0), TT_VARIABLE, "{profile}");
            assert_eq!(at(5, "x", 0), TT_VARIABLE, "{profile}");
        }
    }

    #[test]
    fn parameter_declaration_keywords_use_type_highlighting() {
        let source = "module sub #(parameter int A = 1)();\nendmodule\nmodule top #(parameter int P = 1, localparam int Q = 2)(input logic x);\n  parameter type T = int;\n  localparam R = 3;\n  specparam S = 4;\n  sub u ();\n  defparam u.A = 5;\n  if (P > 0) begin : g end\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            for (line, word) in [
                (0, "parameter"),
                (2, "parameter"),
                (2, "localparam"),
                (3, "parameter"),
                (4, "localparam"),
                (5, "specparam"),
            ] {
                assert_eq!(at(line, word, 0), TT_TYPE, "{profile} {word} line {line}");
            }
            for (line, word) in [
                (7, "defparam"),
                (8, "if"),
                (0, "module"),
                (2, "module"),
                (9, "endmodule"),
            ] {
                assert_eq!(
                    at(line, word, 0),
                    TT_KEYWORD,
                    "{profile} {word} line {line}"
                );
            }
            assert_eq!(at(2, "P", 0), TT_PROPERTY, "{profile}");
        }
    }

    #[test]
    fn package_typedefs_use_type_highlighting_through_scope_import_and_type_parameters() {
        let source = "package pkg;\n  typedef logic [7:0] byte_t;\n  typedef enum logic { X0, X1 } en_t;\n  typedef struct packed { byte_t b; } st_t;\nendpackage\nmodule holder #(parameter type T = int)(input T d);\nendmodule\nmodule top (input pkg::byte_t a, output pkg::en_t e);\n  import pkg::*;\n  st_t s;\n  byte_t l;\n  holder #(byte_t) h0 (.d(l));\n  holder #(.T(pkg::byte_t)) h1 (.d(l));\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            let cases: &[(usize, &str, usize)] = &[
                (1, "byte_t", 0),
                (2, "en_t", 0),
                (3, "byte_t", 0),
                (3, "st_t", 0),
                (7, "byte_t", 0),
                (7, "en_t", 0),
                (9, "st_t", 0),
                (10, "byte_t", 0),
                (11, "byte_t", 0),
                (12, "byte_t", 0),
            ];
            for &(line, word, nth) in cases {
                assert_eq!(
                    at(line, word, nth),
                    TT_TYPE,
                    "{profile} {word}#{nth} on line {line}"
                );
            }
            assert_eq!(at(11, "holder", 0), TT_CLASS, "{profile}");
        }
    }
}
