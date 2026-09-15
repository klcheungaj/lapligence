//! Packages.

use super::*;

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
