//! Concurrent assertion lowering.
//!
//! H20 intentionally starts with the small, deterministic subset that has a
//! single sampled clock and simple packed expressions on either side of an
//! implication.  The owned assertion graph still retains every Slang
//! property/sequence node; forms outside this subset fail closed here rather
//! than becoming an untimed immediate assertion.

use super::*;
use crate::core::db::{AssertionBinaryOp, AssertionExprKind, ConcurrentAssertionKind, EventSpec};
use crate::sim::ir::{
    IrAssertion, IrConcurrentAssertionKind, IrExprKind, IrProcess, IrSampledDomain, IrShape,
};

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
        let sampled_clock = SampledClock {
            signal: parts.clock_signal,
            posedge: parts.posedge,
            gate: None,
        };
        let previous_clock = self.sampled_clock;
        self.sampled_clock = Some(sampled_clock);
        let actions = (|| {
            let pass = self.lower_assertion_action(inst, path, assertion, "pass", if_true)?;
            let fail = self.lower_assertion_action(inst, path, assertion, "fail", if_false)?;
            Ok::<_, String>((pass, fail))
        })();
        self.sampled_clock = previous_clock;
        let (pass_action, fail_action) = actions?;
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
        let previous_clock = self.sampled_clock;
        self.sampled_clock = Some(SampledClock {
            signal: clock_signal,
            posedge,
            gate: None,
        });
        let expressions = (|| {
            let antecedent = antecedent_node
                .map(|node| self.lower_simple_assertion_expr(path, node, "antecedent"))
                .transpose()?;
            let consequent =
                self.lower_simple_assertion_expr(path, consequent_node, "consequent")?;
            Ok::<_, String>((antecedent, consequent))
        })();
        self.sampled_clock = previous_clock;
        let (antecedent, consequent) = expressions?;
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

    /// Lower a sampled-value clocking event. Only a direct signal event is
    /// admitted; event lists, named events, and opaque timing controls remain
    /// explicit unsupported input rather than being guessed as a clock.
    pub(super) fn lower_sampled_clock_event(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        let NodeKind::Expr(ExprKind::ClockingEvent {
            signal,
            posedge,
            gate,
        }) = self.kind(node)
        else {
            return Err(format!(
                "sampled-value clock must be a direct `@(posedge/negedge signal)` event in `{path}`"
            ));
        };
        let signal = self.lower_assertion_signal(path, *signal, "sampled clock")?;
        Ok(SampledClock {
            signal,
            posedge: *posedge,
            gate: *gate,
        })
    }

    /// Resolve the single global clocking block used by the 2009 global
    /// sampled-value functions. Ambiguous or non-edge clocks fail closed.
    pub(super) fn lower_global_sampled_clock(
        &mut self,
        path: &str,
    ) -> Result<SampledClock, String> {
        let blocks = self
            .db
            .node_ids()
            .filter_map(|id| {
                self.db
                    .clocking_block(id)
                    .filter(|info| info.is_global)
                    .filter(|_| self.node(id).parent == Some(self.inst))
                    .map(|info| (id, info))
            })
            .collect::<Vec<_>>();
        let [(_, block)] = blocks.as_slice() else {
            return Err(format!(
                "global sampled-value function requires exactly one global clocking block in `{path}`"
            ));
        };
        let Some(spec) = block.event_specs.first() else {
            return Err(format!("global clocking block has no event in `{path}`"));
        };
        let (event, posedge, gate) = flatten_clock_spec(spec).ok_or_else(|| {
            format!("global clocking block must use one direct edge event in `{path}`")
        })?;
        let signal = self.lower_assertion_signal(path, event, "global sampled clock")?;
        Ok(SampledClock {
            signal,
            posedge,
            gate,
        })
    }

    /// Resolve the default clocking block for the current elaborated
    /// instance. Slang attaches a default clocking declaration to its
    /// containing instance, so using that owner keeps identically named
    /// blocks in sibling instances independent.
    pub(super) fn lower_default_sampled_clock(
        &mut self,
        path: &str,
    ) -> Result<Option<SampledClock>, String> {
        let blocks = self
            .db
            .node_ids()
            .filter_map(|id| {
                self.db
                    .clocking_block(id)
                    .filter(|info| info.is_default)
                    .filter(|_| self.node(id).parent == Some(self.inst))
                    .map(|info| (id, info))
            })
            .collect::<Vec<_>>();
        let block = match blocks.as_slice() {
            [] => return Ok(None),
            [(_, block)] => *block,
            _ => {
                return Err(format!(
                    "multiple default clocking blocks are visible in `{path}`"
                ))
            }
        };
        let Some(spec) = block.event_specs.first() else {
            return Err(format!("default clocking block has no event in `{path}`"));
        };
        let Some((event, posedge, gate)) = flatten_clock_spec(spec) else {
            return Err(format!(
                "default clocking block must use one direct edge event in `{path}`"
            ));
        };
        let signal = self.lower_assertion_signal(path, event, "default sampled clock")?;
        Ok(Some(SampledClock {
            signal,
            posedge,
            gate,
        }))
    }

    /// Infer a sampled-value clock from one direct event-control spec. A
    /// process may have an event list or a non-edge sensitivity; those forms
    /// remain valid process controls but cannot identify one history domain.
    pub(super) fn lower_sampled_clock_spec(
        &mut self,
        path: &str,
        specs: &[EventSpec],
    ) -> Result<Option<SampledClock>, String> {
        let [spec] = specs else {
            return Ok(None);
        };
        let Some((event, posedge, gate)) = flatten_clock_spec(spec) else {
            return Ok(None);
        };
        let signal = self.lower_assertion_signal(path, event, "inferred sampled clock")?;
        Ok(Some(SampledClock {
            signal,
            posedge,
            gate,
        }))
    }

    pub(super) fn lower_sampled_domain(
        &mut self,
        path: &str,
        clock: SampledClock,
        argument: IrExpr,
        gate: Option<IrExpr>,
    ) -> Result<usize, String> {
        if argument.is_real() {
            return Err(format!("sampled-value argument must be packed in `{path}`"));
        }
        let domain = self.model.sampled_domains.len();
        self.model.sampled_domains.push(IrSampledDomain::new(
            clock.signal,
            clock.posedge,
            gate,
            argument,
        ));
        Ok(domain)
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

pub(super) fn sampled_compatible(expression: &IrExpr) -> bool {
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
        IrExprKind::SysFunc(crate::sim::ir::IrSysFunc::Sampled(call)) => {
            sampled_compatible(&call.argument)
        }
        _ => false,
    }
}

fn flatten_clock_spec(spec: &EventSpec) -> Option<(NodeId, bool, Option<NodeId>)> {
    match spec {
        EventSpec::Edge { sig, posedge } => Some((*sig, *posedge, None)),
        EventSpec::Qualified { event, condition } => {
            let (sig, posedge, _) = flatten_clock_spec(event)?;
            Some((sig, posedge, Some(*condition)))
        }
        EventSpec::AnyChange { .. } | EventSpec::Named(_) => None,
    }
}
