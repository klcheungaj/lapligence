//! Recovery.

use super::*;

#[test]
fn resource_limit_failures_log_errors_with_actionable_guidance() {
    const CHILD: &str = "LLG_RESOURCE_LIMIT_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        {
            let _guards = analysis_guards();
            for source_limit in [false, true] {
                let limits = if source_limit {
                    llg::ffi::slang::Limits {
                        max_source_bytes: 1,
                        ..Default::default()
                    }
                } else {
                    llg::ffi::slang::Limits {
                        max_output_bytes: 1,
                        ..Default::default()
                    }
                };
                let analysis = analyze(&CompileOpts {
                    library_units: true,
                    sources: vec![compile::OwnedSource::compilation_unit(
                        "/virtual/top.sv",
                        "module top; endmodule",
                    )],
                    limits,
                    ..Default::default()
                });
                assert!(!analysis.has_feature_data());
                assert!(
                    analysis.diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains(crate::config::FRONTEND_LIMIT_GUIDANCE)
                    }),
                    "{:?}",
                    analysis.diagnostics
                );
            }
        }
        export_limit_recovers_declaration_level_workspace_features();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "features::tests::recovery::resource_limit_failures_log_errors_with_actionable_guidance",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("LLG_LOG", "error")
        .env_remove("LLG_LOG_FILE")
        .output()
        .expect("run isolated limit logging regression");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{stderr}\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.matches("[ERROR] event=slang.resource_limit").count() >= 3,
        "{stderr}"
    );
    for expected in [
        "max_source_bytes=1",
        "max_output_bytes=1",
        "max_output_bytes=262144",
        crate::config::FRONTEND_LIMIT_GUIDANCE,
    ] {
        assert!(stderr.contains(expected), "missing {expected}: {stderr}");
    }
}

#[test]
fn export_limit_recovers_declaration_level_workspace_features() {
    let _guards = analysis_guards();
    let mut top = String::from("module top; leaf u_leaf(); integer value; initial begin\n");
    for _ in 0..600 {
        top.push_str("value = value + 1;\n");
    }
    top.push_str("end endmodule\n");

    let limits = llg::ffi::slang::Limits {
        max_output_bytes: 256 * 1024,
        ..llg::ffi::slang::Limits::default()
    };
    let analysis = analyze(&CompileOpts {
        sources: vec![
            llg::core::compile::OwnedSource::compilation_unit(
                "/virtual/leaf.sv",
                "module leaf; endmodule\n",
            ),
            llg::core::compile::OwnedSource::compilation_unit("/virtual/top.sv", &top),
        ],
        limits,
        ..CompileOpts::default()
    });

    assert_eq!(analysis.outcome, AnalysisOutcome::Compile);
    assert!(
        !analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Fatal),
        "diagnostics: {:?}",
        analysis.diagnostics
    );
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("serving bounded declaration-level navigation")),
        "diagnostics: {:?}",
        analysis.diagnostics
    );
    assert!(workspace_symbols(&analysis, "leaf")
        .iter()
        .any(|symbol| symbol.name == "leaf"));
    assert!(!semantic_tokens_for(&analysis, "/virtual/top.sv")
        .data
        .is_empty());
    let definition = definition_at(&analysis, "/virtual/top.sv", 0, 12)
        .expect("definition of the leaf module reference");
    assert_eq!(definition.uri.path(), "/virtual/leaf.sv");

    let explorer = crate::module_explorer::snapshot_analysis("workspace", &analysis, |path| {
        Some(path.to_owned())
    });
    assert!(
        explorer.modules.iter().any(|module| module.name == "leaf"),
        "snapshot: {explorer:#?}"
    );
    assert!(!explorer.roots.is_empty(), "snapshot: {explorer:#?}");
}

#[test]
fn single_unit_native_limit_recovers_navigation() {
    let _guards = analysis_guards();
    let source = format!(
        "module top; int data; initial begin {} end endmodule",
        "data = data + 1;".repeat(100)
    );
    let analysis = analyze(&CompileOpts {
        sources: vec![compile::OwnedSource::compilation_unit(
            "/virtual/top.sv",
            &source,
        )],
        limits: llg::ffi::slang::Limits {
            max_semantic_nodes: 128,
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(analysis.has_feature_data(), "{:?}", analysis.diagnostics);
    assert!(analysis.diagnostics.iter().any(|diag| diag
        .message
        .contains("serving bounded declaration-level navigation")));
    assert!(!workspace_symbols(&analysis, "data").is_empty());
}

#[test]
fn fatal_preflight_contains_only_the_supplied_fatal_diagnostic() {
    let analysis = Analysis::fatal_preflight("workspace is not ready");

    assert_eq!(analysis.outcome, AnalysisOutcome::Fatal);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].severity, Severity::Fatal);
    assert_eq!(analysis.diagnostics[0].message, "workspace is not ready");
    assert!(analysis.diagnostics[0].file.is_none());
    assert!(analysis.model.modules.is_empty());
    assert!(analysis.tokens.is_empty());
    assert!(analysis.lint.is_empty());
    assert!(analysis.index.decls.is_empty());
    assert!(analysis.index.refs.is_empty());
}

#[test]
fn db_build_failure_is_reported_at_unknown_source_position_and_invalidates_analysis() {
    let error = "node walk failed";
    let diagnostic = db_build_diagnostic(error);
    let analysis = Analysis::new_with_outcome(
        AnalysisOutcome::Compile,
        vec![diagnostic],
        empty_design(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        ConnectionInputs::default(),
    );

    assert_eq!(analysis.outcome, AnalysisOutcome::Compile);
    assert!(!analysis.is_valid());
    assert_eq!(analysis.diagnostics[0].severity, Severity::Error);
    assert!(analysis.diagnostics[0].file.is_none());
    assert_eq!(
        (analysis.diagnostics[0].line, analysis.diagnostics[0].col),
        (0, 0)
    );
    assert!(analysis.diagnostics[0].message.contains(error));
    assert!(analysis.model.modules.is_empty());
    assert!(analysis.tokens.is_empty());
    assert!(analysis.lint.is_empty());
}

/// A minimal model + token set that [`Analysis::has_feature_data`]
/// recognizes as servable.
fn served_feature_parts() -> (DesignModel, Vec<FileTokens>) {
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: Vec::new(),
        modules: vec![ModuleDef {
            name: "m".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 3,
            end_col: 12,
        }],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let tokens = vec![FileTokens {
        path: "/x/top.sv".to_owned(),
        nodes: vec![TokenInfo {
            line: 1,
            col: 8,
            end_line: 1,
            end_col: 9,
            kind: tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
            name: Some("m".to_owned()),
            file: "/x/top.sv".to_owned(),
        }],
    }];
    (model, tokens)
}

/// Feature-serving gate truth table: Parse/Compile outcomes serve
/// best-effort like a valid one whenever any servable data exists;
/// Fatal never serves; and an analysis without any data (empty project)
/// never serves either.
#[test]
fn has_feature_data_truth_table() {
    for outcome in [
        AnalysisOutcome::Valid,
        AnalysisOutcome::Compile,
        AnalysisOutcome::Parse,
    ] {
        let (model, tokens) = served_feature_parts();
        let analysis = Analysis::new_with_outcome(
            outcome,
            Vec::new(),
            model,
            tokens,
            Vec::new(),
            HashMap::new(),
            ConnectionInputs::default(),
        );
        assert!(
            analysis.has_feature_data(),
            "{outcome:?} with feature data must serve"
        );
    }

    // Fatal stays feature-less even if data were present.
    let (model, tokens) = served_feature_parts();
    let fatal = Analysis::new_with_outcome(
        AnalysisOutcome::Fatal,
        Vec::new(),
        model,
        tokens,
        Vec::new(),
        HashMap::new(),
        ConnectionInputs::default(),
    );
    assert!(!fatal.has_feature_data());

    // No data at all (db built but produced nothing): never serves.
    for outcome in [
        AnalysisOutcome::Valid,
        AnalysisOutcome::Compile,
        AnalysisOutcome::Parse,
    ] {
        let analysis = Analysis::new_with_outcome(
            outcome,
            Vec::new(),
            empty_design(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            ConnectionInputs::default(),
        );
        assert!(
            !analysis.has_feature_data(),
            "{outcome:?} without any servable data must not serve"
        );
    }

    // The preflight constructor produces the documented shape: Fatal,
    // no feature data.
    assert!(!Analysis::fatal_preflight("aborted").has_feature_data());
}
