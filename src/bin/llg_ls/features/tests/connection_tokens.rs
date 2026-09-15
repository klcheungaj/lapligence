//! Connection tokens.

use super::*;

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
