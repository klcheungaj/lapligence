//! Document symbols.

use super::*;

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
