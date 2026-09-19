//! Assignments.

use super::*;

/// Full assignment statement assigning `rhs` into the LHS target:
/// `llg_ba/nba(...)` for plain targets (with statement-expression select
/// write-backs), the `_d` variants for real companions, a guarded block for
/// array elements, and `llg_net_write` for collapsed-net members.
pub(in super::super) fn render_assign(
    ctx: &RCtx<'_>,
    lh: &IrLhs,
    rhs: &IrExpr,
    nba: bool,
) -> Result<String, String> {
    if nba {
        return super::super::assignments::render_nba(ctx, lh, rhs, "0ULL");
    }
    // Real companion targets copy the (possibly converted) real value.
    if let IrLhs::Whole(idx) = lh {
        let sig = ctx.model.signal(*idx);
        if let IrType::Real { shortreal } = sig.ty {
            let rr = render_expr_impl(ctx, rhs)?;
            let call = "llg_ba_d";
            return Ok(format!(
                "{call}(&{}, {});",
                sig.c_name,
                round_shortreal(real_code(&rr), shortreal)
            ));
        }
    }
    // A streaming target consumes one packed RHS value, applies the inverse
    // stream permutation, then assigns conventional concatenation slices to
    // its component targets from most significant to least significant.
    if let IrLhs::Stream {
        parts,
        width,
        slice,
        direction,
    } = lh
    {
        let rendered = render_expr_impl(ctx, rhs)?;
        let value = if rhs.width == 0 {
            rendered_to_vector(&rendered, *width, false)?
        } else if let Some(fill) = rendered.fill {
            format!("sv4_fill({fill}, {width}, 0)")
        } else {
            format!("sv4_cast({}, {width}, 0)", rendered.code)
        };
        let mut statements = String::new();
        let mut cursor = *width;
        for (part, part_width) in parts {
            let right = cursor - part_width;
            let left = cursor - 1;
            let part_value = IrExpr::new(
                IrExprKind::Verbatim {
                    code: format!("sv4_part_select(_stream_value, {left}, {right})"),
                    width: *part_width,
                    signed: false,
                },
                *part_width,
                false,
                None,
            );
            statements.push_str(&render_assign(ctx, part, &part_value, false)?);
            cursor = right;
        }
        return Ok(format!(
            "{{ sv4_t _stream_value = sv4_unstream({value}, {slice}, {}); {statements} }}",
            matches!(direction, IrStreamDirection::RightToLeft) as u8
        ));
    }
    if let IrLhs::WholeRef {
        addr,
        width,
        shortreal,
        ..
    } = lh
    {
        if *width == 0 {
            let rendered = render_expr_impl(ctx, rhs)?;
            let value = if rendered.width == 0 {
                rendered.code
            } else {
                format!("sv4_to_real({})", rendered.code)
            };
            return Ok(format!(
                "llg_ba_d({addr}, {});",
                round_shortreal(value, *shortreal)
            ));
        }
    }
    if let IrLhs::Ref {
        addr,
        width,
        signed,
        two_state,
        const_ref,
        bit,
    } = lh
    {
        if *const_ref {
            return Err("write through const ref formal is not supported".to_string());
        }
        let rendered = render_expr_impl(ctx, rhs)?;
        let value = if rendered.width == 0 {
            format!(
                "sv4_from_real({}, {}, {})",
                rendered.code, width, *signed as u8
            )
        } else if let Some(fill) = rendered.fill {
            format!("sv4_fill({fill}, {width}, {})", *signed as u8)
        } else {
            format!("sv4_cast({}, {width}, {})", rendered.code, *signed as u8)
        };
        let value = coerce_two_state(value, *two_state);
        return Ok(if let Some(index) = bit {
            let index = render_expr_impl(ctx, index)?.code;
            format!("llg_ref_write_bit({addr}, sv4_to_index({index}), {value});")
        } else {
            format!("llg_ref_write({addr}, {value});")
        });
    }
    // A real RHS is converted to the target's vector shape up front.
    let converted: Option<(String, u32, bool)> = if rhs.width == 0 {
        let (width, signed) = match lh {
            IrLhs::Whole(idx) => {
                let t = ctx.model.signal(*idx).ty;
                (t.width(), t.signed())
            }
            IrLhs::WholeRef { width, signed, .. } | IrLhs::Ref { width, signed, .. } => {
                (*width, *signed)
            }
            IrLhs::PackedSelect { steps, signed, .. } => {
                (steps.last().map_or(0, |step| step.width), *signed)
            }
            IrLhs::Bit(..) => (1, false),
            IrLhs::Part(_, left, right, _) => (((left - right).abs() + 1) as u32, false),
            IrLhs::IdxPart(_, _, _, width, _, _) => (*width, false),
            IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
                IrElemSel::Whole => {
                    let a = ctx.model.array(*arr);
                    (a.elem_width, a.signed)
                }
                IrElemSel::Part(left, right) => (((left - right).abs() + 1) as u32, false),
                IrElemSel::Bit(_) => (1, false),
                IrElemSel::Indexed { width, .. } => (*width, false),
                IrElemSel::PackedChain(steps) => (steps.last().map_or(0, |step| step.width), false),
            },
            IrLhs::Stream { width, .. } => (*width, false),
        };
        let rr = render_expr_impl(ctx, rhs)?;
        Some((rendered_to_vector(&rr, width, signed)?, width, signed))
    } else {
        None
    };
    // The effective RHS code/fill after conversion.
    let rr = render_expr_impl(ctx, rhs)?;
    let real_converted = converted.is_some();
    let rhs_code: String = converted
        .as_ref()
        .map(|(c, _, _)| c.clone())
        .unwrap_or_else(|| rr.code.clone());
    let rhs_fill: Option<u8> = if converted.is_some() { None } else { rr.fill };

    // Guarded array-element writes.
    if let IrLhs::ArrayElem {
        arr,
        indices,
        elem_sel,
    } = lh
    {
        let ai = ctx.model.array(*arr);
        if ai.real {
            if !matches!(elem_sel, IrElemSel::Whole) {
                return Err("select on a real array element is not supported".to_string());
            }
            let mut index_codes = Vec::with_capacity(indices.len());
            for index in indices {
                index_codes.push(render_expr_impl(ctx, index)?.code);
            }
            let (decls, condition, linear) = array_guard(ai, &index_codes)
                .unwrap_or_else(|| (String::new(), "1".to_string(), "0".to_string()));
            let rr = render_expr_impl(ctx, rhs)?;
            let value = round_shortreal(real_code(&rr), ai.shortreal);
            return Ok(format!(
                "{{ {decls} if ({condition}) llg_ba_d(&{}[({linear})], {value}); }}",
                ai.c_name
            ));
        }
        let call = "llg_ba";
        // Assignment padding follows the RHS's OWN signedness (LRM §10.7);
        // `sv4_cast` keys the extension off the value, so the tags stay the
        // pre-existing target shapes.  Fill literals keep the target shape
        // like every other fill context, and a real RHS arrives
        // pre-converted to the exact target shape.
        let resize = |w: u32, s: bool| -> String {
            if real_converted {
                return coerce_two_state(rhs_code.clone(), ai.two_state);
            }
            coerce_two_state(
                match rhs_fill {
                    Some(f) => format!("sv4_fill({f}, {w}, {})", s as u8),
                    None => format!("sv4_cast({}, {w}, {})", rhs_code, s as u8),
                },
                ai.two_state,
            )
        };
        let mut index_codes = Vec::with_capacity(indices.len());
        for i in indices {
            index_codes.push(render_expr_impl(ctx, i)?.code);
        }
        let base = ai.c_name.clone();
        let elem_target = |lin: &str| -> Result<(String, String), String> {
            let target = format!("&{base}[({lin})]");
            Ok(match elem_sel {
                IrElemSel::PackedChain(_) => {
                    return Err("packed selection chains require the owned emitter".to_owned());
                }
                IrElemSel::Whole => (target, resize(ai.elem_width, ai.signed)),
                IrElemSel::Part(l, r) => {
                    let w = ((l - r).abs() + 1) as u32;
                    let v = resize(w, false);
                    (
                        target,
                        format!(
                            "({{ sv4_t _t = {base}[({lin})]; \
                             sv4_part_select_set(&_t, {l}, {r}, {v}); _t; }})"
                        ),
                    )
                }
                IrElemSel::Bit(idx) => {
                    let v = resize(1, false);
                    let idx_code = render_expr_impl(ctx, idx)?.code;
                    (
                        target,
                        format!(
                            "({{ sv4_t _t = {base}[({lin})]; \
                             sv4_bit_select_set(&_t, sv4_to_index({idx_code}), {v}); _t; }})"
                        ),
                    )
                }
                IrElemSel::Indexed {
                    base: index,
                    width,
                    negative,
                } => {
                    let value = resize(*width, false);
                    let index = render_expr_impl(ctx, index)?.code;
                    let negative = *negative as u8;
                    (target, format!(
                        "({{ sv4_t _t = {base}[({lin})]; \
                         sv4_idx_part_select_set_value(&_t, {index}, {width}, {negative}, {value}); _t; }})"
                    ))
                }
            })
        };
        return Ok(match array_guard(ai, &index_codes) {
            Some((decls, cond, lin)) => {
                let (target, value) = elem_target(&lin)?;
                format!("{{ {decls}if ({cond}) {{ {call}({target}, {value}); }} }}")
            }
            None => {
                let (target, value) = elem_target("0")?;
                format!("{call}({target}, {value});")
            }
        });
    }

    // Collapsed-net members are driven through their driver slot.
    if let IrLhs::Whole(idx) = lh {
        let sig = ctx.model.signal(*idx);
        if let Some((gidx, slot)) = sig.net_driver {
            let net = &ctx.model.net_group(gidx).c_name;
            let value = if real_converted {
                rhs_code.clone()
            } else {
                match rhs_fill {
                    Some(f) => format!(
                        "sv4_fill({f}, {}, {})",
                        sig.ty.width(),
                        sig.ty.signed() as u8
                    ),
                    None => format!(
                        "sv4_cast({}, {}, {})",
                        rhs_code,
                        sig.ty.width(),
                        sig.ty.signed() as u8
                    ),
                }
            };
            return Ok(format!("llg_net_write(&{net}, {slot}, {value});"));
        }
    }
    // Each selected continuous-assignment site owns one driver. Rebuild its
    // value from Z so a moving index releases the previously selected bits.
    if let IrLhs::Bit(idx, ..) | IrLhs::Part(idx, ..) | IrLhs::IdxPart(idx, ..) = lh {
        let sig = ctx.model.signal(*idx);
        if let Some((gidx, slot)) = sig.net_driver {
            let net = &ctx.model.net_group(gidx).c_name;
            let selected_two_state = match lh {
                IrLhs::Bit(_, _, two_state)
                | IrLhs::Part(.., two_state)
                | IrLhs::IdxPart(.., two_state) => *two_state,
                _ => false,
            };
            let resize = |width: &str| {
                coerce_two_state(
                    match rhs_fill {
                        Some(fill) => format!("sv4_fill({fill}, {width}, 0)"),
                        None => format!("sv4_cast({rhs_code}, {width}, 0)"),
                    },
                    selected_two_state || sig.ty.two_state(),
                )
            };
            let update = match lh {
                IrLhs::Bit(_, index, _) => {
                    let index = render_expr_impl(ctx, index)?.code;
                    let value = resize("1");
                    format!("sv4_bit_select_set(&_t, sv4_to_index({index}), {value});")
                }
                IrLhs::Part(_, left, right, _) => {
                    let width = (i128::from(*left) - i128::from(*right)).unsigned_abs() + 1;
                    let value = resize(&width.to_string());
                    format!("sv4_part_select_set(&_t, {left}, {right}, {value});")
                }
                IrLhs::IdxPart(_, base, _, selected_width, neg, _) => {
                    let base = render_expr_impl(ctx, base)?.code;
                    let value = resize(&selected_width.to_string());
                    format!(
                        "sv4_idx_part_select_set_value(&_t, {base}, \
                         {selected_width}, {}, {value});",
                        *neg as u8
                    )
                }
                _ => unreachable!("only selected net targets enter this branch"),
            };
            return Ok(format!(
                "{{ sv4_t _t = sv4_fill(3, {}, {}); {update} \
                 llg_net_write(&{net}, {slot}, _t); }}",
                sig.ty.width(),
                sig.ty.signed() as u8
            ));
        }
    }

    let call = match lh {
        IrLhs::WholeRef { addr, .. } if addr.starts_with("llg_sequence_local_addr(") => {
            "llg_sequence_local_write"
        }
        _ => "llg_ba",
    };
    let two_state = match lh {
        IrLhs::Whole(idx) => ctx.model.signal(*idx).ty.two_state(),
        IrLhs::Bit(idx, _, selected_two_state)
        | IrLhs::Part(idx, .., selected_two_state)
        | IrLhs::IdxPart(idx, .., selected_two_state) => {
            ctx.model.signal(*idx).ty.two_state() || *selected_two_state
        }
        IrLhs::PackedSelect { two_state, .. }
        | IrLhs::WholeRef { two_state, .. }
        | IrLhs::Ref { two_state, .. } => *two_state,
        _ => false,
    };
    // Assignment padding follows the RHS's OWN signedness (LRM §10.7);
    // `sv4_cast` keys the extension off the value, so the tags stay the
    // pre-existing target shapes.  Fill literals keep the target shape like
    // every other fill context, and a real RHS arrives pre-converted to the
    // exact target shape.
    let resize = |code: &str, w: u32, s: bool| -> String {
        if real_converted {
            return coerce_two_state(code.to_string(), two_state);
        }
        coerce_two_state(
            match rhs_fill {
                Some(f) => format!("sv4_fill({f}, {w}, {})", s as u8),
                None => format!("sv4_cast({code}, {w}, {})", s as u8),
            },
            two_state,
        )
    };
    let args = match lh {
        IrLhs::PackedSelect { .. } => {
            return Err("packed activation selects require structured owned emission".to_owned())
        }
        IrLhs::Whole(idx) => {
            let sig = ctx.model.signal(*idx);
            format!(
                "&{}, {}",
                sig.c_name,
                resize(&rhs_code, sig.ty.width(), sig.ty.signed())
            )
        }
        IrLhs::WholeRef {
            addr,
            width,
            signed,
            ..
        } => format!("{addr}, {}", resize(&rhs_code, *width, *signed)),
        IrLhs::Ref { .. } => {
            return Err("reference assignment bypassed reference emission".to_owned())
        }
        IrLhs::Bit(idx, ie, _) => {
            let sig = ctx.model.signal(*idx);
            let v = resize(&rhs_code, 1, false);
            let ic = render_expr_impl(ctx, ie)?.code;
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_bit_select_set(&_t, sv4_to_index({}), {}); _t; }})",
                sig.c_name, sig.c_name, ic, v
            )
        }
        IrLhs::Part(idx, left, right, _) => {
            let sig = ctx.model.signal(*idx);
            let w = ((left - right).abs() + 1) as u32;
            let v = resize(&rhs_code, w, false);
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_part_select_set(&_t, {left}, {right}, {v}); _t; }})",
                sig.c_name, sig.c_name
            )
        }
        IrLhs::IdxPart(idx, be, _, selected_width, neg, _) => {
            let sig = ctx.model.signal(*idx);
            let bc = render_expr_impl(ctx, be)?.code;
            let v = if real_converted {
                rhs_code.clone()
            } else {
                match rhs_fill {
                    Some(f) => format!("sv4_fill({f}, {selected_width}, 0)"),
                    None => format!("sv4_cast({}, {selected_width}, 0)", rhs_code),
                }
            };
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_idx_part_select_set_value(&_t, {}, \
                 {}, {}, {}); _t; }})",
                sig.c_name,
                sig.c_name,
                bc,
                selected_width,
                *neg as u8,
                coerce_two_state(v, two_state)
            )
        }
        IrLhs::ArrayElem { .. } => unreachable!("array elements handled above"),
        IrLhs::Stream { .. } => unreachable!("streaming targets handled above"),
    };
    Ok(format!("{call}({args});"))
}
