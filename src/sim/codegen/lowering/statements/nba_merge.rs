//! Static merge of overridden nonblocking assignments (FIN-001 O2).
//!
//! `q <= 0; if (c) q <= d;` queues two updates of `q` when `c` holds, and
//! the second overwrites the first in the same NBA batch (IEEE 1800-2009 4.6
//! b): "Nonblocking assignment updates shall be performed in the order the
//! statements were executed"). This pass removes such an overridden update
//! in lowering, so the model queues one write of the last value and pays
//! nothing at run time.
//!
//! An update `L <= a` is dropped only when all of these hold:
//!
//! - `L` names a constant packed target: a whole packed variable, a constant
//!   part or bit of one, or a constant packed member chain of one. Nets,
//!   arrays, native storage and run-time selectors stay as written.
//! - `a` is pure (constants, reads and operators): dropping it skips no
//!   effect, call or run-time stop.
//! - In the same statement list an update with the identical target `L`
//!   follows, and every statement between them is another such pure update
//!   of a constant packed target. Nothing between can suspend, jump, call or
//!   stop, so whenever the dropped update is queued the overriding one is
//!   queued later in the same process turn, and no other process runs in
//!   between.
//!
//! The kept updates stay in their queue order: each other target is still
//! written before the overriding update when it was before. What changes is
//! only the value `L` holds while the updates between commit, and the write
//! of the dropped value itself. The pass therefore does not run where a
//! consumer sees each write instead of the change at the T1-D11 check
//! (`docs/lrm_decisions.md`): per storage, `$monitor` marking and sampled
//! value history (`sampled_record_write`); per model, the per-delta dump
//! mode (`--wave-deltas`, S47-D15, which records every write), timing
//! checks, sequential UDP change logs, asynchronous `accept_on`/`reject_on`
//! conditions, gated sampled-value clocks (AA-D6), and toggle and FSM
//! coverage. Waiters, VPI value-change callbacks, `disable iff`, force
//! expressions and switch controls run at the check, which compares the
//! value with the value at the last check, so they see the same change.
//! The default dump writes a variable's end-of-step value, in registration
//! order, only when it differs from the value last written (S47-D2,
//! S47-D14), so a merged model writes the same file.
//!
//! An update directly followed by an `if` or `case` without a uniqueness
//! check, or by a plain block, moves into the front of every branch (an
//! absent `else` or default is added) when some branch begins with an
//! update that overrides it; there it merges, and the other branches queue
//! it after the condition, which is pure, in the same queue position. Its
//! value must then read no local, so the move cannot change which
//! declaration a name refers to.

use std::collections::HashSet;

use crate::sim::ir::{
    IrCaseItem, IrDependency, IrExpr, IrExprKind, IrInitStep, IrLhs, IrModel, IrSignal, IrStmt,
    IrType, IrUniquePriorityCheck,
};

/// Storage that `$monitor`/`$fmonitor` reports name, recorded where the
/// reports are lowered: marking a report dirty happens at each write.
#[derive(Debug, Default)]
pub(in crate::sim::codegen) struct MonitorReads {
    names: HashSet<String>,
    /// A report reads storage chosen at run time (a `ref` formal, a class or
    /// virtual-interface handle) or re-evaluates every slot: any storage may
    /// be marked.
    opaque: bool,
}

impl MonitorReads {
    pub(in crate::sim::codegen) fn note(&mut self, reads: &[IrDependency], every_slot: bool) {
        self.opaque |= every_slot;
        for read in reads {
            self.note_one(read);
        }
    }

    fn note_one(&mut self, read: &IrDependency) {
        match read {
            IrDependency::Scalar(name) | IrDependency::Real(name) => {
                self.names.insert(name.clone());
            }
            IrDependency::PackedRange { storage, .. } => self.note_one(storage),
            IrDependency::InterfaceInstance { arms, .. } => {
                for arm in arms {
                    self.note_one(arm);
                }
            }
            IrDependency::RefFormal { .. } | IrDependency::NativeAccess(_) => self.opaque = true,
            // Array cells, containers, strings, handles, records and
            // activation cells are never merge targets.
            IrDependency::ArrayElement { .. }
            | IrDependency::ArrayContents(_)
            | IrDependency::ContainerContents(_)
            | IrDependency::ContainerShape(_)
            | IrDependency::Object(_)
            | IrDependency::SharedCell { .. }
            | IrDependency::SharedNative { .. }
            | IrDependency::NativeRefFormal { .. }
            | IrDependency::NativeValue(_) => {}
        }
    }
}

/// Drop every overridden nonblocking update of the model's process and
/// subroutine bodies (module docs). Returns how many were dropped.
pub(in crate::sim::codegen) fn merge_overridden_nbas(
    model: &mut IrModel,
    monitors: &MonitorReads,
) -> usize {
    let Some(observed) = observed_storage(model, monitors) else {
        return 0;
    };
    let mut processes = std::mem::take(&mut model.processes);
    let mut funcs = std::mem::take(&mut model.funcs);
    let rules = MergeRules {
        signals: &model.signals,
        observed,
    };
    let mut merged = 0;
    for process in &mut processes {
        merged += rules.list(&mut process.body);
    }
    for func in &mut funcs {
        merged += rules.list(&mut func.body);
    }
    model.processes = processes;
    model.funcs = funcs;
    merged
}

/// C names of the storage a consumer sees at each write; `None` when the
/// model has a consumer that sees each write of any storage, or reads other
/// storage while a write publishes.
fn observed_storage(model: &IrModel, monitors: &MonitorReads) -> Option<HashSet<String>> {
    let coverage_watches = model
        .assertion_coverage
        .as_ref()
        .is_some_and(|coverage| !coverage.toggles.is_empty() || !coverage.fsms.is_empty());
    let async_aborts = model
        .assertions
        .iter()
        .any(|assertion| assertion.abort_condition.is_some() && !assertion.abort_sync);
    let gated_clocks = model
        .sampled_clocks
        .iter()
        .any(|clock| clock.gate.is_some())
        || model.processes.iter().any(|process| {
            process.body.iter().any(|statement| {
                matches!(
                    statement.unlocated(),
                    IrStmt::WaitEvents {
                        gate_at_write: true,
                        ..
                    }
                )
            })
        });
    if (model.waveform && model.wave_deltas)
        || !model.timing_checks.is_empty()
        || !model.udp_states.is_empty()
        || coverage_watches
        || async_aborts
        || gated_clocks
        || monitors.opaque
    {
        return None;
    }
    let mut observed = monitors.names.clone();
    for step in &model.init_steps {
        if let IrInitStep::RegisterSampled(sig)
        | IrInitStep::RegisterSampledHistory { sig, .. }
        | IrInitStep::RegisterSampledValue(sig) = step
        {
            observed.insert(model.signals[*sig].c_name.clone());
        }
    }
    Some(observed)
}

struct MergeRules<'a> {
    signals: &'a [IrSignal],
    /// C names of storage a consumer sees at each write.
    observed: HashSet<String>,
}

impl MergeRules<'_> {
    /// Merge inside nested statements first, then in `list` from its end,
    /// so a run of defaults before an `if` moves into it in source order.
    fn list(&self, list: &mut Vec<IrStmt>) -> usize {
        let mut merged = 0;
        for statement in list.iter_mut() {
            for child in child_lists(statement) {
                merged += self.list(child);
            }
        }
        for index in (0..list.len()).rev() {
            merged += self.at(list, index);
        }
        merged
    }

    /// Drop or move the update at `index` when the module rules allow it.
    fn at(&self, list: &mut Vec<IrStmt>, index: usize) -> usize {
        let Some(lhs) = self.mergeable(&list[index]) else {
            return 0;
        };
        match self.override_after(list, index, lhs) {
            Override::Found => {
                list.remove(index);
                1
            }
            Override::InBranch if movable_value(&list[index]) => {
                let update = list.remove(index);
                let mut merged = 0;
                for branch in branches_completed(&mut list[index]) {
                    branch.insert(0, update.clone());
                    merged += self.at(branch, 0);
                }
                merged
            }
            Override::InBranch | Override::None => 0,
        }
    }

    /// Whether an update with `lhs` follows the one at `index` across pure
    /// updates only, or (`InBranch`) the next statement is a branch whose
    /// arm begins with one.
    fn override_after(&self, list: &[IrStmt], index: usize, lhs: &IrLhs) -> Override {
        for statement in &list[index + 1..] {
            match nba(statement) {
                Some((other, _)) if other == lhs => return Override::Found,
                Some((other, rhs)) if constant_target(other) && pure(rhs) => continue,
                _ => break,
            }
        }
        let Some(next) = list.get(index + 1) else {
            return Override::None;
        };
        if branch_arms(next)
            .is_some_and(|arms| arms.into_iter().any(|arm| self.arm_overrides(arm, lhs)))
        {
            Override::InBranch
        } else {
            Override::None
        }
    }

    fn arm_overrides(&self, arm: &[IrStmt], lhs: &IrLhs) -> bool {
        for (position, statement) in arm.iter().enumerate() {
            match nba(statement) {
                Some((other, _)) if other == lhs => return true,
                Some((other, rhs)) if constant_target(other) && pure(rhs) => continue,
                _ => {}
            }
            return position == 0
                && branch_arms(statement).is_some_and(|arms| {
                    arms.into_iter().any(|inner| self.arm_overrides(inner, lhs))
                });
        }
        false
    }

    /// The target of a droppable update: pure value, constant packed target
    /// that no per-write consumer observes.
    fn mergeable<'s>(&self, statement: &'s IrStmt) -> Option<&'s IrLhs> {
        let (lhs, rhs) = nba(statement)?;
        if !pure(rhs) {
            return None;
        }
        let signal = target_signal(lhs)?;
        let declared = &self.signals[signal];
        let storage = &self.signals[declared.alias.unwrap_or(signal)];
        let plain = |signal: &IrSignal| {
            matches!(signal.ty, IrType::Packed { .. })
                && signal.net_driver.is_none()
                && signal.net_alias.is_empty()
        };
        (plain(declared) && plain(storage) && !self.observed.contains(&storage.c_name))
            .then_some(lhs)
    }
}

enum Override {
    Found,
    InBranch,
    None,
}

/// The target and value of an untimed nonblocking assignment.
fn nba(statement: &IrStmt) -> Option<(&IrLhs, &IrExpr)> {
    match statement.unlocated() {
        IrStmt::Assign {
            lhs,
            rhs,
            nba: true,
        } => Some((lhs, rhs)),
        _ => None,
    }
}

/// The signal a constant packed target writes.
fn target_signal(lhs: &IrLhs) -> Option<usize> {
    match lhs {
        IrLhs::Whole(signal) | IrLhs::Part(signal, ..) => Some(*signal),
        IrLhs::Bit(signal, index, _) => {
            matches!(index.kind, IrExprKind::Const(_)).then_some(*signal)
        }
        IrLhs::PackedSelect { target, steps, .. } => steps
            .iter()
            .all(|step| matches!(step.base.kind, IrExprKind::Const(_)))
            .then(|| target_signal(target))
            .flatten(),
        _ => None,
    }
}

fn constant_target(lhs: &IrLhs) -> bool {
    target_signal(lhs).is_some()
}

/// The arms of a statement an update can move into: each always runs after
/// a pure selection, and the selection reports nothing.
fn branch_arms(statement: &IrStmt) -> Option<Vec<&[IrStmt]>> {
    match statement {
        IrStmt::If {
            cond,
            then_,
            els,
            check: IrUniquePriorityCheck::None,
        } if pure(cond) => Some(
            std::iter::once(then_.as_slice())
                .chain(els.as_deref())
                .collect(),
        ),
        IrStmt::Case {
            sel,
            items,
            check: IrUniquePriorityCheck::None,
            ..
        } if pure(sel) && items.iter().flat_map(|item| &item.exprs).all(pure) => {
            Some(items.iter().map(|item| item.body.as_slice()).collect())
        }
        IrStmt::Block(body) => Some(vec![body.as_slice()]),
        _ => None,
    }
}

/// Every arm of a statement `branch_arms` accepted, adding the arm that runs
/// when no condition or item matches.
fn branches_completed(statement: &mut IrStmt) -> Vec<&mut Vec<IrStmt>> {
    match statement {
        IrStmt::If { then_, els, .. } => {
            let els = els.get_or_insert_with(Vec::new);
            vec![then_, els]
        }
        IrStmt::Case { items, .. } => {
            if !items.iter().any(|item| item.exprs.is_empty()) {
                items.push(IrCaseItem::new(Vec::new(), Vec::new()));
            }
            items.iter_mut().map(|item| &mut item.body).collect()
        }
        IrStmt::Block(body) => vec![body],
        _ => Vec::new(),
    }
}

/// The statement lists nested in `statement` that run as a sequence of
/// their own. A form not listed here keeps its updates as written.
fn child_lists(statement: &mut IrStmt) -> Vec<&mut Vec<IrStmt>> {
    match statement {
        IrStmt::Located { statement, .. } => child_lists(statement),
        IrStmt::Block(body)
        | IrStmt::While { body, .. }
        | IrStmt::Repeat { body, .. }
        | IrStmt::For { body, .. }
        | IrStmt::Forever { body }
        | IrStmt::ActivationScope { body, .. } => vec![body],
        IrStmt::If { then_, els, .. } => std::iter::once(then_).chain(els.as_mut()).collect(),
        IrStmt::Case { items, .. } => items.iter_mut().map(|item| &mut item.body).collect(),
        _ => Vec::new(),
    }
}

/// Whether the value of the update reads no local, so moving it into a
/// nested block cannot bind a name to another declaration.
fn movable_value(statement: &IrStmt) -> bool {
    nba(statement).is_some_and(|(_, rhs)| !reads_local(rhs))
}

fn reads_local(expr: &IrExpr) -> bool {
    let mut found = false;
    visit_pure(expr, &mut |expr| {
        found |= matches!(expr.kind, IrExprKind::LocalRead(_));
    });
    found
}

/// Whether evaluating `expr` has no effect and cannot stop the run.
fn pure(expr: &IrExpr) -> bool {
    visit_pure(expr, &mut |_| {})
}

/// Visit every node of a pure expression; false for any other form.
fn visit_pure(expr: &IrExpr, visit: &mut impl FnMut(&IrExpr)) -> bool {
    visit(expr);
    match &expr.kind {
        IrExprKind::Const(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_)
        | IrExprKind::Fill(_) => true,
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            visit_pure(a, visit) && visit_pure(b, visit)
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::PartSel { base: a, .. } => visit_pure(a, visit),
        IrExprKind::Mux { sel, a, b } => {
            visit_pure(sel, visit) && visit_pure(a, visit) && visit_pure(b, visit)
        }
        IrExprKind::BitSel { base, idx } => visit_pure(base, visit) && visit_pure(idx, visit),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            visit_pure(base, visit) && visit_pure(base_idx, visit) && visit_pure(width_expr, visit)
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            parts.iter().all(|part| visit_pure(part, visit))
        }
        IrExprKind::Predicate { clauses } => clauses.iter().all(|clause| visit_pure(clause, visit)),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
