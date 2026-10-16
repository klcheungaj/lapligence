//! Procedural concurrent assertions (IEEE 1800-2009 16.15.6).
//!
//! A concurrent assertion reached in procedural code is not evaluated where
//! it stands: the statement queues one pending instance on the executing
//! process, the runtime clears the queue at the 16.15.6.2 flush points and
//! matures the rest in the Observed region, and each matured instance begins
//! one evaluation attempt at a leading clock event. This module infers the
//! leading clock a procedure supplies to its assertions and lowers the
//! assertion statement to that queue operation.

use super::*;

/// The clock a procedure infers for its procedural concurrent assertions
/// (16.15.6 a)-c)): the unique `edge_identifier expression1 [iff
/// expression2]` event expression of its single event control.
#[derive(Clone)]
pub(in crate::sim::codegen) enum ProcedureClock {
    /// A `posedge` or `negedge` expression, with its `iff` qualifier.
    Edge(EventSpec),
    /// `edge expression1`, which an assertion clock cannot use in llg.
    BothEdges(NodeId),
}

impl Codegen<'_> {
    /// The clock inferred for the context of procedure body `stmt`, or
    /// `None` when 16.15.6 infers none: a blocking timing control, more than
    /// one event control, or not exactly one qualifying event expression.
    /// Statements of assertion action blocks count as procedure statements;
    /// property and sequence bodies do not.
    pub(in crate::sim::codegen) fn infer_procedure_clock(
        &self,
        stmt: NodeId,
    ) -> Option<ProcedureClock> {
        let mut event_controls = Vec::new();
        let mut stack = vec![stmt];
        while let Some(node) = stack.pop() {
            match self.kind(node) {
                // A property's clocks and delays are not procedure timing.
                NodeKind::AssertionExpr(_) => continue,
                NodeKind::Stmt(
                    StmtKind::DelayControl { .. }
                    | StmtKind::CycleDelayControl { .. }
                    | StmtKind::Wait { .. }
                    | StmtKind::WaitOrder { .. }
                    | StmtKind::WaitFork,
                ) => return None,
                NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                    kind: ConcurrentAssertionKind::Expect,
                    ..
                }) => return None,
                NodeKind::Stmt(StmtKind::Assign {
                    blocking,
                    delay: Some(control),
                    ..
                }) => {
                    if *blocking {
                        return None;
                    }
                    // A nonblocking intra-assignment event control is an
                    // event control of the procedure.
                    if !matches!(control, IntraControl::Delay(_)) {
                        event_controls.push(None);
                    }
                }
                NodeKind::Stmt(StmtKind::EventTrigger {
                    timing: Some(timing),
                    ..
                }) => {
                    if !matches!(timing, EventTriggerTiming::Delay { .. }) {
                        event_controls.push(None);
                    }
                }
                NodeKind::Stmt(StmtKind::EventControl {
                    specs, implicit, ..
                }) => event_controls.push((!implicit).then_some((node, specs))),
                _ => {}
            }
            stack.extend(self.node(node).children.iter().rev().copied());
        }
        let [Some((_, specs))] = event_controls.as_slice() else {
            return None;
        };
        // Spec expressions of this control are not part of the body.
        let control_nodes: HashSet<NodeId> = specs
            .iter()
            .flat_map(|spec| {
                let mut nodes = Vec::new();
                event_spec_nodes(spec, &mut nodes);
                nodes
            })
            .collect();
        let mut candidates: Vec<ProcedureClock> = Vec::new();
        for (index, spec) in specs.iter().enumerate() {
            let Some((signal, posedge, _)) = flatten_clock_spec(spec) else {
                continue;
            };
            // `edge e` is captured as a posedge/negedge pair of one
            // expression node; its negedge half joins the posedge half.
            let paired = specs.iter().enumerate().any(|(other, spec)| {
                other != index
                    && flatten_clock_spec(spec)
                        .is_some_and(|(sig, edge, _)| sig == signal && edge != posedge)
            });
            if paired && !posedge {
                continue;
            }
            if self.terms_appear_in_body(signal, stmt, &control_nodes) {
                continue;
            }
            candidates.push(if paired {
                ProcedureClock::BothEdges(signal)
            } else {
                ProcedureClock::Edge(spec.clone())
            });
        }
        match candidates.as_slice() {
            [clock] => Some(clock.clone()),
            _ => None,
        }
    }

    /// Whether a declaration that `expression` references is referenced
    /// anywhere in procedure body `body`, outside the event expressions of
    /// its event control (16.15.6 c) 2)).
    fn terms_appear_in_body(
        &self,
        expression: NodeId,
        body: NodeId,
        control_nodes: &HashSet<NodeId>,
    ) -> bool {
        let mut terms = HashSet::new();
        let mut stack = vec![expression];
        while let Some(node) = stack.pop() {
            self.referenced_declarations(node, &mut terms);
            stack.extend(self.node(node).children.iter().copied());
        }
        if terms.is_empty() {
            return false;
        }
        let mut stack = vec![body];
        let mut found = HashSet::new();
        while let Some(node) = stack.pop() {
            if control_nodes.contains(&node) {
                continue;
            }
            found.clear();
            self.referenced_declarations(node, &mut found);
            if !found.is_disjoint(&terms) {
                return true;
            }
            match self.kind(node) {
                // An instance body expands a named property or sequence
                // declaration; only its actual arguments are procedure text.
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => stack.extend(
                    self.node(node)
                        .children
                        .iter()
                        .copied()
                        .filter(|child| child != body),
                ),
                _ => stack.extend(self.node(node).children.iter().copied()),
            }
        }
        false
    }

    /// Declarations that expression node `node` itself names.
    fn referenced_declarations(&self, node: NodeId, into: &mut HashSet<NodeId>) {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => {
                into.insert(*target);
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                into.extend(refs.iter().flatten().copied());
            }
            _ => {}
        }
    }

    /// The leading clock that the procedure being lowered supplies to a
    /// procedural assertion without its own clock, or `None` when it infers
    /// none and default clocking applies (16.15.6).
    pub(super) fn procedure_assertion_clock(
        &mut self,
        path: &str,
        origin: NodeId,
    ) -> Result<Option<SampledClock>, String> {
        let Some(body) = self.procedure_body else {
            return Ok(None);
        };
        match self.infer_procedure_clock(body) {
            None => Ok(None),
            Some(ProcedureClock::BothEdges(signal)) => Err(format!(
                "{}: the clock inferred from the procedure (`edge` of the expression at line \
                 {}, column {}) cannot clock a concurrent assertion in llg; give the assertion \
                 a posedge or negedge clock (IEEE 1800-2009 16.15.6)",
                self.source_location(origin),
                self.node(signal).line,
                self.node(signal).col
            )),
            Some(ProcedureClock::Edge(spec)) => self
                .lower_sampled_clock_spec(path, std::slice::from_ref(&spec))?
                .map(Some)
                .ok_or_else(|| {
                    format!(
                        "{}: the clock inferred from the procedure is not a direct signal edge \
                         (IEEE 1800-2009 16.15.6)",
                        self.source_location(origin)
                    )
                }),
        }
    }

    /// Lower a concurrent assertion reached in procedural code: register it
    /// as procedural assertion `index` and return the statement that queues
    /// one of its instances.
    pub(in crate::sim::codegen) fn emit_procedural_concurrent_assertion(
        &mut self,
        inst: NodeId,
        path: &str,
        assertion: NodeId,
    ) -> Result<IrStmt, String> {
        self.reject_procedural_captures(assertion)?;
        self.emit_concurrent_assertion(inst, path, assertion)?;
        let index = self.procedural_assertions;
        let registered = self
            .model
            .assertions
            .last_mut()
            .filter(|registered| registered.identity == assertion.index() as u64)
            .ok_or_else(|| "procedural assertion was not registered".to_owned())?;
        registered.procedural = Some(index);
        self.procedural_assertions = index
            .checked_add(1)
            .ok_or_else(|| "too many procedural concurrent assertions".to_owned())?;
        Ok(IrStmt::QueueProceduralAssertion { index })
    }

    /// Reject the procedural assertion forms whose instances carry values
    /// captured when they are queued (16.15.6.1): automatic variables of the
    /// procedure and `const'` casts, in the assertion or its action blocks.
    /// The runtime queue keeps no per-instance values, so evaluating these
    /// forms from sampled values would silently change their meaning.
    fn reject_procedural_captures(&self, assertion: NodeId) -> Result<(), String> {
        // Report the first offending reference in source order.
        let mut first: Option<(NodeId, Option<NodeId>)> = None;
        let mut stack = vec![assertion];
        while let Some(node) = stack.pop() {
            stack.extend(self.node(node).children.iter().copied());
            let offending = if self.db.is_const_cast(node) {
                Some(None)
            } else if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(node)
            {
                // Property and sequence local variables belong to the
                // attempt, not to the procedure.
                (matches!(self.kind(*target), NodeKind::Var { .. })
                    && self.db.variable_lifetime(*target) == VariableLifetime::Automatic
                    && !self.db.is_assertion_local_var(*target))
                .then_some(Some(*target))
            } else {
                None
            };
            let Some(variable) = offending else {
                continue;
            };
            let position = |id: NodeId| (self.node(id).line, self.node(id).col);
            if first.is_none_or(|(known, _)| position(node) < position(known)) {
                first = Some((node, variable));
            }
        }
        match first {
            None => Ok(()),
            Some((node, None)) => Err(format!(
                "{}: a const cast in a procedural concurrent assertion takes its value when \
                 the assertion is queued (IEEE 1800-2009 16.15.6.1); llg does not capture \
                 queued values",
                self.source_location(node)
            )),
            Some((node, Some(variable))) => Err(format!(
                "{}: automatic variable `{}` in a procedural concurrent assertion takes its \
                 value when the assertion is queued (IEEE 1800-2009 16.15.6.1); llg does not \
                 capture queued values",
                self.source_location(node),
                self.node(variable).name
            )),
        }
    }
}

/// Every expression node an event spec names.
fn event_spec_nodes(spec: &EventSpec, nodes: &mut Vec<NodeId>) {
    match spec {
        EventSpec::Qualified { event, condition } => {
            event_spec_nodes(event, nodes);
            nodes.push(*condition);
        }
        EventSpec::Edge { sig, .. } | EventSpec::AnyChange { sig } | EventSpec::Named(sig) => {
            nodes.push(*sig)
        }
    }
}
