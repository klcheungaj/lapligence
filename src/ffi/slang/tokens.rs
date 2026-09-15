//! Tokens.

use super::*;

pub(super) fn decode_lexical_tokens(
    raw: &[RawLexicalToken],
    files: &[File],
    semantic_nodes: &[SemanticNode],
) -> Result<Vec<LexicalToken>, SlangError> {
    let semantic_ids: HashSet<_> = semantic_nodes.iter().map(|node| node.id).collect();
    raw.iter()
        .map(|token| {
            if token.reserved != 0 || token.flags & !0b111 != 0 {
                return Err(invalid_native("lexical token has unknown flags"));
            }
            let semantic_id = (token.semantic_id != INVALID_ID).then_some(token.semantic_id);
            if semantic_id.is_some_and(|id| !semantic_ids.contains(&id)) {
                return Err(invalid_native("lexical token semantic id does not exist"));
            }
            Ok(LexicalToken {
                range: decode_range(token.range, files)?,
                kind: decode_lexical_kind(token.kind)?,
                role: match token.role {
                    0 => LexicalRole::None,
                    1 => LexicalRole::Declaration,
                    2 => LexicalRole::Reference,
                    3 => LexicalRole::ConnectionLabel,
                    4 => LexicalRole::Keyword,
                    5 => LexicalRole::ConnectionActual,
                    _ => return Err(invalid_native("lexical token has an unknown role")),
                },
                is_missing: token.flags & 1 != 0,
                is_skipped: token.flags & 2 != 0,
                is_macro_expansion: token.flags & 4 != 0,
                semantic_id,
                // SAFETY: native strings borrow from the live snapshot.
                text: unsafe { copy_string(token.text, "lexical token text")? },
            })
        })
        .collect()
}

fn decode_lexical_kind(raw: u32) -> Result<LexicalKind, SlangError> {
    Ok(match raw {
        0 => LexicalKind::Unknown,
        1 => LexicalKind::Module,
        2 => LexicalKind::Interface,
        3 => LexicalKind::Program,
        4 => LexicalKind::Package,
        5 => LexicalKind::Class,
        6 => LexicalKind::Struct,
        7 => LexicalKind::Union,
        8 => LexicalKind::Enum,
        9 => LexicalKind::EnumMember,
        10 => LexicalKind::TypeAlias,
        11 => LexicalKind::Parameter,
        12 => LexicalKind::Port,
        13 => LexicalKind::Variable,
        14 => LexicalKind::Net,
        15 => LexicalKind::Function,
        16 => LexicalKind::Task,
        17 => LexicalKind::Method,
        18 => LexicalKind::Macro,
        19 => LexicalKind::Keyword,
        20 => LexicalKind::String,
        21 => LexicalKind::Number,
        22 => LexicalKind::Operator,
        23 => LexicalKind::Identifier,
        24 => LexicalKind::Genvar,
        _ => return Err(invalid_native("lexical token has an unknown kind")),
    })
}
