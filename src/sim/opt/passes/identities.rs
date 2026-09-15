//! Identities.

use super::*;

// ── Pass: identities ──────────────────────────────────────────────────────────

pub(super) fn ident_expr(e: &mut IrExpr) {
    ident_children(e);
    for _ in 0..8 {
        if !try_identity(e) {
            break;
        }
        ident_children(e);
    }
}

/// Apply identity rules to every descendant (children first).
fn ident_children(e: &mut IrExpr) {
    match &mut e.kind {
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            ident_expr(a);
            ident_expr(b);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::ToTwoState { a } => ident_expr(a),
        IrExprKind::CastToReal { a, .. } => ident_expr(a),
        IrExprKind::Mux { sel, a, b } => {
            ident_expr(sel);
            ident_expr(a);
            ident_expr(b);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                ident_expr(p);
            }
        }
        IrExprKind::Stream { value, .. } => ident_expr(value),
        IrExprKind::Mutation(mutation) => {
            walk_lhs_mut(&mut mutation.lhs, &mut |child| ident_expr(child));
            ident_expr(&mut mutation.value);
        }
        IrExprKind::BitStreamCast { a, .. } => ident_expr(a),
        IrExprKind::DynamicCast(cast) => {
            walk_lhs_mut(&mut cast.lhs, &mut |child| ident_expr(child));
            ident_expr(&mut cast.rhs);
            if let Some(source) = &mut cast.class_source {
                source.expressions_mut(&mut |child| ident_expr(child));
            }
            for value in &mut cast.valid_values {
                ident_expr(value);
            }
        }
        IrExprKind::Inside { value, items } => {
            ident_expr(value);
            for item in items {
                match item {
                    IrInsideItem::Value(item) => ident_expr(item),
                    IrInsideItem::Range { low, high } => {
                        ident_expr(low);
                        ident_expr(high);
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        if let Some(low) = low {
                            ident_expr(low);
                        }
                        if let Some(high) = high {
                            ident_expr(high);
                        }
                    }
                    IrInsideItem::Container { .. } => {}
                }
            }
        }
        IrExprKind::BitSel { base, idx } => {
            ident_expr(base);
            ident_expr(idx);
        }
        IrExprKind::PartSel { base, .. } => ident_expr(base),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            ident_expr(base);
            ident_expr(base_idx);
            ident_expr(width_expr);
        }
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => {
            for i in indices {
                ident_expr(i);
            }
            if let IrElemSel::Bit(idx) | IrElemSel::Indexed { base: idx, .. } = elem_sel {
                ident_expr(idx);
            }
        }
        IrExprKind::EnumMethod(query) => {
            query.expressions_mut(&mut |child| ident_expr(child));
        }
        IrExprKind::CallFn(call) => {
            for arg in &mut call.args {
                match arg {
                    IrCallArg::Val(ex) => ident_expr(ex),
                    IrCallArg::StringVal(value) => {
                        value.expressions_mut(&mut |child| ident_expr(child));
                    }
                    IrCallArg::OutTemp {
                        init,
                        writeback,
                        storage_lhs,
                        storage_read,
                        selector_inits,
                        ..
                    } => {
                        if let Some(init) = init {
                            ident_expr(init);
                        }
                        ident_lhs(writeback);
                        if let Some(storage_lhs) = storage_lhs {
                            ident_lhs(storage_lhs);
                        }
                        if let Some(storage_read) = storage_read {
                            ident_expr(storage_read);
                        }
                        for (_, _, _, _, init) in selector_inits {
                            ident_expr(init);
                        }
                    }
                    IrCallArg::OutAddr(_)
                    | IrCallArg::StringOutAddr(_)
                    | IrCallArg::StringRefAddr { .. }
                    | IrCallArg::ChandleVal(_)
                    | IrCallArg::ChandleAddr(_)
                    | IrCallArg::ChandleRefAddr(_) => {}
                    IrCallArg::RefAddr { read, lhs, .. } => {
                        ident_expr(read);
                        ident_lhs(lhs);
                    }
                    IrCallArg::StringOutTemp {
                        init, storage_read, ..
                    } => {
                        if let Some(init) = init {
                            init.expressions_mut(&mut |child| ident_expr(child));
                        }
                        if let Some(read) = storage_read {
                            read.expressions_mut(&mut |child| ident_expr(child));
                        }
                    }
                }
            }
        }
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::TestPlusArgs { pattern } => {
                pattern.expressions_mut(&mut |expression| ident_expr(expression))
            }
            IrSysFunc::ValuePlusArgs { format, target } => {
                format.expressions_mut(&mut |expression| ident_expr(expression));
                match target {
                    crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                    | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => ident_lhs(lhs),
                    crate::sim::ir::IrPlusArgTarget::String { .. } => {}
                }
            }
            IrSysFunc::System(Some(command)) => {
                command.expressions_mut(&mut |child| ident_expr(child));
            }
            IrSysFunc::System(None) => {}
            IrSysFunc::VpiCall { args, .. } => {
                for arg in args {
                    ident_expr(arg);
                }
            }
            IrSysFunc::LegacyRandom { seed, args, .. } => {
                if let Some(seed) = seed {
                    ident_lhs(seed);
                }
                for arg in args {
                    ident_expr(arg);
                }
            }
            IrSysFunc::Urandom { seed } => {
                if let Some(seed) = seed {
                    ident_expr(seed);
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                ident_expr(max);
                if let Some(min) = min {
                    ident_expr(min);
                }
            }
            IrSysFunc::Clog2(a)
            | IrSysFunc::Bits(a)
            | IrSysFunc::BitQuery { arg: a, .. }
            | IrSysFunc::Rtoi(a)
            | IrSysFunc::Itor(a)
            | IrSysFunc::RealToBits(a)
            | IrSysFunc::BitsToReal(a)
            | IrSysFunc::ShortRealToBits(a)
            | IrSysFunc::BitsToShortReal(a) => ident_expr(a),
            IrSysFunc::Math { args, .. } => {
                for arg in args {
                    ident_expr(arg);
                }
            }
            IrSysFunc::QFull { q_id, status } => {
                ident_expr(q_id);
                ident_lhs(status);
            }
            IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
            IrSysFunc::FileOpen { path, mode } => {
                path.expressions_mut(&mut |expression| ident_expr(expression));
                if let Some(mode) = mode {
                    mode.expressions_mut(&mut |expression| ident_expr(expression));
                }
            }
            IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => {
                ident_expr(descriptor)
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                ident_expr(descriptor);
                ident_expr(offset);
                ident_expr(operation);
            }
            IrSysFunc::FileError { descriptor, .. } => ident_expr(descriptor),
            IrSysFunc::FileInput(input) => {
                input.expressions_mut(&mut |expression| ident_expr(expression));
            }
            IrSysFunc::Sampled(call) => ident_expr(&mut call.argument),
        },
        _ => {}
    }
}

fn ident_lhs(l: &mut IrLhs) {
    match l {
        IrLhs::Bit(_, idx, _) => ident_expr(idx),
        IrLhs::IdxPart(_, base, width, _, _, _) => {
            ident_expr(base);
            ident_expr(width);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for i in indices {
                ident_expr(i);
            }
            if let IrElemSel::Bit(idx) | IrElemSel::Indexed { base: idx, .. } = elem_sel {
                ident_expr(idx);
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                ident_lhs(part);
            }
        }
        _ => {}
    }
}

fn is_zero_const(e: &IrExpr) -> bool {
    matches!(as_packed_const(e).and_then(|v| v.to_u64()), Some(0))
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
            } = &a.kind
            {
                if as_packed_const(inner).is_some_and(|v| !v.is_unknown()) {
                    let replacement = (**inner).clone();
                    *e = replacement;
                    return true;
                }
            }
            false
        }
        // shift by constant 0 → base (same shape guard as the other rules:
        // the replacement must keep the node's recorded width/signedness)
        IrExprKind::Bin {
            op: op @ (IrBinOp::Shl | IrBinOp::Shr | IrBinOp::Ashl | IrBinOp::Ashr),
            a,
            b,
        } => {
            if is_zero_const(b) && a.width == e.width && a.signed == e.signed {
                let replacement = (**a).clone();
                let _ = op;
                *e = replacement;
                return true;
            }
            false
        }
        // single-part concat whose part already has the full width → part
        IrExprKind::Concat { parts } if parts.len() == 1 => {
            if parts[0].width == e.width {
                let replacement = parts[0].clone();
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
        IrExprKind::Mux { sel, a, b } => {
            let pick = as_packed_const(sel).and_then(|v| v.to_u64().map(|u| u != 0));
            match pick {
                Some(choice) => {
                    let chosen = if choice { &**a } else { &**b };
                    if chosen.width == e.width && chosen.signed == e.signed && chosen.fill.is_none()
                    {
                        let replacement = chosen.clone();
                        *e = replacement;
                        true
                    } else {
                        false
                    }
                }
                None => false,
            }
        }
        // Resize(a, w, s) where a already has (w, s) → a
        IrExprKind::Resize { a } if a.width == e.width && a.signed == e.signed => {
            let replacement = (**a).clone();
            *e = replacement;
            true
        }
        // Convert(a, w, s) where a already has (w, s) → a (sv4_cast on an
        // identical shape is the identity)
        IrExprKind::Convert { a } if a.width == e.width && a.signed == e.signed => {
            let replacement = (**a).clone();
            *e = replacement;
            true
        }
        _ => false,
    }
}
