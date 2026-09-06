//! C rendering for non-integral objects. String expression results are owned.
use super::context::RCtx;
use super::expressions::render_expr_impl;
use crate::sim::ir::*;

pub(super) fn string(ctx: &RCtx<'_>, value: &IrStringExpr) -> Result<String, String> {
    Ok(match value {
        IrStringExpr::Literal(bytes) => {
            let literal = bytes
                .iter()
                .map(|byte| format!("\\{:03o}", byte))
                .collect::<String>();
            format!("llg_string_bytes(\"{literal}\", {})", bytes.len())
        }
        IrStringExpr::Read(index) => {
            format!("llg_string_clone(&{})", ctx.model.objects[*index].c_name)
        }
        IrStringExpr::LocalRead(name) => format!("llg_string_clone(&{name})"),
        IrStringExpr::Call {
            function,
            args,
            depth,
        } => {
            let mut rendered = Vec::with_capacity(args.len() + 1);
            for (arg, formal) in args.iter().zip(&ctx.model.funcs[*function].formals) {
                let code = render_expr_impl(ctx, arg)?.code;
                rendered.push(super::expressions::coerce_two_state(code, formal.two_state));
            }
            rendered.push(depth.code());
            format!(
                "{}({})",
                ctx.model.funcs[*function].c_name,
                rendered.join(", ")
            )
        }
        IrStringExpr::Concat(parts) => {
            let mut value = "llg_string_bytes(\"\", 0)".to_owned();
            for part in parts {
                value = format!("llg_string_concat({value}, {})", string(ctx, part)?);
            }
            value
        }
        IrStringExpr::Repeat(value, count) => format!(
            "llg_string_repeat({}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, count)?.code
        ),
        IrStringExpr::FromPacked(value) => format!(
            "llg_string_from_packed({})",
            render_expr_impl(ctx, value)?.code
        ),
        IrStringExpr::Case(value, upper) => format!(
            "llg_string_case({}, {})",
            string(ctx, value)?,
            u8::from(*upper)
        ),
        IrStringExpr::Substr(value, first, last) => format!(
            "llg_string_substr({}, {}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, first)?.code,
            render_expr_impl(ctx, last)?.code
        ),
    })
}

pub(super) fn chandle(ctx: &RCtx<'_>, value: &IrChandleExpr) -> String {
    match value {
        IrChandleExpr::Null => "NULL".to_owned(),
        IrChandleExpr::Read(index) => ctx.model.objects[*index].c_name.clone(),
        IrChandleExpr::LocalRead(name) => name.clone(),
        IrChandleExpr::FormalRead(index) => format!("a{index}"),
        IrChandleExpr::Call {
            function,
            args,
            depth,
        } => {
            let mut args = args.iter().map(|arg| chandle(ctx, arg)).collect::<Vec<_>>();
            args.push(depth.code());
            format!("{}({})", ctx.model.funcs[*function].c_name, args.join(", "))
        }
    }
}

pub(super) fn query(
    ctx: &RCtx<'_>,
    query: &IrObjectQuery,
    width: u32,
    signed: bool,
) -> Result<String, String> {
    Ok(match query {
        IrObjectQuery::StringLen(value) => format!("llg_string_len({})", string(ctx, value)?),
        IrObjectQuery::StringGetc(value, index) => format!(
            "llg_string_getc({}, {})",
            string(ctx, value)?,
            render_expr_impl(ctx, index)?.code
        ),
        IrObjectQuery::StringCompare(a, b, ignore) => format!(
            "llg_string_compare({}, {}, {})",
            string(ctx, a)?,
            string(ctx, b)?,
            u8::from(*ignore)
        ),
        IrObjectQuery::StringAtoi(value, base) => {
            format!("llg_string_atoi({}, {base})", string(ctx, value)?)
        }
        IrObjectQuery::StringPacked(value) => format!(
            "llg_string_to_packed({}, {width}, {})",
            string(ctx, value)?,
            u8::from(signed)
        ),
        IrObjectQuery::ChandleEq(a, b) => format!(
            "sv4_from_u64({} == {}, 1, 0)",
            chandle(ctx, a),
            chandle(ctx, b)
        ),
    })
}

pub(super) fn statement(ctx: &RCtx<'_>, operation: &IrObjectStmt) -> Result<String, String> {
    Ok(match operation {
        IrObjectStmt::StringPrint(value) => {
            format!("    llg_string_print({});\n", string(ctx, value)?)
        }
        IrObjectStmt::StringAssign(index, value) => format!(
            "    llg_string_move(&{}, {});\n",
            ctx.model.objects[*index].c_name,
            string(ctx, value)?
        ),
        IrObjectStmt::StringAssignLocal(target, value) => {
            format!("    llg_string_move(&{target}, {});\n", string(ctx, value)?)
        }
        IrObjectStmt::StringPutc(index, position, value) => format!(
            "    llg_string_putc(&{}, {}, {});\n",
            ctx.model.objects[*index].c_name,
            render_expr_impl(ctx, position)?.code,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::StringItoa(index, value, base) => format!(
            "    llg_string_itoa(&{}, {}, {base});\n",
            ctx.model.objects[*index].c_name,
            render_expr_impl(ctx, value)?.code
        ),
        IrObjectStmt::ChandleAssign(index, value) => format!(
            "    {} = {};\n",
            ctx.model.objects[*index].c_name,
            chandle(ctx, value)
        ),
        IrObjectStmt::ChandleAssignLocal(target, value) => {
            format!("    {target} = {};\n", chandle(ctx, value))
        }
    })
}
