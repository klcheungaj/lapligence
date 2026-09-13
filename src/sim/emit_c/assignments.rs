//! Deferred updates capture driver values or selected nonblocking writes.

use super::constants::round_shortreal;
use super::context::{RCtx, RenderedExpr};
use super::expressions::{array_guard, coerce_two_state, render_expr_impl};
use crate::sim::ir::{
    IrEdge, IrElemSel, IrExpr, IrExprKind, IrLhs, IrStreamDirection, IrTransitionDelay, IrType,
    IrWaitSrc,
};

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
    delay: IrTransitionDelay,
) -> Result<String, String> {
    let rendered = render_expr_impl(ctx, rhs)?;
    let delays = format!(
        "{}ULL, {}ULL, {}ULL",
        delay.rise, delay.fall, delay.turn_off
    );
    match lhs {
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            let value = packed_value(
                &rendered,
                signal.ty.width(),
                signal.ty.signed(),
                signal.ty.two_state(),
            );
            let call = if let Some((group, slot)) = signal.net_driver {
                let net = &ctx.model.net_group(group).c_name;
                format!("llg_inertial_net(&_driver, &{net}, {slot}, {value}, {delays})")
            } else {
                format!(
                    "llg_inertial_assign(&_driver, &{}, {value}, {delays})",
                    signal.c_name
                )
            };
            Ok(format!("{{ static llg_inertial_t* _driver; {call}; }}\n"))
        }
        IrLhs::Bit(index, _, selected_two_state)
        | IrLhs::Part(index, _, _, selected_two_state)
        | IrLhs::IdxPart(index, _, _, _, _, selected_two_state) => {
            render_selected_signal(ctx, *index, lhs, &rendered, &delays, *selected_two_state)
        }
        IrLhs::ArrayElem { .. } => render_selected_array(ctx, lhs, &rendered, &delays),
        _ => Err("inertial update requires a persistent packed target".into()),
    }
}

fn render_selected_signal(
    ctx: &RCtx<'_>,
    index: usize,
    lhs: &IrLhs,
    rendered: &RenderedExpr,
    delays: &str,
    selected_two_state: bool,
) -> Result<String, String> {
    let signal = ctx.model.signal(index);
    let (width, selected_width) = match lhs {
        IrLhs::Bit(..) => (signal.ty.width(), 1),
        IrLhs::Part(_, left, right, ..) => (signal.ty.width(), left.abs_diff(*right) as u32 + 1),
        IrLhs::IdxPart(_, _, _, selected_width, ..) => (signal.ty.width(), *selected_width),
        _ => unreachable!("selected signal renderer received a whole target"),
    };
    let value = packed_value(
        rendered,
        selected_width,
        false,
        signal.ty.two_state() || selected_two_state,
    );
    let mut declarations = String::new();
    let mut update = String::new();
    match lhs {
        IrLhs::Bit(_, select, ..) => {
            declarations.push_str(&format!(
                "sv4_t _llg_inertial_index = {}; ",
                render_expr_impl(ctx, select)?.code
            ));
            update.push_str(
                "sv4_bit_select_set(&_value, sv4_to_index(_llg_inertial_index), _rhs); \
                 sv4_bit_select_set(&_mask, sv4_to_index(_llg_inertial_index), sv4_fill(1, 1, 0)); ",
            );
        }
        IrLhs::Part(_, left, right, ..) => {
            update.push_str(&format!(
                "sv4_part_select_set(&_value, {left}, {right}, _rhs); \
                 sv4_part_select_set(&_mask, {left}, {right}, sv4_fill(1, {selected_width}, 0)); "
            ));
        }
        IrLhs::IdxPart(_, base, _, selected_width, negative, ..) => {
            declarations.push_str(&format!(
                "sv4_t _llg_inertial_index = {}; ",
                render_expr_impl(ctx, base)?.code
            ));
            update.push_str(&format!(
                "sv4_idx_part_select_set_value(&_value, _llg_inertial_index, \
                 {selected_width}, {}, _rhs); \
                 sv4_idx_part_select_set_value(&_mask, _llg_inertial_index, \
                 {selected_width}, {}, sv4_fill(1, {selected_width}, 0)); ",
                *negative as u8, *negative as u8
            ));
        }
        _ => unreachable!("selected signal renderer received an invalid target"),
    }
    let (current, call) = if let Some((group, slot)) = signal.net_driver {
        let net = &ctx.model.net_group(group).c_name;
        (
            format!("*{net}.drivers[{slot}]"),
            format!("llg_inertial_selected_net(&_driver, &{net}, {slot}, _value, _mask, {delays})"),
        )
    } else {
        (
            signal.c_name.clone(),
            format!(
                "llg_inertial_selected_assign(&_driver, &{}, _value, _mask, {delays})",
                signal.c_name
            ),
        )
    };
    Ok(format!(
        "{{ static llg_inertial_t* _driver; {declarations} sv4_t _value = {current}; \
         sv4_t _mask = sv4_fill(0, {width}, {}); sv4_t _rhs = {value}; {update} {call}; }}\n",
        signal.ty.signed() as u8
    ))
}

fn render_selected_array(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rendered: &RenderedExpr,
    delays: &str,
) -> Result<String, String> {
    let IrLhs::ArrayElem {
        arr,
        indices,
        elem_sel,
    } = lhs
    else {
        unreachable!("array renderer received a non-array target")
    };
    let array = ctx.model.array(*arr);
    if array.real {
        return Err("inertial update requires a packed array target".into());
    }
    let mut index_codes = Vec::with_capacity(indices.len());
    for index in indices {
        index_codes.push(render_expr_impl(ctx, index)?.code);
    }
    let (decls, condition, linear) =
        array_guard(array, &index_codes).unwrap_or_else(|| (String::new(), "1".into(), "0".into()));
    let (selected_width, mut selector_decls, mut update) = match elem_sel {
        IrElemSel::Whole => (
            array.elem_width,
            String::new(),
            format!(
                "sv4_t _mask = sv4_fill(1, {}, 0); _value = _rhs;",
                array.elem_width
            ),
        ),
        IrElemSel::Part(left, right) => {
            let width = left.abs_diff(*right) as u32 + 1;
            (
                width,
                String::new(),
                format!(
                    "sv4_t _mask = sv4_fill(0, {}, 0); \
                     sv4_part_select_set(&_value, {left}, {right}, _rhs); \
                     sv4_part_select_set(&_mask, {left}, {right}, sv4_fill(1, {width}, 0));",
                    array.elem_width
                ),
            )
        }
        IrElemSel::Bit(index) => {
            let code = render_expr_impl(ctx, index)?.code;
            (
                1,
                format!("sv4_t _llg_inertial_index = {code};"),
                format!(
                    "sv4_t _mask = sv4_fill(0, {}, 0); \
                     sv4_bit_select_set(&_value, sv4_to_index(_llg_inertial_index), _rhs); \
                     sv4_bit_select_set(&_mask, sv4_to_index(_llg_inertial_index), sv4_fill(1, 1, 0));",
                    array.elem_width
                ),
            )
        }
        IrElemSel::Indexed {
            base,
            width,
            negative,
        } => {
            let code = render_expr_impl(ctx, base)?.code;
            (
                *width,
                format!("sv4_t _llg_inertial_index = {code};"),
                format!(
                    "sv4_t _mask = sv4_fill(0, {}, 0); \
                     sv4_idx_part_select_set_value(&_value, _llg_inertial_index, {width}, {}, _rhs); \
                     sv4_idx_part_select_set_value(&_mask, _llg_inertial_index, {width}, {}, sv4_fill(1, {width}, 0));",
                    array.elem_width, *negative as u8, *negative as u8
                ),
            )
        }
    };
    let value = packed_value(
        rendered,
        selected_width,
        matches!(elem_sel, IrElemSel::Whole) && array.signed,
        array.two_state,
    );
    let target = format!("{}[({linear})]", array.c_name);
    // A dynamic array selector changes the target address between evaluations.
    // Keep one inertial handle per flattened element so a new update to `a[i]`
    // cancels only that element's pending event; switching to `a[j]` must not
    // discard an independently pending update for `a[i]`.
    let call = format!(
        "llg_inertial_selected_assign(&_drivers[({linear})], &{target}, _value, _mask, {delays})"
    );
    if matches!(elem_sel, IrElemSel::Whole) {
        selector_decls.push(' ');
    }
    update = format!("sv4_t _rhs = {value}; {update}");
    Ok(format!(
        "{{ {decls} if ({condition}) {{ static llg_inertial_t* _drivers[{}]; \
         {selector_decls} sv4_t _value = {target}; {update} {call}; }} }}\n",
        array.total
    ))
}

fn store(target: &str, value: &str, ticks: &str, clocking: bool) -> String {
    if clocking {
        format!(
            "llg_clocking_nba_sync_after({target}, {value}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);"
        )
    } else if ticks == "0ULL" {
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
    net: Option<(&str, usize)>,
    clocking: bool,
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
    let call = net.map_or_else(
        || {
            if clocking {
                format!(
                    "llg_clocking_nba_sync_masked_after({target}, _value, _mask, {ticks}, _clocking_drive_sources, _clocking_drive_source_count)"
                )
            } else {
                format!("llg_nba_masked({target}, _value, _mask, {ticks})")
            }
        },
        |(net, slot)| {
            if clocking {
                format!(
                    "llg_clocking_nba_net_sync_masked_after(&{net}, {slot}, _value, _mask, {ticks}, _clocking_drive_sources, _clocking_drive_source_count)"
                )
            } else {
                unreachable!("ordinary nonblocking assignments cannot target net drivers")
            }
        },
    );
    format!("{{ sv4_t _rhs=sv4_cast({value}, {width}, 0); {index} sv4_t _value=sv4_fill(0, {storage_width}, 0); sv4_t _mask=_value; {update_value} {update_mask} {call}; }}")
}

pub(super) fn render_nba(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rhs: &IrExpr,
    ticks: &str,
) -> Result<String, String> {
    render_nba_inner(ctx, lhs, rhs, ticks, false)
}

fn clocking_drive_sources_text(
    ctx: &RCtx<'_>,
    specs: &[(IrWaitSrc, IrEdge)],
) -> Result<String, String> {
    if specs.is_empty() {
        return Err("clocking drive has no associated clocking event".to_owned());
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => Ok(format!(
                "{{ .sig = &{name}, .kind = {}, .ev = NULL }}",
                edge_kind(edge)
            )),
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .sig = NULL, .kind = {}, .ev = {} }}",
                edge_kind(edge),
                super::statements::event_ref_code(ctx, event)?
            )),
            IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. }
            | IrWaitSrc::Real(_) => {
                Err("clocking drive requires a simple signal or named-event clocking event".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "llg_wait_src_t _clocking_drive_sources[] = {{{}}}; int _clocking_drive_source_count = {};",
        entries.join(", "),
        entries.len()
    ))
}

pub(super) fn render_clocking_nba(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rhs: &IrExpr,
    ticks: &str,
    specs: &[(IrWaitSrc, IrEdge)],
) -> Result<String, String> {
    let sources = clocking_drive_sources_text(ctx, specs)?;
    let assignment = render_nba_inner(ctx, lhs, rhs, ticks, true)?;
    Ok(format!("{{ {sources} {assignment} }}"))
}

fn render_nba_inner(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    rhs: &IrExpr,
    ticks: &str,
    allow_net: bool,
) -> Result<String, String> {
    if matches!(lhs, IrLhs::Ref { .. }) {
        return Err("nonblocking assignment through a ref formal is not supported".into());
    }
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
            text.push_str(&render_nba_inner(ctx, part, &expression, ticks, allow_net)?);
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
        if array.real {
            if !matches!(elem_sel, IrElemSel::Whole) {
                return Err("select on a real array element is not supported".to_string());
            }
            let indices = indices
                .iter()
                .map(|index| render_expr_impl(ctx, index).map(|value| value.code))
                .collect::<Result<Vec<_>, _>>()?;
            let (decls, condition, linear) = array_guard(array, &indices)
                .unwrap_or_else(|| (String::new(), "1".into(), "0".into()));
            let real = if value.width == 0 {
                value.code
            } else {
                format!("sv4_to_real({})", value.code)
            };
            let real = super::constants::round_shortreal(real, array.shortreal);
            let call = if ticks == "0ULL" {
                if allow_net {
                    format!(
                        "llg_clocking_nba_d_sync_after(&{}[({linear})], {real}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);",
                        array.c_name
                    )
                } else {
                    format!("llg_nba_d(&{}[({linear})], {real});", array.c_name)
                }
            } else {
                if allow_net {
                    format!(
                        "llg_clocking_nba_d_sync_after(&{}[({linear})], {real}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);",
                        array.c_name
                    )
                } else {
                    format!(
                        "llg_nba_d_after(&{}[({linear})], {real}, {ticks});",
                        array.c_name
                    )
                }
            };
            return Ok(format!("{{ {decls} if ({condition}) {{ {call} }} }}"));
        }
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
            IrElemSel::Whole => store(&target, "_array_rhs", ticks, allow_net),
            IrElemSel::Part(left, right) => selected_store(
                &target,
                array.elem_width,
                Select::Part(*left, *right),
                "_array_rhs",
                ticks,
                None,
                allow_net,
            ),
            IrElemSel::Bit(index) => selected_store(
                &target,
                array.elem_width,
                Select::Bit(render_expr_impl(ctx, index)?.code),
                "_array_rhs",
                ticks,
                None,
                allow_net,
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
                None,
                allow_net,
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
        shortreal,
    } = lhs
    {
        if *width == 0 {
            let real = if value.width == 0 {
                value.code.clone()
            } else {
                format!("sv4_to_real({})", value.code)
            };
            let real = super::constants::round_shortreal(real, *shortreal);
            return Ok(if allow_net {
                format!(
                    "llg_clocking_nba_d_sync_after({addr}, {real}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);"
                )
            } else if ticks == "0ULL" {
                format!("llg_nba_d({addr}, {real});")
            } else {
                format!("llg_nba_d_after({addr}, {real}, {ticks});")
            });
        }
        return Ok(store(
            addr,
            &packed_value(&value, *width, *signed, *two_state),
            ticks,
            allow_net,
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
    let net = signal
        .net_driver
        .map(|(group, slot)| (ctx.model.net_group(group).c_name.as_str(), slot));
    if net.is_some() && !allow_net {
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
        let call = if allow_net {
            "llg_clocking_nba_d_sync_after"
        } else if ticks == "0ULL" {
            "llg_nba_d"
        } else {
            "llg_nba_d_after"
        };
        return Ok(if allow_net {
            format!(
                "{call}({target}, {value}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);"
            )
        } else if ticks != "0ULL" {
            format!("{call}({target}, {value}, {ticks});")
        } else {
            format!("{call}({target}, {value});")
        });
    }
    let signed = select.is_none() && signal.ty.signed();
    let value = packed_value(
        &value,
        width,
        signed,
        signal.ty.two_state() || selected_two_state,
    );
    if let Some((net, slot)) = net {
        return Ok(match select {
            None => {
                if allow_net {
                    format!(
                        "llg_clocking_nba_net_sync_after(&{net}, {slot}, {value}, {ticks}, _clocking_drive_sources, _clocking_drive_source_count);"
                    )
                } else {
                    unreachable!("ordinary nonblocking assignments cannot target net drivers")
                }
            }
            Some(select) => selected_store(
                &target,
                signal.ty.width(),
                select,
                &value,
                ticks,
                Some((net, slot)),
                allow_net,
            ),
        });
    }
    Ok(match select {
        None => store(&target, &value, ticks, allow_net),
        Some(select) => selected_store(
            &target,
            signal.ty.width(),
            select,
            &value,
            ticks,
            net,
            allow_net,
        ),
    })
}
