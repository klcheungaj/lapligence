//! Deferred updates capture driver values or selected nonblocking writes.

use super::constants::round_shortreal;
use super::context::{RCtx, RenderedExpr};
use super::expressions::{array_guard, coerce_two_state, render_expr_impl};
use crate::sim::ir::{IrElemSel, IrExpr, IrExprKind, IrLhs, IrStreamDirection, IrType};

enum Select {
    Bit(String),
    Part(i64, i64),
    Indexed(String, u32, bool),
}

fn packed_value(value: &RenderedExpr, width: u32, signed: bool, two_state: bool) -> String {
    let signed = signed as u8;
    coerce_two_state(
        if value.width == 0 {
            format!("sv4_from_real({}, {width}, {signed})", value.code)
        } else if let Some(fill) = value.fill {
            format!("sv4_fill({fill}, {width}, {signed})")
        } else {
            format!("sv4_cast({}, {width}, {signed})", value.code)
        },
        two_state,
    )
}

pub(super) fn render_inertial(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rhs: &IrExpr,
    ticks: u64,
) -> Result<String, String> {
    let IrLhs::Whole(index) = lhs else {
        return Err("inertial update requires a whole packed driver".into());
    };
    let signal = ctx.model.signal(*index);
    let value = packed_value(
        &render_expr_impl(ctx, rhs)?,
        signal.ty.width(),
        signal.ty.signed(),
        signal.ty.two_state(),
    );
    let call = if let Some((group, slot)) = signal.net_driver {
        let net = &ctx.model.net_group(group).c_name;
        format!("llg_inertial_net(&_driver, &{net}, {slot}, {value}, {ticks}ULL)")
    } else {
        format!(
            "llg_inertial_assign(&_driver, &{}, {value}, {ticks}ULL)",
            signal.c_name
        )
    };
    Ok(format!("{{ static llg_inertial_t* _driver; {call}; }}\n"))
}

fn store(target: &str, value: &str, ticks: &str) -> String {
    if ticks == "0ULL" {
        format!("llg_nba({target}, {value});")
    } else {
        format!("llg_nba_after({target}, {value}, {ticks});")
    }
}

fn selected_store(
    target: &str,
    storage_width: u32,
    selected: Select,
    value: &str,
    ticks: &str,
) -> String {
    let (index, width, update_value, update_mask) = match selected {
        Select::Bit(index) => (
            format!("sv4_t _index={index};"), 1,
            "sv4_bit_select_set(&_value, sv4_to_index(_index), _rhs);".to_owned(),
            "sv4_bit_select_set(&_mask, sv4_to_index(_index), sv4_fill(1, 1, 0));".to_owned(),
        ),
        Select::Part(left, right) => {
            let width = left.abs_diff(right) as u32 + 1;
            (String::new(), width,
             format!("sv4_part_select_set(&_value, {left}, {right}, _rhs);"),
             format!("sv4_part_select_set(&_mask, {left}, {right}, sv4_fill(1, {width}, 0));"))
        }
        Select::Indexed(index, width, negative) => (
            format!("sv4_t _index={index};"), width,
            format!("sv4_idx_part_select_set_value(&_value, _index, {width}, {}, _rhs);", negative as u8),
            format!("sv4_idx_part_select_set_value(&_mask, _index, {width}, {}, sv4_fill(1, {width}, 0));", negative as u8),
        ),
    };
    format!("{{ sv4_t _rhs=sv4_cast({value}, {width}, 0); {index} sv4_t _value=sv4_fill(0, {storage_width}, 0); sv4_t _mask=_value; {update_value} {update_mask} llg_nba_masked({target}, _value, _mask, {ticks}); }}")
}

pub(super) fn render_nba(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rhs: &IrExpr,
    ticks: &str,
) -> Result<String, String> {
    let value = render_expr_impl(ctx, rhs)?;
    if let IrLhs::Stream {
        parts,
        width,
        slice,
        direction,
    } = lhs
    {
        let value = packed_value(&value, *width, false, false);
        let mut text = format!(
            "{{ sv4_t _stream_value=sv4_unstream({value}, {slice}, {});",
            matches!(direction, IrStreamDirection::RightToLeft) as u8
        );
        let mut cursor = *width;
        for (part, width) in parts {
            let right = cursor - width;
            let expression = IrExpr::new(
                IrExprKind::Verbatim {
                    code: format!("sv4_part_select(_stream_value, {}, {right})", cursor - 1),
                    width: *width,
                    signed: false,
                },
                *width,
                false,
                None,
            );
            text.push_str(&render_nba(ctx, part, &expression, ticks)?);
            cursor = right;
        }
        text.push('}');
        return Ok(text);
    }
    if let IrLhs::ArrayElem {
        arr,
        indices,
        elem_sel,
    } = lhs
    {
        let array = ctx.model.array(*arr);
        let (width, signed) = match elem_sel {
            IrElemSel::Whole => (array.elem_width, array.signed),
            IrElemSel::Bit(_) => (1, false),
            IrElemSel::Indexed { width, .. } => (*width, false),
            IrElemSel::Part(left, right) => (left.abs_diff(*right) as u32 + 1, false),
        };
        let rhs = packed_value(&value, width, signed, array.two_state);
        let indices = indices
            .iter()
            .map(|index| render_expr_impl(ctx, index).map(|value| value.code))
            .collect::<Result<Vec<_>, _>>()?;
        let (decls, condition, linear) =
            array_guard(array, &indices).unwrap_or_else(|| (String::new(), "1".into(), "0".into()));
        let target = format!("&{}[({linear})]", array.c_name);
        let assignment = match elem_sel {
            IrElemSel::Whole => store(&target, "_array_rhs", ticks),
            IrElemSel::Part(left, right) => selected_store(
                &target,
                array.elem_width,
                Select::Part(*left, *right),
                "_array_rhs",
                ticks,
            ),
            IrElemSel::Bit(index) => selected_store(
                &target,
                array.elem_width,
                Select::Bit(render_expr_impl(ctx, index)?.code),
                "_array_rhs",
                ticks,
            ),
            IrElemSel::Indexed {
                base,
                width,
                negative,
            } => selected_store(
                &target,
                array.elem_width,
                Select::Indexed(render_expr_impl(ctx, base)?.code, *width, *negative),
                "_array_rhs",
                ticks,
            ),
        };
        return Ok(format!(
            "{{ sv4_t _array_rhs={rhs}; {decls} if ({condition}) {{ {assignment} }} }}"
        ));
    }
    if let IrLhs::WholeRef {
        addr,
        width,
        signed,
        two_state,
    } = lhs
    {
        return Ok(store(
            addr,
            &packed_value(&value, *width, *signed, *two_state),
            ticks,
        ));
    }
    let (index, select, width, selected_two_state) = match lhs {
        IrLhs::Whole(index) => (*index, None, ctx.model.signal(*index).ty.width(), false),
        IrLhs::Bit(index, bit, two_state) => (
            *index,
            Some(Select::Bit(render_expr_impl(ctx, bit)?.code)),
            1,
            *two_state,
        ),
        IrLhs::Part(index, left, right, two_state) => (
            *index,
            Some(Select::Part(*left, *right)),
            left.abs_diff(*right) as u32 + 1,
            *two_state,
        ),
        IrLhs::IdxPart(index, base, _, width, negative, two_state) => (
            *index,
            Some(Select::Indexed(
                render_expr_impl(ctx, base)?.code,
                *width,
                *negative,
            )),
            *width,
            *two_state,
        ),
        _ => unreachable!("other assignment targets handled above"),
    };
    let signal = ctx.model.signal(index);
    if signal.net_driver.is_some() {
        return Err("nonblocking assignment to a net member is not supported".into());
    }
    let target = format!("&{}", signal.c_name);
    if let IrType::Real { shortreal } = signal.ty {
        let value = if value.width == 0 {
            value.code
        } else {
            format!("sv4_to_real({})", value.code)
        };
        let value = round_shortreal(value, shortreal);
        return Ok(if ticks == "0ULL" {
            format!("llg_nba_d({target}, {value});")
        } else {
            format!("llg_nba_d_after({target}, {value}, {ticks});")
        });
    }
    let signed = select.is_none() && signal.ty.signed();
    let value = packed_value(
        &value,
        width,
        signed,
        signal.ty.two_state() || selected_two_state,
    );
    Ok(match select {
        None => store(&target, &value, ticks),
        Some(select) => selected_store(&target, signal.ty.width(), select, &value, ticks),
    })
}
