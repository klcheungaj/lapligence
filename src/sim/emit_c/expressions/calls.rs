//! Calls.

use super::*;

/// Render an expression-position function call with output-formal temps:
/// either a plain call or one GNU statement expression carrying the temps,
/// the writebacks and the result.
/// Bind queue actuals before any callee code, then release their shared cells
/// after copy-out. The runtime also unwinds this scope on process cancellation.
pub(in super::super) fn with_ref_scope(code: String, args: &[IrCallArg], result: Option<&str>) -> String {
    if !args.iter().any(|arg| matches!(arg, IrCallArg::RefAddr { .. })) {
        return code;
    }
    match result {
        Some(ty) => format!(
            "({{ llg_ref_scope_t *_llg_ref_scope = llg_ref_scope_begin(); {ty} _llg_ref_result = {code}; llg_ref_scope_end(_llg_ref_scope); _llg_ref_result; }})"
        ),
        None => format!(
            "    {{ llg_ref_scope_t *_llg_ref_scope = llg_ref_scope_begin();\n{code}        llg_ref_scope_end(_llg_ref_scope);\n    }}\n"
        ),
    }
}

pub(super) fn render_call_expr(
    ctx: &RCtx<'_>,
    call: &crate::sim::ir::IrCallExpr,
) -> Result<RenderedExpr, String> {
    let f = ctx.model.func(call.f);
    let has_ret = f.ret.is_some();
    let ret_w = f.ret.as_ref().map(|t| t.width()).unwrap_or(1);
    let ret_s = f.ret.as_ref().map(|t| t.signed()).unwrap_or(false);

    // Argument codes in C parameter order (outputs, inputs, depth); output
    // temps are collected with their formal index for init/writeback.
    struct TempInfo<'a> {
        idx: usize,
        name: &'a str,
        init: Option<&'a IrExpr>,
        wb: &'a IrLhs,
        storage_lhs: Option<&'a IrLhs>,
        storage_read: Option<&'a IrExpr>,
        selector_inits: &'a [(String, u32, bool, bool, IrExpr)],
    }
    let mut temps: Vec<TempInfo<'_>> = Vec::new();
    struct StringTempInfo<'a> {
        name: &'a str,
        init: Option<&'a IrStringExpr>,
        writeback: &'a str,
        storage_addr: Option<&'a str>,
        storage_read: Option<&'a IrStringExpr>,
    }
    let mut string_temps: Vec<StringTempInfo<'_>> = Vec::new();
    let mut call_args: Vec<String> = Vec::new();
    let formal_order = f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
        .chain(
            f.formals
                .iter()
                .enumerate()
                .filter(|(_, formal)| !formal.is_address()),
        );
    for ((idx, _), arg) in formal_order.zip(&call.args) {
        match arg {
            IrCallArg::StringVal(value) => {
                call_args.push(super::super::objects::string(ctx, value)?);
            }
            IrCallArg::Val(e) => {
                let rendered = render_expr_impl(ctx, e)?;
                let form = &f.formals[idx];
                call_args.push(if form.real {
                    round_shortreal(real_code(&rendered), form.shortreal)
                } else {
                    coerce_two_state(rendered.code, form.two_state)
                });
            }
            IrCallArg::ChandleVal(value) => {
                call_args.push(super::super::objects::chandle(ctx, value)?);
            }
            IrCallArg::ChandleAddr(addr) | IrCallArg::ChandleRefAddr(addr) => {
                call_args.push(addr.clone());
            }
            IrCallArg::OutAddr(addr) => call_args.push(addr.clone()),
            IrCallArg::RefAddr { addr, .. } => call_args.push(addr.clone()),
            IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => {
                call_args.push(addr.clone())
            }
            IrCallArg::StringOutTemp {
                name,
                init,
                writeback,
                storage_addr,
                storage_read,
            } => {
                string_temps.push(StringTempInfo {
                    name,
                    init: init.as_deref(),
                    writeback,
                    storage_addr: storage_addr.as_deref(),
                    storage_read: storage_read.as_deref(),
                });
                call_args.push(
                    storage_addr
                        .as_deref()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("&{name}")),
                );
            }
            IrCallArg::OutTemp {
                name,
                init,
                writeback,
                storage_addr,
                storage_lhs,
                storage_read,
                selector_inits,
            } => {
                temps.push(TempInfo {
                    idx,
                    name,
                    init: init.as_deref(),
                    wb: writeback,
                    storage_lhs: storage_lhs.as_deref(),
                    storage_read: storage_read.as_deref(),
                    selector_inits,
                });
                call_args.push(
                    storage_addr
                        .as_deref()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("&{name}")),
                );
            }
        }
    }
    let call_name = if let Some(virtual_call) = &call.virtual_call {
        call_args.insert(0, super::super::objects::chandle(ctx, &virtual_call.receiver)?);
        format!(
            "llg_vif_call_{}_{}",
            virtual_call.interface, virtual_call.method
        )
    } else {
        if let Some(receiver) = &call.receiver {
            call_args.insert(0, super::super::objects::chandle(ctx, receiver)?);
        }
        super::super::function_call_name(f, call.virtual_dispatch)
    };
    call_args.push(call.depth.code());
    let call_code = format!("{}({})", call_name, call_args.join(", "));

    if temps.is_empty() && string_temps.is_empty() {
        if !call.void_x {
            return Ok(RenderedExpr {
                code: call_code,
                width: ret_w,
                signed: ret_s,
                fill: None,
            });
        }
        return Ok(RenderedExpr {
            code: format!("({{ {call_code}; sv4_x({ret_w}, {}); }})", ret_s as u8),
            width: ret_w,
            signed: ret_s,
            fill: None,
        });
    }

    let mut parts: Vec<String> = Vec::new();
    for t in &temps {
        let form = &f.formals[t.idx];
        for (name, width, signed, two_state, init) in t.selector_inits {
            let rendered = render_expr_impl(ctx, init)?;
            let init = if *width == 0 {
                real_code(&rendered)
            } else {
                coerce_two_state(rendered.code, *two_state)
            };
            parts.push(format!(
                "{} {} = {init}",
                if *width == 0 { "double" } else { "sv4_t" },
                name
            ));
            let _ = signed;
        }
        let init = match t.init {
            Some(expr) if form.real => {
                round_shortreal(real_code(&render_expr_impl(ctx, expr)?), form.shortreal)
            }
            Some(expr) => coerce_two_state(render_expr_impl(ctx, expr)?.code, form.two_state),
            None if form.real => "0.0".to_string(),
            None => packed_default(form.width, form.signed, form.two_state),
        };
        parts.push(format!(
            "{} {} = {init}",
            if form.real { "double" } else { "sv4_t" },
            t.name
        ));
        if let (Some(storage_lhs), Some(_)) = (t.storage_lhs, t.init) {
            let staged = IrExpr::new(
                IrExprKind::LocalRead(t.name.to_string()),
                form.width,
                form.signed,
                None,
            );
            parts.push(
                render_assign(ctx, storage_lhs, &staged, false)?
                    .trim_end_matches(';')
                    .to_string(),
            );
        }
    }
    for t in &string_temps {
        let init = t
            .init
            .map(|value| super::super::objects::string(ctx, value))
            .transpose()?
            .unwrap_or_else(|| "(llg_string_t){0}".to_owned());
        parts.push(format!("llg_string_t {} = {init}", t.name));
        if let Some(storage_addr) = t.storage_addr {
            parts.push(format!(
                "llg_string_move({storage_addr}, llg_string_clone(&{}))",
                t.name
            ));
        }
    }
    if has_ret {
        parts.push(format!(
            "{} _r = {call_code}",
            if matches!(f.ret, Some(crate::sim::ir::IrType::Real { .. })) {
                "double"
            } else {
                "sv4_t"
            }
        ));
    } else {
        parts.push(call_code.clone());
    }
    for t in &temps {
        let form = &f.formals[t.idx];
        let rhs = t.storage_read.cloned().unwrap_or_else(|| {
            IrExpr::new(
                IrExprKind::LocalRead(t.name.to_string()),
                form.width,
                form.signed,
                None,
            )
        });
        let stmt = render_assign(ctx, t.wb, &rhs, false)?;
        parts.push(stmt.trim_end_matches(';').to_string());
    }
    for t in &string_temps {
        let source = t
            .storage_read
            .map(|value| super::super::objects::string(ctx, value))
            .transpose()?
            .unwrap_or_else(|| format!("llg_string_clone(&{})", t.name));
        if t.storage_addr.is_some() {
            parts.push(format!("llg_string_move(&{}, {source})", t.name));
        }
        parts.push(format!(
            "llg_string_move({}, {})",
            t.writeback,
            if t.storage_addr.is_some() {
                format!("llg_string_clone(&{})", t.name)
            } else {
                t.name.to_owned()
            }
        ));
    }
    if has_ret {
        parts.push("_r".to_string());
    } else {
        parts.push(
            if matches!(f.ret, Some(crate::sim::ir::IrType::Real { .. })) {
                "0.0".to_string()
            } else {
                format!("sv4_x({ret_w}, {})", ret_s as u8)
            },
        );
    }
    Ok(RenderedExpr {
        code: format!("({{ {}; }})", parts.join("; ")),
        width: ret_w,
        signed: ret_s,
        fill: None,
    })
}
