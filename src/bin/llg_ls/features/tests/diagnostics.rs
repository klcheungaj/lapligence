//! Diagnostics.

use super::*;

#[test]
fn slang_diagnostics_preserve_location_and_message() {
    let raw = Diag {
        severity: Severity::Syntax,
        file: Some("/x/debug_TEMPLATE.v".to_owned()),
        line: 2,
        col: 22,
        message: "expected a statement".to_owned(),
    };
    let a = Analysis::new(vec![raw.clone()], empty_design(), Vec::new(), Vec::new());
    let map = lsp_diagnostics(&a);
    let diagnostics = &map["/x/debug_TEMPLATE.v"];
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, raw.message);
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diagnostics[0].source.as_deref(), Some("slang"));
    assert_eq!(diagnostics[0].range.start, Position::new(1, 21));
    assert_eq!(a.diagnostics[0], raw);
}

#[test]
fn diagnostics_severity_mapping() {
    let a = Analysis::new(
        vec![
            Diag {
                severity: Severity::Error,
                file: Some("/x/a.sv".to_owned()),
                line: 3,
                col: 5,
                message: "bad".to_owned(),
            },
            Diag {
                severity: Severity::Warning,
                file: Some("/x/a.sv".to_owned()),
                line: 4,
                col: 1,
                message: "warn".to_owned(),
            },
            Diag {
                severity: Severity::Note,
                file: Some("/x/a.sv".to_owned()),
                line: 0,
                col: 0,
                message: "note".to_owned(),
            },
            Diag {
                severity: Severity::Info,
                file: Some("/x/a.sv".to_owned()),
                line: 6,
                col: 2,
                message: "info".to_owned(),
            },
        ],
        empty_design(),
        Vec::new(),
        Vec::new(),
    );
    let map = lsp_diagnostics(&a);
    let diags = map.get("/x/a.sv").expect("diags for a.sv");
    assert_eq!(diags.len(), 4);
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diags[0].range.start.line, 2); // 1-based → 0-based
    assert_eq!(diags[0].range.start.character, 4);
    assert_eq!(diags[1].severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(diags[2].severity, Some(DiagnosticSeverity::INFORMATION));
    assert_eq!(diags[2].range.start.line, 0); // unknown line → (0,0)
    assert_eq!(diags[3].severity, Some(DiagnosticSeverity::HINT));
}

#[test]
fn fileless_synthetic_diagnostic_uses_supplied_fallback_path() {
    let message = "semantic database build failed: node walk failed";
    let a = Analysis::new(
        vec![db_build_diagnostic("node walk failed")],
        empty_design(),
        Vec::new(),
        Vec::new(),
    );

    let map = lsp_diagnostics_with_fallback(&a, Some(Path::new("/x/top.sv")));
    let diagnostics = map.get("/x/top.sv").expect("fallback-file diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, message);
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
}

#[test]
fn lint_diagnostics_mapping() {
    use llg::core::lint::{LintDiag, LintSeverity};
    let a = Analysis::new(
        vec![Diag {
            severity: Severity::Error,
            file: Some("/x/a.sv".to_owned()),
            line: 1,
            col: 1,
            message: "compile error".to_owned(),
        }],
        empty_design(),
        Vec::new(),
        vec![
            LintDiag {
                rule: "unused-signal".to_owned(),
                severity: LintSeverity::Error,
                file: Some("/x/a.sv".to_owned()),
                line: 3,
                col: 5,
                message: "signal `x` in `m` is never used".to_owned(),
            },
            LintDiag {
                rule: "width-mismatch".to_owned(),
                severity: LintSeverity::Warning,
                file: Some("/x/a.sv".to_owned()),
                line: 4,
                col: 1,
                message: "truncation".to_owned(),
            },
            LintDiag {
                rule: "multi-driver".to_owned(),
                severity: LintSeverity::Info,
                file: Some("/x/b.sv".to_owned()),
                line: 0,
                col: 0,
                message: "multiple drivers".to_owned(),
            },
        ],
    );
    let map = lsp_diagnostics(&a);
    let a_diags = map.get("/x/a.sv").expect("diags for a.sv");
    assert_eq!(a_diags.len(), 3, "frontend + two lint diags: {a_diags:?}");
    let lint: Vec<&LspDiagnostic> = a_diags
        .iter()
        .filter(|d| d.source.as_deref() == Some("llg-lint"))
        .collect();
    assert_eq!(lint.len(), 2, "lint diags: {lint:?}");
    assert_eq!(lint[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(
        lint[0].code,
        Some(NumberOrString::String("unused-signal".to_owned()))
    );
    assert_eq!(lint[0].range.start.line, 2); // 1-based → 0-based
    assert_eq!(lint[0].range.start.character, 4);
    assert!(lint[0].message.contains("never used"));
    assert_eq!(lint[1].severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(
        lint[1].code,
        Some(NumberOrString::String("width-mismatch".to_owned()))
    );
    assert_eq!(lint[1].range.start.line, 3);
    assert_eq!(lint[1].range.start.character, 0);
    // /x/b.sv has only a lint finding; it must still get a map entry.
    let b_diags = map.get("/x/b.sv").expect("diags for b.sv");
    assert_eq!(b_diags.len(), 1);
    assert_eq!(b_diags[0].source.as_deref(), Some("llg-lint"));
    assert_eq!(b_diags[0].severity, Some(DiagnosticSeverity::INFORMATION));
    assert_eq!(b_diags[0].range.start, Position::new(0, 0)); // unknown line
}
