//! Source-library, `` `line`` directive and edition-finding record receivers.

use super::*;

/// Copy one library record, rejecting unknown or repeated nodes and empty
/// names.
pub(super) fn decode_source_library(
    item: &RawSourceLibrary,
    semantic_node_count: usize,
    seen: &mut HashSet<u64>,
) -> Result<SourceLibraryBinding, SlangError> {
    let known = usize::try_from(item.semantic_id).is_ok_and(|index| index < semantic_node_count);
    if !known || !seen.insert(item.semantic_id) {
        return Err(invalid_native(
            "source library record names an invalid or repeated semantic node",
        ));
    }
    // SAFETY: stream records and their strings are valid for the callback
    // that delivered them.
    let library = unsafe { copy_string(item.library, "source library name")? };
    if library.is_empty() {
        return Err(invalid_native("source library record has an empty name"));
    }
    Ok(SourceLibraryBinding {
        semantic_id: item.semantic_id,
        library,
    })
}

/// Copy one `` `line`` mapping, rejecting unknown files and offsets outside
/// the file.
pub(super) fn decode_line_directive(
    item: &RawLineDirective,
    files: &[File],
) -> Result<LineDirective, SlangError> {
    let file = files
        .iter()
        .find(|file| file.id == item.file_id)
        .ok_or_else(|| invalid_native("line directive refers to an unknown file"))?;
    if item.physical_offset >= file.byte_len {
        return Err(invalid_native(
            "line directive offset lies outside its file",
        ));
    }
    Ok(LineDirective {
        file_id: item.file_id,
        physical_offset: item.physical_offset,
        logical_line: item.logical_line,
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        logical_file: unsafe { copy_string(item.logical_file, "line directive file")? },
    })
}

/// Sort the received mappings by file and offset so consumers can
/// binary-search them. Repeated records for one line start (an include
/// admitted as several buffers) must agree and collapse to one.
pub(super) fn finish_line_directives(
    mut directives: Vec<LineDirective>,
) -> Result<Vec<LineDirective>, SlangError> {
    directives.sort_by(|left, right| {
        (left.file_id, left.physical_offset).cmp(&(right.file_id, right.physical_offset))
    });
    let mut unique: Vec<LineDirective> = Vec::with_capacity(directives.len());
    for directive in directives {
        match unique.last() {
            Some(last)
                if last.file_id == directive.file_id
                    && last.physical_offset == directive.physical_offset =>
            {
                if *last != directive {
                    return Err(invalid_native(
                        "line directive records disagree for one mapped line",
                    ));
                }
            }
            _ => unique.push(directive),
        }
    }
    Ok(unique)
}

const PROTECT_DECRYPTION: u32 = 1;
const PROTECT_ENCRYPTION: u32 = 2;
const PROTECT_LEGACY: u32 = 1 << 0;
const PROTECT_UNTERMINATED: u32 = 1 << 1;

/// Copy one protected envelope, rejecting unknown kinds, reserved flag bits,
/// missing ranges and records out of file/start order (the ABI sorts them).
pub(super) fn decode_protected_envelope(
    item: &RawProtectedEnvelope,
    files: &[File],
    previous: Option<&ProtectedEnvelope>,
) -> Result<ProtectedEnvelope, SlangError> {
    let range = decode_range(item.range, files)?
        .ok_or_else(|| invalid_native("protected envelope has no source range"))?;
    let scope = decode_range(item.scope, files)?;
    let kind = match item.kind {
        PROTECT_DECRYPTION => ProtectedEnvelopeKind::Decryption,
        PROTECT_ENCRYPTION => ProtectedEnvelopeKind::Encryption,
        _ => return Err(invalid_native("protected envelope has an unknown kind")),
    };
    if item.flags & !(PROTECT_LEGACY | PROTECT_UNTERMINATED) != 0 {
        return Err(invalid_native("protected envelope has unknown flags"));
    }
    if previous.is_some_and(|previous| {
        (previous.range.file_id, previous.range.start) >= (range.file_id, range.start)
    }) {
        return Err(invalid_native(
            "protected envelope records are not sorted by file and start",
        ));
    }
    Ok(ProtectedEnvelope {
        range,
        scope,
        kind,
        legacy: item.flags & PROTECT_LEGACY != 0,
        unterminated: item.flags & PROTECT_UNTERMINATED != 0,
    })
}

/// Copy one syntax-profile finding, rejecting unknown or unbounded ranges and
/// empty labels.
pub(super) fn decode_edition_finding(
    item: &RawEditionFinding,
    files: &[File],
) -> Result<EditionFinding, SlangError> {
    let range = decode_range(item.range, files)?
        .ok_or_else(|| invalid_native("edition finding has no source range"))?;
    // SAFETY: stream records and their strings are valid for the callback
    // that delivered them.
    let label = unsafe { copy_string(item.label, "edition finding label")? };
    if label.is_empty() {
        return Err(invalid_native("edition finding has an empty label"));
    }
    Ok(EditionFinding { range, label })
}

const FSM_PRAGMA: u32 = 1;
const FSM_INSTANCE: u32 = 2;
const FSM_LINE_COMMENT: u32 = 1 << 0;
const FSM_SIGNAL_DECLARATOR: u32 = 1 << 1;
const FSM_PARAMETER_DECLARATOR: u32 = 1 << 2;

/// One decoded `LlgSlangFsmPragma` record.
pub(super) enum DecodedFsmPragma {
    Pragma(FsmPragma),
    Instance(FsmPragmaInstance),
}

/// The scope keys of the received pragmas, so instance records can be
/// checked against them.
pub(super) type FsmScopeKey = (u64, u64, u64);

fn fsm_scope_key(range: SourceRange) -> FsmScopeKey {
    (range.file_id, range.start, range.end)
}

/// Whether `text` starts with the pragma keyword `tool` and white space, as
/// the wrapper selects comments.
fn starts_with_tool(text: &str) -> bool {
    let rest = text.trim_start_matches([' ', '\t', '\r', '\n', '\u{c}', '\u{b}']);
    rest.strip_prefix("tool")
        .and_then(|after| after.chars().next())
        .is_some_and(|next| matches!(next, ' ' | '\t' | '\r' | '\n' | '\u{c}' | '\u{b}'))
}

/// Copy one FSM pragma or instance record (IEEE 1800-2009 40.4). Pragma
/// records must precede instance records, come sorted by file, comment start
/// and ordinal, span their text and its delimiters, and keep one
/// comment's declarators consecutive from ordinal 0; instance records must
/// name a distinct instance node of a scope some pragma reported. Reserved
/// fields and unknown kinds or flags are rejected.
pub(super) fn decode_fsm_pragma(
    item: &RawFsmPragma,
    files: &[File],
    nodes: &[SemanticNode],
    previous: Option<&FsmPragma>,
    scopes: &HashSet<FsmScopeKey>,
    instances: &mut HashSet<(FsmScopeKey, u64)>,
) -> Result<DecodedFsmPragma, SlangError> {
    if item.reserved != 0 {
        return Err(invalid_native(
            "FSM pragma record has a nonzero reserved field",
        ));
    }
    let range = decode_range(item.range, files)?;
    let scope = decode_range(item.scope, files)?;
    let declarator = decode_range(item.declarator, files)?;
    match item.kind {
        FSM_INSTANCE => {
            let scope = scope.ok_or_else(|| invalid_native("FSM instance record has no scope"))?;
            if range.is_some()
                || declarator.is_some()
                || item.flags != 0
                || item.ordinal != 0
                || item.text.len != 0
            {
                return Err(invalid_native("FSM instance record carries pragma fields"));
            }
            let key = fsm_scope_key(scope);
            if !scopes.contains(&key) {
                return Err(invalid_native(
                    "FSM instance record names a scope without pragmas",
                ));
            }
            let node = usize::try_from(item.instance_id)
                .ok()
                .and_then(|index| nodes.get(index))
                .ok_or_else(|| invalid_native("FSM instance record names an unknown node"))?;
            if node.kind != SemanticKind::Instance {
                return Err(invalid_native(
                    "FSM instance record names a node that is not an instance",
                ));
            }
            if !instances.insert((key, item.instance_id)) {
                return Err(invalid_native("FSM instance record is repeated"));
            }
            Ok(DecodedFsmPragma::Instance(FsmPragmaInstance {
                scope,
                instance: item.instance_id,
            }))
        }
        FSM_PRAGMA => {
            if !instances.is_empty() {
                return Err(invalid_native(
                    "FSM pragma record follows an instance record",
                ));
            }
            if item.instance_id != INVALID_ID {
                return Err(invalid_native("FSM pragma record names an instance"));
            }
            let range = range.ok_or_else(|| invalid_native("FSM pragma has no source range"))?;
            if item.flags & !(FSM_LINE_COMMENT | FSM_SIGNAL_DECLARATOR | FSM_PARAMETER_DECLARATOR)
                != 0
            {
                return Err(invalid_native("FSM pragma has unknown flags"));
            }
            let kind = match (
                item.flags & FSM_SIGNAL_DECLARATOR != 0,
                item.flags & FSM_PARAMETER_DECLARATOR != 0,
            ) {
                (false, false) => None,
                (true, false) => Some(FsmDeclaratorKind::Signal),
                (false, true) => Some(FsmDeclaratorKind::Parameter),
                (true, true) => {
                    return Err(invalid_native(
                        "FSM pragma is bound to two declaration kinds",
                    ))
                }
            };
            let declarator = match (kind, declarator) {
                (Some(kind), Some(name)) => Some(FsmPragmaDeclarator {
                    kind,
                    name,
                    ordinal: item.ordinal,
                }),
                (None, None) if item.ordinal == 0 => None,
                _ => {
                    return Err(invalid_native(
                        "FSM pragma declarator fields disagree with its flags",
                    ))
                }
            };
            // SAFETY: stream records and their strings are valid for the
            // callback that delivered them.
            let text = unsafe { copy_string(item.text, "FSM pragma text")? };
            let line_comment = item.flags & FSM_LINE_COMMENT != 0;
            // File text is attached after the stream; the delimiters make the
            // comment exactly two (`//`) or four (`/*`, `*/`) bytes longer.
            let delimiters = if line_comment { 2 } else { 4 };
            let spelled =
                (text.len() as u64).checked_add(delimiters) == Some(range.end - range.start);
            if !spelled || !starts_with_tool(&text) {
                return Err(invalid_native("FSM pragma text does not match its comment"));
            }
            let pragma = FsmPragma {
                range,
                scope,
                line_comment,
                declarator,
                text,
            };
            let ordinal = pragma.declarator.map_or(0, |declarator| declarator.ordinal);
            match previous {
                Some(previous)
                    if (previous.range.file_id, previous.range.start)
                        == (range.file_id, range.start) =>
                {
                    let previous_ordinal = previous
                        .declarator
                        .map(|declarator| (declarator.kind, declarator.ordinal));
                    let consecutive = previous_ordinal.zip(pragma.declarator).is_some_and(
                        |((kind, ordinal_before), declarator)| {
                            kind == declarator.kind
                                && ordinal_before.checked_add(1) == Some(declarator.ordinal)
                        },
                    );
                    if !consecutive
                        || previous.range != pragma.range
                        || previous.scope != pragma.scope
                        || previous.text != pragma.text
                    {
                        return Err(invalid_native(
                            "FSM pragma declarator records of one comment disagree",
                        ));
                    }
                }
                Some(previous)
                    if (previous.range.file_id, previous.range.start)
                        > (range.file_id, range.start) =>
                {
                    return Err(invalid_native(
                        "FSM pragma records are not sorted by file and start",
                    ))
                }
                _ if ordinal != 0 => {
                    return Err(invalid_native(
                        "FSM pragma declarator records do not start at ordinal 0",
                    ))
                }
                _ => {}
            }
            Ok(DecodedFsmPragma::Pragma(pragma))
        }
        _ => Err(invalid_native("FSM pragma record has an unknown kind")),
    }
}
