//! Receivers of lexical targets, macro definitions and references, and
//! include directives.

use super::*;

const MACRO_FUNCTION_LIKE: u32 = 1 << 0;
const MACRO_PREDEFINED: u32 = 1 << 0;

fn range_key(range: SourceRange) -> (u64, u64, u64) {
    (range.file_id, range.start, range.end)
}

/// Copy one lexical target, rejecting a target that names no node or no
/// declaration, and records out of token order.
pub(super) fn decode_lexical_target(
    item: &RawLexicalTarget,
    tokens: &[LexicalToken],
    semantic_node_count: usize,
    previous: Option<&LexicalTarget>,
) -> Result<LexicalTarget, SlangError> {
    let index = |id: u64, count: usize| usize::try_from(id).ok().filter(|index| *index < count);
    let token = index(item.token_index, tokens.len())
        .ok_or_else(|| invalid_native("lexical target names an unknown token"))?;
    if previous.is_some_and(|previous| previous.token > token) {
        return Err(invalid_native("lexical targets are not sorted by token"));
    }
    let target = match (item.semantic_id, item.target_token) {
        (id, INVALID_ID) => index(id, semantic_node_count).map(|_| LexicalTargetRef::Semantic(id)),
        (INVALID_ID, id) => index(id, tokens.len())
            .filter(|index| tokens[*index].role == LexicalRole::Declaration)
            .map(LexicalTargetRef::Token),
        _ => None,
    };
    Ok(LexicalTarget {
        token,
        target: target.ok_or_else(|| {
            invalid_native("lexical target names neither one node nor one declaration")
        })?,
    })
}

/// Copy one macro definition, rejecting ranges that do not nest.
pub(super) fn decode_macro_definition(
    item: &RawMacroDefinition,
    files: &[File],
) -> Result<MacroDefinition, SlangError> {
    let required = |raw: RawRange| -> Result<SourceRange, SlangError> {
        decode_range(raw, files)?
            .ok_or_else(|| invalid_native("macro definition record has no source range"))
    };
    let name = required(item.name)?;
    let range = required(item.range)?;
    let body = required(item.body)?;
    let within = |inner: SourceRange| {
        inner.file_id == range.file_id && inner.start >= range.start && inner.end <= range.end
    };
    if !within(name) || !within(body) || name.start == name.end || body.start < name.end {
        return Err(invalid_native("macro definition ranges do not nest"));
    }
    if item.flags & !MACRO_FUNCTION_LIKE != 0 || item.reserved != 0 {
        return Err(invalid_native("macro definition record has unknown flags"));
    }
    Ok(MacroDefinition {
        name,
        range,
        body,
        function_like: item.flags & MACRO_FUNCTION_LIKE != 0,
    })
}

/// Copy one macro reference, rejecting an unknown definition and records out
/// of range order.
pub(super) fn decode_macro_reference(
    item: &RawMacroReference,
    files: &[File],
    definition_count: usize,
    previous: Option<&MacroReference>,
) -> Result<MacroReference, SlangError> {
    let range = decode_range(item.range, files)?
        .ok_or_else(|| invalid_native("macro reference record has no source range"))?;
    let kind = match item.kind {
        1 => MacroReferenceKind::Use,
        2 => MacroReferenceKind::Test,
        3 => MacroReferenceKind::Undef,
        _ => return Err(invalid_native("macro reference has an unknown kind")),
    };
    let definition = match (item.definition_id, item.flags) {
        (INVALID_ID, MACRO_PREDEFINED) => None,
        (id, 0) => Some(
            usize::try_from(id)
                .ok()
                .filter(|index| *index < definition_count)
                .ok_or_else(|| invalid_native("macro reference names an unknown definition"))?,
        ),
        _ => return Err(invalid_native("macro reference has unknown flags")),
    };
    let reference = MacroReference {
        range,
        kind,
        definition,
    };
    if previous.is_some_and(|previous| range_key(previous.range) > range_key(range)) {
        return Err(invalid_native("macro references are not sorted by range"));
    }
    Ok(reference)
}

/// Copy one include, rejecting an unknown file and records out of operand
/// order.
pub(super) fn decode_include(
    item: &RawInclude,
    files: &[File],
    previous: Option<&Include>,
) -> Result<Include, SlangError> {
    let operand = decode_range(item.operand, files)?
        .ok_or_else(|| invalid_native("include record has no operand range"))?;
    if !files.iter().any(|file| file.id == item.file_id) {
        return Err(invalid_native("include record names an unknown file"));
    }
    if previous.is_some_and(|previous| range_key(previous.operand) > range_key(operand)) {
        return Err(invalid_native("include records are not sorted by operand"));
    }
    Ok(Include {
        operand,
        file_id: item.file_id,
    })
}

/// Copy one declaration extent, rejecting a missing range, a range in another
/// file than the name or not holding it, and records out of order.
pub(super) fn decode_declaration_extent(
    item: &RawDeclarationExtent,
    files: &[File],
    previous: Option<&DeclarationExtent>,
) -> Result<DeclarationExtent, SlangError> {
    let missing = || invalid_native("declaration extent record has no range");
    let name = decode_range(item.name, files)?.ok_or_else(missing)?;
    let range = decode_range(item.range, files)?.ok_or_else(missing)?;
    if range.file_id != name.file_id || range.start > name.start || range.end < name.end {
        return Err(invalid_native(
            "declaration extent record does not hold its name",
        ));
    }
    let key =
        |extent: &DeclarationExtent| (range_key(extent.name), extent.range.start, extent.range.end);
    let extent = DeclarationExtent { name, range };
    if previous.is_some_and(|previous| key(previous) >= key(&extent)) {
        return Err(invalid_native(
            "declaration extent records are not sorted and distinct",
        ));
    }
    Ok(extent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_RANGE: RawRange = RawRange {
        file_id: INVALID_ID,
        start: 0,
        end: 0,
    };

    fn files() -> Vec<File> {
        let text = "`define W 4\n".to_owned();
        vec![File {
            id: 0,
            name: "t.sv".to_owned(),
            byte_len: text.len() as u64,
            text,
        }]
    }

    fn raw_range(start: u64, end: u64) -> RawRange {
        RawRange {
            file_id: 0,
            start,
            end,
        }
    }

    fn token(role: LexicalRole) -> LexicalToken {
        LexicalToken {
            range: None,
            kind: LexicalKind::Identifier,
            role,
            semantic_id: None,
            text: String::new(),
            is_missing: false,
            is_skipped: false,
            is_macro_expansion: false,
            is_directive: false,
            is_unit_forward_reference: false,
            is_control_flow: false,
            is_macro_argument: false,
        }
    }

    #[test]
    fn lexical_targets_name_one_node_or_declaration_in_token_order() {
        let tokens = [
            token(LexicalRole::Reference),
            token(LexicalRole::Declaration),
        ];
        let record = |token_index, semantic_id, target_token| RawLexicalTarget {
            token_index,
            semantic_id,
            target_token,
        };
        let decode = |item: RawLexicalTarget, previous: Option<&LexicalTarget>| {
            decode_lexical_target(&item, &tokens, 3, previous)
        };
        let node = decode(record(0, 2, INVALID_ID), None).expect("semantic target");
        assert_eq!(
            (node.token, node.target),
            (0, LexicalTargetRef::Semantic(2))
        );
        let name = decode(record(0, INVALID_ID, 1), Some(&node)).expect("token target");
        assert_eq!(name.target, LexicalTargetRef::Token(1));
        for (index, invalid) in [
            // An unknown token, node or target, no target, two targets and a
            // target that declares nothing.
            record(2, 0, INVALID_ID),
            record(0, 3, INVALID_ID),
            record(0, INVALID_ID, 2),
            record(0, INVALID_ID, INVALID_ID),
            record(0, 0, 1),
            record(0, INVALID_ID, 0),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(decode(invalid, None).is_err(), "record {index}");
        }
        let last = LexicalTarget { token: 1, ..node };
        assert!(decode(record(0, 0, INVALID_ID), Some(&last)).is_err());
    }

    #[test]
    fn macro_definition_ranges_nest() {
        let valid = RawMacroDefinition {
            name: raw_range(8, 9),
            range: raw_range(0, 11),
            body: raw_range(10, 11),
            flags: 0,
            reserved: 0,
        };
        let decoded = decode_macro_definition(&valid, &files()).expect("definition");
        assert!(!decoded.function_like);
        let function_like = RawMacroDefinition { flags: 1, ..valid };
        assert!(
            decode_macro_definition(&function_like, &files())
                .expect("definition")
                .function_like
        );
        for (index, invalid) in [
            RawMacroDefinition {
                name: NO_RANGE,
                ..valid
            },
            RawMacroDefinition {
                name: raw_range(8, 8),
                ..valid
            },
            RawMacroDefinition {
                body: raw_range(8, 9),
                ..valid
            },
            RawMacroDefinition {
                body: raw_range(10, 12),
                ..valid
            },
            RawMacroDefinition { flags: 2, ..valid },
            RawMacroDefinition {
                reserved: 1,
                ..valid
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                decode_macro_definition(&invalid, &files()).is_err(),
                "record {index}"
            );
        }
    }

    #[test]
    fn macro_references_name_a_definition_unless_predefined() {
        let valid = RawMacroReference {
            range: raw_range(2, 4),
            definition_id: 0,
            kind: 1,
            flags: 0,
        };
        let decode = |item: RawMacroReference, previous: Option<&MacroReference>| {
            decode_macro_reference(&item, &files(), 1, previous)
        };
        let used = decode(valid, None).expect("reference");
        assert_eq!(
            (used.kind, used.definition),
            (MacroReferenceKind::Use, Some(0))
        );
        let predefined = RawMacroReference {
            definition_id: INVALID_ID,
            flags: 1,
            ..valid
        };
        assert_eq!(
            decode(predefined, Some(&used))
                .expect("reference")
                .definition,
            None
        );
        for (index, invalid) in [
            RawMacroReference {
                range: NO_RANGE,
                ..valid
            },
            RawMacroReference { kind: 0, ..valid },
            RawMacroReference { kind: 4, ..valid },
            RawMacroReference {
                definition_id: 1,
                ..valid
            },
            RawMacroReference {
                definition_id: INVALID_ID,
                ..valid
            },
            RawMacroReference { flags: 1, ..valid },
            RawMacroReference { flags: 2, ..valid },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(decode(invalid, None).is_err(), "record {index}");
        }
        let earlier = RawMacroReference {
            range: raw_range(0, 1),
            ..valid
        };
        assert!(decode(earlier, Some(&used)).is_err());
    }

    #[test]
    fn declaration_extents_hold_their_names_in_order() {
        let valid = RawDeclarationExtent {
            name: raw_range(3, 5),
            range: raw_range(1, 9),
        };
        let decode = |item: RawDeclarationExtent, previous: Option<&DeclarationExtent>| {
            decode_declaration_extent(&item, &files(), previous)
        };
        let extent = decode(valid, None).expect("extent");
        assert_eq!((extent.range.start, extent.range.end), (1, 9));
        for (index, invalid) in [
            RawDeclarationExtent {
                range: NO_RANGE,
                ..valid
            },
            RawDeclarationExtent {
                name: NO_RANGE,
                ..valid
            },
            RawDeclarationExtent {
                range: raw_range(4, 9),
                ..valid
            },
            RawDeclarationExtent {
                range: raw_range(1, 4),
                ..valid
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(decode(invalid, None).is_err(), "record {index}");
        }
        assert!(decode(valid, Some(&extent)).is_err(), "a repeat");
        let earlier = RawDeclarationExtent {
            name: raw_range(2, 3),
            range: raw_range(0, 9),
        };
        assert!(decode(earlier, Some(&extent)).is_err());
    }

    #[test]
    fn includes_name_an_admitted_file_in_operand_order() {
        let valid = RawInclude {
            operand: raw_range(2, 4),
            file_id: 0,
        };
        let decode = |item: RawInclude, previous: Option<&Include>| {
            decode_include(&item, &files(), previous)
        };
        let include = decode(valid, None).expect("include");
        assert_eq!(include.file_id, 0);
        for (index, invalid) in [
            RawInclude {
                operand: NO_RANGE,
                ..valid
            },
            RawInclude {
                file_id: 1,
                ..valid
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(decode(invalid, None).is_err(), "record {index}");
        }
        let earlier = RawInclude {
            operand: raw_range(0, 1),
            ..valid
        };
        assert!(decode(earlier, Some(&include)).is_err());
    }
}
