//! Global clocking future sampled value functions (IEEE 1800-2009 16.9.4).
//!
//! `$future_gclk(v)` is the sampled value of `v` at the next global clock
//! tick, while the attempt's interval is defined "as though the future
//! sampled values were known in advance". An assertion calling these
//! functions is therefore evaluated one global clock tick late
//! ([`IrAssertionFuture`]): every subexpression that reads sampled values at
//! the clock event becomes a [`IrSampledFunc::Captured`] operand, evaluated
//! when the event occurs; the future function arguments are evaluated when the
//! event is, at the next global clock tick, where their current sampled value
//! is the future one. Sequence locals stay live, since the deferred engine
//! itself maintains them.

use super::*;
use crate::sim::ir::{IrAssertionFuture, IrPropertyNode, IrSampledCall, IrSampledFunc};

/// What a subexpression requires of the deferred evaluation.
#[derive(Clone, Copy, Default)]
struct Needs {
    /// It calls a global clocking future function.
    future: bool,
    /// It reads a sequence local variable.
    local: bool,
    /// It writes a local or calls a match-item subroutine.
    effect: bool,
}

fn needs(expression: &IrExpr) -> Needs {
    let mut needs = Needs::default();
    let mut copy = expression.clone();
    crate::sim::opt::walk_expr_mut(&mut copy, &mut |node| match &node.kind {
        IrExprKind::SysFunc(function) => {
            if let IrSysFunc::Sampled(call) = &**function {
                needs.future |= call.kind.is_future();
            }
        }
        IrExprKind::LocalRead(name) => {
            needs.local |= name.starts_with("llg_sequence_local_read(");
        }
        IrExprKind::Mutation(_) | IrExprKind::MatchCall(_) => needs.effect = true,
        _ => {}
    });
    needs
}

/// Whether the assertion calls a global clocking future function.
fn calls_future(assertion: &IrAssertion) -> bool {
    assertion
        .expressions()
        .into_iter()
        .any(|expression| needs(expression).future)
}

struct Deferral<'a> {
    slots: u32,
    location: &'a str,
}

impl Deferral<'_> {
    fn slot(&mut self) -> Result<u32, String> {
        let slot = self.slots;
        self.slots = slot.checked_add(1).ok_or_else(|| {
            format!(
                "assertion at {} captures too many operands for its global clocking future sampled value functions",
                self.location
            )
        })?;
        Ok(slot)
    }

    fn limit(&self, what: &str) -> String {
        format!(
            "{what} in an assertion with global clocking future sampled value functions at {} is not supported (IEEE 1800-2009 16.9.4)",
            self.location
        )
    }

    /// Rewrite `expression` so that it reads the values of its clock event's
    /// time step from captured operands.
    fn defer(&mut self, expression: &mut IrExpr) -> Result<(), String> {
        let own = needs(expression);
        if !own.future && !own.local && !own.effect {
            if matches!(expression.kind, IrExprKind::Const(_) | IrExprKind::Fill(_)) {
                return Ok(());
            }
            if expression.width == 0 {
                return Err(self.limit("a real operand"));
            }
            let slot = self.slot()?;
            let argument = expression.clone();
            *expression = IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::Sampled(IrSampledCall::new(
                    IrSampledFunc::Captured,
                    argument,
                    None,
                    u64::from(slot),
                )))),
                expression.width,
                expression.signed,
                None,
            );
            return Ok(());
        }
        match &mut expression.kind {
            IrExprKind::SysFunc(function) => {
                let IrSysFunc::Sampled(call) = &mut **function else {
                    return Err(self.limit("this system function"));
                };
                if !call.kind.is_future() {
                    return Err(self.limit("this sampled value function"));
                }
                let argument = needs(&call.argument);
                if argument.local || argument.effect {
                    return Err(self.limit("a future value of a local variable"));
                }
                if call.kind != IrSampledFunc::Future {
                    call.ticks = u64::from(self.slot()?);
                }
                Ok(())
            }
            IrExprKind::LocalRead(_) => Ok(()),
            IrExprKind::Mutation(mutation) => self.defer(&mut mutation.value),
            IrExprKind::Bin { a, b, .. } => {
                self.defer(a)?;
                self.defer(b)
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a }
            | IrExprKind::CastToPacked { a } => self.defer(a),
            IrExprKind::Mux { sel, a, b } => {
                self.defer(sel)?;
                self.defer(a)?;
                self.defer(b)
            }
            IrExprKind::Concat { parts }
            | IrExprKind::Replicate { parts, .. }
            | IrExprKind::Predicate { clauses: parts } => {
                parts.iter_mut().try_for_each(|part| self.defer(part))
            }
            IrExprKind::BitSel { base, idx } => {
                self.defer(base)?;
                self.defer(idx)
            }
            IrExprKind::PartSel { base, .. } => self.defer(base),
            // The width expression is kept for analysis only.
            IrExprKind::IdxPartSel { base, base_idx, .. } => {
                self.defer(base)?;
                self.defer(base_idx)
            }
            _ => Err(self.limit("this expression form")),
        }
    }
}

impl Codegen<'_> {
    /// Complete an assertion that calls global clocking future sampled value
    /// functions; any other assertion is returned unchanged. The functions'
    /// own illegal contexts (nesting, sequence match items, action blocks,
    /// procedural code, missing global clocking) are frontend errors.
    pub(super) fn finish_future_assertion(
        &mut self,
        path: &str,
        mut assertion: IrAssertion,
    ) -> Result<IrAssertion, String> {
        if !calls_future(&assertion) {
            return Ok(assertion);
        }
        let location = assertion.location.clone();
        let deferral_limit = |what: &str| {
            format!(
                "{what} with global clocking future sampled value functions at {location} is not supported (IEEE 1800-2009 16.9.4)"
            )
        };
        if assertion.kind == IrConcurrentAssertionKind::Expect {
            return Err(deferral_limit("`expect`"));
        }
        if assertion.endpoint.is_some() {
            return Err(deferral_limit("a sequence end point"));
        }
        // Asynchronous aborts act between assertion clock ticks.
        let async_abort = (assertion.abort_condition.is_some() && !assertion.abort_sync)
            || assertion.property.as_ref().is_some_and(|property| {
                property
                    .nodes
                    .iter()
                    .any(|node| matches!(node, IrPropertyNode::Abort { sync: false, .. }))
            });
        if async_abort {
            return Err(deferral_limit("`accept_on`/`reject_on`"));
        }
        let clock = match self.assertion_future_clock.take() {
            Some(clock) => clock,
            None => {
                let source = self.lower_global_sampled_clock(path)?;
                self.intern_sampled_clock(path, source, None)?
            }
        };
        let mut deferral = Deferral {
            slots: 0,
            location: &location,
        };
        for expression in assertion.expressions_mut() {
            deferral.defer(expression)?;
        }
        let slots = deferral.slots;
        assertion.future = Some(IrAssertionFuture { clock, slots });
        Ok(assertion)
    }
}
