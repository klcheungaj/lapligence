//! Logical (`` `line``-mapped) source positions.
//!
//! Every [`Node`](super::Node) keeps its physical file, line and column; that
//! physical range is the diagnostic identity. A `` `line`` directive (IEEE
//! 1364-2001 19.7, IEEE 1800-2009 22.12) only changes the logical file and
//! line that `` `__FILE__`` / `` `__LINE__`` report for the lines after it.
//! This table keeps that logical view as a separate owned fact so consumers
//! can show both without re-reading source or retaining the native snapshot.

use super::DbError;
use crate::ffi::slang::Snapshot as SlangSnapshot;
use std::collections::HashMap;

/// Logical file and line of one physical source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalPosition<'a> {
    pub file: &'a str,
    pub line: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LineMapping {
    /// First physical line, one-based, that this mapping covers.
    physical_line: u32,
    /// Logical line of `physical_line`.
    logical_line: u64,
    logical_file: String,
}

/// Per-file `` `line`` mappings keyed by the physical file name that
/// [`Node::file`](super::Node::file) reports. Files without a directive have
/// no entry, so the common case costs one failed hash lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceMap {
    files: HashMap<String, Vec<LineMapping>>,
}

impl SourceMap {
    /// Build the mappings from a snapshot's `` `line`` records; used by
    /// consumers, such as frontend diagnostics, that run before a Db exists.
    pub fn from_snapshot(snapshot: &SlangSnapshot) -> Result<Self, DbError> {
        Self::from_slang(snapshot)
    }

    pub(super) fn from_slang(snapshot: &SlangSnapshot) -> Result<Self, DbError> {
        let mut files: HashMap<String, Vec<LineMapping>> = HashMap::new();
        // The snapshot sorts records by file and offset; walk each file's text
        // once to turn offsets into the Db's one-based physical lines.
        let mut index = 0;
        while index < snapshot.line_directives.len() {
            let file_id = snapshot.line_directives[index].file_id;
            let file = snapshot
                .files
                .iter()
                .find(|file| file.id == file_id)
                .ok_or_else(|| DbError::InvalidSnapshot("line directive file is missing".into()))?;
            let bytes = file.text.as_bytes();
            let mut line = 1_u32;
            let mut scanned = 0_usize;
            let mut mappings = Vec::new();
            while let Some(directive) = snapshot
                .line_directives
                .get(index)
                .filter(|directive| directive.file_id == file_id)
            {
                let offset = usize::try_from(directive.physical_offset)
                    .ok()
                    .filter(|offset| *offset <= bytes.len())
                    .ok_or_else(|| {
                        DbError::InvalidSnapshot("line directive offset is outside its file".into())
                    })?;
                let newlines = bytes[scanned..offset]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count();
                line = line.saturating_add(u32::try_from(newlines).unwrap_or(u32::MAX));
                scanned = offset;
                mappings.push(LineMapping {
                    physical_line: line,
                    logical_line: directive.logical_line,
                    logical_file: directive.logical_file.clone(),
                });
                index += 1;
            }
            // The same physical file may be admitted under one name more than
            // once (an include reached from two places); its text, and so its
            // mappings, are identical.
            files.entry(file.name.clone()).or_insert(mappings);
        }
        Ok(Self { files })
    }

    /// True when no admitted file contains a `` `line`` directive.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Logical position of a physical line, or `None` when no `` `line``
    /// directive precedes that line in `file`. Callers then use the physical
    /// position, which is also the logical one.
    pub fn logical_position(&self, file: &str, line: u32) -> Option<LogicalPosition<'_>> {
        if self.files.is_empty() {
            return None;
        }
        let mappings = self.files.get(file)?;
        let index = mappings.partition_point(|mapping| mapping.physical_line <= line);
        let mapping = mappings.get(index.checked_sub(1)?)?;
        Some(LogicalPosition {
            file: &mapping.logical_file,
            line: mapping
                .logical_line
                .saturating_add(u64::from(line - mapping.physical_line)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(u32, u64, &str)]) -> SourceMap {
        SourceMap {
            files: HashMap::from([(
                "top.sv".to_owned(),
                entries
                    .iter()
                    .map(|(physical_line, logical_line, file)| LineMapping {
                        physical_line: *physical_line,
                        logical_line: *logical_line,
                        logical_file: (*file).to_owned(),
                    })
                    .collect(),
            )]),
        }
    }

    #[test]
    fn lines_before_the_first_directive_have_no_logical_override() {
        let map = map(&[(5, 100, "gen.sv")]);
        assert_eq!(map.logical_position("top.sv", 4), None);
        assert_eq!(map.logical_position("other.sv", 9), None);
    }

    #[test]
    fn logical_lines_advance_with_physical_lines_until_the_next_directive() {
        let map = map(&[(5, 100, "gen.sv"), (9, 3, "orig.sv")]);
        assert_eq!(
            map.logical_position("top.sv", 5),
            Some(LogicalPosition {
                file: "gen.sv",
                line: 100
            })
        );
        assert_eq!(
            map.logical_position("top.sv", 8),
            Some(LogicalPosition {
                file: "gen.sv",
                line: 103
            })
        );
        assert_eq!(
            map.logical_position("top.sv", 12),
            Some(LogicalPosition {
                file: "orig.sv",
                line: 6
            })
        );
    }
}
