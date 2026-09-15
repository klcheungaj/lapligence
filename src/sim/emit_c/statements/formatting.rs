//! Formatting.

use super::*;

pub(super) fn render_typed_display(
    ctx: &RCtx<'_>,
    fmt: &str,
    args: &[IrDisplayArg],
    scope: &str,
    newline: bool,
    descriptor: Option<&IrExpr>,
    time_unit_fs: u64,
) -> Result<String, String> {
    let scope = c_string_literal(scope);
    let descriptor = descriptor
        .map(|value| render_expr(ctx, value).map(|value| value.code))
        .transpose()?;
    if args.is_empty() {
        return Ok(if let Some(descriptor) = descriptor {
            format!(
                "    {{ uint32_t _llg_file_descriptor = llg_file_descriptor({descriptor}); llg_file_display_typed(_llg_file_descriptor, {fmt}, NULL, 0, {scope}, {}); }}\n",
                newline as u8
            )
        } else {
            let output_fn = if newline {
                "llg_display_typed"
            } else {
                "llg_write_typed"
            };
            format!("    {output_fn}({fmt}, NULL, 0, {scope});\n")
        });
    }
    // Do not put expression calls in a C aggregate initializer.  C does not
    // specify the order in which initializer expressions are evaluated, so a
    // display such as `$display("%d %d", f(), g())` could observe side effects
    // in the opposite order.  Sequential field assignments preserve the HDL
    // argument evaluation order and also make string ownership explicit.
    let mut assignments = Vec::with_capacity(args.len() + 2);
    assignments.push(format!(
        "llg_fmt_arg_t _display_args[{}] = {{0}};",
        args.len()
    ));
    for (index, arg) in args.iter().enumerate() {
        let assignment = match arg {
            IrDisplayArg::Packed(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_PACKED;\n        _display_args[{index}].time_unit_fs = {time_unit_fs}ULL;\n        _display_args[{index}].value.packed = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::Real(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_REAL;\n        _display_args[{index}].time_unit_fs = {time_unit_fs}ULL;\n        _display_args[{index}].value.real = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::String(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_STRING;\n        _display_args[{index}].value.string = {};",
                super::super::objects::string(ctx, value)?
            ),
        };
        assignments.push(assignment);
    }
    let call = if let Some(descriptor) = descriptor {
        assignments.insert(
            1,
            format!("uint32_t _llg_file_descriptor = llg_file_descriptor({descriptor});"),
        );
        format!(
            "llg_file_display_typed(_llg_file_descriptor, {fmt}, _display_args, {}, {scope}, {});",
            args.len(),
            newline as u8
        )
    } else {
        let output_fn = if newline {
            "llg_display_typed"
        } else {
            "llg_write_typed"
        };
        format!(
            "{output_fn}({fmt}, _display_args, {}, {scope});",
            args.len()
        )
    };
    assignments.push(call);
    Ok(format!(
        "    {{\n        {}\n    }}\n",
        assignments.join("\n        ")
    ))
}

pub(super) fn render_severity(
    ctx: &RCtx<'_>,
    level: IrSeverityLevel,
    fmt: &str,
    args: &[IrDisplayArg],
    scope: &str,
    location: &str,
    fatal_finish_number: Option<u8>,
) -> Result<String, String> {
    let scope = c_string_literal(scope);
    let location = c_string_literal(location);
    if args.is_empty() {
        return Ok(format!(
            "    {}\n",
            render_severity_call(
                level,
                fmt,
                "NULL",
                0,
                &scope,
                &location,
                fatal_finish_number,
            )?
        ));
    }
    // Keep argument evaluation in source order. This is shared with typed
    // display emission so side effects in a severity message occur once.
    let mut assignments = Vec::with_capacity(args.len() + 2);
    assignments.push(format!(
        "llg_fmt_arg_t _severity_args[{}] = {{0}};",
        args.len()
    ));
    for (index, arg) in args.iter().enumerate() {
        let assignment = match arg {
            IrDisplayArg::Packed(value) => format!(
                "_severity_args[{index}].kind = LLG_FMT_PACKED;\n        _severity_args[{index}].value.packed = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::Real(value) => format!(
                "_severity_args[{index}].kind = LLG_FMT_REAL;\n        _severity_args[{index}].value.real = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::String(value) => format!(
                "_severity_args[{index}].kind = LLG_FMT_STRING;\n        _severity_args[{index}].value.string = {};",
                super::super::objects::string(ctx, value)?
            ),
        };
        assignments.push(assignment);
    }
    assignments.push(render_severity_call(
        level,
        fmt,
        "_severity_args",
        args.len(),
        &scope,
        &location,
        fatal_finish_number,
    )?);
    Ok(format!(
        "    {{\n        {}\n    }}\n",
        assignments.join("\n        ")
    ))
}

fn render_severity_call(
    level: IrSeverityLevel,
    fmt: &str,
    args: &str,
    n: usize,
    scope: &str,
    location: &str,
    fatal_finish_number: Option<u8>,
) -> Result<String, String> {
    Ok(match level {
        IrSeverityLevel::Fatal => {
            let finish_number = fatal_finish_number
                .ok_or_else(|| "fatal severity is missing its finish number".to_string())?;
            format!("llg_rt_fatal_typed({finish_number}, {fmt}, {args}, {n}, {scope}, {location});")
        }
        IrSeverityLevel::Info => format!(
            "llg_rt_severity_typed(LLG_SEVERITY_INFO, {fmt}, {args}, {n}, {scope}, {location});"
        ),
        IrSeverityLevel::Warning => format!(
            "llg_rt_severity_typed(LLG_SEVERITY_WARNING, {fmt}, {args}, {n}, {scope}, {location});"
        ),
        IrSeverityLevel::Error => format!(
            "llg_rt_severity_typed(LLG_SEVERITY_ERROR, {fmt}, {args}, {n}, {scope}, {location});"
        ),
    })
}
