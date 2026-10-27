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
const TT_TYPE_PARAMETER: u32 = 6;
const TT_PARAMETER: u32 = 7;
const TT_VARIABLE: u32 = 8;
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
const TM_DEFAULT_LIBRARY: u32 = 9;
const TM_CONNECTION_LABEL: u32 = 10;
const TM_CONNECTION_LABEL_NAME: SemanticTokenModifier =
    SemanticTokenModifier::new("connectionLabel");
/// Keyword modifier for type-definition words (`typedef struct union enum`).
const TM_TYPE_DEFINITION: u32 = 11;
const TM_TYPE_DEFINITION_NAME: SemanticTokenModifier = SemanticTokenModifier::new("typeDefinition");
/// Keyword modifier for port directions (`input output inout ref`).
const TM_DIRECTION: u32 = 12;
const TM_DIRECTION_NAME: SemanticTokenModifier = SemanticTokenModifier::new("direction");
/// Keyword modifier for built-in data and net types and declaration kind and
/// qualifier words.
const TM_PRIMITIVE: u32 = 13;
const TM_PRIMITIVE_NAME: SemanticTokenModifier = SemanticTokenModifier::new("primitive");
/// Keyword modifier for control-flow words, named like rust-analyzer's
/// `controlFlow` so a client can give control flow its own scope while every
/// other keyword keeps a plain keyword scope.
const TM_CONTROL_FLOW: u32 = 14;
const TM_CONTROL_FLOW_NAME: SemanticTokenModifier = SemanticTokenModifier::new("controlFlow");

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
            TM_TYPE_DEFINITION_NAME,
            TM_DIRECTION_NAME,
            TM_PRIMITIVE_NAME,
            TM_CONTROL_FLOW_NAME,
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
        let Some((mut token_type, mut modifiers)) = token_type_for(node.kind) else {
            continue;
        };
        // Directive and macro-usage words arrive as operator-kind tokens.
        if node.kind == TOKEN_SLANG_OPERATOR {
            match node.name.as_deref().and_then(backtick_word) {
                Some(BacktickWord::Directive { conditional }) => {
                    token_type = TT_KEYWORD;
                    if conditional {
                        modifiers |= 1 << TM_CONTROL_FLOW;
                    }
                }
                Some(BacktickWord::MacroUsage) => token_type = TT_MACRO,
                None => {}
            }
        }
        // The capture marks words that are control flow only at some syntax
        // positions (a case item `default`, an if/case `unique`, a disable
        // statement); their text class is never consulted.
        if node.kind == TOKEN_SLANG_CONTROL_KEYWORD {
            modifiers |= 1 << TM_CONTROL_FLOW;
        } else if node.kind == TOKEN_SLANG_KEYWORD {
            modifiers |= match node.name.as_deref().map(keyword_class) {
                Some(KeywordClass::TypeDefinition) => 1 << TM_TYPE_DEFINITION,
                Some(KeywordClass::Direction) => 1 << TM_DIRECTION,
                Some(KeywordClass::Primitive) => 1 << TM_PRIMITIVE,
                Some(KeywordClass::ControlFlow) => 1 << TM_CONTROL_FLOW,
                Some(KeywordClass::Other) | None => 0,
            };
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
/// identifier declarations (instance names), bound references to them
/// (`TOKEN_SLANG_BOUND_IDENTIFIER`) and block names (`TOKEN_SLANG_BLOCK`) are
/// separate kinds and keep theirs.
fn is_unresolved_identifier(kind: i32) -> bool {
    kind == TOKEN_SLANG_IDENTIFIER
}

/// A grave-accent word in the token stream.
#[derive(Debug, PartialEq, Eq)]
enum BacktickWord {
    /// An IEEE 1800-2009 22.1 compiler directive; `conditional` for the
    /// conditional-compilation directives of 22.6.
    Directive { conditional: bool },
    /// Any other `` `name ``: a text macro usage (inside a macro body, where
    /// the capture keeps it unexpanded).
    MacroUsage,
}

/// Classify `` `name `` tokens. Compiler directives are keywords (`keyword`,
/// with `controlFlow` for `` `ifdef ``, `` `ifndef ``, `` `elsif ``, `` `else ``
/// and `` `endif ``, which select the source that is compiled, as `if` selects
/// statements); every other grave-accent name is a macro usage. The macro
/// quote and paste tokens (`` `" ``, ``` `` ```) are not words and keep the
/// operator type.
fn backtick_word(name: &str) -> Option<BacktickWord> {
    let word = name.strip_prefix('`')?;
    if !word
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
    {
        return None;
    }
    Some(match word {
        "ifdef" | "ifndef" | "elsif" | "else" | "endif" => {
            BacktickWord::Directive { conditional: true }
        }
        "begin_keywords"
        | "celldefine"
        | "default_nettype"
        | "define"
        | "end_keywords"
        | "endcelldefine"
        | "include"
        | "line"
        | "nounconnected_drive"
        | "pragma"
        | "resetall"
        | "timescale"
        | "unconnected_drive"
        | "undef"
        | "undefineall" => BacktickWord::Directive { conditional: false },
        _ => BacktickWord::MacroUsage,
    })
}

/// Which llg keyword modifier a keyword token carries. Only keyword tokens
/// are classified; user type names keep their own token types.
#[derive(Debug, PartialEq, Eq)]
enum KeywordClass {
    /// `typedef struct union enum`: `typeDefinition`.
    TypeDefinition,
    /// Port directions: `direction`.
    Direction,
    /// Built-in data and net types plus declaration kind and qualifier words:
    /// `primitive`.
    Primitive,
    /// Words that are control flow at every position: `controlFlow`.
    ControlFlow,
    /// Every other keyword is a plain `keyword` without modifier bits.
    Other,
}

/// Classify by keyword text, so words that are keywords in some editions only
/// (for example `uwire`) need no edition knowledge here. The words keep the
/// standard `keyword` type and gain an llg modifier instead of a custom type,
/// so clients without a mapping still see a keyword. Words that are control
/// flow only at some syntax positions (a case item `default`, an if/case
/// `unique`/`unique0`/`priority`, a disable statement's `disable`) arrive as
/// `TOKEN_SLANG_CONTROL_KEYWORD` there and are never classified; elsewhere
/// they keep their text class. VS Code styles a plain `keyword` from
/// `keyword.control`; a client maps `keyword` and `keyword.<modifier>`
/// selectors to its own scopes (the llg_ls readme has the contract).
///
/// The control-flow words are the statement keywords of IEEE 1800-2009
/// A.6.5-A.6.8 that choose, repeat, leave or wait for execution: conditional
/// and case statements (also as conditional generate constructs, A.4.2,
/// property statements, A.2.10, and constraint `if`/`foreach`, A.1.10, which
/// select or iterate the same way), loops, jump statements and wait
/// statements. Block delimiters (`begin`/`end`, `fork`/`join*`), event
/// qualifiers (`iff`), set membership (`inside`) and pattern words
/// (`matches`) stay plain keywords.
fn keyword_class(name: &str) -> KeywordClass {
    match name {
        "typedef" | "struct" | "union" | "enum" => KeywordClass::TypeDefinition,
        "input" | "output" | "inout" | "ref" => KeywordClass::Direction,
        "bit" | "logic" | "reg" | "byte" | "shortint" | "int" | "longint" | "integer" | "time"
        | "shortreal" | "real" | "realtime" | "string" | "chandle" | "event" | "void"
        | "signed" | "unsigned" | "genvar" | "supply0" | "supply1" | "tri" | "triand" | "trior"
        | "trireg" | "tri0" | "tri1" | "uwire" | "wire" | "wand" | "wor" | "parameter"
        | "localparam" | "specparam" | "const" | "var" | "default" | "automatic" | "static" => {
            KeywordClass::Primitive
        }
        "if" | "else" | "case" | "casex" | "casez" | "randcase" | "endcase" | "for" | "foreach"
        | "while" | "do" | "repeat" | "forever" | "return" | "break" | "continue" | "wait"
        | "wait_order" => KeywordClass::ControlFlow,
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
    // Types follow the clangd and rust-analyzer analogues (the llg_ls readme
    // has the table): value parameters are constants (`variable` +
    // `readonly`, VS Code `variable.other.constant`), type parameters are
    // `typeParameter` and ports are `parameter`. A named port or parameter
    // connection label is `function` (user decision, 2026-10-27) so that
    // stock themes color it apart from the `variable`/`parameter` actual it
    // connects; a parameter label keeps `readonly`, which VS Code registers
    // no default scope for on `function`, so it does not change the color.
    let connection = matches!(
        kind,
        TOKEN_SLANG_PORT_CONNECTION_LABEL | TOKEN_SLANG_PARAMETER_CONNECTION_LABEL
    );
    let token_type = match kind {
        TOKEN_SLANG_MODULE | TOKEN_SLANG_PROGRAM => TT_CLASS,
        TOKEN_SLANG_PACKAGE | TOKEN_SLANG_BLOCK => TT_NAMESPACE,
        TOKEN_SLANG_INTERFACE => TT_INTERFACE,
        TOKEN_SLANG_CLASS => TT_CLASS,
        TOKEN_SLANG_STRUCT | TOKEN_SLANG_UNION => TT_STRUCT,
        TOKEN_SLANG_ENUM => TT_ENUM,
        TOKEN_SLANG_ENUM_MEMBER => TT_ENUM_MEMBER,
        TOKEN_SLANG_TYPE_ALIAS => TT_TYPE,
        TOKEN_SLANG_PARAMETER => TT_VARIABLE,
        TOKEN_SLANG_TYPE_PARAMETER => TT_TYPE_PARAMETER,
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
        TOKEN_SLANG_KEYWORD | TOKEN_SLANG_CONTROL_KEYWORD => TT_KEYWORD,
        TOKEN_SLANG_STRING => TT_STRING,
        TOKEN_SLANG_NUMBER => TT_NUMBER,
        TOKEN_SLANG_OPERATOR => TT_OPERATOR,
        TOKEN_SLANG_PORT_CONNECTION_LABEL | TOKEN_SLANG_PARAMETER_CONNECTION_LABEL => TT_FUNCTION,
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
            .unwrap_or_else(|| panic!("no token for {word} on line {line}; stream {decoded:?}"))
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

    /// Text macros in every capture profile: each use site is one `macro`
    /// token, no expansion token is painted over it, and text written in
    /// the actual arguments keeps its own kind at its written position,
    /// through nested macros, a macro argument that is itself a macro, a
    /// declaration whose name is an argument, and an unknown macro (whose
    /// diagnostic leaves the rest of the stream intact).
    #[test]
    fn macro_use_sites_and_arguments_keep_their_own_tokens_in_every_profile() {
        let source = "`define W 8\n\
`define ADD(a, b) ((a) + (b))\n\
`define OUTER(x) `ADD(x, `W)\n\
`define DECL(n) logic n;\n\
module top(input logic [`W-1:0] v, output logic [7:0] y);\n\
  localparam int L = `W;\n\
  assign y = `ADD(v, 1) + `OUTER(v) + `ADD(`W, L);\n\
  `DECL(dv)\n\
  assign dv = `UNKNOWN;\n\
  wire q = v[0];\n\
endmodule\n";
        let readonly = 1 << TM_READONLY;
        let declaration = 1 << TM_DECLARATION;
        for (profile, tokens) in profiles(source) {
            let at = |line: u32, needle: &str, nth: usize| {
                let text = source.lines().nth(line as usize).unwrap();
                let col = text
                    .match_indices(needle)
                    .nth(nth)
                    .unwrap_or_else(|| panic!("no {needle} on line {line}"))
                    .0 as u32;
                let found: Vec<_> = tokens
                    .iter()
                    .filter(|t| (t.0, t.1) == (line, col))
                    .collect();
                assert_eq!(found.len(), 1, "{profile}: {needle} on {line}: {tokens:?}");
                (found[0].2.clone(), found[0].3, found[0].4)
            };
            for (line, needle, nth, text, token_type, bits) in [
                (4, "`W", 0, "`W", TT_MACRO, 0),
                (5, "`W", 0, "`W", TT_MACRO, 0),
                (6, "`ADD", 0, "`ADD", TT_MACRO, 0),
                (6, "v,", 0, "v", TT_PARAMETER, readonly),
                (6, "1)", 0, "1", TT_NUMBER, 0),
                (6, "`OUTER", 0, "`OUTER", TT_MACRO, 0),
                (6, "v)", 0, "v", TT_PARAMETER, readonly),
                (6, "`ADD", 1, "`ADD", TT_MACRO, 0),
                (6, "`W", 0, "`W", TT_MACRO, 0),
                (6, "L)", 0, "L", TT_VARIABLE, readonly),
                (7, "`DECL", 0, "`DECL", TT_MACRO, 0),
                (8, "`UNKNOWN", 0, "`UNKNOWN", TT_MACRO, 0),
                (9, "q", 0, "q", TT_VARIABLE, declaration),
                (9, "v", 0, "v", TT_PARAMETER, readonly),
            ] {
                assert_eq!(
                    at(line, needle, nth),
                    (text.to_owned(), token_type, bits),
                    "{profile}: {needle}#{nth} on line {line}"
                );
            }
            assert_eq!(
                at(7, "dv", 0),
                ("dv".to_owned(), TT_VARIABLE, declaration),
                "{profile}"
            );
            assert_eq!(
                at(8, "dv", 0),
                ("dv".to_owned(), TT_VARIABLE, 0),
                "{profile}"
            );
            // Nothing but the use site and the written arguments: the
            // expansion `((v) + (1))` adds no token on the usage line.
            let line6: Vec<_> = tokens
                .iter()
                .filter(|t| t.0 == 6)
                .map(|t| t.2.as_str())
                .collect();
            assert_eq!(
                line6,
                [
                    "assign", "y", "=", "`ADD", "(", "v", ",", "1", ")", "+", "`OUTER", "(", "v",
                    ")", "+", "`ADD", "(", "`W", ",", "L", ")", ";"
                ],
                "{profile}"
            );
        }
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
                    assert_eq!(kind, TT_VARIABLE, "library={library_units} {node:?}");
                    assert_ne!(modifiers & (1 << TM_READONLY), 0, "{node:?}");
                }
            }
        }
    }

    /// Type-definition words: `keyword` + `typeDefinition`.
    const TYPE_DEFINITION_WORDS: &[&str] = &["typedef", "struct", "union", "enum"];

    /// Port directions: `keyword` + `direction`.
    const DIRECTION_WORDS: &[&str] = &["input", "output", "inout", "ref"];

    /// Built-in data and net types plus declaration kind and qualifier words:
    /// `keyword` + `primitive`.
    const PRIMITIVE_WORDS: &[&str] = &[
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

    /// Each word group with the modifier bitset its keyword tokens carry.
    fn word_groups() -> [(&'static [&'static str], u32); 3] {
        [
            (TYPE_DEFINITION_WORDS, 1 << TM_TYPE_DEFINITION),
            (DIRECTION_WORDS, 1 << TM_DIRECTION),
            (PRIMITIVE_WORDS, 1 << TM_PRIMITIVE),
        ]
    }

    /// One module that uses every word of the three groups at least once.
    const EVERY_WORD_SOURCE: &str = "module all_words #(parameter int P = 1)(input logic i, output wire o, inout tri t);\n  localparam int L = 2;\n  specparam S = 3;\n  bit b; reg r; byte by; shortint si; longint li; integer ig; time tm;\n  shortreal sr; real re; realtime rt; string sg; chandle ch; event ev;\n  logic signed [1:0] sn; logic unsigned [1:0] us;\n  genvar gv;\n  supply0 s0; supply1 s1; triand ta; trior tr; trireg tg; tri0 t0; tri1 t1;\n  uwire uw; wand wa; wor wo;\n  typedef struct packed { bit f; } st_t;\n  typedef union packed { bit f; } un_t;\n  typedef enum { E0 } en_t;\n  st_t user_s;\n  const int ci = 1;\n  var logic vl;\n  static int sv;\n  function automatic void fa(ref int x); endfunction\n  always @* case (sv) 0: user_s = '{default: 0}; endcase\n  assign o = i;\nendmodule\n";

    /// Zero-based `(line, column)` of every whole-word occurrence of `word`.
    fn word_positions(source: &str, word: &str) -> Vec<(u32, u32)> {
        let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
        let mut out = Vec::new();
        for (line, text) in source.lines().enumerate() {
            let bytes = text.as_bytes();
            for (at, _) in text.match_indices(word) {
                if (at == 0 || !is_word(bytes[at - 1]))
                    && bytes.get(at + word.len()).is_none_or(|b| !is_word(*b))
                {
                    out.push((line as u32, at as u32));
                }
            }
        }
        out
    }

    #[test]
    fn keyword_modifiers_sit_at_their_encoded_indices_and_no_custom_types_remain() {
        let legend = legend();
        assert_eq!(legend.token_types[TT_KEYWORD as usize].as_str(), "keyword");
        assert_eq!(
            legend.token_types.len(),
            TT_OPERATOR as usize + 1,
            "only standard token types"
        );
        assert_eq!(
            legend.token_types.last().map(|t| t.as_str()),
            Some("operator")
        );
        for (index, name) in [
            (TM_CONNECTION_LABEL, "connectionLabel"),
            (TM_TYPE_DEFINITION, "typeDefinition"),
            (TM_DIRECTION, "direction"),
            (TM_PRIMITIVE, "primitive"),
            (TM_CONTROL_FLOW, "controlFlow"),
        ] {
            assert_eq!(legend.token_modifiers[index as usize].as_str(), name);
        }
        assert_eq!(legend.token_modifiers.len(), TM_CONTROL_FLOW as usize + 1);
    }

    #[test]
    fn grouped_keywords_carry_their_llg_modifier() {
        let source = "module top(input logic signed [3:0] a, output wire b, inout tri c);\ninteger count; real value; string label;\ntypedef enum { E0 } e_t;\nassign b = a[0];\nendmodule";
        let parsed = compile::parse_source(hp("/virtual/top.sv"), source, &[]).unwrap();
        let tokens = decode(source, &parsed.tokens[0].nodes);
        let count = |bits| {
            tokens
                .iter()
                .filter(|t| (t.3, t.4) == (TT_KEYWORD, bits))
                .count()
        };
        assert_eq!(count(1 << TM_TYPE_DEFINITION), 2, "{tokens:?}");
        assert_eq!(count(1 << TM_DIRECTION), 3, "{tokens:?}");
        assert_eq!(count(1 << TM_PRIMITIVE), 7, "{tokens:?}");
        assert_eq!(count(1 << TM_CONTROL_FLOW), 1, "{tokens:?}");
        assert_eq!(count(0), 2, "{tokens:?}");
        for token in tokens.iter().filter(|t| (t.3, t.4) == (TT_KEYWORD, 0)) {
            assert!(
                matches!(token.2.as_str(), "module" | "endmodule"),
                "{token:?}"
            );
        }
    }

    #[test]
    fn keyword_classes_cover_the_decided_word_lists() {
        for word in TYPE_DEFINITION_WORDS {
            assert_eq!(keyword_class(word), KeywordClass::TypeDefinition, "{word}");
        }
        for word in DIRECTION_WORDS {
            assert_eq!(keyword_class(word), KeywordClass::Direction, "{word}");
        }
        for word in PRIMITIVE_WORDS {
            assert_eq!(keyword_class(word), KeywordClass::Primitive, "{word}");
        }
        for word in CONTROL_FLOW_WORDS {
            assert_eq!(keyword_class(word), KeywordClass::ControlFlow, "{word}");
        }
        for word in [
            "module", "type", "packed", "assign", "begin", "initial", "always", "unique",
            "unique0", "priority", "disable", "iff", "inside", "fork", "join", "matches",
        ] {
            assert_eq!(keyword_class(word), KeywordClass::Other, "{word}");
        }
    }

    /// Every occurrence of every grouped word is `keyword` with exactly its
    /// group's llg modifier in every capture profile, while other keywords
    /// are `keyword` without modifier bits (`case`, `always` and `assign` are
    /// `controlFlow`) and the user typedef name stays `type`.
    #[test]
    fn every_decided_word_gets_its_keyword_modifier_in_every_profile() {
        let source = EVERY_WORD_SOURCE;
        let default_library = 1 << TM_DEFAULT_LIBRARY;
        for (profile, tokens) in profiles(source) {
            for (words, bits) in word_groups() {
                for word in words {
                    let positions = word_positions(source, word);
                    assert!(!positions.is_empty(), "sample lacks {word}");
                    for (line, col) in positions {
                        let token = tokens
                            .iter()
                            .find(|t| (t.0, t.1) == (line, col))
                            .unwrap_or_else(|| {
                                panic!("{profile}: no token for {word} at {line}:{col}")
                            });
                        assert_eq!(
                            (token.3, token.4),
                            (TT_KEYWORD, bits),
                            "{profile} {word} at {line}:{col}"
                        );
                    }
                }
            }
            let token = |line, word| token_at(&tokens, source, line, word, 0);
            let t = token(0, "module");
            assert_eq!((t.3, t.4), (TT_KEYWORD, 0), "{profile} keyword module");
            for (line, word) in [(17, "always"), (17, "case"), (18, "assign")] {
                let t = token(line, word);
                assert_eq!(
                    (t.3, t.4),
                    (TT_KEYWORD, 1 << TM_CONTROL_FLOW),
                    "{profile} keyword {word}"
                );
            }
            let user = token(12, "st_t");
            assert_eq!(user.3, TT_TYPE, "{profile} user typedef use");
            assert_eq!(user.4 & default_library, 0, "{profile} user typedef use");
            let declared = token(9, "st_t");
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
    /// undeclared name makes invalid: package qualifiers, `disable` labels
    /// and generate block steps (`namespace`),
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
                    ("gb", 0, TT_NAMESPACE, 0),
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
                assert_eq!((t.3, t.4), (TT_NAMESPACE, 0), "{profile} disable {label}");
            }
        }
    }

    /// Every `default` of the sample by zero-based line: case, case-inside,
    /// case-generate, property-case and randsequence-case items are control
    /// flow; assignment-pattern keys, `default clocking`, `default disable
    /// iff` and default bins are not.
    const DEFAULT_SOURCE: &str = "module top;\n\
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

    /// Lines of [`DEFAULT_SOURCE`] whose `default` is a case item label.
    const CONTROL_DEFAULT_LINES: &[u32] = &[6, 8, 9, 12, 13];

    #[test]
    fn case_item_default_is_control_flow_and_every_other_default_is_primitive_in_every_profile() {
        let source = DEFAULT_SOURCE;
        let primitive = 1 << TM_PRIMITIVE;
        let control = 1 << TM_CONTROL_FLOW;
        let positions = word_positions(source, "default");
        assert_eq!(positions.len(), 11, "sample defaults");
        for (profile, tokens) in profiles(source) {
            for &(line, col) in &positions {
                let token = tokens
                    .iter()
                    .find(|t| (t.0, t.1) == (line, col))
                    .unwrap_or_else(|| panic!("{profile}: no token for default at {line}:{col}"));
                let bits = if CONTROL_DEFAULT_LINES.contains(&line) {
                    control
                } else {
                    primitive
                };
                assert_eq!(
                    (token.3, token.4),
                    (TT_KEYWORD, bits),
                    "{profile}: default at {line}:{col}"
                );
            }
            // The other grouped words keep their modifier next to a control
            // `default`.
            let t = token_at(&tokens, source, 2, "logic", 0);
            assert_eq!((t.3, t.4), (TT_KEYWORD, primitive), "{profile}");
            let t = token_at(&tokens, source, 8, "case", 0);
            assert_eq!((t.3, t.4), (TT_KEYWORD, control), "{profile}");
        }
    }

    /// Words that are control flow at every position: `keyword` +
    /// `controlFlow`.
    const CONTROL_FLOW_WORDS: &[&str] = &[
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
    ];

    /// Words that are control flow only where the capture marks them.
    const POSITIONAL_CONTROL_WORDS: &[&str] = &["unique", "unique0", "priority", "disable"];

    /// Every control-flow word in statements, conditional generate
    /// constructs, a property and constraints, next to plain keywords that
    /// delimit blocks or qualify events and a `disable iff` reset.
    const CONTROL_SOURCE: &str = "module top(input logic clk, input logic rst);
  typedef logic [3:0] q_t;
  q_t q; int arr [2]; event ev;
  for (genvar i = 0; i < 2; i++) begin : g
    if (i == 0) begin : t end else begin : e end
  end
  case (1) 1: begin : c end default: begin : d end endcase
  property p; @(posedge clk iff rst) disable iff (rst) if (q == 0) q == 1 else q == 2; endproperty
  function automatic int f(int n); return n; endfunction
  initial begin : blk
    unique if (q == 0) q = 1; else if (q inside {[1:2]}) q = 2; else q = 3;
    unique0 if (q == 1) q = 0;
    priority if (q == 2) q = 0;
    unique case (q) 0: q = 1; default: q = 2; endcase
    unique0 casez (q) 4'b1???: q = 0; endcase
    priority casex (q) 4'b0xxx: q = 1; endcase
    randcase 1: q = 0; endcase
    for (int k = 0; k < 2; k++) begin if (k == 1) break; else continue; end
    foreach (arr[k]) arr[k] = k;
    while (q > 0) q--;
    do q++; while (q < 2);
    repeat (2) @(posedge clk);
    wait (rst);
    wait fork;
    wait_order (ev) else q = 0;
    fork : fk join_any
    fork join_none
    disable fk;
    disable fork;
    -> ev;
    forever begin break; end
  end
endmodule
class cls;
  rand int x; rand int arr2 [2];
  constraint cx { if (x > 0) x < 4; else x > 1; foreach (arr2[j]) arr2[j] > 0; }
endclass
";

    /// Zero-based line of [`CONTROL_SOURCE`] whose `disable` is a property
    /// reset, not a disable statement.
    const DISABLE_IFF_LINE: u32 = 7;

    #[test]
    fn control_flow_words_carry_control_flow_and_other_keywords_stay_plain_in_every_profile() {
        let source = CONTROL_SOURCE;
        let control = 1 << TM_CONTROL_FLOW;
        for (profile, tokens) in profiles(source) {
            let bits_at = |line: u32, col: u32, word: &str| {
                let token = tokens
                    .iter()
                    .find(|t| (t.0, t.1) == (line, col))
                    .unwrap_or_else(|| panic!("{profile}: no token for {word} at {line}:{col}"));
                assert_eq!(token.3, TT_KEYWORD, "{profile}: {word} at {line}:{col}");
                token.4
            };
            for word in CONTROL_FLOW_WORDS.iter().chain(POSITIONAL_CONTROL_WORDS) {
                let positions = word_positions(source, word);
                assert!(!positions.is_empty(), "sample lacks {word}");
                for (line, col) in positions {
                    let want = if *word == "disable" && line == DISABLE_IFF_LINE {
                        0
                    } else {
                        control
                    };
                    assert_eq!(
                        bits_at(line, col, word),
                        want,
                        "{profile}: {word} at {line}:{col}"
                    );
                }
            }
            for (line, col) in word_positions(source, "default") {
                assert_eq!(bits_at(line, col, "default"), control, "{profile}: default");
            }
            for word in [
                "module",
                "endmodule",
                "begin",
                "end",
                "fork",
                "join_any",
                "join_none",
                "iff",
                "inside",
                "posedge",
                "property",
                "endproperty",
                "function",
                "endfunction",
                "class",
                "endclass",
                "rand",
                "constraint",
            ] {
                let positions = word_positions(source, word);
                assert!(!positions.is_empty(), "sample lacks {word}");
                for (line, col) in positions {
                    // `wait fork` and `disable fork` reuse the block word.
                    let bits = bits_at(line, col, word);
                    assert_eq!(bits, 0, "{profile}: {word} at {line}:{col}");
                }
            }
            for (word, bits) in [
                ("typedef", 1 << TM_TYPE_DEFINITION),
                ("input", 1 << TM_DIRECTION),
                ("logic", 1 << TM_PRIMITIVE),
                ("automatic", 1 << TM_PRIMITIVE),
            ] {
                for (line, col) in word_positions(source, word) {
                    assert_eq!(bits_at(line, col, word), bits, "{profile}: {word}");
                }
            }
            // `->` is an operator, and block names stay namespaces.
            let (line, col) = word_positions(source, "ev")[2];
            assert_eq!(line, 29, "{profile}");
            let arrow = tokens
                .iter()
                .find(|t| (t.0, t.1) == (line, col - 3))
                .unwrap_or_else(|| panic!("{profile}: no -> token"));
            assert_eq!(
                (arrow.2.as_str(), arrow.3),
                ("->", TT_OPERATOR),
                "{profile}"
            );
            let t = token_at(&tokens, source, 27, "fk", 0);
            assert_eq!((t.3, t.4), (TT_NAMESPACE, 0), "{profile} disable fk");
        }
    }

    /// Process and continuous-assignment keywords (IEEE 1800-2009 9.2, 10.3,
    /// 10.6, 10.11) in modules, a checker and a procedure.
    const PROCESS_SOURCE: &str = "module top(input logic clk, input logic d);
  logic q, r; wire w1, w2;
  initial q = 0;
  final $display(q);
  always @(posedge clk) q <= d;
  always_comb r = d;
  always_ff @(posedge clk) r <= d;
  always_latch if (clk) q = d;
  assign w1 = d;
  alias w1 = w2;
  initial begin
    assign q = d;
    deassign q;
    force r = d;
    release r;
    force w1 = 1'b0;
    release w1;
  end
  property pa; @(posedge clk) always d; endproperty
endmodule
checker chk(logic a, event ck);
  logic m;
  always @(ck) m <= a;
endchecker
primitive udp(output reg uq, input ud, input uc);
  initial uq = 0;
  table
    0 r : ? : 0;
    1 r : ? : 1;
  endtable
endprimitive
";

    /// Zero-based lines of [`PROCESS_SOURCE`] whose `always` is the property
    /// operator and whose `initial` is a UDP initial statement.
    const PROPERTY_ALWAYS_LINE: u32 = 18;
    const UDP_INITIAL_LINE: u32 = 25;

    /// Process introducers and continuous, alias and procedural continuous
    /// assignment keywords are `controlFlow` in every profile; the `always`
    /// property operator and a UDP `initial` stay plain.
    #[test]
    fn process_and_assignment_keywords_are_control_flow_in_every_profile() {
        let source = PROCESS_SOURCE;
        let control = 1 << TM_CONTROL_FLOW;
        for (profile, tokens) in profiles(source) {
            for word in [
                "initial",
                "final",
                "always",
                "always_comb",
                "always_ff",
                "always_latch",
                "assign",
                "alias",
                "deassign",
                "force",
                "release",
            ] {
                let positions = word_positions(source, word);
                assert!(!positions.is_empty(), "sample lacks {word}");
                for (line, col) in positions {
                    let want = match (word, line) {
                        ("always", PROPERTY_ALWAYS_LINE) | ("initial", UDP_INITIAL_LINE) => 0,
                        _ => control,
                    };
                    let token = tokens
                        .iter()
                        .find(|t| (t.0, t.1) == (line, col))
                        .unwrap_or_else(|| {
                            panic!("{profile}: no token for {word} at {line}:{col}")
                        });
                    assert_eq!(
                        (token.3, token.4),
                        (TT_KEYWORD, want),
                        "{profile}: {word} at {line}:{col}"
                    );
                }
            }
            for word in [
                "module",
                "checker",
                "primitive",
                "table",
                "property",
                "begin",
            ] {
                for (line, col) in word_positions(source, word) {
                    let token = tokens.iter().find(|t| (t.0, t.1) == (line, col)).unwrap();
                    assert_eq!((token.3, token.4), (TT_KEYWORD, 0), "{profile}: {word}");
                }
            }
        }
    }

    /// `unique` is control flow only when it qualifies an if or case; the
    /// uniqueness constraint (outside the 2009 edition, still captured) keeps
    /// a plain keyword.
    #[test]
    fn uniqueness_constraint_unique_is_a_plain_keyword() {
        let source = "class cls;
  rand int a, b;
  constraint cu { unique {a, b}; }
endclass
module top; int q; initial unique if (q == 0) q = 1; endmodule
";
        let parsed = compile::parse_source(hp("/virtual/top.sv"), source, &[]).unwrap();
        let tokens = decode(source, &parsed.tokens[0].nodes);
        let t = token_at(&tokens, source, 2, "unique", 0);
        assert_eq!((t.3, t.4), (TT_KEYWORD, 0), "{tokens:?}");
        let t = token_at(&tokens, source, 4, "unique", 0);
        assert_eq!((t.3, t.4), (TT_KEYWORD, 1 << TM_CONTROL_FLOW), "{tokens:?}");
    }

    /// Value parameters, type parameters, connection labels and directives.
    const DECLARATION_SOURCE: &str = "`define W 8\n\
`define TWICE(x) (`W * (x))\n\
`ifdef W\n\
module child #(parameter int P = 1, parameter type T = logic [1:0])(input T a, output logic y);\n\
  T t_local;\n\
  assign y = a[0] ^ P[0];\n\
endmodule\n\
`else\n\
module other; endmodule\n\
`endif\n\
`timescale 1ns/1ps\n\
module top;\n\
  localparam int L = 2;\n\
  logic [1:0] v; logic o;\n\
  child #(.P(L)) u (.a(v), .y(o));\n\
endmodule\n";

    /// Value parameters are constants (`variable` + `readonly`), type
    /// parameters `typeParameter`, port labels `function` and parameter
    /// labels `function` + `readonly` (both with `connectionLabel`), compiler
    /// directives `keyword` (conditional ones with `controlFlow`) and a macro
    /// usage inside a macro body `macro`, in every capture profile.
    #[test]
    fn parameters_labels_and_directives_use_their_analogue_types_in_every_profile() {
        let source = DECLARATION_SOURCE;
        let declaration = 1 << TM_DECLARATION;
        let readonly = 1 << TM_READONLY;
        let label = 1 << TM_CONNECTION_LABEL;
        let control = 1 << TM_CONTROL_FLOW;
        for (profile, tokens) in profiles(source) {
            let at = |line: u32, needle: &str| {
                let col = source
                    .lines()
                    .nth(line as usize)
                    .unwrap()
                    .find(needle)
                    .unwrap() as u32;
                let t = tokens
                    .iter()
                    .find(|t| (t.0, t.1) == (line, col))
                    .unwrap_or_else(|| panic!("{profile}: no token for {needle} on {line}"));
                (t.2.clone(), t.3, t.4)
            };
            for (line, needle, text, token_type, bits) in [
                (0, "`define", "`define", TT_KEYWORD, 0),
                (1, "`define", "`define", TT_KEYWORD, 0),
                (1, "`W", "`W", TT_MACRO, 0),
                (2, "`ifdef", "`ifdef", TT_KEYWORD, control),
                (7, "`else", "`else", TT_KEYWORD, control),
                (9, "`endif", "`endif", TT_KEYWORD, control),
                (10, "`timescale", "`timescale", TT_KEYWORD, 0),
                (3, "P =", "P", TT_VARIABLE, declaration | readonly),
                (3, "T =", "T", TT_TYPE_PARAMETER, declaration),
                (3, "T a", "T", TT_TYPE_PARAMETER, 0),
                (4, "T", "T", TT_TYPE_PARAMETER, 0),
                (5, "P[0]", "P", TT_VARIABLE, readonly),
                (12, "L =", "L", TT_VARIABLE, declaration | readonly),
                (14, "L)", "L", TT_VARIABLE, readonly),
                (14, "P(", "P", TT_FUNCTION, readonly | label),
                (14, "a(", "a", TT_FUNCTION, label),
                (14, "y(", "y", TT_FUNCTION, label),
                (14, "v)", "v", TT_VARIABLE, 0),
                (14, "o)", "o", TT_VARIABLE, 0),
            ] {
                assert_eq!(
                    at(line, needle),
                    (text.to_owned(), token_type, bits),
                    "{profile}: {needle} on line {line}"
                );
            }
        }
    }

    /// Named, implicit and interface port labels and value and type
    /// parameter labels.
    const CONNECTION_SOURCE: &str = "interface bus_if; logic s; modport mp(input s); endinterface
module child #(parameter int W = 1, parameter type T = logic)(input T a, output logic y, bus_if.mp b);
endmodule
module top;
  localparam int L = 2;
  logic v, y, w;
  bus_if bi();
  child #(.W(L), .T(logic [1:0])) u0 (.a(v), .y, .b(bi));
  child #(L, logic) u1 (v, w, bi);
endmodule
";

    /// Every named connection label is `function` + `connectionLabel`
    /// (parameter labels also `readonly`), so stock themes color it apart
    /// from the actual, which keeps its own type, as do positional actuals.
    #[test]
    fn connection_labels_are_functions_and_actuals_keep_their_types_in_every_profile() {
        let source = CONNECTION_SOURCE;
        let readonly = 1 << TM_READONLY;
        let label = 1 << TM_CONNECTION_LABEL;
        for (profile, tokens) in profiles(source) {
            let at = |line: u32, needle: &str, nth: usize| {
                let text = source.lines().nth(line as usize).unwrap();
                let col = text.match_indices(needle).nth(nth).unwrap().0 as u32
                    + needle.starts_with('.') as u32;
                let t = tokens
                    .iter()
                    .find(|t| (t.0, t.1) == (line, col))
                    .unwrap_or_else(|| panic!("{profile}: no token for {needle} on {line}"));
                (t.3, t.4)
            };
            for (line, needle, nth, want) in [
                (7, ".W", 0, (TT_FUNCTION, readonly | label)),
                (7, "L", 0, (TT_VARIABLE, readonly)),
                (7, ".T", 0, (TT_FUNCTION, readonly | label)),
                (7, ".a", 0, (TT_FUNCTION, label)),
                (7, "v", 0, (TT_VARIABLE, 0)),
                (7, ".y", 0, (TT_FUNCTION, label)),
                (7, ".b", 0, (TT_FUNCTION, label)),
                (7, "bi", 0, (TT_VARIABLE, 0)),
                (8, "L", 0, (TT_VARIABLE, readonly)),
                (8, "v", 0, (TT_VARIABLE, 0)),
                (8, "w", 0, (TT_VARIABLE, 0)),
                (8, "bi", 0, (TT_VARIABLE, 0)),
            ] {
                assert_eq!(
                    at(line, needle, nth),
                    want,
                    "{profile}: {needle} on line {line}"
                );
            }
        }
    }

    #[test]
    fn backtick_words_split_directives_macro_usages_and_quote_tokens() {
        for word in [
            "`begin_keywords",
            "`celldefine",
            "`default_nettype",
            "`define",
            "`end_keywords",
            "`endcelldefine",
            "`include",
            "`line",
            "`nounconnected_drive",
            "`pragma",
            "`resetall",
            "`timescale",
            "`unconnected_drive",
            "`undef",
            "`undefineall",
        ] {
            assert_eq!(
                backtick_word(word),
                Some(BacktickWord::Directive { conditional: false }),
                "{word}"
            );
        }
        for word in ["`ifdef", "`ifndef", "`elsif", "`else", "`endif"] {
            assert_eq!(
                backtick_word(word),
                Some(BacktickWord::Directive { conditional: true }),
                "{word}"
            );
        }
        for word in ["`WIDTH", "`_m", "`__FILE__", "`definex"] {
            assert_eq!(
                backtick_word(word),
                Some(BacktickWord::MacroUsage),
                "{word}"
            );
        }
        for word in ["`\"", "``", "`\\`\"", "`", "+", "define"] {
            assert_eq!(backtick_word(word), None, "{word}");
        }
    }

    /// Named blocks, labeled statements and generate blocks: declarations
    /// (`namespace` + `declaration`), end labels and references
    /// (`namespace`). Instance names and members keep their kinds.
    const BLOCK_SOURCE: &str = "module sub(output logic o); assign o = 1'b0; endmodule\n\
module top;\n\
  logic q;\n\
  sub u0(.o());\n\
  for (genvar i = 0; i < 2; i++) begin : g\n\
    logic x;\n\
  end : g\n\
  if (1) begin : gi logic y; end\n\
  initial begin : blk\n\
    fork : f\n\
      q = g[0].x;\n\
    join : f\n\
    lbl: begin\n\
      q = top.g[1].x;\n\
      q = gi.y | u0.o;\n\
    end : lbl\n\
    l2: assert (q == 0);\n\
    disable lbl;\n\
    disable top.blk;\n\
  end : blk\n\
endmodule\n";

    #[test]
    fn block_names_are_namespaces_at_declarations_end_labels_and_references_in_every_profile() {
        let source = BLOCK_SOURCE;
        let declaration = 1 << TM_DECLARATION;
        let declarations = [
            (4, "g", 0),
            (7, "gi", 0),
            (8, "blk", 0),
            (9, "f", 0),
            (12, "lbl", 0),
            (16, "l2", 0),
        ];
        let end_labels = [(6, "g", 0), (11, "f", 0), (15, "lbl", 0), (19, "blk", 0)];
        let references = [
            (10, "g", 0),
            (13, "g", 0),
            (14, "gi", 0),
            (17, "lbl", 0),
            (18, "blk", 0),
        ];
        for (profile, tokens) in profiles(source) {
            for (line, word, nth) in declarations {
                let t = token_at(&tokens, source, line, word, nth);
                assert_eq!(
                    (t.3, t.4),
                    (TT_NAMESPACE, declaration),
                    "{profile} declaration {word} on line {line}"
                );
            }
            for (line, word, nth) in end_labels {
                let t = token_at(&tokens, source, line, word, nth);
                assert_eq!(
                    (t.3, t.4),
                    (TT_NAMESPACE, 0),
                    "{profile} end label {word} on line {line}"
                );
            }
            for (line, word, nth) in references {
                // Library-unit capture elaborates no hierarchy, so a
                // hierarchical step may have no token there (see above).
                let text = source.lines().nth(line).unwrap();
                let hierarchical = text.contains('.');
                if hierarchical && profile == "library_units=true" {
                    let col = text.find(word).unwrap() as u32;
                    if let Some(t) = tokens.iter().find(|t| (t.0, t.1) == (line as u32, col)) {
                        assert_eq!((t.3, t.4), (TT_NAMESPACE, 0), "{profile} {word}");
                    }
                    continue;
                }
                let t = token_at(&tokens, source, line, word, nth);
                assert_eq!(
                    (t.3, t.4),
                    (TT_NAMESPACE, 0),
                    "{profile} reference {word} on line {line}"
                );
            }
            // Instances and the module are not blocks.
            let t = token_at(&tokens, source, 3, "u0", 0);
            assert_eq!(t.3, TT_VARIABLE, "{profile} instance declaration");
            let t = token_at(&tokens, source, 5, "x", 0);
            assert_eq!(t.3, TT_VARIABLE, "{profile} generate member");
            if profile != "library_units=true" {
                let t = token_at(&tokens, source, 14, "u0", 0);
                assert_eq!(t.3, TT_VARIABLE, "{profile} instance reference");
                let t = token_at(&tokens, source, 18, "top", 0);
                assert_ne!(t.3, TT_NAMESPACE, "{profile} hierarchical top");
            }
        }
    }

    /// Full capture binds an end label to its block, so navigation from
    /// `end : blk` reaches the opening name.
    #[test]
    fn block_end_labels_bind_to_their_declaration() {
        let source = BLOCK_SOURCE;
        let compiled = compile::compile(&CompileOpts {
            sources: vec![OwnedSource::compilation_unit(hp("/virtual/top.sv"), source)],
            ..Default::default()
        })
        .unwrap();
        let (_, bindings, _, _, _) = llg::core::tokens::project_slang(
            &compiled.snapshot,
            &[(hp("/virtual/top.sv"), source)],
        );
        let column = |line: usize, word: &str| {
            let text = source.lines().nth(line).unwrap();
            text.rfind(word).unwrap() as u32
        };
        for (end_line, decl_line, word) in
            [(6, 4, "g"), (11, 9, "f"), (15, 12, "lbl"), (19, 8, "blk")]
        {
            let key = (
                hp("/virtual/top.sv").to_owned(),
                end_line as u32,
                column(end_line, word),
            );
            let target = bindings
                .get(&key)
                .unwrap_or_else(|| panic!("end label {word} unbound: {bindings:?}"));
            let decl_col = if word == "lbl" {
                source.lines().nth(decl_line).unwrap().find(word).unwrap() as u32
            } else {
                column(decl_line, word)
            };
            assert_eq!(
                (target.name.as_str(), target.line0, target.col0),
                (word, decl_line as u32, decl_col),
                "end label {word}"
            );
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
                assert_eq!(kind, TT_VARIABLE, "library={library_units} {node:?}");
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
            let want = if node.kind == TOKEN_SLANG_PARAMETER_CONNECTION_LABEL {
                TT_FUNCTION
            } else {
                TT_VARIABLE
            };
            assert_eq!(kind, want, "{node:?}");
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
    fn parameter_declaration_words_are_primitive_keywords_and_other_keywords_stay_plain() {
        let source = "module sub #(parameter int A = 1)();\nendmodule\nmodule top #(parameter int P = 1, localparam int Q = 2)(input logic x);\n  parameter type T = int;\n  localparam R = 3;\n  specparam S = 4;\n  sub u ();\n  defparam u.A = 5;\n  if (P > 0) begin : g end\nendmodule\n";
        for (profile, tokens) in profiles(source) {
            let at = |line, word, nth| type_at(&tokens, source, line, word, nth);
            let t = token_at(&tokens, source, 8, "if", 0);
            assert_eq!(
                (t.3, t.4),
                (TT_KEYWORD, 1 << TM_CONTROL_FLOW),
                "{profile} if"
            );
            for (line, word) in [
                (7, "defparam"),
                (0, "module"),
                (2, "module"),
                (9, "endmodule"),
            ] {
                let t = token_at(&tokens, source, line, word, 0);
                assert_eq!((t.3, t.4), (TT_KEYWORD, 0), "{profile} {word} line {line}");
            }
            for (line, word) in [
                (0, "parameter"),
                (2, "parameter"),
                (2, "localparam"),
                (3, "parameter"),
                (4, "localparam"),
                (5, "specparam"),
            ] {
                let t = token_at(&tokens, source, line, word, 0);
                assert_eq!(
                    (t.3, t.4),
                    (TT_KEYWORD, 1 << TM_PRIMITIVE),
                    "{profile} {word} line {line}"
                );
            }
            assert_eq!(at(2, "P", 0), TT_VARIABLE, "{profile}");
            let t = token_at(&tokens, source, 3, "T", 0);
            assert_eq!(
                (t.3, t.4),
                (TT_TYPE_PARAMETER, 1 << TM_DECLARATION),
                "{profile} T"
            );
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
