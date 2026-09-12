//! Concurrent assertion lowering.
//!
//! H20 intentionally starts with the small, deterministic subset that has a
//! single sampled clock and simple packed expressions on either side of an
//! implication.  The owned assertion graph still retains every Slang
//! property/sequence node; forms outside this subset fail closed here rather
//! than becoming an untimed immediate assertion.

use super::*;
use crate::core::db::{AssertionBinaryOp, AssertionExprKind, ConcurrentAssertionKind};
use crate::sim::ir::{IrAssertion, IrConcurrentAssertionKind, IrExprKind, IrProcess, IrShape};

struct PropertyParts {
    clock_signal: usize,
    posedge: bool,
    disable_signal: Option<usize>,
    antecedent: Option<IrExpr>,
    consequent: IrExpr,
    overlapped: bool,
}

impl Codegen<'_> {
    pub(super) fn emit_concurrent_assertion(
        &mut self,
        inst: NodeId,
        path: &str,
        assertion: NodeId,
    ) -> Result<(), String> {
        self.inst = inst;
        let (kind, property, if_true, if_false, label) = match self.kind(assertion) {
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind,
                property,
                if_true,
                if_false,
                label,
            }) => (*kind, *property, *if_true, *if_false, label.clone()),
            other => return Err(format!("node is not a concurrent assertion: {other:?}")),
        };
        let location = self.source_location(assertion);
        let parts = self.lower_property(path, property)?;
        let pass_action = self.lower_assertion_action(inst, path, assertion, "pass", if_true)?;
        let fail_action = self.lower_assertion_action(inst, path, assertion, "fail", if_false)?;
        let kind = match kind {
            ConcurrentAssertionKind::Assert => IrConcurrentAssertionKind::Assert,
            ConcurrentAssertionKind::Assume => IrConcurrentAssertionKind::Assume,
            ConcurrentAssertionKind::Cover => IrConcurrentAssertionKind::Cover,
        };
        self.model.assertions.push(IrAssertion::new(
            assertion.index() as u64,
            label,
            location,
            kind,
            parts.clock_signal,
            parts.posedge,
            parts.disable_signal,
            parts.antecedent,
            parts.consequent,
            parts.overlapped,
            pass_action,
            fail_action,
        ));
        Ok(())
    }

    fn lower_property(&mut self, path: &str, root: NodeId) -> Result<PropertyParts, String> {
        let mut current = root;
        let mut clock = None;
        let mut disable = None;
        loop {
            match self.kind(current) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    signal,
                    posedge,
                    expr,
                    ..
                }) => {
                    if clock.replace((*signal, *posedge)).is_some() {
                        return Err(format!(
                            "multiple clocks in concurrent assertion at {}",
                            self.source_location(current)
                        ));
                    }
                    current = *expr;
                }
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff {
                    condition, expr, ..
                }) => {
                    if disable.replace(*condition).is_some() {
                        return Err(format!(
                            "multiple disable iff conditions in concurrent assertion at {}",
                            self.source_location(current)
                        ));
                    }
                    current = *expr;
                }
                _ => break,
            }
        }
        if matches!(
            self.kind(current),
            NodeKind::AssertionExpr(AssertionExprKind::Simple { expr, .. })
                if matches!(self.kind(*expr), NodeKind::Expr(ExprKind::AssertionInstance { .. }))
        ) {
            return Err(format!(
                "assertion instances with formal bindings are not supported at {path}"
            ));
        }
        let (clock_node, posedge) = clock.ok_or_else(|| {
            format!("concurrent assertions require one explicit signal clock at {path}")
        })?;
        let clock_signal = self.lower_assertion_signal(path, clock_node, "clock")?;
        let disable_signal = disable
            .map(|condition| self.lower_assertion_signal(path, condition, "disable iff"))
            .transpose()?;
        let (antecedent_node, consequent_node, overlapped) = match self.kind(current) {
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op: AssertionBinaryOp::OverlappedImplication,
                left,
                right,
            }) => (Some(*left), *right, true),
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op: AssertionBinaryOp::NonOverlappedImplication,
                left,
                right,
            }) => (Some(*left), *right, false),
            NodeKind::AssertionExpr(AssertionExprKind::Simple { .. }) => (None, current, true),
            _ => {
                return Err(format!(
                    "unsupported concurrent assertion property at {} (only simple |->/|=> forms are supported)",
                    self.source_location(current)
                ))
            }
        };
        let antecedent = antecedent_node
            .map(|node| self.lower_simple_assertion_expr(path, node, "antecedent"))
            .transpose()?;
        let consequent = self.lower_simple_assertion_expr(path, consequent_node, "consequent")?;
        for (name, expression) in [
            ("antecedent", antecedent.as_ref()),
            ("consequent", Some(&consequent)),
        ] {
            if let Some(expression) = expression {
                if expression.is_real() || !sampled_compatible(expression) {
                    return Err(format!(
                        "unsupported sampled {name} expression in concurrent assertion at {path}"
                    ));
                }
            }
        }
        Ok(PropertyParts {
            clock_signal,
            posedge,
            disable_signal,
            antecedent,
            consequent,
            overlapped,
        })
    }

    fn lower_assertion_signal(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<usize, String> {
        let expression = self.lower_expr(path, node)?;
        let IrExprKind::SigRead(signal) = expression.kind() else {
            return Err(format!(
                "concurrent assertion {role} must be a direct packed signal at {path}"
            ));
        };
        let Some(IrSignal {
            ty: IrType::Packed { .. },
            omit: false,
            ..
        }) = self.model.signals.get(*signal)
        else {
            return Err(format!(
                "concurrent assertion {role} signal is not active packed storage at {path}"
            ));
        };
        Ok(*signal)
    }

    fn lower_simple_assertion_expr(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<IrExpr, String> {
        let NodeKind::AssertionExpr(AssertionExprKind::Simple { expr, repeated }) = self.kind(node)
        else {
            return Err(format!(
                "concurrent assertion {role} must be a simple sequence at {path}"
            ));
        };
        if *repeated {
            return Err(format!(
                "sequence repetition is not supported in concurrent assertion {role} at {path}"
            ));
        }
        self.lower_boolean_expr(path, *expr)
    }

    fn lower_assertion_action(
        &mut self,
        inst: NodeId,
        path: &str,
        assertion: NodeId,
        arm: &str,
        statement: Option<NodeId>,
    ) -> Result<Option<String>, String> {
        let Some(statement) = statement else {
            return Ok(None);
        };
        let action_path = format!("{path}.assertion[{}].{arm}", assertion.index());
        let mut ctx = EmitCtx::new(self, action_path.clone(), inst, "0", None, None, false);
        let body = ctx.lower_stmt(statement)?;
        if ctx.saw_wait {
            return Err(format!(
                "timing control is not supported in concurrent assertion {arm} action at {action_path}"
            ));
        }
        let mut pre_fns = std::mem::take(&mut ctx.pre_fns);
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        let mut writes: Vec<IrDependency> = self
            .collect_process_writes(statement)?
            .into_iter()
            .collect();
        writes.sort_by_key(|dependency| self.dependency_label(dependency));
        let name = self.new_fn_name(&action_path, "assert_action");
        self.model
            .processes
            .push(IrProcess::new_with_kind_and_writes(
                name.clone(),
                action_path,
                IrProcessKind::Synthetic,
                IrShape::RunOnce,
                writes,
                pre_fns,
                body,
                self.origin(assertion),
            ));
        self.assertion_action_procs.insert(name.clone());
        Ok(Some(name))
    }
}

fn sampled_compatible(expression: &IrExpr) -> bool {
    match expression.kind() {
        IrExprKind::Const(_) | IrExprKind::SigRead(_) | IrExprKind::Fill(_) => true,
        IrExprKind::Bin { a, b, .. } => sampled_compatible(a) && sampled_compatible(b),
        IrExprKind::Un { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::ToTwoState { a } => sampled_compatible(a),
        IrExprKind::Mux { sel, a, b } => {
            sampled_compatible(sel) && sampled_compatible(a) && sampled_compatible(b)
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            parts.iter().all(sampled_compatible)
        }
        IrExprKind::Stream { value, .. } => sampled_compatible(value),
        IrExprKind::Inside { value, items } => {
            sampled_compatible(value)
                && items.iter().all(|item| match item {
                    crate::sim::ir::IrInsideItem::Value(value) => sampled_compatible(value),
                    crate::sim::ir::IrInsideItem::Range { low, high } => {
                        sampled_compatible(low) && sampled_compatible(high)
                    }
                    crate::sim::ir::IrInsideItem::OpenRange { low, high } => {
                        low.as_ref().is_none_or(sampled_compatible)
                            && high.as_ref().is_none_or(sampled_compatible)
                    }
                    crate::sim::ir::IrInsideItem::Container { .. } => false,
                })
        }
        IrExprKind::BitSel { base, idx } => sampled_compatible(base) && sampled_compatible(idx),
        IrExprKind::PartSel { base, .. } => sampled_compatible(base),
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            sampled_compatible(base)
                && sampled_compatible(base_idx)
                && sampled_compatible(width_expr)
        }
        IrExprKind::BitStreamCast { a, .. } => sampled_compatible(a),
        _ => false,
    }
}
