//! Cross-module regression tests for the feature facade.

use super::*;
use llg::core::elab::{Val, Value};
use llg::core::model::{GenScopeModel, TypeInfo};
use llg::core::tokens;

/// Serializes tests that temporarily change the process working directory.
static ANALYSIS_CWD_LOCK: Mutex<()> = Mutex::new(());

/// Guards for tests that run real analyses.  Analyses CREATE the process
/// shadow base and park the process CWD inside it, so they must be serialized
/// against the shadow staging/cleanup tests. Lock order is fixed: the analysis
/// CWD lock first, then TEST_PROCESS_SHADOW_LOCK (never reversed).
fn analysis_guards() -> (
    std::sync::MutexGuard<'static, ()>,
    std::sync::MutexGuard<'static, ()>,
) {
    let cwd = ANALYSIS_CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let shadow = crate::features::TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    (cwd, shadow)
}

#[test]
fn module_graph_from_slang_deduplicates_definitions_and_uses_utf16_columns() {
    let _guards = analysis_guards();
    let name = "/virtual/module_graph.sv";
    let source = "/*😀*/ module top; endmodule\n";
    let opts = CompileOpts::default();
    let mut out = llg::core::compile::compile_sources(
        &[llg::core::compile::OwnedSource::compilation_unit(
            name, source,
        )],
        &opts,
    )
    .expect("compile Slang graph fixture");
    assert!(!out.snapshot.has_errors(), "{:?}", out.diagnostics);

    let database = llg::core::db::Db::from_slang(&out.snapshot).expect("import graph fixture");
    let definition = out
        .snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.kind == llg::ffi::slang::SemanticKind::Definition && node.name == "top")
        .cloned()
        .expect("top definition");
    out.snapshot.semantic_nodes.push(definition);

    let graph = module_graph_from_slang(&out.snapshot, &[(name, source)], Some(&database));
    assert_eq!(graph.definitions.len(), 1);
    let top = &graph.definitions[0];
    assert_eq!(top.name, "top");
    assert_eq!(top.file.as_deref(), Some(name));
    assert_eq!((top.line, top.col), (1, 15));

    assert_eq!((top.end_line, top.end_col), (1, 18));
}

#[test]
fn slang_module_explorer_keeps_repeated_nested_instances_and_source_roots() {
    let _guards = analysis_guards();
    let sources = [
        llg::core::compile::OwnedSource::compilation_unit(
            "/virtual/tb.sv",
            "module tb; top u_top(); endmodule\n",
        ),
        llg::core::compile::OwnedSource::compilation_unit(
            "/virtual/top.sv",
            "module top; child #(.WIDTH(8)) u_child(); child #(.WIDTH(16)) u_wide(); endmodule\n",
        ),
        llg::core::compile::OwnedSource::compilation_unit(
            "/virtual/child.sv",
            "module child #(parameter int WIDTH = 1); logic [WIDTH-1:0] value; endmodule\n",
        ),
    ];

    for configured_top in [None, Some("top".to_owned())] {
        let analysis = analyze(&CompileOpts {
            sources: sources.to_vec(),
            top: configured_top,
            ..CompileOpts::default()
        });
        assert_eq!(
            analysis.outcome,
            AnalysisOutcome::Valid,
            "{:?}",
            analysis.diagnostics
        );

        let snapshot = crate::module_explorer::snapshot_analysis("workspace", &analysis, |path| {
            Some(path.to_owned())
        });
        let tb = snapshot
            .roots
            .iter()
            .find(|root| root.module_type == "tb")
            .unwrap_or_else(|| panic!("missing source root tb: {snapshot:#?}"));
        let top = tb
            .children
            .iter()
            .find(|child| child.instance_name == "u_top")
            .unwrap_or_else(|| panic!("missing nested top: {snapshot:#?}"));
        let children = top
            .children
            .iter()
            .filter(|child| child.module_type == "child")
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2, "{snapshot:#?}");
        assert!(
            children
                .iter()
                .all(|child| child.content_source.as_deref() == Some("elaborated")),
            "{snapshot:#?}"
        );
        assert_eq!(
            children
                .iter()
                .map(|child| child.params[0].value.as_deref())
                .collect::<Vec<_>>(),
            [Some("32'sd8"), Some("32'sd16")],
            "{snapshot:#?}"
        );
    }
}

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
            "features::tests::resource_limit_failures_log_errors_with_actionable_guidance",
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
fn navigation_capture_keeps_repeated_instance_labels_and_scoped_references() {
    let _guards = analysis_guards();
    let leaf = "module leaf(input logic clk);\nfunction int f(input int value); f = value; endfunction\nendmodule\n";
    let top = "module top(input logic clk);\nleaf first(.clk(clk));\nleaf second(.clk(clk));\nendmodule\n";
    let analysis = analyze(&CompileOpts {
        library_units: true,
        sources: vec![
            compile::OwnedSource::compilation_unit("/virtual/leaf.sv", leaf),
            compile::OwnedSource::compilation_unit("/virtual/top.sv", top),
        ],
        ..Default::default()
    });
    assert!(analysis.has_feature_data(), "{:?}", analysis.diagnostics);
    for line in [1, 2] {
        let text = top.lines().nth(line).unwrap();
        let label = text.find(".clk").unwrap() as u32 + 1;
        let actual = text.find("(clk)").unwrap() as u32 + 1;
        let target = definition_at(&analysis, "/virtual/top.sv", line as u32, label)
            .expect("port label definition");
        assert_eq!(target.uri.path(), "/virtual/leaf.sv");
        let target = definition_at(&analysis, "/virtual/top.sv", line as u32, actual)
            .expect("actual definition");
        assert_eq!(target.uri.path(), "/virtual/top.sv");
        assert_eq!(target.range.start.line, 0);
    }
    let reference = leaf.lines().nth(1).unwrap().find("= value").unwrap() as u32 + 2;
    let target = definition_at(&analysis, "/virtual/leaf.sv", 1, reference)
        .expect("formal argument definition");
    assert_eq!(target.range.start.line, 1);
    assert_eq!(target.range.start.character, 25);
}

#[test]
fn navigation_capture_keeps_duplicate_module_contents_in_their_own_files() {
    let _guards = analysis_guards();
    let analysis = analyze(&CompileOpts {
        library_units: true,
        sources: vec![
            compile::OwnedSource::compilation_unit(
                "/virtual/one.sv",
                "module debug_top(input logic left); endmodule",
            ),
            compile::OwnedSource::compilation_unit(
                "/virtual/two.sv",
                "module debug_top(input logic right); endmodule",
            ),
        ],
        ..Default::default()
    });
    assert_eq!(analysis.module_graph.definitions.len(), 2);
    for (file, port) in [("/virtual/one.sv", "left"), ("/virtual/two.sv", "right")] {
        let definition = analysis
            .module_graph
            .definitions
            .iter()
            .find(|definition| definition.file.as_deref() == Some(file))
            .unwrap();
        assert_eq!(
            definition
                .ports
                .iter()
                .map(|port| port.name.as_str())
                .collect::<Vec<_>>(),
            [port]
        );
    }
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

/// Restores the process CWD and removes the temp dir even when the body
/// panics, so a failing test cannot strand other tests in a deleted CWD.
struct TempDirGuard {
    dir: std::path::PathBuf,
    orig: std::path::PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.orig);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Hand-built model + tokens for /x/top.sv:
/// - module `m` (line 1..3) with input port `clk` and parameter `W` = 32'sd8,
/// - instance `top.u0` of type `m`,
/// - package `p`,
/// - function `add` (line 4) and task `run` (line 5) on `u0`,
/// - tokens for `clk`, `u0`, `W`, `add`, and `run`.
///
/// Returned as parts so tests can assemble the analysis with or without
/// synthetic semantic bindings.
fn sample_parts() -> (DesignModel, Vec<FileTokens>) {
    let module = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 3,
        end_col: 12,
    };
    let port = PortModel {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        },
    };
    let param = ParamModel {
        name: "W".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, true))),
        ty: TypeInfo {
            kind: "int".to_owned(),
            width: None,
            signed: true,
            type_name: None,
        },
        local: false,
    };
    let int_ty = || TypeInfo {
        kind: "int".to_owned(),
        width: None,
        signed: true,
        type_name: None,
    };
    let add = FuncDef {
        name: "add".to_owned(),
        is_task: false,
        automatic: true,
        file: Some("/x/top.sv".to_owned()),
        line: 4,
        col: 8,
        ret: Some(int_ty()),
        args: vec![
            FuncArgDef {
                name: "a".to_owned(),
                direction: Direction::Input,
                ty: int_ty(),
                has_default: false,
                const_ref: false,
                ref_static: false,
            },
            FuncArgDef {
                name: "b".to_owned(),
                direction: Direction::Input,
                ty: int_ty(),
                has_default: true,
                const_ref: false,
                ref_static: false,
            },
        ],
        dpi_import: None,
        scope: "top.u0".to_owned(),
    };
    let run = FuncDef {
        name: "run".to_owned(),
        is_task: true,
        automatic: false,
        file: Some("/x/top.sv".to_owned()),
        line: 5,
        col: 8,
        ret: None,
        args: vec![FuncArgDef {
            name: "n".to_owned(),
            direction: Direction::Input,
            ty: int_ty(),
            has_default: false,
            const_ref: false,
            ref_static: false,
        }],
        dpi_import: None,
        scope: "top.u0".to_owned(),
    };
    let inst = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 20,
        ports: vec![port],
        signals: Vec::new(),
        params: vec![param],
        gen_scopes: Vec::new(),
        funcs: vec![add, run],
        children: Vec::new(),
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![inst],
        modules: vec![module],
        packages: vec![PackageDef {
            name: "p".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 5,
            col: 1,
            params: Vec::new(),
            enum_consts: Vec::new(),
        }],
        classes: Vec::new(),
    };
    let tokens = vec![FileTokens {
        path: "/x/top.sv".to_owned(),
        nodes: vec![
            TokenInfo {
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 9,
                kind: llg::core::tokens::TOKEN_SLANG_MODULE
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("m".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 1,
                col: 5,
                end_line: 1,
                end_col: 8,
                kind: llg::core::tokens::TOKEN_SLANG_PORT
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("clk".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 1,
                col: 20,
                end_line: 1,
                end_col: 22,
                kind: llg::core::tokens::TOKEN_SLANG_IDENTIFIER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("u0".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            // This fixture repeats a declaration site to ensure the index
            // deduplicates explicit declarations.
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 4,
                col: 8,
                end_line: 4,
                end_col: 11,
                kind: llg::core::tokens::TOKEN_SLANG_FUNCTION
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("add".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 5,
                col: 8,
                end_line: 5,
                end_col: 11,
                kind: llg::core::tokens::TOKEN_SLANG_TASK
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("run".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
        ],
    }];
    (model, tokens)
}

/// [`sample_parts`] assembled through [`Analysis::new`].
fn sample_analysis() -> Analysis {
    let (model, tokens) = sample_parts();
    Analysis::new(Vec::new(), model, tokens, Vec::new())
}

/// [`sample_parts`] assembled with synthetic semantic reference bindings.
fn sample_analysis_with_bindings(bindings: RefBindings) -> Analysis {
    let (model, tokens) = sample_parts();
    Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        tokens,
        Vec::new(),
        bindings,
        ConnectionInputs::default(),
    )
}

#[test]
fn navigation_ranges_use_utf16_after_supplementary_text() {
    let dir = std::env::temp_dir().join(format!(
        "llg-features-utf16-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temporary source directory");
    let _guard = TempDirGuard {
        dir: dir.clone(),
        orig: std::env::current_dir().expect("current directory"),
    };
    let path = dir.join("unicode.sv");
    let file = path.to_string_lossy().into_owned();
    let source = "module top;\nlogic /* 😀 */ data;\nassign data = data;\nendmodule\n";
    std::fs::write(&path, source).expect("write temporary source");

    let ty = TypeInfo {
        kind: "logic".to_owned(),
        width: Some(1),
        signed: false,
        type_name: None,
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some(file.clone()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: vec![SignalModel {
                name: "data".to_owned(),
                kind: "wire".to_owned(),
                ty: ty.clone(),
            }],
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        }],
        modules: vec![ModuleDef {
            name: "top".to_owned(),
            file: Some(file.clone()),
            line: 1,
            col: 1,
            end_line: 4,
            end_col: 1,
        }],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let token = |line: u32, col: u32, kind: i32| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + 4,
        kind,
        name: Some("data".to_owned()),
        file: file.clone(),
    };
    let tokens = vec![FileTokens {
        path: file.clone(),
        nodes: vec![
            TokenInfo {
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 12,
                kind: tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("top".to_owned()),
                file: file.clone(),
            },
            // Token coordinates already use one-based UTF-16; the emoji in
            // the comment consumes two code units before `data`.
            token(
                2,
                16,
                tokens::TOKEN_SLANG_NET + tokens::TOKEN_DECLARATION_OFFSET,
            ),
            token(
                2,
                16,
                tokens::TOKEN_SLANG_NET + tokens::TOKEN_DECLARATION_OFFSET,
            ),
            token(3, 8, tokens::TOKEN_SLANG_IDENTIFIER),
            token(3, 15, tokens::TOKEN_SLANG_IDENTIFIER),
        ],
    }];
    let analysis = Analysis::new(Vec::new(), model, tokens, Vec::new());

    let declaration = analysis
        .tokens
        .iter()
        .flat_map(|file_tokens| file_tokens.nodes.iter())
        .find(|node| node.name.as_deref() == Some("data") && node.line == 2)
        .expect("normalized declaration token");
    assert_eq!(declaration.col, 16);
    assert_eq!(declaration.end_col, 20);
    assert_eq!(
        FeatureSourceMap::new("😀data\nwire \\escaped😀name ;\n".to_owned()).normalize_1based(
            1,
            2,
            Some("data")
        ),
        (1, 3),
        "a supplementary character before a name consumes two UTF-16 units"
    );
    assert_eq!(lsp_name_len("escaped😀name"), 13);

    let entry = analysis
        .index
        .entry_at(&file, 1, 15)
        .expect("data declaration at UTF-16 column");
    assert_eq!(
        entry_location(entry).range,
        Range::new(Position::new(1, 15), Position::new(1, 19),)
    );
    assert!(token_at(&analysis, &file, 1, 15).is_some());

    let hover = hover_at(&analysis, &file, 1, 15).expect("hover on data");
    assert_eq!(
        hover.range,
        Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
    );
    let fallback_hover = hover_fallback(&analysis, &file, 1, 16).expect("fallback hover");
    assert_eq!(
        fallback_hover.range,
        Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
    );

    let definition = definition_at(&analysis, &file, 2, 7).expect("definition of data use");
    assert_eq!(definition.range.start, Position::new(1, 15));
    assert_eq!(definition.range.end, Position::new(1, 19));

    let references = references_at(&analysis, &file, 1, 15);
    let reference_starts: HashSet<_> = references
        .iter()
        .map(|location| (location.range.start.line, location.range.start.character))
        .collect();
    assert!(reference_starts.contains(&(1, 15)));
    assert!(reference_starts.contains(&(2, 7)));
    assert!(reference_starts.contains(&(2, 14)));

    let (rename_range, placeholder) =
        crate::rename::prepare_rename(&analysis, &file, 1, 15).expect("rename on data declaration");
    assert_eq!(placeholder, "data");
    assert_eq!(
        rename_range,
        Range::new(Position::new(1, 15), Position::new(1, 19))
    );

    let semantic = semantic_tokens_for(&analysis, &file);
    let mut line = 0u32;
    let mut col = 0u32;
    let mut found_data = false;
    for token in semantic.data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        if (line, col) == (1, 15) {
            assert_eq!(token.length, 4);
            found_data = true;
        }
    }
    assert!(
        found_data,
        "semantic token must use the UTF-16 declaration column"
    );
}

#[test]
fn hover_on_port_shows_direction_and_type() {
    let a = sample_analysis();
    let hover = hover_at(&a, "/x/top.sv", 0, 4).expect("hover on port");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("input logic"), "value: {value}");
    assert!(value.contains("clk"), "value: {value}");
}

#[test]
fn hover_on_param_shows_value() {
    let a = sample_analysis();
    let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on param");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("= 32'sd8"), "value: {value}");
    assert!(value.contains("parameter W"), "value: {value}");
}

#[test]
fn hover_on_missing_position_is_none() {
    let a = sample_analysis();
    assert!(hover_at(&a, "/x/top.sv", 9, 9).is_none());
}

/// An [`Analysis`] carrying a macro table over one synthetic source:
/// `` `define WIDTH 8 `` on line 1, a usage of it on line 4, an
/// undefined usage on line 5.
fn macro_analysis() -> Analysis {
    let text = concat!(
        "`define WIDTH 8\n",
        "module m;\n",
        "endmodule\n",
        "x = `WIDTH;\n",
        "y = `MISSING;\n",
    );
    let (model, tokens) = sample_parts();
    Analysis::new(Vec::new(), model, tokens, Vec::new()).with_macros(macros::build_table(
        &[],
        &[("/x/top.sv", text)],
        Some("llg.toml"),
    ))
}

#[test]
fn hover_on_macro_usage_shows_resolved_value() {
    let a = macro_analysis();
    // `` `WIDTH `` starts at 0-based col 4 on line 3; click mid-name.
    let hover = hover_at(&a, "/x/top.sv", 3, 6).expect("hover on macro usage");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value,
        "```systemverilog\nmacro WIDTH = 8\n\ndefined at /x/top.sv:1\n```"
    );
    let range = hover.range.expect("hover range");
    assert_eq!(range.start, Position::new(3, 4));
    assert_eq!(range.end, Position::new(3, 10));
}

#[test]
fn hover_on_undefined_macro_names_the_config() {
    let a = macro_analysis();
    let hover = hover_at(&a, "/x/top.sv", 4, 5).expect("hover on undefined macro");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(
        value.contains("`MISSING` is not defined under the current configuration."),
        "value: {value}"
    );
    assert!(
        value.contains("Checked the `[compile] defines` in llg.toml"),
        "the config note must name the checked source: {value}"
    );
}

#[test]
fn hover_on_define_site_shows_the_same_value() {
    let a = macro_analysis();
    // The NAME identifier of `` `define WIDTH 8 `` (0-based col 8).
    let hover = hover_at(&a, "/x/top.sv", 0, 9).expect("hover on define site");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("macro WIDTH = 8"), "value: {value}");
}

#[test]
fn hover_on_function_like_macro_renders_args() {
    let text = "`define MAX(a, b) ((a) > (b)) ? (a) : (b)\nx = `MAX(p, q);\n";
    let (model, tokens) = sample_parts();
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new()).with_macros(macros::build_table(
        &[],
        &[("/x/top.sv", text)],
        None,
    ));
    let hover = hover_at(&a, "/x/top.sv", 1, 5).expect("hover on function-like usage");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value,
        concat!(
            "```systemverilog\nmacro MAX(a, b) = ((a) > (b)) ? (a) : (b)\n",
            "\ndefined at /x/top.sv:1\n```"
        )
    );
}

#[test]
fn hover_on_function_name_shows_signature_and_scope() {
    let a = sample_analysis();
    // `function int add(...)` at 1-based (4, 8) → 0-based (3, 7).
    let hover = hover_at(&a, "/x/top.sv", 3, 7).expect("hover on function name");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(
        value.contains("function int add(input int a, input int b)"),
        "value: {value}"
    );
    assert!(value.contains("top.u0"), "scope missing: {value}");
    assert!(
        value.contains("automatic"),
        "storage class missing: {value}"
    );
}

#[test]
fn hover_on_task_name_shows_signature_and_static() {
    let a = sample_analysis();
    // `task run(...)` at 1-based (5, 8) → 0-based (4, 7).
    let hover = hover_at(&a, "/x/top.sv", 4, 7).expect("hover on task name");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("task run(input int n)"), "value: {value}");
    assert!(value.contains("static"), "storage class missing: {value}");
}

#[test]
fn hover_on_param_decl_shows_elaborated_value_line() {
    let a = sample_analysis();
    // `parameter W` at 1-based (2, 5) → 0-based (1, 4).
    let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on param decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value, "```systemverilog\nparameter W: int\nvalue = 32'sd8\n```",
        "the elaborated value must be its own short line: {value}"
    );
}

#[test]
fn hover_on_bound_param_reference_shows_elaborated_value() {
    // A ref position bound through `ref_bindings` to the W declaration
    // (0-based line0=1, col0=4): the value must come from the committed
    // model via the TARGET's coordinates — even when the reference sits
    // in a different file than its declaration.
    let mut bindings: RefBindings = HashMap::new();
    bindings.insert(
        ("/x/other.sv".to_owned(), 6, 2),
        DeclTarget {
            name: "W".to_owned(),
            kind: "parameter".to_owned(),
            file: "/x/top.sv".to_owned(),
            line0: 1,
            col0: 4,
            via_label: false,
            via_connection: false,
        },
    );
    let a = sample_analysis_with_bindings(bindings);
    let hover = hover_at(&a, "/x/other.sv", 6, 2).expect("hover on bound W ref");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(
        value.contains("parameter W") && value.contains("\nvalue = 32'sd8"),
        "cross-file ref-site hover must show the declaration's value: {value}"
    );
    assert!(
        !value.contains("= 32'sd8\nvalue"),
        "the value must not render twice: {value}"
    );
}

#[test]
fn unresolved_semantic_reference_does_not_fall_back_by_name() {
    let (model, mut token_files) = sample_parts();
    token_files[0].nodes.push(TokenInfo {
        line: 7,
        col: 3,
        end_line: 7,
        end_col: 4,
        kind: tokens::TOKEN_SLANG_PARAMETER,
        name: Some("W".to_owned()),
        file: "/x/top.sv".to_owned(),
    });
    let key = ("/x/top.sv".to_owned(), 6, 2);
    let analysis = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        token_files,
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            unresolved_bindings: [key].into_iter().collect(),
            ..ConnectionInputs::default()
        },
    );

    assert!(definition_at(&analysis, "/x/top.sv", 6, 2).is_none());
    assert!(hover_at(&analysis, "/x/top.sv", 6, 2).is_none());
    assert!(references_at(&analysis, "/x/top.sv", 6, 2).is_empty());
}

#[test]
fn hover_on_unresolved_param_omits_the_value_line() {
    let (mut model, tokens) = sample_parts();
    model.top_instances[0].params[0].value = None;
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
    let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on unresolved param");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value, "```systemverilog\nparameter W: int\n```",
        "an unresolved value must omit the line silently: {value}"
    );
}

#[test]
fn hover_on_non_param_symbols_has_no_value_line() {
    let a = sample_analysis();
    for (line, col, what) in [(0u32, 4u32, "port clk"), (0, 19, "instance u0")] {
        let hover =
            hover_at(&a, "/x/top.sv", line, col).unwrap_or_else(|| panic!("hover on {what}"));
        let value = match hover.contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover on {what}"),
        };
        assert!(
            !value.contains("value = "),
            "{what} hover must not gain a value line: {value}"
        );
    }
}

#[test]
fn param_elab_value_is_scoped_and_omits_divergent_overrides() {
    fn bits(v: u64) -> Val {
        Val::Bits(Value::from_u64(v, 32, true))
    }
    let (mut model, tokens) = sample_parts();
    // A second clone of module `m` overrides W differently; both carry a
    // generate scope with a branch-local parameter.
    let mut u1 = model.top_instances[0].clone();
    u1.name = "u1".to_owned();
    u1.full_name = "top.u1".to_owned();
    u1.params[0].value = Some(bits(4));
    u1.gen_scopes = vec![GenScopeModel {
        name: "g_wide".to_owned(),
        full_name: "top.u1.g_wide".to_owned(),
        params: vec![ParamModel {
            name: "BRANCH".to_owned(),
            value: Some(bits(4)),
            ty: TypeInfo {
                kind: "int".to_owned(),
                width: None,
                signed: true,
                type_name: None,
            },
            local: true,
        }],
        children: Vec::new(),
    }];
    model.top_instances[0].gen_scopes = u1.gen_scopes.clone();
    model.top_instances[0].gen_scopes[0].params[0].value = Some(bits(8));
    model.top_instances.push(u1);
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());

    // W diverges across clones → ambiguous at module granularity → None.
    assert_eq!(param_elab_value(&a, "/x/top.sv", 1, "W"), None);
    // BRANCH diverges across the clones' g_wide scopes → ambiguous too.
    assert_eq!(param_elab_value(&a, "/x/top.sv", 2, "BRANCH"), None);

    // Making both clones agree resolves the value even from a gen-scope.
    let (mut model, tokens) = sample_parts();
    let mut u1 = model.top_instances[0].clone();
    u1.name = "u1".to_owned();
    u1.full_name = "top.u1".to_owned();
    model.top_instances.push(u1);
    model.top_instances[0].gen_scopes = vec![GenScopeModel {
        name: "g_wide".to_owned(),
        full_name: "top.g_wide".to_owned(),
        params: vec![ParamModel {
            name: "BRANCH".to_owned(),
            value: Some(bits(8)),
            ty: TypeInfo {
                kind: "int".to_owned(),
                width: None,
                signed: true,
                type_name: None,
            },
            local: true,
        }],
        children: Vec::new(),
    }];
    model.top_instances[1].gen_scopes = model.top_instances[0].gen_scopes.clone();
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
    assert_eq!(
        param_elab_value(&a, "/x/top.sv", 1, "W"),
        Some(&bits(8)),
        "unanimous clones resolve to the shared value"
    );
    assert_eq!(
        param_elab_value(&a, "/x/top.sv", 2, "BRANCH"),
        Some(&bits(8)),
        "generate-scope parameters resolve through their gen scopes"
    );

    // The divergent case renders WITHOUT a value at all (no stale inline
    // number either — the display model clears it).
    let (mut model, tokens) = sample_parts();
    let mut u1 = model.top_instances[0].clone();
    u1.name = "u1".to_owned();
    u1.full_name = "top.u1".to_owned();
    u1.params[0].value = Some(bits(4));
    model.top_instances.push(u1);
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
    let hover = hover_at(&a, "/x/top.sv", 1, 4).expect("hover on ambiguous param");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value, "```systemverilog\nparameter W: int\n```",
        "ambiguity omits rather than guesses: {value}"
    );
}

#[test]
fn definition_on_instance_resolves_to_module_def() {
    let a = sample_analysis();
    let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("definition of u0");
    assert_eq!(loc.uri, Url::from_file_path("/x/top.sv").unwrap());
    assert_eq!(loc.range.start.line, 0);
    assert_eq!(loc.range.start.character, 7); // module m at col 8 → 0-based 7
}

#[test]
fn definition_on_module_name_resolves_in_place() {
    let a = sample_analysis();
    let loc = definition_at(&a, "/x/top.sv", 0, 7).expect("definition of m");
    assert_eq!(loc.range.start.line, 0);
    assert_eq!(loc.range.start.character, 7);
}

#[test]
fn definition_at_bound_position_serves_the_semantic_binding_target() {
    // Synthetic semantic capture: the reference at 0-based (0,19) (`u0`) is
    // bound to a declaration in another file.  The binding path must win
    // over the index (which would resolve the instance to /x/top.sv).
    let mut bindings: RefBindings = HashMap::new();
    bindings.insert(
        ("/x/top.sv".to_owned(), 0, 19),
        DeclTarget {
            name: "u0".to_owned(),
            kind: "module".to_owned(),
            file: "/x/bound.sv".to_owned(),
            line0: 4,
            col0: 2,
            via_label: false,
            via_connection: false,
        },
    );
    let a = sample_analysis_with_bindings(bindings);
    let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("binding-precise definition");
    assert_eq!(loc.uri, Url::from_file_path("/x/bound.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(4, 2));
    // Same identifier-width range convention as `entry_location`: the
    // range spans exactly the target name.
    assert_eq!(loc.range.end, Position::new(4, 4));
}

#[test]
fn definition_at_unbound_position_falls_back_to_index_resolution() {
    let a = sample_analysis_with_bindings(HashMap::new());
    // (0,19) is the `u0` instance declaration; with no binding for the
    // position the index resolves the instance to its module definition.
    let loc = definition_at(&a, "/x/top.sv", 0, 19).expect("fallback definition");
    assert_eq!(loc.uri, Url::from_file_path("/x/top.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 7));
}

#[test]
fn port_labels_join_ref_bindings_in_new_with_outcome() {
    let a = multiline_port_analysis();
    // Both continuation-line labels are registered as port labels AND
    // folded into `ref_bindings`, targeting the child module's port
    // declarations in /x/a.sv.
    let clk = a
        .ref_bindings
        .get(&("/x/b.sv".to_owned(), 1, 3))
        .expect("clk label binding");
    assert_eq!(clk.name, "clk");
    assert_eq!(clk.kind, "port");
    assert_eq!(clk.file, "/x/a.sv");
    assert_eq!((clk.line0, clk.col0), (0, 23));
    assert!(
        clk.via_label,
        "label-folded bindings must be tagged via_label"
    );
    let o = a
        .ref_bindings
        .get(&("/x/b.sv".to_owned(), 2, 3))
        .expect("o label binding");
    assert_eq!(o.name, "o");
    assert_eq!(o.kind, "port");
    assert_eq!(o.file, "/x/a.sv");
    // Definition at the bound label positions serves the folded targets.
    let loc = definition_at(&a, "/x/b.sv", 1, 3).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
}

#[test]
fn merged_ref_bindings_prefers_semantic_targets_on_collision() {
    // Semantic bindings are inserted after port-label ones. Where both
    // capture the same position, the elaboration-backed target wins.
    let mut index = SymbolIndex::default();
    index.decls.push(SymEntry {
        name: "clk".to_owned(),
        kind: SymKind::Port,
        file: "/x/child.sv".to_owned(),
        line: 3,
        col: 4,
        end_line: 3,
        end_col: 7,
        is_decl: true,
        scope: None,
        detail: None,
    });
    index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);
    index
        .port_labels
        .insert(("/x/top.sv".to_owned(), 10, 11), 0);

    let mut semantic: RefBindings = HashMap::new();
    // Overlaps the (8,9) port-label entry with a different target...
    semantic.insert(
        ("/x/top.sv".to_owned(), 8, 9),
        DeclTarget {
            name: "clk".to_owned(),
            kind: "net".to_owned(),
            file: "/x/elab.sv".to_owned(),
            line0: 6,
            col0: 1,
            via_label: false,
            via_connection: false,
        },
    );
    // ...and adds a position the label heuristic never saw.
    semantic.insert(
        ("/x/top.sv".to_owned(), 20, 21),
        DeclTarget {
            name: "rst".to_owned(),
            kind: "net".to_owned(),
            file: "/x/elab.sv".to_owned(),
            line0: 12,
            col0: 2,
            via_label: false,
            via_connection: false,
        },
    );

    let merged = merged_ref_bindings(
        &index,
        &empty_design(),
        semantic,
        &ConnectionInputs::default(),
    );
    assert_eq!(
        merged
            .get(&("/x/top.sv".to_owned(), 8, 9))
            .map(|t| t.file.as_str()),
        Some("/x/elab.sv"),
        "semantic binding must win on collision"
    );
    let label_only = merged
        .get(&("/x/top.sv".to_owned(), 10, 11))
        .expect("disjoint port-label entry survives");
    assert_eq!(
        (label_only.file.as_str(), label_only.line0, label_only.col0),
        ("/x/child.sv", 3, 4)
    );
    let semantic_only = merged
        .get(&("/x/top.sv".to_owned(), 20, 21))
        .expect("semantic-only entry survives");
    assert_eq!(semantic_only.name, "rst");
}

#[test]
fn connection_pairs_bind_actuals_to_the_parent_scope_declaration() {
    // A resolved label (`.clk` at 0-based (8,9)) keeps its
    // child-port target (`via_label`); the paired ACTUAL identifier binds
    // to its OWN declaration in the instantiating (parent) scope, NOT to
    // the child port.  Two same-named declarations exist in the file in
    // DIFFERENT module spans; the innermost span containing the
    // instantiation line picks the parent module's declaration.
    let mut index = SymbolIndex::default();
    // Child port `clk` in the child definition file.
    index.decls.push(SymEntry {
        name: "clk".to_owned(),
        kind: SymKind::Port,
        file: "/x/child.sv".to_owned(),
        line: 3,
        col: 4,
        end_line: 3,
        end_col: 7,
        is_decl: true,
        scope: None,
        detail: None,
    });
    // Parent-scope net `wa` inside module `other` (decoy span).
    index.decls.push(SymEntry {
        name: "wa".to_owned(),
        kind: SymKind::Net,
        file: "/x/top.sv".to_owned(),
        line: 1,
        col: 2,
        end_line: 1,
        end_col: 4,
        is_decl: true,
        scope: Some("other".to_owned()),
        detail: None,
    });
    // Parent-scope net `wa` inside module `parent` — the enclosing scope.
    index.decls.push(SymEntry {
        name: "wa".to_owned(),
        kind: SymKind::Net,
        file: "/x/top.sv".to_owned(),
        line: 6,
        col: 2,
        end_line: 6,
        end_col: 4,
        is_decl: true,
        scope: Some("parent".to_owned()),
        detail: None,
    });
    index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: Vec::new(),
        modules: vec![
            ModuleDef {
                name: "other".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 1,
                col: 1,
                end_line: 3,
                end_col: 10,
            },
            ModuleDef {
                name: "parent".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 5,
                col: 1,
                end_line: 20,
                end_col: 10,
            },
        ],
        packages: Vec::new(),
        classes: Vec::new(),
    };

    let connections = ConnectionInputs {
        parse_decls: None,
        pairs: vec![NamedPortConn {
            file: "/x/top.sv".to_owned(),
            label: (9, 10),
            label_name: "clk".to_owned(),
            kind: ConnKind::Port,
            actual: Some((9, 13)),
            actual_name: Some("wa".to_owned()),
            inst_type: Some("m".to_owned()),
        }],
        fallback_bindings: HashMap::new(),
        ..ConnectionInputs::default()
    };
    let merged = merged_ref_bindings(&index, &model, HashMap::new(), &connections);
    let label = merged
        .get(&("/x/top.sv".to_owned(), 8, 9))
        .expect("label binding");
    assert_eq!(
        (label.file.as_str(), label.line0, label.col0),
        ("/x/child.sv", 3, 4),
        "the label must still navigate to the CHILD port"
    );
    assert!(label.via_label && !label.via_connection);
    let actual = merged
        .get(&("/x/top.sv".to_owned(), 8, 12))
        .expect("actual binding");
    assert_eq!(
        (actual.file.as_str(), actual.line0, actual.col0),
        ("/x/top.sv", 6, 2),
        "the actual must navigate to its parent-scope declaration"
    );
    assert_eq!(actual.name, "wa");
    assert_eq!(actual.kind, "net");
    assert!(actual.via_connection && !actual.via_label);
}

#[test]
fn connection_actual_fold_never_overrides_an_explicit_binding() {
    // Collision rule: the ACTUAL fold only fills unbound positions; an
    // existing explicit binding (elaboration-backed or fallback) wins.
    let mut index = SymbolIndex::default();
    index.decls.push(SymEntry {
        name: "wa".to_owned(),
        kind: SymKind::Net,
        file: "/x/top.sv".to_owned(),
        line: 1,
        col: 2,
        end_line: 1,
        end_col: 4,
        is_decl: true,
        scope: None,
        detail: None,
    });
    index.decls.push(SymEntry {
        name: "clk".to_owned(),
        kind: SymKind::Port,
        file: "/x/child.sv".to_owned(),
        line: 3,
        col: 4,
        end_line: 3,
        end_col: 7,
        is_decl: true,
        scope: None,
        detail: None,
    });
    index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

    let mut semantic: RefBindings = HashMap::new();
    semantic.insert(
        ("/x/top.sv".to_owned(), 8, 12),
        DeclTarget {
            name: "wa".to_owned(),
            kind: "net".to_owned(),
            file: "/x/elab.sv".to_owned(),
            line0: 6,
            col0: 1,
            via_label: false,
            via_connection: false,
        },
    );

    let connections = ConnectionInputs {
        parse_decls: None,
        pairs: vec![NamedPortConn {
            file: "/x/top.sv".to_owned(),
            label: (9, 10),
            label_name: "clk".to_owned(),
            kind: ConnKind::Port,
            actual: Some((9, 13)),
            actual_name: Some("wa".to_owned()),
            inst_type: Some("m".to_owned()),
        }],
        fallback_bindings: HashMap::new(),
        ..ConnectionInputs::default()
    };
    let merged = merged_ref_bindings(&index, &empty_design(), semantic, &connections);
    let kept = merged
        .get(&("/x/top.sv".to_owned(), 8, 12))
        .expect("pre-existing binding survives");
    assert_eq!(
        (kept.file.as_str(), kept.line0, kept.col0),
        ("/x/elab.sv", 6, 1),
        "existing explicit binding must win at the actual position"
    );
    assert!(!kept.via_connection);
}

#[test]
fn connection_actual_without_parent_scope_candidate_stays_unbound() {
    // No same-name declaration exists in the instantiating file: NO
    // binding is emitted for the actual (never the child port).
    let mut index = SymbolIndex::default();
    index.decls.push(SymEntry {
        name: "clk".to_owned(),
        kind: SymKind::Port,
        file: "/x/child.sv".to_owned(),
        line: 3,
        col: 4,
        end_line: 3,
        end_col: 7,
        is_decl: true,
        scope: None,
        detail: None,
    });
    index.port_labels.insert(("/x/top.sv".to_owned(), 8, 9), 0);

    let connections = ConnectionInputs {
        parse_decls: None,
        pairs: vec![NamedPortConn {
            file: "/x/top.sv".to_owned(),
            label: (9, 10),
            label_name: "clk".to_owned(),
            kind: ConnKind::Port,
            actual: Some((9, 13)),
            actual_name: Some("ghost".to_owned()),
            inst_type: Some("m".to_owned()),
        }],
        fallback_bindings: HashMap::new(),
        ..ConnectionInputs::default()
    };
    let merged = merged_ref_bindings(&index, &empty_design(), HashMap::new(), &connections);
    assert!(!merged.contains_key(&("/x/top.sv".to_owned(), 8, 12)));
    assert!(merged.contains_key(&("/x/top.sv".to_owned(), 8, 9)));
}

/// End-to-end over Slang's classifier-labeled connection tokens.
///
/// Pins three facts at once:
///
/// * labels stay indexed as REFERENCES (never declarations) and keep
///   navigating to the child module's declaration;
/// * connected-signal/RHS positions navigate to their PARENT-scope
///   declaration;
/// * the semantic-token stream marks exactly the label positions with the
///   `connectionLabel` modifier (`function/connectionLabel`,
///   `property/readonly+connectionLabel`) while the connected signals
///   stay plain `variable`.
#[test]
fn connection_label_tokens_index_as_references_and_highlight_as_labels() {
    // `/x/a.sv`:
    //   line 1: `module m(input logic clk);`   — port `clk` at col 22
    //   line 2: `  parameter int W = 4;`       — param `W` at col 17
    // `/x/b.sv`:
    //   line 1: `module top; m #(.W(w)) u0 (.clk(c));`
    //     type `m` col 13, override label `W` col 18, RHS `w` col 20,
    //     instance `u0` col 24, port label `clk` col 29, actual `c` col 33
    //   line 2: `  logic w;` — parent net `w` at col 9
    //   line 3: `  logic c;` — parent net `c` at col 9
    let node = |line: u32, col: u32, t: i32, name: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: String::new(),
    };
    let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
        FileTokens {
            path: path.to_owned(),
            nodes: nodes
                .into_iter()
                .map(|(l, c, t, n)| {
                    let mut v = node(l, c, t, n);
                    v.file = path.to_owned();
                    v
                })
                .collect(),
        }
    };

    let a_file = mk(
        vec![
            (
                1,
                8,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "m",
            ),
            (
                1,
                22,
                tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET,
                "clk",
            ),
            (
                2,
                17,
                tokens::TOKEN_SLANG_PARAMETER + tokens::TOKEN_DECLARATION_OFFSET,
                "W",
            ),
        ],
        "/x/a.sv",
    );
    let b_file = mk(
        vec![
            (1, 13, tokens::TOKEN_SLANG_MODULE, "m"),
            (1, 18, tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL, "W"),
            (1, 20, tokens::TOKEN_SLANG_IDENTIFIER, "w"),
            (
                1,
                24,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
            ),
            (1, 29, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "clk"),
            (1, 33, tokens::TOKEN_SLANG_IDENTIFIER, "c"),
            (
                2,
                9,
                tokens::TOKEN_SLANG_VARIABLE + tokens::TOKEN_DECLARATION_OFFSET,
                "w",
            ),
            (
                3,
                9,
                tokens::TOKEN_SLANG_VARIABLE + tokens::TOKEN_DECLARATION_OFFSET,
                "c",
            ),
        ],
        "/x/b.sv",
    );

    let ty = TypeInfo {
        kind: "logic".to_owned(),
        width: Some(1),
        signed: false,
        type_name: None,
    };
    let module_m = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/a.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 2,
        end_col: 26,
    };
    let module_top = ModuleDef {
        name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 4,
        end_col: 12,
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 24,
        ports: vec![PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: ty.clone(),
        }],
        signals: Vec::new(),
        params: vec![ParamModel {
            name: "W".to_owned(),
            value: None,
            ty: ty.clone(),
            local: false,
        }],
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: Vec::new(),
        signals: vec![
            SignalModel {
                name: "w".to_owned(),
                kind: "net".to_owned(),
                ty: ty.clone(),
            },
            SignalModel {
                name: "c".to_owned(),
                kind: "net".to_owned(),
                ty: ty.clone(),
            },
        ],
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: vec![module_top, module_m],
        packages: Vec::new(),
        classes: Vec::new(),
    };

    let pairs = vec![
        NamedPortConn {
            file: "/x/b.sv".to_owned(),
            label: (1, 29),
            label_name: "clk".to_owned(),
            kind: ConnKind::Port,
            actual: Some((1, 33)),
            actual_name: Some("c".to_owned()),
            inst_type: Some("m".to_owned()),
        },
        NamedPortConn {
            file: "/x/b.sv".to_owned(),
            label: (1, 18),
            label_name: "W".to_owned(),
            kind: ConnKind::Param,
            actual: Some((1, 20)),
            actual_name: Some("w".to_owned()),
            inst_type: Some("m".to_owned()),
        },
    ];
    let a = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        vec![a_file, b_file],
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_decls: None,
            pairs,
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        },
    );

    // Labels are REF entries, never declarations.
    let port_label_entry = a
        .index
        .entry_at("/x/b.sv", 0, 28)
        .expect("port label indexed");
    assert!(!port_label_entry.is_decl);
    let param_label_entry = a
        .index
        .entry_at("/x/b.sv", 0, 17)
        .expect("param override label indexed");
    assert!(!param_label_entry.is_decl);

    // Labels navigate to the CHILD module's declarations…
    let loc = definition_at(&a, "/x/b.sv", 0, 28).expect("definition at .clk label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 21));
    let loc = definition_at(&a, "/x/b.sv", 0, 17).expect("definition at .W label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16));

    // …while the connected signal / override RHS stay parent-scope.
    let loc = definition_at(&a, "/x/b.sv", 0, 32).expect("definition at actual c");
    assert_eq!(loc.uri, Url::from_file_path("/x/b.sv").unwrap());
    let loc = definition_at(&a, "/x/b.sv", 0, 19).expect("definition at override RHS w");
    assert_eq!(loc.uri, Url::from_file_path("/x/b.sv").unwrap());

    // Semantic surface: exactly the label rows carry `connectionLabel`.
    let legend = crate::semantic_tokens::legend();
    let data = semantic_tokens_for(&a, "/x/b.sv").data;
    let decode_sym = |want_line: u64, want_col: u64| -> String {
        let mut line = 0u64;
        let mut col = 0u64;
        for token in &data {
            line += token.delta_line as u64;
            col = if token.delta_line == 0 {
                col + token.delta_start as u64
            } else {
                token.delta_start as u64
            };
            if (line, col) == (want_line, want_col) {
                let base = legend.token_types[token.token_type as usize].as_str();
                let modifiers: Vec<String> = legend
                    .token_modifiers
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| token.token_modifiers_bitset & (1u32 << bit) != 0)
                    .map(|(_, name)| name.as_str().to_owned())
                    .collect();
                return if modifiers.is_empty() {
                    base.to_owned()
                } else {
                    format!("{base}/{}", modifiers.join("+"))
                };
            }
        }
        panic!("no semantic token at {want_line}:{want_col}");
    };
    assert_eq!(decode_sym(0, 28), "function/connectionLabel");
    assert_eq!(decode_sym(0, 17), "property/readonly+connectionLabel");
    assert_eq!(decode_sym(0, 32), "variable");
    assert_eq!(decode_sym(0, 19), "variable");
}

#[test]
fn fallback_connection_bindings_survive_the_merge() {
    // Parse-fallback mode carries pre-resolved label+actual bindings;
    // they are inserted before semantic bindings (which are empty there).
    let connections = ConnectionInputs {
        parse_decls: None,
        pairs: Vec::new(),
        fallback_bindings: [
            (
                ("/x/tb.sv".to_owned(), 4, 12),
                DeclTarget {
                    name: "clk".to_owned(),
                    kind: "port".to_owned(),
                    file: "/x/child.sv".to_owned(),
                    line0: 0,
                    col0: 23,
                    via_label: true,
                    via_connection: false,
                },
            ),
            (
                ("/x/tb.sv".to_owned(), 4, 16),
                DeclTarget {
                    name: "clk".to_owned(),
                    kind: "port".to_owned(),
                    file: "/x/child.sv".to_owned(),
                    line0: 0,
                    col0: 23,
                    via_label: false,
                    via_connection: true,
                },
            ),
        ]
        .into_iter()
        .collect(),
        ..ConnectionInputs::default()
    };
    let index = SymbolIndex::default();
    let merged = merged_ref_bindings(&index, &empty_design(), HashMap::new(), &connections);
    assert_eq!(merged.len(), 2, "both fallback entries survive: {merged:?}");
    assert!(merged[&("/x/tb.sv".to_owned(), 4, 12)].via_label);
    assert!(merged[&("/x/tb.sv".to_owned(), 4, 16)].via_connection);
}

#[test]
fn references_include_declaration() {
    let a = sample_analysis();
    let refs = references_at(&a, "/x/top.sv", 0, 4);
    assert!(refs
        .iter()
        .any(|l| l.range.start.line == 0 && l.range.start.character == 4));
}

#[test]
fn references_options_include_declaration_preserves_existing_results() {
    let a = cross_file_analysis();
    let existing = references_at(&a, "/x/a.sv", 0, 7);
    let with_option = references_at_with_options(&a, "/x/a.sv", 0, 7, true);

    assert_eq!(with_option, existing);
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path("/x/a.sv").unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path("/x/b.sv").unwrap()
            && location.range.start == Position::new(0, 12)
    }));
}

#[test]
fn references_options_exclude_indexed_declaration() {
    let a = cross_file_analysis();
    let refs = references_at_with_options(&a, "/x/a.sv", 0, 7, false);

    assert!(!refs.iter().any(|location| {
        location.uri == Url::from_file_path("/x/a.sv").unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(refs.iter().any(|location| {
        location.uri == Url::from_file_path("/x/b.sv").unwrap()
            && location.range.start == Position::new(0, 12)
    }));
}

#[test]
fn references_options_filter_explicit_slang_declaration() {
    let node = |line: u32, ty: i32| TokenInfo {
        line,
        col: 1,
        end_line: line,
        end_col: 7,
        kind: ty,
        name: Some("thing".to_owned()),
        file: "/x/fallback.sv".to_owned(),
    };
    let a = Analysis::new(
        Vec::new(),
        empty_design(),
        vec![FileTokens {
            path: "/x/fallback.sv".to_owned(),
            nodes: vec![
                node(
                    1,
                    tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                ),
                node(2, tokens::TOKEN_SLANG_IDENTIFIER),
            ],
        }],
        Vec::new(),
    );

    assert!(a
        .index
        .entry_at("/x/fallback.sv", 0, 0)
        .is_some_and(|entry| entry.is_decl));
    let with_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, true);
    let without_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, false);

    assert_eq!(with_declaration.len(), 2);
    assert_eq!(without_declaration.len(), 1);
    assert_eq!(without_declaration[0].range.start, Position::new(1, 0));
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

#[test]
fn document_symbols_contain_module_with_port_child() {
    let a = sample_analysis();
    let syms = document_symbols(&a, "/x/top.sv");
    let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
    assert_eq!(module.kind, SymbolKind::MODULE);
    assert_eq!(module.range.start.line, 0);
    let children = module.children.as_ref().expect("children");
    let port = children
        .iter()
        .find(|c| c.name == "clk")
        .expect("port child");
    assert_eq!(port.kind, SymbolKind::PROPERTY);
    assert_eq!(port.range.start.line, 0);
    assert_eq!(port.range.start.character, 4);
    assert!(syms
        .iter()
        .any(|s| s.name == "p" && s.kind == SymbolKind::PACKAGE));
}

#[test]
fn document_symbols_include_functions_and_tasks() {
    let a = sample_analysis();
    let syms = document_symbols(&a, "/x/top.sv");
    let add = syms
        .iter()
        .find(|s| s.name == "add")
        .expect("function symbol");
    assert_eq!(add.kind, SymbolKind::FUNCTION);
    assert_eq!(add.range.start, Position::new(3, 7));
    assert_eq!(
        add.detail.as_deref(),
        Some("function int add(input int a, input int b)")
    );
    let run = syms.iter().find(|s| s.name == "run").expect("task symbol");
    assert_eq!(run.kind, SymbolKind::FUNCTION);
    assert_eq!(run.range.start, Position::new(4, 7));
    assert_eq!(run.detail.as_deref(), Some("task run(input int n)"));
}

#[test]
fn document_symbol_children_carry_type_only_details() {
    // Model details carry names/values (`input logic clk`,
    // `parameter W: int = 32'sd8`); document-symbol children must render
    // the type text without the declared name.
    let a = sample_analysis();
    let syms = document_symbols(&a, "/x/top.sv");
    let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
    let children = module.children.as_ref().expect("children");
    let port = children
        .iter()
        .find(|c| c.name == "clk")
        .expect("port child");
    assert_eq!(port.detail.as_deref(), Some("input logic"));
    let param = children.iter().find(|c| c.name == "W").expect("param");
    assert_eq!(param.detail.as_deref(), Some("parameter int"));
}

#[test]
fn decl_details_drive_type_only_details() {
    let a = sample_analysis();
    // Keys are the 1-based declaration positions of `clk` and `W`.
    let mut snippets = HashMap::new();
    snippets.insert(
        ("/x/top.sv".to_owned(), 1u32, 5u32),
        "input logic [1:0] clk".to_owned(),
    );
    snippets.insert(("/x/top.sv".to_owned(), 2u32, 5u32), "int W".to_owned());
    let a = a.with_decl_details(snippets);
    let syms = document_symbols(&a, "/x/top.sv");
    let module = syms.iter().find(|s| s.name == "m").expect("module symbol");
    let children = module.children.as_ref().expect("children");
    let port = children
        .iter()
        .find(|c| c.name == "clk")
        .expect("port child");
    assert_eq!(port.detail.as_deref(), Some("input logic [1:0]"));
    let param = children.iter().find(|c| c.name == "W").expect("param");
    // Parameters keep their model keyword and gain the declared type word.
    assert_eq!(param.detail.as_deref(), Some("parameter int"));
}

/// Model + tokens for /x/top.sv with an instantiation chain: top-level
/// instance `tb` (of module `top`) containing `u0` (of module `m`).
fn hierarchy_parts() -> (DesignModel, Vec<FileTokens>) {
    let leaf_port = PortModel {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        },
    };
    let leaf = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "tb.u0".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 2,
        col: 9,
        ports: vec![leaf_port],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let tb = InstanceModel {
        name: "tb".to_owned(),
        def_name: "top".to_owned(),
        full_name: "tb".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 6,
        col: 3,
        ports: Vec::new(),
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![leaf],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![tb],
        modules: vec![
            ModuleDef {
                name: "top".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 5,
                col: 8,
                end_line: 7,
                end_col: 12,
            },
            ModuleDef {
                name: "m".to_owned(),
                file: Some("/x/top.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 3,
                end_col: 12,
            },
        ],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let tokens = vec![FileTokens {
        path: "/x/top.sv".to_owned(),
        nodes: vec![
            TokenInfo {
                line: 2,
                col: 9,
                end_line: 2,
                end_col: 11,
                kind: tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("u0".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 6,
                col: 3,
                end_line: 6,
                end_col: 5,
                kind: tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("tb".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
        ],
    }];
    (model, tokens)
}

#[test]
fn document_symbols_attach_instance_children_with_type_detail() {
    let (model, tokens) = hierarchy_parts();
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
    let syms = document_symbols(&a, "/x/top.sv");

    // `top` instantiates `m` through `u0`: an Object-kind leaf whose
    // detail is the instantiated TYPE, ranged over the refined
    // instance-name token (1-based (2,9) → 0-based (1,8)).
    let top = syms
        .iter()
        .find(|s| s.name == "top" && s.kind == SymbolKind::MODULE)
        .expect("top symbol");
    let children = top.children.as_ref().expect("top children");
    let inst = children
        .iter()
        .find(|c| c.name == "u0")
        .expect("instance child");
    assert_eq!(inst.kind, SymbolKind::OBJECT);
    assert_eq!(inst.detail.as_deref(), Some("m"));
    assert_eq!(inst.range.start, Position::new(1, 8));
    assert_eq!(inst.selection_range, inst.range);
    assert!(inst.children.is_none(), "instances are leaf symbols");

    // `m` itself instantiates nothing: no Object children.
    let m = syms
        .iter()
        .find(|s| s.name == "m" && s.kind == SymbolKind::MODULE)
        .expect("m symbol");
    let m_children = m.children.as_deref().unwrap_or_default();
    assert!(
        !m_children.iter().any(|c| c.kind == SymbolKind::OBJECT),
        "m children: {m_children:?}"
    );
}

#[test]
fn type_only_detail_helpers_degrade_without_guessing() {
    // Trailing-name stripping.
    assert_eq!(
        strip_trailing_name("input logic [31:0] q").as_deref(),
        Some("input logic [31:0]")
    );
    assert_eq!(strip_trailing_name("wire w1").as_deref(), Some("wire"));
    assert_eq!(
        strip_trailing_name("array logic [3:0] mem").as_deref(),
        Some("array logic [3:0]")
    );
    // Degenerate inputs yield nothing instead of a wrong guess.
    assert_eq!(strip_trailing_name(""), None);
    assert_eq!(strip_trailing_name("clk"), None);
    assert_eq!(strip_trailing_name("var w1"), None, "unknown-type marker");
    assert_eq!(strip_trailing_name("logic [7:0]"), None, "no name tail");
    // Parameter detail shapes.
    assert_eq!(
        param_type_detail(Some("parameter W: int = 32'sd8"), None).as_deref(),
        Some("parameter int")
    );
    assert_eq!(
        param_type_detail(Some("localparam DEPTH: int"), None).as_deref(),
        Some("localparam int")
    );
    assert_eq!(
        param_type_detail(Some("localparam S: struct pair_t"), None).as_deref(),
        Some("localparam struct pair_t")
    );
    // Unknown types degrade to the bare keyword; non-parameter shapes to
    // nothing.
    assert_eq!(
        param_type_detail(Some("localparam X: other"), None).as_deref(),
        Some("localparam")
    );
    assert_eq!(param_type_detail(Some("module top"), None), None);
    // A captured snippet wins over the legacy colon shape.
    assert_eq!(
        param_type_detail(Some("parameter W: int = 4"), Some("logic [7:0] W")).as_deref(),
        Some("parameter logic [7:0]")
    );
    // Model-derived fallback texts.
    let port = PortModel {
        name: "bus".to_owned(),
        direction: Direction::Inout,
        ty: TypeInfo {
            kind: "other".to_owned(),
            width: None,
            signed: false,
            type_name: None,
        },
    };
    assert_eq!(port_type_only(&port).as_deref(), Some("inout"));
    let sig = SignalModel {
        name: "mem".to_owned(),
        kind: "array".to_owned(),
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(8),
            signed: false,
            type_name: None,
        },
    };
    assert_eq!(signal_type_only(&sig).as_deref(), Some("array logic [7:0]"));
}

#[test]
fn completion_filters_by_prefix() {
    let a = sample_analysis();
    let items = completion_at(&a, "/x/top.sv", 0, 3, "mod");
    assert!(
        items.iter().any(|i| i.label == "module"),
        "items: {items:?}"
    );
    let all = completion_at(&a, "/x/top.sv", 0, 0, "");
    assert!(
        all.iter()
            .any(|i| i.label == "m" && i.kind == Some(CompletionItemKind::MODULE)),
        "items: {all:?}"
    );
}

#[test]
fn completion_includes_function_and_task_names() {
    let a = sample_analysis();
    let all = completion_at(&a, "/x/top.sv", 0, 0, "");
    assert!(
        all.iter()
            .any(|i| i.label == "add" && i.kind == Some(CompletionItemKind::FUNCTION)),
        "items: {all:?}"
    );
    assert!(
        all.iter()
            .any(|i| i.label == "run" && i.kind == Some(CompletionItemKind::FUNCTION)),
        "items: {all:?}"
    );
    // Prefix filtering applies to function candidates too.
    let pre = completion_at(&a, "/x/top.sv", 0, 2, "ad");
    assert!(pre.iter().any(|i| i.label == "add"), "items: {pre:?}");
    // The model and index-backed sources must not double-list a function.
    assert_eq!(all.iter().filter(|i| i.label == "add").count(), 1);
}

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

#[test]
fn semantic_tokens_require_exact_source_identity() {
    let a = sample_analysis();
    assert!(semantic_tokens_for(&a, "/symlink/top.sv").data.is_empty());
    assert!(!semantic_tokens_for(&a, "/x/top.sv").data.is_empty());
}

#[test]
fn semantic_tokens_are_empty_for_a_file_with_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

    // Assert
    assert!(tokens.data.is_empty());
}

#[test]
fn semantic_tokens_remain_available_when_another_file_has_a_syntax_error() {
    // Arrange
    let mut analysis = sample_analysis();
    analysis.diagnostics.push(Diag {
        severity: Severity::Syntax,
        file: Some("/other/top.sv".to_owned()),
        line: 1,
        col: 1,
        message: "incomplete module".to_owned(),
    });

    // Act
    let tokens = semantic_tokens_for(&analysis, "/x/top.sv");

    // Assert
    assert!(!tokens.data.is_empty());
}

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
fn settings_obj(entries: Vec<(&str, LSPAny)>) -> LSPAny {
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

/// Regression: the semantic-token stream includes the `module` keyword as
/// well as the declaration identifier.
#[test]
fn semantic_tokens_cover_the_module_declaration_keyword() {
    use tower_lsp::lsp_types::SemanticTokenType;

    let _guards = analysis_guards();
    let fixture = std::env::temp_dir().join(format!("llg_modkw_{}", std::process::id()));
    let rtl = fixture.join("rtl");
    std::fs::create_dir_all(&rtl).expect("create fixture tree");
    let sv = rtl.join("modkw.sv");
    std::fs::write(
        &sv,
        "module mod_kw(input logic clk);\n  wire w;\nendmodule\n",
    )
    .expect("write design");

    let opts = CompileOpts {
        files: vec![sv.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let analysis = analyze_with_config(&opts, &LintConfig::default());
    assert!(
        analysis.is_valid(),
        "analysis failed: {:?}",
        analysis.diagnostics
    );

    let legend = crate::semantic_tokens::legend();
    let keyword_index = legend
        .token_types
        .iter()
        .position(|t| *t == SemanticTokenType::KEYWORD)
        .expect("keyword type in legend") as u32;

    // Decode the delta-encoded stream back to absolute (line, col, len).
    let tokens = semantic_tokens_for(&analysis, &sv.to_string_lossy());
    let mut line = 0u32;
    let mut col = 0u32;
    let mut keywords: Vec<(u32, u32, u32)> = Vec::new();
    for token in &tokens.data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        if token.token_type == keyword_index {
            keywords.push((line, col, token.length));
        }
    }
    assert!(
        keywords.contains(&(0, 0, "module".len() as u32)),
        "expected a keyword token over `module` at 0:0, got {keywords:?}"
    );
    assert!(
        keywords.contains(&(2, 0, "endmodule".len() as u32)),
        "expected a keyword token over `endmodule` at 2:0, got {keywords:?}"
    );

    cleanup_process_shadow();
    let _ = std::fs::remove_dir_all(fixture);
}

/// `settings_to_lint_config` maps the documented settings shape: a disabled
/// rule, a severity override, and untouched defaults for unmentioned rules.
#[test]
fn settings_to_lint_config_maps_rule_overrides() {
    let settings = settings_obj(vec![(
        "lint",
        settings_obj(vec![(
            "rules",
            settings_obj(vec![
                (
                    "unused-signal",
                    settings_obj(vec![("enabled", LSPAny::Bool(false))]),
                ),
                (
                    "width-mismatch",
                    settings_obj(vec![("severity", LSPAny::String("error".to_owned()))]),
                ),
            ]),
        )]),
    )]);

    let cfg = settings_to_lint_config(&settings);
    assert!(
        !cfg.is_enabled("unused-signal"),
        "unused-signal should be disabled"
    );
    assert_eq!(cfg.severity("width-mismatch"), Some(LintSeverity::Error));
    assert!(
        cfg.is_enabled("incomplete-case"),
        "unmentioned rule should stay enabled"
    );
    assert_eq!(cfg.severity("incomplete-case"), None);
}

/// A global `"enabled": false` disables every known rule; a per-rule entry
/// can re-enable one.
#[test]
fn settings_to_lint_config_global_enabled_false_disables_all() {
    let settings = settings_obj(vec![(
        "lint",
        settings_obj(vec![("enabled", LSPAny::Bool(false))]),
    )]);
    let cfg = settings_to_lint_config(&settings);
    assert!(!cfg.is_enabled("unused-signal"));
    assert!(!cfg.is_enabled("naming-style"));

    let with_override = settings_obj(vec![(
        "lint",
        settings_obj(vec![
            ("enabled", LSPAny::Bool(false)),
            (
                "rules",
                settings_obj(vec![(
                    "casez-misuse",
                    settings_obj(vec![("enabled", LSPAny::Bool(true))]),
                )]),
            ),
        ]),
    )]);
    let cfg = settings_to_lint_config(&with_override);
    assert!(!cfg.is_enabled("unused-signal"));
    assert!(cfg.is_enabled("casez-misuse"));
}

/// A bare `{"rules": ...}` payload (no `lint` wrapper) is accepted.
#[test]
fn settings_to_lint_config_accepts_bare_rules_object() {
    let settings = settings_obj(vec![(
        "rules",
        settings_obj(vec![(
            "casez-misuse",
            settings_obj(vec![("enabled", LSPAny::Bool(false))]),
        )]),
    )]);
    let cfg = settings_to_lint_config(&settings);
    assert!(!cfg.is_enabled("casez-misuse"));
    assert!(cfg.is_enabled("unused-signal"));
}

/// Hand-built two-file `Analysis` proving cross-file resolution:
///
/// * `/x/a.sv`: `module m(input logic clk, output logic [3:0] o); assign o = clk; endmodule`
///   (module decl + port decls + assignment references), plus `package p`.
/// * `/x/b.sv`: `module top; m u0(.clk(c), .o(o)); endmodule`
///   (instance `u0` of `m`; the `m` type name and the named port
///   connections are reference sites).
///
/// Token roles mirror the Slang lexical snapshot: definitions carry the
/// declaration offset, expression and type uses are identifiers, and named
/// port connections are connection labels.
fn cross_file_analysis() -> Analysis {
    let node = |line: u32, col: u32, t: i32, name: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: String::new(), // filled below
    };
    let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
        FileTokens {
            path: path.to_owned(),
            nodes: nodes
                .into_iter()
                .map(|(l, c, t, n)| {
                    let mut v = node(l, c, t, n);
                    v.file = path.to_owned();
                    v
                })
                .collect(),
        }
    };

    let a_file = mk(
        vec![
            (
                1,
                8,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "m",
            ),
            (
                1,
                24,
                tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET,
                "clk",
            ),
            (
                1,
                47,
                tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET,
                "o",
            ),
            (2, 10, tokens::TOKEN_SLANG_IDENTIFIER, "o"),
            (2, 14, tokens::TOKEN_SLANG_IDENTIFIER, "clk"),
        ],
        "/x/a.sv",
    );
    let b_file = mk(
        vec![
            (
                1,
                8,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "top",
            ),
            (1, 13, tokens::TOKEN_SLANG_MODULE, "m"),
            (
                1,
                15,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
            ),
            (1, 19, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "clk"),
            (1, 23, tokens::TOKEN_SLANG_IDENTIFIER, "c"),
            (1, 28, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "o"),
            (1, 30, tokens::TOKEN_SLANG_IDENTIFIER, "o"),
        ],
        "/x/b.sv",
    );

    let module_m = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/a.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 2,
        end_col: 16,
    };
    let port = |name: &str, dir: Direction| PortModel {
        name: name.to_owned(),
        direction: dir,
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        },
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 15,
        ports: vec![port("clk", Direction::Input), port("o", Direction::Output)],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: vec![port("c", Direction::Input), port("o", Direction::Output)],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: vec![module_m],
        packages: vec![PackageDef {
            name: "p".to_owned(),
            file: Some("/x/a.sv".to_owned()),
            line: 3,
            col: 1,
            params: Vec::new(),
            enum_consts: Vec::new(),
        }],
        classes: Vec::new(),
    };
    Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new())
}

/// Hand-built analysis for two `Bar` instantiations inside `Foo`:
/// `Bar Bar(...)` and `Bar u_bar(...)`.  The first instance deliberately
/// shares its name with the module type so the type-reference resolver's
/// scope and kind behavior can be tested independently from frontend details.
fn module_type_instance_collision_analysis() -> Analysis {
    let mut analysis = cross_file_analysis();
    analysis.model.design_name = "Foo".to_owned();
    analysis.model.modules[0].name = "Bar".to_owned();
    analysis.model.modules.push(ModuleDef {
        name: "Foo".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 4,
        end_col: 12,
    });
    {
        let top = &mut analysis.model.top_instances[0];
        top.name = "Foo".to_owned();
        top.def_name = "Foo".to_owned();
        top.full_name = "Foo".to_owned();

        let first = &mut top.children[0];
        first.name = "Bar".to_owned();
        first.def_name = "Bar".to_owned();
        first.full_name = "Foo.Bar".to_owned();

        let mut second = first.clone();
        second.name = "u_bar".to_owned();
        second.full_name = "Foo.u_bar".to_owned();
        second.line = 2;
        second.col = 7;
        top.children.push(second);
    }

    for file_tokens in &mut analysis.tokens {
        for node in &mut file_tokens.nodes {
            if file_tokens.path == "/x/a.sv"
                && node.kind == tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET
                && node.name.as_deref() == Some("m")
            {
                node.name = Some("Bar".to_owned());
            }
            if file_tokens.path == "/x/b.sv" {
                if node.kind == tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET
                    && node.name.as_deref() == Some("top")
                {
                    node.name = Some("Foo".to_owned());
                }
                if node.kind == tokens::TOKEN_SLANG_MODULE && node.name.as_deref() == Some("m") {
                    node.name = Some("Bar".to_owned());
                }
                if node.kind == tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET
                    && node.name.as_deref() == Some("u0")
                {
                    node.name = Some("Bar".to_owned());
                }
            }
        }
    }
    let b_file = analysis
        .tokens
        .iter_mut()
        .find(|file_tokens| file_tokens.path == "/x/b.sv")
        .expect("collision fixture file");
    b_file.nodes.extend([
        TokenInfo {
            line: 2,
            col: 3,
            end_line: 2,
            end_col: 6,
            kind: tokens::TOKEN_SLANG_MODULE,
            name: Some("Bar".to_owned()),
            file: "/x/b.sv".to_owned(),
        },
        TokenInfo {
            line: 2,
            col: 7,
            end_line: 2,
            end_col: 12,
            kind: tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
            name: Some("u_bar".to_owned()),
            file: "/x/b.sv".to_owned(),
        },
    ]);
    analysis.index = SymbolIndex::build(&analysis);
    analysis
}

#[test]
fn module_type_definition_ignores_same_named_instance_in_scope() {
    // Arrange
    let analysis = module_type_instance_collision_analysis();

    // Act
    let first_type = definition_at(&analysis, "/x/b.sv", 0, 12);
    let second_type = definition_at(&analysis, "/x/b.sv", 1, 2);

    // Assert
    for location in [first_type, second_type] {
        let location = location.expect("module type definition");
        assert_eq!(location.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(location.range.start, Position::new(0, 7));
    }
}

#[test]
fn instance_name_definition_still_resolves_when_name_matches_module_type() {
    // Arrange
    let analysis = module_type_instance_collision_analysis();

    // Act
    let colliding_instance = definition_at(&analysis, "/x/b.sv", 0, 14);
    let ordinary_instance = definition_at(&analysis, "/x/b.sv", 1, 6);

    // Assert
    for location in [colliding_instance, ordinary_instance] {
        let location = location.expect("instance definition");
        assert_eq!(location.uri, Url::from_file_path("/x/a.sv").unwrap());
        assert_eq!(location.range.start, Position::new(0, 7));
    }
}

/// Hand-built two-file `Analysis` for a multi-line instantiation:
///
/// * `/x/a.sv`: same as [`cross_file_analysis`] (`module m(input logic
///   clk, output logic [3:0] o); assign o = clk; endmodule`).
/// * `/x/b.sv`: the instantiation is spread over several lines, so the
///   labels live on continuation lines below the instance name:
///
///   ```text
///   module top; m u0(    ← instance `u0` at 0-based (0, 8)
///       .clk(c),         ← label at 0-based (1, 3)
///       .o(o)            ← label at 0-based (2, 3)
///   );                   ← closing at 0-based (3, 0)
///   ```
///
///   plus a decoy connection-label token named `clk` at 0-based (10, 0)
///   that must not be treated as an instance port label: its column is 0,
///   so it is outside the indented continuation lines.
///
/// The `mk` helper takes 1-based positions (as `TokenInfo` reports
/// them); the index converts them to 0-based.
fn multiline_port_analysis() -> Analysis {
    let node = |line: u32, col: u32, t: i32, name: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: String::new(), // filled below
    };
    let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
        FileTokens {
            path: path.to_owned(),
            nodes: nodes
                .into_iter()
                .map(|(l, c, t, n)| {
                    let mut v = node(l, c, t, n);
                    v.file = path.to_owned();
                    v
                })
                .collect(),
        }
    };

    let a_file = mk(
        vec![
            (
                1,
                8,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "m",
            ),
            (
                1,
                24,
                tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET,
                "clk",
            ),
            (
                1,
                47,
                tokens::TOKEN_SLANG_PORT + tokens::TOKEN_DECLARATION_OFFSET,
                "o",
            ),
            (2, 10, tokens::TOKEN_SLANG_IDENTIFIER, "o"),
            (2, 14, tokens::TOKEN_SLANG_IDENTIFIER, "clk"),
        ],
        "/x/a.sv",
    );
    let mut b_file = mk(
        vec![
            // Instance name token (1-based (1,9) → 0-based (0,8)).
            (
                1,
                9,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
            ),
            // `.clk` label (1-based (2,4) → 0-based (1,3)).
            (2, 4, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "clk"),
            // Inner expression ref `c` in `.clk(c)`.
            (2, 8, tokens::TOKEN_SLANG_IDENTIFIER, "c"),
            // `.o` label (1-based (3,4) → 0-based (2,3)).
            (3, 4, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "o"),
            // Inner expression ref `o` in `.o(o)`.
            (3, 8, tokens::TOKEN_SLANG_IDENTIFIER, "o"),
            // Decoy label at 0-based (10, 0), outside the instance span.
            (11, 1, tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL, "clk"),
        ],
        "/x/b.sv",
    );
    // Nameless closing `);` at 0-based (3, 0) (1-based (4, 1)): included
    // for fixture fidelity; the index skips nameless tokens.
    b_file.nodes.push(TokenInfo {
        line: 4,
        col: 1,
        end_line: 4,
        end_col: 2,
        kind: 0,
        name: None,
        file: "/x/b.sv".to_owned(),
    });

    let module_m = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/a.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 2,
        end_col: 16,
    };
    let port = |name: &str, dir: Direction| PortModel {
        name: name.to_owned(),
        direction: dir,
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        },
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 9,
        ports: vec![port("clk", Direction::Input), port("o", Direction::Output)],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: vec![port("c", Direction::Input), port("o", Direction::Output)],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: vec![module_m],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new())
}

#[test]
fn port_label_multiline_definition_jumps_to_child_port_decl_across_files() {
    let a = multiline_port_analysis();
    // Both continuation-line labels must be registered as port labels.
    assert!(
        a.index
            .port_labels
            .contains_key(&("/x/b.sv".to_owned(), 1, 3)),
        "port_labels: {:?}",
        a.index.port_labels
    );
    assert!(
        a.index
            .port_labels
            .contains_key(&("/x/b.sv".to_owned(), 2, 3)),
        "port_labels: {:?}",
        a.index.port_labels
    );
    // `.clk` at 0-based (1, 3) in file B → m's clk port decl in file A
    // (0-based (0, 23)), not the enclosing scope's same-named object.
    let loc = definition_at(&a, "/x/b.sv", 1, 3).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
}

#[test]
fn port_label_multiline_hover_shows_child_port() {
    let a = multiline_port_analysis();
    // `.o` label at 0-based (2, 3) in file B.
    let hover = hover_at(&a, "/x/b.sv", 2, 3).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    assert!(value.contains("o"), "value: {value}");
}

#[test]
fn port_label_multiline_decoy_at_column_zero_is_not_a_port_label() {
    let a = multiline_port_analysis();
    // The decoy at 0-based (10, 0) is a connection label named `clk`, but
    // its column is 0, so the continuation-line rule rejects it. It falls
    // back to ordinary name-based resolution (which lands on the same
    // workspace `clk` declaration here).
    assert!(
        !a.index
            .port_labels
            .contains_key(&("/x/b.sv".to_owned(), 10, 0)),
        "decoy must not be a port label: {:?}",
        a.index.port_labels
    );
    let loc = definition_at(&a, "/x/b.sv", 10, 0).expect("decoy name-based resolution");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 23), "loc: {loc:?}");
}

#[test]
fn entry_at_on_instance_returns_instance_decl() {
    let a = cross_file_analysis();
    let e = a.index.entry_at("/x/b.sv", 0, 14).expect("entry at u0");
    assert_eq!(e.name, "u0");
    assert_eq!(e.kind, SymKind::Instance);
    assert!(e.is_decl);
    assert_eq!(e.scope.as_deref(), Some("top"));
}

#[test]
fn definition_on_instance_jumps_to_module_def_across_files() {
    let a = cross_file_analysis();
    let loc = definition_at(&a, "/x/b.sv", 0, 14).expect("definition of u0");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 7)); // module m at col 8 → 0-based 7
}

#[test]
fn definition_on_module_type_ref_jumps_to_def_across_files() {
    let a = cross_file_analysis();
    // The `m` type name at the instantiation site in file B.
    let loc = definition_at(&a, "/x/b.sv", 0, 12).expect("definition of m ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 7));
}

#[test]
fn references_on_module_decl_span_both_files() {
    let a = cross_file_analysis();
    let refs = references_at(&a, "/x/a.sv", 0, 7); // module m decl
    assert!(
        refs.iter().any(|l| {
            l.uri == Url::from_file_path("/x/a.sv").unwrap() && l.range.start == Position::new(0, 7)
        }),
        "missing m decl in refs: {refs:?}"
    );
    assert!(
        refs.iter().any(|l| {
            l.uri == Url::from_file_path("/x/b.sv").unwrap()
                && l.range.start == Position::new(0, 12)
        }),
        "missing u0-site ref in refs: {refs:?}"
    );
}

#[test]
fn workspace_symbols_filters_by_query() {
    let a = cross_file_analysis();
    let syms = workspace_symbols(&a, "m");
    assert_eq!(syms.len(), 1, "syms: {syms:?}");
    assert_eq!(syms[0].name, "m");
    assert_eq!(syms[0].kind, SymbolKind::MODULE);
    assert_eq!(
        syms[0].location.uri,
        Url::from_file_path("/x/a.sv").unwrap()
    );

    let top_syms = workspace_symbols(&a, "TOP");
    assert!(
        top_syms
            .iter()
            .any(|s| s.name == "top" && s.kind == SymbolKind::OBJECT),
        "top_syms: {top_syms:?}"
    );
}

#[test]
fn symbol_index_merge_unions_and_dedupes_shared_occurrences() {
    let a = cross_file_analysis();
    // Merging an index with itself must dedupe every declaration and
    // reference by position while keeping the lookup maps functional.
    let merged = SymbolIndex::merge([&a.index, &a.index]);
    assert_eq!(
        merged.decls.len(),
        a.index.decls.len(),
        "duplicate declarations leaked through merge: {} vs {}",
        merged.decls.len(),
        a.index.decls.len()
    );
    assert_eq!(
        merged.refs.len(),
        a.index.refs.len(),
        "duplicate references leaked through merge: {} vs {}",
        merged.refs.len(),
        a.index.refs.len()
    );
    let e = merged
        .entry_at("/x/a.sv", 0, 23)
        .expect("entry at clk port after merge");
    assert_eq!(e.name, "clk");
    assert_eq!(e.kind, SymKind::Port);
    assert_eq!(merged.resolve(e).len(), 1, "resolution after merge");
}

#[test]
fn entry_at_on_port_token_returns_port_decl() {
    let a = cross_file_analysis();
    let e = a
        .index
        .entry_at("/x/a.sv", 0, 23)
        .expect("entry at clk port");
    assert_eq!(e.name, "clk");
    assert_eq!(e.kind, SymKind::Port);
    assert!(e.is_decl);
    assert_eq!(e.scope.as_deref(), Some("m"));
}

#[test]
fn ref_inside_assign_resolves_to_port_decl() {
    let a = cross_file_analysis();
    // `clk` in `assign o = clk;` at (line 2, col 14) → 0-based (1, 13).
    let e = a
        .index
        .entry_at("/x/a.sv", 1, 13)
        .expect("entry at clk ref");
    assert!(!e.is_decl);
    let resolved = a.index.resolve(e);
    assert_eq!(resolved.len(), 1, "resolved: {resolved:?}");
    assert_eq!(resolved[0].kind, SymKind::Port);
    assert_eq!(resolved[0].file, "/x/a.sv");
    assert_eq!((resolved[0].line, resolved[0].col), (0, 23));
    let loc = definition_at(&a, "/x/a.sv", 1, 13).expect("definition of clk ref");
    assert_eq!(loc.range.start, Position::new(0, 23));
}

#[test]
fn port_references_span_both_files() {
    let a = cross_file_analysis();
    // All references of the `o` port (file A decl) include the assignment
    // site in file A and the named connection `.o(o)` in file B.
    let o_decl = a.index.entry_at("/x/a.sv", 0, 46).expect("o port decl");
    assert_eq!(o_decl.name, "o");
    let refs = a.index.all_references(o_decl);
    let sites: Vec<(String, u32, u32)> = refs
        .iter()
        .map(|r| (r.file.clone(), r.line, r.col))
        .collect();
    assert!(
        sites.contains(&("/x/a.sv".to_owned(), 0, 46)),
        "missing o decl: {sites:?}"
    );
    assert!(
        sites.contains(&("/x/a.sv".to_owned(), 1, 9)),
        "missing assign o site: {sites:?}"
    );
    assert!(
        sites.contains(&("/x/b.sv".to_owned(), 0, 27)),
        "missing .o named connection: {sites:?}"
    );
    assert!(
        sites.contains(&("/x/b.sv".to_owned(), 0, 29)),
        "missing .o inner ref: {sites:?}"
    );
}

#[test]
fn port_label_definition_jumps_to_child_port_decl_across_files() {
    let a = cross_file_analysis();
    // `.clk` in `m u0(.clk(c), .o(o));` at 0-based (0, 18) in file B must
    // resolve to module m's `clk` port declaration in file A, not the
    // enclosing module's scope.
    let loc = definition_at(&a, "/x/b.sv", 0, 18).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 23)); // clk port decl in file A
}

#[test]
fn port_label_hover_shows_child_port() {
    let a = cross_file_analysis();
    // `.o` label at 0-based (0, 27) in file B.
    let hover = hover_at(&a, "/x/b.sv", 0, 27).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    assert!(value.contains("o"), "value: {value}");
}

#[test]
fn references_on_port_decl_include_instantiation_labels() {
    let a = cross_file_analysis();
    // References of the `o` port decl in file A (0, 46) include the
    // instantiation-site `.o` label in file B.
    let refs = references_at(&a, "/x/a.sv", 0, 46);
    assert!(
        refs.iter().any(|l| {
            l.uri == Url::from_file_path("/x/b.sv").unwrap()
                && l.range.start == Position::new(0, 27)
        }),
        "missing .o label site: {refs:?}"
    );
}

#[test]
fn port_label_synthesizes_missing_port_decl() {
    // File A declares module `m` but its token stream has no port-decl
    // tokens; file B instantiates it with a named connection.  The port
    // declaration is synthesized at the module header so goto-definition
    // still lands in the def file.
    let node = |line: u32, col: u32, t: i32, name: &str, file: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: file.to_owned(),
    };
    let a_file = FileTokens {
        path: "/x/a.sv".to_owned(),
        nodes: vec![node(
            1,
            8,
            tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
            "m",
            "/x/a.sv",
        )],
    };
    let b_file = FileTokens {
        path: "/x/b.sv".to_owned(),
        nodes: vec![
            node(1, 13, tokens::TOKEN_SLANG_MODULE, "m", "/x/b.sv"),
            node(
                1,
                15,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
                "/x/b.sv",
            ),
            node(
                1,
                19,
                tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL,
                "clk",
                "/x/b.sv",
            ),
        ],
    };
    let module_m = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/a.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 1,
        end_col: 10,
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 15,
        ports: vec![PortModel {
            name: "clk".to_owned(),
            direction: Direction::Input,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        }],
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: Vec::new(),
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: vec![module_m],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let a = Analysis::new(Vec::new(), model, vec![a_file, b_file], Vec::new());
    let loc = definition_at(&a, "/x/b.sv", 0, 18).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    // module m at 1-based col 8 (0-based 7) + name len 1 + port index 0.
    assert_eq!(loc.range.start, Position::new(0, 8));
}

#[test]
fn index_is_built_by_analyze() {
    let a = empty_analysis();
    assert!(a.index.decls.is_empty());
    assert!(a.index.refs.is_empty());
    assert!(a.index.entry_at("/nope.sv", 0, 0).is_none());
    assert!(a.index.decls_in_file("/nope.sv").is_empty());
}

/// Real compile of `tests/elaboration/top3.sv`: line 34 instantiates
/// `hier_ref u_hier (.clk(clk), .o(o));` inside module `tb`, which itself
/// declares ports named `clk`/`o`.  The labels must resolve to the CHILD
/// module's ports (hier_ref), not the enclosing module's same-named ones;
/// the inner expression refs must keep resolving to the enclosing scope.
#[test]
fn analyze_full_pipeline_named_ports_resolve_to_child() {
    let _guards = analysis_guards();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elaboration/top3.sv");
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
    // `.clk` label at 0-based (33, 22) → hier_ref's clk port decl
    // (20, 16), NOT tb's clk port decl (29, 16).
    let loc = definition_at(&a, &path_str, 33, 22).expect("definition of .clk label");
    assert_eq!(loc.range.start, Position::new(20, 16), "loc: {loc:?}");
    // `.o` label at (33, 33) → hier_ref's o port decl (21, 23).
    let loc = definition_at(&a, &path_str, 33, 33).expect("definition of .o label");
    assert_eq!(loc.range.start, Position::new(21, 23), "loc: {loc:?}");
    // The inner `clk` ref (the connection ACTUAL) resolves to the
    // ACTUAL signal's own declaration in the instantiating scope —
    // tb's clk port decl (29, 16), NOT the child module's same-named
    // port, even though both modules declare `clk`.
    let loc = definition_at(&a, &path_str, 33, 26).expect("definition of inner clk ref");
    assert_eq!(loc.range.start, Position::new(29, 16), "loc: {loc:?}");
    assert_ne!(
        loc.range.start,
        Position::new(20, 16),
        "the actual must not jump into the child module"
    );
    // The label and the actual of the SAME connection resolve to
    // DIFFERENT declarations (child port vs parent-scope decl).
    let label_loc = definition_at(&a, &path_str, 33, 22).expect("label location");
    assert_ne!(label_loc.range.start, loc.range.start);
    // Hover on the `.o` label shows the child port.
    let hover = hover_at(&a, &path_str, 33, 33).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    // References of hier_ref's `o` port decl include the `.o` label site.
    let refs = references_at(&a, &path_str, 21, 23);
    assert!(
        refs.iter().any(|l| l.range.start == Position::new(33, 33)),
        "missing .o label site: {refs:?}"
    );
}

/// Full compile of a two-file design whose instantiation is spread over
/// several lines: `m u0(\n  .clk(clk),\n  .o(o)\n);` inside module `top`,
/// which itself declares signals named `clk`/`o`.  The continuation-line
/// labels must resolve to the CHILD module's ports in a.sv, not the
/// enclosing module's same-named signals.
#[test]
fn analyze_full_pipeline_multiline_named_ports_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_mlport_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let a_sv = dir.join("a.sv");
    let b_sv = dir.join("b.sv");
    std::fs::write(
        &a_sv,
        "module m(\n  input logic clk,\n  output logic [3:0] o\n);\n  assign o = clk;\nendmodule\n",
    )
    .expect("write a.sv");
    std::fs::write(
        &b_sv,
        "module top;\n  logic clk;\n  logic [3:0] o;\n  m u0(\n    .clk(clk),\n    .o(o)\n  );\nendmodule\n",
    )
    .expect("write b.sv");
    let opts = CompileOpts {
        files: vec![
            a_sv.to_string_lossy().into_owned(),
            b_sv.to_string_lossy().into_owned(),
        ],
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
    let b_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("b.sv"))
        .expect("tokens for b.sv")
        .path
        .clone();
    let ft = file_tokens(&a, &b_path).expect("b.sv tokens");
    // The port-connection labels are the only tokens in b.sv carrying the
    // classifier's connection-label synthetic type; find them by name and
    // continuation line rather than hard-coding positions.
    let label = |name: &str| -> TokenInfo {
        ft.nodes
            .iter()
            .find(|n| {
                n.kind == tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL
                    && n.name.as_deref() == Some(name)
                    && n.line > 2
            })
            .expect("label token")
            .clone()
    };
    let clk_label = label("clk");
    let o_label = label("o");
    assert!(
        o_label.line > clk_label.line,
        "labels must be on consecutive continuation lines: {clk_label:?} {o_label:?}"
    );
    let clk_pos = (clk_label.line - 1, clk_label.col - 1);
    let o_pos = (o_label.line - 1, o_label.col - 1);
    // Both continuation-line labels are registered as port labels.
    assert!(
        a.index
            .port_labels
            .contains_key(&(b_path.clone(), clk_pos.0, clk_pos.1)),
        "port_labels: {:?}",
        a.index.port_labels
    );
    assert!(
        a.index
            .port_labels
            .contains_key(&(b_path.clone(), o_pos.0, o_pos.1)),
        "port_labels: {:?}",
        a.index.port_labels
    );
    // `.clk` → m's clk port decl in a.sv (1-based (2,15) → 0-based
    // (1,14)), NOT top's `logic clk` (0-based (1,8)).
    let loc = definition_at(&a, &b_path, clk_pos.0, clk_pos.1).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 14), "loc: {loc:?}");
    // `.o` → m's o port decl in a.sv (1-based (3,22) → 0-based (2,21)).
    let loc = definition_at(&a, &b_path, o_pos.0, o_pos.1).expect("definition of .o label");
    assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 21), "loc: {loc:?}");
    // Hover on the `.o` label shows the child port.
    let hover = hover_at(&a, &b_path, o_pos.0, o_pos.1).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    assert!(value.contains("o"), "value: {value}");
}

/// 0-based `(line, col)` of the `occurrence`-th (0-based) `needle` in
/// `text` — the same convention as the stdio suite's `position_at`.
fn pos_of(text: &str, needle: &str, occurrence: usize) -> (u32, u32) {
    let mut start = 0;
    for _ in 0..=occurrence {
        let found = text[start..]
            .find(needle)
            .unwrap_or_else(|| panic!("needle {needle:?} not found"));
        start += found;
    }
    let line = text[..start].matches('\n').count() as u32;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    (line, (start - line_start) as u32)
}

/// Real compile of a design with a named PARAMETER override:
/// `child u0 #(.W(4), .D(W)) (.clk(clk), .q(t_q));` inside module `top`,
/// which declares a DECOY same-name `localparam int W`.  The `.W`/`.D`
/// labels must resolve to the CHILD module's parameter declarations while
/// the RHS reference `W` inside `.D(W)` stays on the decoy localparam in
/// the instantiating scope — never crossing namespaces in either
/// direction.
#[test]
fn analyze_full_pipeline_named_param_overrides_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_param_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let child_sv = dir.join("child.sv");
    let top_sv = dir.join("top.sv");
    let child_text = concat!(
        "module child #(\n",
        "  parameter int W = 8,\n",
        "  parameter int D = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    let top_text = concat!(
        "module top;\n",
        "  localparam int W = 1;\n",
        "  logic clk;\n",
        "  logic [7:0] t_q;\n",
        "  child #(.W(4), .D(W)) u0 (.clk(clk), .q(t_q));\n",
        "endmodule\n",
    );
    std::fs::write(&child_sv, child_text).expect("write child.sv");
    std::fs::write(&top_sv, top_text).expect("write top.sv");
    let opts = CompileOpts {
        files: vec![
            child_sv.to_string_lossy().into_owned(),
            top_sv.to_string_lossy().into_owned(),
        ],
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
    let top_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("top.sv"))
        .expect("tokens for top.sv")
        .path
        .clone();
    // The override labels sit inside the `#(...)` clause, which in valid
    // SV PRECEDES the instance name.
    let (wl, wc) = pos_of(top_text, "#(.W", 0);
    let w_label = (wl, wc + 3);
    let (dl, dc) = pos_of(top_text, ", .D", 0);
    let d_label = (dl, dc + 3);
    assert_eq!(w_label, (4, 11), "sanity: .W label position");
    // The scanned pair must be recorded with Param flavor.
    assert!(
        a.index
            .param_labels
            .contains_key(&(top_path.clone(), w_label.0, w_label.1)),
        "param_labels must contain the .W label: {:?}",
        a.index.param_labels
    );
    // `.W` label → CHILD module's parameter declaration in child.sv
    // (1-based (2,17) → 0-based (1,16)), NOT the decoy localparam.
    let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
    let decoy = (dcl, dcc + 15);
    assert_eq!(decoy, (1, 17), "sanity: decoy position");
    let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    assert_ne!(loc.range.start, Position::new(decoy.0, decoy.1));
    // `.D` label → child's D parameter (0-based (2,16)).
    let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
    // The RHS `W` inside `.D(W)` resolves to the DECOY localparam in the
    // instantiating scope — the exact opposite direction of the label.
    let (rl, rc) = pos_of(top_text, "(W)", 0);
    let rhs = (rl, rc + 1);
    let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
    assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
    assert_eq!(
        loc.range.start,
        Position::new(decoy.0, decoy.1),
        "loc: {loc:?}"
    );
    // Hover on the `.W` label shows the child parameter.
    let hover = hover_at(&a, &top_path, w_label.0, w_label.1).expect("hover on .W label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("parameter"), "value: {value}");
}

/// Multi-line variant of the parameter override navigation: the labels
/// sit on continuation lines below `child u0 #(` and must still resolve
/// to the CHILD module's parameters, while the RHS stays parent-scope.
#[test]
fn analyze_full_pipeline_multiline_named_param_overrides_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_mlparam_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let child_sv = dir.join("child.sv");
    let top_sv = dir.join("top.sv");
    let child_text = concat!(
        "module child #(\n",
        "  parameter int W = 8,\n",
        "  parameter int D = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    let top_text = concat!(
        "module top;\n",
        "  localparam int W = 1;\n",
        "  logic clk;\n",
        "  logic [7:0] t_q;\n",
        "  child #(\n",
        "    .W(4),\n",
        "    .D(W)\n",
        "  ) u0 (\n",
        "    .clk(clk),\n",
        "    .q(t_q)\n",
        "  );\n",
        "endmodule\n",
    );
    std::fs::write(&child_sv, child_text).expect("write child.sv");
    std::fs::write(&top_sv, top_text).expect("write top.sv");
    let opts = CompileOpts {
        files: vec![
            child_sv.to_string_lossy().into_owned(),
            top_sv.to_string_lossy().into_owned(),
        ],
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
    let top_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("top.sv"))
        .expect("tokens for top.sv")
        .path
        .clone();
    let (wl, wc) = pos_of(top_text, ".W(", 0);
    let w_label = (wl, wc + 1);
    let (dl, dc) = pos_of(top_text, ".D(", 0);
    let d_label = (dl, dc + 1);
    assert_eq!(w_label, (5, 5), "sanity: continuation-line .W position");
    // Continuation-line labels are registered as parameter labels.
    assert!(
        a.index
            .param_labels
            .contains_key(&(top_path.clone(), w_label.0, w_label.1)),
        "param_labels: {:?}",
        a.index.param_labels
    );
    assert!(
        a.index
            .param_labels
            .contains_key(&(top_path.clone(), d_label.0, d_label.1)),
        "param_labels: {:?}",
        a.index.param_labels
    );
    let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
    let decoy = (dcl, dcc + 15);
    // `.W` → child's W parameter in child.sv (0-based (1,16)).
    let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    // `.D` → child's D parameter (0-based (2,16)).
    let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
    // RHS `W` stays on the decoy localparam in the instantiating scope.
    let (rl, rc) = pos_of(top_text, "(W)", 0);
    let rhs = (rl, rc + 1);
    let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
    assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
    assert_eq!(
        loc.range.start,
        Position::new(decoy.0, decoy.1),
        "loc: {loc:?}"
    );
}

/// Hand-built analysis proving the UNSOLVABLE-label guard: an override
/// label whose instance's module definition carries no such parameter
/// yields NO definition — not the same-file `localparam W` decoy that
/// name-based resolution would pick.
#[test]
fn unresolved_param_override_label_yields_no_definition() {
    let node = |line: u32, col: u32, t: i32, name: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: "/x/b.sv".to_owned(),
    };
    // Two identical views make (1,6) a genuine `localparam W` DECL entry;
    // the single-view token at (2,14) is the dropped-looking LABEL ref.
    let b_file = FileTokens {
        path: "/x/b.sv".to_owned(),
        nodes: vec![
            node(
                1,
                9,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "top",
            ),
            node(
                2,
                6,
                tokens::TOKEN_SLANG_PARAMETER + tokens::TOKEN_DECLARATION_OFFSET,
                "W",
            ),
            node(
                3,
                10,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
            ),
            node(3, 15, tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL, "W"),
        ],
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 3,
        col: 10,
        ports: Vec::new(),
        signals: Vec::new(),
        // The override target exists on the instance (so the label token
        // passes the signal-name gate and is indexed as a REF), but the
        // DEFINITION module `m` is absent from the model below — the
        // override can never resolve to a declaration.
        params: vec![ParamModel {
            name: "W".to_owned(),
            value: None,
            ty: TypeInfo {
                kind: "int".to_owned(),
                width: None,
                signed: true,
                type_name: None,
            },
            local: false,
        }],
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: Vec::new(),
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: Vec::new(), // def module unknown → resolution impossible
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let pairs = vec![NamedPortConn {
        file: "/x/b.sv".to_owned(),
        label: (3, 15),
        label_name: "W".to_owned(),
        actual: None,
        actual_name: None,
        inst_type: Some("m".to_owned()),
        kind: ConnKind::Param,
    }];
    let a = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        vec![b_file],
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_decls: None,
            pairs,
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        },
    );
    // Sanity: the label is indexed as a REF and known-unresolved.
    let entry = a.index.entry_at("/x/b.sv", 2, 14).expect("label entry");
    assert!(!entry.is_decl);
    assert!(
        a.index.is_unresolved_param_label("/x/b.sv", 2, 14),
        "label must be recorded unresolved"
    );
    // NO definition — especially not the decoy localparam at (1,5).
    assert!(
        definition_at(&a, "/x/b.sv", 2, 14).is_none(),
        "unresolvable override label must yield no result"
    );
}

/// Hand-built analysis proving [`resolve_param_label`] synthesis: when the
/// index has no parameter declaration tokens for the child module, the
/// override label still binds to a synthesized decl anchored at the
/// module header, kept disjoint from synthesized port anchors.
#[test]
fn resolved_param_override_synthesizes_missing_child_decl() {
    let node = |line: u32, col: u32, t: i32, name: &str, file: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: file.to_owned(),
    };
    let a_file = FileTokens {
        path: "/x/a.sv".to_owned(),
        nodes: vec![node(
            1,
            8,
            tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
            "m",
            "/x/a.sv",
        )],
    };
    let b_file = FileTokens {
        path: "/x/b.sv".to_owned(),
        nodes: vec![
            node(1, 13, tokens::TOKEN_SLANG_MODULE, "m", "/x/b.sv"),
            node(
                1,
                15,
                tokens::TOKEN_SLANG_IDENTIFIER + tokens::TOKEN_DECLARATION_OFFSET,
                "u0",
                "/x/b.sv",
            ),
            node(
                1,
                20,
                tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL,
                "W",
                "/x/b.sv",
            ),
        ],
    };
    let module_m = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/a.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 1,
        end_col: 10,
    };
    let u0 = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 15,
        ports: Vec::new(),
        signals: Vec::new(),
        params: vec![ParamModel {
            name: "W".to_owned(),
            value: None,
            ty: TypeInfo {
                kind: "int".to_owned(),
                width: None,
                signed: true,
                type_name: None,
            },
            local: false,
        }],
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    };
    let top = InstanceModel {
        name: "top".to_owned(),
        def_name: "top".to_owned(),
        full_name: "top".to_owned(),
        file: Some("/x/b.sv".to_owned()),
        line: 1,
        col: 1,
        ports: Vec::new(),
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: vec![u0],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![top],
        modules: vec![module_m],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let pairs = vec![NamedPortConn {
        file: "/x/b.sv".to_owned(),
        label: (1, 20),
        label_name: "W".to_owned(),
        actual: None,
        actual_name: None,
        inst_type: Some("m".to_owned()),
        kind: ConnKind::Param,
    }];
    let a = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        vec![a_file, b_file],
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_decls: None,
            pairs,
            fallback_bindings: HashMap::new(),
            ..ConnectionInputs::default()
        },
    );
    let loc = definition_at(&a, "/x/b.sv", 0, 19).expect("definition of .W label");
    assert_eq!(loc.uri, Url::from_file_path("/x/a.sv").unwrap());
    // Synthesized anchor: header col 7 + name len 1 + stride 64 + idx 0.
    assert_eq!(loc.range.start, Position::new(0, 72), "loc: {loc:?}");
    let bound = a
        .ref_bindings
        .get(&("/x/b.sv".to_owned(), 0, 19))
        .expect("label binding folded into ref_bindings");
    assert_eq!(bound.kind, "parameter");
    assert!(bound.via_label);
}

/// Hand-built two-file `Analysis` proving package-item resolution:
///
/// * `/x/p.sv`: `package my_pkg; parameter int P = 3; typedef enum logic
///   [1:0] { IDLE, RUN } state_t; endpackage` — package decl at 0-based
///   (0,8), `P` param decl at (1,16), `IDLE`/`RUN` enum const decls at
///   (2,29)/(2,35).
/// * `/x/u.sv`: `module top` with reference tokens in both spellings the
///   index must handle: full `my_pkg::P` / `my_pkg::IDLE` at (2,15)/
///   (3,15), bare `P` / `IDLE` at (4,15)/(5,15), and a bare `my_pkg` at
///   (6,15).
///
/// Token roles mirror the Slang snapshot: package, parameter, and enum member
/// declarations carry the declaration offset, while uses are identifiers.
fn package_item_analysis() -> Analysis {
    let node = |line: u32, col: u32, t: i32, name: &str| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + name.len() as u32,
        kind: t,
        name: Some(name.to_owned()),
        file: String::new(), // filled below
    };
    let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
        FileTokens {
            path: path.to_owned(),
            nodes: nodes
                .into_iter()
                .map(|(l, c, t, n)| {
                    let mut v = node(l, c, t, n);
                    v.file = path.to_owned();
                    v
                })
                .collect(),
        }
    };

    let p_file = mk(
        vec![
            (
                1,
                9,
                tokens::TOKEN_SLANG_PACKAGE + tokens::TOKEN_DECLARATION_OFFSET,
                "my_pkg",
            ),
            (
                2,
                17,
                tokens::TOKEN_SLANG_PARAMETER + tokens::TOKEN_DECLARATION_OFFSET,
                "P",
            ),
            (
                3,
                30,
                tokens::TOKEN_SLANG_ENUM_MEMBER + tokens::TOKEN_DECLARATION_OFFSET,
                "IDLE",
            ),
            (
                3,
                36,
                tokens::TOKEN_SLANG_ENUM_MEMBER + tokens::TOKEN_DECLARATION_OFFSET,
                "RUN",
            ),
        ],
        "/x/p.sv",
    );
    let u_file = mk(
        vec![
            (
                1,
                8,
                tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                "top",
            ),
            (3, 16, tokens::TOKEN_SLANG_IDENTIFIER, "my_pkg::P"),
            (4, 16, tokens::TOKEN_SLANG_IDENTIFIER, "my_pkg::IDLE"),
            (5, 16, tokens::TOKEN_SLANG_IDENTIFIER, "P"),
            (6, 16, tokens::TOKEN_SLANG_IDENTIFIER, "IDLE"),
            (7, 16, tokens::TOKEN_SLANG_IDENTIFIER, "my_pkg"),
        ],
        "/x/u.sv",
    );

    let int_ty = || TypeInfo {
        kind: "int".to_owned(),
        width: None,
        signed: true,
        type_name: None,
    };
    let int_val = |v: u64| Val::Bits(Value::from_u64(v, 32, true));
    let enum_val = |v: u64| Val::Bits(Value::from_u64(v, 2, true));
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some("/x/u.sv".to_owned()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: Vec::new(),
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        }],
        modules: vec![ModuleDef {
            name: "top".to_owned(),
            file: Some("/x/u.sv".to_owned()),
            line: 1,
            col: 8,
            end_line: 1,
            end_col: 11,
        }],
        packages: vec![PackageDef {
            name: "my_pkg".to_owned(),
            file: Some("/x/p.sv".to_owned()),
            line: 1,
            col: 1,
            params: vec![ParamModel {
                name: "P".to_owned(),
                value: Some(int_val(3)),
                ty: int_ty(),
                local: false,
            }],
            enum_consts: vec![
                EnumConstDef {
                    name: "IDLE".to_owned(),
                    value: Some(enum_val(0)),
                    file: Some("/x/p.sv".to_owned()),
                    line: 3,
                    col: 30,
                },
                EnumConstDef {
                    name: "RUN".to_owned(),
                    value: Some(enum_val(1)),
                    file: Some("/x/p.sv".to_owned()),
                    line: 3,
                    col: 36,
                },
            ],
        }],
        classes: Vec::new(),
    };
    Analysis::new(Vec::new(), model, vec![p_file, u_file], Vec::new())
}

#[test]
fn package_item_definition_resolves_full_and_bare_spellings() {
    let a = package_item_analysis();
    // `my_pkg::P` (0-based (2,15) in u.sv) → P decl at (1,16) in p.sv.
    let loc = definition_at(&a, "/x/u.sv", 2, 15).expect("definition of my_pkg::P ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    // `my_pkg::IDLE` (0-based (3,15)) → IDLE decl at (2,29) in p.sv.
    let loc = definition_at(&a, "/x/u.sv", 3, 15).expect("definition of my_pkg::IDLE ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(2, 29), "loc: {loc:?}");
    // Bare `P` (0-based (4,15)) falls back to the workspace-wide same-name
    // declaration: the package param.
    let loc = definition_at(&a, "/x/u.sv", 4, 15).expect("definition of bare P ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    // Bare `IDLE` (0-based (5,15)) likewise resolves to the package enum
    // const.
    let loc = definition_at(&a, "/x/u.sv", 5, 15).expect("definition of bare IDLE ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(2, 29), "loc: {loc:?}");
    // `my_pkg` alone (0-based (6,15)) → the package declaration.
    let loc = definition_at(&a, "/x/u.sv", 6, 15).expect("definition of my_pkg ref");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(0, 8), "loc: {loc:?}");
}

#[test]
fn parse_enum_binding_uses_member_range_and_rejects_ambiguous_target() {
    let base = package_item_analysis();
    let target = ParseEnumDecl {
        name: "IDLE".to_owned(),
        file: "/x/p.sv".to_owned(),
        line1: 3,
        col1: 30,
        scope: Some("my_pkg".to_owned()),
    };
    let use_key = ("/x/u.sv".to_owned(), 3, 23);
    let mut bindings = HashMap::new();
    bindings.insert(
        use_key.clone(),
        DeclTarget {
            name: "IDLE".to_owned(),
            kind: "enum constant".to_owned(),
            file: "/x/p.sv".to_owned(),
            line0: 2,
            col0: 29,
            via_label: false,
            via_connection: false,
        },
    );
    let a = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        base.model,
        base.tokens,
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_enum_decls: vec![target.clone()],
            parse_enum_bindings: bindings,
            parse_enum_ref_positions: [use_key.clone()].into_iter().collect(),
            parse_enum_tokens: vec![TokenInfo {
                line: 4,
                col: 24,
                end_line: 4,
                end_col: 28,
                kind: tokens::TOKEN_SLANG_ENUM_MEMBER + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("IDLE".to_owned()),
                file: "/x/u.sv".to_owned(),
            }],
            ..ConnectionInputs::default()
        },
    );
    // The qualified token's package prefix starts at column 15, but the
    // binding key and returned range are anchored to the member at 23.
    let loc = definition_at(&a, "/x/u.sv", 3, 23).expect("enum member binding");
    assert_eq!(loc.uri, Url::from_file_path("/x/p.sv").unwrap());
    assert_eq!(
        loc.range,
        Range::new(Position::new(2, 29), Position::new(2, 33))
    );
    assert!(references_at(&a, "/x/u.sv", 3, 23)
        .iter()
        .any(|location| location.range.start == Position::new(2, 29)));

    let ambiguous = package_item_analysis();
    let second = ParseEnumDecl {
        name: "IDLE".to_owned(),
        file: "/x/other.sv".to_owned(),
        line1: 7,
        col1: 12,
        scope: Some("my_pkg".to_owned()),
    };
    let ambiguous = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        ambiguous.model,
        ambiguous.tokens,
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_enum_decls: vec![target, second],
            unresolved_enum_refs: [use_key.clone()].into_iter().collect(),
            parse_enum_ref_positions: [use_key.clone()].into_iter().collect(),
            parse_enum_tokens: vec![TokenInfo {
                line: 4,
                col: 24,
                end_line: 4,
                end_col: 28,
                kind: tokens::TOKEN_SLANG_ENUM_MEMBER + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("IDLE".to_owned()),
                file: "/x/u.sv".to_owned(),
            }],
            ..ConnectionInputs::default()
        },
    );
    assert!(definition_at(&ambiguous, "/x/u.sv", 3, 23).is_none());
    assert!(references_at(&ambiguous, "/x/u.sv", 3, 23).is_empty());
}

#[test]
fn parse_class_qualified_enum_binding_targets_the_member() {
    let target = ParseEnumDecl {
        name: "READY".to_owned(),
        file: "/x/classes.sv".to_owned(),
        line1: 2,
        col1: 27,
        scope: Some("StateHolder".to_owned()),
    };
    let key = ("/x/use.sv".to_owned(), 3, 23);
    let mut bindings = HashMap::new();
    bindings.insert(
        key.clone(),
        DeclTarget {
            name: "READY".to_owned(),
            kind: "enum constant".to_owned(),
            file: target.file.clone(),
            line0: 1,
            col0: 26,
            via_label: false,
            via_connection: false,
        },
    );
    let a = Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        empty_design(),
        vec![FileTokens {
            path: "/x/use.sv".to_owned(),
            nodes: vec![TokenInfo {
                line: 4,
                col: 11,
                end_line: 4,
                end_col: 28,
                kind: tokens::TOKEN_SLANG_IDENTIFIER,
                name: Some("StateHolder::READY".to_owned()),
                file: "/x/use.sv".to_owned(),
            }],
        }],
        Vec::new(),
        HashMap::new(),
        ConnectionInputs {
            parse_enum_decls: vec![target],
            parse_enum_bindings: bindings,
            parse_enum_ref_positions: [key].into_iter().collect(),
            parse_enum_tokens: vec![TokenInfo {
                line: 4,
                col: 24,
                end_line: 4,
                end_col: 24,
                kind: tokens::TOKEN_SLANG_ENUM_MEMBER + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("READY".to_owned()),
                file: "/x/use.sv".to_owned(),
            }],
            ..ConnectionInputs::default()
        },
    );
    let loc = definition_at(&a, "/x/use.sv", 3, 23).expect("class enum member");
    assert_eq!(loc.uri, Url::from_file_path("/x/classes.sv").unwrap());
    assert_eq!(loc.range.start, Position::new(1, 26));
    assert_eq!(loc.range.end, Position::new(1, 31));
}

#[test]
fn package_item_hover_shows_enum_value() {
    let a = package_item_analysis();
    // IDLE decl at 0-based (2,29) in p.sv.
    let hover = hover_at(&a, "/x/p.sv", 2, 29).expect("hover on IDLE decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("IDLE"), "value: {value}");
    assert!(value.contains("enum const"), "value: {value}");
    assert!(value.contains("0"), "value: {value}");
    // Hover through the use-site ref shows the same package item.
    let hover = hover_at(&a, "/x/u.sv", 3, 15).expect("hover on my_pkg::IDLE ref");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("IDLE"), "value: {value}");
}

#[test]
fn package_item_completion_after_scope_prefix() {
    let a = package_item_analysis();
    // Cursor at the end of `my_pkg::` (8 chars) → the scope branch fires.
    let items = completion_at(&a, "/x/u.sv", 0, 8, "my_pkg::");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"P"), "items: {labels:?}");
    assert!(labels.contains(&"IDLE"), "items: {labels:?}");
    assert!(labels.contains(&"RUN"), "items: {labels:?}");
    // Prefix filtering applies to the item after `::` (cursor at the end
    // of the typed prefix, so col == line length).
    let items = completion_at(&a, "/x/u.sv", 0, 9, "my_pkg::I");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"IDLE"), "items: {labels:?}");
    assert!(!labels.contains(&"P"), "items: {labels:?}");
    // Unknown packages offer nothing.
    let items = completion_at(&a, "/x/u.sv", 0, 6, "nope::");
    assert!(items.is_empty(), "items: {items:?}");
}

#[test]
fn package_document_symbol_stays_flat() {
    let a = package_item_analysis();
    let syms = document_symbols(&a, "/x/p.sv");
    let pkg = syms
        .iter()
        .find(|s| s.name == "my_pkg" && s.kind == SymbolKind::PACKAGE)
        .expect("package symbol");
    // v1: package items are not document children (they surface through
    // completion/hover/goto); the package symbol itself is flat and uses
    // the model declaration position (1-based (1,1) → 0-based (0,0)).
    assert!(pkg.children.is_none(), "children: {:?}", pkg.children);
    assert_eq!(pkg.range.start, Position::new(0, 0));
}

/// Full compile of a two-file design with package items used from a
/// module: `my_pkg::P` in a parameter context, qualified enum members,
/// an imported bare member, and `my_pkg::RUN` in a case. The Slang snapshot
/// preserves the member coordinates and supplies reference bindings through
/// the normal LSP provider path.
#[test]
fn analyze_full_pipeline_package_items() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_pkg_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let p_sv = dir.join("p.sv");
    let u_sv = dir.join("u.sv");
    std::fs::write(
        &p_sv,
        "package my_pkg;\n  parameter int P = 3;\n  typedef enum logic [1:0] { IDLE, RUN } state_t;\nendpackage\n",
    )
    .expect("write p.sv");
    std::fs::write(
        &u_sv,
        "module top;\n  import my_pkg::*;\n  parameter int W = my_pkg::P;\n  logic [1:0] s;\n  logic [1:0] x;\n  always_comb begin\n    s = my_pkg::IDLE;\n    s = IDLE;\n    case (x)\n      my_pkg::IDLE: s = 2'b00;\n      default: s = my_pkg::RUN;\n    endcase\n  end\nendmodule\n",
    )
    .expect("write u.sv");
    let opts = CompileOpts {
        files: vec![
            p_sv.to_string_lossy().into_owned(),
            u_sv.to_string_lossy().into_owned(),
        ],
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

    // The model carries the package items.
    let pkg = a
        .model
        .packages
        .iter()
        .find(|p| clean_name(&p.name) == "my_pkg")
        .expect("my_pkg in model");
    assert_eq!(pkg.params.len(), 1, "params: {:?}", pkg.params);
    let p = &pkg.params[0];
    assert_eq!(p.name, "P");
    assert_eq!(
        p.value.as_ref().and_then(|v| match v {
            Val::Bits(b) => b.to_u64(),
            Val::Str(_) | Val::Real(_) => None,
        }),
        Some(3),
        "P value: {:?}",
        p.value
    );
    let const_names: Vec<&str> = pkg.enum_consts.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        const_names,
        vec!["IDLE", "RUN"],
        "enum consts: {:?}",
        pkg.enum_consts
    );
    let idle = pkg
        .enum_consts
        .iter()
        .find(|e| e.name == "IDLE")
        .expect("IDLE enum const");
    assert_eq!(
        idle.value.as_ref().and_then(|v| match v {
            Val::Bits(b) => b.to_u64(),
            Val::Str(_) | Val::Real(_) => None,
        }),
        Some(0)
    );

    // The index carries the item declarations with their package scope.
    let p_decl = a
        .index
        .decls
        .iter()
        .find(|d| d.name == "P" && d.kind == SymKind::Param && d.scope.as_deref() == Some("my_pkg"))
        .expect("P decl in index");
    assert_eq!(p_decl.file, p_sv.to_string_lossy());
    // Package parameters are non-overridable and Slang represents them as
    // local parameters even when the source uses the `parameter` keyword.
    assert_eq!(p_decl.detail.as_deref(), Some("localparam P: int = 32'sd3"));
    let idle_decl = a
        .index
        .decls
        .iter()
        .find(|d| {
            d.name == "IDLE" && d.kind == SymKind::EnumConst && d.scope.as_deref() == Some("my_pkg")
        })
        .expect("IDLE decl in index");
    assert!(idle_decl.detail.as_deref().unwrap_or("").contains("IDLE"));

    // Definition on the decl positions resolves in place (cross-file from
    // the package file's perspective is trivially the same file).
    let loc = definition_at(&a, &p_sv.to_string_lossy(), p_decl.line, p_decl.col)
        .expect("definition of P decl");
    assert_eq!(loc.range.start, Position::new(p_decl.line, p_decl.col));
    let loc = definition_at(&a, &p_sv.to_string_lossy(), idle_decl.line, idle_decl.col)
        .expect("definition of IDLE decl");
    assert_eq!(
        loc.range.start,
        Position::new(idle_decl.line, idle_decl.col)
    );

    // Hover on the enum const decl shows its name/value.
    let hover = hover_at(&a, &p_sv.to_string_lossy(), idle_decl.line, idle_decl.col)
        .expect("hover on IDLE decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("IDLE"), "value: {value}");

    let source = std::fs::read_to_string(&u_sv).expect("read u.sv");
    let member_position = |line0: u32| {
        let line = source.lines().nth(line0 as usize).expect("source line");
        let byte_col = line.find("IDLE").expect("IDLE member");
        (line0, line[..byte_col].encode_utf16().count() as u32)
    };
    for line0 in [6, 9] {
        let (use_line, use_col) = member_position(line0);
        let loc = definition_at(&a, &u_sv.to_string_lossy(), use_line, use_col)
            .expect("qualified enum member definition");
        assert_eq!(
            loc.range.start,
            Position::new(idle_decl.line, idle_decl.col),
            "qualified use at {use_line}:{use_col}"
        );
    }
    let (bare_line, bare_col) = member_position(7);
    let loc = definition_at(&a, &u_sv.to_string_lossy(), bare_line, bare_col)
        .expect("imported bare enum member definition");
    assert_eq!(
        loc.range.start,
        Position::new(idle_decl.line, idle_decl.col)
    );

    // Completion after `my_pkg::` offers the package items.
    let items = completion_at(&a, &u_sv.to_string_lossy(), 0, 8, "my_pkg::");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"P"), "items: {labels:?}");
    assert!(labels.contains(&"IDLE"), "items: {labels:?}");
    assert!(labels.contains(&"RUN"), "items: {labels:?}");
}

/// Hand-built `Analysis` for /x/c.sv:
///
/// ```text
/// class Counter;                  ← class name at 1-based (1,7)
///   int count;                    ← field at 1-based (2,7)
///   function new();               ← constructor at 1-based (3,3)
///     count = 0;
///   endfunction
///   function int get();           ← get at 1-based (6,3)
///     get = count;
///   endfunction
/// endclass
/// ```
///
/// Model positions mirror the real pipeline: the class points at the
/// `class` keyword, methods at the `function` keyword, and the field at its
/// identifier. Tokens use Slang's explicit declaration kinds.
fn class_analysis() -> Analysis {
    let int_ty = || TypeInfo {
        kind: "int".to_owned(),
        width: None,
        signed: true,
        type_name: None,
    };
    let counter = ClassDef {
        name: "Counter".to_owned(),
        file: Some("/x/c.sv".to_owned()),
        line: 1,
        col: 1,
        methods: vec![
            FuncDef {
                name: "new".to_owned(),
                is_task: false,
                automatic: false,
                file: Some("/x/c.sv".to_owned()),
                line: 3,
                col: 3,
                ret: None,
                args: Vec::new(),
                dpi_import: None,
                scope: "Counter".to_owned(),
            },
            FuncDef {
                name: "get".to_owned(),
                is_task: false,
                automatic: false,
                file: Some("/x/c.sv".to_owned()),
                line: 6,
                col: 3,
                ret: Some(int_ty()),
                args: Vec::new(),
                dpi_import: None,
                scope: "Counter".to_owned(),
            },
        ],
        fields: vec![ClassFieldDef {
            name: "count".to_owned(),
            ty: int_ty(),
            line: 2,
            col: 7,
        }],
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: Vec::new(),
        modules: Vec::new(),
        packages: Vec::new(),
        classes: vec![counter],
    };
    let tokens = vec![FileTokens {
        path: "/x/c.sv".to_owned(),
        nodes: vec![
            TokenInfo {
                line: 1,
                col: 7,
                end_line: 1,
                end_col: 14,
                kind: tokens::TOKEN_SLANG_CLASS + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("Counter".to_owned()),
                file: "/x/c.sv".to_owned(),
            },
            TokenInfo {
                line: 2,
                col: 7,
                end_line: 2,
                end_col: 12,
                kind: tokens::TOKEN_SLANG_VARIABLE + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("count".to_owned()),
                file: "/x/c.sv".to_owned(),
            },
            TokenInfo {
                line: 3,
                col: 13,
                end_line: 3,
                end_col: 16,
                kind: tokens::TOKEN_SLANG_METHOD + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("new".to_owned()),
                file: "/x/c.sv".to_owned(),
            },
            TokenInfo {
                line: 6,
                col: 16,
                end_line: 6,
                end_col: 19,
                kind: tokens::TOKEN_SLANG_METHOD + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("get".to_owned()),
                file: "/x/c.sv".to_owned(),
            },
        ],
    }];
    Analysis::new(Vec::new(), model, tokens, Vec::new())
}

#[test]
fn hover_on_class_name_shows_members() {
    let a = class_analysis();
    // `Counter` at 1-based (1,7) → 0-based (0,6).
    let hover = hover_at(&a, "/x/c.sv", 0, 6).expect("hover on class name");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("class Counter"), "value: {value}");
    assert!(value.contains("get"), "method list missing: {value}");
    assert!(value.contains("count"), "field list missing: {value}");
}

#[test]
fn hover_on_class_method_shows_signature() {
    let a = class_analysis();
    // `function int get()` at 1-based (6,3) → 0-based (5,2).
    let hover = hover_at(&a, "/x/c.sv", 5, 2).expect("hover on get decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("function int get()"), "value: {value}");
    // Hover on the method *name* (0-based (5,15)) falls back to the
    // parse-tree token and still shows the signature.
    let hover = hover_at(&a, "/x/c.sv", 5, 15).expect("hover on get name");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("function int get()"), "value: {value}");
}

#[test]
fn document_symbols_include_class_with_members() {
    let a = class_analysis();
    let syms = document_symbols(&a, "/x/c.sv");
    let cls = syms
        .iter()
        .find(|s| s.name == "Counter")
        .expect("class symbol");
    assert_eq!(cls.kind, SymbolKind::CLASS);
    let children = cls.children.as_ref().expect("class children");
    let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"new"), "children: {names:?}");
    assert!(names.contains(&"get"), "children: {names:?}");
    assert!(names.contains(&"count"), "children: {names:?}");
    let get = children
        .iter()
        .find(|c| c.name == "get")
        .expect("get child");
    assert_eq!(
        get.detail.as_deref(),
        Some("function int get()"),
        "child detail missing: {get:?}"
    );
}

#[test]
fn completion_after_class_scope_prefix_offers_members() {
    let a = class_analysis();
    let items = completion_at(&a, "/x/c.sv", 0, 9, "Counter::");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"new"), "items: {labels:?}");
    assert!(labels.contains(&"get"), "items: {labels:?}");
    assert!(labels.contains(&"count"), "items: {labels:?}");
    assert!(
        items
            .iter()
            .any(|i| i.label == "get" && i.kind == Some(CompletionItemKind::FUNCTION)),
        "get must be a function: {items:?}"
    );
    assert!(
        items
            .iter()
            .any(|i| i.label == "count" && i.kind == Some(CompletionItemKind::VARIABLE)),
        "count must be a variable: {items:?}"
    );
    // Prefix filtering applies after `::`.
    let items = completion_at(&a, "/x/c.sv", 0, 10, "Counter::g");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"get"), "items: {labels:?}");
    assert!(!labels.contains(&"count"), "items: {labels:?}");
    // Unknown classes offer nothing.
    let items = completion_at(&a, "/x/c.sv", 0, 8, "Nope::");
    assert!(items.is_empty(), "items: {items:?}");
}

/// Full compile of a design with a class declaration used from a module:
/// the model and index must pick up the class, its methods (`new`/`get`)
/// and its field (`count`), and the LSP features must surface them.
#[test]
fn analyze_full_pipeline_classes() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_class_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let sv = dir.join("c.sv");
    std::fs::write(
        &sv,
        "class Counter;\n  int count;\n  function new();\n    count = 0;\n  endfunction\n  function int get();\n    get = count;\n  endfunction\nendclass\nmodule top;\n  Counter c;\nendmodule\n",
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

    // Model: the class carries methods (constructor first) and fields.
    let cls = a
        .model
        .classes
        .iter()
        .find(|c| clean_name(&c.name) == "Counter")
        .expect("Counter class in model");
    let source_method_names: Vec<&str> = cls
        .methods
        .iter()
        .filter(|method| method.file.is_some())
        .map(|method| method.name.as_str())
        .collect();
    assert_eq!(
        source_method_names,
        vec!["new", "get"],
        "methods: {:?}",
        cls.methods
    );
    assert!(
        cls.methods
            .iter()
            .any(|method| method.name == "randomize" && method.file.is_none()),
        "Slang built-in class methods remain available: {:?}",
        cls.methods
    );
    let get = cls
        .methods
        .iter()
        .find(|m| m.name == "get")
        .expect("get method");
    let get_ret = get.ret.as_ref().expect("get return type");
    assert_eq!(get_ret.type_name, None);
    assert_eq!(get_ret.kind, "int");
    assert_eq!(get_ret.width, Some(32));
    assert!(get_ret.signed);
    assert_eq!(get.scope, "Counter", "method scope: {get:?}");
    let new = cls.methods.iter().find(|m| m.name == "new").expect("ctor");
    assert_eq!(new.ret, None, "constructor has no source return type");
    let field_names: Vec<&str> = cls.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(field_names, vec!["count"], "fields: {:?}", cls.fields);

    // Index: class/method/field decls with the class scope.
    let class_decl = a
        .index
        .decls
        .iter()
        .find(|d| d.name == "Counter" && d.kind == SymKind::Class)
        .expect("class decl");
    assert_eq!(class_decl.file, path);
    let get_decl = a
        .index
        .decls
        .iter()
        .find(|d| {
            d.name == "get" && d.kind == SymKind::Function && d.scope.as_deref() == Some("Counter")
        })
        .expect("get method decl");
    let field_decl = a
        .index
        .decls
        .iter()
        .find(|d| {
            d.name == "count" && d.kind == SymKind::Var && d.scope.as_deref() == Some("Counter")
        })
        .expect("count field decl");
    assert_eq!(
        get_decl.detail.as_deref(),
        Some("function int get()"),
        "get detail: {get_decl:?}"
    );
    assert_eq!(
        field_decl.detail.as_deref(),
        Some("int count"),
        "count detail: {field_decl:?}"
    );

    // Hover on the class decl shows the members; on the method, the
    // signature.
    let hover =
        hover_at(&a, &path, class_decl.line, class_decl.col).expect("hover on Counter decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("class Counter"), "value: {value}");
    assert!(value.contains("get"), "value: {value}");
    let hover = hover_at(&a, &path, get_decl.line, get_decl.col).expect("hover on get decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("function int get()"), "value: {value}");

    // Document symbols: Counter with new/get/count children.
    let syms = document_symbols(&a, &path);
    let cls_sym = syms
        .iter()
        .find(|s| s.name == "Counter")
        .expect("Counter symbol");
    assert_eq!(cls_sym.kind, SymbolKind::CLASS);
    let children = cls_sym.children.as_ref().expect("class children");
    let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    for want in ["new", "get", "count"] {
        assert!(names.contains(&want), "children: {names:?}");
    }

    // Completion after `Counter::` offers methods and fields.
    let items = completion_at(&a, &path, 0, 9, "Counter::");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for want in ["new", "get", "count"] {
        assert!(labels.contains(&want), "items: {labels:?}");
    }
}

#[test]
fn shadow_path_round_trips_absolute_paths() {
    let base = process_shadow_base();
    for real in [
        "/repo/rtl/top.sv",
        "/tmp/proj/sub dir/top.sv",
        "/a/b/c/d.sv",
        "/workspaces/llg/src/bin/llg/features.rs",
    ] {
        let shadow = shadow_path(Path::new(real), &base);
        assert!(
            shadow.starts_with(&base),
            "shadow not under the tree: {shadow:?}"
        );
        assert_eq!(
            real_path(&shadow, &base),
            Some(PathBuf::from(real)),
            "round-trip failed for {real}"
        );
    }
}

#[test]
fn real_path_rejects_paths_outside_shadow_tree() {
    let base = process_shadow_base();
    assert_eq!(real_path(Path::new("/repo/rtl/top.sv"), &base), None);
    assert_eq!(real_path(Path::new("/other/x.sv"), &base), None);
    // The shadow tree root itself has no real path.
    assert_eq!(real_path(&base, &base), None);
}

/// Full compile of a design staged at its deterministic shadow path: the
/// analysis must be keyed by the shadow path (model, tokens, lint).
#[test]
fn analyze_full_pipeline_compiles_shadow_path() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_shadow_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");

    // Stage an unsaved buffer: write the *shadow* copy only; the real file
    // exists on disk too (as in a workspace) but the compile must read the
    // shadow copy.
    let real = dir.join("rtl").join("top.sv");
    std::fs::create_dir_all(real.parent().expect("parent dir")).expect("create rtl dir");
    std::fs::write(&real, "module top; endmodule\n").expect("write real file");
    let shadow = shadow_path(&real, &dir);
    std::fs::create_dir_all(shadow.parent().expect("shadow parent")).expect("create shadow dir");
    std::fs::write(&shadow, "module top; logic unused_sig; endmodule\n")
        .expect("write shadow file");

    let shadow_str = shadow.to_string_lossy().into_owned();
    let opts = CompileOpts {
        files: vec![shadow_str.clone()],
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
    assert!(
        a.model
            .modules
            .iter()
            .any(|m| m.file.as_deref() == Some(shadow_str.as_str())),
        "module files: {:?}",
        a.model
            .modules
            .iter()
            .map(|m| m.file.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        a.tokens.iter().any(|ft| ft.path == shadow_str),
        "token files: {:?}",
        a.tokens
            .iter()
            .map(|ft| ft.path.clone())
            .collect::<Vec<_>>()
    );
    // The lint finding (unused signal) is keyed by the shadow path too.
    let map = lsp_diagnostics(&a);
    let diags = map
        .iter()
        .find(|(f, _)| *f == &shadow_str)
        .map(|(_, v)| v)
        .expect("diagnostics for the shadow path");
    assert!(
        diags.iter().any(|d| {
            d.source.as_deref() == Some("llg-lint") && d.message.contains("unused_sig")
        }),
        "unused-signal lint missing: {diags:?}"
    );
}
