//! opt — optimization passes over the simulator IR.
//!
//! Conservative, correctness-preserving passes over an [`IrModel`], run
//! between lowering ([`crate::sim::codegen`]) and C11 emission
//! ([`crate::sim::emit_c`]).  Passes never recompute wake behavior: wait
//! sensitivity lists are fixed at lowering time and are not touched here.
//!
//! - `fold_constants`: bottom-up constant folding over [`core::elab::Value`]
//!   math (X/Z-correct; casez/casex wildcards for the wildcard comparisons).
//!   Division/modulo/power fold only for fully known operands of at most
//!   64 bits (the runtime's limits).  Calls are never folded through.  Real
//!   arithmetic folds only when both operands are real literals.  Unsized
//!   fill markers are ignored while folding — the runtime ops see concrete
//!   values only (fill affects assignment conversions, which stay untouched).
//! - `identities`: algebraic identities applied to a bounded fixpoint
//!   together with folding (folding exposes identities and vice versa).
//! - `prune_branches`: statements whose conditions are constants collapse to
//!   the taken branch, under the runtime truthiness contract (X/Z ⇒ false).
//!   A `wait (cond)` with a false constant stays: it is a zero-delay guard
//!   whose spin trips the runtime's deadlock guard.  Control-flow labels
//!   whose only jump was pruned away are stripped afterwards.
//! - `unused_storage`: signals that are neither read nor written anywhere get
//!   `omit = true`; the emitter skips their declarations.  Indices are never
//!   remapped.

use std::collections::HashSet;

use crate::core::elab::{self, Bit, Value};
use crate::sim::ir::{
    IrBinOp, IrCallArg, IrCaseKind, IrConst, IrElemSel, IrExpr, IrExprKind, IrLhs, IrModel,
    IrPreFn, IrRealBinOp, IrRealUnOp, IrStmt, IrSysFunc, IrUnOp, IrWaitSrc, LLG_MAX_WIDTH,
};

/// Run the enabled passes over `model` in a fixed order.
pub fn run(model: &mut IrModel, cfg: &OptConfig) {
    if cfg.fold_constants || cfg.identities {
        // Identities and constant folding interleave to a bounded fixpoint:
        // folding collapses operands so identity rules can fire (e.g. a
        // shift-by-zero inside a concat), and identity rewrites expose new
        // all-constant subtrees.
        let rounds = if cfg.identities { 2 } else { 1 };
        for _ in 0..rounds {
            if cfg.fold_constants {
                walk_model_exprs_mut(model, &mut fold_expr);
            }
            if cfg.identities {
                walk_model_exprs_mut(model, &mut ident_expr);
                walk_model_exprs_mut(model, &mut ident_expr);
            }
        }
    }
    if cfg.prune_branches {
        for p in &mut model.processes {
            prune_stmt_list(&mut p.body);
            for pre in &mut p.pre_fns {
                if let IrPreFn::Branch { body, .. } = pre {
                    prune_stmt_list(body);
                }
            }
            // Pruning can drop the branch carrying the only `goto` to a
            // control-flow label; strip the orphans so the emitted C stays
            // `-Wall` clean.
            strip_unreferenced_labels(&mut p.body);
            for pre in &mut p.pre_fns {
                if let IrPreFn::Branch { body, .. } = pre {
                    strip_unreferenced_labels(body);
                }
            }
        }
        for func in &mut model.funcs {
            prune_stmt_list(&mut func.body);
            for pre in &mut func.pre_fns {
                if let IrPreFn::Branch { body, .. } = pre {
                    prune_stmt_list(body);
                }
            }
            strip_unreferenced_labels(&mut func.body);
            for pre in &mut func.pre_fns {
                if let IrPreFn::Branch { body, .. } = pre {
                    strip_unreferenced_labels(body);
                }
            }
        }
    }
    if cfg.unused_storage {
        mark_unused_storage(model);
    }
}

/// Which optimization passes to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptConfig {
    /// Fold expressions whose operands are all constants.
    pub fold_constants: bool,
    /// Apply algebraic identities (double negation, shift-by-zero, …).
    pub identities: bool,
    /// Prune statements with constant conditions.
    pub prune_branches: bool,
    /// Omit signal storage that is neither read nor written anywhere.
    pub unused_storage: bool,
}

impl Default for OptConfig {
    fn default() -> Self {
        OptConfig {
            fold_constants: true,
            identities: true,
            prune_branches: true,
            unused_storage: true,
        }
    }
}

impl OptConfig {
    /// No passes at all.
    pub fn none() -> Self {
        OptConfig {
            fold_constants: false,
            identities: false,
            prune_branches: false,
            unused_storage: false,
        }
    }
}

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

fn value_to_const(v: &Value) -> IrConst {
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
fn as_packed_const(e: &IrExpr) -> Option<Value> {
    match &e.kind {
        IrExprKind::Const(c) => const_to_value(c),
        _ => None,
    }
}

/// The literal-real payload of `e`, when present.
fn real_of(e: &IrExpr) -> Option<f64> {
    match &e.kind {
        IrExprKind::Const(c) => c.real,
        _ => None,
    }
}

// ── Expression walking (every slot, recursively into statements) ─────────────

fn walk_lhs_mut(l: &mut IrLhs, f: &mut impl FnMut(&mut IrExpr)) {
    match l {
        IrLhs::Bit(_, idx) => walk_expr_mut(idx, f),
        IrLhs::IdxPart(_, base, width, _) => {
            walk_expr_mut(base, f);
            walk_expr_mut(width, f);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for i in indices {
                walk_expr_mut(i, f);
            }
            if let IrElemSel::Bit(idx) = elem_sel {
                walk_expr_mut(idx, f);
            }
        }
        _ => {}
    }
}

fn walk_call_args_mut(args: &mut [IrCallArg], f: &mut impl FnMut(&mut IrExpr)) {
    for arg in args {
        match arg {
            IrCallArg::Val(e) => walk_expr_mut(e, f),
            IrCallArg::OutTemp {
                init, writeback, ..
            } => {
                if let Some(init) = init {
                    walk_expr_mut(init, f);
                }
                walk_lhs_mut(writeback, f);
            }
            IrCallArg::OutAddr(_) => {}
        }
    }
}

fn walk_expr_mut(e: &mut IrExpr, f: &mut impl FnMut(&mut IrExpr)) {
    match &mut e.kind {
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            walk_expr_mut(a, f);
            walk_expr_mut(b, f);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a } => walk_expr_mut(a, f),
        IrExprKind::CastToReal { a, .. } => walk_expr_mut(a, f),
        IrExprKind::Mux { sel, a, b } => {
            walk_expr_mut(sel, f);
            walk_expr_mut(a, f);
            walk_expr_mut(b, f);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                walk_expr_mut(p, f);
            }
        }
        IrExprKind::BitSel { base, idx } => {
            walk_expr_mut(base, f);
            walk_expr_mut(idx, f);
        }
        IrExprKind::PartSel { base, .. } => walk_expr_mut(base, f),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            walk_expr_mut(base, f);
            walk_expr_mut(base_idx, f);
            walk_expr_mut(width_expr, f);
        }
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => {
            for i in indices {
                walk_expr_mut(i, f);
            }
            if let IrElemSel::Bit(idx) = elem_sel {
                walk_expr_mut(idx, f);
            }
        }
        IrExprKind::CallFn(call) => walk_call_args_mut(&mut call.args, f),
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::Clog2(a)
            | IrSysFunc::Bits(a)
            | IrSysFunc::BitQuery { arg: a, .. }
            | IrSysFunc::Rtoi(a)
            | IrSysFunc::Itor(a)
            | IrSysFunc::RealToBits(a)
            | IrSysFunc::BitsToReal(a)
            | IrSysFunc::ShortRealToBits(a)
            | IrSysFunc::BitsToShortReal(a) => walk_expr_mut(a, f),
            IrSysFunc::Time { .. } => {}
        },
        _ => {}
    }
    f(e);
}

/// Visit every expression slot of one statement (recursively).
fn walk_stmt_mut(s: &mut IrStmt, f: &mut impl FnMut(&mut IrExpr)) {
    match s {
        IrStmt::Block(b) | IrStmt::Forever { body: b } => walk_stmts_mut(b, f),
        IrStmt::Repeat { count, body } => {
            walk_expr_mut(count, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::DeclLocal {
            init: Some(init), ..
        } => walk_expr_mut(init, f),
        IrStmt::Assign { lhs, rhs, .. } => {
            walk_lhs_mut(lhs, f);
            walk_expr_mut(rhs, f);
        }
        IrStmt::If { cond, then_, els } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(then_, f);
            if let Some(els) = els {
                walk_stmts_mut(els, f);
            }
        }
        IrStmt::While { cond, body } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::WaitCond { cond, body, .. } => {
            walk_expr_mut(cond, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            walk_stmts_mut(init, f);
            walk_expr_mut(cond, f);
            walk_stmts_mut(incr, f);
            walk_stmts_mut(body, f);
        }
        IrStmt::Case { sel, items, .. } => {
            walk_expr_mut(sel, f);
            for item in items {
                for e in &mut item.exprs {
                    walk_expr_mut(e, f);
                }
                walk_stmts_mut(&mut item.body, f);
            }
        }
        IrStmt::Force { value, .. } => walk_expr_mut(value, f),
        IrStmt::Display { args, .. } => {
            for (e, _) in args {
                walk_expr_mut(e, f);
            }
        }
        IrStmt::WaveLimit(limit) => walk_expr_mut(limit, f),
        IrStmt::Call(call) => {
            walk_call_args_mut(&mut call.args, f);
            // Temp initializers and copy-out select indices are expression
            // slots of the call node.
            for (_, _, init) in &mut call.temps {
                if let Some(init) = init {
                    walk_expr_mut(init, f);
                }
            }
            for (lh, ..) in &mut call.copyouts {
                walk_lhs_mut(lh, f);
            }
        }
        IrStmt::Return { value: Some(value) } => walk_expr_mut(value, f),
        _ => {}
    }
}

fn walk_stmts_mut(stmts: &mut [IrStmt], f: &mut impl FnMut(&mut IrExpr)) {
    for s in stmts {
        walk_stmt_mut(s, f);
    }
}

fn walk_pre_fn_mut(pre: &mut IrPreFn, f: &mut impl FnMut(&mut IrExpr)) {
    match pre {
        IrPreFn::Branch { body, .. } => walk_stmts_mut(body, f),
        IrPreFn::MonEval { args, .. } => {
            for e in args {
                walk_expr_mut(e, f);
            }
        }
    }
}

fn walk_model_exprs_mut(model: &mut IrModel, f: &mut impl FnMut(&mut IrExpr)) {
    for func in &mut model.funcs {
        for pre in &mut func.pre_fns {
            walk_pre_fn_mut(pre, f);
        }
        walk_stmts_mut(&mut func.body, f);
    }
    for p in &mut model.processes {
        for pre in &mut p.pre_fns {
            walk_pre_fn_mut(pre, f);
        }
        walk_stmts_mut(&mut p.body, f);
    }
}

// ── Pass: fold_constants ──────────────────────────────────────────────────────

/// Bottom-up constant folding: children first (the walker visits them before
/// calling this), then this node when its operands are all constants.
fn fold_expr(e: &mut IrExpr) {
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
            if total > LLG_MAX_WIDTH as usize {
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

enum Folded {
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

// ── Pass: identities ──────────────────────────────────────────────────────────

fn ident_expr(e: &mut IrExpr) {
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
        | IrExprKind::Convert { a } => ident_expr(a),
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
            if let IrElemSel::Bit(idx) = elem_sel {
                ident_expr(idx);
            }
        }
        IrExprKind::CallFn(call) => {
            for arg in &mut call.args {
                match arg {
                    IrCallArg::Val(ex) => ident_expr(ex),
                    IrCallArg::OutTemp {
                        init, writeback, ..
                    } => {
                        if let Some(init) = init {
                            ident_expr(init);
                        }
                        ident_lhs(writeback);
                    }
                    IrCallArg::OutAddr(_) => {}
                }
            }
        }
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::Clog2(a)
            | IrSysFunc::Bits(a)
            | IrSysFunc::BitQuery { arg: a, .. }
            | IrSysFunc::Rtoi(a)
            | IrSysFunc::Itor(a)
            | IrSysFunc::RealToBits(a)
            | IrSysFunc::BitsToReal(a)
            | IrSysFunc::ShortRealToBits(a)
            | IrSysFunc::BitsToShortReal(a) => ident_expr(a),
            IrSysFunc::Time { .. } => {}
        },
        _ => {}
    }
}

fn ident_lhs(l: &mut IrLhs) {
    match l {
        IrLhs::Bit(_, idx) => ident_expr(idx),
        IrLhs::IdxPart(_, base, width, _) => {
            ident_expr(base);
            ident_expr(width);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for i in indices {
                ident_expr(i);
            }
            if let IrElemSel::Bit(idx) = elem_sel {
                ident_expr(idx);
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

// ── Pass: prune_branches ──────────────────────────────────────────────────────

/// The runtime truthiness of a constant condition (`X/Z ⇒ false`; real
/// constants follow the C `llg_real_to_bool` exactly: `v != 0.0`, so NaN
/// is truthy); `None` when the condition is not a constant.
fn truthy_const(e: &IrExpr) -> Option<bool> {
    match &e.kind {
        IrExprKind::Const(c) => match c.real {
            Some(r) => Some(r != 0.0),
            None => as_packed_const(e).map(|v| !v.is_unknown() && v.bits.contains(&Bit::One)),
        },
        _ => None,
    }
}

fn prune_stmt_list(stmts: &mut Vec<IrStmt>) {
    let old = std::mem::take(stmts);
    let mut out = Vec::with_capacity(old.len());
    for s in old {
        match s {
            IrStmt::If { cond, then_, els } => match truthy_const(&cond) {
                Some(true) => {
                    let mut taken = then_;
                    prune_stmt_list(&mut taken);
                    out.extend(taken);
                }
                Some(false) => {
                    if let Some(mut alt) = els {
                        prune_stmt_list(&mut alt);
                        out.extend(alt);
                    }
                }
                None => {
                    let mut then_ = then_;
                    prune_stmt_list(&mut then_);
                    let els = els.map(|mut e| {
                        prune_stmt_list(&mut e);
                        e
                    });
                    out.push(IrStmt::If { cond, then_, els });
                }
            },
            other => {
                let mut other = other;
                prune_nested_in_place(&mut other);
                out.push(other);
            }
        }
    }
    *stmts = out;
}

fn prune_nested_in_place(s: &mut IrStmt) {
    match s {
        IrStmt::Block(b) => prune_stmt_list(b),
        IrStmt::If { then_, els, .. } => {
            prune_stmt_list(then_);
            if let Some(els) = els {
                prune_stmt_list(els);
            }
        }
        IrStmt::While { cond, body } => match truthy_const(cond) {
            // A constant-false condition can never enter: an empty block.
            // (A constant-true condition keeps looping — its body carries
            // the waits.)
            Some(false) => {
                *s = IrStmt::Block(Vec::new());
            }
            _ => prune_stmt_list(body),
        },
        IrStmt::WaitCond { cond, body, .. } => match truthy_const(cond) {
            // wait (true) runs its body immediately; wait (false) STAYS —
            // zero-delay guard spin semantics.
            Some(true) => {
                let mut taken = std::mem::take(body);
                prune_stmt_list(&mut taken);
                *s = IrStmt::Block(taken);
            }
            _ => prune_stmt_list(body),
        },
        IrStmt::Repeat { body, .. } | IrStmt::Forever { body } => prune_stmt_list(body),
        IrStmt::For {
            init, incr, body, ..
        } => {
            prune_stmt_list(init);
            prune_stmt_list(incr);
            prune_stmt_list(body);
        }
        IrStmt::Case { sel, kind, items } => {
            if let Some(mut picked) = pick_case_branch(sel, *kind, items) {
                prune_stmt_list(&mut picked);
                *s = IrStmt::Block(picked);
                return;
            }
            for item in items {
                prune_stmt_list(&mut item.body);
            }
        }
        _ => {}
    }
}

/// Names of every label REFERENCED by a `goto` in the tree (`Label`
/// statements are definitions, not references).
fn collect_goto_names(stmts: &[IrStmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            IrStmt::Goto(l) => {
                out.insert(l.clone());
            }
            IrStmt::Block(b) | IrStmt::Forever { body: b } => collect_goto_names(b, out),
            IrStmt::If { then_, els, .. } => {
                collect_goto_names(then_, out);
                if let Some(els) = els {
                    collect_goto_names(els, out);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                collect_goto_names(b, out)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_goto_names(init, out);
                collect_goto_names(incr, out);
                collect_goto_names(body, out);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_goto_names(&item.body, out);
                }
            }
            IrStmt::WaitCond { body: b, .. } => collect_goto_names(b, out),
            _ => {}
        }
    }
}

/// Drop `Label` statements that no `goto` in the same tree targets anymore:
/// pruning can remove the branch carrying the only jump to a `_bk`/`_ct`/
/// `_xb`/`_id` label, leaving an unreferenced C label behind (`-Wall` warns
/// on those).  Safe because every goto targets a label inside the same
/// emitted function (= the same body tree).
fn strip_unreferenced_labels(stmts: &mut Vec<IrStmt>) {
    let mut referenced = HashSet::new();
    collect_goto_names(stmts, &mut referenced);
    strip_labels_in(stmts, &referenced);
}

fn strip_labels_in(stmts: &mut Vec<IrStmt>, referenced: &HashSet<String>) {
    stmts.retain(|s| !matches!(s, IrStmt::Label(l) if !referenced.contains(l)));
    for s in stmts.iter_mut() {
        match s {
            IrStmt::Block(b) | IrStmt::Forever { body: b } => strip_labels_in(b, referenced),
            IrStmt::If { then_, els, .. } => {
                strip_labels_in(then_, referenced);
                if let Some(els) = els {
                    strip_labels_in(els, referenced);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                strip_labels_in(b, referenced)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                strip_labels_in(init, referenced);
                strip_labels_in(incr, referenced);
                strip_labels_in(body, referenced);
            }
            IrStmt::Case { items, .. } => {
                for item in items.iter_mut() {
                    strip_labels_in(&mut item.body, referenced);
                }
            }
            IrStmt::WaitCond { body: b, .. } => strip_labels_in(b, referenced),
            _ => {}
        }
    }
}

/// Pick the reachable case arm for a constant selector.  Strict
/// first-match-wins over the item order:
///
/// - prune to item k's body iff items[0..k] are all constant and provably
///   unmatched AND item k constant-matches;
/// - prune to a default arm iff EVERY item is constant and provably unmatched
///   (a non-constant item can never be proven unreachable, so it blocks);
/// - otherwise leave the case untouched.
///
/// Wildcard kinds compare with the elab casez/casex helpers: both selector and
/// item are constants here, so their verdict is exact.
fn pick_case_branch(
    sel: &IrExpr,
    kind: IrCaseKind,
    items: &[crate::sim::ir::IrCaseItem],
) -> Option<Vec<IrStmt>> {
    let sel_v = as_packed_const(sel)?;

    /// Provability of one item against the constant selector.
    enum Verdict {
        /// Default arm (no item expressions).
        Default,
        /// Every expression constant and none matches.
        Unmatched,
        /// Some expression constant-matches.
        Matched,
        /// Some expression is not constant: nothing is provable.
        Unknown,
    }

    let verdict = |item: &crate::sim::ir::IrCaseItem| -> Verdict {
        if item.exprs.is_empty() {
            return Verdict::Default;
        }
        for ex in &item.exprs {
            let Some(ev) = as_packed_const(ex) else {
                return Verdict::Unknown;
            };
            let matched = match kind {
                IrCaseKind::Exact => elab::case_eq(&sel_v, &ev),
                IrCaseKind::Casex => elab::casex_eq(&sel_v, &ev),
                IrCaseKind::Casez => elab::casez_eq(&sel_v, &ev),
            };
            if matched.to_u64() == Some(1) {
                return Verdict::Matched;
            }
        }
        Verdict::Unmatched
    };

    // First-match-wins scan.  A default arm or an unknown (non-constant)
    // item cannot be proven unmatched, so everything after it stays
    // unprovable and must keep the case.
    let mut prefix_proven = true;
    for item in items {
        match verdict(item) {
            Verdict::Unmatched => {}
            Verdict::Matched if prefix_proven => return Some(item.body.clone()),
            _ => prefix_proven = false,
        }
    }
    // Default pruning only when every non-default item is const-unmatched
    // (any Matched/Unknown/Default ordering hazard keeps the case intact).
    let mut default_body: Option<Vec<IrStmt>> = None;
    let mut all_provably_dead = true;
    for item in items {
        match verdict(item) {
            Verdict::Unmatched => {}
            Verdict::Default => {
                default_body.get_or_insert_with(|| item.body.clone());
            }
            _ => all_provably_dead = false,
        }
    }
    match default_body {
        _ if !all_provably_dead => None,
        Some(body) => Some(body),
        // Every item was constant-unmatched and there is no default: nothing
        // runs.
        None => Some(Vec::new()),
    }
}

// ── Pass: unused_storage ──────────────────────────────────────────────────────

#[derive(Default)]
struct Rw {
    reads: HashSet<usize>,
    writes: HashSet<usize>,
}

impl Rw {
    fn read(&mut self, i: usize) {
        self.reads.insert(i);
    }
    fn write(&mut self, i: usize) {
        self.writes.insert(i);
    }
}

fn mark_unused_storage(model: &mut IrModel) {
    let mut rw = Rw::default();
    // Sensitivity lists and link sources are reads (plain globals only;
    // array-element addresses carry brackets and match nothing).
    let mut sens: Vec<String> = Vec::new();
    for p in &model.processes {
        if let crate::sim::ir::IrShape::SensLoop { reads } = &p.shape {
            sens.extend(reads.iter().cloned());
        }
        for x in &p.body {
            sens_lists_of(x, &mut sens);
        }
        for pre in &p.pre_fns {
            if let IrPreFn::Branch { body, .. } = pre {
                for x in body {
                    sens_lists_of(x, &mut sens);
                }
            }
        }
    }
    for func in &model.funcs {
        collect_pre_fns_rw(&func.pre_fns, model, &mut rw);
        collect_stmts_rw(&func.body, model, &mut rw);
    }
    for p in &model.processes {
        collect_pre_fns_rw(&p.pre_fns, model, &mut rw);
        collect_stmts_rw(&p.body, model, &mut rw);
    }
    for name in &sens {
        if let Some(i) = model.signals.iter().position(|sg| sg.c_name == *name) {
            rw.read(i);
        }
    }
    // Declaration initializers count as writes.
    for step in &model.init_steps {
        if let crate::sim::ir::IrInitStep::SetScalar { sig, .. } = step {
            rw.write(*sig);
        }
    }
    let waveform = model.waveform;
    let flags: Vec<bool> = model
        .signals
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            !(waveform && sig.hdl_name.is_some())
                && !rw.reads.contains(&i)
                && !rw.writes.contains(&i)
        })
        .collect();
    for (sig, omit) in model.signals.iter_mut().zip(flags) {
        sig.omit = omit;
    }
}

/// Reads/writes of a function's or process's attached helpers: monitor/strobe
/// evaluators read their arguments; fork-branch coroutines run whole
/// statement trees.
fn collect_pre_fns_rw(pre_fns: &[IrPreFn], model: &IrModel, rw: &mut Rw) {
    for pre in pre_fns {
        match pre {
            IrPreFn::MonEval { args, .. } => {
                for e in args {
                    collect_expr_reads(e, model, rw);
                }
            }
            IrPreFn::Branch { body, .. } => collect_stmts_rw(body, model, rw),
        }
    }
}

/// Sensitivity lists nested inside statements (`wait (cond)`/event-control
/// wait sources).  Every statement position is visited: blocks, if/else,
/// loops, case items + default, and wait bodies.  Named-event entries are
/// skipped here (they are not signal storage).
fn sens_lists_of(s: &IrStmt, out: &mut Vec<String>) {
    match s {
        IrStmt::WaitCond { sens, body, .. } => {
            out.extend(sens.iter().cloned());
            for x in body {
                sens_lists_of(x, out);
            }
        }
        IrStmt::WaitEvents { specs } => {
            for (src, _) in specs {
                if let IrWaitSrc::Sig(name) = src {
                    out.push(name.clone());
                }
            }
        }
        IrStmt::WaitAny { sens } => out.extend(sens.iter().cloned()),
        IrStmt::Block(b)
        | IrStmt::While { body: b, .. }
        | IrStmt::Repeat { body: b, .. }
        | IrStmt::Forever { body: b } => {
            for x in b {
                sens_lists_of(x, out);
            }
        }
        IrStmt::If { then_, els, .. } => {
            for x in then_ {
                sens_lists_of(x, out);
            }
            if let Some(els) = els {
                for x in els {
                    sens_lists_of(x, out);
                }
            }
        }
        IrStmt::For {
            init, incr, body, ..
        } => {
            for x in init.iter().chain(incr.iter()).chain(body.iter()) {
                sens_lists_of(x, out);
            }
        }
        IrStmt::Case { items, .. } => {
            for item in items {
                for x in &item.body {
                    sens_lists_of(x, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_stmts_rw(stmts: &[IrStmt], model: &IrModel, rw: &mut Rw) {
    for s in stmts {
        collect_stmt_rw(s, model, rw);
    }
}

fn collect_stmt_rw(s: &IrStmt, model: &IrModel, rw: &mut Rw) {
    match s {
        IrStmt::Block(b) => collect_stmts_rw(b, model, rw),
        IrStmt::DeclLocal {
            init: Some(init), ..
        } => collect_expr_reads(init, model, rw),
        IrStmt::Assign { lhs, rhs, .. } => {
            collect_lhs_rw(lhs, model, rw);
            collect_expr_reads(rhs, model, rw);
        }
        IrStmt::If { cond, then_, els } => {
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(then_, model, rw);
            if let Some(els) = els {
                collect_stmts_rw(els, model, rw);
            }
        }
        IrStmt::While { cond, body } | IrStmt::Repeat { count: cond, body } => {
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::Forever { body } => collect_stmts_rw(body, model, rw),
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            collect_stmts_rw(init, model, rw);
            collect_expr_reads(cond, model, rw);
            collect_stmts_rw(incr, model, rw);
            collect_stmts_rw(body, model, rw);
        }
        IrStmt::Case { sel, items, .. } => {
            collect_expr_reads(sel, model, rw);
            for item in items {
                for e in &item.exprs {
                    collect_expr_reads(e, model, rw);
                }
                collect_stmts_rw(&item.body, model, rw);
            }
        }
        IrStmt::WaitCond { cond, sens, body } => {
            collect_expr_reads(cond, model, rw);
            for name in sens {
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == *name) {
                    rw.read(i);
                }
            }
            collect_stmts_rw(body, model, rw);
        }
        // Event-control and combinational wait sources are reads of the
        // named signal globals (lowering emits their addresses into the wait
        // arrays; a signal referenced only here still needs its storage).
        // Named-event entries (`IrWaitSrc::Event`) are NOT storage — the
        // event globals are always emitted, so they are simply skipped.
        IrStmt::WaitEvents { specs } => {
            for (src, _) in specs {
                if let IrWaitSrc::Sig(name) = src {
                    if let Some(i) = model.signals.iter().position(|sg| sg.c_name == *name) {
                        rw.read(i);
                    }
                }
            }
        }
        IrStmt::WaitAny { sens } => {
            for name in sens {
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == *name) {
                    rw.read(i);
                }
            }
        }
        IrStmt::Force { sig, value } => {
            rw.write(*sig);
            collect_expr_reads(value, model, rw);
        }
        IrStmt::Release { sig } => rw.write(*sig),
        IrStmt::Display { args, .. } => {
            for (e, _) in args {
                collect_expr_reads(e, model, rw);
            }
        }
        IrStmt::WaveLimit(limit) => collect_expr_reads(limit, model, rw),
        IrStmt::Call(call) => collect_call_rw(call, model, rw),
        IrStmt::Return { value: Some(value) } => collect_expr_reads(value, model, rw),
        _ => {}
    }
}

fn collect_call_rw(call: &crate::sim::ir::IrCall, model: &IrModel, rw: &mut Rw) {
    for arg in &call.args {
        match arg {
            IrCallArg::Val(e) => collect_expr_reads(e, model, rw),
            IrCallArg::OutAddr(addr) => {
                // `&G_sig` / `G_sig`: a passed output/inout actual both reads
                // and writes its target (function-scope names never match).
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                    rw.write(i);
                }
            }
            IrCallArg::OutTemp {
                init, writeback, ..
            } => {
                if let Some(init) = init {
                    collect_expr_reads(init, model, rw);
                }
                collect_lhs_rw(writeback, model, rw);
            }
        }
    }
    // Caller-side output/inout temps live here, not in `args`: their
    // initializers read the actual's current value.
    for (_, _, init) in &call.temps {
        if let Some(init) = init {
            collect_expr_reads(init, model, rw);
        }
    }
    // Copy-out assignments after the call write the actual targets (their
    // select indices are reads).
    for (lh, ..) in &call.copyouts {
        collect_lhs_rw(lh, model, rw);
    }
}

fn collect_lhs_rw(l: &IrLhs, model: &IrModel, rw: &mut Rw) {
    match l {
        IrLhs::Whole(i) => rw.write(*i),
        IrLhs::WholeRef { .. } => {}
        IrLhs::Bit(i, idx) => {
            rw.write(*i);
            collect_expr_reads(idx, model, rw);
        }
        IrLhs::Part(i, ..) | IrLhs::IdxPart(i, ..) => rw.write(*i),
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for idx in indices {
                collect_expr_reads(idx, model, rw);
            }
            if let IrElemSel::Bit(idx) = elem_sel {
                collect_expr_reads(idx, model, rw);
            }
        }
    }
}

fn collect_expr_reads(e: &IrExpr, model: &IrModel, rw: &mut Rw) {
    match &e.kind {
        IrExprKind::SigRead(i) => rw.read(*i),
        IrExprKind::LocalRead(_) | IrExprKind::FormalRead(_) => {}
        IrExprKind::Verbatim { .. } => {}
        _ => {
            // Recurse structurally via a read-only walker.
            collect_children_reads(e, model, rw);
        }
    }
}

fn collect_children_reads(e: &IrExpr, model: &IrModel, rw: &mut Rw) {
    match &e.kind {
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            collect_expr_reads(a, model, rw);
            collect_expr_reads(b, model, rw);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::CastToReal { a, .. } => collect_expr_reads(a, model, rw),
        IrExprKind::Mux { sel, a, b } => {
            collect_expr_reads(sel, model, rw);
            collect_expr_reads(a, model, rw);
            collect_expr_reads(b, model, rw);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            for p in parts {
                collect_expr_reads(p, model, rw);
            }
        }
        IrExprKind::BitSel { base, idx } => {
            collect_expr_reads(base, model, rw);
            collect_expr_reads(idx, model, rw);
        }
        IrExprKind::PartSel { base, .. } => collect_expr_reads(base, model, rw),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            collect_expr_reads(base, model, rw);
            collect_expr_reads(base_idx, model, rw);
            collect_expr_reads(width_expr, model, rw);
        }
        IrExprKind::ArrayRead {
            arr,
            indices,
            elem_sel,
        } => {
            let _ = arr;
            for i in indices {
                collect_expr_reads(i, model, rw);
            }
            if let IrElemSel::Bit(idx) = elem_sel {
                collect_expr_reads(idx, model, rw);
            }
        }
        IrExprKind::CallFn(call) => collect_call_rw_readonly(&call.args, model, rw),
        IrExprKind::SysFunc(sf) => match sf {
            IrSysFunc::Clog2(a)
            | IrSysFunc::Bits(a)
            | IrSysFunc::BitQuery { arg: a, .. }
            | IrSysFunc::Rtoi(a)
            | IrSysFunc::Itor(a)
            | IrSysFunc::RealToBits(a)
            | IrSysFunc::BitsToReal(a)
            | IrSysFunc::ShortRealToBits(a)
            | IrSysFunc::BitsToShortReal(a) => collect_expr_reads(a, model, rw),
            IrSysFunc::Time { .. } => {}
        },
        _ => {}
    }
}

fn collect_call_rw_readonly(args: &[IrCallArg], model: &IrModel, rw: &mut Rw) {
    for arg in args {
        match arg {
            IrCallArg::Val(e) => collect_expr_reads(e, model, rw),
            IrCallArg::OutAddr(addr) => {
                let name = addr.trim_start_matches('&');
                if let Some(i) = model.signals.iter().position(|sg| sg.c_name == name) {
                    rw.read(i);
                }
            }
            IrCallArg::OutTemp {
                init, writeback, ..
            } => {
                if let Some(init) = init {
                    collect_expr_reads(init, model, rw);
                }
                // Writebacks inside expression calls copy temps back into the
                // actuals: count as writes.
                collect_lhs_write_only(writeback, model, rw);
            }
        }
    }
}

fn collect_lhs_write_only(l: &IrLhs, _model: &IrModel, rw: &mut Rw) {
    match l {
        IrLhs::Whole(i) => rw.write(*i),
        IrLhs::Bit(i, _) | IrLhs::Part(i, ..) | IrLhs::IdxPart(i, ..) => rw.write(*i),
        _ => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{
        IrCall, IrCallArg, IrCaseItem, IrDepth, IrEdge, IrProcess, IrShape, IrSignal, IrType,
    };

    // ── builders ──────────────────────────────────────────────────────────

    fn konst(v: u64, w: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![v],
                x: vec![0],
                z: vec![0],
                width: w,
                signed: false,
                real: None,
                fill: None,
            }),
            w,
            false,
            None,
        )
    }

    fn xkonst(w: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![0],
                x: vec![u64::MAX],
                z: vec![0],
                width: w,
                signed: false,
                real: None,
                fill: None,
            }),
            w,
            false,
            None,
        )
    }

    fn zkonst(w: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![0],
                x: vec![0],
                z: vec![u64::MAX],
                width: w,
                signed: false,
                real: None,
                fill: None,
            }),
            w,
            false,
            None,
        )
    }

    fn rkonst(v: f64) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![0],
                x: vec![0],
                z: vec![0],
                width: 0,
                signed: true,
                real: Some(v),
                fill: None,
            }),
            0,
            true,
            None,
        )
    }

    fn bin(op: IrBinOp, a: IrExpr, b: IrExpr, w: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(a),
                b: Box::new(b),
            },
            w,
            false,
            None,
        )
    }

    fn bitneg(a: IrExpr) -> IrExpr {
        let (w, sg) = (a.width, a.signed);
        IrExpr::new(
            IrExprKind::Un {
                op: IrUnOp::BitNeg,
                a: Box::new(a),
            },
            w,
            sg,
            None,
        )
    }

    fn assign(lhs: IrLhs, rhs: IrExpr) -> IrStmt {
        IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        }
    }

    fn sigs(n: usize) -> Vec<IrSignal> {
        (0..n)
            .map(|i| IrSignal {
                c_name: format!("G_s{i}"),
                hdl_name: Some(format!("t.s{i}")),
                ty: IrType::Packed {
                    width: 8,
                    signed: false,
                },
                net_driver: None,
                omit: false,
            })
            .collect()
    }

    fn model_with(body: Vec<IrStmt>, signals: Vec<IrSignal>) -> IrModel {
        IrModel {
            design_name: "t".to_string(),
            precision_ps: 1,
            waveform: false,
            signals,
            net_groups: Vec::new(),
            arrays: Vec::new(),
            events: Vec::new(),
            funcs: Vec::new(),
            processes: vec![IrProcess {
                c_name: "p_t_proc_0".to_string(),
                label: "t.always".to_string(),
                shape: IrShape::RunOnce,
                pre_fns: Vec::new(),
                body,
            }],
            init_steps: Vec::new(),
            spawns: Vec::new(),
            final_spawns: Vec::new(),
        }
    }

    fn fold_only() -> OptConfig {
        OptConfig {
            fold_constants: true,
            identities: false,
            prune_branches: false,
            unused_storage: false,
        }
    }

    fn idents_only() -> OptConfig {
        OptConfig {
            fold_constants: false,
            identities: true,
            prune_branches: false,
            unused_storage: false,
        }
    }

    fn prune_only() -> OptConfig {
        OptConfig {
            fold_constants: false,
            identities: false,
            prune_branches: true,
            unused_storage: false,
        }
    }

    fn storage_only() -> OptConfig {
        OptConfig {
            fold_constants: false,
            identities: false,
            prune_branches: false,
            unused_storage: true,
        }
    }

    fn proc_body(m: &IrModel) -> &[IrStmt] {
        &m.processes[0].body
    }

    fn const_payload(e: &IrExpr) -> Option<(u64, u32)> {
        match &e.kind {
            IrExprKind::Const(c) if c.real.is_none() => Some((c.bits[0], c.width)),
            _ => None,
        }
    }

    fn first_assign_rhs_of(s: &IrStmt) -> &IrExpr {
        match s {
            IrStmt::Assign { rhs, .. } => rhs,
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    /// The assigned RHS of the first statement, for payload assertions.
    fn first_assign_rhs(m: &IrModel) -> &IrExpr {
        match &m.processes[0].body[0] {
            IrStmt::Assign { rhs, .. } => rhs,
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    /// The sole statement of a pruned body, unwrapped from its splice Block.
    fn single_pruned_stmt(m: &IrModel) -> &IrStmt {
        assert_eq!(m.processes[0].body.len(), 1, "one statement expected");
        match &m.processes[0].body[0] {
            IrStmt::Block(b) => {
                assert_eq!(b.len(), 1, "single spliced statement expected");
                &b[0]
            }
            other => other,
        }
    }

    // ── fold_constants ────────────────────────────────────────────────────

    #[test]
    fn fold_add_constants() {
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Add, konst(5, 8), konst(6, 8), 8),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((11, 8)));
    }

    #[test]
    fn fold_x_propagation_yields_the_x_constant() {
        // elab arithmetic propagates X/Z exactly like the runtime ops, so
        // Add(5, X) folds to the 8-bit all-X constant.  (Only Div/Mod/Pow
        // stay symbolic for unknown operands.)
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Add, konst(5, 8), xkonst(8), 8),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        match &first_assign_rhs(&m).kind {
            IrExprKind::Const(c) => {
                assert_eq!(c.width, 8);
                assert!(c.x.iter().any(|&v| v != 0), "result carries X bits");
                assert_eq!((c.bits[0], c.z[0]), (0, 0));
            }
            other => panic!("expected folded X constant, got {other:?}"),
        }
    }

    #[test]
    fn fold_division_guard() {
        // Unknown operand: stays symbolic.
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Div, konst(6, 32), xkonst(32), 32),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        if let IrStmt::Assign { rhs, .. } = &proc_body(&m)[0] {
            assert!(matches!(rhs.kind, IrExprKind::Bin { .. }));
        }
        // Known 64-bit-or-less operands fold.
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Div, konst(6, 32), konst(3, 32), 32),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((2, 32)));
    }

    #[test]
    fn fold_case_equality_is_literal_over_x_z() {
        // === over an X operand yields 0 (unequal), not X.
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::CaseEq, xkonst(8), konst(255, 8), 1),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((0, 1)));

        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::CaseEq, konst(255, 8), konst(255, 8), 1),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((1, 1)));
    }

    #[test]
    fn fold_real_arithmetic_on_literals_only() {
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                IrExpr::new(
                    IrExprKind::RealBin {
                        op: IrRealBinOp::Add,
                        a: Box::new(rkonst(1.5)),
                        b: Box::new(rkonst(2.0)),
                    },
                    0,
                    true,
                    None,
                ),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        match &proc_body(&m)[0] {
            IrStmt::Assign { rhs, .. } => match &rhs.kind {
                IrExprKind::Const(c) => {
                    assert_eq!(c.real, Some(3.5));
                }
                other => panic!("expected folded real constant, got {other:?}"),
            },
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    // ── identities ────────────────────────────────────────────────────────

    #[test]
    fn identity_double_bitneg_collapses() {
        let inner = konst(5, 4);
        let e = bitneg(bitneg(inner));
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((5, 4)));
    }

    #[test]
    fn identity_double_bitneg_preserves_z_semantics() {
        let e = bitneg(bitneg(zkonst(1)));
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(
            first_assign_rhs(&m).kind,
            IrExprKind::Un {
                op: IrUnOp::BitNeg,
                ..
            }
        ));
    }

    #[test]
    fn identity_shift_by_zero_returns_base() {
        let e = bin(IrBinOp::Shl, konst(7, 8), konst(0, 8), 8);
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((7, 8)));
    }

    #[test]
    fn identity_single_part_concat() {
        let e = IrExpr::new(
            IrExprKind::Concat {
                parts: vec![konst(9, 8)],
            },
            8,
            false,
            None,
        );
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((9, 8)));

        // Two parts stay concatenated.
        let e = IrExpr::new(
            IrExprKind::Concat {
                parts: vec![konst(1, 4), konst(2, 4)],
            },
            8,
            false,
            None,
        );
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(
            &proc_body(&m)[0],
            IrStmt::Assign { rhs: IrExpr { kind: IrExprKind::Concat { parts }, .. }, .. } if parts.len() == 2
        ));
    }

    #[test]
    fn identity_mux_with_known_select_picks_branch() {
        let mk_mux = |sel: IrExpr| {
            IrExpr::new(
                IrExprKind::Mux {
                    sel: Box::new(sel),
                    a: Box::new(konst(9, 8)),
                    b: Box::new(konst(3, 8)),
                },
                8,
                false,
                None,
            )
        };
        // Select 1 → a.
        let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(konst(1, 1)))], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((9, 8)));
        // Select 0 → b.
        let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(konst(0, 1)))], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((3, 8)));
        // X select must NOT choose.
        let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(xkonst(1)))], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(
            &proc_body(&m)[0],
            IrStmt::Assign {
                rhs: IrExpr {
                    kind: IrExprKind::Mux { .. },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn identity_resize_same_shape_collapses() {
        let inner = konst(300, 16);
        let e = IrExpr::resize_to(inner, 16, false);
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((300, 16)));

        // Different target width keeps the resize.
        let e = IrExpr::resize_to(konst(300, 16), 8, false);
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(
            &proc_body(&m)[0],
            IrStmt::Assign {
                rhs: IrExpr {
                    kind: IrExprKind::Resize { .. },
                    ..
                },
                ..
            }
        ));
    }

    // ── prune_branches ────────────────────────────────────────────────────

    fn if_stmt(cond: IrExpr, then_: Vec<IrStmt>, els: Option<Vec<IrStmt>>) -> IrStmt {
        IrStmt::If { cond, then_, els }
    }

    fn marker(tag: u64) -> IrStmt {
        assign(IrLhs::Whole(0), konst(tag, 8))
    }

    #[test]
    fn prune_if_true_takes_then() {
        let mut m = model_with(
            vec![if_stmt(
                konst(1, 1),
                vec![marker(10)],
                Some(vec![marker(20)]),
            )],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(proc_body(&m).len(), 1);
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((10, 8)));
    }

    #[test]
    fn prune_if_false_takes_else() {
        let mut m = model_with(
            vec![if_stmt(
                konst(0, 1),
                vec![marker(10)],
                Some(vec![marker(20)]),
            )],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
    }

    #[test]
    fn prune_if_unknown_condition_takes_else_per_runtime_contract() {
        let mut m = model_with(
            vec![if_stmt(xkonst(1), vec![marker(10)], Some(vec![marker(20)]))],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
    }

    #[test]
    fn real_truthiness_matches_runtime_nan_is_truthy() {
        // The C runtime's llg_real_to_bool is `v != 0.0`: NaN is truthy.
        assert_eq!(truthy_const(&rkonst(f64::NAN)), Some(true));
        assert_eq!(truthy_const(&rkonst(0.0)), Some(false));
        assert_eq!(truthy_const(&rkonst(-1.5)), Some(true));
    }

    #[test]
    fn wide_constant_truthiness_checks_every_limb() {
        let high_bit = IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![0, 1u64 << 36],
                x: vec![0, 0],
                z: vec![0, 0],
                width: 128,
                signed: false,
                real: None,
                fill: None,
            }),
            128,
            false,
            None,
        );
        assert_eq!(truthy_const(&high_bit), Some(true));
    }

    #[test]
    fn prune_if_folded_nan_real_takes_then_zero_takes_else() {
        // fold + prune: 0.0/0.0 folds to a NaN real constant, and runtime
        // truthiness (`llg_real_to_bool`) treats NaN as truthy — pruning to
        // the ELSE arm here would diverge from an opt-off run.
        let cfg = OptConfig {
            fold_constants: true,
            identities: false,
            prune_branches: true,
            unused_storage: false,
        };
        let nan_div = IrExpr::new(
            IrExprKind::RealBin {
                op: IrRealBinOp::Div,
                a: Box::new(rkonst(0.0)),
                b: Box::new(rkonst(0.0)),
            },
            0,
            true,
            None,
        );
        let mut m = model_with(
            vec![if_stmt(nan_div, vec![marker(10)], Some(vec![marker(20)]))],
            sigs(1),
        );
        run(&mut m, &cfg);
        assert_eq!(
            const_payload(first_assign_rhs(&m)),
            Some((10, 8)),
            "a NaN condition must keep the THEN arm"
        );

        // A folded zero real takes the ELSE arm.
        let zero_mul = IrExpr::new(
            IrExprKind::RealBin {
                op: IrRealBinOp::Mul,
                a: Box::new(rkonst(5.0)),
                b: Box::new(rkonst(0.0)),
            },
            0,
            true,
            None,
        );
        let mut m = model_with(
            vec![if_stmt(zero_mul, vec![marker(10)], Some(vec![marker(20)]))],
            sigs(1),
        );
        run(&mut m, &cfg);
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
    }

    #[test]
    fn prune_while_false_becomes_empty_block() {
        let mut m = model_with(
            vec![IrStmt::While {
                cond: konst(0, 1),
                body: vec![marker(10)],
            }],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert!(matches!(proc_body(&m)[0], IrStmt::Block(ref b) if b.is_empty()));
    }

    #[test]
    fn prune_wait_cond_true_splices_body_false_stays() {
        let wait_true = IrStmt::WaitCond {
            cond: konst(1, 1),
            sens: vec!["G_s0".to_string()],
            body: vec![marker(42)],
        };
        let mut m = model_with(vec![wait_true], sigs(1));
        run(&mut m, &prune_only());
        assert_eq!(
            const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
            Some((42, 8))
        );

        // A false constant condition STAYS: zero-delay guard spin semantics.
        let wait_false = IrStmt::WaitCond {
            cond: konst(0, 1),
            sens: vec![],
            body: vec![marker(42)],
        };
        let mut m = model_with(vec![wait_false], sigs(1));
        run(&mut m, &prune_only());
        assert!(matches!(proc_body(&m)[0], IrStmt::WaitCond { .. }));
    }

    #[test]
    fn prune_case_picks_matching_arm_and_respects_order() {
        let case = |items: Vec<IrCaseItem>| IrStmt::Case {
            sel: konst(2, 4),
            kind: IrCaseKind::Exact,
            items,
        };
        let item = |vals: &[u64], tag: u64| IrCaseItem {
            exprs: vals.iter().map(|v| konst(*v, 4)).collect(),
            body: vec![marker(tag)],
        };
        // Matching item wins over later default.
        let mut m = model_with(
            vec![case(vec![item(&[8], 10), item(&[2], 20), item(&[], 30)])],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(
            const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
            Some((20, 8))
        );

        // An earlier non-constant item could match: leave the case alone.
        let blocking = IrCaseItem {
            exprs: vec![IrExpr::new(IrExprKind::SigRead(0), 4, false, None)],
            body: vec![marker(99)],
        };
        let mut m = model_with(vec![case(vec![blocking, item(&[2], 20)])], sigs(1));
        run(&mut m, &prune_only());
        assert!(matches!(proc_body(&m)[0], IrStmt::Case { .. }));

        // All items unmatched, default present → default body.
        let mut m = model_with(vec![case(vec![item(&[8], 10), item(&[], 30)])], sigs(1));
        run(&mut m, &prune_only());
        assert_eq!(
            const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
            Some((30, 8))
        );
    }

    // ── orphaned-label strip (post-prune) ─────────────────────────────────

    #[test]
    fn prune_strips_label_whose_only_goto_was_pruned() {
        // `if (0) break;` prunes the then-arm carrying the only goto to
        // `_bk1`; the trailing label must not survive as dead C.
        let mut m = model_with(
            vec![
                if_stmt(konst(0, 1), vec![IrStmt::Goto("_bk1".to_string())], None),
                marker(10),
                IrStmt::Label("_bk1".to_string()),
            ],
            sigs(1),
        );
        run(&mut m, &prune_only());
        let body = proc_body(&m);
        assert_eq!(body.len(), 1, "label stripped with its pruned goto");
        assert_eq!(const_payload(first_assign_rhs_of(&body[0])), Some((10, 8)));
    }

    #[test]
    fn prune_keeps_labels_that_are_still_goto_targets() {
        // A live goto (unknown condition) keeps its target; a DIFFERENT
        // unreferenced label in the same tree still goes away.  Nested
        // definitions are covered too.
        let mut m = model_with(
            vec![
                if_stmt(
                    IrExpr::new(IrExprKind::SigRead(0), 1, false, None),
                    vec![IrStmt::Goto("_ct1".to_string())],
                    None,
                ),
                IrStmt::Block(vec![
                    IrStmt::Label("_ct1".to_string()),
                    IrStmt::Label("_xb9".to_string()),
                    marker(11),
                ]),
                IrStmt::Label("_bk2".to_string()),
                IrStmt::Goto("_bk2".to_string()),
            ],
            sigs(1),
        );
        run(&mut m, &prune_only());
        let labels: Vec<&str> = collect_all_labels(proc_body(&m));
        assert!(labels.contains(&"_ct1"), "live target kept");
        assert!(labels.contains(&"_bk2"), "goto-referenced label kept");
        assert!(
            !labels.contains(&"_xb9"),
            "unreferenced nested label stripped"
        );
    }

    fn collect_all_labels(stmts: &[IrStmt]) -> Vec<&str> {
        let mut out = Vec::new();
        fn walk<'a>(stmts: &'a [IrStmt], out: &mut Vec<&'a str>) {
            for s in stmts {
                match s {
                    IrStmt::Label(l) => out.push(l.as_str()),
                    IrStmt::Goto(l) => out.push(l.as_str()),
                    IrStmt::Block(b) | IrStmt::Forever { body: b } => walk(b, out),
                    IrStmt::If { then_, els, .. } => {
                        walk(then_, out);
                        if let Some(els) = els {
                            walk(els, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(stmts, &mut out);
        out
    }

    // ── unused_storage ────────────────────────────────────────────────────

    #[test]
    fn unused_storage_omits_only_fully_dead_signals() {
        let body = vec![
            // Writes s1, reads s2.
            assign(IrLhs::Whole(1), konst(1, 8)),
            assign(
                IrLhs::Whole(2),
                IrExpr::new(IrExprKind::SigRead(1), 8, false, None),
            ),
        ];
        let mut signals = sigs(4);
        signals[3].c_name = "G_s3".to_string();
        let mut m = model_with(body, signals);
        // s3 is read by a combinational sensitivity list.
        m.processes[0].shape = IrShape::SensLoop {
            reads: vec!["G_s3".to_string()],
        };
        run(&mut m, &storage_only());
        assert!(m.signals[0].omit, "untouched signal is omitted");
        assert!(!m.signals[1].omit, "written signal stays");
        assert!(!m.signals[2].omit, "read signal stays");
        assert!(!m.signals[3].omit, "sensitivity-read signal stays");
    }

    #[test]
    fn waveform_keeps_user_storage_but_not_synthesized_storage() {
        let mut signals = sigs(2);
        signals[1].hdl_name = None;
        let mut m = model_with(Vec::new(), signals);
        m.waveform = true;

        run(&mut m, &storage_only());

        assert!(!m.signals[0].omit, "waveform-visible user signal stays");
        assert!(
            m.signals[1].omit,
            "unreferenced synthesized signal is omitted"
        );
    }

    #[test]
    fn unused_storage_keeps_wait_event_sources() {
        // s1 appears ONLY in an @(posedge …) or-list nested in a block; s2
        // only in a wait_any sensitivity set under an if arm.  Both keep
        // their declarations: emission references their globals in the wait
        // arrays (`&G_s1`), so omitting them breaks the generated C.
        let body = vec![
            assign(IrLhs::Whole(0), konst(1, 8)),
            IrStmt::Block(vec![IrStmt::WaitEvents {
                specs: vec![(IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Posedge)],
            }]),
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(0), 8, false, None),
                then_: vec![IrStmt::WaitAny {
                    sens: vec!["G_s2".to_string()],
                }],
                els: None,
            },
        ];
        let mut m = model_with(body, sigs(3));
        run(&mut m, &storage_only());
        assert!(!m.signals[0].omit, "written signal stays");
        assert!(!m.signals[1].omit, "event-source-only signal stays");
        assert!(!m.signals[2].omit, "wait-any-source-only signal stays");
    }

    #[test]
    fn unused_storage_handles_mixed_event_wait_sources() {
        // A mixed @(a or ev) wait keeps the signal half's storage (its
        // address is emitted into the wait table); the named-event half is
        // not storage — it must be skipped without matching any signal name.
        let body = vec![IrStmt::WaitEvents {
            specs: vec![
                (IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Any),
                (IrWaitSrc::Event(0), IrEdge::Any),
            ],
        }];
        let mut m = model_with(body, sigs(3));
        m.events.push(crate::sim::ir::IrEvent {
            c_name: "E_tb_ev".to_string(),
        });
        run(&mut m, &storage_only());
        assert!(m.signals[0].omit, "untouched signal is still omitted");
        assert!(
            !m.signals[1].omit,
            "mixed-list signal source keeps its storage"
        );
        assert!(m.signals[2].omit, "untouched signal is still omitted");
    }

    #[test]
    fn unused_storage_keeps_wait_cond_sens_only_signal() {
        // s3 is named only in the `wait (cond)` sensitivity list — the
        // condition itself is a constant and never reads it.  Nested inside
        // a while body to cover non-toplevel statement positions.
        let body = vec![IrStmt::While {
            cond: konst(1, 1),
            body: vec![IrStmt::WaitCond {
                cond: konst(0, 1),
                sens: vec!["G_s3".to_string()],
                body: vec![marker(42)],
            }],
        }];
        let mut m = model_with(body, sigs(4));
        run(&mut m, &storage_only());
        assert!(!m.signals[3].omit, "wait-cond sens-only signal stays");
        assert!(m.signals[1].omit && m.signals[2].omit, "dead ones go");
    }

    #[test]
    fn unused_storage_keeps_wait_sources_in_fork_branches() {
        // Fork-branch coroutine bodies live on the process's pre_fns; their
        // wait sources are reads too.
        let mut m = model_with(vec![assign(IrLhs::Whole(0), konst(1, 8))], sigs(4));
        m.processes[0].pre_fns = vec![IrPreFn::Branch {
            c_name: "p_t_b0".to_string(),
            body: vec![
                IrStmt::WaitEvents {
                    specs: vec![(IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Any)],
                },
                IrStmt::WaitAny {
                    sens: vec!["G_s2".to_string(), "G_s3".to_string()],
                },
            ],
        }];
        run(&mut m, &storage_only());
        assert!(!m.signals[0].omit);
        assert!(!m.signals[1].omit, "fork-branch event source stays");
        assert!(!m.signals[2].omit, "fork-branch wait-any source stays");
        assert!(!m.signals[3].omit, "fork-branch wait-any source stays");
    }

    #[test]
    fn unused_storage_counts_call_temp_inits_and_copyouts() {
        // Statement-position task calls keep temp inits and copy-out targets
        // INSIDE the Call node: the temp init reads s2, the copy-out writes
        // s3 (its bit index is also a read slot), and the Val/OutAddr args
        // read+write s4/s1.
        let call = IrStmt::Call(IrCall {
            f: 0,
            args: vec![
                IrCallArg::Val(IrExpr::new(IrExprKind::SigRead(4), 8, false, None)),
                IrCallArg::OutAddr("&G_s1".to_string()),
            ],
            depth: IrDepth::PROC,
            temps: vec![(
                "_a0".to_string(),
                0,
                Some(IrExpr::new(IrExprKind::SigRead(2), 8, false, None)),
            )],
            copyouts: vec![(IrLhs::Bit(3, konst(0, 3)), "_a0".to_string(), 8, false)],
        });
        let mut m = model_with(vec![call], sigs(5));
        run(&mut m, &storage_only());
        assert!(!m.signals[1].omit, "OutAddr actual stays (read + write)");
        assert!(!m.signals[2].omit, "temp-init read stays");
        assert!(!m.signals[3].omit, "copy-out target stays");
        assert!(!m.signals[4].omit, "Val argument read stays");
        assert!(m.signals[0].omit, "untouched signal is omitted");
    }

    // ── shortreal cast folding ────────────────────────────────────────────

    #[test]
    fn fold_shortreal_cast_rounds_through_f32_like_the_runtime() {
        // 16777217 = 2^24 + 1 needs 25 mantissa bits: the runtime cast
        // renders `(double)(float)(x)` and C float rounding drops it to
        // 2^24, so the fold must produce exactly that value (a full-precision
        // fold would make downstream real math diverge from opt-off runs).
        let cast = |shortreal: bool| {
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(konst(16_777_217, 32)),
                    shortreal,
                },
                0,
                true,
                None,
            )
        };
        let mut m = model_with(vec![assign(IrLhs::Whole(0), cast(true))], sigs(1));
        run(&mut m, &fold_only());
        match &first_assign_rhs(&m).kind {
            IrExprKind::Const(c) => {
                assert_eq!(c.real, Some(16_777_216.0f64));
            }
            other => panic!("expected folded real constant, got {other:?}"),
        }

        // A plain real cast keeps full double precision.
        let mut m = model_with(vec![assign(IrLhs::Whole(0), cast(false))], sigs(1));
        run(&mut m, &fold_only());
        match &first_assign_rhs(&m).kind {
            IrExprKind::Const(c) => {
                assert_eq!(c.real, Some(16_777_217.0f64));
            }
            other => panic!("expected folded real constant, got {other:?}"),
        }
    }

    // ── case pruning provability ──────────────────────────────────────────

    fn case_stmt(sel: IrExpr, kind: IrCaseKind, items: Vec<IrCaseItem>) -> IrStmt {
        IrStmt::Case { sel, kind, items }
    }

    fn citem(vals: &[u64], tag: u64) -> IrCaseItem {
        IrCaseItem {
            exprs: vals.iter().map(|v| konst(*v, 4)).collect(),
            body: vec![marker(tag)],
        }
    }

    /// A constant item with Z wildcard bits (`4'b1z0z` for bits=8, z=3).
    fn czitem(bits: u64, z: u64, tag: u64) -> IrCaseItem {
        IrCaseItem {
            exprs: vec![IrExpr::new(
                IrExprKind::Const(IrConst {
                    bits: vec![bits],
                    x: vec![0],
                    z: vec![z],
                    width: 4,
                    signed: false,
                    real: None,
                    fill: None,
                }),
                4,
                false,
                None,
            )],
            body: vec![marker(tag)],
        }
    }

    fn def_item(tag: u64) -> IrCaseItem {
        IrCaseItem {
            exprs: vec![],
            body: vec![marker(tag)],
        }
    }

    #[test]
    fn prune_case_nonconst_item_blocks_default_pruning() {
        // [const-unmatched, DEFAULT, non-const]: the non-const item could
        // still match at runtime, so nothing about the default is provable.
        let items = vec![
            citem(&[8], 10),
            def_item(30),
            IrCaseItem {
                exprs: vec![IrExpr::new(IrExprKind::SigRead(0), 4, false, None)],
                body: vec![marker(99)],
            },
        ];
        let mut m = model_with(
            vec![case_stmt(konst(2, 4), IrCaseKind::Exact, items)],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert!(
            matches!(proc_body(&m)[0], IrStmt::Case { .. }),
            "a remaining non-const item must block default pruning"
        );
    }

    #[test]
    fn prune_case_default_arm_blocks_later_items() {
        // A default arm can never be proven unmatched, so a matching item
        // after it cannot be proven reachable either.
        let mut m = model_with(
            vec![case_stmt(
                konst(2, 4),
                IrCaseKind::Exact,
                vec![def_item(30), citem(&[2], 20)],
            )],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert!(matches!(proc_body(&m)[0], IrStmt::Case { .. }));
    }

    #[test]
    fn prune_casez_wildcard_item_vs_known_selector() {
        let sel = konst(0b1000, 4);
        // LRM 12.5.1: z item bits are wildcards against a known selector —
        // 4'b1000 ==='z' 4'b1z0z matches → prunes to that item.
        let mut m = model_with(
            vec![case_stmt(
                sel.clone(),
                IrCaseKind::Casez,
                vec![
                    citem(&[0b0101], 10),
                    czitem(0b1000, 0b0011, 20),
                    def_item(30),
                ],
            )],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(
            const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
            Some((20, 8))
        );

        // A known-bit mismatch dominates: 4'b1110 does not match 4'b1000
        // under casez → all items unmatched → default.
        let mut m = model_with(
            vec![case_stmt(
                sel,
                IrCaseKind::Casez,
                vec![czitem(0b1110, 0, 40), def_item(30)],
            )],
            sigs(1),
        );
        run(&mut m, &prune_only());
        assert_eq!(
            const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
            Some((30, 8))
        );
    }

    // ── identity guards / interleave ──────────────────────────────────────

    #[test]
    fn identity_mux_narrower_branch_keeps_the_mux() {
        // The selected branch is narrower than the mux node; Verilog widens
        // ?: to max(branch widths), so a narrower branch may NOT stand in.
        let e = IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(konst(0, 1)),
                a: Box::new(konst(9, 8)),
                b: Box::new(konst(3, 4)),
            },
            8,
            false,
            None,
        );
        let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(
            &proc_body(&m)[0],
            IrStmt::Assign {
                rhs: IrExpr {
                    kind: IrExprKind::Mux { .. },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn fold_pow_mod_guards_stay_symbolic() {
        // Unknown operand: stays symbolic.
        for op in [IrBinOp::Pow, IrBinOp::Mod] {
            let mut m = model_with(
                vec![assign(
                    IrLhs::Whole(0),
                    bin(op, konst(6, 32), xkonst(32), 32),
                )],
                sigs(1),
            );
            run(&mut m, &fold_only());
            assert!(
                matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }),
                "{op:?} with an unknown operand must stay symbolic"
            );
            // Fully known but >64-bit operands stay runtime calls too.
            let wide = konst(6, 128);
            let mut m = model_with(
                vec![assign(IrLhs::Whole(0), bin(op, wide, konst(3, 128), 128))],
                sigs(1),
            );
            run(&mut m, &fold_only());
            assert!(
                matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }),
                "{op:?} over 128-bit operands must stay symbolic"
            );
        }
        // Known narrow operands fold.
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Pow, konst(3, 32), konst(3, 32), 32),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((27, 32)));
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(IrBinOp::Mod, konst(7, 32), konst(3, 32), 32),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert_eq!(const_payload(first_assign_rhs(&m)), Some((1, 32)));
    }

    #[test]
    fn fold_and_identities_interleave_to_fixpoint() {
        // Folding produces the constant zero that the shift-by-zero identity
        // needs; identities alone see an unfoldable Sub and do nothing.
        let shift = || {
            IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Shl,
                    a: Box::new(IrExpr::new(IrExprKind::SigRead(0), 8, false, None)),
                    b: Box::new(bin(IrBinOp::Sub, konst(4, 8), konst(4, 8), 8)),
                },
                8,
                false,
                None,
            )
        };
        let mut m = model_with(vec![assign(IrLhs::Whole(0), shift())], sigs(1));
        run(&mut m, &idents_only());
        assert!(matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }));

        // With both passes on, fold exposes the zero, the identity replaces
        // the shift, and the result survives the second round untouched.
        let both = OptConfig {
            fold_constants: true,
            identities: true,
            prune_branches: false,
            unused_storage: false,
        };
        let mut m = model_with(vec![assign(IrLhs::Whole(0), shift())], sigs(1));
        run(&mut m, &both);
        assert!(matches!(first_assign_rhs(&m).kind, IrExprKind::SigRead(0)));
    }
}
