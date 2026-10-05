//! Diagnostics.

use super::*;

#[test]
fn slang_diagnostics_preserve_location_and_message() {
    let raw = Diag {
        severity: Severity::Syntax,
        logical: None,
        file: Some(hp("/x/debug_TEMPLATE.v").to_owned()),
        line: 2,
        col: 22,
        message: "expected a statement".to_owned(),
    };
    let a = Analysis::new(vec![raw.clone()], empty_design(), Vec::new(), Vec::new());
    let map = lsp_diagnostics(&a);
    let diagnostics = &map[hp("/x/debug_TEMPLATE.v")];
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
                logical: None,
                file: Some(hp("/x/a.sv").to_owned()),
                line: 3,
                col: 5,
                message: "bad".to_owned(),
            },
            Diag {
                severity: Severity::Warning,
                logical: None,
                file: Some(hp("/x/a.sv").to_owned()),
                line: 4,
                col: 1,
                message: "warn".to_owned(),
            },
            Diag {
                severity: Severity::Note,
                logical: None,
                file: Some(hp("/x/a.sv").to_owned()),
                line: 0,
                col: 0,
                message: "note".to_owned(),
            },
            Diag {
                severity: Severity::Info,
                logical: None,
                file: Some(hp("/x/a.sv").to_owned()),
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
    let diags = map.get(hp("/x/a.sv")).expect("diags for a.sv");
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

    let map = lsp_diagnostics_with_fallback(&a, Some(Path::new(hp("/x/top.sv"))));
    let diagnostics = map.get(hp("/x/top.sv")).expect("fallback-file diagnostics");
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
            logical: None,
            file: Some(hp("/x/a.sv").to_owned()),
            line: 1,
            col: 1,
            message: "compile error".to_owned(),
        }],
        empty_design(),
        Vec::new(),
        vec![
            LintDiag {
                rule: "unused-signal".to_owned(),
                logical: None,
                severity: LintSeverity::Error,
                file: Some(hp("/x/a.sv").to_owned()),
                line: 3,
                col: 5,
                message: "signal `x` in `m` is never used".to_owned(),
            },
            LintDiag {
                rule: "width-mismatch".to_owned(),
                logical: None,
                severity: LintSeverity::Warning,
                file: Some(hp("/x/a.sv").to_owned()),
                line: 4,
                col: 1,
                message: "truncation".to_owned(),
            },
            LintDiag {
                rule: "multi-driver".to_owned(),
                logical: None,
                severity: LintSeverity::Info,
                file: Some(hp("/x/b.sv").to_owned()),
                line: 0,
                col: 0,
                message: "multiple drivers".to_owned(),
            },
        ],
    );
    let map = lsp_diagnostics(&a);
    let a_diags = map.get(hp("/x/a.sv")).expect("diags for a.sv");
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
    let b_diags = map.get(hp("/x/b.sv")).expect("diags for b.sv");
    assert_eq!(b_diags.len(), 1);
    assert_eq!(b_diags[0].source.as_deref(), Some("llg-lint"));
    assert_eq!(b_diags[0].severity, Some(DiagnosticSeverity::INFORMATION));
    assert_eq!(b_diags[0].range.start, Position::new(0, 0)); // unknown line
}

/// RTL-106: frontend diagnostics keep their physical range and add the
/// `` `line``-mapped origin as related information, for Slang diagnostics
/// (rich projection) and llg-owned edition diagnostics (compact projection).
#[test]
fn line_mapped_frontend_diagnostics_add_their_origin_as_related_information() {
    let _guards = analysis_guards();
    let dir = resolved_temp_dir(&format!("llg_ls_line_origin_{}", std::process::id()));
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("mapped.sv");
    // The directive on line 2 makes physical line 3 orig.sv:40, so line 5 is 42.
    std::fs::write(
        &sv,
        "module top;\n`line 40 \"orig.sv\" 0\n  logic a;\n  initial begin\n    a = undefined_name;\n  end\nendmodule\n",
    )
    .expect("write design");
    let v = dir.join("mapped.v");
    // The directive on line 3 makes physical line 4, the later form, gen.v:100.
    std::fs::write(
        &v,
        "module top;\n  reg r;\n`line 100 \"gen.v\" 0\n  assign r = 1'b1;\nendmodule\n",
    )
    .expect("write design");
    for (file, edition, expected_line, origin) in [
        (
            &sv,
            compile::LanguageEdition::SystemVerilog2009,
            4,
            "orig.sv:42",
        ),
        (&v, compile::LanguageEdition::Verilog2001, 3, "gen.v:100"),
    ] {
        let path = file.to_string_lossy().into_owned();
        let a = analyze(&CompileOpts {
            files: vec![path.clone()],
            edition,
            ..Default::default()
        });
        let map = lsp_diagnostics(&a);
        let diagnostic = map[&path]
            .iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .unwrap_or_else(|| panic!("{path}: no error in {:?}", map[&path]));
        assert_eq!(diagnostic.range.start.line, expected_line, "{diagnostic:?}");
        let related = diagnostic
            .related_information
            .as_ref()
            .unwrap_or_else(|| panic!("{path}: no related information: {diagnostic:?}"));
        assert_eq!(related[0].message, format!("`line origin: {origin}"));
        assert_eq!(related[0].location.range, diagnostic.range);
        assert_eq!(
            related[0].location.uri,
            Url::from_file_path(file).expect("file URI")
        );
    }
}

#[test]
fn lint_diagnostics_add_their_line_mapped_origin() {
    let lint = LintDiag {
        rule: "incomplete-case".to_owned(),
        logical: Some(compile::LogicalLine {
            file: "orig.sv".to_owned(),
            line: 12,
        }),
        severity: LintSeverity::Warning,
        file: Some(hp("/x/a.sv").to_owned()),
        line: 3,
        col: 5,
        message: "case without default".to_owned(),
    };
    let a = Analysis::new(Vec::new(), empty_design(), Vec::new(), vec![lint]);
    let map = lsp_diagnostics(&a);
    let diagnostic = &map[hp("/x/a.sv")][0];
    assert_eq!(diagnostic.range.start, Position::new(2, 4));
    let related = diagnostic.related_information.as_ref().expect("origin");
    assert_eq!(related[0].message, "`line origin: orig.sv:12");
}
