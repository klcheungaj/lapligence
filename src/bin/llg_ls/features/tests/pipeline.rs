//! Pipeline.

use super::*;

/// Exercises the full compile+model+tokens pipeline against a checked-in
/// SystemVerilog file.  Skips gracefully when the file is missing.
#[test]
fn analyze_full_pipeline_on_params() {
    let _guards = analysis_guards();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elaboration/params.sv");
    if !path.exists() {
        return;
    }
    let path_str = path.to_string_lossy().into_owned();
    let opts = CompileOpts {
        files: vec![path_str.clone()],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected diagnostics: {:?}",
        a.diagnostics
    );
    assert!(!a.model.top_instances.is_empty(), "expected top instances");
    assert!(
        a.tokens
            .iter()
            .any(|ft| ft.path == path_str || ft.path.ends_with("params.sv")),
        "expected tokens for params.sv"
    );
    let file = a
        .model
        .modules
        .iter()
        .find(|m| m.file.as_deref().is_some_and(|f| f.ends_with("params.sv")))
        .and_then(|m| m.file.clone())
        .unwrap_or_else(|| path_str.clone());
    let syms = document_symbols(&a, &file);
    assert!(
        syms.iter()
            .any(|s| s.name == "param_top" || s.name == "param_child"),
        "syms: {syms:?}"
    );
}

/// Full compile of a design whose module declares a function and a task,
/// instantiated once: the model must carry per-instance clones and the LSP
/// features must surface them (signature hover, document symbols, completion).
#[test]
fn analyze_full_pipeline_extracts_funcs() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_funcs_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("funcs_fixture.sv");
    std::fs::write(
        &sv,
        "module calc;\n\
         function automatic int add(input int a, input int b);\n\
           add = a + b;\n\
         endfunction\n\
         task automatic run(input int n);\n\
           $display(\"%d\", n);\n\
         endtask\n\
         endmodule\n\
         module top;\n\
           calc c0();\n\
         endmodule\n",
    )
    .expect("write design");
    let path = sv.to_string_lossy().into_owned();
    let opts = CompileOpts {
        files: vec![path.clone()],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected diagnostics: {:?}",
        a.diagnostics
    );

    let c0 = a
        .model
        .instance("top.c0")
        .or_else(|| a.model.top_instances.iter().find(|i| i.name == "c0"))
        .expect("c0 instance");
    let add = c0.func("add").expect("add func");
    assert!(!add.is_task);
    assert!(add.automatic);
    let add_ret = add.ret.as_ref().expect("add return type");
    assert_eq!(add_ret.type_name, None);
    assert_eq!(add_ret.kind, "int");
    assert_eq!(add_ret.width, Some(32));
    assert!(add_ret.signed);
    assert_eq!(add.args.len(), 2);
    assert_eq!(add.args[0].direction, Direction::Input);
    assert_eq!(add.args[0].name, "a");
    let run = c0.func("run").expect("run task");
    assert!(run.is_task);
    assert_eq!(run.args.len(), 1);

    // Index decl carries the signature; hover at that position shows it
    // plus the instance scope and storage class.
    let decl = a
        .index
        .decls
        .iter()
        .find(|d| d.name == "add" && d.kind == SymKind::Function)
        .expect("add decl");
    assert_eq!(
        decl.detail.as_deref(),
        Some("function int add(input int a, input int b)")
    );
    let hover = hover_at(&a, &path, decl.line, decl.col).expect("hover on add");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(
        value.contains("function int add(input int a, input int b)"),
        "value: {value}"
    );
    assert!(value.contains("top.c0"), "scope missing: {value}");
    assert!(
        value.contains("automatic"),
        "storage class missing: {value}"
    );

    // Document symbols and completion include the function.
    let syms = document_symbols(&a, &path);
    assert!(
        syms.iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::FUNCTION),
        "syms: {syms:?}"
    );
    let items = completion_at(&a, &path, 0, 0, "");
    assert!(
        items
            .iter()
            .any(|i| { i.label == "add" && i.kind == Some(CompletionItemKind::FUNCTION) }),
        "items: {items:?}"
    );
}

/// Full compile of a tiny design with a known lint finding: `unused_sig`
/// is never read or written, so the `unused-signal` rule (Warning) fires
/// and must surface as a `llg-lint` diagnostic.
#[test]
fn analyze_full_pipeline_reports_unused_signal_lint() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("lint_fixture.sv");
    std::fs::write(
        &sv,
        "module t;\n  logic used;\n  logic unused_sig;\n  assign used = 1'b0;\nendmodule\n",
    )
    .expect("write design");
    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected compile diagnostics: {:?}",
        a.diagnostics
    );
    let map = lsp_diagnostics(&a);
    let diags = map
        .iter()
        .find(|(f, _)| f.ends_with("lint_fixture.sv"))
        .map(|(_, v)| v)
        .expect("diagnostics for the fixture file");
    let lint: Vec<&LspDiagnostic> = diags
        .iter()
        .filter(|d| d.source.as_deref() == Some("llg-lint"))
        .collect();
    assert!(
        lint.iter()
            .any(|d| { d.message.contains("unused_sig") && d.message.contains("never used") }),
        "unused-signal finding missing: {lint:?}"
    );
    assert!(
        lint.iter().any(|d| {
            d.code == Some(NumberOrString::String("unused-signal".to_owned()))
                && d.severity == Some(DiagnosticSeverity::WARNING)
        }),
        "unused-signal code/severity missing: {lint:?}"
    );
}

/// Full pipeline over a syntax-broken project: one clean unit plus one file
/// with a real syntax error. The outcome stays Parse with unchanged diagnostics,
/// while Slang's lexical snapshot retains declaration-level navigation data.
#[test]
fn analyze_syntax_broken_project_serves_parse_tree_declarations() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_pfb_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let clean_sv = dir.join("fb_clean.sv");
    std::fs::write(
        &clean_sv,
        "module fb_clean(input logic clk);\n  logic value;\nendmodule\n",
    )
    .expect("write clean design");
    let broken_sv = dir.join("fb_broken.sv");
    std::fs::write(&broken_sv, "module fb_broken;\n  assign broken_sig = ;\n")
        .expect("write unterminated design");
    let opts = CompileOpts {
        files: vec![
            clean_sv.to_string_lossy().into_owned(),
            broken_sv.to_string_lossy().into_owned(),
        ],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);

    // Diagnostics are unchanged: the syntax error is reported and the
    // outcome stays Parse.
    assert!(
        a.diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Syntax)),
        "expected a syntax diagnostic: {:?}",
        a.diagnostics
    );
    assert!(!a
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Fatal)));
    assert_eq!(a.outcome, AnalysisOutcome::Parse);
    assert!(a.has_feature_data(), "fallback must carry feature data");

    // The recovery snapshot retains module declarations without inventing
    // elaborated instances for the broken compilation.
    assert!(
        !a.model.modules.is_empty(),
        "modules: {:?}",
        a.model.modules
    );
    assert!(a.model.modules.iter().any(|m| m.name == "fb_clean"));
    assert!(a.model.modules.iter().any(|m| m.name == "fb_broken"));
    assert!(
        a.model
            .modules
            .iter()
            .all(|m| m.file.as_deref().is_some_and(|f| !f.is_empty()) && m.line > 0),
        "module decl positions: {:?}",
        a.model.modules
    );

    // Tokens come from the parse tree (keywords at minimum).
    assert!(
        a.tokens.iter().any(|ft| {
            ft.path.ends_with("fb_clean.sv")
                && ft.nodes.iter().any(|n| n.name.as_deref() == Some("module"))
        }),
        "parse tokens missing: {:?}",
        a.tokens
    );

    // The declared module is navigable: index decl + workspace symbol +
    // hover on the declaration name.
    let clean_path = clean_sv.to_string_lossy().into_owned();
    let decl = a
        .index
        .decls
        .iter()
        .find(|d| d.kind == SymKind::Module && d.name == "fb_clean")
        .expect("fb_clean decl in index");
    assert_eq!(decl.file, clean_path);
    let syms = workspace_symbols(&a, "fb_clean");
    assert!(
        syms.iter().any(|s| s.name == "fb_clean"),
        "workspace symbols: {syms:?}"
    );
    // The broken file's declaration serves too (parse-error recovery
    // keeps its module header).
    assert!(
        workspace_symbols(&a, "fb_broken")
            .iter()
            .any(|s| s.name == "fb_broken"),
        "workspace symbols for the broken unit: {syms:?}"
    );
    let hover = hover_at(&a, &clean_path, decl.line, decl.col).expect("hover on fb_clean");
    match hover.contents {
        HoverContents::Markup(m) => {
            assert!(m.value.contains("module fb_clean"), "value: {}", m.value)
        }
        _ => panic!("expected markup hover"),
    }
    // Document symbols expose the module for its file even though no
    // elaborated instance data exists.
    let doc = document_symbols(&a, &clean_path);
    assert!(
        doc.iter()
            .any(|s| s.name == "fb_clean" && s.kind == SymbolKind::MODULE),
        "document symbols: {doc:?}"
    );

    cleanup_process_shadow();
}

/// Build an `LSPAny::Object` from string-keyed entries.  The settings
/// payloads in these tests are hand-built because the bin has no direct
/// `serde_json` dependency (only the `lsp_types` aliases are nameable).
pub(super) fn settings_obj(entries: Vec<(&str, LSPAny)>) -> LSPAny {
    let mut map = LSPObject::new();
    for (k, v) in entries {
        map.insert(k.to_owned(), v);
    }
    LSPAny::Object(map)
}

/// `analyze_with_config` honors a per-rule `enabled: false`: the fixture
/// that produces an `unused-signal` finding under the default config is
/// quiet when the rule is disabled.
#[test]
fn analyze_with_config_disables_rule() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_cfg_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("lint_cfg_fixture.sv");
    std::fs::write(
        &sv,
        "module t;\n  logic used;\n  logic unused_sig;\n  assign used = 1'b0;\nendmodule\n",
    )
    .expect("write design");
    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        top: None,
        ..Default::default()
    };

    let default = analyze(&opts);
    assert!(
        default.lint.iter().any(|d| d.rule == "unused-signal"),
        "expected unused-signal finding under default config: {:?}",
        default.lint
    );

    let mut cfg = LintConfig::default();
    cfg.set(
        "unused-signal",
        RuleConfig {
            enabled: false,
            severity: None,
        },
    );
    let disabled = analyze_with_config(&opts, &cfg);
    assert!(
        !disabled.lint.iter().any(|d| d.rule == "unused-signal"),
        "unused-signal finding present despite being disabled: {:?}",
        disabled.lint
    );
}

/// `analyze_with_config` applies a `severity` override: the
/// `width-mismatch` extension finding (Info by default) is reported as
/// Error when configured.
#[test]
fn analyze_with_config_severity_override() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_sev_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("lint_sev_fixture.sv");
    std::fs::write(
        &sv,
        "module wm;\n  logic [3:0] x;\n  logic [7:0] y;\n  assign y = x;\nendmodule\n",
    )
    .expect("write design");
    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        top: None,
        ..Default::default()
    };

    let default = analyze(&opts);
    let width = default.lint.iter().find(|d| d.rule == "width-mismatch");
    assert!(
        width.is_some(),
        "expected width-mismatch finding under default config: {:?}",
        default.lint
    );
    assert_eq!(width.expect("width finding").severity, LintSeverity::Info);

    let mut cfg = LintConfig::default();
    cfg.set(
        "width-mismatch",
        RuleConfig {
            enabled: true,
            severity: Some(LintSeverity::Error),
        },
    );
    let overridden = analyze_with_config(&opts, &cfg);
    let width = overridden.lint.iter().find(|d| d.rule == "width-mismatch");
    assert!(
        width.is_some(),
        "expected width-mismatch finding under configured run: {:?}",
        overridden.lint
    );
    assert_eq!(width.expect("width finding").severity, LintSeverity::Error);
}

/// Slang analysis neither changes the caller's working directory nor creates
/// frontend artifacts alongside the source file.
#[test]
fn analyze_leaves_source_tree_and_cwd_unchanged() {
    let _guards = analysis_guards();
    let fixture = std::env::temp_dir().join(format!("llg_scratch_probe_{}", std::process::id()));
    let rtl = fixture.join("rtl");
    std::fs::create_dir_all(&rtl).expect("create fixture tree");
    let sv = rtl.join("top.sv");
    std::fs::write(&sv, "module top; endmodule\n").expect("write design");

    fn listing(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(current) = pending.pop() {
            for entry in std::fs::read_dir(&current).expect("read dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    pending.push(path.clone());
                }
                out.push(path);
            }
        }
        out.sort();
        out
    }
    let before = listing(&fixture);

    // Run the analysis with the CWD pointing INSIDE the fixture tree.
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: fixture.clone(),
        orig: orig_cwd.clone(),
    };
    std::env::set_current_dir(&rtl).expect("chdir into fixture tree");

    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        top: None,
        ..Default::default()
    };
    let analysis = analyze_with_config(&opts, &LintConfig::default());
    assert!(
        analysis.is_valid(),
        "analysis failed: {:?}",
        analysis.diagnostics
    );

    assert_eq!(listing(&fixture), before, "fixture tree gained entries");
    assert_eq!(
        std::env::current_dir().expect("cwd after analysis"),
        std::fs::canonicalize(&rtl).unwrap_or(rtl.clone())
    );

    let _ = std::fs::remove_dir_all(fixture);
}
