//! Shared analysis helpers for lint rules.
//!
//! Read/write collection, expression-width computation, scope/instance
//! iteration and port-link bookkeeping.  Everything is owned-db based (no VPI
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

#![allow(non_upper_case_globals)] // vpi op-type constants are lowercase by convention

use std::collections::{HashMap, HashSet};

use crate::core::db::{Db, ExprKind, NodeId, NodeKind, StmtKind};
use crate::core::model::Direction;
use crate::ffi::vpi::{self, ValueData};

/// True when `id` is a net, variable or array node.
pub fn is_signal(db: &Db, id: NodeId) -> bool {
    matches!(
        db.node_kind(id),
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
    )
}

/// The signal a ref-like expression ultimately refers to: a plain `Ref`
/// target, or the base of a bit/part/indexed-part select.  `None` when the
/// expression is not (or cannot be resolved to) a net/var/array.
pub fn signal_of_ref(db: &Db, id: NodeId) -> Option<NodeId> {
    match db.node_kind(id) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(id),
        NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|t| is_signal(db, *t)),
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
/// - `Constant` → `vpiSize` (unsized literals like `'1` report size -1 → None).
/// - `Ref` → width of the referenced object (signal or parameter).
/// - `BitSelect` → 1; `PartSelect` → `|left - right| + 1`; `IndexedPartSelect`
///   → the width expression.
/// - `Cast` → the cast target type width.
/// - `Operation` → per-op rule (see [`op_width`]).
/// - Anything else (calls, hier paths, unknown) → `None`.
pub fn expr_width(db: &Db, id: NodeId) -> Option<u32> {
    match db.node_kind(id) {
        NodeKind::Expr(ExprKind::Constant { size, value, .. }) => {
            if *size <= 0 {
                // Unsupported size or unsized literal (`'1`, `'x`): unknown.
                return None;
            }
            // Surelog folds `vpiSize` to the LHS width during elaboration but
            // keeps the literal's full value string; taking the max of the two
            // still catches real truncation (e.g. `8'hff` into 4 bits).
            let size = *size as u32;
            Some(constant_value_bits(value).map_or(size, |b| size.max(b)))
        }
        NodeKind::Expr(ExprKind::Ref { target }) => target.and_then(|t| object_width(db, t)),
        NodeKind::Expr(ExprKind::BitSelect { .. }) => Some(1),
        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
            match (const_i128(db, *left), const_i128(db, *right)) {
                (Some(l), Some(r)) => Some(((l - r).abs() + 1) as u32),
                _ => None,
            }
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect { width_expr, .. }) => {
            expr_width(db, *width_expr)
        }
        NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty.width,
        NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => op_width(db, *op, operands),
        _ => None,
    }
}

/// Width of an operation result from its operands.
fn op_width(db: &Db, op: i32, operands: &[NodeId]) -> Option<u32> {
    use vpi::{
        vpiAddOp, vpiArithLShiftOp, vpiArithRShiftOp, vpiBitAndOp, vpiBitNegOp, vpiBitOrOp,
        vpiBitXNorOp, vpiBitXorOp, vpiCaseEqOp, vpiCaseNeqOp, vpiConcatOp, vpiConditionOp,
        vpiDivOp, vpiEqOp, vpiGeOp, vpiGtOp, vpiLShiftOp, vpiLeOp, vpiLogAndOp, vpiLogOrOp,
        vpiLtOp, vpiMinusOp, vpiModOp, vpiMultOp, vpiMultiConcatOp, vpiNeqOp, vpiNotOp, vpiPlusOp,
        vpiPowerOp, vpiRShiftOp, vpiSubOp, vpiUnaryAndOp, vpiUnaryNandOp, vpiUnaryNorOp,
        vpiUnaryOrOp, vpiUnaryXNorOp, vpiUnaryXorOp, vpiWildEqOp, vpiWildNeqOp,
    };
    // Self-determined arithmetic / bitwise ops: max operand width.
    if matches!(
        op,
        vpiAddOp
            | vpiSubOp
            | vpiMultOp
            | vpiDivOp
            | vpiModOp
            | vpiMinusOp
            | vpiPlusOp
            | vpiBitNegOp
            | vpiBitAndOp
            | vpiBitOrOp
            | vpiBitXorOp
            | vpiBitXNorOp
            | vpiPowerOp
    ) {
        return operands.iter().filter_map(|o| expr_width(db, *o)).max();
    }
    // Shifts: width of the left operand (the shift amount is self-determined).
    if matches!(
        op,
        vpiLShiftOp | vpiRShiftOp | vpiArithLShiftOp | vpiArithRShiftOp
    ) {
        return operands.first().and_then(|o| expr_width(db, *o));
    }
    // Comparisons, logical and reduction ops produce exactly one bit.
    if matches!(
        op,
        vpiEqOp
            | vpiNeqOp
            | vpiCaseEqOp
            | vpiCaseNeqOp
            | vpiWildEqOp
            | vpiWildNeqOp
            | vpiGtOp
            | vpiGeOp
            | vpiLtOp
            | vpiLeOp
            | vpiLogAndOp
            | vpiLogOrOp
            | vpiNotOp
            | vpiUnaryAndOp
            | vpiUnaryNandOp
            | vpiUnaryOrOp
            | vpiUnaryNorOp
            | vpiUnaryXorOp
            | vpiUnaryXNorOp
    ) {
        return Some(1);
    }
    if op == vpiConcatOp {
        // Sum of operand widths; unknown operand → unknown total.
        return operands
            .iter()
            .map(|o| expr_width(db, *o))
            .sum::<Option<u32>>();
    }
    if op == vpiMultiConcatOp {
        // Replication: first operand is the repeat count, the rest are parts.
        return match (
            operands.first().and_then(|o| expr_width(db, *o)),
            operands
                .iter()
                .skip(1)
                .map(|o| expr_width(db, *o))
                .sum::<Option<u32>>(),
        ) {
            (Some(count), Some(parts)) => Some(count * parts),
            _ => None,
        };
    }
    if op == vpiConditionOp {
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
        NodeKind::Expr(ExprKind::Constant { value, .. }) => match value {
            ValueData::Int(v) => Some(*v as i128),
            ValueData::UInt(v) => Some(*v as i128),
            ValueData::Scalar(v) => Some(*v as i128),
            _ => None,
        },
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
        _ => None,
    }
}

/// Number of bits needed to represent `v` (at least one).
fn bits_needed(v: u128) -> u32 {
    if v == 0 {
        1
    } else {
        (128 - v.leading_zeros()) as u32
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
    walk_writes(db, root, &mut seen, &mut out);
    out
}

/// Every signal actively driven in the tree rooted at `root`.
///
/// This has the same whole-object behavior as [`collect_writes`], including
/// selected LHS expressions, but deliberately excludes `release` and
/// `deassign`: those statements cancel a driver rather than establish one.
pub fn collect_driver_writes(db: &Db, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_driver_writes(db, root, &mut seen, &mut out);
    out
}

fn walk_reads(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::Assign { .. })
        | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
        | NodeKind::Stmt(StmtKind::Force { .. })
        | NodeKind::ContAssign { .. } => {
            if let Some(rhs) = db.node(node).children.get(1) {
                walk_reads(db, *rhs, seen, out);
            }
            if let Some(lhs) = db.node(node).children.get(0) {
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
    match db.node_kind(node) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => {
            if seen.insert(node) {
                out.push(node);
            }
        }
        NodeKind::Expr(ExprKind::Ref { target }) => {
            if let Some(t) = target {
                if is_signal(db, *t) && seen.insert(*t) {
                    out.push(*t);
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
                if seen.insert(*t) {
                    out.push(*t);
                }
            }
        }
        _ => {}
    }
}

fn walk_writes(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::Assign { .. })
        | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
        | NodeKind::Stmt(StmtKind::Force { .. })
        | NodeKind::Stmt(StmtKind::Release { .. })
        | NodeKind::Stmt(StmtKind::Deassign { .. })
        | NodeKind::ContAssign { .. } => {
            if let Some(lhs) = db.node(node).children.get(0) {
                add_lhs_write(db, *lhs, seen, out);
            }
            return;
        }
        _ => {}
    }
    for c in &db.node(node).children {
        walk_writes(db, *c, seen, out);
    }
}

fn walk_driver_writes(db: &Db, node: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    match db.node_kind(node) {
        NodeKind::Stmt(StmtKind::Assign { .. })
        | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
        | NodeKind::Stmt(StmtKind::Force { .. })
        | NodeKind::ContAssign { .. } => {
            if let Some(lhs) = db.node(node).children.first() {
                add_driver_lhs_write(db, *lhs, seen, out);
            }
            return;
        }
        NodeKind::Stmt(StmtKind::Release { .. }) | NodeKind::Stmt(StmtKind::Deassign { .. }) => {
            return;
        }
        _ => {}
    }
    for c in &db.node(node).children {
        walk_driver_writes(db, *c, seen, out);
    }
}

fn add_lhs_write(db: &Db, lhs: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
    if let Some(sig) = signal_of_ref(db, lhs) {
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
    if let Some(sig) = driver_signal_of_lhs(db, lhs) {
        if seen.insert(sig) {
            out.push(sig);
        }
    }
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
    for top in &db.tops {
        walk(db, *top, &mut out);
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
                NodeKind::ModuleInst { .. } => walk_scopes(db, *c, out),
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
    for top in &db.tops {
        walk_scopes(db, *top, &mut out);
    }
    out.into_iter()
}

/// Display path of a scope (module instance or gen scope), e.g. `"top.u0"` or
/// `"top.genblk[0]"`.  The top instance yields its own name (unlike
/// [`Db::instance_path`], which returns `""` for the top).
pub fn scope_path(db: &Db, id: NodeId) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = Some(id);
    while let Some(nid) = cur {
        let node = db.node(nid);
        if matches!(
            node.kind,
            NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
        ) {
            let name = strip_lib(&node.name);
            if !name.is_empty() {
                parts.push(name.to_string());
            }
        }
        cur = node.parent;
    }
    parts.reverse();
    parts.join(".")
}

/// Arena nodes of Surelog's SYNTHESIZED per-port COPY interface instances.
///
/// For every interface-typed port of an instance, Surelog clones the whole
/// interface into the child module as one synthetic interface instance per
/// port (named after the port).  These copies are implementation views, not
/// user-written instantiation sites: their ports carry no parent-side
/// connection by construction.  Identification starts exactly like
/// `sim::codegen::Codegen::collect_iface_copies`: a copy is the `low`
/// connection target of some port — either directly (bare interface port) or
/// through the copy's modport whose parent is the copy instance — reachable
/// from any top through child instances and generate scopes.
///
/// Surelog v1.86 additionally emits an UNWIRED twin clone next to the wired
/// one for bare-interface ports (verified empirically; the twin is referenced
/// by nothing).  Those twins are caught conservatively: a sibling interface
/// instance sharing a wired copy's (parent, name) can only be another clone
/// of the same port view — SystemVerilog forbids two same-named instances in
/// one scope, so a user-written instance can never match.
pub fn iface_copy_instances(db: &Db) -> HashSet<NodeId> {
    /// Interface instances grouped by (parent scope, raw name).
    fn collect_iface_groups(
        db: &Db,
        inst: NodeId,
        groups: &mut HashMap<(NodeId, String), Vec<NodeId>>,
    ) {
        for c in &db.node(inst).children {
            match db.node_kind(*c) {
                NodeKind::ModuleInst {
                    is_interface: true, ..
                } => {
                    groups
                        .entry((inst, db.node(*c).name.clone()))
                        .or_default()
                        .push(*c);
                    collect_iface_groups(db, *c, groups);
                }
                NodeKind::ModuleInst { .. } => collect_iface_groups(db, *c, groups),
                NodeKind::GenScopeArray => {
                    for gs in &db.node(*c).children {
                        if matches!(db.node_kind(*gs), NodeKind::GenScope) {
                            collect_iface_groups(db, *gs, groups);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The `low`-reachable wired copies (mirrors codegen's collector).
    fn walk_wired(db: &Db, inst: NodeId, out: &mut HashSet<NodeId>) {
        for c in &db.node(inst).children {
            match db.node_kind(*c) {
                NodeKind::Port { low, .. } => {
                    if let Some(l) = low {
                        match db.node_kind(*l) {
                            NodeKind::ModPort => {
                                if let Some(iface) = db.node(*l).parent {
                                    if matches!(
                                        db.node_kind(iface),
                                        NodeKind::ModuleInst {
                                            is_interface: true,
                                            ..
                                        }
                                    ) {
                                        out.insert(iface);
                                    }
                                }
                            }
                            NodeKind::ModuleInst {
                                is_interface: true, ..
                            } => {
                                out.insert(*l);
                            }
                            _ => {}
                        }
                    }
                }
                NodeKind::ModuleInst { .. } => walk_wired(db, *c, out),
                NodeKind::GenScopeArray => {
                    for gs in &db.node(*c).children {
                        if matches!(db.node_kind(*gs), NodeKind::GenScope) {
                            walk_wired(db, *gs, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut wired = HashSet::new();
    let mut groups = HashMap::new();
    for top in &db.tops {
        walk_wired(db, *top, &mut wired);
        collect_iface_groups(db, *top, &mut groups);
    }
    let mut out = wired;
    for (_scope, members) in groups {
        if members.iter().any(|m| out.contains(m)) {
            out.extend(members);
        }
    }
    out
}

/// Display path of the scope declaring `sig` (its parent scope); empty when
/// the signal has no declaring scope.
pub fn signal_scope_path(db: &Db, sig: NodeId) -> String {
    db.node(sig)
        .parent
        .map(|p| scope_path(db, p))
        .unwrap_or_default()
}

/// Strip the `lib@` prefix Surelog puts on library-qualified names.
pub(crate) fn strip_lib(s: &str) -> &str {
    s.split_once('@').map(|(_, rest)| rest).unwrap_or(s)
}

/// Every signal connected to a port (as `vpiHighConn` or `vpiLowConn`).
///
/// Port-connected signals are exempt from the unused-signal rule: the port
/// binding is the use.
pub fn port_connected_signals(db: &Db) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    for (id, _) in iter_instances(db) {
        for c in &db.node(id).children {
            if let NodeKind::Port { high, low, .. } = db.node_kind(*c) {
                out.extend(high.iter().copied());
                out.extend(low.iter().copied());
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

/// Driver counts contributed by port links: an input port's low connection
/// (child-side signal) is driven by the parent, an output port's high
/// connection (parent-side signal) is driven by the child, and inout ports
/// drive both sides.
pub fn port_link_drivers(db: &Db) -> HashMap<NodeId, u32> {
    let mut out: HashMap<NodeId, u32> = HashMap::new();
    for (id, _) in iter_instances(db) {
        for c in &db.node(id).children {
            let NodeKind::Port {
                direction,
                high,
                low,
                ..
            } = db.node_kind(*c)
            else {
                continue;
            };
            let (a, b) = match direction {
                Direction::Input => (*low, None),
                Direction::Output => (*high, None),
                Direction::Inout => {
                    if low == high {
                        (*low, None)
                    } else {
                        (*low, *high)
                    }
                }
                Direction::None => (None, None),
            };
            for side in [a, b] {
                if let Some(sig) = side {
                    *out.entry(sig).or_insert(0) += 1;
                }
            }
        }
    }
    out
}

/// Signals driven by a connected port link, without counting top-level
/// external ports or ports already classified as unconnected.
///
/// This set intentionally has different semantics from [`port_link_drivers`],
/// whose historical counts are used by `multi-driver` and include every
/// resolved side.  It is the active-flow view needed by `undriven-signal`.
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
                    if let Some(sig) = low {
                        out.insert(*sig);
                    }
                }
                Direction::Output => {
                    if let Some(sig) = high {
                        out.insert(*sig);
                    }
                }
                Direction::Inout => {
                    if let Some(sig) = low {
                        out.insert(*sig);
                    }
                    if let Some(sig) = high {
                        out.insert(*sig);
                    }
                }
                Direction::None => {}
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
                    if let Some(sig) = low {
                        out.insert(*sig);
                    }
                }
                Direction::Inout => {
                    if let Some(sig) = low {
                        out.insert(*sig);
                    }
                    if let Some(expr) = high_expr {
                        out.extend(collect_reads(db, *expr));
                    }
                    if let Some(sig) = high {
                        out.insert(*sig);
                    }
                }
                Direction::None => {}
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
                if let Some(sig) = low {
                    out.insert(*sig);
                }
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
            if let Some(sig) = low {
                out.insert(*sig);
            }
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
            if matches!(term.direction, vpi::vpiInput | vpi::vpiInout) {
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
            if !matches!(term.direction, vpi::vpiOutput | vpi::vpiInout) {
                continue;
            }
            if let Some(sig) = driver_signal_of_lhs(db, term.expr) {
                out.insert(sig);
            }
        }
    }
    out
}

/// True for `always_comb` / `always @*` processes (combinational).  Latches
/// (`always_latch`) and explicit event controls are not combinational.
pub fn is_comb_process(db: &Db, id: NodeId) -> bool {
    if let NodeKind::Process {
        kind: crate::core::db::ProcessKind::Always { always_type },
    } = db.node_kind(id)
    {
        if *always_type == vpi::vpiAlwaysComb {
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
        if *always_type == vpi::vpiAlwaysComb || *always_type == vpi::vpiAlwaysLatch {
            return true;
        }
    }
    has_implicit_event(db, id)
}

/// True when the tree rooted at `root` contains an `@*` event control
/// (`EventControl { implicit: true }`).
pub fn has_implicit_event(db: &Db, root: NodeId) -> bool {
    match db.node_kind(root) {
        NodeKind::Stmt(StmtKind::EventControl { implicit: true, .. }) => return true,
        _ => {}
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
        NodeKind::Stmt(StmtKind::DelayControl { .. }) => return true,
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
        for top in &db.tops {
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
    fn collect_writes_finds_assign_and_cont_assign_lhs() {
        let db = db_of(
            "module t; logic clk, a, b; always @(posedge clk) a <= b; assign b = a; endmodule",
        );
        let a = find_signal(&db, "a");
        let b = find_signal(&db, "b");
        // The process writes only `a`.
        let proc = db
            .tops
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
            .tops
            .iter()
            .flat_map(|t| db.node(*t).children.clone())
            .find(|c| matches!(db.node_kind(*c), NodeKind::Process { .. }))
            .expect("a process");
        let reads = collect_reads(&db, proc);
        assert!(reads.contains(&clk), "posedge clk is a read");
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
        assert_eq!(insts[0].0, db.tops[0], "top first");
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
            !drivers.contains_key(&in_a),
            "input port must not drive the parent net"
        );
        assert!(
            drivers.contains_key(&out_b),
            "output port drives the parent net"
        );
        assert_eq!(drivers[&out_b], 1);
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
        db.tops
            .iter()
            .flat_map(|t| db.node(*t).children.clone())
            .filter(|c| matches!(db.node_kind(*c), NodeKind::ContAssign { .. }))
            .collect()
    }
}
