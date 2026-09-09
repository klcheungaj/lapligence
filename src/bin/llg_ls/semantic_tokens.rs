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
        TOKEN_SLANG_MODULE | TOKEN_SLANG_PROGRAM | TOKEN_SLANG_PACKAGE => TT_NAMESPACE,
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
}
