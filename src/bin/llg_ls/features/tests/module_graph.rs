//! Module graph.

use super::*;

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
