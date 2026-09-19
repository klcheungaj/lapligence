//! Lvalues.

use super::*;

/// Render the direct packed storage address accepted by the stochastic queue
/// runtime outputs. Lowering currently restricts these LHS values to whole
/// signals or whole packed subroutine storage, so selected aliases cannot be
/// mistaken for pointer-compatible runtime arguments.
pub(in super::super) fn render_lhs_address(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<String, String> {
    match lhs {
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                return Err(
                    "stochastic queue output reached emission with a non-net packed target".into(),
                );
            }
            Ok(format!("&{}", signal.c_name))
        }
        IrLhs::WholeRef { addr, width, .. } if *width != 0 => Ok(addr.clone()),
        IrLhs::WholeRef { .. } => {
            Err("stochastic queue output reached emission with a real target".into())
        }
        _ => Err("stochastic queue output requires whole packed storage".into()),
    }
}

/// Per-dimension size (`|left - right| + 1`).
fn dim_size(ai: &crate::sim::ir::IrArray, k: usize) -> u64 {
    (i64::from(ai.dims[k].0) - i64::from(ai.dims[k].1)).unsigned_abs() + 1
}

/// The per-dimension guard shared by array element reads and writes: temp
/// declarations, the in-range condition, and the row-major linear index
/// expression.  `None` for a degenerate dimension-less array.
pub(crate) fn array_guard(
    ai: &crate::sim::ir::IrArray,
    index_codes: &[String],
) -> Option<(String, String, String)> {
    let nd = ai.dims.len();
    if nd == 0 {
        return None;
    }
    let mut decls = String::new();
    let mut conds = Vec::new();
    let mut terms = Vec::new();
    let mut stride = 1u64;
    for k in (0..nd).rev() {
        let (l, r) = ai.dims[k];
        let off = if l >= r {
            format!("{l} - _v{k}")
        } else {
            format!("_v{k} - {l}")
        };
        decls.push_str(&format!(
            "sv4_t _i{k} = {}; int64_t _v{k} = 0; \
             int _valid{k} = sv4_to_index_i64(_i{k}, &_v{k}) && _v{k} >= {} && _v{k} <= {}; \
             int64_t _o{k} = _valid{k} ? ({off}) : 0; ",
            index_codes[k],
            l.min(r),
            l.max(r)
        ));
        conds.push(format!("_valid{k}"));
        if stride == 1 {
            terms.push(format!("(uint64_t)_o{k}"));
        } else {
            terms.push(format!("(uint64_t)_o{k} * {stride}ULL"));
        }
        stride *= dim_size(ai, k);
    }
    terms.reverse();
    Some((decls, conds.join(" && "), terms.join(" + ")))
}

/// Guarded element read: `({...})` yielding the element for known in-range
/// indices and all-X otherwise; `{arr}[0]` for degenerate arrays.
pub(super) fn guarded_array_read(ai: &crate::sim::ir::IrArray, index_codes: &[String]) -> String {
    match array_guard(ai, index_codes) {
        Some((decls, cond, lin)) => format!(
            "({{ {decls}({cond}) ? {}[({lin})] : {}; }})",
            ai.c_name,
            packed_default(ai.elem_width, ai.signed, ai.two_state)
        ),
        None => format!("{}[0]", ai.c_name),
    }
}

pub(super) fn guarded_real_array_read(
    ai: &crate::sim::ir::IrArray,
    index_codes: &[String],
) -> String {
    match array_guard(ai, index_codes) {
        Some((decls, cond, lin)) => {
            format!("({{ {decls}({cond}) ? {}[({lin})] : 0.0; }})", ai.c_name)
        }
        None => format!("{}[0]", ai.c_name),
    }
}

pub(super) fn lhs_shape(ctx: &RCtx<'_>, lhs: &IrLhs) -> (u32, bool) {
    match lhs {
        IrLhs::Whole(index) => {
            let ty = ctx.model.signal(*index).ty;
            (ty.width(), ty.signed())
        }
        IrLhs::WholeRef { width, signed, .. } | IrLhs::Ref { width, signed, .. } => {
            (*width, *signed)
        }
        IrLhs::PackedSelect { steps, signed, .. } => {
            (steps.last().map_or(0, |step| step.width), *signed)
        }
        IrLhs::Bit(..) => (1, false),
        IrLhs::Part(_, left, right, _) => (left.abs_diff(*right) as u32 + 1, false),
        IrLhs::IdxPart(_, _, _, width, _, _) => (*width, false),
        IrLhs::ArrayElem { arr, elem_sel, .. } => {
            let array = ctx.model.array(*arr);
            match elem_sel {
                IrElemSel::Whole => (array.elem_width, array.signed),
                IrElemSel::Part(left, right) => (left.abs_diff(*right) as u32 + 1, false),
                IrElemSel::Bit(_) => (1, false),
                IrElemSel::Indexed { width, .. } => (*width, false),
                IrElemSel::PackedChain(steps) => (steps.last().map_or(0, |step| step.width), false),
            }
        }
        IrLhs::Stream { width, .. } => (*width, false),
    }
}

fn retag_lhs_value(value: RenderedExpr, width: u32, signed: bool) -> RenderedExpr {
    if width == 0 {
        return RenderedExpr {
            code: if value.width == 0 {
                value.code
            } else {
                format!("sv4_to_real({})", value.code)
            },
            width,
            signed,
            fill: None,
        };
    }
    let code = if value.width == 0 {
        format!("sv4_from_real({}, {width}, {})", value.code, signed as u8)
    } else if let Some(fill) = value.fill {
        format!("sv4_fill({fill}, {width}, {})", signed as u8)
    } else {
        format!("sv4_resize({}, {width}, {})", value.code, signed as u8)
    };
    RenderedExpr {
        code,
        width,
        signed,
        fill: None,
    }
}

fn lhs_two_state(ctx: &RCtx<'_>, lhs: &IrLhs) -> bool {
    match lhs {
        IrLhs::Whole(index) => ctx.model.signal(*index).ty.two_state(),
        IrLhs::PackedSelect { two_state, .. }
        | IrLhs::WholeRef { two_state, .. }
        | IrLhs::Ref { two_state, .. } => *two_state,
        IrLhs::Bit(index, _, selected_two_state)
        | IrLhs::Part(index, .., selected_two_state)
        | IrLhs::IdxPart(index, .., selected_two_state) => {
            ctx.model.signal(*index).ty.two_state() || *selected_two_state
        }
        IrLhs::ArrayElem { arr, .. } => ctx.model.array(*arr).two_state,
        IrLhs::Stream { .. } => false,
    }
}

fn coerce_lhs_read(ctx: &RCtx<'_>, lhs: &IrLhs, mut value: RenderedExpr) -> RenderedExpr {
    if value.width != 0 && lhs_two_state(ctx, lhs) {
        value.code = coerce_two_state(value.code, true);
    }
    value
}

/// Read an assignment target through the same descriptor shapes used by
/// ordinary expressions. Dynamic indices have already been replaced by local
/// captures before this helper is called.
pub(super) fn render_lhs_value(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    width: u32,
    signed: bool,
) -> Result<RenderedExpr, String> {
    let value = match lhs {
        IrLhs::PackedSelect { .. } => {
            return Err("packed activation selects require structured owned emission".to_owned())
        }
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            IrExpr::new(
                IrExprKind::SigRead(*index),
                signal.ty.width(),
                signal.ty.signed(),
                None,
            )
        }
        IrLhs::WholeRef {
            addr,
            width,
            signed,
            ..
        } => {
            return Ok(coerce_lhs_read(
                ctx,
                lhs,
                RenderedExpr {
                    code: format!("*({addr})"),
                    width: *width,
                    signed: *signed,
                    fill: None,
                },
            ));
        }
        IrLhs::Ref {
            addr,
            width,
            signed,
            bit,
            ..
        } => {
            let code = if let Some(index) = bit {
                let index = render_expr_impl(ctx, index)?.code;
                format!("sv4_bit_select(llg_ref_read({addr}), sv4_to_index({index}))")
            } else {
                format!("llg_ref_read({addr})")
            };
            return Ok(coerce_lhs_read(
                ctx,
                lhs,
                RenderedExpr {
                    code,
                    width: *width,
                    signed: *signed,
                    fill: None,
                },
            ));
        }
        IrLhs::Bit(index, select, _) => {
            let signal = ctx.model.signal(*index);
            IrExpr::new(
                IrExprKind::BitSel {
                    base: Box::new(IrExpr::new(
                        IrExprKind::SigRead(*index),
                        signal.ty.width(),
                        signal.ty.signed(),
                        None,
                    )),
                    idx: Box::new(select.clone()),
                },
                1,
                false,
                None,
            )
        }
        IrLhs::Part(index, left, right, _) => {
            let signal = ctx.model.signal(*index);
            IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(IrExpr::new(
                        IrExprKind::SigRead(*index),
                        signal.ty.width(),
                        signal.ty.signed(),
                        None,
                    )),
                    left: *left,
                    right: *right,
                },
                left.abs_diff(*right) as u32 + 1,
                false,
                None,
            )
        }
        IrLhs::IdxPart(index, base, width_expr, selected_width, neg, _) => {
            let signal = ctx.model.signal(*index);
            IrExpr::new(
                IrExprKind::IdxPartSel {
                    base: Box::new(IrExpr::new(
                        IrExprKind::SigRead(*index),
                        signal.ty.width(),
                        signal.ty.signed(),
                        None,
                    )),
                    base_idx: Box::new(base.clone()),
                    width_expr: Box::new(width_expr.clone()),
                    neg: *neg,
                },
                *selected_width,
                false,
                None,
            )
        }
        IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel,
        } => {
            let (element_width, element_signed) = lhs_shape(ctx, lhs);
            IrExpr::new(
                IrExprKind::ArrayRead {
                    arr: *arr,
                    indices: indices.clone(),
                    elem_sel: elem_sel.clone(),
                },
                element_width,
                element_signed,
                None,
            )
        }
        IrLhs::Stream { .. } => {
            return Err(
                "streaming assignment target cannot be read as a mutation expression".into(),
            )
        }
    };
    Ok(coerce_lhs_read(
        ctx,
        lhs,
        retag_lhs_value(render_expr_impl(ctx, &value)?, width, signed),
    ))
}

pub(super) fn capture_lhs_indices(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<(String, IrLhs), String> {
    capture_lhs_indices_with_prefix(ctx, lhs, "_llg_mut_idx")
}

pub(super) fn capture_lhs_indices_with_prefix(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    index_prefix: &str,
) -> Result<(String, IrLhs), String> {
    fn capture(
        ctx: &RCtx<'_>,
        expression: &IrExpr,
        declarations: &mut String,
        next: &mut usize,
        index_prefix: &str,
    ) -> Result<IrExpr, String> {
        let rendered = render_expr_impl(ctx, expression)?;
        let name = format!("{index_prefix}{next}");
        *next += 1;
        let ty = if expression.width == 0 {
            "double"
        } else {
            "sv4_t"
        };
        declarations.push_str(&format!("{ty} {name} = {}; ", rendered.code));
        Ok(IrExpr::new(
            IrExprKind::LocalRead(name),
            expression.width,
            expression.signed,
            expression.fill,
        ))
    }

    fn visit(
        ctx: &RCtx<'_>,
        lhs: &mut IrLhs,
        declarations: &mut String,
        next: &mut usize,
        index_prefix: &str,
    ) -> Result<(), String> {
        match lhs {
            IrLhs::PackedSelect { .. } => {
                return Err(
                    "packed activation selects require structured owned emission".to_owned(),
                )
            }
            IrLhs::Ref {
                bit: Some(index), ..
            } => {
                **index = capture(ctx, index, declarations, next, index_prefix)?;
            }
            IrLhs::Bit(_, index, _) => {
                *index = capture(ctx, index, declarations, next, index_prefix)?;
            }
            IrLhs::IdxPart(_, base, width_expr, _, _, _) => {
                *base = capture(ctx, base, declarations, next, index_prefix)?;
                *width_expr = capture(ctx, width_expr, declarations, next, index_prefix)?;
            }
            IrLhs::ArrayElem {
                indices, elem_sel, ..
            } => {
                for index in indices {
                    *index = capture(ctx, index, declarations, next, index_prefix)?;
                }
                match elem_sel {
                    IrElemSel::Bit(index) => {
                        **index = capture(ctx, index, declarations, next, index_prefix)?;
                    }
                    IrElemSel::Indexed { base, .. } => {
                        **base = capture(ctx, base, declarations, next, index_prefix)?;
                    }
                    IrElemSel::PackedChain(_) => {
                        return Err(
                            "packed selection chains require structured owned emission".to_owned()
                        );
                    }
                    IrElemSel::Whole | IrElemSel::Part(..) => {}
                }
            }
            IrLhs::Stream { parts, .. } => {
                for (part, _) in parts {
                    visit(ctx, part, declarations, next, index_prefix)?;
                }
            }
            IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Part(..) => {}
        }
        Ok(())
    }

    let mut captured = lhs.clone();
    let mut declarations = String::new();
    let mut next = 0;
    visit(
        ctx,
        &mut captured,
        &mut declarations,
        &mut next,
        index_prefix,
    )?;
    Ok((declarations, captured))
}

pub(super) fn render_mutation_expr(
    ctx: &RCtx<'_>,
    expression: &IrExpr,
    mutation: &crate::sim::ir::IrMutationExpr,
) -> Result<RenderedExpr, String> {
    let (mut declarations, lhs) = capture_lhs_indices(ctx, &mutation.lhs)?;
    let needs_current = mutation.reads_current || mutation.post;
    let current = if needs_current {
        Some(render_lhs_value(
            ctx,
            &lhs,
            mutation.current_width,
            mutation.current_signed,
        )?)
    } else {
        None
    };
    if let Some(current) = &current {
        let ty = if mutation.current_width == 0 {
            "double"
        } else {
            "sv4_t"
        };
        if mutation.post {
            declarations.push_str(&format!(
                "{ty} _llg_mut_old = {}; {ty} _llg_mut_current = _llg_mut_old; ",
                current.code
            ));
        } else {
            declarations.push_str(&format!("{ty} _llg_mut_current = {}; ", current.code));
        }
    }
    let value = render_expr_impl(ctx, &mutation.value)?;
    let value_ty = if value.width == 0 { "double" } else { "sv4_t" };
    declarations.push_str(&format!("{value_ty} _llg_mut_new = {}; ", value.code));
    let new_expr = IrExpr::new(
        IrExprKind::LocalRead("_llg_mut_new".to_owned()),
        value.width,
        value.signed,
        None,
    );
    let write = render_assign(ctx, &lhs, &new_expr, false)?;
    let result = if mutation.post {
        "_llg_mut_old".to_owned()
    } else {
        render_lhs_value(ctx, &lhs, expression.width, expression.signed)?.code
    };
    Ok(RenderedExpr {
        code: format!("({{ {declarations} {write} {result}; }})"),
        width: expression.width,
        signed: expression.signed,
        fill: None,
    })
}
