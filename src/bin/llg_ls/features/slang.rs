//! Projection of Slang's owned diagnostics to LSP coordinates.

use super::*;
use llg::core::tokens::SourcePositions;
use llg::ffi::slang::{
    DiagnosticProvider, DiagnosticSeverity as SlangSeverity, Snapshot, SourceRange,
};
use tower_lsp::lsp_types::{DiagnosticRelatedInformation, Location};

pub(crate) struct SlangSource<'a> {
    pub(crate) name: &'a str,
    pub(crate) text: &'a str,
    pub(crate) is_compilation_unit: bool,
}

/// Project the frontend diagnostics of `snapshot` that `dispositions` (one
/// per diagnostic; missing entries keep) does not drop.
pub(super) fn project_snapshot_diagnostics(
    inputs: &[SlangSource<'_>],
    snapshot: &Snapshot,
    dispositions: &[Disposition],
) -> Vec<(String, LspDiagnostic)> {
    let inputs_by_name: HashMap<_, _> = inputs
        .iter()
        .map(|input| (input.name, SourcePositions::new(input.text)))
        .collect();
    let files_by_id: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.id, file.name.as_str()))
        .collect();
    let fallback = inputs
        .iter()
        .find(|input| input.is_compilation_unit)
        .or_else(|| inputs.first());
    let source_map = (!snapshot.line_directives.is_empty())
        .then(|| llg::core::db::SourceMap::from_snapshot(snapshot).ok())
        .flatten();
    snapshot
        .diagnostics
        .iter()
        .enumerate()
        .filter(|(_, diagnostic)| diagnostic.severity != SlangSeverity::Ignored)
        .filter_map(|(index, diagnostic)| {
            let disposition = dispositions
                .get(index)
                .copied()
                .unwrap_or(Disposition::Keep);
            if disposition == Disposition::Drop {
                return None;
            }
            let primary = diagnostic
                .primary
                .and_then(|range| project_range(range, &files_by_id, &inputs_by_name));
            let (path, range) =
                primary.or_else(|| fallback.map(|input| (input.name.to_owned(), zero_range())))?;
            let provider = match diagnostic.provider {
                DiagnosticProvider::Compilation => "compiler",
                DiagnosticProvider::Analysis => "analysis",
            };
            let mut related_information: Vec<_> = diagnostic
                .related
                .iter()
                .filter_map(|related| {
                    let (path, range) =
                        project_range(related.range?, &files_by_id, &inputs_by_name)?;
                    Some(DiagnosticRelatedInformation {
                        location: Location::new(Url::from_file_path(path).ok()?, range),
                        message: related.message.clone(),
                    })
                })
                .collect();
            if let Some(origin) = source_map.as_ref().and_then(|map| {
                let logical = map.logical_position(&path, range.start.line.saturating_add(1))?;
                mapped_origin(&path, range, logical.file, logical.line)
            }) {
                related_information.insert(0, origin);
            }
            let name = if diagnostic.name.is_empty() {
                format!("{:?}.{}", diagnostic.subsystem, diagnostic.code)
            } else {
                format!(
                    "{:?}.{}.{}",
                    diagnostic.subsystem, diagnostic.code, diagnostic.name
                )
            };
            let (severity, message) = match disposition {
                Disposition::Hint(reason) => (
                    DiagnosticSeverity::HINT,
                    hint_message(&diagnostic.message, reason),
                ),
                _ => (
                    match diagnostic.severity {
                        SlangSeverity::Ignored => unreachable!(),
                        SlangSeverity::Note => DiagnosticSeverity::INFORMATION,
                        SlangSeverity::Warning => DiagnosticSeverity::WARNING,
                        SlangSeverity::Error | SlangSeverity::Fatal => DiagnosticSeverity::ERROR,
                    },
                    diagnostic.message.clone(),
                ),
            };
            Some((
                path,
                LspDiagnostic {
                    range,
                    severity: Some(severity),
                    code: Some(NumberOrString::String(format!("slang.{provider}.{name}"))),
                    code_description: None,
                    source: Some(format!("slang-{provider}")),
                    message,
                    related_information: (!related_information.is_empty())
                        .then_some(related_information),
                    tags: None,
                    data: None,
                },
            ))
        })
        .collect()
}

/// Related information naming the `` `line``-mapped origin (IEEE 1364-2001
/// 19.7, IEEE 1800-2009 22.12) of a diagnostic. The location stays the
/// physical buffer range, which is the one an editor can open; the mapped
/// file need not exist on disk.
pub(super) fn mapped_origin(
    path: &str,
    range: Range,
    logical_file: &str,
    logical_line: u64,
) -> Option<DiagnosticRelatedInformation> {
    Some(DiagnosticRelatedInformation {
        location: Location::new(Url::from_file_path(path).ok()?, range),
        message: format!("`line origin: {logical_file}:{logical_line}"),
    })
}

fn project_range(
    range: SourceRange,
    files: &HashMap<u64, &str>,
    inputs: &HashMap<&str, SourcePositions<'_>>,
) -> Option<(String, Range)> {
    let path = *files.get(&range.file_id)?;
    let source = inputs.get(path)?;
    Some((
        path.to_owned(),
        Range::new(
            byte_position(source, range.start),
            byte_position(source, range.end.max(range.start)),
        ),
    ))
}

/// LSP position of a byte offset in an indexed source.
pub(super) fn byte_position(source: &SourcePositions<'_>, offset: u64) -> Position {
    let (line, column) = source.position(offset);
    Position::new(line - 1, column - 1)
}

fn zero_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offsets_use_utf16_coordinates() {
        let text = "module m;\n string s = \"😀λ\";\nendmodule\n";
        assert_eq!(
            byte_position(&SourcePositions::new(text), text.find('😀').unwrap() as u64),
            Position::new(1, 13)
        );
        // CR, LF and CRLF each end one line; an offset inside a CRLF pair and
        // one inside a character clamp.
        let text = "a\rb\nc\r\nd😀";
        let source = SourcePositions::new(text);
        for (offset, expected) in [
            (0, (0, 0)),
            (2, (1, 0)),
            (4, (2, 0)),
            (6, (3, 0)),
            (7, (3, 0)),
            (10, (3, 1)),
            (100, (3, 3)),
        ] {
            let (line, character) = expected;
            assert_eq!(
                byte_position(&source, offset),
                Position::new(line, character),
                "{offset}"
            );
        }
    }
}
