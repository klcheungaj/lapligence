//! Expression, assignment-target, array-access, and call rendering.

use super::constants::{c_string_literal, emit_const, round_shortreal};
use super::context::{RCtx, RenderedExpr};
use super::EmitError;
use crate::sim::ir::{
    IrBinOp, IrBitQuery, IrCallArg, IrContainerElement, IrContainerKind, IrElemSel, IrEnumMethod,
    IrEnumQuery, IrExpr, IrExprKind, IrFileInput, IrFileInputTarget, IrFileReadTarget,
    IrInsideItem, IrLhs, IrPlusArgText, IrRandomFunc, IrRealBinOp, IrRealUnOp, IrStreamDirection,
    IrStringExpr, IrSysFunc, IrType, IrUnOp,
};

mod queries;
use queries::{render_enum_query, render_vpi_call};
mod system_functions;
use system_functions::render_legacy_random;
mod input;
use input::{render_file_input, render_test_plusargs, render_value_plusargs};
mod lvalues;
pub(super) use lvalues::render_lhs_address;
pub(crate) use lvalues::array_guard;
use lvalues::{
    guarded_array_read, guarded_real_array_read, lhs_shape, render_lhs_value,
    capture_lhs_indices, capture_lhs_indices_with_prefix, render_mutation_expr,
};
mod casts;
use casts::render_dynamic_cast;
mod assignments;
pub(super) use assignments::render_assign;
mod calls;
pub(super) use calls::with_ref_scope;
use calls::render_call_expr;


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
        IrExprKind::EnumMethod(query) => RenderedExpr {
            code: render_enum_query(ctx, query)?,
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
            let base = if s.net_alias.is_empty() {
                s.c_name.clone()
            } else {
                format!("llg_net_alias_read(&llg_net_alias_{idx})")
            };
            let code = if ctx.sampled {
                let pointer = if s.net_alias.is_empty() {
                    format!("&{}", s.c_name)
                } else {
                    format!("&llg_net_alias_{idx}.visible")
                };
                format!("(*llg_sampled_value({pointer}))")
            } else {
                base
            };
            RenderedExpr {
                code,
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
        IrExprKind::CallFn(call) => {
            let mut rendered = render_call_expr(ctx, call)?;
            let result_type = if rendered.width == 0 { "double" } else { "sv4_t" };
            rendered.code = with_ref_scope(rendered.code, &call.args, Some(result_type));
            rendered
        },
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
        IrExprKind::DynamicCast(cast) => render_dynamic_cast(ctx, e, cast)?,
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
                IrBinOp::LogImpl => {
                    if !real {
                        // Implication is short-circuiting: a known-false
                        // antecedent determines the result and must not
                        // evaluate the consequent.  An X/Z antecedent still
                        // evaluates it because `x -> 1` is known true.
                        format!(
                            "({{ sv4_t _llg_logic_left = {}; \
                             (!sv4_to_bool(_llg_logic_left) && !sv4_is_unknown(_llg_logic_left)) \
                             ? sv4_from_u64(1, 1, 0) : sv4_logimpl(_llg_logic_left, {}); }})",
                            ra.code, rb.code
                        )
                    } else if ra.width == 0 && rb.width == 0 {
                        format!(
                            "sv4_from_u64((!{} || {}) ? 1ULL : 0ULL, 1, 0)",
                            bool_code(&ra),
                            bool_code(&rb)
                        )
                    } else if ra.width == 0 {
                        format!(
                            "({{ int _llg_logic_left = llg_real_to_bool({}); \
                             _llg_logic_left ? sv4_logimpl(sv4_from_u64(1, 1, 0), {}) \
                             : sv4_from_u64(1, 1, 0); }})",
                            ra.code, rb.code
                        )
                    } else {
                        format!(
                            "({{ sv4_t _llg_logic_left = {}; \
                             (!sv4_to_bool(_llg_logic_left) && !sv4_is_unknown(_llg_logic_left)) \
                             ? sv4_from_u64(1, 1, 0) : sv4_logimpl(_llg_logic_left, \
                             sv4_from_u64((uint64_t)llg_real_to_bool({}), 1, 0)); }})",
                            ra.code, rb.code
                        )
                    }
                }
                IrBinOp::LogEquiv => {
                    let left = if ra.width == 0 {
                        format!(
                            "sv4_from_u64((uint64_t)llg_real_to_bool({}), 1, 0)",
                            ra.code
                        )
                    } else {
                        ra.code.clone()
                    };
                    let right = if rb.width == 0 {
                        format!(
                            "sv4_from_u64((uint64_t)llg_real_to_bool({}), 1, 0)",
                            rb.code
                        )
                    } else {
                        rb.code.clone()
                    };
                    // Equivalence is not a short-circuit operator.  Locals
                    // make both evaluation and the left-to-right order
                    // explicit even when operands contain mutations/calls.
                    format!(
                        "({{ sv4_t _llg_logic_left = {}; sv4_t _llg_logic_right = {}; \
                         sv4_logequiv(_llg_logic_left, _llg_logic_right); }})",
                        left, right
                    )
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
        IrExprKind::BitStreamCast {
            a,
            source_width,
            target_two_state,
        } => {
            let ra = w(a)?;
            if ra.width == 0 || ra.width != *source_width {
                return Err("bit-stream cast requires a fixed packed source".to_owned());
            }
            let mut code = format!("sv4_cast({}, {}, {})", ra.code, e.width, e.signed as u8);
            if *target_two_state {
                code = format!("sv4_to_two_state({code})");
            }
            RenderedExpr {
                code,
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
            IrSysFunc::TestPlusArgs { pattern } => render_test_plusargs(ctx, pattern)?,
            IrSysFunc::Sampled(call) => {
                use crate::sim::ir::IrSampledFunc;
                match call.kind {
                    IrSampledFunc::Sampled => {
                        let sampled_ctx = RCtx {
                            model: ctx.model,
                            func: ctx.func,
                            sampled: true,
                            activation_label: ctx.activation_label.clone(),
                        };
                        let argument = render_expr_impl(&sampled_ctx, &call.argument)?;
                        RenderedExpr {
                            code: argument.code,
                            width: argument.width,
                            signed: argument.signed,
                            fill: argument.fill,
                        }
                    }
                    IrSampledFunc::Past => {
                        let domain = call
                            .domain
                            .ok_or_else(|| "sampled past call has no domain".to_string())?;
                        RenderedExpr {
                            code: format!("llg_sampled_domain_past({}, {}ULL)", domain, call.ticks),
                            width: e.width,
                            signed: e.signed,
                            fill: None,
                        }
                    }
                    IrSampledFunc::Rose
                    | IrSampledFunc::Fell
                    | IrSampledFunc::Stable
                    | IrSampledFunc::Changed => {
                        let domain = call
                            .domain
                            .ok_or_else(|| "sampled status call has no domain".to_string())?;
                        let status = match call.kind {
                            IrSampledFunc::Rose => 0,
                            IrSampledFunc::Fell => 1,
                            IrSampledFunc::Stable => 2,
                            IrSampledFunc::Changed => 3,
                            _ => unreachable!(),
                        };
                        RenderedExpr {
                            code: format!(
                                "sv4_from_u64((uint64_t)llg_sampled_domain_status({}, {}), 1, 0)",
                                domain, status
                            ),
                            width: 1,
                            signed: false,
                            fill: None,
                        }
                    }
                }
            }
            IrSysFunc::ValuePlusArgs { format, target } => {
                render_value_plusargs(ctx, format, target, e.width, e.signed)?
            }
            IrSysFunc::System(command) => {
                let (command, has_command) = match command.as_ref() {
                    Some(command) => (super::objects::string(ctx, command)?, 1),
                    None => ("llg_string_bytes(\"\", 0)".to_owned(), 0),
                };
                RenderedExpr {
                    code: format!("llg_system({command}, {has_command})"),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::VpiCall { site, name, args } => {
                render_vpi_call(ctx, *site, name, args, e.width, e.signed)?
            }
            IrSysFunc::LegacyRandom { kind, seed, args } => {
                render_legacy_random(ctx, *kind, seed.as_deref(), args)?
            }
            IrSysFunc::Urandom { seed } => {
                let code = match seed {
                    Some(seed) => format!("llg_urandom_seed({})", w(seed)?.code),
                    None => "llg_urandom()".to_owned(),
                };
                RenderedExpr {
                    code,
                    width: 32,
                    signed: false,
                    fill: None,
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                let max = w(max)?.code;
                let (min, has_min) = match min {
                    Some(min) => (w(min)?.code, 1),
                    None => ("sv4_from_u64(0, 32, 0)".to_owned(), 0),
                };
                RenderedExpr {
                    code: format!("llg_urandom_range({max}, {min}, {has_min})"),
                    width: 32,
                    signed: false,
                    fill: None,
                }
            }
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
            IrSysFunc::QFull { q_id, status } => {
                let q_id = w(q_id)?;
                let status = render_lhs_address(ctx, status)?;
                RenderedExpr {
                    code: format!("llg_q_full({}, {})", q_id.code, status),
                    width: 32,
                    signed: true,
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
            IrSysFunc::FileOpen { path, mode } => {
                let path = super::objects::string(ctx, path)?;
                let mode_code = mode
                    .as_ref()
                    .map(|mode| super::objects::string(ctx, mode))
                    .transpose()?;
                let mode_code = mode_code.unwrap_or_else(|| "llg_string_bytes(\"\", 0)".to_owned());
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64((uint64_t)llg_file_open({path}, {mode_code}, {}), 32, 1)",
                        mode.is_some() as u8
                    ),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::FileTell(descriptor) => {
                let descriptor = w(descriptor)?;
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64((uint64_t)llg_file_tell(llg_file_descriptor({})), 64, 1)",
                        descriptor.code
                    ),
                    width: 64,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                let descriptor = w(descriptor)?;
                let offset = w(offset)?;
                let operation = w(operation)?;
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64((uint64_t)llg_file_seek(llg_file_descriptor({}), {}, {}), 32, 1)",
                        descriptor.code, offset.code, operation.code
                    ),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::FileError {
                descriptor,
                message,
            } => {
                let descriptor = w(descriptor)?;
                let target = message.as_deref().unwrap_or("NULL");
                let message_arg = if message.is_some() {
                    target.to_owned()
                } else {
                    "NULL".to_owned()
                };
                RenderedExpr {
                    code: format!(
                        "({{ int _llg_file_error = llg_file_error(llg_file_descriptor({}), {}); sv4_from_u64((uint64_t)_llg_file_error, 32, 1); }})",
                        descriptor.code, message_arg
                    ),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::FileEof(descriptor) => {
                let descriptor = w(descriptor)?;
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64((uint64_t)llg_file_eof(llg_file_descriptor({})), 32, 1)",
                        descriptor.code
                    ),
                    width: 32,
                    signed: true,
                    fill: None,
                }
            }
            IrSysFunc::FileInput(input) => render_file_input(ctx, input)?,
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
