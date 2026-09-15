//! Diagnostics.

use super::*;

pub(super) fn decode_related(
    raw: &[RawRelatedDiagnostic],
    files: &[File],
) -> Result<Vec<RelatedDiagnostic>, SlangError> {
    raw.iter()
        .map(|item| {
            Ok(RelatedDiagnostic {
                range: decode_range(item.range, files)?,
                // SAFETY: native strings borrow from the live snapshot.
                message: unsafe { copy_string(item.message, "related diagnostic message")? },
            })
        })
        .collect()
}

pub(super) fn decode_diagnostics(
    raw: &[RawDiagnostic],
    related: &[RelatedDiagnostic],
    files: &[File],
) -> Result<Vec<Diagnostic>, SlangError> {
    raw.iter()
        .map(|item| {
            let related_range = checked_window(
                item.related_start,
                item.related_count,
                related.len(),
                "diagnostic related records",
            )?;
            Ok(Diagnostic {
                provider: match item.provider {
                    1 => DiagnosticProvider::Compilation,
                    2 => DiagnosticProvider::Analysis,
                    _ => return Err(invalid_native("diagnostic has an unknown provider")),
                },
                severity: match item.severity {
                    0 => DiagnosticSeverity::Ignored,
                    1 => DiagnosticSeverity::Note,
                    2 => DiagnosticSeverity::Warning,
                    3 => DiagnosticSeverity::Error,
                    4 => DiagnosticSeverity::Fatal,
                    _ => return Err(invalid_native("diagnostic has an unknown severity")),
                },
                subsystem: decode_diagnostic_subsystem(item.subsystem)?,
                code: item.code,
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "diagnostic name")? },
                // SAFETY: native strings borrow from the live snapshot.
                option_name: unsafe { copy_string(item.option_name, "diagnostic option name")? },
                // SAFETY: native strings borrow from the live snapshot.
                message: unsafe { copy_string(item.message, "diagnostic message")? },
                primary: decode_range(item.primary, files)?,
                related: related[related_range].to_vec(),
            })
        })
        .collect()
}

fn decode_diagnostic_subsystem(raw: u32) -> Result<DiagnosticSubsystem, SlangError> {
    Ok(match raw {
        0 => DiagnosticSubsystem::Invalid,
        1 => DiagnosticSubsystem::General,
        2 => DiagnosticSubsystem::Lexer,
        3 => DiagnosticSubsystem::Numeric,
        4 => DiagnosticSubsystem::Preprocessor,
        5 => DiagnosticSubsystem::Parser,
        6 => DiagnosticSubsystem::Declarations,
        7 => DiagnosticSubsystem::Expressions,
        8 => DiagnosticSubsystem::Statements,
        9 => DiagnosticSubsystem::Types,
        10 => DiagnosticSubsystem::Lookup,
        11 => DiagnosticSubsystem::SysFuncs,
        12 => DiagnosticSubsystem::ConstEval,
        13 => DiagnosticSubsystem::Compilation,
        14 => DiagnosticSubsystem::Analysis,
        15 => DiagnosticSubsystem::Meta,
        16 => DiagnosticSubsystem::Driver,
        17 => DiagnosticSubsystem::Tidy,
        18 => DiagnosticSubsystem::Netlist,
        _ => return Err(invalid_native("diagnostic has an unknown subsystem")),
    })
}
