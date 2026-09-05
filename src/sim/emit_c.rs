//! emit_c — C11 backend rendering an [`crate::sim::ir::IrModel`] to `model.c`.
//!
//! This module is the backend half of the simulator pipeline
//! (database lowering → [`crate::sim::ir`] → optimization → this module).
//! It consumes only IR types plus the leaf text-formatting helpers below; it
//! has no dependency on the lowering frontend or the Surelog binding layer.
//! Lowering ([`crate::sim::codegen`]) makes every decision (widths,
//! signedness, fills, sensitivity, names) and records it in the IR; the
//! renderers here turn those decisions into exactly the same C11 text the
//! pre-IR emitter produced.
//!
//! The leaf helpers (identifier sanitization, global-name construction,
//! constant initializers, timescale strings) live here because they own the C
//! surface conventions; lowering borrows them via `crate::sim::emit_c::*` when
//! it fills the IR's `c_name` fields and constant payloads.

use std::collections::HashSet;

use crate::sim::ir::{
    IrBinOp, IrCallArg, IrConst, IrElemSel, IrExpr, IrExprKind, IrFunc, IrLhs, IrModel,
    IrRealBinOp, IrRealUnOp, IrSysFunc, IrType, IrUnOp, IrWaitSrc, LLG_MAX_WIDTH,
};

/// Number of 64-bit limbs covering [`LLG_MAX_WIDTH`] bits.  Keep in sync with
/// `LLG_LIMBS` in `src/sim/rt/llg_rt.h` (16).
pub(crate) const LLG_LIMBS: usize = (LLG_MAX_WIDTH as usize).div_ceil(64);

/// Strip the `lib@` prefix Surelog puts on top-instance names.
pub(crate) fn strip_lib(name: &str) -> String {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => name.to_string(),
    }
}

/// Sanitize a string for use as a C identifier fragment.
pub(crate) fn ident(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn global_name(path: &str, name: &str) -> String {
    format!("G_{}_{}", ident(path), ident(name))
}

pub(crate) fn real_global_name(path: &str, name: &str) -> String {
    format!("D_{}_{}", ident(path), ident(name))
}

pub(crate) fn event_global_name(path: &str, name: &str) -> String {
    format!("E_{}_{}", ident(path), ident(name))
}

pub(crate) fn escaped_char(c: char) -> String {
    match c {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        c if c.is_ascii_control() => format!("\\{:03o}", c as u32),
        _ => c.to_string(),
    }
}

fn c_string_literal(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        out.push_str(&escaped_char(ch));
    }
    out.push('"');
    out
}

/// The Verilog `timescale string for a value in ps (e.g. 1000 ps → "1ns").
pub(crate) fn ps_to_timescale_str(v: u64) -> String {
    const PAIRS: [(u64, &str); 13] = [
        (1_000_000_000_000, "1s"),
        (100_000_000_000, "100ms"),
        (10_000_000_000, "10ms"),
        (1_000_000_000, "1ms"),
        (100_000_000, "100us"),
        (10_000_000, "10us"),
        (1_000_000, "1us"),
        (100_000, "100ns"),
        (10_000, "10ns"),
        (1_000, "1ns"),
        (100, "100ps"),
        (10, "10ps"),
        (1, "1ps"),
    ];
    for (val, s) in PAIRS {
        if val == v {
            return s.to_string();
        }
    }
    format!("{v}ps")
}

pub(crate) fn emit_real_literal(value: f64) -> String {
    if value.is_nan() {
        "NAN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-INFINITY".to_string()
        } else {
            "INFINITY".to_string()
        }
    } else {
        format!("{value:.17e}")
    }
}

/// Render a lowered constant: `SV4_INIT(...)` up to 64 packed bits,
/// `sv4_from_limbs((uint64_t[]){...}, ...)` beyond, and the bare double
/// literal for real constants.
pub(crate) fn emit_const(c: &IrConst) -> String {
    if let Some(value) = c.real {
        return emit_real_literal(value);
    }
    let signed = if c.signed { 1 } else { 0 };
    if c.width <= 64 {
        let b = c.bits.first().copied().unwrap_or(0);
        let x = c.x.first().copied().unwrap_or(0);
        let z = c.z.first().copied().unwrap_or(0);
        format!("SV4_INIT({b}ULL, {x}ULL, {z}ULL, {}, {signed})", c.width)
    } else {
        let mut bs = [0u64; LLG_LIMBS];
        let mut xs = [0u64; LLG_LIMBS];
        let mut zs = [0u64; LLG_LIMBS];
        let nb = c.bits.len().min(LLG_LIMBS);
        let nx = c.x.len().min(LLG_LIMBS);
        let nz = c.z.len().min(LLG_LIMBS);
        bs[..nb].copy_from_slice(&c.bits[..nb]);
        xs[..nx].copy_from_slice(&c.x[..nx]);
        zs[..nz].copy_from_slice(&c.z[..nz]);
        let b = bs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        let x = xs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        let z = zs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "sv4_from_limbs((uint64_t[]){{{b}}}, (uint64_t[]){{{x}}}, \
             (uint64_t[]){{{z}}}, {}, {signed})",
            c.width
        )
    }
}

/// Render a constant converted to an explicit `(width, signed)` vector target:
/// real payloads go through `sv4_from_real` (at most 64 bits), everything else
/// through the value-preserving `sv4_cast` keyed on the constant's own
/// signedness (LRM §10.7 assignment padding).
pub(crate) fn emit_const_for_vector(
    c: &IrConst,
    width: u32,
    signed: bool,
) -> Result<String, String> {
    if c.real.is_some() && width > 64 {
        return Err(format!(
            "real-to-packed constant conversion target is {width} bits wide; \
             v1 supports at most 64 bits"
        ));
    }
    Ok(match c.real {
        Some(value) => format!(
            "sv4_from_real({}, {}, {})",
            emit_real_literal(value),
            width,
            signed as u8
        ),
        None => format!("sv4_cast({}, {}, {})", emit_const(c), width, signed as u8),
    })
}

pub(crate) fn emit_const_for_real(c: &IrConst) -> String {
    match c.real {
        Some(value) => emit_real_literal(value),
        None => format!("sv4_to_real({})", emit_const(c)),
    }
}

/// Round a rendered real expression through C `float` for shortreal targets.
pub(crate) fn round_shortreal(code: String, shortreal: bool) -> String {
    if shortreal {
        format!("((double)(float)({code}))")
    } else {
        code
    }
}

/// All-X constant expression for a `w`-bit global signal initializer,
/// mirroring the runtime's `sv4_x(w, 0)`: X bits fill the width's limbs,
/// limbs beyond the width are zero.  A brace initializer (not a function
/// call) so the generated C stays a valid static initializer.
pub(crate) fn emit_all_x_init(width: u32) -> String {
    let nlimbs = (width as usize).div_ceil(64);
    let mut xz = Vec::with_capacity(LLG_LIMBS);
    for i in 0..LLG_LIMBS {
        xz.push(if i < nlimbs {
            if i == nlimbs - 1 && !width.is_multiple_of(64) {
                (1u64 << (width % 64)) - 1
            } else {
                u64::MAX
            }
        } else {
            0
        });
    }
    let x = xz
        .iter()
        .map(|v| format!("{v}ULL"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {{ 0 }}, {{ {x} }}, {{ 0 }}, {width}, 0 }}")
}

/// All-Z constant expression for a `w`-bit global initializer, mirroring
/// [`emit_all_x_init`] (the `SV4_Z` macro clamps to 64 bits, so wide nets use
/// this brace initializer to stay a valid static initializer).
pub(crate) fn emit_all_z_init(width: u32) -> String {
    let nlimbs = (width as usize).div_ceil(64);
    let mut zz = Vec::with_capacity(LLG_LIMBS);
    for i in 0..LLG_LIMBS {
        zz.push(if i < nlimbs {
            if i == nlimbs - 1 && !width.is_multiple_of(64) {
                (1u64 << (width % 64)) - 1
            } else {
                u64::MAX
            }
        } else {
            0
        });
    }
    let z = zz
        .iter()
        .map(|v| format!("{v}ULL"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {{ 0 }}, {{ 0 }}, {{ {z} }}, {width}, 0 }}")
}

// ── Expression rendering ──────────────────────────────────────────────────────

/// A rendered expression: C code plus its self-determined width/signedness.
/// `width == 0` marks a real (double) value; `fill` mirrors the IR node's
/// unsized-fill marker for assignment/call/return conversions.
pub struct RenderedExpr {
    pub code: String,
    pub width: u32,
    pub signed: bool,
    pub fill: Option<u8>,
}

/// Render context: the model tables plus the enclosing C function when
/// rendering a function body (formal reads resolve through it).
pub struct RCtx<'m> {
    pub model: &'m IrModel,
    pub func: Option<&'m IrFunc>,
}

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
fn bool_code(e: &RenderedExpr) -> String {
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
pub fn render_expr(ctx: &RCtx<'_>, e: &IrExpr) -> Result<RenderedExpr, String> {
    let w = |x: &IrExpr| render_expr(ctx, x);
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
                code: format!("sv4_bit_select({}, sv4_to_u64({}))", rb.code, ri.code),
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
            width_expr,
            neg,
        } => {
            let rb = w(base)?;
            let rbi = w(base_idx)?;
            let rwe = w(width_expr)?;
            RenderedExpr {
                code: format!(
                    "sv4_idx_part_select({}, sv4_to_u64({}), \
                     (uint16_t)sv4_to_u64({}), {})",
                    rb.code, rbi.code, rwe.code, *neg as u8
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
                        format!("sv4_bit_select({elem}, sv4_to_u64({}))", ri.code),
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
    ((ai.dims[k].0 - ai.dims[k].1).abs() + 1) as u64
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
        let (l, _r) = ai.dims[k];
        let off = if l >= _r {
            format!("{l} - sv4_to_i64(_i{k})")
        } else {
            format!("sv4_to_i64(_i{k}) - {l}")
        };
        decls.push_str(&format!(
            "sv4_t _i{k} = {}; int64_t _o{k} = {off}; ",
            index_codes[k]
        ));
        conds.push(format!(
            "sv4_fits_i64(_i{k}) && _o{k} >= 0 && _o{k} < {}",
            dim_size(ai, k)
        ));
        if stride == 1 {
            terms.push(format!("_o{k}"));
        } else {
            terms.push(format!("_o{k} * {stride}"));
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
            "({{ {decls}({cond}) ? {}[({lin})] : sv4_x({}, {}); }})",
            ai.c_name, ai.elem_width, ai.signed as u8
        ),
        None => format!("{}[0]", ai.c_name),
    }
}

/// Convert a rendered value to an explicit `(width, signed)` vector target —
/// the rendering-side half of `expr_to_vector`: real payloads go through
/// `sv4_from_real`, unsized fills through `sv4_fill`, everything else through
/// the value-preserving `sv4_cast`.
fn rendered_to_vector(r: &RenderedExpr, width: u32, signed: bool) -> Result<String, String> {
    if r.width == 0 {
        if width > 64 {
            return Err(format!(
                "real-to-packed conversion target is {width} bits wide; \
                 v1 supports at most 64 bits"
            ));
        }
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
pub fn render_assign(
    ctx: &RCtx<'_>,
    lh: &IrLhs,
    rhs: &IrExpr,
    nba: bool,
) -> Result<String, String> {
    // Real companion targets copy the (possibly converted) real value.
    if let IrLhs::Whole(idx) = lh {
        let sig = ctx.model.signal(*idx);
        if let IrType::Real { shortreal } = sig.ty {
            let rr = render_expr(ctx, rhs)?;
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
            IrLhs::Part(_, left, right) => (((left - right).abs() + 1) as u32, false),
            IrLhs::IdxPart(idx, ..) => (ctx.model.signal(*idx).ty.width(), false),
            IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
                IrElemSel::Whole => {
                    let a = ctx.model.array(*arr);
                    (a.elem_width, a.signed)
                }
                IrElemSel::Part(left, right) => (((left - right).abs() + 1) as u32, false),
                IrElemSel::Bit(_) => (1, false),
            },
        };
        let rr = render_expr(ctx, rhs)?;
        Some((rendered_to_vector(&rr, width, signed)?, width, signed))
    } else {
        None
    };
    // The effective RHS code/fill after conversion.
    let rr = render_expr(ctx, rhs)?;
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
                return rhs_code.clone();
            }
            match rhs_fill {
                Some(f) => format!("sv4_fill({f}, {w}, {})", s as u8),
                None => format!("sv4_cast({}, {w}, {})", rhs_code, s as u8),
            }
        };
        let mut index_codes = Vec::with_capacity(indices.len());
        for i in indices {
            index_codes.push(render_expr(ctx, i)?.code);
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
                    let idx_code = render_expr(ctx, idx)?.code;
                    (
                        target,
                        format!(
                            "({{ sv4_t _t = {base}[({lin})]; \
                             sv4_bit_select_set(&_t, sv4_to_u64({idx_code}), {v}); _t; }})"
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
    // A select LHS on a collapsed-net member never reaches emission (groups
    // with select writes are skipped at lowering); the error is a backstop.
    if let IrLhs::Bit(idx, _) | IrLhs::Part(idx, ..) | IrLhs::IdxPart(idx, ..) = lh {
        if ctx.model.signal(*idx).net_driver.is_some() {
            return Err("select LHS on an inout-net member is not supported".to_string());
        }
    }

    let call = if nba { "llg_nba" } else { "llg_ba" };
    // Assignment padding follows the RHS's OWN signedness (LRM §10.7);
    // `sv4_cast` keys the extension off the value, so the tags stay the
    // pre-existing target shapes.  Fill literals keep the target shape like
    // every other fill context, and a real RHS arrives pre-converted to the
    // exact target shape.
    let resize = |code: &str, w: u32, s: bool| -> String {
        if real_converted {
            return code.to_string();
        }
        match rhs_fill {
            Some(f) => format!("sv4_fill({f}, {w}, {})", s as u8),
            None => format!("sv4_cast({code}, {w}, {})", s as u8),
        }
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
        } => format!("{addr}, {}", resize(&rhs_code, *width, *signed)),
        IrLhs::Bit(idx, ie) => {
            let sig = ctx.model.signal(*idx);
            let v = resize(&rhs_code, 1, false);
            let ic = render_expr(ctx, ie)?.code;
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_bit_select_set(&_t, sv4_to_u64({}), {}); _t; }})",
                sig.c_name, sig.c_name, ic, v
            )
        }
        IrLhs::Part(idx, left, right) => {
            let sig = ctx.model.signal(*idx);
            let w = ((left - right).abs() + 1) as u32;
            let v = resize(&rhs_code, w, false);
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_part_select_set(&_t, {left}, {right}, {v}); _t; }})",
                sig.c_name, sig.c_name
            )
        }
        IrLhs::IdxPart(idx, be, we, neg) => {
            let sig = ctx.model.signal(*idx);
            let bc = render_expr(ctx, be)?.code;
            let wc = render_expr(ctx, we)?.code;
            let v = if real_converted {
                rhs_code.clone()
            } else {
                match rhs_fill {
                    Some(f) => format!("sv4_fill({f}, (uint16_t)sv4_to_u64({wc}), 0)"),
                    None => format!("sv4_cast({}, (uint16_t)sv4_to_u64({wc}), 0)", rhs_code),
                }
            };
            format!(
                "&{}, ({{ sv4_t _t = {}; sv4_idx_part_select_set(&_t, sv4_to_u64({}), \
                 (uint16_t)sv4_to_u64({}), {}, {}); _t; }})",
                sig.c_name, sig.c_name, bc, wc, *neg as u8, v
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
    for (idx, arg) in call.args.iter().enumerate() {
        match arg {
            IrCallArg::Val(e) => call_args.push(render_expr(ctx, e)?.code),
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
            Some(expr) => render_expr(ctx, expr)?.code,
            None => format!("sv4_x({}, {})", form.width, form.signed as u8),
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

// ── Statement rendering ───────────────────────────────────────────────────────

/// Render one statement, reproducing the pre-IR emitter's text shape exactly
/// (including its indentation conventions).
pub fn render_stmt(ctx: &RCtx<'_>, st: &crate::sim::ir::IrStmt) -> Result<String, String> {
    use crate::sim::ir::{IrJoinKind, IrStmt};
    fn block_stmts(ctx: &RCtx<'_>, stmts: &[crate::sim::ir::IrStmt]) -> Result<String, String> {
        let mut out = String::new();
        for s in stmts {
            out.push_str(&render_stmt(ctx, s)?);
        }
        Ok(out)
    }
    let out = match st {
        IrStmt::Block(stmts) => {
            format!("{{\n{}}}\n", block_stmts(ctx, stmts)?)
        }
        IrStmt::DeclLocal {
            name,
            width,
            signed,
            init,
        } => {
            let init = match init {
                Some(e) => render_expr(ctx, e)?.code,
                None => format!("sv4_x({width}, {})", *signed as u8),
            };
            format!("    sv4_t {name} = {init};\n")
        }
        IrStmt::Assign { lhs, rhs, nba } => {
            format!("    {}\n", render_assign(ctx, lhs, rhs, *nba)?)
        }
        IrStmt::If { cond, then_, els } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!("if ({}) {{\n", bool_code(&rc));
            out.push_str(&block_stmts(ctx, then_)?);
            out.push_str("}\n");
            if let Some(els) = els {
                out.push_str("else {\n");
                out.push_str(&block_stmts(ctx, els)?);
                out.push_str("}\n");
            }
            out
        }
        IrStmt::While { cond, body } => {
            let rc = render_expr(ctx, cond)?;
            format!(
                "while ({}) {{\n{}}}\n",
                bool_code(&rc),
                block_stmts(ctx, body)?
            )
        }
        IrStmt::Repeat { count, body } => {
            let rc = render_expr(ctx, count)?;
            format!(
                "{{ uint64_t _rc = sv4_to_u64({}); for (uint64_t _ri = 0; _ri < _rc; _ri++) {{\n{}}}}}\n",
                rc.code,
                block_stmts(ctx, body)?
            )
        }
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = String::from("{\n");
            out.push_str(&block_stmts(ctx, init)?);
            out.push_str(&format!("for (; {};) {{\n", bool_code(&rc)));
            out.push_str(&block_stmts(ctx, body)?);
            out.push_str(&block_stmts(ctx, incr)?);
            out.push_str("}\n}\n");
            out
        }
        IrStmt::Forever { body } => {
            format!("for (;;) {{\n{}}}\n", block_stmts(ctx, body)?)
        }
        IrStmt::Case { sel, kind, items } => {
            let rs = render_expr(ctx, sel)?;
            if rs.width == 0 {
                // Lowering rejects real selectors before emission.
                return Err("internal: real-valued case selector reached emission".to_string());
            }
            let cmp = kind.cmp_fn();
            let mut out = String::new();
            let mut first = true;
            let mut default_item = None;
            for item in items {
                if item.exprs.is_empty() {
                    if default_item.replace(item).is_some() {
                        return Err("internal: case has multiple default items".to_string());
                    }
                    continue;
                }
                let mut conds = Vec::with_capacity(item.exprs.len());
                for e in &item.exprs {
                    let re = render_expr(ctx, e)?;
                    conds.push(format!("sv4_to_bool({cmp}({}, {}))", rs.code, re.code));
                }
                if first {
                    first = false;
                    out.push_str(&format!("if ({}) {{\n", conds.join(" || ")));
                } else {
                    out.push_str(&format!("else if ({}) {{\n", conds.join(" || ")));
                }
                out.push_str(&block_stmts(ctx, &item.body)?);
                out.push_str("}\n");
            }
            if let Some(item) = default_item {
                if first {
                    out.push_str("if (1) {\n");
                } else {
                    out.push_str("else {\n");
                }
                out.push_str(&block_stmts(ctx, &item.body)?);
                out.push_str("}\n");
            }
            out
        }
        IrStmt::Delay { ticks } => format!("    llg_wait_time({ticks});\n"),
        IrStmt::WaitEvents { specs } => wait_events_text(ctx, specs)?,
        IrStmt::EventTrigger { ev } => {
            format!("    llg_event_trigger(&{});\n", ctx.model.event(*ev).c_name)
        }
        IrStmt::WaitAny { sens } => wait_any_text(sens),
        IrStmt::WaitCond { cond, sens, body } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!("    for (;;) {{\n        if ({}) break;\n", bool_code(&rc));
            if sens.is_empty() {
                out.push_str("        llg_wait_time(0);\n");
            } else {
                out.push_str(&wait_any_text(sens));
            }
            out.push_str("    }\n");
            out.push_str(&block_stmts(ctx, body)?);
            out
        }
        IrStmt::Fork {
            join_kind,
            branches,
        } => {
            let j = match join_kind {
                IrJoinKind::Join => "LLG_JOIN",
                IrJoinKind::None => "LLG_JOIN_NONE",
                IrJoinKind::Any => "LLG_JOIN_ANY",
            };
            let mut out = String::from("{\n");
            out.push_str(&format!(
                "    llg_fork_group_t* grp = llg_fork_group_new({j});\n"
            ));
            for (name, label) in branches {
                out.push_str(&format!("    llg_fork({name}, \"{label}\", grp);\n"));
            }
            out.push_str("    llg_join(grp);\n}\n");
            out
        }
        IrStmt::WaitFork => "    llg_wait_fork();\n".to_string(),
        IrStmt::DisableFork => "    llg_disable_fork();\n".to_string(),
        IrStmt::Force { sig, value } => {
            let sig_name = ctx.model.signal(*sig).c_name.clone();
            let rv = render_expr(ctx, value)?;
            format!("    llg_force(&{sig_name}, {});\n", rv.code)
        }
        IrStmt::Release { sig } => {
            format!("    llg_release(&{});\n", ctx.model.signal(*sig).c_name)
        }
        IrStmt::Display { fmt, args, newline } => {
            let output_fn = if *newline { "llg_display" } else { "llg_write" };
            let mut out = format!("    {output_fn}({fmt}");
            for (e, _) in args {
                out.push_str(&format!(", {}", render_expr(ctx, e)?.code));
            }
            out.push_str(");\n");
            out
        }
        IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
        } => {
            let f = if *strobe { "llg_strobe" } else { "llg_monitor" };
            format!("    {f}({fmt}, {n_args}, {eval});\n")
        }
        IrStmt::MonitorEnable(on) => {
            format!("    llg_monitor_set({});\n", (*on) as u8)
        }
        IrStmt::WaveFile(path) => {
            format!(
                "    llg_wave_file({}, llg_time());\n",
                c_string_literal(path)
            )
        }
        IrStmt::WaveDumpVars => "    llg_wave_dumpvars(llg_time());\n".to_string(),
        IrStmt::WaveOn => "    llg_wave_on(llg_time());\n".to_string(),
        IrStmt::WaveOff => "    llg_wave_off(llg_time());\n".to_string(),
        IrStmt::WaveDumpAll => "    llg_wave_dumpall(llg_time());\n".to_string(),
        IrStmt::WaveFlush => "    llg_wave_flush(llg_time());\n".to_string(),
        IrStmt::WaveLimit(limit) => format!(
            "    llg_wave_limit(sv4_to_u64({}), llg_time());\n",
            render_expr(ctx, limit)?.code
        ),
        IrStmt::Finish => "    llg_rt_finish();\n".to_string(),
        IrStmt::PrintTimescale {
            unit_ps,
            precision_ps,
            label,
        } => {
            format!(
                "    printf(\"{label}: timescale is {}/{}\\n\");\n",
                ps_to_timescale_str(*unit_ps),
                ps_to_timescale_str(*precision_ps)
            )
        }
        IrStmt::Call(call) => {
            let f = ctx.model.func(call.f);
            let mut out = String::new();
            for (tname, formal_idx, init) in &call.temps {
                let form = &f.formals[*formal_idx];
                let init = match init {
                    Some(e) => render_expr(ctx, e)?.code,
                    None => format!("sv4_x({}, {})", form.width, form.signed as u8),
                };
                out.push_str(&format!("        sv4_t {tname} = {init};\n"));
            }
            let mut call_args: Vec<String> = Vec::new();
            for arg in &call.args {
                match arg {
                    IrCallArg::Val(e) => call_args.push(render_expr(ctx, e)?.code),
                    IrCallArg::OutAddr(addr) => call_args.push(addr.clone()),
                    IrCallArg::OutTemp { name, .. } => call_args.push(format!("&{name}")),
                }
            }
            call_args.push(call.depth.code());
            out.push_str(&format!(
                "        {}({});\n",
                f.c_name,
                call_args.join(", ")
            ));
            for (lh, tname, w, s) in &call.copyouts {
                let rhs = IrExpr::new(IrExprKind::LocalRead(tname.clone()), *w, *s, None);
                out.push_str(&format!(
                    "        {}\n",
                    render_assign(ctx, lh, &rhs, false)?
                ));
            }
            out
        }
        IrStmt::Return { value } => {
            let f = ctx.func.ok_or_else(|| {
                "internal: return rendered outside a function context".to_string()
            })?;
            match (&f.ret, value) {
                (Some(crate::sim::ir::IrType::Packed { width, signed }), Some(v)) => {
                    let (w, sg) = (*width, *signed);
                    let rv = render_expr(ctx, v)?;
                    let code = match rv.fill {
                        Some(fill) => format!("sv4_fill({fill}, {}, {})", w, sg as u8),
                        None if rv.width == 0 => {
                            format!("sv4_from_real({}, {}, {})", rv.code, w, sg as u8)
                        }
                        None => arg_resize(&rv.code, w, sg),
                    };
                    format!("        _ret = {code};\n        return _ret;\n")
                }
                (Some(_), None) => "        return _ret;\n".to_string(),
                (None, _) => "        return;\n".to_string(),
                (Some(crate::sim::ir::IrType::Real { .. }), _) => {
                    return Err(
                        "internal: real function returns are rejected at lowering".to_string()
                    );
                }
            }
        }
        IrStmt::Goto(label) => format!("        goto {label};\n"),
        IrStmt::Label(label) => format!("    {label}: ;\n"),
        IrStmt::Nop => String::new(),
    };
    Ok(out)
}

/// The `llg_wait_any` suspension block (or `llg_wait_time(0)` when the read
/// set is empty).
fn wait_any_text(sens: &[String]) -> String {
    if sens.is_empty() {
        return "    llg_wait_time(0);\n".to_string();
    }
    let list = sens
        .iter()
        .map(|s| format!("&{s}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "    {{\n        sv4_t* s0[] = {{{list}}};\n        llg_wait_any(s0, {});\n    }}\n",
        sens.len()
    )
}

/// Render one atomic multi-source wait.  The call shape follows the source
/// mix: all signals keep the historical `llg_wait_any_events` text, a single
/// event uses `llg_wait_event`, an event-only list uses `llg_wait_events`,
/// and mixed signal/event lists use ONE atomic `llg_wait_mixed` call (never
/// sequential waits, so no trigger can be lost between them).  Total over IR
/// shapes: an empty spec list renders as `llg_wait_time(0)` like `WaitAny`
/// with an empty read set — unreachable from today's lowering (which routes
/// empty spec lists to `WaitAny`) but the emitter must not produce
/// non-compilable C (`ev[] = {}`) if that ever changes.
fn wait_events_text(
    ctx: &RCtx<'_>,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    if specs.is_empty() {
        return Ok("    llg_wait_time(0);\n".to_string());
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let n_events = specs
        .iter()
        .filter(|(s, _)| matches!(s, IrWaitSrc::Event(_)))
        .count();
    if n_events == 0 {
        // Pure signal or-list: the pre-events shape, byte for byte.
        let mut entries = Vec::with_capacity(specs.len());
        for (sig, edge) in specs {
            let IrWaitSrc::Sig(name) = sig else {
                unreachable!("n_events == 0 with an event entry")
            };
            entries.push(format!("{{ &{name}, {} }}", edge_kind(edge)));
        }
        return Ok(format!(
            "    {{\n        llg_event_spec_t ev[] = {{{}}};\n        \
             llg_wait_any_events(ev, {});\n    }}\n",
            entries.join(", "),
            specs.len()
        ));
    }
    if n_events == specs.len() {
        if specs.len() == 1 {
            let IrWaitSrc::Event(idx) = specs[0].0 else {
                unreachable!("n_events == specs.len() with a signal entry")
            };
            return Ok(format!(
                "    llg_wait_event(&{});\n",
                ctx.model.event(idx).c_name
            ));
        }
        let mut names = Vec::with_capacity(specs.len());
        for (src, _) in specs {
            let IrWaitSrc::Event(idx) = src else {
                unreachable!("pure-event list with a signal entry")
            };
            names.push(format!("&{}", ctx.model.event(*idx).c_name));
        }
        return Ok(format!(
            "    {{\n        const llg_event_t* const ev[] = {{{}}};\n        \
             llg_wait_events(ev, {});\n    }}\n",
            names.join(", "),
            specs.len()
        ));
    }
    // Mixed signal + event sources: one atomic registration.
    let mut entries = Vec::with_capacity(specs.len());
    for (src, edge) in specs {
        match src {
            IrWaitSrc::Sig(name) => {
                entries.push(format!("{{ &{name}, {}, 0 }}", edge_kind(edge)));
            }
            IrWaitSrc::Event(idx) => {
                entries.push(format!("{{ 0, 0, &{} }}", ctx.model.event(*idx).c_name));
            }
        }
    }
    Ok(format!(
        "    {{\n        llg_wait_src_t src[] = {{{}}};\n        \
         llg_wait_mixed(src, {});\n    }}\n",
        entries.join(", "),
        specs.len()
    ))
}

/// Render a helper function attached to a process/function: fork-branch
/// coroutines and monitor/strobe evaluators.
pub fn render_pre_fn(ctx: &RCtx<'_>, pre: &crate::sim::ir::IrPreFn) -> Result<String, String> {
    match pre {
        crate::sim::ir::IrPreFn::Branch { c_name, body } => {
            let mut out = format!("static void {c_name}(llg_proc_t* self) {{\n    (void)self;\n");
            for s in body {
                out.push_str(&render_stmt(ctx, s)?);
            }
            out.push_str("    llg_proc_done(self);\n    return;\n}\n\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::MonEval { c_name, args } => {
            let mut out = format!("static void {c_name}(sv4_t* out) {{\n    (void)out;\n");
            for (i, e) in args.iter().enumerate() {
                out.push_str(&format!("    out[{i}] = {};\n", render_expr(ctx, e)?.code));
            }
            out.push_str("}\n");
            Ok(out)
        }
    }
}

// ── Model rendering ───────────────────────────────────────────────────────────

/// Render the complete `model.c` for a lowered (and optimized) IR model:
/// the header comment the driver parses, signal/net/array storage, function
/// prototypes and bodies, process functions, and `main()`.
pub fn render(model: &IrModel) -> Result<String, String> {
    let mut out = format!(
        "// llg-generated C11 model for design `{}`\n",
        model.design_name
    );
    if model.waveform {
        out.push_str("#define LLG_WAVEFORM 1\n");
    }
    out.push_str("#include \"llg_rt.h\"\n");
    if model.waveform {
        out.push_str("#include \"llg_wave.h\"\n");
    }
    out.push_str(
        "\n#include <stdio.h>\n#include <math.h>\n\n\
         /* signals start all-X; driven by processes and link processes */\n",
    );
    render_signal_decls(model, &mut out);
    out.push('\n');
    // Arrays start all-X; elements are filled in `main()` (a function call
    // is not a valid static initializer).
    for a in &model.arrays {
        out.push_str(&format!("sv4_t {}[{}];\n", a.c_name, a.total));
    }
    out.push('\n');
    if model.waveform {
        out.push_str(
            "static uint64_t llg_wave_final_time;\n\
             static void llg_wave_capture_final_time(llg_proc_t* self) {\n\
             \x20   llg_wave_final_time = llg_time();\n\
             \x20   llg_proc_done(self);\n\
             \x20   return;\n\
             }\n\n",
        );
    }
    // Functions/tasks become static C functions (prototypes first so bodies
    // may call each other regardless of declaration order), emitted before
    // any process code references them.
    for f in &model.funcs {
        out.push_str(&func_prototype(f));
    }
    let ctx = RCtx { model, func: None };
    for f in &model.funcs {
        let fctx = RCtx {
            model,
            func: Some(f),
        };
        for pre in &f.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_func_body(&fctx, f)?);
    }
    // Three passes lower comb drivers, links, then always/initial processes,
    // so every comb process, link, and process runs at t=0 in that order;
    // push order equals spawn order.
    for p in &model.processes {
        for pre in &p.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_process_fn(&ctx, p)?);
    }
    out.push_str(&render_main(model)?);
    Ok(out)
}

/// Signal globals plus collapsed inout-net group storage.
fn render_signal_decls(model: &IrModel, out: &mut String) {
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for sig in &model.signals {
        if sig.net_driver.is_some() || sig.omit || !emitted.insert(sig.c_name.as_str()) {
            // Net-group members: storage is emitted with its group; omitted
            // signals are pruned by `unused_storage`.
            continue;
        }
        match sig.ty {
            IrType::Real { .. } => out.push_str(&format!("double {} = 0.0;\n", sig.c_name)),
            IrType::Packed { width, .. } => {
                // `SV4_X` clamps to 64 bits, so init wide signals with an
                // all-X brace initializer mirroring the runtime `sv4_x`.
                let init = if width <= 64 {
                    format!("SV4_X({width})")
                } else {
                    emit_all_x_init(width)
                };
                out.push_str(&format!("sv4_t {} = {init};\n", sig.c_name));
            }
        }
    }
    let mut groups_emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for g in &model.net_groups {
        if !groups_emitted.insert(g.c_name.as_str()) {
            continue;
        }
        let init = if g.width <= 64 {
            format!("SV4_Z({})", g.width)
        } else {
            emit_all_z_init(g.width)
        };
        let mut driver_ptrs = Vec::with_capacity(g.n_drivers);
        for slot in 0..g.n_drivers {
            let cell = format!("{}_d{}", g.c_name, slot);
            out.push_str(&format!("sv4_t {cell} = {init};\n"));
            driver_ptrs.push(format!("&{cell}"));
        }
        out.push_str(&format!(
            "static llg_net_t {} = {{ {init}, {}, {}, {}, {{ {} }} }};\n",
            g.c_name,
            g.width,
            g.signed as u8,
            g.n_drivers,
            driver_ptrs.join(", ")
        ));
    }
    // Named events: one waiter-table global per declared event (never pruned
    // — events are wakeup channels, not storage).
    for ev in &model.events {
        out.push_str(&format!(
            "static llg_event_t {} = {{{{ 0 }}, 0 }};\n",
            ev.c_name
        ));
    }
}

/// The C parameter list of a lowered function: outputs first (`o{formal
/// idx}`), then inputs (`a{formal idx}`), then the recursion depth.
fn func_params(f: &IrFunc) -> String {
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        if form.is_out {
            params.push(format!("sv4_t* o{idx}"));
        }
    }
    for (idx, form) in f.formals.iter().enumerate() {
        if !form.is_out {
            params.push(format!("sv4_t a{idx}"));
        }
    }
    params.push("int depth".to_string());
    params.join(", ")
}

fn func_prototype(f: &IrFunc) -> String {
    let ret_t = if f.ret.is_some() { "sv4_t" } else { "void" };
    format!("static {ret_t} {}({});\n", f.c_name, func_params(f))
}

/// The recursion depth guard at the top of every emitted function; it returns
/// all-X (or nothing) beyond [`crate::sim::ir::LLG_MAX_WIDTH`]-safe nesting.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

fn render_func_body(ctx: &RCtx<'_>, f: &IrFunc) -> Result<String, String> {
    let ret_t = if f.ret.is_some() { "sv4_t" } else { "void" };
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    // The all-X return value used by the recursion guard.
    let ret_clause = if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_string()
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \
         \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        \
         {ret_clause}\n    }}\n",
        f.c_name
    ));
    if let Some(IrType::Packed { width, signed }) = f.ret {
        // Function-name return variable → `_ret` local.
        out.push_str(&format!(
            "    sv4_t _ret = sv4_x({width}, {});\n",
            signed as u8
        ));
    }
    for l in &f.locals {
        out.push_str(&format!(
            "    sv4_t {} = sv4_x({}, {});\n",
            l.c_name, l.width, l.signed as u8
        ));
    }
    out.push_str(&block_stmts_of(ctx, &f.body)?);
    out.push_str("    ");
    if f.ret.is_some() {
        out.push_str("return _ret;\n");
    }
    out.push_str("}\n\n");
    Ok(out)
}

fn block_stmts_of(ctx: &RCtx<'_>, stmts: &[crate::sim::ir::IrStmt]) -> Result<String, String> {
    let mut out = String::new();
    for s in stmts {
        out.push_str(&render_stmt(ctx, s)?);
    }
    Ok(out)
}

fn render_process_fn(ctx: &RCtx<'_>, p: &crate::sim::ir::IrProcess) -> Result<String, String> {
    use crate::sim::ir::{IrShape, IrStmt};
    let mut out = format!(
        "static void {}(llg_proc_t* self) {{\n    (void)self;\n",
        p.c_name
    );
    let body = block_stmts_of(ctx, &p.body)?;
    match &p.shape {
        IrShape::RunOnce => {
            out.push_str(&body);
            out.push_str("    llg_proc_done(self);\n    return;\n");
        }
        IrShape::Loop => {
            // always / always_comb / always_ff bodies contain their own waits.
            out.push_str("for (;;) {\n");
            out.push_str(&body);
            out.push_str("    }\n");
        }
        IrShape::SensLoop { reads } => {
            out.push_str(&body);
            out.push_str("    for (;;) {\n");
            out.push_str(&wait_any_text(reads));
            // The in-loop copy indents one level deeper than the first
            // evaluation (plain drivers and begin blocks alike).
            // Control-flow labels in the body (break/continue/disable
            // targets) would be DEFINED twice — once per copy — so the
            // copy is relabeled with a suffix.  Lowering guarantees every
            // `goto` targets a label inside the same body tree, so the
            // rename stays internally consistent.
            let renamed = rename_stmt_labels(&p.body);
            for s in &renamed {
                let text = render_stmt(ctx, s)?;
                out.push_str("    ");
                out.push_str(&text);
            }
            out.push_str("    }\n");
        }
    }
    let _ = IrStmt::Nop;
    out.push_str("}\n\n");
    Ok(out)
}

/// Suffix appended to control-flow labels in the re-evaluation copy of a
/// combinational (`SensLoop`) process body.
const LOOP_COPY_SUFFIX: &str = "_r";

/// Collect every label DEFINED in a statement tree.
fn collect_label_names(stmts: &[crate::sim::ir::IrStmt], out: &mut HashSet<String>) {
    use crate::sim::ir::IrStmt;
    for s in stmts {
        match s {
            IrStmt::Label(l) => {
                out.insert(l.clone());
            }
            IrStmt::Block(b) | IrStmt::Forever { body: b } => collect_label_names(b, out),
            IrStmt::If { then_, els, .. } => {
                collect_label_names(then_, out);
                if let Some(els) = els {
                    collect_label_names(els, out);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                collect_label_names(b, out)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_label_names(init, out);
                collect_label_names(incr, out);
                collect_label_names(body, out);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_label_names(&item.body, out);
                }
            }
            IrStmt::WaitCond { body: b, .. } => collect_label_names(b, out),
            _ => {}
        }
    }
}

/// Rewrite `Label`/`Goto` strings in place for the names defined in
/// `names` (a goto can only target a label defined in the same tree).
fn rename_labels_in(stmts: &mut [crate::sim::ir::IrStmt], names: &HashSet<String>) {
    use crate::sim::ir::IrStmt;
    for s in stmts {
        match s {
            IrStmt::Label(l) | IrStmt::Goto(l) => {
                if names.contains(l.as_str()) {
                    l.push_str(LOOP_COPY_SUFFIX);
                }
            }
            IrStmt::Block(b) | IrStmt::Forever { body: b } => rename_labels_in(b, names),
            IrStmt::If { then_, els, .. } => {
                rename_labels_in(then_, names);
                if let Some(els) = els {
                    rename_labels_in(els, names);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                rename_labels_in(b, names)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                rename_labels_in(init, names);
                rename_labels_in(incr, names);
                rename_labels_in(body, names);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    rename_labels_in(&mut item.body, names);
                }
            }
            IrStmt::WaitCond { body: b, .. } => rename_labels_in(b, names),
            _ => {}
        }
    }
}

/// A relabeled clone of a combinational process body for its re-evaluation
/// copy (see the `SensLoop` renderer).  Bodies without labels are returned
/// unchanged.
fn rename_stmt_labels(stmts: &[crate::sim::ir::IrStmt]) -> Vec<crate::sim::ir::IrStmt> {
    let mut names = HashSet::new();
    collect_label_names(stmts, &mut names);
    let mut out = stmts.to_vec();
    if !names.is_empty() {
        rename_labels_in(&mut out, &names);
    }
    out
}

fn render_main(model: &IrModel) -> Result<String, String> {
    use crate::sim::ir::IrInitStep;
    let mut out = String::from("int main(void) {\n    llg_rt_init();\n");
    for step in &model.init_steps {
        match step {
            IrInitStep::FillArrayX(arr) => {
                let a = model.array(*arr);
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = sv4_x({}, {}); }}\n",
                    a.total, a.c_name, a.elem_width, a.signed as u8
                ));
            }
            IrInitStep::SetArrayElem { arr, index, value } => {
                let a = model.array(*arr);
                out.push_str(&format!(
                    "    {}[{}] = {};\n",
                    a.c_name,
                    index,
                    emit_const_for_vector(value, a.elem_width, a.signed)?
                ));
            }
            IrInitStep::SetScalar { sig, value } => {
                let s = model.signal(*sig);
                let v = match s.ty {
                    IrType::Real { shortreal } => {
                        round_shortreal(emit_const_for_real(value), shortreal)
                    }
                    IrType::Packed { width, signed } => {
                        emit_const_for_vector(value, width, signed)?
                    }
                };
                out.push_str(&format!("    {} = {};\n", s.c_name, v));
            }
            IrInitStep::WriteNet { group, slot, value } => {
                let g = model.net_group(*group);
                out.push_str(&format!(
                    "    llg_net_write(&{}, {}, {});\n",
                    g.c_name,
                    slot,
                    emit_const(value)
                ));
            }
        }
    }
    if model.waveform {
        out.push_str(&format!(
            "    if (llg_wave_model_init({}ULL) != 0) return 1;\n",
            model.precision_ps
        ));
        for sig in &model.signals {
            let Some(hdl_name) = &sig.hdl_name else {
                continue;
            };
            if sig.omit {
                continue;
            }
            let registration = match sig.ty {
                IrType::Packed { width, .. } => format!(
                    "llg_wave_register_sv4({}, &{}, {})",
                    c_string_literal(hdl_name),
                    sig.c_name,
                    width
                ),
                IrType::Real { .. } => format!(
                    "llg_wave_register_real({}, &{})",
                    c_string_literal(hdl_name),
                    sig.c_name
                ),
            };
            out.push_str(&format!("    if ({registration} != 0) return 1;\n"));
        }
        for array in &model.arrays {
            for index in 0..array.total {
                let hdl_name = format!("{}[{index}]", array.hdl_name);
                out.push_str(&format!(
                    "    if (llg_wave_register_sv4({}, &{}[{}], {}) != 0) return 1;\n",
                    c_string_literal(&hdl_name),
                    array.c_name,
                    index,
                    array.elem_width
                ));
            }
        }
    }
    for (fname, label) in model.spawn_list() {
        out.push_str(&format!("    llg_spawn({fname}, \"{label}\");\n"));
    }
    // Capture scheduler exit time before user finals. Finals cannot advance
    // time, and registering this first also preserves the timestamp if a
    // user final calls `$finish` and stops the remaining final queue.
    if model.waveform {
        out.push_str(
            "    llg_spawn_final(llg_wave_capture_final_time, \
             \"llg.wave.capture_final_time\");\n",
        );
    }
    // Final blocks (`final begin … end`, SV 1800-2005 §10.7) register with
    // the runtime and run after the main scheduler loop exits.
    for fname in &model.final_spawns {
        let label = model
            .processes
            .iter()
            .find(|p| p.c_name == *fname)
            .map(|p| p.label.clone())
            .unwrap_or_default();
        out.push_str(&format!("    llg_spawn_final({fname}, \"{label}\");\n"));
    }
    out.push_str("    llg_rt_run();\n");
    if !model.final_spawns.is_empty() || model.waveform {
        out.push_str("    llg_rt_run_finals();\n");
    }
    if model.waveform {
        out.push_str("    return llg_wave_close(llg_wave_final_time);\n}\n");
    } else {
        out.push_str("    return 0;\n}\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{
        IrArray, IrConst, IrExpr, IrExprKind, IrProcess, IrShape, IrSignal, IrStmt,
    };

    fn packed_const(value: u64) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![value],
                x: vec![0],
                z: vec![0],
                width: 64,
                signed: false,
                real: None,
                fill: None,
            }),
            64,
            false,
            None,
        )
    }

    #[test]
    fn non_waveform_model_has_no_waveform_integration() {
        let c = render(&IrModel {
            design_name: "plain".to_string(),
            ..IrModel::default()
        })
        .unwrap();

        assert!(!c.contains("#define LLG_WAVEFORM 1"));
        assert!(!c.contains("llg_wave.h"));
        assert!(!c.contains("llg_wave_model_init"));
        assert!(c.ends_with("    return 0;\n}\n"));
    }

    #[test]
    fn waveform_model_emits_controls_hierarchy_and_final_time_close() {
        let controls = vec![
            IrStmt::WaveFile("trace\\\"name.vcd".to_string()),
            IrStmt::WaveDumpVars,
            IrStmt::WaveOn,
            IrStmt::WaveOff,
            IrStmt::WaveDumpAll,
            IrStmt::WaveFlush,
            IrStmt::WaveLimit(packed_const(4096)),
        ];
        let model = IrModel {
            design_name: "top".to_string(),
            precision_ps: 10,
            waveform: true,
            signals: vec![
                IrSignal {
                    c_name: "G_top_g_0__value".to_string(),
                    hdl_name: Some("top\u{1f}g[0]\u{1f}value".to_string()),
                    ty: IrType::Packed {
                        width: 12,
                        signed: false,
                    },
                    net_driver: None,
                    omit: false,
                },
                IrSignal {
                    c_name: "g_net_0.resolved".to_string(),
                    hdl_name: Some("top\u{1f}alias".to_string()),
                    ty: IrType::Packed {
                        width: 1,
                        signed: false,
                    },
                    net_driver: Some((0, 0)),
                    omit: false,
                },
                IrSignal {
                    c_name: "D_top_r".to_string(),
                    hdl_name: Some("top\u{1f}r".to_string()),
                    ty: IrType::Real { shortreal: false },
                    net_driver: None,
                    omit: false,
                },
                IrSignal {
                    c_name: "G_top_pca$0_en".to_string(),
                    hdl_name: None,
                    ty: IrType::Packed {
                        width: 1,
                        signed: false,
                    },
                    net_driver: None,
                    omit: false,
                },
            ],
            net_groups: vec![crate::sim::ir::IrNetGroup {
                c_name: "g_net_0".to_string(),
                width: 1,
                signed: false,
                n_drivers: 1,
            }],
            arrays: vec![IrArray {
                c_name: "G_top_mem".to_string(),
                hdl_name: "top\u{1f}mem".to_string(),
                elem_width: 8,
                signed: false,
                dims: vec![(1, 0)],
                total: 2,
            }],
            processes: vec![IrProcess {
                c_name: "p_top_initial_0".to_string(),
                label: "top.initial".to_string(),
                shape: IrShape::RunOnce,
                pre_fns: Vec::new(),
                body: controls,
            }],
            spawns: vec!["p_top_initial_0".to_string()],
            ..IrModel::default()
        };

        let c = render(&model).unwrap();

        assert_eq!(c.matches("#define LLG_WAVEFORM 1").count(), 1);
        assert!(c.contains("#include \"llg_wave.h\""));
        assert!(c.contains("llg_wave_file(\"trace\\\\\\\"name.vcd\", llg_time());"));
        assert!(c.contains("llg_wave_dumpvars(llg_time());"));
        assert!(c.contains("llg_wave_on(llg_time());"));
        assert!(c.contains("llg_wave_off(llg_time());"));
        assert!(c.contains("llg_wave_dumpall(llg_time());"));
        assert!(c.contains("llg_wave_flush(llg_time());"));
        assert!(c.contains("llg_wave_limit(sv4_to_u64("));
        assert!(c.contains("llg_wave_model_init(10ULL)"));
        assert!(
            c.contains("llg_wave_register_sv4(\"top\\037g[0]\\037value\", &G_top_g_0__value, 12)")
        );
        assert!(c.contains("llg_wave_register_sv4(\"top\\037alias\", &g_net_0.resolved, 1)"));
        assert!(c.contains("llg_wave_register_real(\"top\\037r\", &D_top_r)"));
        assert!(!c.contains("llg_wave_register_sv4(\"G_top_pca$0_en"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[0]\", &G_top_mem[0], 8)"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[1]\", &G_top_mem[1], 8)"));
        assert!(c.contains("llg_spawn_final(llg_wave_capture_final_time"));
        assert!(c.contains("return llg_wave_close(llg_wave_final_time);"));
    }
}
