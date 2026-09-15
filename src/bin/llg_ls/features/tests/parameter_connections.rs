//! Parameter connections.

use super::*;

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
