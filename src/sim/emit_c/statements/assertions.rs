//! Assertions.

use super::*;

fn diagnostic_location(origin: &crate::sim::semantic::Origin) -> String {
    match origin {
        crate::sim::semantic::Origin::Source {
            path, line, column, ..
        } => format!("{path}:{line}:{column}"),
        crate::sim::semantic::Origin::Synthetic { reason } => {
            format!("<synthetic: {reason}>")
        }
    }
}

pub(super) fn unique_priority_call(check: &IrUniquePriorityCheck, matched: &str, has_default: bool) -> String {
    let Some(kind) = check.kind_code() else {
        return String::new();
    };
    let location = check
        .origin()
        .map(diagnostic_location)
        .unwrap_or_else(|| "<unknown>".to_owned());
    format!(
        "    llg_unique_priority_check({kind}, {matched}, {}, {});\n",
        has_default as u8,
        c_string_literal(&location)
    )
}

pub(super) fn render_assertion_control(
    ctx: &RCtx<'_>,
    kind: IrAssertionControlKind,
    args: &[IrExpr],
    scopes: &[String],
) -> Result<String, String> {
    let kind = match kind {
        IrAssertionControlKind::On => "LLG_ASSERTION_CONTROL_ON",
        IrAssertionControlKind::Off => "LLG_ASSERTION_CONTROL_OFF",
        IrAssertionControlKind::Kill => "LLG_ASSERTION_CONTROL_KILL",
        IrAssertionControlKind::Control => "LLG_ASSERTION_CONTROL_FULL",
    };
    let mut out = String::from("    {\n");
    let mut names = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let rendered = render_expr(ctx, arg)?;
        if rendered.width == 0 {
            return Err("assertion control arguments must be integral".to_owned());
        }
        let name = format!("_llg_assertion_control_arg_{index}");
        out.push_str(&format!("        sv4_t {name} = {};\n", rendered.code));
        names.push(name);
    }
    if names.is_empty() {
        out.push_str("        (void)llg_assertion_control(");
        out.push_str(kind);
        out.push_str(", NULL, 0, ");
    } else {
        out.push_str(&format!(
            "        sv4_t _llg_assertion_control_args[{}] = {{ {} }};\n",
            names.len(),
            names.join(", ")
        ));
        out.push_str(&format!(
            "        (void)llg_assertion_control({kind}, _llg_assertion_control_args, {}, ",
            names.len()
        ));
    }
    if scopes.is_empty() {
        out.push_str("NULL, 0);\n");
    } else {
        let scope_count = scopes.len();
        let rendered_scopes = scopes
            .iter()
            .map(|scope| c_string_literal(scope))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "(const char* const[]){{ {rendered_scopes} }}, {scope_count});\n"
        ));
    }
    out.push_str("    }\n");
    Ok(out)
}

pub(super) struct ImmediateAssertionRender<'a> {
    pub(super) kind: IrImmediateAssertionKind,
    pub(super) condition: &'a IrExpr,
    pub(super) if_true: Option<&'a [crate::sim::ir::IrStmt]>,
    pub(super) if_false: Option<&'a [crate::sim::ir::IrStmt]>,
    pub(super) label: &'a str,
    pub(super) location: &'a str,
    pub(super) identity: u64,
    pub(super) scopes: &'a [&'a ActivationRenderScope],
}

pub(super) fn render_immediate_assertion(
    ctx: &RCtx<'_>,
    assertion: ImmediateAssertionRender<'_>,
) -> Result<String, String> {
    let ImmediateAssertionRender {
        kind,
        condition,
        if_true,
        if_false,
        label,
        location,
        identity,
        scopes,
    } = assertion;
    let rendered = render_expr(ctx, condition)?;
    let condition_name = "_llg_assert_condition";
    let declaration = if rendered.width == 0 {
        format!("double {condition_name} = {};", rendered.code)
    } else {
        format!("sv4_t {condition_name} = {};", rendered.code)
    };
    let condition_bool = if rendered.width == 0 {
        format!("llg_real_to_bool({condition_name})")
    } else {
        format!("sv4_to_bool({condition_name})")
    };
    let label = c_string_literal(label);
    let location = c_string_literal(location);
    let mut out = format!("{{\n    {declaration}\n    if ({condition_bool}) {{\n");
    if kind == IrImmediateAssertionKind::Cover {
        out.push_str(&format!(
            "        llg_assertion_cover({identity}ULL, {label}, {location});\n"
        ));
    }
    if let Some(if_true) = if_true {
        out.push_str(&block_stmts_for_assertion(ctx, if_true, scopes)?);
    }
    out.push_str("    } else {\n");
    if let Some(if_false) = if_false {
        out.push_str(&block_stmts_for_assertion(ctx, if_false, scopes)?);
    } else if kind != IrImmediateAssertionKind::Cover {
        let kind = match kind {
            IrImmediateAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrImmediateAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrImmediateAssertionKind::Cover => unreachable!("cover has no default failure"),
        };
        out.push_str(&format!(
            "        llg_assertion_failure({kind}, {identity}ULL, {label}, {location});\n"
        ));
    }
    out.push_str("    }\n}\n");
    Ok(out)
}

pub(super) struct DeferredImmediateAssertionRender<'a> {
    pub(super) kind: IrImmediateAssertionKind,
    pub(super) condition: &'a IrExpr,
    pub(super) if_true: Option<&'a crate::sim::ir::IrDeferredAction>,
    pub(super) if_false: Option<&'a crate::sim::ir::IrDeferredAction>,
    pub(super) label: &'a str,
    pub(super) location: &'a str,
    pub(super) scope: &'a str,
    pub(super) identity: u64,
}

pub(super) fn render_deferred_immediate_assertion(
    ctx: &RCtx<'_>,
    assertion: DeferredImmediateAssertionRender<'_>,
) -> Result<String, String> {
    let DeferredImmediateAssertionRender {
        kind,
        condition,
        if_true,
        if_false,
        label,
        location,
        scope,
        identity,
    } = assertion;
    let rendered = render_expr(ctx, condition)?;
    let condition_name = "_llg_assert_condition";
    let declaration = if rendered.width == 0 {
        format!("double {condition_name} = {};", rendered.code)
    } else {
        format!("sv4_t {condition_name} = {};", rendered.code)
    };
    let condition_bool = if rendered.width == 0 {
        format!("llg_real_to_bool({condition_name})")
    } else {
        format!("sv4_to_bool({condition_name})")
    };
    let kind = match kind {
        IrImmediateAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
        IrImmediateAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
        IrImmediateAssertionKind::Cover => "LLG_ASSERTION_COVER",
    };
    let label = c_string_literal(label);
    let location = c_string_literal(location);
    let scope = c_string_literal(scope);
    let mut out = format!("if (llg_deferred_assertion_enabled({kind}, {label}, {scope})) {{\n    {declaration}\n    if ({condition_bool}) {{\n");
    out.push_str(&deferred_assertion_enqueue_text(
        ctx, kind, true, identity, &label, &location, &scope, if_true,
    )?);
    out.push_str("    } else {\n");
    out.push_str(&deferred_assertion_enqueue_text(
        ctx, kind, false, identity, &label, &location, &scope, if_false,
    )?);
    out.push_str("    }\n}\n");
    Ok(out)
}

fn deferred_assertion_enqueue_text(
    ctx: &RCtx<'_>,
    kind: &str,
    passed: bool,
    identity: u64,
    label: &str,
    location: &str,
    scope: &str,
    action: Option<&crate::sim::ir::IrDeferredAction>,
) -> Result<String, String> {
    let Some(action) = action else {
        return Ok(format!(
            "        llg_deferred_assertion_scoped({kind}, {}, {identity}ULL, {label}, {location}, {scope}, NULL, NULL);\n",
            passed as u8
        ));
    };
    let frame = format!("_assertion_frame_{}", action.frame().index());
    let mut out = format!(
        "        {{\n            llg_frame_t* {frame} = llg_frame_new({}u);\n",
        action.captures().len()
    );
    for capture in action.captures() {
        let initial = render_expr(ctx, capture.initial())?.code;
        out.push_str(
            &format_frame_capture(&frame, capture.storage(), &initial)?
                .replace("    ", "            "),
        );
    }
    out.push_str(&format!(
        "            llg_deferred_assertion_scoped({kind}, {}, {identity}ULL, {label}, {location}, {scope}, {}, {frame});\n        }}\n",
        passed as u8,
        action.c_name(),
    ));
    Ok(out)
}

fn block_stmts_for_assertion(
    ctx: &RCtx<'_>,
    stmts: &[crate::sim::ir::IrStmt],
    scopes: &[&ActivationRenderScope],
) -> Result<String, String> {
    let mut out = String::new();
    for stmt in stmts {
        out.push_str(&render_stmt_scoped(ctx, stmt, scopes)?);
        out.push_str(&activation_guard(ctx));
    }
    Ok(out)
}
