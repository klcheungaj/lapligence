//! Cross file.

use super::*;

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
pub(super) fn cross_file_analysis() -> Analysis {
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
pub(super) fn multiline_port_analysis() -> Analysis {
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
