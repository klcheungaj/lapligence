//! Queries.

use super::*;

pub(super) fn render_enum_query(ctx: &RCtx<'_>, query: &IrEnumQuery) -> Result<String, String> {
    let member_values = query
        .members
        .iter()
        .map(|member| render_expr_impl(ctx, &member.value).map(|value| value.code))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let member_count = query.members.len();
    Ok(match query.method {
        IrEnumMethod::First => query
            .members
            .first()
            .ok_or_else(|| "enum query has no first member".to_owned())
            .and_then(|member| render_expr_impl(ctx, &member.value).map(|value| value.code))?,
        IrEnumMethod::Last => query
            .members
            .last()
            .ok_or_else(|| "enum query has no last member".to_owned())
            .and_then(|member| render_expr_impl(ctx, &member.value).map(|value| value.code))?,
        IrEnumMethod::Num => format!("sv4_from_u64({member_count}ULL, 32, 1)"),
        IrEnumMethod::Next | IrEnumMethod::Prev => {
            let receiver = query
                .receiver
                .as_ref()
                .ok_or_else(|| "enum navigation query has no receiver".to_owned())?;
            let step = query
                .step
                .as_ref()
                .ok_or_else(|| "enum navigation query has no step".to_owned())?;
            let receiver = render_expr_impl(ctx, receiver)?.code;
            let step = render_expr_impl(ctx, step)?.code;
            let default = render_expr_impl(ctx, &query.default)?.code;
            let direction = if query.method == IrEnumMethod::Next {
                1
            } else {
                -1
            };
            format!(
                "sv4_enum_navigate({receiver}, {step}, (const sv4_t[]){{ {member_values} }}, \
                 {member_count}, {default}, {direction})"
            )
        }
    })
}

pub(super) fn render_vpi_call(
    ctx: &RCtx<'_>,
    site: usize,
    name: &str,
    args: &[IrExpr],
    result_width: u32,
    result_signed: bool,
) -> Result<RenderedExpr, String> {
    let mut declarations = String::new();
    let mut values = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let rendered = render_expr_impl(ctx, arg)?;
        if rendered.width == 0 {
            let value_name = format!("_llg_vpi_arg{index}_real");
            declarations.push_str(&format!("double {value_name} = {}; ", rendered.code));
            values.push(format!(
                "{{ LLG_FMT_REAL, 0, {}, 1, sv4_x(1, 0), {value_name} }}",
                rendered.signed as u8
            ));
        } else {
            let value_name = format!("_llg_vpi_arg{index}_packed");
            declarations.push_str(&format!("sv4_t {value_name} = {}; ", rendered.code));
            values.push(format!(
                "{{ LLG_FMT_PACKED, {}, {}, 0, {value_name}, 0.0 }}",
                rendered.width, rendered.signed as u8
            ));
        }
    }
    let args_name = "_llg_vpi_args";
    let array_len = args.len().max(1);
    let call = if result_width == 0 {
        format!(
            "llg_vpi_call_real_function_site({site}ULL, {}, {args_name}, {})",
            c_string_literal(name),
            args.len()
        )
    } else {
        format!(
            "llg_vpi_call_function_site({site}ULL, {}, {args_name}, {}, {}, {})",
            c_string_literal(name),
            args.len(),
            result_width,
            result_signed as u8
        )
    };
    let value = format!(
        "({{ {declarations} llg_vpi_arg_t {args_name}[{}] = {{ {} }}; {call}; }})",
        array_len,
        if values.is_empty() {
            "{ 0 }".to_owned()
        } else {
            values.join(", ")
        }
    );
    Ok(RenderedExpr {
        code: value,
        width: result_width,
        signed: result_signed,
        fill: None,
    })
}
