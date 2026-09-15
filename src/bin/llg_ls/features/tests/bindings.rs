//! Bindings.

use super::*;

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
