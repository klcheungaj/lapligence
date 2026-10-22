//! Shared analysis helpers for lint rules.
//!
//! Read/write collection, expression-width computation, scope/instance
//! iteration and port-link bookkeeping.  Everything is owned-db based (no native AST
//! access) and deterministic: results are deduped and returned in
//! first-encounter order.
//!
//! Conventions shared by the rules:
//! - "signal" means a `Net`/`Var`/`Array` arena node (parameters are handled
//!   separately by [`object_width`]).
//! - `collect_reads`/`collect_writes` walk a statement/expression tree and
//!   exclude an assignment's LHS base from reads (the base is the write
//!   target), while select indices/bounds on the LHS still count as reads.
//! - `force`/`release`/`deassign` LHS objects count as writes (the signal is
//!   used), but do not contribute reads.
//! - `collect_driver_writes` is the active-driver variant used by rules that
//!   must distinguish `force` from `release`/`deassign`.

use std::collections::HashSet;

use crate::core::db::Operation;
use crate::core::db::{
    AlwaysKind, Db, Direction, ExprKind, IntraControl, NodeId, NodeKind, ProductionKind, StmtKind,
};
use crate::core::value::ValueData;

/// True when `id` is a net, variable or array node.
pub fn is_signal(db: &Db, id: NodeId) -> bool {
    !db.is_conditional_pattern_binding(id)
        && matches!(
            db.node_kind(id),
            NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
        )
}

/// The signal a ref-like expression ultimately refers to: a plain `Ref`
/// target, or the base of a bit/part/indexed-part select.  `None` when the
/// expression is not (or cannot be resolved to) a net/var/array.
pub fn signal_of_ref(db: &Db, id: NodeId) -> Option<NodeId> {
    if db.is_conditional_pattern_binding(id) {
        return None;
    }
    match db.node_kind(id) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => {
            // A clockvar stands for its clocking signal; one bound to a
            // select resolves to the selected signal (SV §14.5).
            Some(
                db.clocking_var(id)
                    .and_then(|var| var.source.or_else(|| signal_of_ref(db, var.expression)))
                    .unwrap_or(id),
            )
        }
        NodeKind::Expr(ExprKind::Ref { target }) => {
            target.and_then(|target| signal_of_ref(db, target))
        }
        NodeKind::Expr(ExprKind::BitSelect { base, .. })
        | NodeKind::Expr(ExprKind::PartSelect { base, .. })
        | NodeKind::Expr(ExprKind::IndexedPartSelect { base, .. }) => signal_of_ref(db, *base),
        _ => None,
    }
}

/// Width of a declared object (net, var, array or parameter); `None` when the
/// type is unsized or unknown.
pub fn object_width(db: &Db, id: NodeId) -> Option<u32> {
    match db.node_kind(id) {
        NodeKind::Net { ty, .. } | NodeKind::Var { ty } | NodeKind::Array { ty } => ty.width,
        NodeKind::Param { ty, .. } => ty.width,
        _ => None,
    }
}

/// Computed width of an expression, per LRM self-determined width rules.
///
/// - `Constant` → captured width (unsized literals like `'1` report size -1 → None).
/// - `Ref` → width of the referenced object (signal or parameter).
/// - `BitSelect` → 1; `PartSelect` → `|left - right| + 1`; `IndexedPartSelect`
///   → the width expression.
/// - `Cast` → the cast target type width.
/// - `Operation` → per-op rule (see [`op_width`]).
/// - Anything else (calls, hier paths, unknown) → `None`.
pub fn expr_width(db: &Db, id: NodeId) -> Option<u32> {
    if db.is_implicit_conversion(id) {
        if let NodeKind::Expr(ExprKind::Cast { operand, .. }) = db.node_kind(id) {
            return expr_width(db, *operand);
        }
    }
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { size, value, .. }) => {
            if *size <= 0 {
                // Unsupported size or unsized literal (`'1`, `'x`): unknown.
                return None;
            }
            // Preserve explicitly sized literal width when contextual typing
            // has changed the containing expression's width.
            let size = *size as u32;
            Some(constant_value_bits(value).map_or(size, |b| size.max(b)))
        }
        NodeKind::Expr(ExprKind::Ref { target }) => target.and_then(|t| object_width(db, t)),
        NodeKind::Expr(ExprKind::BitSelect { .. }) => Some(1),
        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
            match (const_i128(db, *left), const_i128(db, *right)) {
                (Some(l), Some(r)) => u32::try_from(l.abs_diff(r).checked_add(1)?).ok(),
                _ => None,
            }
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect { width_expr, .. }) => {
            u32::try_from(const_i128(db, *width_expr)?).ok()
        }
        NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty.width,
        NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => op_width(db, *op, operands),
        NodeKind::Expr(ExprKind::Conditional {
            if_true, if_false, ..
        }) => expr_width(db, *if_true)
            .zip(expr_width(db, *if_false))
            .map(|(a, b)| a.max(b)),
        _ => None,
    }
}

/// Width of an operation result from its operands.
fn op_width(db: &Db, op: Operation, operands: &[NodeId]) -> Option<u32> {
    // Self-determined arithmetic / bitwise ops: max operand width.
    if matches!(
        op,
        Operation::Add
            | Operation::Subtract
            | Operation::Multiply
            | Operation::Divide
            | Operation::Modulo
            | Operation::UnaryMinus
            | Operation::UnaryPlus
            | Operation::BitwiseNot
            | Operation::BitwiseAnd
            | Operation::BitwiseOr
            | Operation::BitwiseXor
            | Operation::BitwiseXnor
            | Operation::Power
    ) {
        return operands.iter().filter_map(|o| expr_width(db, *o)).max();
    }
    // Shifts: width of the left operand (the shift amount is self-determined).
    if matches!(
        op,
        Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight
    ) {
        return operands.first().and_then(|o| expr_width(db, *o));
    }
    // Comparisons, logical and reduction ops produce exactly one bit.
    if matches!(
        op,
        Operation::Equal
            | Operation::NotEqual
            | Operation::CaseEqual
            | Operation::CaseNotEqual
            | Operation::WildEqual
            | Operation::WildNotEqual
            | Operation::Greater
            | Operation::GreaterEqual
            | Operation::Less
            | Operation::LessEqual
            | Operation::LogicalAnd
            | Operation::LogicalOr
            | Operation::LogicalNot
            | Operation::ReductionAnd
            | Operation::ReductionNand
            | Operation::ReductionOr
            | Operation::ReductionNor
            | Operation::ReductionXor
            | Operation::ReductionXnor
    ) {
        return Some(1);
    }
    if op == Operation::Concat {
        // Sum of operand widths; unknown operand → unknown total.
        return operands.iter().try_fold(0_u32, |width, operand| {
            width.checked_add(expr_width(db, *operand)?)
        });
    }
    if op == Operation::MultiConcat {
        // Replication: first operand is the repeat count, the rest are parts.
        return match (
            operands
                .first()
                .and_then(|o| const_i128(db, *o))
                .and_then(|count| u32::try_from(count).ok()),
            operands.iter().skip(1).try_fold(0_u32, |width, operand| {
                width.checked_add(expr_width(db, *operand)?)
            }),
        ) {
            (Some(count), Some(parts)) => count.checked_mul(parts),
            _ => None,
        };
    }
    if op == Operation::Conditional {
        // Mux: width of the data branches (skip the selector).
        return operands
            .iter()
            .skip(1)
            .filter_map(|o| expr_width(db, *o))
            .max();
    }
    None
}

/// Read a constant expression node as an integer; `None` unless it is a plain
/// int/uint/scalar constant.
pub fn const_i128(db: &Db, id: NodeId) -> Option<i128> {
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { value, .. }) => value.to_i128(),
        _ => None,
    }
}

/// Bit length of a literal's value as written (underscores excluded);
/// `None` for formats without a fixed width (e.g. strings).
fn constant_value_bits(value: &ValueData) -> Option<u32> {
    let digits = |s: &str| s.chars().filter(|c| *c != '_').count() as u32;
    match value {
        ValueData::Bin(s) => Some(digits(s).max(1)),
        ValueData::Oct(s) => Some(digits(s).saturating_mul(3).max(1)),
        ValueData::Hex(s) => Some(digits(s).saturating_mul(4).max(1)),
        ValueData::Dec(s) => {
            let cleaned: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            cleaned.parse::<u128>().ok().map(bits_needed)
        }
        ValueData::Int(v) => Some(bits_needed(v.unsigned_abs() as u128)),
        ValueData::UInt(v) => Some(bits_needed(*v as u128)),
        ValueData::Scalar(_) => Some(1),
        ValueData::Vector { bit_width, .. } => u32::try_from(*bit_width).ok(),
        _ => None,
    }
}

/// Number of bits needed to represent `v` (at least one).
fn bits_needed(v: u128) -> u32 {
    if v == 0 {
        1
    } else {
        128 - v.leading_zeros()
    }
}

/// Every signal read in the statement/expression tree rooted at `root`,
/// deduped, in first-encounter order.
///
/// An assignment's LHS base signal is excluded (it is the write target); its
/// select indices/bounds are included.  `force` reads its RHS only;
/// `release`/`deassign` read nothing.
pub fn collect_reads(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_reads(db, root, &mut seen, &mut out);
    out
}

/// Every signal written in the tree rooted at `root` (assignment / cont-assign
/// / procedural continuous assign / force / release / deassign LHS bases),
/// deduped, in first-encounter order.
pub fn collect_writes(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_writes(db, root, false, &mut seen, &mut out);
    out
}

/// [`collect_writes`] that also resolves each member of a concatenation,
/// streaming or assignment-pattern target (`assign {a, b} = ...`).  Kept apart
/// so that rules built on [`collect_writes`] keep their established behavior.
pub fn collect_writes_with_members(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_writes(db, root, true, &mut seen, &mut out);
    out
}

/// Every signal actively driven in the tree rooted at `root`.
///
/// This has the same whole-object behavior as [`collect_writes`], including
/// selected LHS expressions, but deliberately excludes `release` and
/// `deassign`: those statements cancel a driver rather than establish one.
/// Output, inout and non-`const` `ref` actuals of calls are drivers too (see
/// `call_written_actuals`).
pub fn collect_driver_writes(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_driver_writes(db, root, &mut seen, &mut out);
    out
}

fn walk_reads(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::DelayControl { delay })
        | NodeKind::Stmt(StmtKind::Assign {
            delay: Some(IntraControl::Delay(delay)),
            ..
        }) => walk_reads(db, *delay, seen, out),
        NodeKind::Stmt(StmtKind::CycleDelayControl { count })
        | NodeKind::Stmt(StmtKind::Assign {
            delay: Some(IntraControl::Cycle { count, .. }),
            ..
        }) => walk_reads(db, *count, seen, out),
        _ => {}
    }
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::ProcContAssign { lhs, rhs } | StmtKind::Force { lhs, rhs }) => {
            walk_reads(db, *rhs, seen, out);
            walk_lhs_select_reads(db, *lhs, seen, out);
            return;
        }
        NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
            if let Some(initializer) = db.var_initializer(*declaration) {
                walk_reads(db, initializer, seen, out);
            }
            return;
        }
        NodeKind::Expr(ExprKind::NewArray { size, initializer }) => {
            walk_reads(db, *size, seen, out);
            if let Some(initializer) = initializer {
                walk_reads(db, *initializer, seen, out);
            }
            return;
        }
        NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
            if let Some(rhs) = db.node(node).children.get(1) {
                walk_reads(db, *rhs, seen, out);
            }
            if let Some(lhs) = db.node(node).children.first() {
                walk_lhs_select_reads(db, *lhs, seen, out);
            }
            return;
        }
        NodeKind::Stmt(StmtKind::Release { .. }) | NodeKind::Stmt(StmtKind::Deassign { .. }) => {
            return;
        }
        _ => {}
    }
    add_read(db, node, seen, out);
    for c in &db.node(node).children {
        walk_reads(db, *c, seen, out);
    }
}

/// Walk only the index/bounds expressions of an assignment LHS (the base
/// itself is the write target, never a read).
fn walk_lhs_select_reads(db: &Db, lhs: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(lhs) {
        NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
            walk_reads(db, *index, seen, out);
        }
        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
            walk_reads(db, *left, seen, out);
            walk_reads(db, *right, seen, out);
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base_expr,
            width_expr,
            ..
        }) => {
            walk_reads(db, *base_expr, seen, out);
            walk_reads(db, *width_expr, seen, out);
        }
        _ => {}
    }
}

fn add_read(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    // Reading a clockvar bound to a computed expression reads its operands.
    let clockvar = match db.node_kind(node) {
        NodeKind::Expr(ExprKind::Ref { target: Some(t) } | ExprKind::ScopeRef { target: t }) => {
            Some(*t)
        }
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
            refs.iter().rev().flatten().copied().next()
        }
        _ => Some(node),
    };
    if let Some(var) = clockvar.and_then(|clockvar| db.clocking_var(clockvar)) {
        if var.source.is_none() && signal_of_ref(db, var.expression).is_none() {
            walk_reads(db, var.expression, seen, out);
            return;
        }
    }
    match db.node_kind(node) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => {
            if let Some(signal) = signal_of_ref(db, node) {
                if seen.insert(signal) {
                    out.push(signal);
                }
            }
        }
        NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
            if let Some(signal) = signal_of_ref(db, *t) {
                if seen.insert(signal) {
                    out.push(signal);
                }
            }
        }
        // `cb.x` names a clockvar through its clocking block scope.
        NodeKind::Expr(ExprKind::ScopeRef { target }) if db.is_clocking_var(*target) => {
            if let Some(signal) = signal_of_ref(db, *target) {
                if seen.insert(signal) {
                    out.push(signal);
                }
            }
        }
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
            if let Some(t) = refs
                .iter()
                .rev()
                .flatten()
                .find(|target| is_signal(db, **target))
            {
                if let Some(signal) = signal_of_ref(db, *t) {
                    if seen.insert(signal) {
                        out.push(signal);
                    }
                }
            }
        }
        _ => {}
    }
}

fn walk_writes(
    db: &Db,
    node: NodeId,
    members: bool,
    seen: &mut HashSet<NodeId>,
    out: &mut Vec<NodeId>,
) {
    match db.node_kind(node) {
        NodeKind::Stmt(
            StmtKind::ProcContAssign { lhs, .. }
            | StmtKind::Force { lhs, .. }
            | StmtKind::Release { lhs }
            | StmtKind::Deassign { lhs },
        ) => {
            add_lhs_write(db, *lhs, members, seen, out);
            return;
        }
        NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
            if db.var_initializer(*declaration).is_some() {
                add_lhs_write(db, *declaration, members, seen, out);
            }
            return;
        }
        NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
            if let Some(lhs) = db.node(node).children.first() {
                add_lhs_write(db, *lhs, members, seen, out);
            }
            return;
        }
        _ => {}
    }
    for c in &db.node(node).children {
        walk_writes(db, *c, members, seen, out);
    }
}

fn walk_driver_writes(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::ProcContAssign { lhs, .. } | StmtKind::Force { lhs, .. }) => {
            add_driver_lhs_write(db, *lhs, seen, out);
        }
        NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
            if let Some(initializer) = db.var_initializer(*declaration) {
                add_driver_lhs_write(db, *declaration, seen, out);
                // An initializer can call a subroutine with output actuals.
                walk_driver_writes(db, initializer, seen, out);
            }
            return;
        }
        NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
            if let Some(lhs) = db.node(node).children.first() {
                add_driver_lhs_write(db, *lhs, seen, out);
            }
        }
        NodeKind::Stmt(StmtKind::Release { .. }) | NodeKind::Stmt(StmtKind::Deassign { .. }) => {
            return;
        }
        // Assignments in expression position (`for` initializers and steps,
        // `a = (b = 1)`) and the increment/decrement operators, which behave
        // as blocking assignments (IEEE 1800-2009 11.4.2), write their first
        // operand.
        NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            assignment,
            ..
        }) if *assignment
            || matches!(
                op,
                Operation::Assignment
                    | Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement
                    | Operation::OverloadUpdate
                    | Operation::OverloadPostUpdate
            ) =>
        {
            if let Some(lhs) = operands.first() {
                add_driver_lhs_write(db, *lhs, seen, out);
            }
        }
        // The loop supplies the (read-only) iterator variables of a
        // `foreach` (IEEE 1800-2009 12.7.3), and a `for` initializer assigns
        // the variables it declares.
        NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => {
            for var in vars.iter().flatten() {
                add_driver_lhs_write(db, *var, seen, out);
            }
        }
        NodeKind::Stmt(StmtKind::For { vars, .. }) => {
            for var in vars {
                add_driver_lhs_write(db, *var, seen, out);
            }
        }
        NodeKind::FuncCall { .. } | NodeKind::SysCall { .. } | NodeKind::MethodCall { .. } => {
            for actual in call_written_actuals(db, node) {
                add_driver_lhs_write(db, actual, seen, out);
            }
        }
        // A randsequence production's formals and result, a rule's implicit
        // result variables and their default-valued reset sources are
        // written or initialized by each generation of the production
        // (IEEE 1800-2009 18.17.6, 18.17.7).
        NodeKind::Production(ProductionKind::Declaration(_) | ProductionKind::Rule(_)) => {
            for child in &db.node(node).children {
                if matches!(
                    db.node_kind(*child),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                ) && db.node(*child).parent == Some(node)
                {
                    add_driver_lhs_write(db, *child, seen, out);
                }
            }
        }
        _ => {}
    }
    // Calls nest inside assignment right-hand sides, conditions and actuals.
    for c in &db.node(node).children {
        walk_driver_writes(db, *c, seen, out);
    }
}

/// The actuals a call writes: output and inout actuals of any subroutine
/// (user task or function, DPI import, `$sscanf`/`$fscanf`/`$fgets`/`$fread`
/// destinations, `std::randomize` arguments, ...) and non-`const` `ref`
/// actuals. Slang binds every output/inout actual as an assignment operation
/// whose first operand is the written lvalue, so this needs no per-task table;
/// `ref` actuals are plain references and follow the callee's formal
/// direction. `$random`'s seed is the one system argument that is written
/// without that wrapper (IEEE 1364-2001 17.9.1: the seed is an in-out
/// parameter, as for the `$dist_*` functions in 17.9.2).
fn call_written_actuals(db: &Db, call: NodeId) -> Vec<NodeId> {
    let children = db.node(call).children.as_slice();
    let (actuals, callee) = match db.node_kind(call) {
        NodeKind::MethodCall {
            receiver, callee, ..
        } => (
            if receiver.is_some() && children.first() == receiver.as_ref() {
                &children[1..]
            } else {
                children
            },
            *callee,
        ),
        NodeKind::FuncCall { callee, .. } => (children, *callee),
        NodeKind::SysCall { name } => {
            let seed = (name == "$random")
                .then(|| children.first().copied())
                .flatten();
            let mut written = call_assigned_actuals(db, children);
            written.extend(seed);
            return written;
        }
        _ => return Vec::new(),
    };
    let mut written = call_assigned_actuals(db, actuals);
    // A built-in method's callee is absent or an opaque node; a user method
    // resolves to its declaration and is handled through its formals below.
    if let NodeKind::MethodCall { name, receiver, .. } = db.node_kind(call) {
        if callee.is_none_or(|callee| !matches!(db.node_kind(callee), NodeKind::FuncTask { .. })) {
            written.extend(builtin_method_writes(db, call, name, *receiver, actuals));
        }
    }
    let formals: Vec<NodeId> = callee
        .map(|callee| {
            db.node(callee)
                .children
                .iter()
                .copied()
                .filter(|child| matches!(db.node_kind(*child), NodeKind::FuncArg { .. }))
                .collect()
        })
        .unwrap_or_default();
    for (actual, formal) in actuals.iter().zip(&formals) {
        if matches!(
            db.node_kind(*formal),
            NodeKind::FuncArg {
                direction: Direction::Ref,
                const_ref: false,
                ..
            }
        ) {
            written.push(*actual);
        }
    }
    written
}

/// Signals a built-in method call writes beyond assignment-wrapped actuals:
/// the receiver of a method that stores into it (`q.push_back(x)`,
/// `s.putc(i, c)`, `s.itoa(n)`), the message variable of a mailbox
/// `get`/`peek` (IEEE 1800-2009 15.4.5 to 15.4.8) and the variables listed in
/// `obj.randomize(v, ...)`, which are randomized (18.11).
fn builtin_method_writes(
    db: &Db,
    call: NodeId,
    name: &str,
    receiver: Option<NodeId>,
    actuals: &[NodeId],
) -> Vec<NodeId> {
    match name {
        "push_back" | "push_front" | "insert" | "putc" | "itoa" | "hextoa" | "octtoa"
        | "bintoa" | "realtoa" => receiver.into_iter().collect(),
        "get" | "try_get" | "peek" | "try_peek" => {
            let is_mailbox = receiver
                .and_then(|receiver| {
                    db.type_descriptor(receiver).or_else(|| {
                        signal_of_ref(db, receiver).and_then(|signal| db.type_descriptor(signal))
                    })
                })
                .is_some_and(|ty| ty.name.starts_with("mailbox"));
            match actuals {
                [message] if is_mailbox => vec![*message],
                _ => Vec::new(),
            }
        }
        "randomize" if db.randomize_call(call).is_some() => actuals
            .iter()
            .copied()
            .filter(|actual| matches!(db.node_kind(*actual), NodeKind::Expr(_)))
            .collect(),
        _ => Vec::new(),
    }
}

/// First operands of the assignment-wrapped (output/inout) actuals.
fn call_assigned_actuals(db: &Db, actuals: &[NodeId]) -> Vec<NodeId> {
    actuals
        .iter()
        .filter_map(|actual| match db.node_kind(*actual) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands.first().copied(),
            _ => None,
        })
        .collect()
}

fn add_lhs_write(
    db: &Db,
    lhs: NodeId,
    members: bool,
    seen: &mut HashSet<NodeId>,
    out: &mut Vec<NodeId>,
) {
    if members {
        add_lhs_members(db, lhs, |db, id| [signal_of_ref(db, id), None], seen, out);
    } else if let Some(sig) = signal_of_ref(db, lhs) {
        if seen.insert(sig) {
            out.push(sig);
        }
    }
}

/// Add the signal each member of an assignment target resolves to.  A
/// concatenation, streaming or assignment-pattern target writes each of its
/// members (IEEE 1800-2009 11.4.12 and 11.4.14.3 for concatenations and
/// streaming targets, 10.9 for assignment patterns); any other target is
/// resolved as a whole by `resolve`.
fn add_lhs_members(
    db: &Db,
    lhs: NodeId,
    resolve: fn(&Db, NodeId) -> [Option<NodeId>; 2],
    seen: &mut HashSet<NodeId>,
    out: &mut Vec<NodeId>,
) {
    match db.node_kind(lhs) {
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::Concat | Operation::AssignmentPattern,
            operands,
            ..
        }) => {
            for member in operands {
                add_lhs_members(db, *member, resolve, seen, out);
            }
            return;
        }
        NodeKind::Expr(ExprKind::Streaming { streams, .. }) => {
            for stream in streams {
                add_lhs_members(db, stream.value, resolve, seen, out);
            }
            return;
        }
        _ => {}
    }
    for sig in resolve(db, lhs).into_iter().flatten() {
        if seen.insert(sig) {
            out.push(sig);
        }
    }
}

/// Resolve a writable expression to its whole declared object.
///
/// Unlike [`signal_of_ref`], this helper also follows unpacked
/// [`ExprKind::ArraySelect`] bases and hierarchical-path endpoints.  A write
/// to any selected bit or element therefore drives the complete owning
/// object, which is the conservative granularity used by the linter.
pub fn driver_signal_of_lhs(db: &Db, id: NodeId) -> Option<NodeId> {
    match db.node_kind(id) {
        NodeKind::Expr(
            ExprKind::BitSelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. }
            | ExprKind::ArraySelect { base, .. },
        ) => driver_signal_of_lhs(db, *base),
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
            .iter()
            .rev()
            .flatten()
            .copied()
            .find(|target| is_signal(db, *target)),
        _ => signal_of_ref(db, id),
    }
}

fn add_driver_lhs_write(db: &Db, lhs: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    add_lhs_members(db, lhs, driven_objects_of_lhs, seen, out);
}

/// The objects a write to `id` drives: [`driver_signal_of_lhs`] plus, for a
/// write to a member of a struct or union, the declared object that owns it.
/// The frontend models such a member as a detached node (no parent) at the end
/// of a hierarchical path (or as the base of a member-array select, which the
/// database maps to its owner); a class property or a signal in another scope
/// has a parent and stays the only driven object.
fn driven_objects_of_lhs(db: &Db, id: NodeId) -> [Option<NodeId>; 2] {
    [driver_signal_of_lhs(db, id), member_owner_of_lhs(db, id)]
}

fn member_owner_of_lhs(db: &Db, id: NodeId) -> Option<NodeId> {
    if let Some((owner, _)) = db.array_select_path(id) {
        return driver_signal_of_lhs(db, owner);
    }
    match db.node_kind(id) {
        NodeKind::Expr(
            ExprKind::BitSelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. }
            | ExprKind::ArraySelect { base, .. },
        ) => member_owner_of_lhs(db, *base),
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
            let mut signals: Vec<NodeId> = refs
                .iter()
                .flatten()
                .copied()
                .filter(|target| is_signal(db, *target))
                .collect();
            let mut stepped = false;
            while signals.len() > 1
                && signals
                    .last()
                    .is_some_and(|last| db.node(*last).parent.is_none())
            {
                signals.pop();
                stepped = true;
            }
            stepped.then(|| signals.last().copied()).flatten()
        }
        _ => None,
    }
}

/// The executable body of a function/task node: the frontend-recorded body
/// statement.  A subroutine with loop or block scopes lists extra scope-only
/// statement children after the body, so the last statement child is only a
/// fallback for nodes that record no body.
pub fn subroutine_body(db: &Db, id: NodeId) -> Option<NodeId> {
    db.subroutine_body(id).or_else(|| {
        db.node(id)
            .children
            .iter()
            .rev()
            .copied()
            .find(|child| matches!(db.node_kind(*child), NodeKind::Stmt(_)))
    })
}

/// Default-value expressions of a function/task node's formals. They are
/// evaluated at the call site but read the declaring scope's signals.
pub fn subroutine_formal_defaults(db: &Db, id: NodeId) -> Vec<NodeId> {
    db.node(id)
        .children
        .iter()
        .filter_map(|child| match db.node_kind(*child) {
            NodeKind::FuncArg { default, .. } => *default,
            _ => None,
        })
        .collect()
}

/// Every node in the design tree (from the top instances), depth-first, in
/// deterministic order.
pub fn all_nodes(db: &Db) -> Vec<NodeId> {
    fn walk(db: &Db, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for c in &db.node(id).children {
            walk(db, *c, out);
        }
    }
    let mut out = Vec::new();
    for top in db.tops() {
        walk(db, *top, &mut out);
    }
    out
}

/// Every node reachable from all independent design roots.  Package and
/// class definitions are not descendants of elaborated top instances, so
/// source-oriented rules that promise function/task coverage must use this
/// traversal rather than [`all_nodes`].
pub fn all_design_nodes(db: &Db) -> Vec<NodeId> {
    fn walk(db: &Db, id: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
        if !seen.insert(id) {
            return;
        }
        out.push(id);
        for child in &db.node(id).children {
            walk(db, *child, seen, out);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for root in db
        .tops()
        .iter()
        .chain(db.packages().iter())
        .chain(db.classes().iter())
    {
        walk(db, *root, &mut seen, &mut out);
    }
    out
}

/// Iterate every "instance" scope — module instances and generate scopes —
/// paired with a display path.  Generate scopes count as instances because
/// they declare their own signals (checked by `unused-signal`).
pub fn iter_instances(db: &Db) -> impl Iterator<Item = (NodeId, String)> + '_ {
    fn walk_scopes(db: &Db, id: NodeId, out: &mut Vec<(NodeId, String)>) {
        out.push((id, scope_path(db, id)));
        for c in &db.node(id).children {
            match db.node_kind(*c) {
                // A conditional or case generate block is a scope of its own,
                // a direct child of the enclosing scope (no loop array).
                NodeKind::ModuleInst { .. } | NodeKind::GenScope => walk_scopes(db, *c, out),
                NodeKind::GenScopeArray => {
                    for g in &db.node(*c).children {
                        if matches!(db.node_kind(*g), NodeKind::GenScope) {
                            walk_scopes(db, *g, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for top in db.tops() {
        walk_scopes(db, *top, &mut out);
    }
    out.into_iter()
}

/// Display path of a scope (module instance or gen scope), e.g. `"top.u0"` or
/// `"top.g[0]"`.  The top instance yields its own name (unlike
/// [`Db::instance_path`], which returns `""` for the top); loop generate
/// blocks follow [`Db::hierarchical_name_parts`], the naming `%m` uses.
pub fn scope_path(db: &Db, id: NodeId) -> String {
    db.hierarchical_name_parts(id).join(".")
}

/// Display path of the scope declaring `sig` (its parent scope); empty when
/// the signal has no declaring scope.
pub fn signal_scope_path(db: &Db, sig: NodeId) -> String {
    db.node(sig)
        .parent
        .map(|p| scope_path(db, p))
        .unwrap_or_default()
}

/// Every signal connected to a port on either side of a binding.
///
/// Port-connected signals are exempt from the unused-signal rule: the port
/// binding is the use.
pub fn port_connected_signals(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        for c in &db.node(id).children {
            if let NodeKind::Port { high, low, .. } = db.node_kind(*c) {
                out.extend(high.iter().copied());
                out.extend(port_internal_signals(db, *c, *low));
            }
        }
    }
    out
}

/// True when an instance port carries no usable parent-side connection:
/// it is omitted from the connection list / a positional gap, or explicitly
/// left open with an empty `.p()` connection.  A resolved signal ref, an
/// expression or a constant connection all count as connected; `` `.* `` and
/// `.name` shorthand connections produce ordinary resolved refs and are
/// connected.
pub fn port_unconnected(db: &Db, id: NodeId) -> bool {
    match db.node_kind(id) {
        NodeKind::Port {
            high_present,
            high_open,
            ..
        } => !*high_present || *high_open,
        _ => false,
    }
}

/// The constant part of its signal that a driver target selects: the
/// unpacked element indices, outermost first (fewer than the array has
/// select a whole sub-array), and within them a bit interval `(low, high)`
/// in declared indices (`None` for every bit).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrivenRegion {
    pub elements: Vec<i128>,
    pub bits: Option<(i128, i128)>,
}

impl DrivenRegion {
    /// A bit interval of a signal without unpacked dimensions.
    pub fn bits(low: i128, high: i128) -> Self {
        Self {
            elements: Vec::new(),
            bits: Some((low, high)),
        }
    }

    /// Whether the two regions share a bit: one's elements contain the
    /// other's, and their bit intervals (if both have one) intersect.
    pub fn overlaps(&self, other: &Self) -> bool {
        let common = self.elements.len().min(other.elements.len());
        if self.elements[..common] != other.elements[..common] {
            return false;
        }
        if self.elements.len() != other.elements.len() {
            return true;
        }
        match (self.bits, other.bits) {
            (Some(left), Some(right)) => left.0 <= right.1 && right.0 <= left.1,
            _ => true,
        }
    }
}

/// The part of its signal a driver target drives; `None` for a whole signal
/// or a select whose indices or bounds are not constants.
pub type DrivenBits = Option<DrivenRegion>;

/// Whether `base` names `signal` itself: a reference to it, or the signal
/// node an unpacked element select hangs off.
fn names_signal(db: &Db, base: NodeId, signal: NodeId) -> bool {
    matches!(
        db.node_kind(base),
        NodeKind::Expr(ExprKind::Ref { .. })
            | NodeKind::Net { .. }
            | NodeKind::Var { .. }
            | NodeKind::Array { .. }
    ) && signal_of_ref(db, base) == Some(signal)
}

/// The elements of `signal` that `expr` selects with constant unpacked
/// indices (empty for the signal itself); `None` for any other expression.
fn driven_elements(db: &Db, expr: NodeId, signal: NodeId) -> Option<Vec<i128>> {
    if names_signal(db, expr, signal) {
        return Some(Vec::new());
    }
    let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = db.node_kind(expr) else {
        return None;
    };
    let mut elements = driven_elements(db, *base, signal)?;
    for index in indices {
        elements.push(const_i128(db, *index)?);
    }
    Some(elements)
}

/// The part of `signal` that the target expression `expr` drives: constant
/// unpacked element selects, then a constant bit, part or indexed part
/// select, or the whole signal (`None`).
pub fn driven_bit_interval(db: &Db, expr: NodeId, signal: NodeId) -> DrivenBits {
    let region = |base: &NodeId, bits: Option<(i128, i128)>| {
        let elements = driven_elements(db, *base, signal)?;
        Some(DrivenRegion { elements, bits })
    };
    match db.node_kind(expr) {
        NodeKind::Expr(ExprKind::ArraySelect { .. }) => {
            let elements = driven_elements(db, expr, signal)?;
            Some(DrivenRegion {
                elements,
                bits: None,
            })
        }
        NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
            let index = const_i128(db, *index)?;
            region(base, Some((index, index)))
        }
        NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
            let (left, right) = (const_i128(db, *left)?, const_i128(db, *right)?);
            region(base, Some((left.min(right), left.max(right))))
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base,
            base_expr,
            width_expr,
            neg,
        }) => {
            let (start, width) = (const_i128(db, *base_expr)?, const_i128(db, *width_expr)?);
            if width < 1 {
                return None;
            }
            let bits = if *neg {
                (start.checked_sub(width - 1)?, start)
            } else {
                (start, start.checked_add(width - 1)?)
            };
            region(base, Some(bits))
        }
        _ => None,
    }
}

/// The signal an unpacked element select (with optional bit selects inside
/// the element) is rooted at.
fn element_select_signal(db: &Db, expr: NodeId) -> Option<NodeId> {
    match db.node_kind(expr) {
        NodeKind::Expr(
            ExprKind::ArraySelect { base, .. }
            | ExprKind::BitSelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. },
        ) => {
            if matches!(
                db.node_kind(*base),
                NodeKind::Expr(ExprKind::ArraySelect { .. })
            ) {
                element_select_signal(db, *base)
            } else if matches!(
                db.node_kind(expr),
                NodeKind::Expr(ExprKind::ArraySelect { .. })
            ) {
                signal_of_ref(db, *base)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The signals a port expression target names, each with the bits it
/// selects: every member of a concatenation, a select's base signal or a
/// whole signal (SV 23.2.2.1 port references, 23.2.2.2 explicit ports).
fn port_expression_targets(db: &Db, expr: NodeId, out: &mut Vec<(NodeId, DrivenBits)>) {
    if let NodeKind::Expr(ExprKind::Operation {
        op: Operation::Concat,
        operands,
        ..
    }) = db.node_kind(expr)
    {
        for member in operands {
            port_expression_targets(db, *member, out);
        }
        return;
    }
    if let Some(signal) = signal_of_ref(db, expr).or_else(|| element_select_signal(db, expr)) {
        out.push((signal, driven_bit_interval(db, expr, signal)));
    }
}

/// The child-side signals of a port: its whole internal symbol `low`, or
/// every signal its select or concatenation port expression names
/// (`Db::port_internal_expression`), with the bits it covers.
pub fn port_internal_targets(
    db: &Db,
    port: NodeId,
    low: Option<NodeId>,
) -> Vec<(NodeId, DrivenBits)> {
    if let Some(low) = low {
        return vec![(low, None)];
    }
    let mut out = Vec::new();
    if let Some(expr) = db.port_internal_expression(port) {
        port_expression_targets(db, expr, &mut out);
    }
    out
}

/// The parent-side signals an output or inout connection drives: each
/// member of a concatenation actual and the base of a select actual, with
/// the bits it covers, or the resolved whole `high` signal.
pub fn port_actual_targets(
    db: &Db,
    high: Option<NodeId>,
    high_expr: Option<NodeId>,
) -> Vec<(NodeId, DrivenBits)> {
    let mut out = Vec::new();
    if let Some(expr) = high_expr {
        port_expression_targets(db, expr, &mut out);
    }
    if out.is_empty() {
        out.extend(high.map(|high| (high, None)));
    }
    out
}

/// [`port_internal_targets`] without the bit intervals.
pub fn port_internal_signals(db: &Db, port: NodeId, low: Option<NodeId>) -> Vec<NodeId> {
    port_internal_targets(db, port, low)
        .into_iter()
        .map(|(signal, _)| signal)
        .collect()
}

/// Drivers contributed by port links, with the bits each drives: an input
/// port drives its child-side signals, an output port its parent-side
/// signal, and an inout port both sides.  A parent-side bits interval comes
/// from a constant select actual (`.p(v[3])`).
pub fn port_link_drivers(db: &Db) -> Vec<(NodeId, DrivenBits)> {
    let mut out = Vec::new();
    for (id, _) in iter_instances(db) {
        for c in &db.node(id).children {
            let NodeKind::Port {
                direction,
                high,
                low,
                high_expr,
                ..
            } = db.node_kind(*c)
            else {
                continue;
            };
            let parent = || port_actual_targets(db, *high, *high_expr);
            match direction {
                Direction::Input => out.extend(port_internal_targets(db, *c, *low)),
                Direction::Output => out.extend(parent()),
                Direction::Inout => {
                    out.extend(port_internal_targets(db, *c, *low));
                    if low.is_none() || low != high {
                        out.extend(parent());
                    }
                }
                Direction::Mixed | Direction::None | Direction::Ref | Direction::Unsupported => {}
            }
        }
    }
    out
}

/// Signals driven by a connected port link, without counting top-level
/// external ports or ports already classified as unconnected.
///
/// This set intentionally has different semantics from [`port_link_drivers`],
/// whose driver list is used by `multi-driver` and includes every resolved
/// side.  It is the active-flow view needed by `undriven-signal`.
pub fn connected_port_link_drivers(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        let is_top = matches!(db.node_kind(id), NodeKind::ModuleInst { is_top: true, .. });
        if is_top {
            continue;
        }
        for c in &db.node(id).children {
            let NodeKind::Port {
                direction,
                high,
                low,
                high_expr,
                ..
            } = db.node_kind(*c)
            else {
                continue;
            };
            if port_unconnected(db, *c) {
                continue;
            }
            match direction {
                Direction::Input => {
                    out.extend(port_internal_signals(db, *c, *low));
                }
                Direction::Output => {
                    out.extend(
                        port_actual_targets(db, *high, *high_expr)
                            .into_iter()
                            .map(|t| t.0),
                    );
                }
                Direction::Inout => {
                    out.extend(port_internal_signals(db, *c, *low));
                    out.extend(
                        port_actual_targets(db, *high, *high_expr)
                            .into_iter()
                            .map(|t| t.0),
                    );
                }
                // A `ref` port aliases the actual: either side may write it
                // (IEEE 1800-2009 23.3.3 shared variable behavior), so both count as driven.
                Direction::Ref => {
                    out.extend(port_internal_signals(db, *c, *low));
                    out.extend(high.iter().copied());
                }
                Direction::Mixed | Direction::None | Direction::Unsupported => {}
            }
        }
    }
    out
}

/// Signals read through a connected port link, without counting top-level
/// external ports or ports already classified as unconnected.  Input and
/// inout high connections use the owned expression tree when available, with
/// the resolved `high` target retained as a direct-connection fallback.
pub fn port_link_reads(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        let is_top = matches!(db.node_kind(id), NodeKind::ModuleInst { is_top: true, .. });
        if is_top {
            continue;
        }
        for c in &db.node(id).children {
            let NodeKind::Port {
                direction,
                high,
                high_expr,
                low,
                ..
            } = db.node_kind(*c)
            else {
                continue;
            };
            if port_unconnected(db, *c) {
                continue;
            }
            match direction {
                Direction::Input => {
                    if let Some(expr) = high_expr {
                        out.extend(collect_reads(db, *expr));
                    }
                    if let Some(sig) = high {
                        out.insert(*sig);
                    }
                }
                Direction::Output => {
                    out.extend(port_internal_signals(db, *c, *low));
                }
                Direction::Inout => {
                    out.extend(port_internal_signals(db, *c, *low));
                    if let Some(expr) = high_expr {
                        out.extend(collect_reads(db, *expr));
                    }
                    if let Some(sig) = high {
                        out.insert(*sig);
                    }
                }
                Direction::Mixed | Direction::None | Direction::Ref | Direction::Unsupported => {}
            }
        }
    }
    out
}

/// Backing signals of external top-level input and inout ports.
pub fn top_external_port_signals(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        if !matches!(db.node_kind(id), NodeKind::ModuleInst { is_top: true, .. }) {
            continue;
        }
        for c in &db.node(id).children {
            let NodeKind::Port { direction, low, .. } = db.node_kind(*c) else {
                continue;
            };
            if matches!(direction, Direction::Input | Direction::Inout) {
                out.extend(port_internal_signals(db, *c, *low));
            }
        }
    }
    out
}

/// Signals backing a child port whose parent-side connection is absent or
/// explicitly open.  `unconnected-port` owns these cases, so another rule
/// must not repeat them as undriven child formals.
pub fn unconnected_port_signals(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        if matches!(db.node_kind(id), NodeKind::ModuleInst { is_top: true, .. }) {
            continue;
        }
        for c in &db.node(id).children {
            if !port_unconnected(db, *c) {
                continue;
            }
            let NodeKind::Port { high, low, .. } = db.node_kind(*c) else {
                continue;
            };
            if let Some(sig) = high {
                out.insert(*sig);
            }
            out.extend(port_internal_signals(db, *c, *low));
        }
    }
    out
}

/// Signals read by input/inout terminals of captured structural primitives.
/// Primitive-array objects are skipped because the database has no terminal
/// expressions for their members.
pub fn gate_terminal_reads(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for id in all_nodes(db) {
        let NodeKind::Gate { class, terms, .. } = db.node_kind(id) else {
            continue;
        };
        if *class == crate::core::db::PrimClass::Array {
            continue;
        }
        for term in terms {
            if matches!(term.direction, Direction::Input | Direction::Inout) {
                out.extend(collect_reads(db, term.expr));
                if let Some(sig) = read_signal_of_expr(db, term.expr) {
                    out.insert(sig);
                }
            }
        }
    }
    out
}

/// Resolve a terminal expression's final signal when its expression is a
/// hierarchical path.  Ordinary refs/selects are already covered by
/// `collect_reads`; this narrow supplement keeps gate input activity visible
/// without changing that shared helper's existing semantics.
fn read_signal_of_expr(db: &Db, id: NodeId) -> Option<NodeId> {
    match db.node_kind(id) {
        NodeKind::Expr(
            ExprKind::BitSelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. }
            | ExprKind::ArraySelect { base, .. },
        ) => read_signal_of_expr(db, *base),
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
            .iter()
            .rev()
            .flatten()
            .find_map(|target| read_signal_of_expr(db, *target)),
        _ => signal_of_ref(db, id),
    }
}

/// Signals driven by output/inout terminals of captured structural
/// primitives.  Primitive-array objects are skipped because their terminal
/// expressions are unavailable.
pub fn gate_terminal_drivers(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for id in all_nodes(db) {
        let NodeKind::Gate { class, terms, .. } = db.node_kind(id) else {
            continue;
        };
        if *class == crate::core::db::PrimClass::Array {
            continue;
        }
        for term in terms {
            if !matches!(term.direction, Direction::Output | Direction::Inout) {
                continue;
            }
            if let Some(sig) = driver_signal_of_lhs(db, term.expr) {
                out.insert(sig);
            }
        }
    }
    out
}

/// The variables local to `process`: block-local declarations and `for`
/// loop variables declared by its statements, plus any variable whose
/// declaration node lies under it. A declaration node may live in a scope
/// outside the process, so the declaring statements identify it.
pub fn process_local_vars(db: &Db, process: NodeId) -> HashSet<NodeId> {
    fn walk(db: &Db, node: NodeId, out: &mut HashSet<NodeId>) {
        match db.node_kind(node) {
            NodeKind::Stmt(StmtKind::For { vars, .. }) => out.extend(vars.iter().copied()),
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                out.insert(*declaration);
            }
            NodeKind::Var { .. } | NodeKind::Array { .. } => {
                out.insert(node);
            }
            _ => {}
        }
        for child in &db.node(node).children {
            walk(db, *child, out);
        }
    }
    let mut out = HashSet::new();
    walk(db, process, &mut out);
    out
}

/// True for `always_comb` / `always @*` processes (combinational).  Latches
/// (`always_latch`) and explicit event controls are not combinational.
pub fn is_comb_process(db: &Db, id: NodeId) -> bool {
    if let NodeKind::Process {
        kind: crate::core::db::ProcessKind::Always { always_type },
    } = db.node_kind(id)
    {
        if *always_type == AlwaysKind::Comb {
            return true;
        }
    }
    has_implicit_event(db, id)
}

/// True for `always_comb` / `always_latch` / `always @*` processes — the
/// set the incomplete-case rule cares about.
pub fn is_comb_or_latch_process(db: &Db, id: NodeId) -> bool {
    if let NodeKind::Process {
        kind: crate::core::db::ProcessKind::Always { always_type },
    } = db.node_kind(id)
    {
        if matches!(always_type, AlwaysKind::Comb | AlwaysKind::Latch) {
            return true;
        }
    }
    has_implicit_event(db, id)
}

/// True when the tree rooted at `root` contains an `@*` event control
/// (`EventControl { implicit: true }`).
pub fn has_implicit_event(db: &Db, root: NodeId) -> bool {
    if let NodeKind::Stmt(StmtKind::EventControl { implicit: true, .. }) = db.node_kind(root) {
        return true;
    }
    db.node(root)
        .children
        .iter()
        .any(|c| has_implicit_event(db, *c))
}

/// True when the process body contains a delay control or a non-implicit
/// (timed/edge) event control; such processes are not purely combinational.
pub fn has_timing_control(db: &Db, root: NodeId) -> bool {
    match db.node_kind(root) {
        NodeKind::Stmt(StmtKind::DelayControl { .. } | StmtKind::CycleDelayControl { .. }) => {
            return true
        }
        NodeKind::Stmt(StmtKind::EventControl { implicit, .. }) => return !*implicit,
        _ => {}
    }
    db.node(root)
        .children
        .iter()
        .any(|c| has_timing_control(db, *c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, find_signal};

    fn db_of(sv: &str) -> Db {
        let (db, _) = build_design(sv, "");
        db
    }

    /// First top-level continuous assignment of the design.
    fn top_cont_assign(db: &Db) -> NodeId {
        for top in db.tops() {
            for c in &db.node(*top).children {
                if matches!(db.node_kind(*c), NodeKind::ContAssign { .. }) {
                    return *c;
                }
            }
        }
        panic!("no top-level cont assign");
    }

    #[test]
    fn signal_of_ref_resolves_through_selects() {
        let db = db_of("module t; logic [7:0] a; logic b; assign b = a[3]; endmodule");
        let a = find_signal(&db, "a");
        let ca = top_cont_assign(&db);
        let rhs = db.node(ca).children[1];
        assert_eq!(signal_of_ref(&db, rhs), Some(a));
        assert_eq!(expr_width(&db, rhs), Some(1));
    }

    #[test]
    fn collect_reads_excludes_lhs_base() {
        let db = db_of("module t; logic a, b, c; assign a = b ^ c; endmodule");
        let a = find_signal(&db, "a");
        let b = find_signal(&db, "b");
        let c = find_signal(&db, "c");
        let ca = top_cont_assign(&db);
        let reads = collect_reads(&db, ca);
        assert!(!reads.contains(&a), "LHS base must not be a read");
        assert!(reads.contains(&b));
        assert!(reads.contains(&c));
    }

    #[test]
    fn procedural_drivers_use_typed_operands_for_reads_and_writes() {
        let db = db_of(
            "module t; reg q, source; initial begin \
             force q = source; release q; assign q = source; deassign q; end endmodule",
        );
        let q = find_signal(&db, "q");
        let source = find_signal(&db, "source");
        let mut assignments = 0;
        let mut cancellations = 0;
        for id in db.node_ids() {
            match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::Force { .. } | StmtKind::ProcContAssign { .. }) => {
                    assert_eq!(collect_reads(&db, id), vec![source]);
                    assert_eq!(collect_writes(&db, id), vec![q]);
                    assert_eq!(collect_driver_writes(&db, id), vec![q]);
                    assignments += 1;
                }
                NodeKind::Stmt(StmtKind::Release { .. } | StmtKind::Deassign { .. }) => {
                    assert!(collect_reads(&db, id).is_empty());
                    assert_eq!(collect_writes(&db, id), vec![q]);
                    assert!(collect_driver_writes(&db, id).is_empty());
                    cancellations += 1;
                }
                _ => {}
            }
        }
        assert_eq!((assignments, cancellations), (2, 2));
    }

    #[test]
    fn collect_writes_finds_assign_and_cont_assign_lhs() {
        let db = db_of(
            "module t; logic clk, a, b; always @(posedge clk) a <= b; assign b = a; endmodule",
        );
        let a = find_signal(&db, "a");
        let b = find_signal(&db, "b");
        // The process writes only `a`.
        let proc = db
            .tops()
            .iter()
            .flat_map(|t| db.node(*t).children.clone())
            .find(|c| matches!(db.node_kind(*c), NodeKind::Process { .. }))
            .expect("a process");
        let proc_writes = collect_writes(&db, proc);
        assert_eq!(proc_writes, vec![a]);
        // The cont assign writes only `b`.
        let ca = top_cont_assign(&db);
        let ca_writes = collect_writes(&db, ca);
        assert_eq!(ca_writes, vec![b]);
    }

    #[test]
    fn collect_reads_includes_event_control_signal() {
        let db = db_of("module t; logic clk, x; always @(posedge clk) x <= 1'b0; endmodule");
        let clk = find_signal(&db, "clk");
        let proc = db
            .tops()
            .iter()
            .flat_map(|t| db.node(*t).children.clone())
            .find(|c| matches!(db.node_kind(*c), NodeKind::Process { .. }))
            .expect("a process");
        let reads = collect_reads(&db, proc);
        assert!(reads.contains(&clk), "posedge clk is a read");
    }

    #[test]
    fn collect_reads_includes_statement_and_assignment_delay_values() {
        let db = db_of(
            "module t; int before_delay, assignment_delay; logic x; \
             initial begin #before_delay x = #assignment_delay 1'b0; end endmodule",
        );
        let process = db
            .tops()
            .iter()
            .flat_map(|top| db.node(*top).children.iter().copied())
            .find(|id| matches!(db.node_kind(*id), NodeKind::Process { .. }))
            .expect("initial process");
        let reads = collect_reads(&db, process);
        assert!(reads.contains(&find_signal(&db, "before_delay")));
        assert!(reads.contains(&find_signal(&db, "assignment_delay")));
        assert!(
            !reads.contains(&find_signal(&db, "x")),
            "assignment target is not a read"
        );
    }

    #[test]
    fn expr_width_concat_part_select_arith() {
        let db = db_of(
            "module t; logic [7:0] a, b; logic [15:0] c; logic [3:0] d; \
             assign c = {a, b}; assign d = c[7:4]; endmodule",
        );
        // First assign: concat {a, b} → 16 bits.
        let cas = top_cont_assigns(&db);
        let (ca1, ca2) = (cas[0], cas[1]);
        let rhs1 = db.node(ca1).children[1];
        assert_eq!(expr_width(&db, rhs1), Some(16));
        // Second assign: part select [7:4] → 4 bits.
        let rhs2 = db.node(ca2).children[1];
        assert_eq!(expr_width(&db, rhs2), Some(4));
        let lhs2 = db.node(ca2).children[0];
        assert_eq!(expr_width(&db, lhs2), Some(4));
    }

    #[test]
    fn expr_width_operation_groups() {
        let db = db_of(
            "module t; logic [3:0] a; logic [7:0] b, c, d; logic [1:0] s; \
             assign b = a + c; assign b = a << s; assign b = (a == c); \
             assign b = (a & c) | d; endmodule",
        );
        let cas = top_cont_assigns(&db);
        // a + c → max(4, 8) = 8
        let sum = db.node(cas[0]).children[1];
        assert_eq!(expr_width(&db, sum), Some(8));
        // a << s → LHS width 4
        let shift = db.node(cas[1]).children[1];
        assert_eq!(expr_width(&db, shift), Some(4));
        // a == c → 1
        let cmp = db.node(cas[2]).children[1];
        assert_eq!(expr_width(&db, cmp), Some(1));
        // (a & c) | d → max(max(4,8), 8) = 8
        let mix = db.node(cas[3]).children[1];
        assert_eq!(expr_width(&db, mix), Some(8));
    }

    #[test]
    fn expr_width_uses_select_and_replication_values() {
        let db = db_of(
            "module t; logic [7:0] a; logic [2:0] b; logic [23:0] c; \
             assign b = a[1 +: 3]; assign c = {3{a}}; endmodule",
        );
        let assignments = top_cont_assigns(&db);
        assert_eq!(
            expr_width(&db, db.node(assignments[0]).children[1]),
            Some(3)
        );
        assert_eq!(
            expr_width(&db, db.node(assignments[1]).children[1]),
            Some(24)
        );
    }

    #[test]
    fn iter_instances_includes_gen_scopes() {
        let db = db_of(
            "module t; logic [3:0] s; genvar i; \
             for (i = 0; i < 4; i = i + 1) begin : g \
               logic x; assign x = s[i]; \
             end endmodule",
        );
        let insts: Vec<(NodeId, String)> = iter_instances(&db).collect();
        // Top plus one gen scope per unrolled iteration.
        assert!(
            insts.len() >= 5,
            "expected top + 4 gen scopes, got {}",
            insts.len()
        );
        assert_eq!(insts[0].0, db.tops()[0], "top first");
        // Every gen scope path starts with the top name.
        for (_id, path) in &insts[1..] {
            assert!(path.starts_with("t."), "gen scope path {path:?}");
        }
    }

    #[test]
    fn port_link_drivers_maps_direction_to_side() {
        let db = db_of(
            "module child(input wire a, output wire b); endmodule \
             module top; wire in_a, out_b; child u0 (.a(in_a), .b(out_b)); endmodule",
        );
        let in_a = find_signal(&db, "in_a");
        let out_b = find_signal(&db, "out_b");
        let drivers = port_link_drivers(&db);
        // An input port drives the child-side signal (`low`), not the parent
        // net `in_a`; an output port drives the parent-side signal `out_b`.
        assert!(
            !drivers.iter().any(|(signal, _)| *signal == in_a),
            "input port must not drive the parent net"
        );
        let out_b_drivers: Vec<_> = drivers
            .iter()
            .filter(|(signal, _)| *signal == out_b)
            .collect();
        assert_eq!(
            out_b_drivers,
            [&(out_b, None)],
            "output port drives the whole parent net once"
        );
    }

    #[test]
    fn port_link_drivers_record_constant_bit_ranges() {
        let db = db_of(
            "module child(input wire a, output wire b); endmodule \
             module top; wire [3:0] n; child u0 (.a(n[0]), .b(n[2])); \
             child u1 (.a(n[0]), .b(n[3:3])); endmodule",
        );
        let n = find_signal(&db, "n");
        let mut bits: Vec<_> = port_link_drivers(&db)
            .into_iter()
            .filter(|(signal, _)| *signal == n)
            .map(|(_, bits)| bits)
            .collect();
        bits.sort();
        assert_eq!(
            bits,
            [
                Some(DrivenRegion::bits(2, 2)),
                Some(DrivenRegion::bits(3, 3))
            ]
        );
    }

    #[test]
    fn connected_port_activity_maps_source_and_skips_open_ports() {
        let db = db_of(
            "module child(input wire a, output wire b); endmodule \
             module top; wire in_a, out_b; \
             child connected (.a(in_a), .b(out_b)); \
             child open (.b(out_b)); endmodule",
        );
        let connected = iter_instances(&db)
            .find(|(_, path)| path == "top.connected")
            .map(|(id, _)| id)
            .expect("connected child instance");
        let open = iter_instances(&db)
            .find(|(_, path)| path == "top.open")
            .map(|(id, _)| id)
            .expect("open child instance");
        let connected_input =
            db.node(connected)
                .children
                .iter()
                .find_map(|c| match db.node_kind(*c) {
                    NodeKind::Port { low: Some(low), .. } if db.node(*c).name == "a" => Some(*low),
                    _ => None,
                });
        let open_input = db
            .node(open)
            .children
            .iter()
            .find_map(|c| match db.node_kind(*c) {
                NodeKind::Port { low: Some(low), .. } if db.node(*c).name == "a" => Some(*low),
                _ => None,
            });
        let connected_output =
            db.node(connected)
                .children
                .iter()
                .find_map(|c| match db.node_kind(*c) {
                    NodeKind::Port { low: Some(low), .. } if db.node(*c).name == "b" => Some(*low),
                    _ => None,
                });
        let in_a = find_signal(&db, "in_a");
        let out_b = find_signal(&db, "out_b");
        let reads = port_link_reads(&db);
        let drivers = connected_port_link_drivers(&db);

        assert!(reads.contains(&in_a), "connected input actual is read");
        assert!(reads.contains(&connected_output.expect("connected output low")));
        assert!(drivers.contains(&connected_input.expect("connected input low")));
        assert!(
            drivers.contains(&out_b),
            "connected output actual is driven"
        );
        assert!(open_input.is_some(), "open input has a backing signal");
        assert!(
            !drivers.contains(&open_input.expect("open input low")),
            "unconnected input is not an active port-link driver"
        );
    }

    #[test]
    fn captures_named_and_positional_high_connection_expressions() {
        let db = db_of(
            "module child(input wire i); endmodule \
             module top; logic named_floating, positional_floating; \
             child named (.i(named_floating & 1'b1)); \
             child positional (positional_floating | 1'b0); endmodule",
        );
        let port_for = |path: &str| {
            let instance = iter_instances(&db)
                .find(|(_, instance_path)| instance_path == path)
                .map(|(id, _)| id)
                .expect("child instance");
            db.node(instance)
                .children
                .iter()
                .copied()
                .find(|child| {
                    db.node(*child).name == "i"
                        && matches!(db.node_kind(*child), NodeKind::Port { .. })
                })
                .expect("child input port")
        };
        let named_port = port_for("top.named");
        let positional_port = port_for("top.positional");
        let high_expr = |port: NodeId| match db.node_kind(port) {
            NodeKind::Port {
                high,
                high_expr: Some(expr),
                high_present,
                high_open,
                ..
            } => {
                assert!(high.is_none(), "an operation has no direct high target");
                assert!(*high_present, "the expression is a present connection");
                assert!(!*high_open, "the expression is not an open connection");
                *expr
            }
            _ => panic!("expected captured high expression"),
        };
        let named_expr = high_expr(named_port);
        let positional_expr = high_expr(positional_port);
        assert!(matches!(
            db.node_kind(named_expr),
            NodeKind::Expr(ExprKind::Operation { .. })
        ));
        assert!(matches!(
            db.node_kind(positional_expr),
            NodeKind::Expr(ExprKind::Operation { .. })
        ));
        assert!(db.node(named_port).children.contains(&named_expr));
        assert!(db.node(positional_port).children.contains(&positional_expr));

        let named_floating = find_signal(&db, "named_floating");
        let positional_floating = find_signal(&db, "positional_floating");
        let reads = port_link_reads(&db);
        assert!(reads.contains(&named_floating), "named actual is read");
        assert!(
            reads.contains(&positional_floating),
            "positional actual is read"
        );
        assert_eq!(reads.len(), 2, "each actual is collected once");
        let drivers = connected_port_link_drivers(&db);
        assert!(!drivers.contains(&named_floating));
        assert!(!drivers.contains(&positional_floating));
    }

    #[test]
    fn gate_terminal_activity_maps_input_and_output_terms() {
        let db = db_of("module t; wire a, b, y; and g(y, a, b); endmodule");
        let a = find_signal(&db, "a");
        let b = find_signal(&db, "b");
        let y = find_signal(&db, "y");
        let reads = gate_terminal_reads(&db);
        let drivers = gate_terminal_drivers(&db);
        assert!(reads.contains(&a), "gate input a is read");
        assert!(reads.contains(&b), "gate input b is read");
        assert!(drivers.contains(&y), "gate output y is driven");
    }

    /// All top-level cont assigns, in capture order.
    fn top_cont_assigns(db: &Db) -> Vec<NodeId> {
        db.tops()
            .iter()
            .flat_map(|t| db.node(*t).children.clone())
            .filter(|c| matches!(db.node_kind(*c), NodeKind::ContAssign { .. }))
            .collect()
    }
}
