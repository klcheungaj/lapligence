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
//!   64 bits as a conservative optimization-cost guard; wider operations stay
//!   symbolic for the arbitrary-width runtime. Calls are never folded through.
//!   Real arithmetic folds only when both operands are real literals. Unsized
//!   fill markers are ignored while folding — the runtime ops see concrete
//!   values only (fill affects assignment conversions, which stay untouched).
//! - `identities`: algebraic identities applied to a bounded fixpoint
//!   together with folding (folding exposes identities and vice versa).
//! - `prune_branches`: statements whose conditions are constants collapse to
//!   the taken branch, under the runtime truthiness contract (a known one bit
//!   is true; an otherwise X/Z-ambiguous value is false).
//!   A `wait (cond)` with a false or unknown constant stays suspended; it
//!   must not become a fall-through path. Control-flow labels
//!   whose only jump was pruned away are stripped afterwards.
//! - `unused_storage`: signals that are neither read nor written anywhere get
//!   `omit = true`; the emitter skips their declarations.  Indices are never
//!   remapped.

use std::collections::HashSet;

use crate::core::elab::{self, Bit, Value};
use crate::sim::execution::{ExecutionModel, ExecutionProcess, TriggerPlan};
use crate::sim::ir::{
    IrBinOp, IrCallArg, IrCaseKind, IrConst, IrDependency, IrExpr, IrExprKind, IrFormal,
    IrInsideItem, IrLhs, IrModel, IrPreFn, IrRealBinOp, IrRealUnOp, IrStmt, IrStochasticStmt,
    IrStreamSelector, IrStreamTarget, IrSysFunc, IrUnOp, IrWaitSrc,
};

mod constants;
use constants::{as_packed_const, real_of, value_to_const};
mod traversal;
use traversal::{walk_execution_exprs_mut, walk_lhs_mut, walk_model_exprs_mut};
mod folding;
use folding::fold_expr;
mod identities;
use identities::ident_expr;
mod control_flow;
#[cfg(test)]
use control_flow::truthy_const;
use control_flow::{prune_stmt_list, strip_unreferenced_labels};
mod storage;
use storage::mark_unused_storage;

/// Run the enabled passes over `model` in a fixed order.
#[cfg(test)]
pub(crate) fn run_ir(model: &mut IrModel, cfg: &OptConfig) {
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
        prune_model_control(model);
    }
    if cfg.unused_storage {
        mark_unused_storage(model, None);
    }
}

/// Optimize executable blocks in place. No block is copied through or rebuilt
/// from the staging process shape, so distinct resume blocks remain distinct.
pub(crate) fn run_execution(model: &mut ExecutionModel, cfg: &OptConfig) {
    let (ir, processes) = model.optimization_parts();
    if cfg.fold_constants || cfg.identities {
        let rounds = if cfg.identities { 2 } else { 1 };
        for _ in 0..rounds {
            if cfg.fold_constants {
                walk_model_exprs_mut(ir, &mut fold_expr);
                walk_execution_exprs_mut(processes, &mut fold_expr);
            }
            if cfg.identities {
                for _ in 0..2 {
                    walk_model_exprs_mut(ir, &mut ident_expr);
                    walk_execution_exprs_mut(processes, &mut ident_expr);
                }
            }
        }
    }
    if cfg.prune_branches {
        prune_model_control(ir);
        for process in processes.iter_mut() {
            for block in &mut process.blocks {
                prune_stmt_list(&mut block.operations);
                strip_unreferenced_labels(&mut block.operations);
            }
        }
    }
    if cfg.unused_storage {
        mark_unused_storage(ir, Some(processes));
    }
}

fn prune_model_control(model: &mut IrModel) {
    for process in &mut model.processes {
        prune_stmt_list(&mut process.body);
        for pre in &mut process.pre_fns {
            if let IrPreFn::Branch { body, .. }
            | IrPreFn::CapturedBranch { body, .. }
            | IrPreFn::DeferredAssertion { body, .. } = pre
            {
                prune_stmt_list(body);
            }
        }
        // Pruning can drop the branch carrying the only `goto` to a
        // control-flow label; strip the orphans so the emitted C stays
        // `-Wall` clean.
        strip_unreferenced_labels(&mut process.body);
        for pre in &mut process.pre_fns {
            if let IrPreFn::Branch { body, .. }
            | IrPreFn::CapturedBranch { body, .. }
            | IrPreFn::DeferredAssertion { body, .. } = pre
            {
                strip_unreferenced_labels(body);
            }
        }
    }
    for function in &mut model.funcs {
        prune_stmt_list(&mut function.body);
        for pre in &mut function.pre_fns {
            if let IrPreFn::Branch { body, .. }
            | IrPreFn::CapturedBranch { body, .. }
            | IrPreFn::DeferredAssertion { body, .. } = pre
            {
                prune_stmt_list(body);
            }
        }
        strip_unreferenced_labels(&mut function.body);
        for pre in &mut function.pre_fns {
            if let IrPreFn::Branch { body, .. }
            | IrPreFn::CapturedBranch { body, .. }
            | IrPreFn::DeferredAssertion { body, .. } = pre
            {
                strip_unreferenced_labels(body);
            }
        }
    }
}

#[cfg(test)]
fn run(model: &mut IrModel, cfg: &OptConfig) {
    run_ir(model, cfg);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
