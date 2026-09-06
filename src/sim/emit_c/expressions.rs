//! Expression, assignment-target, array-access, and call rendering.

use super::constants::{emit_const, round_shortreal};
use super::context::{RCtx, RenderedExpr};
use super::EmitError;
use crate::sim::ir::{
    IrBinOp, IrBitQuery, IrCallArg, IrElemSel, IrExpr, IrExprKind, IrLhs, IrRealBinOp, IrRealUnOp,
    IrSysFunc, IrType, IrUnOp,
};

/// The real-value code of a rendered operand: bare for real expressions,
/// `sv4_to_real(...)` for packed ones.
fn real_code(e: &RenderedExpr) -> String {
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
            let code = if form.is_out {
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
                        format!("sv4_logand({}, {})", ra.code, rb.code)
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
                        format!("sv4_logor({}, {})", ra.code, rb.code)
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
            let elem = guarded_array_read(ai, &index_codes);
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
                precision_ps,
                unit_ps,
                kind,
            } => {
                let width = kind.width();
                RenderedExpr {
                    code: format!(
                        "sv4_from_u64(llg_time_scaled({}, {}), {}, 0)",
                        precision_ps, unit_ps, width
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
    // Real companion targets copy the (possibly converted) real value.
    if let IrLhs::Whole(idx) = lh {
        let sig = ctx.model.signal(*idx);
        if let IrType::Real { shortreal } = sig.ty {
            let rr = render_expr_impl(ctx, rhs)?;
            let call = if nba { "llg_nba_d" } else { "llg_ba_d" };
            return Ok(format!(
                "{call}(&{}, {});",
                sig.c_name,
                round_shortreal(real_code(&rr), shortreal)
            ));
        }
    }
    // A real RHS is converted to the target's vector shape up front.
    let converted: Option<(String, u32, bool)> = if rhs.width == 0 {
        let (width, signed) = match lh {
            IrLhs::Whole(idx) => {
                let t = ctx.model.signal(*idx).ty;
                (t.width(), t.signed())
            }
            IrLhs::WholeRef { width, signed, .. } => (*width, *signed),
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
            },
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
        let call = if nba { "llg_nba" } else { "llg_ba" };
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
            if nba {
                return Err("nonblocking assignment to an inout-net member is not \
                     supported (nets cannot be nonblocking targets)"
                    .to_string());
            }
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
            if nba {
                return Err("nonblocking assignment to a net select is not supported".to_string());
            }
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

    let call = if nba { "llg_nba" } else { "llg_ba" };
    let two_state = match lh {
        IrLhs::Whole(idx) => ctx.model.signal(*idx).ty.two_state(),
        IrLhs::Bit(idx, _, selected_two_state)
        | IrLhs::Part(idx, .., selected_two_state)
        | IrLhs::IdxPart(idx, .., selected_two_state) => {
            ctx.model.signal(*idx).ty.two_state() || *selected_two_state
        }
        IrLhs::WholeRef { two_state, .. } => *two_state,
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
    }
    let mut temps: Vec<TempInfo<'_>> = Vec::new();
    let mut call_args: Vec<String> = Vec::new();
    let formal_order = f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_out)
        .chain(
            f.formals
                .iter()
                .enumerate()
                .filter(|(_, formal)| !formal.is_out),
        );
    for ((idx, _), arg) in formal_order.zip(&call.args) {
        match arg {
            IrCallArg::Val(e) => call_args.push(coerce_two_state(
                render_expr_impl(ctx, e)?.code,
                f.formals[idx].two_state,
            )),
            IrCallArg::OutAddr(addr) => call_args.push(addr.clone()),
            IrCallArg::OutTemp {
                name,
                init,
                writeback,
            } => {
                temps.push(TempInfo {
                    idx,
                    name,
                    init: init.as_deref(),
                    wb: writeback,
                });
                call_args.push(format!("&{name}"));
            }
        }
    }
    call_args.push(call.depth.code());
    let call_code = format!("{}({})", f.c_name, call_args.join(", "));

    if temps.is_empty() {
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
        let init = match t.init {
            Some(expr) => coerce_two_state(render_expr_impl(ctx, expr)?.code, form.two_state),
            None => packed_default(form.width, form.signed, form.two_state),
        };
        parts.push(format!("sv4_t {} = {init}", t.name));
    }
    if has_ret {
        parts.push(format!("sv4_t _r = {call_code}"));
    } else {
        parts.push(call_code.clone());
    }
    for t in &temps {
        let form = &f.formals[t.idx];
        let rhs = IrExpr::new(
            IrExprKind::LocalRead(t.name.to_string()),
            form.width,
            form.signed,
            None,
        );
        let stmt = render_assign(ctx, t.wb, &rhs, false)?;
        parts.push(stmt.trim_end_matches(';').to_string());
    }
    if has_ret {
        parts.push("_r".to_string());
    } else {
        parts.push(format!("sv4_x({ret_w}, {})", ret_s as u8));
    }
    Ok(RenderedExpr {
        code: format!("({{ {} }})", parts.join("; ")),
        width: ret_w,
        signed: ret_s,
        fill: None,
    })
}
