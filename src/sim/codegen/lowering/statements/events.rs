//! Events.

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
        let spec_pairs = self.lower_event_specs(specs)?;
        let wait = if implicit || spec_pairs.is_empty() {
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
                        ident(&self.path)
                    )));
                IrStmt::WaitEvents {
                    specs: vec![(IrWaitSrc::Event(IrEventRef::Static(event)), IrEdge::Any)],
                }
            } else {
                IrStmt::WaitAny { sens: reads }
            }
        } else {
            IrStmt::WaitEvents { specs: spec_pairs }
        };
        self.saw_wait = true;
        let mut out = vec![wait];
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
        let mapped_formal = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self
                .func
                .as_ref()
                .is_some_and(|func| func.arg_ir.contains_key(target))
        );
        if simple
            && condition.is_none()
            && self.cg.nested_proc_local_ref(expression).is_none()
            && !mapped_formal
        {
            let (name, info) = self.cg.resolve_signal_id(&self.path, expression)?;
            if info.real {
                if edge != IrEdge::Any {
                    return Err(format!(
                        "edge control on real-valued signal `{name}` is not supported in `{}`",
                        self.path
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
