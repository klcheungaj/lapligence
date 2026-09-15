//! Constants.

use super::*;

// ── Constant ↔ Value conversion ───────────────────────────────────────────────

fn const_limb_bit(limbs: &[u64], i: usize) -> bool {
    limbs.get(i / 64).copied().unwrap_or(0) & (1u64 << (i % 64)) != 0
}

/// The packed value of an `IrConst`.  The unsized-fill marker is dropped:
/// runtime operations never see it (it only steers assignment conversions),
/// so folding must extend like the C `sv4_*` helpers do — zero or sign, never
/// fill.
fn const_to_value(c: &IrConst) -> Option<Value> {
    if c.real.is_some() {
        return None;
    }
    // `Value.bits` is MSB-first; limb bit `i` counts from the LSB.
    let bit_at = |i: usize| {
        if const_limb_bit(&c.x, i) {
            Bit::X
        } else if const_limb_bit(&c.z, i) {
            Bit::Z
        } else if const_limb_bit(&c.bits, i) {
            Bit::One
        } else {
            Bit::Zero
        }
    };
    let bits: Vec<Bit> = (0..c.width as usize).rev().map(bit_at).collect();
    Some(Value {
        bits,
        signed: c.signed,
        fill: None,
    })
}

pub(super) fn value_to_const(v: &Value) -> IrConst {
    let nlimbs = v.width().div_ceil(64).max(1);
    let mut bits = vec![0u64; nlimbs];
    let mut x = vec![0u64; nlimbs];
    let mut z = vec![0u64; nlimbs];
    for i in 0..v.width() {
        match v.bit_lsb(i) {
            Bit::One => bits[i / 64] |= 1u64 << (i % 64),
            Bit::X => x[i / 64] |= 1u64 << (i % 64),
            Bit::Z => z[i / 64] |= 1u64 << (i % 64),
            Bit::Zero => {}
        }
    }
    IrConst {
        bits,
        x,
        z,
        width: v.width() as u32,
        signed: v.signed,
        real: None,
        fill: None,
    }
}

/// The packed constant payload of `e`, when its kind is a plain constant.
pub(super) fn as_packed_const(e: &IrExpr) -> Option<Value> {
    match &e.kind {
        IrExprKind::Const(c) => const_to_value(c),
        _ => None,
    }
}

/// The literal-real payload of `e`, when present.
pub(super) fn real_of(e: &IrExpr) -> Option<f64> {
    match &e.kind {
        IrExprKind::Const(c) => c.real,
        _ => None,
    }
}
