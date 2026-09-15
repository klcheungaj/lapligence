//! Classes.

use super::*;

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
