//! Events.

use super::super::collection::EventEvaluation;
use super::*;

impl EmitCtx<'_, '_> {
    /// Lower `@(…)`: explicit edge/any specs become ONE atomic
    /// `llg_wait_any_events`; implicit sensitivity waits on the body's read
    /// set.
    pub(super) fn lower_event_control(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (specs, implicit, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit,
                body,
            }) => (specs, *implicit, *body),
            _ => unreachable!("non-event-control passed to lower_event_control"),
        };
        let body = body.ok_or_else(|| "event_control without body".to_string())?;
        let mut out = if !implicit && self.event_specs_need_process(specs) {
            self.lower_process_evaluated_event(h, specs)?
        } else {
            vec![self.lower_event_wait(specs, implicit, body)?]
        };
        self.saw_wait = true;
        // The body may be a `Stmt(Empty)` placeholder for a bare
        // `@(posedge clk);`; skip it.
        if !matches!(self.cg.kind(body), NodeKind::Stmt(StmtKind::Empty)) {
            // A sampled-value call without its fourth clocking argument may
            // use a process's single direct edge control. Keep this context
            // only while lowering the controlled body; nested controls save
            // and restore their own inferred domain.
            let previous_clock = self.cg.sampled_clock;
            if let Some(clock) = self
                .cg
                .lower_sampled_clock_spec(&self.path, specs)
                .ok()
                .flatten()
            {
                self.cg.sampled_clock = Some(clock);
            }
            let body_result = self.lower_stmt(body);
            self.cg.sampled_clock = previous_clock;
            out.extend(body_result?);
        }
        Ok(out)
    }

    fn lower_event_wait(
        &mut self,
        specs: &[EventSpec],
        implicit: bool,
        body: NodeId,
    ) -> Result<IrStmt, String> {
        let spec_pairs = self.lower_event_specs(specs)?;
        Ok(if implicit || spec_pairs.is_empty() {
            // @* / always_comb without explicit sensitivity, or a condition
            // that produced no specs: wait on the body's read set.
            let reads = if implicit && self.process_kind == Some(AlwaysKind::Always) {
                self.cg.collect_at_star_signals(&self.path, body)?
            } else {
                self.cg.collect_read_signals(&self.path, body)?
            };
            if implicit && self.process_kind == Some(AlwaysKind::Always) && reads.is_empty() {
                // An empty `@*` sensitivity list waits forever. Keep the
                // explicit never-triggered event as the source-level sentinel.
                let event = self.cg.model.events.len();
                self.cg
                    .model
                    .events
                    .push(crate::sim::ir::IrEvent::new(format!(
                        "E_{}_at_star_empty_{event}",
                        self.cg.c_path_ident(&self.path)
                    )));
                IrStmt::WaitEvents {
                    specs: vec![(IrWaitSrc::Event(IrEventRef::Static(event)), IrEdge::Any)],
                }
            } else {
                IrStmt::WaitAny { sens: reads }
            }
        } else {
            IrStmt::WaitEvents { specs: spec_pairs }
        })
    }

    /// Whether an explicit event control evaluates a helper that is a legal
    /// zero-time function but not a read-only runtime callback. Expressions
    /// that fail classification outright keep the ordinary path, which
    /// reports the same diagnostic.
    fn event_specs_need_process(&self, specs: &[EventSpec]) -> bool {
        specs.iter().any(|spec| match spec {
            EventSpec::Qualified { event, condition } => {
                self.event_specs_need_process(std::slice::from_ref(event))
                    || self.expression_needs_process(*condition)
            }
            EventSpec::Named(_) => false,
            EventSpec::AnyChange { sig } | EventSpec::Edge { sig, .. } => {
                self.expression_needs_process(*sig)
            }
        })
    }

    fn expression_needs_process(&self, expression: NodeId) -> bool {
        matches!(
            self.cg.classify_event_expression(expression, &self.path),
            Ok(EventEvaluation::Process(_))
        )
    }

    /// Lower `@(…)` whose expression needs process evaluation (see
    /// [`EventEvaluation`]). The waiting process evaluates every source once
    /// when the control is reached, then suspends on the union of their read
    /// sets; after each wake it re-evaluates every source and resumes only
    /// when one detects its change (SV 9.4.2: a changed operand with an
    /// unchanged result is no event; edges use the least significant bit).
    /// Qualifiers are evaluated only for a detected change. The number of
    /// helper evaluations is unspecified by the language; this form performs
    /// one per source at arm time and one per source per wake.
    fn lower_process_evaluated_event(
        &mut self,
        h: NodeId,
        specs: &[EventSpec],
    ) -> Result<Vec<IrStmt>, String> {
        fn flatten(
            spec: &EventSpec,
            condition: Option<NodeId>,
            out: &mut Vec<(NodeId, IrEdge, Option<NodeId>)>,
        ) -> Result<(), ()> {
            match spec {
                EventSpec::Qualified { event, condition } => flatten(event, Some(*condition), out),
                EventSpec::Named(_) => Err(()),
                EventSpec::AnyChange { sig } => {
                    out.push((*sig, IrEdge::Any, condition));
                    Ok(())
                }
                EventSpec::Edge { sig, posedge } => {
                    out.push((
                        *sig,
                        if *posedge {
                            IrEdge::Posedge
                        } else {
                            IrEdge::Negedge
                        },
                        condition,
                    ));
                    Ok(())
                }
            }
        }
        let path = self.cg.source_path(&self.path);
        let mut sources = Vec::new();
        for spec in specs {
            flatten(spec, None, &mut sources).map_err(|()| {
                format!(
                    "named events cannot share an event control with a process-evaluated helper expression in `{path}`"
                )
            })?;
        }
        let bit = |value: u64, x: u64, z: u64| -> Result<IrExpr, String> {
            let constant = IrConst::packed(vec![value], vec![x], vec![z], 1, false, None)
                .map_err(|error| error.to_string())?;
            Ok(IrExpr::new(IrExprKind::Const(constant), 1, false, None))
        };
        let (zero, one, unknown_x, unknown_z) =
            (bit(0, 0, 0)?, bit(1, 0, 0)?, bit(0, 1, 0)?, bit(0, 0, 1)?);
        let binary = |op: IrBinOp, a: IrExpr, b: IrExpr| {
            IrExpr::new(
                IrExprKind::Bin {
                    op,
                    a: Box::new(a),
                    b: Box::new(b),
                },
                1,
                false,
                None,
            )
        };
        let local = |name: &str, width: u32, signed: bool| {
            IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, signed, None)
        };
        let target = |name: &str, width: u32, signed: bool, two_state: bool| IrLhs::WholeRef {
            addr: format!("&{name}"),
            width,
            signed,
            two_state,
            shortreal: false,
        };
        let hit = format!("_llg_evh{}", h.0);
        let mut sens = Vec::new();
        let mut arm = Vec::new();
        let mut current = Vec::new();
        let mut detect = Vec::new();
        let mut advance = Vec::new();
        for (index, (expression, edge, condition)) in sources.into_iter().enumerate() {
            if self.cg.event_target_of(expression).is_some() {
                return Err(format!(
                    "named events cannot share an event control with a process-evaluated helper expression in `{path}`"
                ));
            }
            let value = self.cg.lower_expr(&self.path, expression)?;
            if value.is_real() {
                return Err(format!(
                    "real-valued event expressions with process-evaluated helpers are not supported in `{path}`"
                ));
            }
            let value = if edge == IrEdge::Any {
                value
            } else {
                IrExpr::convert_to(value, 1, false)
            };
            let (width, signed) = (value.width, value.signed);
            for dependency in self.cg.collect_read_signals(&self.path, expression)? {
                if !sens.contains(&dependency) {
                    sens.push(dependency);
                }
            }
            let last = format!("_llg_evl{}_{index}", h.0);
            let next = format!("_llg_evn{}_{index}", h.0);
            arm.push(IrStmt::DeclLocal {
                name: last.clone(),
                width,
                signed,
                init: Some(Box::new(value.clone())),
                two_state: false,
            });
            current.push(IrStmt::DeclLocal {
                name: next.clone(),
                width,
                signed,
                init: Some(Box::new(value)),
                two_state: false,
            });
            let (old, new) = (local(&last, width, signed), local(&next, width, signed));
            let changed = match edge {
                IrEdge::Any => binary(IrBinOp::CaseNeq, old, new),
                IrEdge::Posedge | IrEdge::Negedge => {
                    let (from, to) = if edge == IrEdge::Posedge {
                        (zero.clone(), one.clone())
                    } else {
                        (one.clone(), zero.clone())
                    };
                    let unknown = binary(
                        IrBinOp::LogOr,
                        binary(IrBinOp::CaseEq, old.clone(), unknown_x.clone()),
                        binary(IrBinOp::CaseEq, old.clone(), unknown_z.clone()),
                    );
                    binary(
                        IrBinOp::LogOr,
                        binary(
                            IrBinOp::LogAnd,
                            binary(IrBinOp::CaseEq, old, from.clone()),
                            binary(IrBinOp::CaseNeq, new.clone(), from),
                        ),
                        binary(IrBinOp::LogAnd, unknown, binary(IrBinOp::CaseEq, new, to)),
                    )
                }
            };
            let found = IrStmt::Assign {
                lhs: target(&hit, 1, false, true),
                rhs: one.clone(),
                nba: false,
            };
            let then_ = match condition {
                Some(condition) => vec![IrStmt::If {
                    cond: self.cg.lower_expr(&self.path, condition)?,
                    then_: vec![found],
                    els: None,
                    check: IrUniquePriorityCheck::None,
                }],
                None => vec![found],
            };
            detect.push(IrStmt::If {
                cond: changed,
                then_,
                els: None,
                check: IrUniquePriorityCheck::None,
            });
            advance.push(IrStmt::Assign {
                lhs: target(&last, width, signed, false),
                rhs: local(&next, width, signed),
                nba: false,
            });
        }
        arm.push(IrStmt::DeclLocal {
            name: hit.clone(),
            width: 1,
            signed: false,
            init: Some(Box::new(zero)),
            two_state: true,
        });
        let mut iteration = current;
        iteration.extend(detect);
        iteration.extend(advance);
        arm.push(IrStmt::While {
            cond: IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(local(&hit, 1, false)),
                },
                1,
                false,
                None,
            ),
            body: vec![IrStmt::WaitAny { sens }, IrStmt::Block(iteration)],
        });
        Ok(vec![IrStmt::Block(arm)])
    }

    pub(super) fn wait_body(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let mut body = Vec::new();
        if let Some(b) = self.cg.node(h).children.get(1) {
            if !matches!(self.cg.kind(*b), NodeKind::Stmt(StmtKind::Empty)) {
                body = self.lower_stmt(*b)?;
            }
        }
        Ok(body)
    }

    pub(super) fn triggered_event_ref(
        &mut self,
        mut node: NodeId,
    ) -> Result<Option<IrEventRef>, String> {
        loop {
            let operand = match self.cg.kind(node) {
                NodeKind::Expr(ExprKind::Cast { operand, .. }) => Some(*operand),
                _ => None,
            };
            if let Some(operand) = operand {
                node = operand;
                continue;
            }
            let receiver = match self.cg.kind(node) {
                NodeKind::MethodCall {
                    name,
                    receiver: Some(receiver),
                    ..
                } if name == "triggered" => Some(*receiver),
                _ => None,
            };
            let Some(receiver) = receiver else {
                return Ok(None);
            };
            let target = self.cg.event_target_of(receiver).ok_or_else(|| {
                format!(
                    "event triggered property has an unresolved receiver in `{}`",
                    self.path
                )
            })?;
            return self.cg.event_ref_of(&target, &self.path).map(Some);
        }
    }

    pub(super) fn unsupported_event_trigger_timing(
        &self,
        timing: &EventTriggerTiming,
        reason: &str,
    ) -> String {
        let control = match timing {
            EventTriggerTiming::Delay { control, .. }
            | EventTriggerTiming::Event { control, .. }
            | EventTriggerTiming::Repeat { control, .. }
            | EventTriggerTiming::Unsupported { control } => *control,
        };
        let node = self.cg.node(control);
        let file = node.file.as_deref().unwrap_or("<unknown>");
        format!(
            "{reason} at {file}:{}:{} in `{}`",
            node.line, node.col, self.path
        )
    }

    pub(super) fn lower_nonblocking_event_trigger(
        &mut self,
        statement: NodeId,
        event: IrEventRef,
        timing: Option<&EventTriggerTiming>,
    ) -> Result<Vec<IrStmt>, String> {
        let Some(timing) = timing else {
            return Ok(vec![IrStmt::NonblockingEventTrigger {
                ev: event,
                ticks: None,
            }]);
        };
        match timing {
            EventTriggerTiming::Delay { expression, .. } => {
                let ticks = self
                    .cg
                    .lower_procedural_delay(&self.path, statement, *expression)?;
                Ok(vec![IrStmt::NonblockingEventTrigger {
                    ev: event,
                    ticks: Some(ticks),
                }])
            }
            EventTriggerTiming::Event { specs, .. } => {
                Ok(vec![IrStmt::NonblockingEventTriggerWhen {
                    ev: event,
                    specs: self.lower_event_specs(specs)?,
                    repeat: None,
                }])
            }
            EventTriggerTiming::Repeat {
                count,
                event: inner,
                ..
            } => {
                let EventTriggerTiming::Event { specs, .. } = inner.as_ref() else {
                    return Err(self.unsupported_event_trigger_timing(
                        timing,
                        "repeat nonblocking event triggers require an event control",
                    ));
                };
                let repeat = self.cg.lower_expr(&self.path, *count)?;
                if repeat.is_real() {
                    return Err(self.unsupported_event_trigger_timing(
                        timing,
                        "repeat nonblocking event triggers require a packed count",
                    ));
                }
                Ok(vec![IrStmt::NonblockingEventTriggerWhen {
                    ev: event,
                    specs: self.lower_event_specs(specs)?,
                    repeat: Some(repeat),
                }])
            }
            EventTriggerTiming::Unsupported { .. } => Err(self.unsupported_event_trigger_timing(
                timing,
                "unsupported nonblocking event-trigger timing",
            )),
        }
    }

    /// Resolve the captured event specs into atomic wait sources: signal
    /// globals with edges, plus named events.  A mixed list stays ONE
    /// `WaitEvents` statement (one runtime call), so a trigger can never be
    /// lost between two separate waits.
    pub(in super::super) fn lower_event_specs(
        &mut self,
        specs: &[EventSpec],
    ) -> Result<Vec<(IrWaitSrc, IrEdge)>, String> {
        specs
            .iter()
            .map(|spec| self.lower_event_spec(spec, None))
            .collect()
    }

    fn event_evaluator(&mut self, expression: NodeId) -> Result<(String, bool), String> {
        self.cg
            .check_event_expression_effects(expression, &self.path)?;
        let value = self.cg.lower_expr(&self.path, expression)?;
        let context = self.cg.event_context(expression)?;
        let name = self.cg.new_fn_name(&self.path, "event_eval");
        let real = value.is_real();
        if real {
            self.pre_fns.push(crate::sim::ir::IrPreFn::RealEval {
                c_name: name.clone(),
                value,
                context,
            });
        } else {
            self.pre_fns.push(crate::sim::ir::IrPreFn::MonEval {
                c_name: name.clone(),
                args: vec![value],
                context,
                item: false,
            });
        }
        Ok((name, real))
    }

    fn lower_event_spec(
        &mut self,
        spec: &EventSpec,
        condition: Option<NodeId>,
    ) -> Result<(IrWaitSrc, IrEdge), String> {
        let (expression, edge, event) = match spec {
            EventSpec::Qualified { event, condition } => {
                return self.lower_event_spec(event, Some(*condition))
            }
            EventSpec::Named(event) => {
                let target = self.cg.event_target_of(*event).ok_or_else(|| {
                    format!(
                        "event control has an unresolved named event in `{}`",
                        self.path
                    )
                })?;
                (None, IrEdge::Any, Some(target))
            }
            EventSpec::AnyChange { sig } => {
                (Some(*sig), IrEdge::Any, self.cg.event_target_of(*sig))
            }
            EventSpec::Edge { sig, posedge } => {
                if self.cg.event_target_of(*sig).is_some() {
                    return Err(format!(
                        "edge control on a named event is not supported in `{}`",
                        self.path
                    ));
                }
                (
                    Some(*sig),
                    if *posedge {
                        IrEdge::Posedge
                    } else {
                        IrEdge::Negedge
                    },
                    None,
                )
            }
        };
        let condition = condition
            .map(|expression| {
                self.event_evaluator(expression).and_then(|(name, real)| {
                    if real {
                        Err(format!(
                            "real-valued event qualifiers are not supported in `{}`",
                            self.path
                        ))
                    } else {
                        Ok(name)
                    }
                })
            })
            .transpose()?;
        if let Some(event) = event {
            let event = self.cg.event_ref_of(&event, &self.path)?;
            return Ok((
                match condition {
                    Some(condition) => IrWaitSrc::FilteredEvent { event, condition },
                    None => IrWaitSrc::Event(event),
                },
                edge,
            ));
        }
        let expression = expression.ok_or_else(|| "event control has no expression".to_string())?;
        let simple = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        );
        // Member paths share aggregate storage, so event waits must compare the selected value.
        let selected_aggregate_member = self.cg.packed_member_info(expression).is_some()
            || self.cg.unpacked_member_info(expression).is_some();
        let mapped_formal = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self
                .func
                .as_ref()
                .is_some_and(|func| func.arg_ir.contains_key(target))
        );
        let persistent_subroutine_signal = match self.cg.kind(expression) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().rev().copied().flatten().next()
            }
            _ => None,
        }
        .and_then(|target| {
            self.cg
                .func
                .as_ref()
                .and_then(|function| function.persistent.get(&target))
                .or_else(|| self.cg.static_formals.get(&(self.inst, target)))
                .or_else(|| self.cg.static_task_locals.get(&(self.inst, target)))
        });
        // A member of an unpacked-structure net has no signal of its own;
        // it waits through the evaluated path like other projections.
        let unresolved_member = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::HierPath { .. })
        ) && persistent_subroutine_signal.is_none()
            && self.cg.resolve_signal_id(&self.path, expression).is_err()
            && self.cg.unpacked_path_for_expr(expression).is_some();
        if simple
            && !selected_aggregate_member
            && !unresolved_member
            && condition.is_none()
            && self.cg.nested_proc_local_ref(expression).is_none()
            && !mapped_formal
        {
            let (name, info) = match self.cg.resolve_signal_id(&self.path, expression) {
                Ok(resolved) => resolved,
                Err(error) => {
                    let Some(signal) = persistent_subroutine_signal else {
                        return Err(error);
                    };
                    if signal.real {
                        if edge != IrEdge::Any {
                            return Err(format!(
                                "edge control on real-valued signal `{}` is not supported in `{}`",
                                self.cg.signal_label(signal.ir),
                                self.cg.source_path(&self.path)
                            ));
                        }
                        return Ok((IrWaitSrc::Real(signal.global.clone()), edge));
                    }
                    return Ok((
                        IrWaitSrc::Sig(self.cg.signal_dependency_name(signal.ir)),
                        edge,
                    ));
                }
            };
            if info.real {
                if edge != IrEdge::Any {
                    return Err(format!(
                        "edge control on real-valued signal `{}` is not supported in `{}`",
                        self.cg.signal_label(info.ir),
                        self.cg.source_path(&self.path)
                    ));
                }
                return Ok((IrWaitSrc::Real(name), edge));
            }
            return Ok((
                IrWaitSrc::Sig(self.cg.signal_dependency_name(info.ir)),
                edge,
            ));
        }
        let reads = self.cg.collect_read_signals(&self.path, expression)?;
        let (eval, real) = self.event_evaluator(expression)?;
        if real && edge != IrEdge::Any {
            return Err(format!(
                "edge control on real-valued expressions is not supported in `{}`",
                self.path
            ));
        }
        Ok((
            if real {
                IrWaitSrc::EvaluatedReal {
                    eval,
                    condition,
                    reads,
                }
            } else {
                IrWaitSrc::Evaluated {
                    eval,
                    condition,
                    reads,
                }
            },
            edge,
        ))
    }
}
