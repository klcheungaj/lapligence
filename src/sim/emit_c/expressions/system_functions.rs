//! System functions.

use super::*;

pub(super) fn render_legacy_random(
    ctx: &RCtx<'_>,
    kind: IrRandomFunc,
    seed: Option<&IrLhs>,
    args: &[IrExpr],
) -> Result<RenderedExpr, String> {
    let mut declarations = String::new();
    let (seed_lhs, seed_call) = if let Some(seed) = seed {
        let (seed_declarations, captured) = capture_lhs_indices(ctx, seed)?;
        declarations.push_str(&seed_declarations);
        let (width, signed) = lhs_shape(ctx, &captured);
        if width == 0 {
            return Err("legacy random seed must be packed storage".to_owned());
        }
        let value = render_lhs_value(ctx, &captured, width, signed)?;
        declarations.push_str(&format!(
            "int32_t _llg_random_seed = (int32_t)sv4_to_i64(sv4_cast({}, 32, 1)); ",
            value.code
        ));
        (Some(captured), Some("&_llg_random_seed".to_owned()))
    } else {
        (None, None)
    };

    let mut parameter_names = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let value = render_expr_impl(ctx, arg)?;
        if value.width == 0 {
            return Err("legacy random parameters must be packed".to_owned());
        }
        let name = format!("_llg_random_arg{index}");
        declarations.push_str(&format!("sv4_t {name} = sv4_cast({}, 32, 1); ", value.code));
        parameter_names.push(format!("(int32_t)sv4_to_i64({name})"));
    }

    let call = if kind == IrRandomFunc::Random && seed_call.is_none() {
        "llg_random_default()".to_owned()
    } else {
        let name = kind.runtime_name();
        let mut arguments = Vec::with_capacity(1 + parameter_names.len());
        arguments.push(seed_call.ok_or_else(|| {
            "legacy distribution function is missing its seed address".to_owned()
        })?);
        arguments.extend(parameter_names);
        format!("{name}({})", arguments.join(", "))
    };
    declarations.push_str(&format!("int32_t _llg_random_result = {call}; "));
    if let Some(seed_lhs) = seed_lhs {
        let result = IrExpr::new(
            IrExprKind::Verbatim {
                code: "sv4_from_i64((int64_t)_llg_random_seed, 32)".to_owned(),
                width: 32,
                signed: true,
            },
            32,
            true,
            None,
        );
        declarations.push_str(&render_assign(ctx, &seed_lhs, &result, false)?);
        declarations.push(' ');
    }
    Ok(RenderedExpr {
        code: format!("({{ {declarations} sv4_from_i64((int64_t)_llg_random_result, 32); }})"),
        width: 32,
        signed: true,
        fill: None,
    })
}
