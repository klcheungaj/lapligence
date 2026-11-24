//! Hover.

use super::*;

#[test]
fn hover_on_port_shows_direction_and_type() {
    let a = sample_analysis();
    let hover = hover_at(&a, hp("/x/top.sv"), 0, 4).expect("hover on port");
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
    let hover = hover_at(&a, hp("/x/top.sv"), 1, 4).expect("hover on param");
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
    assert!(hover_at(&a, hp("/x/top.sv"), 9, 9).is_none());
}

const MACROS: &str = concat!(
    "`define WIDTH 8\n",
    "`define MAX(a, b) ((a) > (b)) ? (a) : (b)\n",
    "`define ENABLE\n",
    "module m;\n",
    "  int x = `WIDTH;\n",
    "  int y = `MISSING;\n",
    "  int z = `MAX(x, y);\n",
    "`ifdef ENABLE\n",
    "`endif\n",
    "`ifdef ABSENT\n",
    "`endif\n",
    "  string f = `__FILE__;\n",
    "  int c = `CONFIGURED;\n",
    "endmodule\n",
);

/// [`MACROS`] compiled with `CONFIGURED=3`. The caller holds
/// [`analysis_guards`].
fn macro_analysis() -> Analysis {
    let mut a = analyze(&CompileOpts {
        sources: vec![compile::OwnedSource::compilation_unit(
            hp("/x/top.sv"),
            MACROS,
        )],
        defines: vec!["CONFIGURED=3".to_owned()],
        editor: true,
        ..Default::default()
    });
    a.attach_macro_config_note("llg.toml");
    a
}

fn macro_hover(a: &Analysis, marked: &str) -> Option<Hover> {
    let (line, col) = cursor(MACROS, marked);
    hover_at(a, hp("/x/top.sv"), line, col)
}

#[test]
fn hover_on_macro_usage_shows_resolved_value() {
    let _guards = analysis_guards();
    let a = macro_analysis();
    let expected = |value: &str, line: u32| {
        format!(
            "```systemverilog\n{value}\n\ndefined at {}:{line}\n```",
            hp("/x/top.sv")
        )
    };
    let width = macro_hover(&a, "`WI|DTH;");
    assert_eq!(
        width.as_ref().and_then(|hover| hover.range),
        Some(Range::new(Position::new(4, 10), Position::new(4, 16)))
    );
    assert_eq!(hover_text(width), expected("macro WIDTH = 8", 1));
    for marked in ["= `|MAX(x", "`define |MAX(a"] {
        assert_eq!(
            hover_text(macro_hover(&a, marked)),
            expected("macro MAX(a, b) = ((a) > (b)) ? (a) : (b)", 2)
        );
    }
    for marked in ["`define |ENABLE", "`ifdef EN|ABLE"] {
        assert_eq!(
            hover_text(macro_hover(&a, marked)),
            expected("macro ENABLE", 3)
        );
    }
    assert_eq!(
        hover_text(macro_hover(&a, "`define |WIDTH")),
        expected("macro WIDTH = 8", 1)
    );
    assert_eq!(
        hover_text(macro_hover(&a, "`|CONFIGURED")),
        "```systemverilog\nmacro CONFIGURED = 3\n```"
    );
}

#[test]
fn hover_on_undefined_macro_names_the_config() {
    let _guards = analysis_guards();
    let a = macro_analysis();
    let value = hover_text(macro_hover(&a, "`|MISSING"));
    assert!(
        value.contains("`MISSING` is not defined under the current configuration."),
        "{value}"
    );
    assert!(
        value.contains("Checked the `[compile] defines` in llg.toml"),
        "the config note must name the checked source: {value}"
    );
    assert!(macro_hover(&a, "`ifdef AB|SENT").is_none());
    assert!(macro_hover(&a, "`__FI|LE__").is_none());
}

#[test]
fn hover_on_function_name_shows_signature_and_scope() {
    let a = sample_analysis();
    // `function int add(...)` at 1-based (4, 8) → 0-based (3, 7).
    let hover = hover_at(&a, hp("/x/top.sv"), 3, 7).expect("hover on function name");
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
    let hover = hover_at(&a, hp("/x/top.sv"), 4, 7).expect("hover on task name");
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
    let hover = hover_at(&a, hp("/x/top.sv"), 1, 4).expect("hover on param decl");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value, "```systemverilog\nparameter W: int\nvalue = 32'sd8\n```",
        "the elaborated value must be its own short line: {value}"
    );
}

/// The value comes from the model at the target's coordinates, wherever the
/// reference is written.
#[test]
fn hover_on_bound_param_reference_shows_elaborated_value() {
    let (_, mut token_files) = sample_parts();
    token_files.push(FileTokens {
        path: hp("/x/other.sv").to_owned(),
        nodes: vec![TokenInfo {
            line: 7,
            col: 3,
            end_line: 7,
            end_col: 4,
            kind: tokens::TOKEN_SLANG_PARAMETER,
            name: Some("W".to_owned()),
            file: hp("/x/other.sv").to_owned(),
        }],
    });
    let mut bindings = tokens::Bindings::default();
    bindings.references.insert(
        (hp("/x/other.sv").to_owned(), 6, 2),
        DeclTarget {
            name: "W".to_owned(),
            kind: "parameter".to_owned(),
            file: hp("/x/top.sv").to_owned(),
            line0: 1,
            col0: 4,
            via_label: false,
            via_connection: false,
        },
    );
    let a = sample_analysis_with_bindings(token_files, bindings);
    let value = hover_text(hover_at(&a, hp("/x/other.sv"), 6, 2));
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
fn hover_on_unresolved_param_omits_the_value_line() {
    let (mut model, tokens) = sample_parts();
    model.top_instances[0].params[0].value = None;
    let a = Analysis::new(Vec::new(), model, tokens, Vec::new());
    let hover = hover_at(&a, hp("/x/top.sv"), 1, 4).expect("hover on unresolved param");
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
            hover_at(&a, hp("/x/top.sv"), line, col).unwrap_or_else(|| panic!("hover on {what}"));
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
    assert_eq!(param_elab_value(&a, hp("/x/top.sv"), 1, "W"), None);
    // BRANCH diverges across the clones' g_wide scopes → ambiguous too.
    assert_eq!(param_elab_value(&a, hp("/x/top.sv"), 2, "BRANCH"), None);

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
        param_elab_value(&a, hp("/x/top.sv"), 1, "W"),
        Some(&bits(8)),
        "unanimous clones resolve to the shared value"
    );
    assert_eq!(
        param_elab_value(&a, hp("/x/top.sv"), 2, "BRANCH"),
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
    let hover = hover_at(&a, hp("/x/top.sv"), 1, 4).expect("hover on ambiguous param");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert_eq!(
        value, "```systemverilog\nparameter W: int\n```",
        "ambiguity omits rather than guesses: {value}"
    );
}
