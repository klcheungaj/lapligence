//! Expression, assignment-target, array-access, and call rendering.

use super::constants::{emit_const, round_shortreal};
use super::context::{RCtx, RenderedExpr};
use super::EmitError;
use crate::sim::ir::{
    IrBinOp, IrBitQuery, IrCallArg, IrContainerElement, IrContainerKind, IrElemSel, IrExpr,
    IrExprKind, IrInsideItem, IrLhs, IrRealBinOp, IrRealUnOp, IrStreamDirection, IrStringExpr,
    IrSysFunc, IrType, IrUnOp,
};

/// The real-value code of a rendered operand: bare for real expressions,
/// `sv4_to_real(...)` for packed ones.
pub(super) fn real_code(e: &RenderedExpr) -> String {
    if e.width == 0 {
        e.code.clone()
    } else {
        format!("sv4_to_real({})", e.code)
    }
}

/// The boolean code of a rendered operand (`sv4_to_bool` / `llg_real_to_bool`).
pub(super) fn bool_code(e: &RenderedExpr) -> String {
    if e.width == 0 {
        format!("llg_real_to_bool({})", e.code)
    } else {
        format!("sv4_to_bool({})", e.code)
    }
}

/// Value-preserving conversion of a rendered value to `(width, signed)`:
/// `sv4_cast` keys the extension off the SOURCE's signedness (LRM 1800-2009
/// §6.24.1 / §10.7 — an unsigned source zero-extends even into a signed
/// target and vice versa).  Kept next to its users in this module because it
/// owns the exact C shapes.
pub(crate) fn arg_resize(code: &str, width: u32, signed: bool) -> String {
    format!("sv4_cast({code}, {width}, {})", signed as u8)
}

/// Render one IR expression to C, reproducing the pre-IR emitter's text
/// decision-for-decision from the recorded widths/signedness/fills.
pub fn render_expr(ctx: &RCtx<'_>, e: &IrExpr) -> Result<RenderedExpr, EmitError> {
    super::check_capacity(
        ctx.model
            .expression_capacity(e, ctx.func)
            .map_err(EmitError::InvalidIr)?,
    )?;
    render_expr_impl(ctx, e).map_err(EmitError::new)
}

pub(super) fn render_expr_impl(ctx: &RCtx<'_>, e: &IrExpr) -> Result<RenderedExpr, String> {
    super::check_capacity(u128::from(e.width)).map_err(|error| error.to_string())?;
    let w = |x: &IrExpr| render_expr_impl(ctx, x);
    let out = match &e.kind {
        IrExprKind::Container(operation) => RenderedExpr {
            code: super::containers::expression(ctx, operation)?,
            width: e.width,
            signed: e.signed,
            fill: None,
        },
        IrExprKind::ObjectQuery(query) => RenderedExpr {
            code: super::objects::query(ctx, query, e.width, e.signed)?,
            width: e.width,
            signed: e.signed,
            fill: None,
        },
        IrExprKind::Const(c) => RenderedExpr {
            code: emit_const(c),
            width: c.width,
            signed: c.signed,
            fill: c.fill,
        },
        IrExprKind::SigRead(idx) => {
            let s = ctx.model.signal(*idx);
            RenderedExpr {
                code: s.c_name.clone(),
                width: s.ty.width(),
                signed: s.ty.signed(),
                fill: None,
            }
        }
        IrExprKind::LocalRead(name) => RenderedExpr {
            code: name.clone(),
            width: e.width,
            signed: e.signed,
            fill: None,
        },
        IrExprKind::FormalRead(idx) => {
            let f = ctx
                .func
                .ok_or_else(|| "internal: formal read outside a function context".to_string())?;
            let form = f
                .formals
                .get(*idx)
                .ok_or_else(|| "internal: formal read out of range".to_string())?;
            let code = if form.real {
                if form.is_ref() {
                    return Err("real ref formal is not supported".to_string());
                }
                if form.is_out {
                    round_shortreal(format!("*o{idx}"), form.shortreal)
                } else {
                    round_shortreal(format!("a{idx}"), form.shortreal)
                }
            } else if form.is_ref() {
                format!("llg_ref_read(r{idx})")
            } else if form.is_out {
                format!("sv4_resize(*o{idx}, {}, {})", form.width, form.signed as u8)
            } else {
                format!("a{idx}")
            };
            RenderedExpr {
                code,
                width: form.width,
                signed: form.signed,
                fill: None,
            }
        }
        IrExprKind::CallFn(call) => render_call_expr(ctx, call)?,
        IrExprKind::EventTriggered(event) => {
            let event = super::statements::event_ref_code(ctx, event)?;
            RenderedExpr {
                code: format!("sv4_from_u64(llg_event_triggered({event}) ? 1ULL : 0ULL, 1, 0)"),
                width: 1,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::Mutation(mutation) => render_mutation_expr(ctx, e, mutation)?,
        IrExprKind::Bin { op, a, b } => {
            let ra = w(a)?;
            let rb = w(b)?;
            let real = ra.width == 0 || rb.width == 0;
            let code = match op {
                IrBinOp::Add => format!("sv4_add({}, {})", ra.code, rb.code),
                IrBinOp::Sub => format!("sv4_sub({}, {})", ra.code, rb.code),
                IrBinOp::Mul => format!("sv4_mul({}, {})", ra.code, rb.code),
                IrBinOp::Div => format!("sv4_div({}, {})", ra.code, rb.code),
                IrBinOp::Mod => format!("sv4_mod({}, {})", ra.code, rb.code),
                IrBinOp::Pow => format!("sv4_pow({}, {})", ra.code, rb.code),
                IrBinOp::BitAnd => format!("sv4_and({}, {})", ra.code, rb.code),
                IrBinOp::BitOr => format!("sv4_or({}, {})", ra.code, rb.code),
                IrBinOp::BitXor => format!("sv4_xor({}, {})", ra.code, rb.code),
                IrBinOp::BitXNor => format!("sv4_xnor({}, {})", ra.code, rb.code),
                IrBinOp::LogAnd => {
                    if real {
                        format!(
                            "sv4_from_u64(({} && {}) ? 1ULL : 0ULL, 1, 0)",
                            bool_code(&ra),
                            bool_code(&rb)
                        )
                    } else {
                        // Evaluate the left operand once. X/Z is not a
                        // definite false: evaluate the right side in that case
                        // so the four-state truth table can resolve X && 0.
                        format!(
                            "({{ sv4_t _llg_logic_left = {}; \
                             (!sv4_to_bool(_llg_logic_left) && !sv4_is_unknown(_llg_logic_left)) \
                             ? sv4_from_u64(0, 1, 0) : sv4_logand(_llg_logic_left, {}); }})",
                            ra.code, rb.code
                        )
                    }
                }
                IrBinOp::LogOr => {
                    if real {
                        format!(
                            "sv4_from_u64(({} || {}) ? 1ULL : 0ULL, 1, 0)",
                            bool_code(&ra),
                            bool_code(&rb)
                        )
                    } else {
                        // Case-item chains also use logical OR. Calling
                        // sv4_logor eagerly evaluates later item expressions,
                        // even when an earlier item already matched.
                        format!(
                            "({{ sv4_t _llg_logic_left = {}; \
                             sv4_to_bool(_llg_logic_left) ? sv4_from_u64(1, 1, 0) \
                             : sv4_logor(_llg_logic_left, {}); }})",
                            ra.code, rb.code
                        )
                    }
                }
                IrBinOp::Eq => cmp_expr(&ra, &rb, "=="),
                IrBinOp::Neq => cmp_expr(&ra, &rb, "!="),
                IrBinOp::Lt => cmp_expr(&ra, &rb, "<"),
                IrBinOp::Le => cmp_expr(&ra, &rb, "<="),
                IrBinOp::Gt => cmp_expr(&ra, &rb, ">"),
                IrBinOp::Ge => cmp_expr(&ra, &rb, ">="),
                IrBinOp::CaseEq => format!("sv4_case_eq({}, {})", ra.code, rb.code),
                IrBinOp::CaseNeq => format!("sv4_case_neq({}, {})", ra.code, rb.code),
                IrBinOp::WildEq => format!("sv4_wild_eq({}, {})", ra.code, rb.code),
                IrBinOp::WildNeq => format!("sv4_wild_neq({}, {})", ra.code, rb.code),
                IrBinOp::Shl => format!("sv4_shl({}, {})", ra.code, rb.code),
                IrBinOp::Shr => format!("sv4_shr({}, {})", ra.code, rb.code),
                IrBinOp::Ashl => format!("sv4_ashl({}, {})", ra.code, rb.code),
                IrBinOp::Ashr => format!("sv4_ashr({}, {})", ra.code, rb.code),
            };
            RenderedExpr {
                code,
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::Un { op, a } => {
            let ra = w(a)?;
            let code = match op {
                IrUnOp::Neg => format!("sv4_neg({})", ra.code),
                IrUnOp::LogNot => {
                    if ra.width == 0 {
                        format!(
                            "sv4_from_u64(llg_real_to_bool({}) ? 0ULL : 1ULL, 1, 0)",
                            ra.code
                        )
                    } else {
                        format!("sv4_lognot({})", ra.code)
                    }
                }
                IrUnOp::BitNeg => format!("sv4_bitneg({})", ra.code),
                IrUnOp::RedAnd => format!("sv4_reduce_and({})", ra.code),
                IrUnOp::RedNand => format!("sv4_reduce_nand({})", ra.code),
                IrUnOp::RedOr => format!("sv4_reduce_or({})", ra.code),
                IrUnOp::RedNor => format!("sv4_reduce_nor({})", ra.code),
                IrUnOp::RedXor => format!("sv4_reduce_xor({})", ra.code),
                IrUnOp::RedXNor => format!("sv4_reduce_xnor({})", ra.code),
            };
            RenderedExpr {
                code,
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::Mux { sel, a, b } => {
            let rsel = w(sel)?;
            let ra = w(a)?;
            let rb = w(b)?;
            let code = if ra.width == 0 || rb.width == 0 {
                format!(
                    "({} ? {} : {})",
                    bool_code(&rsel),
                    real_code(&ra),
                    real_code(&rb)
                )
            } else if rsel.width == 0 {
                format!("({} ? {} : {})", bool_code(&rsel), ra.code, rb.code)
            } else {
                format!("sv4_mux({}, {}, {})", rsel.code, ra.code, rb.code)
            };
            RenderedExpr {
                code,
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::Concat { parts } => {
            let mut parts_r = Vec::with_capacity(parts.len());
            for p in parts {
                parts_r.push(w(p)?);
            }
            let mut code = parts_r[0].code.clone();
            for p in &parts_r[1..] {
                code = format!("sv4_concat({code}, {})", p.code);
            }
            RenderedExpr {
                code,
                width: e.width,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::Replicate { count, parts } => {
            let mut parts_r = Vec::with_capacity(parts.len());
            for p in parts {
                parts_r.push(w(p)?);
            }
            let mut pat = parts_r[0].code.clone();
            for p in &parts_r[1..] {
                pat = format!("sv4_concat({pat}, {})", p.code);
            }
            RenderedExpr {
                code: format!("sv4_repeat({pat}, {count})"),
                width: e.width,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::Stream {
            value,
            slice,
            direction,
        } => {
            let value = w(value)?;
            RenderedExpr {
                code: format!(
                    "sv4_stream({}, {slice}, {})",
                    value.code,
                    matches!(direction, IrStreamDirection::RightToLeft) as u8
                ),
                width: e.width,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::Inside { value, items } => {
            let value = w(value)?;
            let value_local = RenderedExpr {
                code: "_inside_value".to_owned(),
                width: value.width,
                signed: value.signed,
                fill: None,
            };
            let value_type = if value.width == 0 { "double" } else { "sv4_t" };
            let mut code = format!(
                "({{ {value_type} _inside_value = {}; sv4_t _inside_result = SV4_C(0, 1); ",
                value.code
            );
            for (index, item) in items.iter().enumerate() {
                match item {
                    IrInsideItem::Value(item) => {
                        let item = w(item)?;
                        let item_type = if item.width == 0 { "double" } else { "sv4_t" };
                        let item_name = format!("_inside_item_{index}");
                        let item_local = RenderedExpr {
                            code: item_name.clone(),
                            width: item.width,
                            signed: item.signed,
                            fill: None,
                        };
                        let comparison = if value.width == 0 || item.width == 0 {
                            cmp_expr(&value_local, &item_local, "==")
                        } else {
                            format!("sv4_wild_eq(_inside_value, {item_name})")
                        };
                        code.push_str(&format!(
                            "{item_type} {item_name} = {}; \
                             _inside_result = sv4_logor(_inside_result, {comparison}); ",
                            item.code
                        ));
                    }
                    IrInsideItem::Range { low, high } => {
                        let low = w(low)?;
                        let high = w(high)?;
                        let low_type = if low.width == 0 { "double" } else { "sv4_t" };
                        let high_type = if high.width == 0 { "double" } else { "sv4_t" };
                        let low_local = RenderedExpr {
                            code: format!("_inside_low_{index}"),
                            width: low.width,
                            signed: low.signed,
                            fill: None,
                        };
                        let high_local = RenderedExpr {
                            code: format!("_inside_high_{index}"),
                            width: high.width,
                            signed: high.signed,
                            fill: None,
                        };
                        let comparison = if value.width == 0 || low.width == 0 || high.width == 0 {
                            format!(
                                "sv4_logand({}, {})",
                                cmp_expr(&value_local, &low_local, ">="),
                                cmp_expr(&value_local, &high_local, "<=")
                            )
                        } else {
                            format!(
                                "sv4_inside_range(_inside_value, _inside_low_{index}, _inside_high_{index})"
                            )
                        };
                        code.push_str(&format!(
                            "{low_type} _inside_low_{index} = {}; \
                             {high_type} _inside_high_{index} = {}; \
                             _inside_result = sv4_logor(_inside_result, {comparison}); ",
                            low.code, high.code
                        ));
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        let low = low.as_ref().map(w).transpose()?;
                        let high = high.as_ref().map(w).transpose()?;
                        let mut comparisons = Vec::new();
                        if let Some(low) = &low {
                            let low_type = if low.width == 0 { "double" } else { "sv4_t" };
                            let low_local = RenderedExpr {
                                code: format!("_inside_low_{index}"),
                                width: low.width,
                                signed: low.signed,
                                fill: None,
                            };
                            comparisons.push(cmp_expr(&value_local, &low_local, ">="));
                            code.push_str(&format!(
                                "{low_type} _inside_low_{index} = {}; ",
                                low.code
                            ));
                        }
                        if let Some(high) = &high {
                            let high_type = if high.width == 0 { "double" } else { "sv4_t" };
                            let high_local = RenderedExpr {
                                code: format!("_inside_high_{index}"),
                                width: high.width,
                                signed: high.signed,
                                fill: None,
                            };
                            comparisons.push(cmp_expr(&value_local, &high_local, "<="));
                            code.push_str(&format!(
                                "{high_type} _inside_high_{index} = {}; ",
                                high.code
                            ));
                        }
                        let comparison = comparisons
                            .into_iter()
                            .reduce(|left, right| format!("sv4_logand({left}, {right})"))
                            .ok_or_else(|| "inside open range has no endpoint".to_owned())?;
                        code.push_str(&format!(
                            "_inside_result = sv4_logor(_inside_result, {comparison}); "
                        ));
                    }
                    IrInsideItem::Container { container } => {
                        let container_model =
                            ctx.model.containers.get(*container).ok_or_else(|| {
                                "inside container index is out of bounds".to_owned()
                            })?;
                        let IrContainerElement::Packed { width, signed, .. } =
                            &container_model.element
                        else {
                            return Err("inside container element must be packed".to_owned());
                        };
                        let name = &container_model.c_name;
                        let loop_index = format!("_inside_index_{index}");
                        let (size_fn, get_fn, index_arg) = match &container_model.kind {
                            IrContainerKind::Dynamic => (
                                "llg_dyn_size",
                                "llg_dyn_get",
                                format!("sv4_from_u64((uint64_t){loop_index}, 32, 1)"),
                            ),
                            IrContainerKind::Queue { .. } => (
                                "llg_queue_size",
                                "llg_queue_get",
                                format!("sv4_from_u64((uint64_t){loop_index}, 32, 1)"),
                            ),
                            IrContainerKind::Associative { .. } => (
                                "llg_assoc_count",
                                "llg_assoc_value_at",
                                format!("(size_t){loop_index}"),
                            ),
                        };
                        let item_name = format!("_inside_container_item_{index}");
                        let item = RenderedExpr {
                            code: item_name.clone(),
                            width: *width,
                            signed: *signed,
                            fill: None,
                        };
                        let comparison = if value.width == 0 {
                            cmp_expr(&value_local, &item, "==")
                        } else {
                            format!("sv4_wild_eq(_inside_value, {item_name})")
                        };
                        code.push_str(&format!(
                            "for (size_t {loop_index} = 0; {loop_index} < (size_t){size_fn}(&{name}); ++{loop_index}) {{ \
                             sv4_t {item_name} = {get_fn}(&{name}, \
                             {index_arg}); \
                             _inside_result = sv4_logor(_inside_result, {comparison}); }} ",
                        ));
                    }
                }
            }
            code.push_str("_inside_result; })");
            RenderedExpr {
                code,
                width: 1,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::BitSel { base, idx } => {
            let rb = w(base)?;
            let ri = w(idx)?;
            RenderedExpr {
                code: format!("sv4_bit_select({}, sv4_to_index({}))", rb.code, ri.code),
                width: 1,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::PartSel { base, left, right } => {
            let rb = w(base)?;
            RenderedExpr {
                code: format!("sv4_part_select({}, {left}, {right})", rb.code),
                width: ((left - right).abs() + 1) as u32,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            neg,
            ..
        } => {
            let rb = w(base)?;
            let rbi = w(base_idx)?;
            RenderedExpr {
                code: format!(
                    "sv4_idx_part_select_value({}, {}, {}, {})",
                    rb.code, rbi.code, e.width, *neg as u8
                ),
                width: e.width,
                signed: false,
                fill: None,
            }
        }
        IrExprKind::ArrayRead {
            arr,
            indices,
            elem_sel,
        } => {
            let ai = ctx.model.array(*arr);
            let mut index_codes = Vec::with_capacity(indices.len());
            for i in indices {
                index_codes.push(w(i)?.code);
            }
            if ai.real && !matches!(elem_sel, IrElemSel::Whole) {
                return Err("select on a real array element is not supported".to_string());
            }
            let elem = if ai.real {
                guarded_real_array_read(ai, &index_codes)
            } else {
                guarded_array_read(ai, &index_codes)
            };
            let (code, width, signed) = match elem_sel {
                IrElemSel::Whole => (elem, ai.elem_width, ai.signed),
                IrElemSel::Part(l, r) => (
                    format!("sv4_part_select({elem}, {l}, {r})"),
                    ((l - r).abs() + 1) as u32,
                    false,
                ),
                IrElemSel::Bit(idx) => {
                    let ri = w(idx)?;
                    (
                        format!("sv4_bit_select({elem}, sv4_to_index({}))", ri.code),
                        1,
                        false,
                    )
                }
                IrElemSel::Indexed {
                    base,
                    width,
                    negative,
                } => (
                    format!(
                        "sv4_idx_part_select_value({elem}, {}, {width}, {})",
                        w(base)?.code,
                        *negative as u8
                    ),
                    *width,
                    false,
                ),
            };
            RenderedExpr {
                code,
                width,
                signed,
                fill: None,
            }
        }
        IrExprKind::CastToReal { a, shortreal } => {
            let ra = w(a)?;
            RenderedExpr {
                code: round_shortreal(real_code(&ra), *shortreal),
                width: 0,
                signed: true,
                fill: None,
            }
        }
        IrExprKind::CastToPacked { a } => {
            let ra = w(a)?;
            if ra.width == 0 {
                RenderedExpr {
                    code: format!(
                        "sv4_from_real({}, {}, {})",
                        ra.code, e.width, e.signed as u8
                    ),
                    width: e.width,
                    signed: e.signed,
                    fill: None,
                }
            } else {
                RenderedExpr {
                    code: format!("sv4_resize({}, {}, {})", ra.code, e.width, e.signed as u8),
                    width: e.width,
                    signed: e.signed,
                    fill: None,
                }
            }
        }
        IrExprKind::Resize { a } => {
            let ra = w(a)?;
            RenderedExpr {
                code: format!("sv4_resize({}, {}, {})", ra.code, e.width, e.signed as u8),
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::Convert { a } => {
            let ra = w(a)?;
            RenderedExpr {
                code: format!("sv4_cast({}, {}, {})", ra.code, e.width, e.signed as u8),
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::ToTwoState { a } => {
            let ra = w(a)?;
            RenderedExpr {
                code: format!("sv4_to_two_state({})", ra.code),
                width: e.width,
                signed: e.signed,
                fill: None,
            }
        }
        IrExprKind::Fill(f) => RenderedExpr {
            code: format!("sv4_fill({f}, {}, {})", e.width, e.signed as u8),
            width: e.width,
            signed: e.signed,
            fill: Some(*f),
        },
        IrExprKind::Verbatim {
            code,
            width,
            signed,
        } => RenderedExpr {
            code: code.clone(),
            width: *width,
            signed: *signed,
            fill: None,
        },
        IrExprKind::RealBin { op, a, b } => {
            let ra = w(a)?;
            let rb = w(b)?;
            let (acode, bcode) = (real_code(&ra), real_code(&rb));
            let code = match op {
                IrRealBinOp::Add => format!("({acode} + {bcode})"),
                IrRealBinOp::Sub => format!("({acode} - {bcode})"),
                IrRealBinOp::Mul => format!("({acode} * {bcode})"),
                IrRealBinOp::Div => format!("({acode} / {bcode})"),
                IrRealBinOp::Mod => format!("fmod({acode}, {bcode})"),
                IrRealBinOp::Pow => format!("pow({acode}, {bcode})"),
            };
            RenderedExpr {
                code,
                width: 0,
                signed: true,
                fill: None,
            }
        }
        IrExprKind::RealUn { op, a } => {
            let ra = w(a)?;
            let code = match op {
                IrRealUnOp::Neg => format!("(-{})", ra.code),
            };
            RenderedExpr {
                code,
                width: 0,
                signed: true,
                fill: None,
            }
        }
        IrExprKind::SysFunc(f) => match f {
            IrSysFunc::Math { kind, args } => {
                use crate::sim::ir::IrMathFunc;
                let name = match kind {
                    IrMathFunc::Ln => "log",
                    IrMathFunc::Log10 => "log10",
                    IrMathFunc::Exp => "exp",
                    IrMathFunc::Sqrt => "sqrt",
                    IrMathFunc::Pow => "pow",
                    IrMathFunc::Floor => "floor",
                    IrMathFunc::Ceil => "ceil",
                    IrMathFunc::Sin => "sin",
                    IrMathFunc::Cos => "cos",
                    IrMathFunc::Tan => "tan",
                    IrMathFunc::Asin => "asin",
                    IrMathFunc::Acos => "acos",
                    IrMathFunc::Atan => "atan",
                    IrMathFunc::Atan2 => "atan2",
                    IrMathFunc::Hypot => "hypot",
                    IrMathFunc::Sinh => "sinh",
                    IrMathFunc::Cosh => "cosh",
                    IrMathFunc::Tanh => "tanh",
                    IrMathFunc::Asinh => "asinh",
                    IrMathFunc::Acosh => "acosh",
                    IrMathFunc::Atanh => "atanh",
                };
                let args = args
                    .iter()
                    .map(|arg| w(arg).map(|value| real_code(&value)))
                    .collect::<Result<Vec<_>, _>>()?;
                RenderedExpr {
                    code: format!("{name}({})", args.join(", ")),
                    width: 0,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::Realtime {
                precision_fs,
                unit_fs,
            } => RenderedExpr {
                code: format!("((double)llg_time() * {precision_fs}.0 / {unit_fs}.0)"),
                width: 0,
                signed: true,
                fill: None,
            },
            IrSysFunc::Rtoi(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_rtoi({})", real_code(&arg)),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::Itor(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_to_real({})", arg.code),
                    width: 0,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::RealToBits(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_realtobits({})", real_code(&arg)),
                    width: 64,
                    signed: false,
                    fill: None,
                }
            }
            IrSysFunc::BitsToReal(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_bitstoreal({})", arg.code),
                    width: 0,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::ShortRealToBits(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_shortrealtobits({})", real_code(&arg)),
                    width: 32,
                    signed: false,
                    fill: None,
                }
            }
            IrSysFunc::BitsToShortReal(arg) => {
                let arg = w(arg)?;
                RenderedExpr {
                    code: format!("sv4_bitstoshortreal({})", arg.code),
                    width: 0,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::BitQuery { kind, arg } => {
                let arg = w(arg)?;
                let code = match kind {
                    IrBitQuery::CountOnes => format!("sv4_countones({})", arg.code),
                    IrBitQuery::OneHot => format!("sv4_onehot({}, 0)", arg.code),
                    IrBitQuery::OneHot0 => format!("sv4_onehot({}, 1)", arg.code),
                    IrBitQuery::IsUnknown => {
                        format!("sv4_from_u64(sv4_is_unknown({}), 1, 0)", arg.code)
                    }
                };
                let (width, signed) = kind.result_type();
                RenderedExpr {
                    code,
                    width,
                    signed,
                    fill: None,
                }
            }
            IrSysFunc::Clog2(a) => {
                let ra = w(a)?;
                RenderedExpr {
                    code: format!("sv4_clog2({})", ra.code),
                    width: 32,
                    signed: false,
                    fill: None,
                }
            }
            IrSysFunc::Time {
                precision_fs,
                unit_fs,
                kind,
            } => {
                let width = kind.width();
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64(llg_time_scaled({}, {}), {}, 0)",
                        precision_fs, unit_fs, width
                    ),
                    width,
                    signed: false,
                    fill: None,
                }
            }
            IrSysFunc::Bits(a) => {
                let ra = w(a)?;
                RenderedExpr {
                    code: format!("SV4_C({}, 32)", ra.width),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
        },
    };
    Ok(out)
}

/// Comparison/equality over two operands: real operands compare through their
/// real codes and yield `sv4_from_u64`, packed operands use the `sv4_*`
/// comparators.
fn cmp_expr(ra: &RenderedExpr, rb: &RenderedExpr, op: &str) -> String {
    if ra.width == 0 || rb.width == 0 {
        format!(
            "sv4_from_u64(({} {op} {}) ? 1ULL : 0ULL, 1, 0)",
            real_code(ra),
            real_code(rb)
        )
    } else {
        let f = match op {
            "==" => "sv4_eq",
            "!=" => "sv4_neq",
            "<" => "sv4_lt",
            "<=" => "sv4_le",
            ">" => "sv4_gt",
            _ => "sv4_ge",
        };
        format!("{f}({}, {})", ra.code, rb.code)
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
fn guarded_array_read(ai: &crate::sim::ir::IrArray, index_codes: &[String]) -> String {
    match array_guard(ai, index_codes) {
        Some((decls, cond, lin)) => format!(
            "({{ {decls}({cond}) ? {}[({lin})] : {}; }})",
            ai.c_name,
            packed_default(ai.elem_width, ai.signed, ai.two_state)
        ),
        None => format!("{}[0]", ai.c_name),
    }
}

fn guarded_real_array_read(ai: &crate::sim::ir::IrArray, index_codes: &[String]) -> String {
    match array_guard(ai, index_codes) {
        Some((decls, cond, lin)) => {
            format!("({{ {decls}({cond}) ? {}[({lin})] : 0.0; }})", ai.c_name)
        }
        None => format!("{}[0]", ai.c_name),
    }
}

pub(super) fn coerce_two_state(code: String, two_state: bool) -> String {
    if two_state {
        format!("sv4_to_two_state({code})")
    } else {
        code
    }
}

pub(super) fn packed_default(width: u32, signed: bool, two_state: bool) -> String {
    if two_state {
        format!("sv4_from_u64(0, {width}, {})", signed as u8)
    } else {
        format!("sv4_x({width}, {})", signed as u8)
    }
}

/// Convert a rendered value to an explicit `(width, signed)` vector target —
/// the rendering-side half of `expr_to_vector`: real payloads go through
/// `sv4_from_real`, unsized fills through `sv4_fill`, everything else through
/// the value-preserving `sv4_cast`.
fn rendered_to_vector(r: &RenderedExpr, width: u32, signed: bool) -> Result<String, String> {
    if r.width == 0 {
        Ok(format!(
            "sv4_from_real({}, {}, {})",
            r.code, width, signed as u8
        ))
    } else if let Some(f) = r.fill {
        Ok(format!("sv4_fill({f}, {width}, {})", signed as u8))
    } else {
        Ok(arg_resize(&r.code, width, signed))
    }
}

fn lhs_shape(ctx: &RCtx<'_>, lhs: &IrLhs) -> (u32, bool) {
    match lhs {
        IrLhs::Whole(index) => {
            let ty = ctx.model.signal(*index).ty;
            (ty.width(), ty.signed())
        }
        IrLhs::WholeRef { width, signed, .. } | IrLhs::Ref { width, signed, .. } => {
            (*width, *signed)
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
        IrLhs::WholeRef { two_state, .. } | IrLhs::Ref { two_state, .. } => *two_state,
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
fn render_lhs_value(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    width: u32,
    signed: bool,
) -> Result<RenderedExpr, String> {
    let value = match lhs {
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
            ..
        } => {
            return Ok(coerce_lhs_read(
                ctx,
                lhs,
                RenderedExpr {
                    code: format!("llg_ref_read({addr})"),
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

fn capture_lhs_indices(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<(String, IrLhs), String> {
    fn capture(
        ctx: &RCtx<'_>,
        expression: &IrExpr,
        declarations: &mut String,
        next: &mut usize,
    ) -> Result<IrExpr, String> {
        let rendered = render_expr_impl(ctx, expression)?;
        let name = format!("_llg_mut_idx{next}");
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
    ) -> Result<(), String> {
        match lhs {
            IrLhs::Bit(_, index, _) => {
                *index = capture(ctx, index, declarations, next)?;
            }
            IrLhs::IdxPart(_, base, width_expr, _, _, _) => {
                *base = capture(ctx, base, declarations, next)?;
                *width_expr = capture(ctx, width_expr, declarations, next)?;
            }
            IrLhs::ArrayElem {
                indices, elem_sel, ..
            } => {
                for index in indices {
                    *index = capture(ctx, index, declarations, next)?;
                }
                match elem_sel {
                    IrElemSel::Bit(index) => {
                        **index = capture(ctx, index, declarations, next)?;
                    }
                    IrElemSel::Indexed { base, .. } => {
                        **base = capture(ctx, base, declarations, next)?;
                    }
                    IrElemSel::Whole | IrElemSel::Part(..) => {}
                }
            }
            IrLhs::Stream { parts, .. } => {
                for (part, _) in parts {
                    visit(ctx, part, declarations, next)?;
                }
            }
            IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Part(..) => {}
        }
        Ok(())
    }

    let mut captured = lhs.clone();
    let mut declarations = String::new();
    let mut next = 0;
    visit(ctx, &mut captured, &mut declarations, &mut next)?;
    Ok((declarations, captured))
}

fn render_mutation_expr(
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

/// Full assignment statement assigning `rhs` into the LHS target:
/// `llg_ba/nba(...)` for plain targets (with statement-expression select
/// write-backs), the `_d` variants for real companions, a guarded block for
/// array elements, and `llg_net_write` for collapsed-net members.
pub(super) fn render_assign(
    ctx: &RCtx<'_>,
    lh: &IrLhs,
    rhs: &IrExpr,
    nba: bool,
) -> Result<String, String> {
    if nba {
        return super::assignments::render_nba(ctx, lh, rhs, "0ULL");
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
        return Ok(format!(
            "llg_ref_write({addr}, {});",
            coerce_two_state(value, *two_state)
        ));
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

    let call = "llg_ba";
    let two_state = match lh {
        IrLhs::Whole(idx) => ctx.model.signal(*idx).ty.two_state(),
        IrLhs::Bit(idx, _, selected_two_state)
        | IrLhs::Part(idx, .., selected_two_state)
        | IrLhs::IdxPart(idx, .., selected_two_state) => {
            ctx.model.signal(*idx).ty.two_state() || *selected_two_state
        }
        IrLhs::WholeRef { two_state, .. } | IrLhs::Ref { two_state, .. } => *two_state,
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
        IrLhs::Ref {
            addr,
            width,
            signed,
            const_ref,
            ..
        } => {
            if *const_ref {
                return Err("write through const ref formal is not supported".to_string());
            }
            return Ok(format!(
                "llg_ref_write({addr}, {});",
                resize(&rhs_code, *width, *signed)
            ));
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

/// Render an expression-position function call with output-formal temps:
/// either a plain call or one GNU statement expression carrying the temps,
/// the writebacks and the result.
fn render_call_expr(
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
                call_args.push(super::objects::string(ctx, value)?);
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
                call_args.push(super::objects::chandle(ctx, value)?);
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
    call_args.push(call.depth.code());
    let call_code = format!("{}({})", f.c_name, call_args.join(", "));

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
            .map(|value| super::objects::string(ctx, value))
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
            .map(|value| super::objects::string(ctx, value))
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
