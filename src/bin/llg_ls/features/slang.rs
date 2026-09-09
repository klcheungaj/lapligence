//! Projection of Slang's owned diagnostics to LSP coordinates.

use super::*;
use llg::ffi::slang::{
    DiagnosticProvider, DiagnosticSeverity as SlangSeverity, Snapshot, SourceRange,
};
use tower_lsp::lsp_types::{DiagnosticRelatedInformation, Location};

pub(crate) struct SlangSource<'a> {
    pub(crate) name: &'a str,
    pub(crate) text: &'a str,
    pub(crate) is_compilation_unit: bool,
}

pub(super) fn project_snapshot_diagnostics(
    inputs: &[SlangSource<'_>],
    snapshot: &Snapshot,
) -> Vec<(String, LspDiagnostic)> {
    let inputs_by_name: HashMap<_, _> = inputs
        .iter()
        .map(|input| (input.name, input.text))
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
    snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity != SlangSeverity::Ignored)
        .filter_map(|diagnostic| {
            let primary = diagnostic
                .primary
                .and_then(|range| project_range(range, &files_by_id, &inputs_by_name));
            let (path, range) =
                primary.or_else(|| fallback.map(|input| (input.name.to_owned(), zero_range())))?;
            let provider = match diagnostic.provider {
                DiagnosticProvider::Compilation => "compiler",
                DiagnosticProvider::Analysis => "analysis",
            };
            let related_information: Vec<_> = diagnostic
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
            let name = if diagnostic.name.is_empty() {
                format!("{:?}.{}", diagnostic.subsystem, diagnostic.code)
            } else {
                format!(
                    "{:?}.{}.{}",
                    diagnostic.subsystem, diagnostic.code, diagnostic.name
                )
            };
            Some((
                path,
                LspDiagnostic {
                    range,
                    severity: Some(match diagnostic.severity {
                        SlangSeverity::Ignored => unreachable!(),
                        SlangSeverity::Note => DiagnosticSeverity::INFORMATION,
                        SlangSeverity::Warning => DiagnosticSeverity::WARNING,
                        SlangSeverity::Error | SlangSeverity::Fatal => DiagnosticSeverity::ERROR,
                    }),
                    code: Some(NumberOrString::String(format!("slang.{provider}.{name}"))),
                    code_description: None,
                    source: Some(format!("slang-{provider}")),
                    message: diagnostic.message.clone(),
                    related_information: (!related_information.is_empty())
                        .then_some(related_information),
                    tags: None,
                    data: None,
                },
            ))
        })
        .collect()
}

fn project_range(
    range: SourceRange,
    files: &HashMap<u64, &str>,
    inputs: &HashMap<&str, &str>,
) -> Option<(String, Range)> {
    let path = *files.get(&range.file_id)?;
    let text = *inputs.get(path)?;
    Some((
        path.to_owned(),
        Range::new(
            byte_position(text, range.start),
            byte_position(text, range.end.max(range.start)),
        ),
    ))
}

fn byte_position(text: &str, offset: u64) -> Position {
    let mut end = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut line = 0;
    let mut character = 0;
    let mut chars = text[..end].chars().peekable();
    while let Some(value) = chars.next() {
        match value {
            '\r' => {
                line += 1;
                character = 0;
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            '\n' => {
                line += 1;
                character = 0;
            }
            _ => character += value.len_utf16() as u32,
        }
    }
    Position::new(line, character)
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
            byte_position(text, text.find('😀').unwrap() as u64),
            Position::new(1, 13)
        );
    }
}
