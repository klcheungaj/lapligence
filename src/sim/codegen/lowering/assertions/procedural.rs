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
use crate::sim::ir::{IrSampledCall, IrSampledFunc};

/// The clock a procedure infers for its procedural concurrent assertions
/// (16.15.6 a)-c)): the unique `edge_identifier expression1 [iff
/// expression2]` event expression of its single event control.
#[derive(Clone)]
pub(in crate::sim::codegen) enum ProcedureClock {
    /// A `posedge` or `negedge` expression, with its `iff` qualifier.
    Edge(EventSpec),
    /// `edge expression1`, captured as its posedge and its negedge event
    /// expressions (each with the `iff` qualifier), the clocking event of
    /// both edges.
    BothEdges(Vec<EventSpec>),
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
                ProcedureClock::BothEdges(
                    specs
                        .iter()
                        .filter(|spec| {
                            flatten_clock_spec(spec).is_some_and(|(sig, _, _)| sig == signal)
                        })
                        .cloned()
                        .collect(),
                )
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
            // `edge expression1` is the clocking event of both edges, ticking
            // once per time step like any other event clock.
            Some(ProcedureClock::BothEdges(specs)) => {
                self.assertion_clock_from_specs(path, &specs, 0).map(Some)
            }
            Some(ProcedureClock::Edge(spec)) => self
                .lower_sampled_clock_spec(path, std::slice::from_ref(&spec))?
                .map(Some)
                .ok_or_else(|| {
                    self.locate_error(
                        origin,
                        "the clock inferred from the procedure is not a direct signal edge \
                         (IEEE 1800-2009 16.15.6)",
                    )
                }),
        }
    }

    /// The clocking event that procedure body `body` infers for a checker
    /// instantiated in it, which its `$inferred_clock` formal defaults
    /// stand for (17.4), as its event expressions, or `None` when the
    /// procedure infers none.
    pub(in crate::sim::codegen) fn procedure_inferred_event(
        &self,
        body: NodeId,
    ) -> Option<Vec<EventSpec>> {
        match self.infer_procedure_clock(body)? {
            ProcedureClock::Edge(spec) => Some(vec![spec]),
            ProcedureClock::BothEdges(specs) => Some(specs),
        }
    }

    /// Lower a concurrent assertion reached in procedural code: register it
    /// as procedural assertion `index` and return the statement that queues
    /// one of its instances. `captures` are the values each instance saves,
    /// lowered in the procedure, for the sites [`Self::queued_value_sites`]
    /// found (slot order); the assertion reads them through
    /// [`crate::sim::ir::IrSampledFunc::Queued`].
    pub(in crate::sim::codegen) fn emit_procedural_concurrent_assertion(
        &mut self,
        inst: NodeId,
        path: &str,
        assertion: NodeId,
        sites: &[QueuedValueSite],
        captures: Vec<IrExpr>,
    ) -> Result<IrStmt, String> {
        let previous = self.enter_queued_values(sites, &captures);
        let result = self.emit_concurrent_assertion(inst, path, assertion);
        self.queued_values = previous;
        result?;
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
        Ok(IrStmt::QueueProceduralAssertion { index, captures })
    }

    /// Make the nodes of `sites` read their saved values while one
    /// assertion is lowered; returns the map to restore afterwards.
    pub(in crate::sim::codegen) fn enter_queued_values(
        &mut self,
        sites: &[QueuedValueSite],
        captures: &[IrExpr],
    ) -> HashMap<NodeId, QueuedValue> {
        let mut values = HashMap::new();
        for site in sites {
            let capture = &captures[site.slot as usize];
            values.insert(
                site.node,
                QueuedValue {
                    slot: site.slot,
                    width: capture.width,
                    signed: capture.signed,
                },
            );
        }
        std::mem::replace(&mut self.queued_values, values)
    }

    /// The read of a value the instance being lowered saved when it was
    /// queued, for a node [`Self::queued_value_sites`] found.
    pub(in crate::sim::codegen) fn queued_value_read(&self, node: NodeId) -> Option<IrExpr> {
        let value = self.queued_values.get(&node)?;
        // The argument records the shape only; the runtime reads the slot.
        let shape = IrConst::packed(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            value.width,
            value.signed,
            None,
        )
        .ok()?;
        Some(IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::Sampled(IrSampledCall::new(
                IrSampledFunc::Queued,
                IrExpr::new(IrExprKind::Const(shape), value.width, value.signed, None),
                None,
                u64::from(value.slot),
            )))),
            value.width,
            value.signed,
            None,
        ))
    }

    /// The values a procedural concurrent assertion instance saves when it
    /// is queued (16.15.6.1): every automatic variable of the procedure and
    /// every outermost `const'` cast in the assertion or its action blocks.
    /// References to one automatic variable share a slot; slots follow
    /// source order. Property and sequence local variables belong to the
    /// attempt, not to the procedure, and are not saved.
    pub(in crate::sim::codegen) fn queued_value_sites(
        &self,
        assertion: NodeId,
    ) -> QueuedValueSites {
        let mut found: Vec<(NodeId, Option<NodeId>)> = Vec::new();
        let mut stack = vec![assertion];
        while let Some(node) = stack.pop() {
            if self.db.is_const_cast(node) {
                found.push((node, None));
                continue;
            }
            stack.extend(self.node(node).children.iter().copied());
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(node)
            {
                if self.automatic_procedure_value(*target) {
                    found.push((node, Some(*target)));
                }
            }
        }
        let position = |id: NodeId| (self.node(id).line, self.node(id).col, id.index());
        found.sort_by_key(|(node, _)| position(*node));
        let mut sites = QueuedValueSites::default();
        let mut slots: HashMap<NodeId, u32> = HashMap::new();
        for (node, variable) in found {
            let slot = match variable.and_then(|variable| slots.get(&variable)) {
                Some(slot) => *slot,
                None => {
                    let slot = sites.sources.len() as u32;
                    sites.sources.push(node);
                    if let Some(variable) = variable {
                        slots.insert(variable, slot);
                    }
                    slot
                }
            };
            sites.sites.push(QueuedValueSite { node, slot });
        }
        sites
    }

    /// Whether `declaration` is an automatic variable of the procedure or of
    /// an automatic subroutine it runs, including a formal other than `ref`.
    /// Property and sequence local variables belong to the attempt.
    fn automatic_procedure_value(&self, declaration: NodeId) -> bool {
        match self.kind(declaration) {
            NodeKind::Var { .. } => {
                self.db.variable_lifetime(declaration) == VariableLifetime::Automatic
                    && !self.db.is_assertion_local_var(declaration)
            }
            NodeKind::FuncArg { direction, .. } => {
                *direction != DbDirection::Ref
                    && self.node(declaration).parent.is_some_and(|subroutine| {
                        matches!(
                            self.kind(subroutine),
                            NodeKind::FuncTask {
                                automatic: true,
                                ..
                            }
                        )
                    })
            }
            _ => false,
        }
    }

    /// A located error for a saved value llg cannot keep: `capture` is the
    /// value as lowered in the procedure.
    pub(in crate::sim::codegen) fn check_queued_value(
        &self,
        node: NodeId,
        capture: &IrExpr,
    ) -> Result<(), String> {
        if capture.width != 0 && !capture.is_real() {
            return Ok(());
        }
        let what = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => format!("automatic variable `{}`", self.node(*target).name),
            _ => "const cast".to_owned(),
        };
        Err(self.locate_error(
            node,
            format!(
                "{what} in a procedural concurrent assertion takes its value when the \
             assertion is queued (IEEE 1800-2009 16.15.6.1); llg saves packed integral values \
             only"
            ),
        ))
    }

    /// Reject a saved value in `expression`, which is evaluated outside the
    /// attempts of the assertion being lowered (`what`: a `disable iff`
    /// condition, a clock's `iff` or a sampled value function's history).
    pub(in crate::sim::codegen) fn reject_queued_read(
        &self,
        expression: &IrExpr,
        what: &str,
    ) -> Result<(), String> {
        if self.queued_values.is_empty() {
            return Ok(());
        }
        let mut reads = false;
        crate::sim::opt::walk_expr_mut(&mut expression.clone(), &mut |node| {
            if let IrExprKind::SysFunc(function) = &node.kind {
                if let IrSysFunc::Sampled(call) = &**function {
                    reads |= call.kind == IrSampledFunc::Queued;
                }
            }
        });
        if !reads {
            return Ok(());
        }
        let message = format!(
            "{what} of this procedural concurrent assertion reads an automatic variable or a \
             const cast, whose value the instance saves when it is queued (IEEE 1800-2009 \
             16.15.6.1); llg evaluates it outside the instance's attempts and does not support \
             this"
        );
        Err(match self.assertion_inferred_origin {
            Some(origin) => self.locate_error(origin, message),
            None => message,
        })
    }

    /// Reject queued values in an assertion lowered outside the procedure
    /// that queues it: a procedural checker's static assertion, whose
    /// automatic or `const'` actuals 17.3 substitutes when it is queued.
    pub(in crate::sim::codegen) fn reject_queued_values(
        &self,
        assertion: NodeId,
    ) -> Result<(), String> {
        let sites = self.queued_value_sites(assertion);
        let Some(site) = sites.sites.first() else {
            return Ok(());
        };
        let what = match self.kind(site.node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => format!("automatic variable `{}`", self.node(*target).name),
            _ => "a const cast".to_owned(),
        };
        Err(self.locate_error(
            site.node,
            format!(
                "{what} in a procedural concurrent assertion takes its value when the \
             assertion is queued (IEEE 1800-2009 16.15.6.1); llg saves queued values for \
             assertions written in the procedure, not for the assertions of a procedural \
             checker (17.3)"
            ),
        ))
    }
}

/// One node of a procedural concurrent assertion that reads a value saved
/// when the instance was queued, and its slot in the instance's frame.
#[derive(Clone, Copy, Debug)]
pub(in crate::sim::codegen) struct QueuedValueSite {
    pub(in crate::sim::codegen) node: NodeId,
    pub(in crate::sim::codegen) slot: u32,
}

/// The saved values of one procedural concurrent assertion: `sources` are
/// the nodes evaluated in the procedure, one per slot; `sites` every node
/// reading a slot.
#[derive(Default)]
pub(in crate::sim::codegen) struct QueuedValueSites {
    pub(in crate::sim::codegen) sources: Vec<NodeId>,
    pub(in crate::sim::codegen) sites: Vec<QueuedValueSite>,
}

/// Slot and shape of one saved value while its assertion is lowered.
#[derive(Clone, Copy, Debug)]
pub(in crate::sim::codegen) struct QueuedValue {
    slot: u32,
    width: u32,
    signed: bool,
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
