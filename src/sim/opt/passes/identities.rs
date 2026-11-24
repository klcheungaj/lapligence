//! Identities.

use super::*;

// ── Pass: identities ──────────────────────────────────────────────────────────

/// Apply the identity rules at `e`, whose operands the caller has rewritten: the
/// expression walk visits them first. Each rule replaces `e` with one of its
/// descendants or fixes the bounds of a select, so the rules run out.
pub(super) fn ident_expr(e: &mut IrExpr) {
    while try_identity(e) {}
}

/// Move an operand out of the expression it is about to replace.
fn take(operand: &mut IrExpr) -> IrExpr {
    std::mem::replace(operand, IrExpr::new(IrExprKind::Fill(0), 1, false, Some(0)))
}

fn is_zero_const(e: &IrExpr) -> bool {
    matches!(as_packed_const(e).and_then(|v| v.to_u64()), Some(0))
}

/// The value of a fully known constant select index, read as the runtime
/// does (`sv4_to_index_i64`: signed values sign-extend).
fn known_select_index(e: &IrExpr) -> Option<i64> {
    let value = as_packed_const(e).filter(|value| !value.is_unknown())?;
    if value.signed {
        value.to_i128().and_then(|index| i64::try_from(index).ok())
    } else {
        value.to_u128().and_then(|index| i64::try_from(index).ok())
    }
}

/// Try one algebraic identity at this node; `true` when the node was replaced.
fn try_identity(e: &mut IrExpr) -> bool {
    match &mut e.kind {
        // BitNeg(BitNeg(x)) → x only for a proven known packed constant.
        // For a runtime Z, the first negation produces X and the second must
        // remain X, so the algebraic identity is not generally valid.
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a,
        } => {
            if let IrExprKind::Un {
                op: IrUnOp::BitNeg,
                a: inner,
            } = &mut a.kind
            {
                if as_packed_const(inner).is_some_and(|v| !v.is_unknown()) {
                    let replacement = take(inner);
                    *e = replacement;
                    return true;
                }
            }
            false
        }
        // shift by constant 0 → base (same shape guard as the other rules:
        // the replacement must keep the node's recorded width/signedness)
        IrExprKind::Bin {
            op: IrBinOp::Shl | IrBinOp::Shr | IrBinOp::Ashl | IrBinOp::Ashr,
            a,
            b,
        } => {
            if is_zero_const(b) && a.width == e.width && a.signed == e.signed {
                let replacement = take(a);
                *e = replacement;
                return true;
            }
            false
        }
        // Concatenation is unsigned and self-determined, including a single
        // operand. Do not expose a signed child or resurrect a fill marker
        // (which can also live on the literal rather than its enclosing node).
        IrExprKind::Concat { parts } if parts.len() == 1 => {
            let part = &mut parts[0];
            if part.width == e.width
                && part.signed == e.signed
                && e.fill.is_none()
                && part.fill.is_none()
                && !matches!(&part.kind, IrExprKind::Fill(_))
                && !matches!(&part.kind, IrExprKind::Const(value) if value.fill.is_some())
            {
                let replacement = take(part);
                *e = replacement;
                return true;
            }
            false
        }
        // mux with a fully known constant select → chosen branch
        // (an X/Z select must NOT choose).  The replacement keeps the mux's
        // self-determined shape: Verilog widens ?: to max(branches), so a
        // narrower branch may not stand in for it (concat/replication
        // operands are width-sensitive at runtime).
        IrExprKind::Mux { sel, a, b }
        | IrExprKind::ArrayMux { sel, a, b, .. }
        | IrExprKind::StructMux { sel, a, b, .. } => {
            let pick = as_packed_const(sel).and_then(|v| v.to_u64().map(|u| u != 0));
            match pick {
                Some(choice) => {
                    let chosen = if choice { a } else { b };
                    if chosen.width == e.width && chosen.signed == e.signed && chosen.fill.is_none()
                    {
                        let replacement = take(chosen);
                        *e = replacement;
                        true
                    } else {
                        false
                    }
                }
                None => false,
            }
        }
        // An indexed part-select whose index is a known constant (a
        // generate iteration's `v[i % 28 +: 4]`) reads the same window as a
        // constant part-select: both copy `width` bits from the lowest
        // selected position and give x for positions outside the value. The
        // constant form builds no index value at run time, and a shared
        // instance body keeps the bounds as per-instance integers. Only
        // windows starting at bit 0 or above fold; an x/z index, or `-:`
        // below zero, stays dynamic (all x). A bit-select stays one: the
        // runtime reads a single bit of any width directly, while a
        // part-select of a value wider than 64 bits copies a window.
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            neg,
            ..
        } if e.width > 0 => {
            let span = i64::from(e.width) - 1;
            let low = known_select_index(base_idx).and_then(|index| {
                if *neg {
                    (index >= 0).then(|| index - span)
                } else {
                    Some(index)
                }
            });
            match low
                .filter(|low| *low >= 0)
                .and_then(|low| low.checked_add(span).map(|left| (left, low)))
            {
                Some((left, right)) => {
                    let base = Box::new(take(base));
                    e.kind = IrExprKind::PartSel { base, left, right };
                    true
                }
                None => false,
            }
        }
        // Resize(a, w, s) where a already has (w, s) → a
        IrExprKind::Resize { a } if a.width == e.width && a.signed == e.signed => {
            let replacement = take(a);
            *e = replacement;
            true
        }
        // Convert(a, w, s) where a already has (w, s) → a (sv4_cast on an
        // identical shape is the identity)
        IrExprKind::Convert { a } if a.width == e.width && a.signed == e.signed => {
            let replacement = take(a);
            *e = replacement;
            true
        }
        _ => false,
    }
}
