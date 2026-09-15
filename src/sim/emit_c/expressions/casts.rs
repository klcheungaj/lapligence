//! Casts.

use super::*;

pub(super) fn render_dynamic_cast(
    ctx: &RCtx<'_>,
    expression: &IrExpr,
    cast: &crate::sim::ir::IrDynamicCast,
) -> Result<RenderedExpr, String> {
    if let (Some(target), Some(source), Some(expected)) = (
        cast.class_target.as_deref(),
        cast.class_source.as_ref(),
        cast.class_expected,
    ) {
        let source = super::super::objects::chandle(ctx, source)?;
        return Ok(RenderedExpr {
            code: format!(
                "({{ void *_llg_cast_source = {source}; int _llg_cast_ok = \
                 _llg_cast_source && llg_class_is_a(_llg_cast_source, {expected}); \
                 if (_llg_cast_ok) *({target}) = _llg_cast_source; \
                 sv4_from_u64((uint64_t)_llg_cast_ok, 1, 0); }})"
            ),
            width: expression.width,
            signed: expression.signed,
            fill: None,
        });
    }
    let (mut declarations, lhs) = capture_lhs_indices(ctx, &cast.lhs)?;
    let rhs = render_expr_impl(ctx, &cast.rhs)?;
    let (rhs_type, rhs_name) = if rhs.width == 0 {
        ("double", "_llg_cast_source_real")
    } else {
        ("sv4_t", "_llg_cast_source")
    };
    declarations.push_str(&format!("{rhs_type} {rhs_name} = {}; ", rhs.code));
    let value = if cast.target_width == 0 {
        let code = if rhs.width == 0 {
            rhs_name.to_owned()
        } else {
            format!("sv4_to_real({rhs_name})")
        };
        let code = super::super::constants::round_shortreal(code, cast.target_shortreal);
        declarations.push_str(&format!("double _llg_cast_real = {code}; "));
        IrExpr::new(
            IrExprKind::LocalRead("_llg_cast_real".to_owned()),
            0,
            true,
            None,
        )
    } else {
        let mut code = if rhs.width == 0 {
            format!(
                "sv4_from_real({}, {}, {})",
                rhs_name, cast.target_width, cast.target_signed as u8
            )
        } else if let Some(fill) = rhs.fill {
            format!(
                "sv4_fill({fill}, {}, {})",
                cast.target_width, cast.target_signed as u8
            )
        } else {
            format!(
                "sv4_cast({}, {}, {})",
                rhs_name, cast.target_width, cast.target_signed as u8
            )
        };
        if cast.target_two_state {
            code = format!("sv4_to_two_state({code})");
        }
        declarations.push_str(&format!("sv4_t _llg_cast_value = {code}; "));
        IrExpr::new(
            IrExprKind::LocalRead("_llg_cast_value".to_owned()),
            cast.target_width,
            cast.target_signed,
            None,
        )
    };
    let mut checks = Vec::with_capacity(cast.valid_values.len());
    for valid in &cast.valid_values {
        let rendered = render_expr_impl(ctx, valid)?;
        checks.push(format!(
            "sv4_to_bool(sv4_case_eq({rhs_name}, {}))",
            rendered.code
        ));
    }
    // Enum membership is a case-equality check: an X/Z source value is never
    // a valid known enum member, and every member is checked before the
    // destination is touched.
    let ok = if checks.is_empty() {
        "1".to_owned()
    } else {
        format!("({})", checks.join(" || "))
    };
    let write = render_assign(ctx, &lhs, &value, false)?;
    declarations.push_str(&format!("int _llg_cast_ok = {ok}; "));
    Ok(RenderedExpr {
        code: format!(
            "({{ {declarations} if (_llg_cast_ok) {{ {write} }} sv4_from_u64((uint64_t)_llg_cast_ok, 1, 0); }})"
        ),
        width: expression.width,
        signed: expression.signed,
        fill: None,
    })
}
