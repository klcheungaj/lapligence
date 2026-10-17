//! Sequence endpoints (IEEE 1800-2009 16.9.11, 16.14.5-16.14.6, 9.4.2.4,
//! 9.4.4).
//!
//! `.triggered`, `.matched` and an event control on a sequence observe the
//! end points of one sequence instance, whatever tick each match started on.
//! Each distinct instance becomes one hidden cover of every match (an
//! [`IrAssertionEndpoint`]) that starts an attempt on every tick of its
//! clock. A match triggers the endpoint's event in the Observed region, so
//! `@(seq)`, `wait(seq.triggered)` and `.triggered` read ordinary event
//! state, and latches `.matched` for each destination clock. Endpoints are
//! created before the assertions that read them, so within one Observed pass
//! a consumer sees the matches of the same time step.

use super::*;
use crate::sim::ir::{IrAssertionEndpoint, IrEventRef, IrRuntimeQuery};

impl Codegen<'_> {
    /// Whether `node` is a named sequence instance usable as an endpoint
    /// operand. Slang captures `s` and `s(args)` as an owned, already-bound
    /// instance expression, and only admits sequence instances as the
    /// operand of a sequence method or an event control.
    pub(in super::super) fn is_sequence_endpoint_operand(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::AssertionInstance { .. })
        )
    }

    /// Create the endpoints of every sequence instance an event-control list
    /// waits on (`@(seq)`, 9.4.2.4), before the list is lowered: event-target
    /// resolution then sees each instance as its endpoint's event.
    pub(in super::super) fn ensure_event_spec_endpoints(
        &mut self,
        path: &str,
        specs: &[EventSpec],
    ) -> Result<(), String> {
        for spec in specs {
            match spec {
                EventSpec::Qualified { event, .. } => {
                    self.ensure_event_spec_endpoints(path, std::slice::from_ref(event))?
                }
                EventSpec::Named(node) | EventSpec::AnyChange { sig: node }
                    if self.is_sequence_endpoint_operand(*node) =>
                {
                    self.sequence_endpoint(path, *node, None)?;
                }
                EventSpec::Edge { sig, .. } if self.is_sequence_endpoint_operand(*sig) => {
                    return Err(format!(
                        "an edge event control on a sequence is illegal at {} (SV 9.4.2.4)",
                        self.source_location(*sig)
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `.triggered` of sequence instance `receiver`. Inside an assertion the
    /// status is a runtime query of the endpoint; in procedural code it is the
    /// endpoint event's triggered state, so a `wait` is woken by the match.
    pub(in super::super) fn lower_sequence_triggered(
        &mut self,
        path: &str,
        receiver: NodeId,
    ) -> Result<IrExpr, String> {
        let context = self.assertion_leading_clock.and(self.sampled_clock);
        let index = self.sequence_endpoint(path, receiver, context)?;
        let endpoint = self.model.assertions[index]
            .endpoint()
            .cloned()
            .ok_or_else(|| "sequence endpoint lost its endpoint record".to_owned())?;
        if let Some(context) = context {
            // 16.9.11: "the ending clock of the sequence instance to which
            // triggered is applied shall always be the same as the clock in
            // the context where the application of method triggered appears".
            let assertion = &self.model.assertions[index];
            let ending = assertion
                .consequent_sequence()
                .and_then(|sequence| {
                    sequence
                        .trailing_clock
                        .map(|signal| (signal, sequence.trailing_posedge))
                })
                .unwrap_or((assertion.clock_signal(), assertion.posedge()));
            if ending != (context.signal, context.posedge) {
                return Err(format!(
                    "`.triggered` at {} needs its sequence to end on the clock of its context (SV 16.9.11); use `.matched` across clocks",
                    self.source_location(receiver)
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::RuntimeQuery(IrRuntimeQuery::SequenceTriggered(endpoint.index())),
                1,
                false,
                None,
            ));
        }
        Ok(IrExpr::new(
            IrExprKind::EventTriggered(IrEventRef::Static(endpoint.event())),
            1,
            false,
            None,
        ))
    }

    /// `.matched` of sequence instance `receiver` at the current sampled
    /// clock of a sequence (16.14.6: "matched can only be used in sequence
    /// expressions").
    pub(in super::super) fn lower_sequence_matched(
        &mut self,
        path: &str,
        receiver: NodeId,
    ) -> Result<IrExpr, String> {
        let Some(context) = self.assertion_leading_clock.and(self.sampled_clock) else {
            return Err(format!(
                "sequence `.matched` at {} can only be used in a sequence expression (SV 16.14.6)",
                self.source_location(receiver)
            ));
        };
        if context.gate.is_some() {
            return Err(format!(
                "sequence `.matched` under an `iff`-qualified clock is not supported at {}",
                self.source_location(receiver)
            ));
        }
        let index = self.sequence_endpoint(path, receiver, Some(context))?;
        let endpoint = self.model.assertions[index]
            .endpoint_mut()
            .ok_or_else(|| "sequence endpoint lost its endpoint record".to_owned())?;
        let slot = endpoint.matched_slot((context.signal, context.posedge))?;
        let endpoint = endpoint.index();
        Ok(IrExpr::new(
            IrExprKind::RuntimeQuery(IrRuntimeQuery::SequenceMatched { endpoint, slot }),
            1,
            false,
            None,
        ))
    }

    /// The assertion-table index of the endpoint of `receiver` in `path`,
    /// lowering it on first use. Instances without arguments share one
    /// endpoint per sequence and scope; an instance with arguments is its own
    /// endpoint. `context` is the clock a sequence without its own or a
    /// default clock inherits from the sequence that reads it.
    pub(in super::super) fn sequence_endpoint(
        &mut self,
        path: &str,
        receiver: NodeId,
        context: Option<SampledClock>,
    ) -> Result<usize, String> {
        let NodeKind::Expr(ExprKind::AssertionInstance {
            target,
            body,
            bindings,
        }) = self.kind(receiver)
        else {
            return Err(format!(
                "sequence method receiver at {} is not a sequence instance",
                self.source_location(receiver)
            ));
        };
        let (target, body) = (*target, *body);
        let key = (
            path.to_owned(),
            if bindings.is_empty() {
                target
            } else {
                receiver
            },
        );
        if let Some(&index) = self.sequence_endpoints.get(&key) {
            self.bind_endpoint_event(receiver, index);
            return Ok(index);
        }
        // 16.10: local variables reach a `.triggered`/`.matched` instance
        // only as entire actual arguments that flow back out of it; that
        // flow is not represented by an independent endpoint.
        for binding in bindings {
            if self.references_assertion_local(binding.actual) {
                return Err(format!(
                    "a local variable passed to a sequence endpoint at {} is not supported",
                    self.source_location(receiver)
                ));
            }
        }
        // An endpoint is an attempt context of its own, not a part of the
        // sequence or process being lowered.
        let saved_clock = self.sampled_clock.take();
        let saved_leading = self.assertion_leading_clock.take();
        let saved_depth = std::mem::replace(&mut self.assertion_instance_depth, 0);
        let saved_capture =
            std::mem::replace(&mut self.assertion_local_capture_at_attempt_entry, false);
        let saved_locals = self.assertion_local_bindings.take();
        let saved_initializers = self.assertion_local_initializers.take();
        let saved_changes = std::mem::take(&mut self.assertion_clock_changes);
        let parts = (|| {
            let own = self.explicit_leading_clock(path, body)?;
            let inherited = if own.is_none() && self.lower_default_sampled_clock(path)?.is_none() {
                context.map(|clock| (clock.signal, clock.posedge, clock.gate))
            } else {
                None
            };
            let flowing = match inherited {
                Some((signal, posedge, gate)) => Some(SampledClock {
                    signal,
                    posedge,
                    gate,
                }),
                None => own.or(self.lower_default_sampled_clock(path)?),
            };
            self.assertion_clock_changes = self.collect_clock_changes(path, body, flowing)?;
            self.lower_assertion_instance(receiver, true, |this, body| {
                this.lower_property_inner(path, body, inherited, None, receiver)
            })?
            .ok_or_else(|| "sequence endpoint body is missing".to_owned())
        })();
        self.sampled_clock = saved_clock;
        self.assertion_leading_clock = saved_leading;
        self.assertion_instance_depth = saved_depth;
        self.assertion_local_capture_at_attempt_entry = saved_capture;
        self.assertion_local_bindings = saved_locals;
        self.assertion_local_initializers = saved_initializers;
        self.assertion_clock_changes = saved_changes;
        let parts = parts.map_err(|error| {
            format!(
                "{error} (sequence endpoint at {})",
                self.source_location(receiver)
            )
        })?;
        if parts.antecedent.is_some()
            || parts.antecedent_sequence.is_some()
            || parts.abort_condition.is_some()
            || parts.disable_signal.is_some()
        {
            return Err(format!(
                "sequence endpoint at {} is not a sequence",
                self.source_location(receiver)
            ));
        }
        let label = self.node(target).name.clone();
        let event = self
            .new_event_info(self.c_name(
                "E",
                path,
                &["sequence_endpoint", &label, &receiver.index().to_string()],
            ))
            .ir;
        let index = u32::try_from(self.sequence_endpoint_count)
            .map_err(|_| "too many sequence endpoints".to_owned())?;
        self.sequence_endpoint_count += 1;
        let identity = receiver.index() as u64;
        let location = self.source_location(receiver);
        let assertion = if let Some(consequent) = parts.consequent_sequence {
            IrAssertion::new_sequence(
                identity,
                path.to_owned(),
                label,
                location,
                IrConcurrentAssertionKind::CoverSequence,
                parts.clock_signal,
                parts.posedge,
                None,
                None,
                consequent,
                true,
                None,
                None,
            )
        } else {
            let consequent = parts.consequent.ok_or_else(|| {
                format!("sequence endpoint at {location} has no lowered sequence")
            })?;
            IrAssertion::new(
                identity,
                path.to_owned(),
                label,
                location,
                IrConcurrentAssertionKind::CoverSequence,
                parts.clock_signal,
                parts.posedge,
                None,
                None,
                consequent,
                true,
                None,
                None,
            )
        };
        let mut assertion = assertion.with_endpoint(IrAssertionEndpoint::new(index, event));
        if parts.clock_gate.is_some() {
            let gate = self.intern_sampled_clock(
                path,
                SampledClockSource::Edge(SampledClock {
                    signal: parts.clock_signal,
                    posedge: parts.posedge,
                    gate: parts.clock_gate,
                }),
                None,
            )?;
            assertion = assertion.with_clock_gate(gate);
        }
        let table_index = self.model.assertions.len();
        self.model.assertions.push(assertion);
        self.sequence_endpoints.insert(key, table_index);
        self.bind_endpoint_event(receiver, table_index);
        Ok(table_index)
    }

    /// Resolve `receiver` as a named event to the endpoint's event.
    fn bind_endpoint_event(&mut self, receiver: NodeId, table_index: usize) {
        let Some(event) = self.model.assertions[table_index]
            .endpoint()
            .map(IrAssertionEndpoint::event)
        else {
            return;
        };
        let global = self.model.event(event).c_name().to_owned();
        self.event_globals
            .entry(receiver)
            .or_insert(EventInfo { global, ir: event });
    }

    fn references_assertion_local(&self, node: NodeId) -> bool {
        let mut pending = vec![node];
        while let Some(current) = pending.pop() {
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(current)
            {
                if self.db.is_assertion_local_var(*target) {
                    return true;
                }
            }
            pending.extend(self.node(current).children.iter().copied());
        }
        false
    }
}
