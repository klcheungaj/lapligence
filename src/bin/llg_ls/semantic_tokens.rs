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
const TT_MODIFIER: u32 = 16;
const TT_STRING: u32 = 18;
const TT_NUMBER: u32 = 19;
const TT_OPERATOR: u32 = 21;
const TM_DECLARATION: u32 = 0;
const TM_READONLY: u32 = 2;
const TM_DEFAULT_LIBRARY: u32 = 9;
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
        if is_unresolved_identifier(node.kind) {
            continue;
        }
        let Some((mut token_type, modifiers)) = token_type_for(node.kind) else {
            continue;
        };
        if token_type == TT_KEYWORD {
            if let Some(KeywordClass::Primitive | KeywordClass::Declaration) =
                node.name.as_deref().map(keyword_class)
            {
                token_type = TT_MODIFIER;
            }
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

/// A reference the capture could not resolve to any symbol is lexically a
/// plain identifier. It gets no token, so the editor leaves it uncolored
/// instead of painting a name that has no definition as a variable. Plain
/// identifier declarations (instance names, labels) and bound references to
/// them (`TOKEN_SLANG_BOUND_IDENTIFIER`) are separate kinds and keep theirs.
fn is_unresolved_identifier(kind: i32) -> bool {
    kind == TOKEN_SLANG_IDENTIFIER
}

/// How a keyword token is presented. Only keyword-derived tokens are
/// retyped; user type names keep their own token types.
#[derive(Debug, PartialEq, Eq)]
enum KeywordClass {
    /// Built-in data and net types plus `struct`/`union`/`enum`.
    Primitive,
    /// Port directions, kind words and declaration qualifiers.
    Declaration,
    /// Every other keyword stays `keyword`.
    Other,
}

/// Classify by keyword text, so words that are keywords in some editions only
/// (for example `uwire`) need no edition knowledge here. `Primitive` and
/// `Declaration` both encode as `modifier` without modifier bits today (VS Code
/// maps it to storage.modifier, the C++ `int`/`const` look, apart from
/// module/class/typedef names); they stay separate so a custom `primitive`
/// token type is a one-line change in `encode`.
fn keyword_class(name: &str) -> KeywordClass {
    match name {
        "bit" | "logic" | "reg" | "byte" | "shortint" | "int" | "longint" | "integer" | "time"
        | "shortreal" | "real" | "realtime" | "string" | "chandle" | "event" | "void"
        | "signed" | "unsigned" | "genvar" | "supply0" | "supply1" | "tri" | "triand" | "trior"
        | "trireg" | "tri0" | "tri1" | "uwire" | "wire" | "wand" | "wor" | "struct" | "union"
        | "enum" => KeywordClass::Primitive,
        "input" | "output" | "inout" | "ref" | "parameter" | "localparam" | "specparam"
        | "const" | "var" | "typedef" | "default" | "automatic" | "static" => {
            KeywordClass::Declaration
        }
        _ => KeywordClass::Other,
    }
}

fn token_type_for(kind: i32) -> Option<(u32, u32)> {
    let (kind, slang_declaration) = token_base_kind(kind);
    let declaration = slang_declaration || matches!(kind, TOKEN_GENVAR_DECL);
    let readonly = matches!(
        kind,
        TOKEN_SLANG_PARAMETER | TOKEN_SLANG_PORT | TOKEN_SLANG_ENUM_MEMBER
    );
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
        | TOKEN_SLANG_BOUND_IDENTIFIER
        | TOKEN_GENVAR_DECL
        | TOKEN_GENVAR_REF => TT_VARIABLE,
        TOKEN_SLANG_FUNCTION | TOKEN_SLANG_TASK | TOKEN_SLANG_SYSTEM_SUBROUTINE => TT_FUNCTION,
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
    if kind == TOKEN_SLANG_SYSTEM_SUBROUTINE {
        modifiers |= 1 << TM_DEFAULT_LIBRARY;
    }
    Some((token_type, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_paths::host_path as hp;
    use llg::core::compile::{self, CompileOpts, OwnedSource};

    /// Absolute zero-based line, column, text, token type and modifier bitset
    /// of one encoded token.
    type Decoded = (u32, u32, String, u32, u32);

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
                (
                    line,
                    col,
                    text,
                    token.token_type,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }

    /// Token of the `nth` whole-word occurrence of `word` on zero-based `line`.
    fn token_at<'a>(
        decoded: &'a [Decoded],
        source: &str,
        line: usize,
        word: &str,
        nth: usize,
    ) -> &'a Decoded {
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
    }

    /// Token type of the `nth` whole-word occurrence of `word` on zero-based `line`.
    fn type_at(decoded: &[Decoded], source: &str, line: usize, word: &str, nth: usize) -> u32 {
        token_at(decoded, source, line, word, nth).3
    }

    /// Token streams of one source under every capture profile that serves tokens.
    fn profiles(source: &str) -> Vec<(String, Vec<Decoded>)> {
        let mut out = Vec::new();
        let parsed = compile::parse_source(hp("/virtual/top.sv"), source, &[]).unwrap();
        out.push((
            "isolated".to_owned(),
            decode(source, &parsed.tokens[0].nodes),
        ));
        for library_units in [false, true] {
            let compiled = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit(hp("/virtual/top.sv"), source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&compiled.snapshot, &[(hp("/virtual/top.sv"), source)]);
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
                sources: vec![OwnedSource::compilation_unit(hp("/virtual/top.sv"), source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[(hp("/virtual/top.sv"), source)]);
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
    fn primitive_and_declaration_words_are_plain_modifiers() {
        let source = "module top(input logic signed [3:0] a, output wire b, inout tri c);\ninteger count; real value; string label;\nassign b = a[0];\nendmodule";
        let parsed = compile::parse_source(hp("/virtual/top.sv"), source, &[]).unwrap();
        let nodes = &parsed.tokens[0].nodes;
        let encoded = encode(nodes);
        assert_eq!(encoded.data.len(), nodes.len());
        let mut seen = 0;
        for (node, token) in nodes.iter().zip(&encoded.data) {
            let expected = match node.name.as_deref().unwrap_or_default() {
                "logic" | "signed" | "wire" | "tri" | "integer" | "real" | "string" => {
                    (TT_MODIFIER, 0)
                }
                "input" | "output" | "inout" => (TT_MODIFIER, 0),
                "module" | "endmodule" | "assign" => (TT_KEYWORD, 0),
                _ => continue,
            };
            seen += 1;
            assert_eq!(
                (token.token_type, token.token_modifiers_bitset),
                expected,
                "{node:?}"
            );
        }
        assert_eq!(seen, 13);
    }

    #[test]
    fn keyword_classes_cover_the_decided_word_lists() {
        for word in [
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
            "struct",
            "union",
            "enum",
        ] {
            assert_eq!(keyword_class(word), KeywordClass::Primitive, "{word}");
        }
        for word in [
            "input",
            "output",
            "inout",
            "ref",
            "parameter",
            "localparam",
            "specparam",
            "const",
            "var",
            "typedef",
            "default",
            "automatic",
            "static",
        ] {
            assert_eq!(keyword_class(word), KeywordClass::Declaration, "{word}");
        }
        for word in [
            "module", "type", "packed", "assign", "begin", "initial", "always", "case",
        ] {
            assert_eq!(keyword_class(word), KeywordClass::Other, "{word}");
        }
    }

    #[test]
    fn every_decided_word_row_encodes_modifier_without_modifier_bits() {
        let source = "module top;\n  const int ci = 1;\n  var logic vl;\n  genvar g;\n  supply0 s0;\n  uwire uw;\n  event ev;\n  chandle ch;\n  typedef struct packed { bit [1:0] f; } st_t;\n  typedef union packed { bit [1:0] f; } un_t;\n  typedef enum { E0 } en_t;\n  specparam SP = 1;\n  localparam int LP = 1;\n  function void fn(ref int r); endfunction\n  st_t user_s;\n  function automatic int fa(input int p); return p; endfunction\n  static int sv;\n  always @* case (sv) 0: ; default: ; endcase\nendmodule\n";
        let default_library = 1 << TM_DEFAULT_LIBRARY;
        for (profile, tokens) in profiles(source) {
            let token = |line, word| token_at(&tokens, source, line, word, 0);
            for (line, word) in [
                (1, "int"),
                (2, "logic"),
                (3, "genvar"),
                (4, "supply0"),
                (5, "uwire"),
                (6, "event"),
                (7, "chandle"),
                (8, "bit"),
                (13, "void"),
                (8, "struct"),
                (9, "union"),
                (10, "enum"),
            ] {
                let t = token(line, word);
                assert_eq!((t.3, t.4), (TT_MODIFIER, 0), "{profile} primitive {word}");
            }
            for (line, word) in [
                (1, "const"),
                (2, "var"),
                (11, "specparam"),
                (12, "localparam"),
                (13, "ref"),
                (8, "typedef"),
                (15, "automatic"),
                (16, "static"),
                (17, "default"),
            ] {
                let t = token(line, word);
                assert_eq!((t.3, t.4), (TT_MODIFIER, 0), "{profile} declaration {word}");
            }
            for (line, word) in [(0, "module"), (17, "always"), (17, "case")] {
                let t = token(line, word);
                assert_eq!((t.3, t.4), (TT_KEYWORD, 0), "{profile} keyword {word}");
            }
            let user = token(14, "st_t");
            assert_eq!(user.3, TT_TYPE, "{profile} user typedef use");
            assert_eq!(user.4 & default_library, 0, "{profile} user typedef use");
            let declared = token(8, "st_t");
            assert_eq!(declared.3, TT_TYPE, "{profile} typedef decl");
            assert_eq!(declared.4 & default_library, 0, "{profile} typedef decl");
        }
    }

    #[test]
    fn unresolved_identifier_references_get_no_token_in_every_capture_profile() {
        let source = "module top;\n  logic [3:0] v, w;\n  assign w = v + ghost;\n  always_comb v = w;\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let has_token = |line: u32, word: &str| {
                let col = source
                    .lines()
                    .nth(line as usize)
                    .unwrap()
                    .find(word)
                    .unwrap() as u32;
                tokens.iter().any(|token| token.0 == line && token.1 == col)
            };
            assert!(!has_token(2, "ghost"), "{profile}: {tokens:?}");
            assert!(has_token(1, "v"), "{profile}: declaration keeps its token");
            let declared = token_at(&tokens, source, 3, "v", 0);
            assert_eq!(declared.3, TT_VARIABLE, "{profile}");
            assert_eq!(
                token_at(&tokens, source, 3, "w", 0).3,
                TT_VARIABLE,
                "{profile}"
            );
        }
    }

    /// Names that resolve keep their real kinds, also inside a block that an
    /// undeclared name makes invalid: package qualifiers, `disable` labels,
    /// hierarchical path steps and system subroutine names. Only the
    /// undeclared name has no token.
    #[test]
    fn resolved_names_keep_tokens_in_erroneous_blocks_and_qualified_paths_in_every_capture_profile()
    {
        let source =
            "package pk; int pv; function int pf(int a); return a; endfunction endpackage\n\
interface ifc; logic sig; endinterface\n\
module sub(input logic i, output logic o); assign o = i; endmodule\n\
module top;\n\
  logic [3:0] v, w;\n\
  ifc bus();\n\
  sub u0(.i(v[0]), .o(w[0]));\n\
  for (genvar g = 0; g < 2; g++) begin : gb logic q; end\n\
  initial begin : bad\n\
    v = ghost + w;\n\
    w = pk::pf(v) + pk::pv + gb[0].q + u0.o + bus.sig + $bits(v);\n\
    disable bad;\n\
  end\n\
  initial begin : good\n\
    w = pk::pf(v) + pk::pv + gb[0].q + u0.o + bus.sig + $bits(v);\n\
    $display(\"%0d\", v);\n\
    disable good;\n\
  end\n\
endmodule\n";
        let default_library = 1 << TM_DEFAULT_LIBRARY;
        let readonly = 1 << TM_READONLY;
        for (profile, tokens) in profiles(source) {
            let find = |line: usize, word: &str, nth: usize| {
                let text = source.lines().nth(line).unwrap();
                let col = text
                    .match_indices(word)
                    .filter(|(at, _)| {
                        let bytes = text.as_bytes();
                        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
                        (*at == 0 || !is_word(bytes[at - 1]))
                            && bytes.get(at + word.len()).is_none_or(|b| !is_word(*b))
                    })
                    .nth(nth)
                    .unwrap()
                    .0 as u32;
                tokens
                    .iter()
                    .find(|t| t.0 == line as u32 && t.1 == col)
                    .cloned()
            };
            let token = |line: usize, word: &str, nth: usize| {
                find(line, word, nth).unwrap_or_else(|| {
                    panic!("{profile}: no token for {word}#{nth} on line {line}")
                })
            };
            let col = source.lines().nth(9).unwrap().find("ghost").unwrap() as u32;
            assert!(
                !tokens.iter().any(|t| t.0 == 9 && t.1 == col),
                "{profile}: undeclared name got a token: {tokens:?}"
            );
            assert_eq!(token(9, "v", 0).3, TT_VARIABLE, "{profile}: bad-block v");
            assert_eq!(token(9, "w", 0).3, TT_VARIABLE, "{profile}: bad-block w");
            for line in [10, 14] {
                let expect = [
                    ("w", 0, TT_VARIABLE, 0),
                    ("pk", 0, TT_NAMESPACE, 0),
                    ("pf", 0, TT_FUNCTION, 0),
                    ("v", 0, TT_VARIABLE, 0),
                    ("pk", 1, TT_NAMESPACE, 0),
                    ("pv", 0, TT_VARIABLE, 0),
                    ("gb", 0, TT_VARIABLE, 0),
                    ("q", 0, TT_VARIABLE, 0),
                    ("u0", 0, TT_VARIABLE, 0),
                    ("o", 0, TT_PARAMETER, readonly),
                    ("bus", 0, TT_VARIABLE, 0),
                    ("sig", 0, TT_VARIABLE, 0),
                    ("$bits", 0, TT_FUNCTION, default_library),
                    ("v", 1, TT_VARIABLE, 0),
                ];
                for (word, nth, token_type, modifiers) in expect {
                    // Library-unit capture elaborates no hierarchy: Slang leaves
                    // hierarchical names in an uninstantiated body unresolved,
                    // so those steps may have no token there.
                    let hierarchical = matches!(word, "gb" | "q" | "u0" | "o" | "bus" | "sig");
                    if hierarchical && profile == "library_units=true" {
                        if let Some(t) = find(line, word, nth) {
                            assert_eq!((t.3, t.4), (token_type, modifiers), "{profile} {word}");
                        }
                        continue;
                    }
                    let t = token(line, word, nth);
                    assert_eq!(
                        (t.3, t.4),
                        (token_type, modifiers),
                        "{profile} line {line} {word}#{nth}"
                    );
                }
            }
            let display = token(15, "$display", 0);
            assert_eq!(
                (display.3, display.4),
                (TT_FUNCTION, default_library),
                "{profile}"
            );
            for (line, label) in [(11, "bad"), (16, "good")] {
                let t = token(line, label, 0);
                assert_eq!((t.3, t.4), (TT_VARIABLE, 0), "{profile} disable {label}");
            }
        }
    }

    #[test]
    fn parameter_highlighting_does_not_leak_into_shadowing_arguments() {
        let source = "module top; parameter int WIDTH = 8;\nfunction automatic int f(input int WIDTH); return WIDTH + 1; endfunction\nendmodule";
        for library_units in [false, true] {
            let out = compile::compile(&CompileOpts {
                library_units,
                sources: vec![OwnedSource::compilation_unit(hp("/virtual/top.sv"), source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[(hp("/virtual/top.sv"), source)]);
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
        let parsed = compile::parse_source(hp("/virtual/top.sv"), source, &[]).unwrap();
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
                sources: vec![OwnedSource::compilation_unit(hp("/virtual/top.sv"), source)],
                ..Default::default()
            })
            .unwrap();
            let files = from_slang_snapshot(&out.snapshot, &[(hp("/virtual/top.sv"), source)]);
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
    fn parameter_declaration_keywords_use_modifier_and_keyword_highlighting() {
        let source = "module sub #(parameter int A = 1)();\nendmodule\nmodule top #(parameter int P = 1, localparam int Q = 2)(input logic x);\n  parameter type T = int;\n  localparam R = 3;\n  specparam S = 4;\n  sub u ();\n  defparam u.A = 5;\n  if (P > 0) begin : g end\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
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
            for (line, word) in [
                (0, "parameter"),
                (2, "parameter"),
                (2, "localparam"),
                (3, "parameter"),
                (4, "localparam"),
                (5, "specparam"),
            ] {
                let token = token_at(&tokens, source, line, word, 0);
                assert_eq!(token.3, TT_MODIFIER, "{profile} {word} line {line}");
                assert_eq!(token.4, 0, "{profile} {word} line {line}");
            }
            assert_eq!(at(2, "P", 0), TT_PROPERTY, "{profile}");
        }
    }

    #[test]
    fn enum_members_are_readonly_enum_members_at_declarations_and_references() {
        let source = "package pkg;\n  typedef enum logic [1:0] { PA, PB } pe_t;\n  enum { ANON0, ANON1 = 5 } anon;\n  typedef enum { R[2], S[1:3] } rng_t;\nendpackage\nmodule top;\n  import pkg::*;\n  typedef enum logic [1:0] { IDLE, RUN } state_t;\n  enum { X0, X1 } local_anon;\n  state_t st;\n  pkg::pe_t pe;\n  pkg::rng_t rv;\n  initial begin\n    st = IDLE;\n    pe = pkg::PB;\n    if (pe == PA) st = RUN;\n    local_anon = X1;\n    pkg::anon = pkg::ANON1;\n    rv = R1;\n    rv = S3;\n  end\nendmodule\n";
        let declarations: &[(usize, &str)] = &[
            (1, "PA"),
            (1, "PB"),
            (2, "ANON0"),
            (2, "ANON1"),
            (3, "R"),
            (3, "S"),
            (7, "IDLE"),
            (7, "RUN"),
            (8, "X0"),
            (8, "X1"),
        ];
        let references: &[(usize, &str, usize)] = &[
            (13, "IDLE", 0),
            (14, "PB", 0),
            (15, "PA", 0),
            (15, "RUN", 0),
            (16, "X1", 0),
            (17, "ANON1", 0),
            (18, "R1", 0),
            (19, "S3", 0),
        ];
        for (profile, tokens) in profiles(source) {
            for &(line, word) in declarations {
                let token = token_at(&tokens, source, line, word, 0);
                assert_eq!(token.3, TT_ENUM_MEMBER, "{profile} {word} line {line}");
                assert_eq!(
                    token.4,
                    (1 << TM_DECLARATION) | (1 << TM_READONLY),
                    "{profile} {word} line {line}"
                );
            }
            for &(line, word, nth) in references {
                let token = token_at(&tokens, source, line, word, nth);
                assert_eq!(token.3, TT_ENUM_MEMBER, "{profile} {word} line {line}");
                assert_eq!(token.4, 1 << TM_READONLY, "{profile} {word} line {line}");
            }
            for (line, nth) in [(14, 0), (17, 0), (17, 1)] {
                let text = source.lines().nth(line).unwrap();
                let col = text.match_indices("pkg").nth(nth).unwrap().0 as u32;
                if let Some(qualifier) = tokens.iter().find(|t| t.0 == line as u32 && t.1 == col) {
                    assert_ne!(qualifier.3, TT_ENUM_MEMBER, "{profile} line {line}");
                    assert_eq!(qualifier.4 & (1 << TM_READONLY), 0, "{profile} line {line}");
                }
            }
        }
    }

    #[test]
    fn enum_members_in_other_scopes_are_enum_members() {
        let source = "class C;\n  typedef enum { K0, K1 } kind_t;\n  kind_t k = K1;\nendclass\nmodule top;\n  function automatic int pick(input int v);\n    enum { L0, L1 } local_e;\n    local_e = L1;\n    return v + int'(local_e);\n  endfunction\n  C::kind_t ck = C::K0;\n  parameter enum { P0, P1 } pe = P1;\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            for (line, word, nth, declaration) in [
                (1, "K0", 0, true),
                (1, "K1", 0, true),
                (2, "K1", 0, false),
                (6, "L0", 0, true),
                (6, "L1", 0, true),
                (7, "L1", 0, false),
                (10, "K0", 0, false),
                (11, "P0", 0, true),
                (11, "P1", 0, true),
                (11, "P1", 1, false),
            ] {
                let token = token_at(&tokens, source, line, word, nth);
                assert_eq!(token.3, TT_ENUM_MEMBER, "{profile} {word} line {line}");
                let expected = (1 << TM_READONLY) | ((declaration as u32) << TM_DECLARATION);
                assert_eq!(token.4, expected, "{profile} {word} line {line}");
            }
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
