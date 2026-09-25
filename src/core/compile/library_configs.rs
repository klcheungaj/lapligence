//! Separate configuration declarations from map paths without interpreting HDL.
//!
//! The product follows the supplied V/SV Annex A library-text grammar. The
//! native parser still owns configuration syntax and semantics. The derived
//! input retains the original name, byte length and byte offsets; only
//! non-configuration bytes are replaced by whitespace. No emitted-C fallback
//! or additional filesystem access is involved.

use super::{LibraryMapWorkBudget, StartupError, StartupErrorKind};
use std::ops::Range;

fn malformed(offset: usize, message: &str) -> StartupError {
    StartupError::new(
        StartupErrorKind::InvalidArgument,
        format!("library map configuration at byte {offset}: {message}"),
    )
}

fn trivia(bytes: &[u8], mut index: usize) -> Result<usize, StartupError> {
    loop {
        match bytes.get(index) {
            Some(byte) if byte.is_ascii_whitespace() => index += 1,
            Some(b'/') if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            Some(b'/') if bytes.get(index + 1) == Some(&b'*') => {
                let start = index;
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                if index + 1 >= bytes.len() {
                    return Err(malformed(start, "unterminated comment"));
                }
                index += 2;
            }
            _ => return Ok(index),
        }
    }
}

fn identifier_end(bytes: &[u8], mut index: usize) -> usize {
    if bytes.get(index) == Some(&b'\\') {
        // Escaped identifiers end at whitespace, not at HDL punctuation.
        index += 1;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
    } else {
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'$'))
        {
            index += 1;
        }
    }
    index
}

/// Scan only to the unescaped end keyword (and optional named end clause).
/// The caller reserves linear tokenization work for the entire input before
/// this scan. Bodies are disjoint; trivia lookahead adds at most one more pass.
pub(super) fn declaration_end(text: &str, mut index: usize) -> Result<usize, StartupError> {
    let bytes = text.as_bytes();
    let start = index;
    while index < bytes.len() {
        index = trivia(bytes, index)?;
        let Some(byte) = bytes.get(index).copied() else {
            break;
        };
        if byte == b'"' {
            let quote = index;
            index += 1;
            loop {
                match bytes.get(index) {
                    None => return Err(malformed(quote, "unterminated string")),
                    Some(b'"') => {
                        index += 1;
                        break;
                    }
                    Some(b'\\') => index = (index + 2).min(bytes.len()),
                    _ => index += 1,
                }
            }
        } else if byte == b'\\' || byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') {
            let end = identifier_end(bytes, index);
            if &bytes[index..end] == b"endconfig" {
                let next = trivia(bytes, end)?;
                if bytes.get(next) == Some(&b':') {
                    let name = trivia(bytes, next + 1)?;
                    return Ok(identifier_end(bytes, name));
                }
                return Ok(end);
            }
            index = end;
        } else {
            index += 1;
        }
    }
    Err(malformed(start, "missing endconfig"))
}

pub(super) fn project(
    text: &str,
    declarations: &[Range<usize>],
    work: &mut LibraryMapWorkBudget,
) -> Result<Option<String>, StartupError> {
    if declarations.is_empty() {
        return Ok(None);
    }
    work.charge_usize(text.len(), "configuration projection")?;
    work.charge_allocation_usize(text.len(), "configuration projection")?;
    let mut bytes = text.as_bytes().to_vec();
    let mut previous = 0;
    for range in declarations {
        if range.start < previous || range.end < range.start || range.end > bytes.len() {
            return Err(StartupError::new(StartupErrorKind::Internal, "invalid map configuration span"));
        }
        mask(&mut bytes[previous..range.start]);
        previous = range.end;
    }
    mask(&mut bytes[previous..]);
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| StartupError::new(StartupErrorKind::Internal, "invalid configuration projection UTF-8"))
}

fn mask(bytes: &mut [u8]) {
    for byte in bytes {
        if !matches!(*byte, b'\r' | b'\n') {
            *byte = b' ';
        }
    }
}

/// Restore exact admitted map text after Slang parsed the position-preserving
/// projection. Semantic ranges still refer to the same configuration bytes.
/// Diagnostics and owned import must see original UTF-8, not a space per byte,
/// so UTF-16 columns remain correct even after non-ASCII text on the same line.
/// Ownership moves from the already-budgeted originals; no second text clone.
pub(super) fn restore_source_text(
    files: &mut [crate::ffi::slang::File],
    originals: Vec<super::OwnedSource>,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    if originals.is_empty() {
        return Ok(());
    }
    let size = originals.len().checked_mul(std::mem::size_of::<(String, String)>())
        .ok_or_else(|| work.limit_error("configuration provenance index"))?;
    work.charge_allocation_usize(size, "configuration provenance index")?;
    let mut texts = std::collections::HashMap::with_capacity(originals.len());
    for original in originals {
        work.charge_usize(original.name.len(), "configuration provenance name")?;
        if texts.insert(original.name, original.text).is_some() {
            return Err(StartupError::new(StartupErrorKind::Internal, "duplicate original map"));
        }
    }
    for file in files {
        work.charge_usize(file.name.len(), "configuration provenance lookup")?;
        if let Some(text) = texts.remove(&file.name) {
            if file.byte_len != text.len() as u64 || file.text.len() != text.len() {
                return Err(StartupError::new(
                    StartupErrorKind::Internal,
                    format!("configuration map length changed during capture: {}", file.name),
                ));
            }
            file.text = text;
        }
    }
    if !texts.is_empty() {
        return Err(StartupError::new(
            StartupErrorKind::Internal, "configuration map missing from native snapshot",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
