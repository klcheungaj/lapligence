//! Events.

use super::super::collection::EventEvaluation;
use super::*;
use crate::sim::ir::IrRuntimeQuery;

/// One value evaluated when a process-evaluated event control is armed.
pub(super) struct ProcessEventArm {
    /// Local that holds the armed value while the process waits.
    pub(super) local: String,
    pub(super) width: u32,
    pub(super) signed: bool,
    pub(super) value: IrExpr,
}

/// An event control that the waiting process evaluates itself instead of a
/// read-only runtime callback, because a helper it calls has effects (see
/// [`EventEvaluation`]). Arming and waiting are separate so a nonblocking
/// assignment can arm in its issuing process and wait in a detached one.
pub(super) struct ProcessEventPlan {
    /// Values of every source when the control is armed, in source order.
    pub(super) arm: Vec<ProcessEventArm>,
    /// Two-state flag set once a source detects its event.
    hit: String,
    /// Re-evaluate every source, set `hit` on a detected event and advance
    /// the armed values.
    iteration: Vec<IrStmt>,
    /// The single atomic suspension between iterations.
    wait: IrStmt,
    /// Unsigned one-bit zero that clears `hit`.
    zero: IrExpr,
}

impl ProcessEventPlan {
    fn hit_declaration(&self) -> IrStmt {
        IrStmt::DeclLocal {
            name: self.hit.clone(),
            width: 1,
            signed: false,
            init: Some(Box::new(self.zero.clone())),
            two_state: true,
        }
    }

    fn wait_loop(&self) -> IrStmt {
        IrStmt::While {
            cond: IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(IrExpr::new(
                        IrExprKind::LocalRead(self.hit.clone()),
                        1,
                        false,
                        None,
                    )),
                },
                1,
                false,
                None,
            ),
            body: vec![self.wait.clone(), IrStmt::Block(self.iteration.clone())],
        }
    }

    /// Arm and wait in the current process.
    pub(super) fn blocking(self) -> IrStmt {
        let mut block = self
            .arm
            .iter()
            .map(|arm| IrStmt::DeclLocal {
                name: arm.local.clone(),
                width: arm.width,
                signed: arm.signed,
                init: Some(Box::new(arm.value.clone())),
                two_state: false,
            })
            .collect::<Vec<_>>();
        block.push(self.hit_declaration());
        block.push(self.wait_loop());
        IrStmt::Block(block)
    }

    /// Wait in a process that did not arm the control. `armed` reads each
    /// armed value, in [`Self::arm`] order, from wherever the arming process
    /// left it. The first iteration runs before any suspension, so a change
    /// between arming and the first run of this process is still detected.
    pub(super) fn resumed(self, armed: Vec<IrExpr>) -> Vec<IrStmt> {
        let mut block = self
            .arm
            .iter()
            .zip(armed)
            .map(|(arm, value)| IrStmt::DeclLocal {
                name: arm.local.clone(),
                width: arm.width,
                signed: arm.signed,
                init: Some(Box::new(value)),
                two_state: false,
            })
            .collect::<Vec<_>>();
        block.push(self.hit_declaration());
        block.push(IrStmt::Block(self.iteration.clone()));
        block.push(self.wait_loop());
        vec![IrStmt::Block(block)]
    }
}

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
    pub(super) fn event_specs_need_process(&self, specs: &[EventSpec]) -> bool {
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
    /// [`EventEvaluation`]) into one block that arms and waits in the current
    /// process (see [`ProcessEventPlan`]).
    fn lower_process_evaluated_event(
        &mut self,
        h: NodeId,
        specs: &[EventSpec],
    ) -> Result<Vec<IrStmt>, String> {
        Ok(vec![self.process_event_plan(h, specs)?.blocking()])
    }

    /// Plan an event control that the waiting process evaluates itself. Every
    /// value source is evaluated once when the control is armed; the process
    /// then suspends on the union of the sources' read sets and of the named
    /// events in the list. After each wake it re-evaluates every value source
    /// and resumes only when one detects its change (SV 9.4.2: a changed
    /// operand with an unchanged result is no event; packed edges use the
    /// least significant bit; real values compare IEEE bit patterns) or a named
    /// event's trigger count moved. Qualifiers run only for a detected source.
    /// The language leaves the number of helper evaluations unspecified; this
    /// form performs one per value source at arm time and one per value source
    /// per wake.
    pub(super) fn process_event_plan(
        &mut self,
        h: NodeId,
        specs: &[EventSpec],
    ) -> Result<ProcessEventPlan, String> {
        enum Source {
            Value(NodeId, IrEdge),
            Named(NodeId),
        }
        fn flatten(
            cg: &Codegen<'_>,
            spec: &EventSpec,
            condition: Option<NodeId>,
            out: &mut Vec<(Source, Option<NodeId>)>,
        ) {
            match spec {
                EventSpec::Qualified { event, condition } => {
                    flatten(cg, event, Some(*condition), out)
                }
                EventSpec::Named(event) => out.push((Source::Named(*event), condition)),
                EventSpec::AnyChange { sig } if cg.event_target_of(*sig).is_some() => {
                    out.push((Source::Named(*sig), condition))
                }
                EventSpec::AnyChange { sig } => {
                    out.push((Source::Value(*sig, IrEdge::Any), condition))
                }
                EventSpec::Edge { sig, posedge } => out.push((
                    Source::Value(
                        *sig,
                        if *posedge {
                            IrEdge::Posedge
                        } else {
                            IrEdge::Negedge
                        },
                    ),
                    condition,
                )),
            }
        }
        let path = self.cg.source_path(&self.path);
        let mut sources = Vec::new();
        for spec in specs {
            flatten(self.cg, spec, None, &mut sources);
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
        let mut events = Vec::new();
        let mut arm = Vec::new();
        let mut current = Vec::new();
        let mut detect = Vec::new();
        let mut advance = Vec::new();
        for (index, (source, condition)) in sources.into_iter().enumerate() {
            let last = format!("_llg_evl{}_{index}", h.0);
            let next = format!("_llg_evn{}_{index}", h.0);
            let (value, changed) = match source {
                Source::Named(event) => {
                    let target = self.cg.event_target_of(event).ok_or_else(|| {
                        format!("event control has an unresolved named event in `{path}`")
                    })?;
                    let IrEventRef::Static(event) = self.cg.event_ref_of(&target, &self.path)?
                    else {
                        return Err(format!(
                            "an event handle other than a declared named event cannot share an event control with a process-evaluated helper expression in `{path}`"
                        ));
                    };
                    events.push((IrWaitSrc::Event(IrEventRef::Static(event)), IrEdge::Any));
                    let count = IrExpr::new(
                        IrExprKind::RuntimeQuery(IrRuntimeQuery::EventTriggerCount(event)),
                        64,
                        false,
                        None,
                    );
                    let changed = binary(
                        IrBinOp::CaseNeq,
                        local(&last, 64, false),
                        local(&next, 64, false),
                    );
                    (count, changed)
                }
                Source::Value(expression, edge) => {
                    let value = self.cg.lower_expr(&self.path, expression)?;
                    let value = if value.is_real() {
                        if edge != IrEdge::Any {
                            return Err(format!(
                                "edge control on real-valued expressions is not supported in `{path}`"
                            ));
                        }
                        // Real any-change compares IEEE bit patterns, as the
                        // runtime's real waits do: signed zeros differ and a
                        // repeated NaN payload is no change.
                        IrExpr::new(
                            IrExprKind::SysFunc(Box::new(IrSysFunc::RealToBits(Box::new(value)))),
                            64,
                            false,
                            None,
                        )
                    } else if edge == IrEdge::Any {
                        value
                    } else {
                        IrExpr::convert_to(value, 1, false)
                    };
                    for dependency in self
                        .cg
                        .collect_evaluator_sensitivity(&self.path, expression)?
                    {
                        if !sens.contains(&dependency) {
                            sens.push(dependency);
                        }
                    }
                    let (width, signed) = (value.width, value.signed);
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
                    (value, changed)
                }
            };
            let (width, signed) = (value.width, value.signed);
            arm.push(ProcessEventArm {
                local: last.clone(),
                width,
                signed,
                value: value.clone(),
            });
            current.push(IrStmt::DeclLocal {
                name: next.clone(),
                width,
                signed,
                init: Some(Box::new(value)),
                two_state: false,
            });
            let found = IrStmt::Assign {
                lhs: target(&hit, 1, false, true),
                rhs: one.clone(),
                nba: false,
            };
            let then_ = match condition {
                Some(condition) => {
                    let condition = self.cg.lower_expr(&self.path, condition)?;
                    if condition.is_real() {
                        return Err(format!(
                            "real-valued event qualifiers are not supported in `{path}`"
                        ));
                    }
                    vec![IrStmt::If {
                        cond: condition,
                        then_: vec![found],
                        els: None,
                        check: IrUniquePriorityCheck::None,
                    }]
                }
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
        let wait = if events.is_empty() {
            IrStmt::WaitAny { sens }
        } else {
            // One atomic wait covers the named events and every value
            // source's dependencies, so no trigger is lost between waits.
            // A selected-bit dependency waits on its whole storage: an
            // unrelated change only costs one re-evaluation.
            let mut specs = events;
            for dependency in sens {
                let source = match &dependency {
                    IrDependency::Real(name) => IrWaitSrc::Real(name.clone()),
                    other => match other.scalar_name() {
                        Some(name) => IrWaitSrc::Sig(name.to_owned()),
                        None => {
                            return Err(format!(
                                "a named event cannot share an event control with a process-evaluated helper that reads array, container or string storage in `{path}`"
                            ))
                        }
                    },
                };
                if !specs.iter().any(|(existing, _)| *existing == source) {
                    specs.push((source, IrEdge::Any));
                }
            }
            IrStmt::WaitEvents { specs }
        };
        let mut iteration = current;
        iteration.extend(detect);
        iteration.extend(advance);
        Ok(ProcessEventPlan {
            arm,
            hit,
            iteration,
            wait,
            zero,
        })
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
            EventTriggerTiming::Event { specs, .. } if self.event_specs_need_process(specs) => {
                self.process_evaluated_event_trigger(statement, event, specs, None)
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
                if self.event_specs_need_process(specs) {
                    return self.process_evaluated_event_trigger(
                        statement,
                        event,
                        specs,
                        Some(repeat),
                    );
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

    /// `->> @(...) ev` whose control calls a helper with effects: like the
    /// runtime's detached waiter, a detached process waits for the control
    /// (armed at issue) and then queues the trigger in NBA (SV 15.5.2).
    fn process_evaluated_event_trigger(
        &mut self,
        statement: NodeId,
        event: IrEventRef,
        specs: &[EventSpec],
        repeat: Option<IrExpr>,
    ) -> Result<Vec<IrStmt>, String> {
        if !matches!(event, IrEventRef::Static(_)) {
            return Err(format!(
                "a nonblocking trigger of an event handle cannot use a process-evaluated event control in `{}`",
                self.cg.source_path(&self.path)
            ));
        }
        let frame = self.cg.new_frame_id()?;
        let action = vec![IrStmt::NonblockingEventTrigger {
            ev: event,
            ticks: None,
        }];
        Ok(vec![self.spawn_process_evaluated_action(
            statement,
            specs,
            repeat,
            frame,
            Vec::new(),
            action,
            "event_trigger",
        )?])
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
                real_item: false,
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
        // A modport expression port waits on its port expression (SV 25.5.4).
        let expression = self
            .cg
            .modport_expression_target(expression)
            .unwrap_or(expression);
        let simple = matches!(
            self.cg.kind(expression),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        );
        // Member paths share aggregate storage, so event waits must compare the selected value.
        let selected_aggregate_member = self.cg.packed_member_info(expression).is_some()
            || self.cg.unpacked_member_info(expression).is_some()
            || self.cg.packed_element_member_select(expression).is_some();
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
        // A whole string or class handle waits on its change marker
        // (SIM-007), which toggles on every changed store; neither has edges.
        if simple && condition.is_none() && !mapped_formal {
            if let Some(object) = self.cg.object_of(&self.path, expression).filter(|object| {
                matches!(
                    self.cg.model.objects[*object].ty,
                    crate::sim::ir::IrObjectType::Chandle | crate::sim::ir::IrObjectType::String
                )
            }) {
                if edge != IrEdge::Any {
                    return Err(format!(
                        "edge control on a string or class handle is not supported in `{}`",
                        self.cg.source_path(&self.path)
                    ));
                }
                let object = self.cg.reference_object(object);
                return Ok((
                    IrWaitSrc::Sig(format!("{}_llg_dep", self.cg.model.objects[object].c_name)),
                    edge,
                ));
            }
        }
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
