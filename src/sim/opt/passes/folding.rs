//! Folding.

use super::*;

// ── Pass: fold_constants ──────────────────────────────────────────────────────

/// Bottom-up constant folding: children first (the walker visits them before
/// calling this), then this node when its operands are all constants.
pub(super) fn fold_expr(e: &mut IrExpr) {
    let folded = match &e.kind {
        IrExprKind::Bin { op, a, b } => match (as_packed_const(a), as_packed_const(b)) {
            (Some(va), Some(vb)) => bin_value(*op, &va, &vb, e.width, e.signed).map(Folded::Bits),
            _ => None,
        },
        IrExprKind::Un { op, a } => as_packed_const(a)
            .and_then(|va| un_value(*op, &va, e.width))
            .map(Folded::Bits),
        IrExprKind::Mux { sel, a, b } => {
            match (as_packed_const(sel), as_packed_const(a), as_packed_const(b)) {
                (Some(vs), Some(va), Some(vb)) => Some(Folded::Bits(
                    elab::cond(&vs, &va, &vb).resize(e.width as usize, e.signed),
                )),
                _ => None,
            }
        }
        IrExprKind::Concat { parts } => {
            let mut vals = Vec::with_capacity(parts.len());
            for p in parts {
                match as_packed_const(p) {
                    Some(v) => vals.push(v),
                    None => return,
                }
            }
            Some(Folded::Bits(elab::concat(&vals)))
        }
        IrExprKind::Replicate { count, parts } => {
            let mut vals = Vec::with_capacity(parts.len());
            for p in parts {
                match as_packed_const(p) {
                    Some(v) => vals.push(v),
                    None => return,
                }
            }
            let pat = elab::concat(&vals);
            if *count == 0 || pat.is_unknown() {
                return;
            }
            let Some(total) = pat.width().checked_mul(*count as usize) else {
                return;
            };
            // Width admission belongs to lowering/IR validation. Refuse an
            // inconsistent node here without importing backend policy.
            if total != e.width as usize {
                return;
            }
            let mut bits = Vec::with_capacity(total);
            for _ in 0..*count {
                bits.extend(pat.bits.iter().cloned());
            }
            Some(Folded::Bits(Value::from_bits(bits, false)))
        }
        IrExprKind::Resize { a } => as_packed_const(a)
            .map(|va| va.resize(e.width as usize, e.signed))
            .map(Folded::Bits),
        // Mirror the runtime's `sv4_cast` (value-preserving conversion).
        IrExprKind::Convert { a } => as_packed_const(a)
            .map(|va| va.cast(e.width as usize, e.signed))
            .map(Folded::Bits),
        IrExprKind::ToTwoState { a } => as_packed_const(a).map(|mut value| {
            for bit in &mut value.bits {
                if matches!(bit, Bit::X | Bit::Z) {
                    *bit = Bit::Zero;
                }
            }
            Folded::Bits(value)
        }),
        // Real → packed rounding lives in the C runtime (`sv4_from_real`);
        // do not reproduce it here.
        IrExprKind::CastToPacked { .. } => None,
        IrExprKind::CastToReal { a, shortreal } => as_packed_const(a).map(|va| {
            let r = va.to_real();
            // A shortreal target rounds through C `float` at runtime
            // (`round_shortreal`: `(double)(float)x`); folding must produce
            // that exact value or downstream real math diverges.  `as f32`
            // rounds to nearest-even like the C conversion and widening back
            // is exact — the same emulation `core::elab` applies to shortreal
            // parameters.
            Folded::Real(if *shortreal { (r as f32) as f64 } else { r })
        }),
        IrExprKind::RealBin { op, a, b } => match (real_of(a), real_of(b)) {
            (Some(x), Some(y)) => {
                let r = match op {
                    IrRealBinOp::Add => x + y,
                    IrRealBinOp::Sub => x - y,
                    IrRealBinOp::Mul => x * y,
                    IrRealBinOp::Div => x / y,
                    IrRealBinOp::Mod => x % y,
                    IrRealBinOp::Pow => x.powf(y),
                };
                Some(Folded::Real(r))
            }
            _ => None,
        },
        IrExprKind::RealUn { op, a } => real_of(a).map(|x| match op {
            IrRealUnOp::Neg => Folded::Real(-x),
        }),
        IrExprKind::SysFunc(IrSysFunc::Clog2(a)) => {
            as_packed_const(a).map(|va| Folded::Bits(elab::clog2(&va)))
        }
        _ => None,
    };
    if let Some(folded) = folded {
        match folded {
            Folded::Bits(v) => {
                e.kind = IrExprKind::Const(value_to_const(&v));
            }
            Folded::Real(r) => {
                e.kind = IrExprKind::Const(IrConst {
                    bits: vec![0],
                    x: vec![0],
                    z: vec![0],
                    width: 0,
                    signed: true,
                    real: Some(r),
                    fill: None,
                });
            }
        }
    }
}

pub(super) enum Folded {
    Bits(Value),
    Real(f64),
}

fn bin_value(
    op: IrBinOp,
    a: &Value,
    b: &Value,
    node_width: u32,
    node_signed: bool,
) -> Option<Value> {
    use IrBinOp::*;
    // Division/modulo/power stay runtime calls unless both operands are fully
    // known and within the runtime's 64-bit operand limit.
    if matches!(op, Div | Mod | Pow)
        && (a.is_unknown() || b.is_unknown() || a.width() > 64 || b.width() > 64)
    {
        return None;
    }
    let v = match op {
        Add => elab::add(a, b),
        Sub => elab::sub(a, b),
        Mul => elab::mul(a, b),
        Div => elab::div(a, b),
        Mod => elab::rem(a, b),
        Pow => elab::power(a, b),
        BitAnd => elab::bit_and(a, b),
        BitOr => elab::bit_or(a, b),
        BitXor => elab::bit_xor(a, b),
        BitXNor => elab::bit_xnor(a, b),
        LogAnd => elab::log_and(a, b),
        LogOr => elab::log_or(a, b),
        LogImpl => elab::log_imply(a, b),
        LogEquiv => elab::log_equiv(a, b),
        Eq => elab::eq(a, b),
        Neq => elab::neq(a, b),
        CaseEq => elab::case_eq(a, b),
        CaseNeq => elab::case_neq(a, b),
        WildEq => elab::wildcard_eq(a, b),
        WildNeq => elab::wildcard_neq(a, b),
        Lt => elab::lt(a, b),
        Le => elab::le(a, b),
        Gt => elab::gt(a, b),
        Ge => elab::ge(a, b),
        Shl => elab::shl(a, b),
        Shr => elab::shr(a, b),
        Ashl => elab::arith_shl(a, b),
        Ashr => elab::arith_shr(a, b),
    };
    Some(resize_like_runtime(&v, node_width, node_signed))
}

fn un_value(op: IrUnOp, a: &Value, node_width: u32) -> Option<Value> {
    use IrUnOp::*;
    let v = match op {
        Neg => elab::minus(a),
        LogNot => elab::log_not(a),
        BitNeg => elab::bit_neg(a),
        RedAnd => elab::unary_and(a),
        RedNand => elab::unary_nand(a),
        RedOr => elab::unary_or(a),
        RedNor => elab::unary_nor(a),
        RedXor => elab::unary_xor(a),
        RedXNor => elab::unary_xnor(a),
    };
    Some(resize_like_runtime(&v, node_width, v.signed))
}

/// Retag/extend a folded value to the node's recorded shape without applying
/// fill-extension semantics (the runtime helpers never do).
fn resize_like_runtime(v: &Value, width: u32, signed: bool) -> Value {
    let mut plain = v.clone();
    if plain.fill.is_some() {
        plain.fill = None;
    }
    plain.resize(width as usize, signed)
}
