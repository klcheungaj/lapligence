//! Hover.

use super::*;

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
