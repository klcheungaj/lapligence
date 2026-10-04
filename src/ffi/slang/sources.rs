//! Source-library and `` `line`` directive record receivers.

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
