//! Slang diagnostic capture and projection over admitted in-memory sources.

use super::*;
use llg::core::compile::slang::{
    self, CompileOptions, CompileRequest, Define, DiagnosticProvider,
    DiagnosticSeverity as SlangSeverity, Limits, ParameterOverride, Source, SourceRange,
};
use tower_lsp::lsp_types::{DiagnosticRelatedInformation, Location};

pub(crate) const SLANG_DIAGNOSTICS_ENV: &str = "LLG_SLANG_DIAGNOSTICS";
const MIN_CAPTURE_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_CAPTURE_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_SLANG_SOURCE_SNAPSHOTS: usize = 4_096;
pub(crate) const MAX_SLANG_PATH_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const MAX_SLANG_OPTION_COUNT: usize = 4_096;
pub(crate) const MAX_SLANG_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlangConfigPreflightError {
    Count { kind: &'static str, count: usize },
    Bytes,
}

/// Check borrowed configuration before constructing the owned vectors needed
/// by the safe FFI facade. Directory counts are conservative across the two
/// already-deduplicated config lists; rejecting cross-list duplicates above
/// the native count limit avoids allocating an unbounded temporary set.
pub(crate) fn slang_config_preflight(
    defines: &[String],
    top: Option<&str>,
    source_dirs: &[PathBuf],
    include_dirs: &[PathBuf],
    parameter_overrides: &BTreeMap<String, String>,
) -> Result<(), SlangConfigPreflightError> {
    for (kind, count) in [
        ("defines", defines.len()),
        (
            "include directories",
            source_dirs.len().saturating_add(include_dirs.len()),
        ),
        ("parameter overrides", parameter_overrides.len()),
    ] {
        if count > MAX_SLANG_OPTION_COUNT {
            return Err(SlangConfigPreflightError::Count { kind, count });
        }
    }

    let mut bytes = top.map_or(0_u64, |top| top.len() as u64);
    for define in defines {
        let define_bytes = define
            .split_once('=')
            .map_or(define.len(), |(name, value)| {
                name.len().saturating_add(value.len())
            });
        bytes = bytes.saturating_add(define_bytes as u64);
    }
    for path in source_dirs.iter().chain(include_dirs) {
        bytes = bytes.saturating_add(path.to_string_lossy().len() as u64);
    }
    for (name, value) in parameter_overrides {
        bytes = bytes
            .saturating_add(name.len() as u64)
            .saturating_add(value.len() as u64);
    }
    if bytes > MAX_SLANG_CONFIG_BYTES {
        return Err(SlangConfigPreflightError::Bytes);
    }
    Ok(())
}

/// Slang is built by the Cargo feature, while diagnostic comparison remains
/// an explicit migration mode so normal all-feature builds keep their current
/// language-server behavior.
pub(crate) fn slang_diagnostics_enabled() -> bool {
    std::env::var_os(SLANG_DIAGNOSTICS_ENV).is_some_and(|value| value == "1")
}

/// One exact source snapshot admitted by the LSP input-budget pass.
pub(crate) struct SlangSource<'a> {
    pub(crate) name: &'a str,
    pub(crate) text: &'a str,
    pub(crate) is_compilation_unit: bool,
}

/// Run the opt-in frontend and project its owned diagnostics for publication.
///
/// This result is diagnostic-only. Callers must not use Slang success to
/// upgrade the Surelog-backed semantic [`AnalysisOutcome`].
pub(crate) fn slang_lsp_diagnostics(
    inputs: &[SlangSource<'_>],
    defines: &[String],
    top: Option<&str>,
    include_dirs: &[String],
    parameter_overrides: &BTreeMap<String, String>,
    max_source_bytes: u64,
) -> Vec<(String, LspDiagnostic)> {
    let sources: Vec<_> = inputs
        .iter()
        .map(|input| {
            if input.is_compilation_unit {
                Source::compilation_unit(input.name, input.text)
            } else {
                Source::include(input.name, input.text)
            }
        })
        .collect();
    let source_name_bytes = inputs.iter().fold(0_u64, |total, input| {
        total.saturating_add(input.name.len() as u64)
    });
    let native_source_bytes = max_source_bytes.saturating_add(source_name_bytes).max(1);
    let output_bytes = max_source_bytes
        .saturating_mul(4)
        .clamp(MIN_CAPTURE_OUTPUT_BYTES, MAX_CAPTURE_OUTPUT_BYTES);
    let options = CompileOptions {
        defines: defines
            .iter()
            .map(|define| {
                let (name, value) = define
                    .split_once('=')
                    .map_or((define.as_str(), None), |(name, value)| {
                        (name, Some(value.to_owned()))
                    });
                Define {
                    name: name.to_owned(),
                    value,
                }
            })
            .collect(),
        top_modules: top.into_iter().map(str::to_owned).collect(),
        include_dirs: include_dirs.to_vec(),
        parameter_overrides: parameter_overrides
            .iter()
            .map(|(name, value)| ParameterOverride {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        limits: Limits {
            max_sources: u64::try_from(sources.len()).unwrap_or(u64::MAX).max(1),
            // LSP admission bounds source text. The safe FFI limit also
            // charges logical filenames, so account for that fixed metadata
            // without shrinking the configured text boundary.
            max_source_bytes: native_source_bytes,
            max_output_bytes: output_bytes,
            ..Limits::default()
        },
    };
    let request = CompileRequest {
        sources: &sources,
        options: &options,
    };
    match slang::compile(&request) {
        Ok(snapshot) => project_snapshot_diagnostics(inputs, &snapshot),
        Err(error) => vec![integration_diagnostic(
            inputs,
            "slang.bridge",
            format!("Slang diagnostic analysis failed: {error}"),
        )],
    }
}

/// Report a bounded-input preflight failure without invoking the native bridge.
pub(crate) fn slang_input_rejected(
    path: &str,
    code: &str,
    message: String,
) -> Vec<(String, LspDiagnostic)> {
    vec![integration_diagnostic_at(path.to_owned(), code, message)]
}

fn project_snapshot_diagnostics(
    inputs: &[SlangSource<'_>],
    snapshot: &slang::Snapshot,
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
            let (path, range) = match primary {
                Some(value) => value,
                None => {
                    let fallback = fallback?;
                    (fallback.name.to_owned(), zero_range())
                }
            };
            let provider = match diagnostic.provider {
                DiagnosticProvider::Compilation => "compiler",
                DiagnosticProvider::Analysis => "analysis",
            };
            let related_information: Vec<_> = diagnostic
                .related
                .iter()
                .filter_map(|related| {
                    let (related_path, range) =
                        project_range(related.range?, &files_by_id, &inputs_by_name)?;
                    let uri = Url::from_file_path(related_path).ok()?;
                    Some(DiagnosticRelatedInformation {
                        location: Location::new(uri, range),
                        message: related.message.clone(),
                    })
                })
                .collect();
            let code_name = if diagnostic.name.is_empty() {
                format!("{}.{}", diagnostic.subsystem, diagnostic.code)
            } else {
                format!(
                    "{}.{}.{}",
                    diagnostic.subsystem, diagnostic.code, diagnostic.name
                )
            };
            Some((
                path,
                LspDiagnostic {
                    range,
                    severity: Some(match diagnostic.severity {
                        SlangSeverity::Ignored => unreachable!("ignored diagnostics are filtered"),
                        SlangSeverity::Note => DiagnosticSeverity::INFORMATION,
                        SlangSeverity::Warning => DiagnosticSeverity::WARNING,
                        SlangSeverity::Error | SlangSeverity::Fatal => DiagnosticSeverity::ERROR,
                    }),
                    code: Some(NumberOrString::String(format!(
                        "slang.{provider}.{code_name}"
                    ))),
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
    files_by_id: &HashMap<u64, &str>,
    inputs_by_name: &HashMap<&str, &str>,
) -> Option<(String, Range)> {
    let path = *files_by_id.get(&range.file_id)?;
    let text = *inputs_by_name.get(path)?;
    let start = byte_position(text, range.start);
    let end = byte_position(text, range.end.max(range.start));
    Some((path.to_owned(), Range::new(start, end)))
}

fn byte_position(text: &str, byte_offset: u64) -> Position {
    let requested = usize::try_from(byte_offset)
        .unwrap_or(usize::MAX)
        .min(text.len());
    let mut end = requested;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut line = 0_u32;
    let mut character = 0_u32;
    let mut values = text[..end].chars().peekable();
    while let Some(value) = values.next() {
        match value {
            '\r' => {
                line = line.saturating_add(1);
                character = 0;
                if values.peek() == Some(&'\n') {
                    values.next();
                }
            }
            '\n' => {
                line = line.saturating_add(1);
                character = 0;
            }
            _ => character = character.saturating_add(value.len_utf16() as u32),
        }
    }
    Position::new(line, character)
}

fn integration_diagnostic(
    inputs: &[SlangSource<'_>],
    code: &str,
    message: String,
) -> (String, LspDiagnostic) {
    let path = inputs
        .iter()
        .find(|input| input.is_compilation_unit)
        .or_else(|| inputs.first())
        .map_or_else(|| "<slang>".to_owned(), |input| input.name.to_owned());
    integration_diagnostic_at(path, code, message)
}

fn integration_diagnostic_at(path: String, code: &str, message: String) -> (String, LspDiagnostic) {
    (
        path,
        LspDiagnostic {
            range: zero_range(),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(code.to_owned())),
            code_description: None,
            source: Some("slang-integration".to_owned()),
            message,
            related_information: None,
            tags: None,
            data: None,
        },
    )
}

fn zero_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offsets_project_to_utf16_positions() {
        let source = "module m;\n  string s = \"😀λ\";\nendmodule\n";
        let emoji = source.find('😀').expect("emoji byte offset");
        let lambda_end = source.find('λ').expect("lambda byte offset") + 'λ'.len_utf8();

        assert_eq!(byte_position(source, emoji as u64), Position::new(1, 14));
        assert_eq!(
            byte_position(source, lambda_end as u64),
            Position::new(1, 17)
        );
        assert_eq!(byte_position("a\rβ", 2), Position::new(1, 0));
        assert_eq!(byte_position("a\r\nβ", 3), Position::new(1, 0));
        assert_eq!(byte_position("a\r\n😀", 7), Position::new(1, 2));
    }

    #[test]
    fn rejected_input_has_integration_origin() {
        let diagnostics = slang_input_rejected(
            "/workspace/top.sv",
            "slang.source-limit",
            "too many sources".to_owned(),
        );

        assert_eq!(diagnostics[0].0, "/workspace/top.sv");
        assert_eq!(
            diagnostics[0].1.source.as_deref(),
            Some("slang-integration")
        );
        assert_eq!(
            diagnostics[0].1.code,
            Some(NumberOrString::String("slang.source-limit".to_owned()))
        );
    }

    #[test]
    fn config_preflight_accepts_and_rejects_option_count_boundary() {
        let at_limit = vec!["D".to_owned(); MAX_SLANG_OPTION_COUNT];
        assert_eq!(
            slang_config_preflight(&at_limit, None, &[], &[], &BTreeMap::new()),
            Ok(())
        );

        let over_limit = vec!["D".to_owned(); MAX_SLANG_OPTION_COUNT + 1];
        assert!(matches!(
            slang_config_preflight(&over_limit, None, &[], &[], &BTreeMap::new()),
            Err(SlangConfigPreflightError::Count {
                kind: "defines",
                count
            }) if count == MAX_SLANG_OPTION_COUNT + 1
        ));
    }

    #[test]
    fn config_preflight_preserves_exact_byte_boundary() {
        let exact = vec![PathBuf::from("x".repeat(MAX_SLANG_CONFIG_BYTES as usize))];
        assert_eq!(
            slang_config_preflight(&[], None, &exact, &[], &BTreeMap::new()),
            Ok(())
        );
        let over = vec![PathBuf::from(
            "x".repeat(MAX_SLANG_CONFIG_BYTES as usize + 1),
        )];
        assert_eq!(
            slang_config_preflight(&[], None, &over, &[], &BTreeMap::new()),
            Err(SlangConfigPreflightError::Bytes)
        );
    }

    #[test]
    fn malformed_source_reaches_the_lsp_projection_with_slang_origin() {
        let source = "module broken;\n  string label = \"😀\";\n  logic value = ;\nendmodule\n";
        let inputs = [SlangSource {
            name: "/workspace/broken.sv",
            text: source,
            is_compilation_unit: true,
        }];

        let projected = slang_lsp_diagnostics(
            &inputs,
            &[],
            Some("broken"),
            &[],
            &BTreeMap::new(),
            source.len() as u64,
        );
        let (_, diagnostic) = projected
            .iter()
            .find(|(_, diagnostic)| diagnostic.source.as_deref() == Some("slang-compiler"))
            .expect("Slang compiler diagnostic");

        assert_eq!(projected[0].0, "/workspace/broken.sv");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code.starts_with("slang.compiler.")
        ));
        assert!(diagnostic.range.start.line >= 2, "{diagnostic:?}");

        let mut analysis = Analysis::new(Vec::new(), empty_design(), Vec::new(), Vec::new());
        let outcome = analysis.outcome;
        analysis.attach_slang_diagnostics(projected);
        let published = lsp_diagnostics(&analysis);
        assert!(published["/workspace/broken.sv"]
            .iter()
            .any(|diagnostic| diagnostic.source.as_deref() == Some("slang-compiler")));
        assert_eq!(analysis.outcome, outcome);
        assert!(!analysis.has_feature_data());
    }
}
