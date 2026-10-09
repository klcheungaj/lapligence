//! Concurrent assertion lowering.
//!
//! The H22 sequence lowerer uses one shared sampled-clock automaton for
//! concatenation, repetition, and the admitted sequence combinators. H23 adds
//! bounded one-cycle property composition and recursive use of owned named
//! sequence/property bodies; H24 adds per-attempt local storage, local input
//! formal capture, and ordered match-item effects. Forms outside the
//! executable subset fail closed here rather than becoming an untimed
//! immediate assertion.

use std::collections::HashSet;

use super::*;
use crate::core::db::{
    AssertionBinaryOp, AssertionExprKind, AssertionRange, AssertionRepetition,
    AssertionRepetitionKind, AssertionUnaryOp, ConcurrentAssertionKind, EventSpec,
};
use crate::sim::ir::{
    IrAssertion, IrBinOp, IrConcurrentAssertionKind, IrExpr, IrExprKind, IrLhs, IrProcess,
    IrSampledClock, IrSampledClockKind, IrSampledDomain, IrSequence, IrSequenceJoin,
    IrSequenceJoinKind, IrSequenceLocal, IrSequenceRange, IrSequenceTransition, IrShape, IrUnOp,
};

/// Upper bound on automaton states for one lowered sequence graph. Literal
/// repetition bounds (`s[*1000]`) unroll their body once per iteration;
/// unbounded forms never unroll beyond their lower bound. Exceeding the
/// budget is an explicit lowering error rather than an unbounded C table.
const SEQUENCE_STATE_BUDGET: u32 = 1 << 20;

struct PropertyParts {
    clock_signal: usize,
    posedge: bool,
    disable_signal: Option<usize>,
    antecedent: Option<IrExpr>,
    consequent: Option<IrExpr>,
    antecedent_sequence: Option<IrSequence>,
    consequent_sequence: Option<IrSequence>,
    overlapped: bool,
    abort_condition: Option<IrExpr>,
    abort_reject: bool,
    abort_sync: bool,
}

struct SequenceBuilder {
    next_state: u32,
    next_scope: u32,
    transitions: Vec<IrSequenceTransition>,
    atoms: Vec<IrExpr>,
    first_match: bool,
    first_match_states: Vec<u32>,
    match_items: Vec<IrExpr>,
    joins: Vec<IrSequenceJoin>,
}

#[derive(Clone, Copy)]
struct SequenceFragment {
    /// Empty-word alternative, separate from the nonempty NFA paths.
    empty: bool,
    start: u32,
    accept: u32,
    leading_clock: Option<SampledClock>,
    trailing_clock: Option<SampledClock>,
}

impl SequenceBuilder {
    fn new() -> Self {
        Self {
            next_state: 0,
            next_scope: 1,
            transitions: Vec::new(),
            atoms: Vec::new(),
            first_match: false,
            first_match_states: Vec::new(),
            match_items: Vec::new(),
            joins: Vec::new(),
        }
    }

    fn state(&mut self) -> Result<u32, String> {
        let state = self.next_state;
        if state >= SEQUENCE_STATE_BUDGET {
            return Err(format!(
                "sequence automaton exceeds the {SEQUENCE_STATE_BUDGET}-state lowering budget"
            ));
        }
        self.next_state = self
            .next_state
            .checked_add(1)
            .ok_or_else(|| "sequence automaton has too many states".to_owned())?;
        Ok(state)
    }

    fn edge_with_clock(
        &mut self,
        from: u32,
        to: u32,
        delay: AssertionRange,
        atom: Option<IrExpr>,
        clock: Option<SampledClock>,
    ) -> Result<(), String> {
        let atom = atom
            .map(|expr| {
                let index = self.atoms.len();
                self.atoms.push(expr);
                u32::try_from(index)
                    .map_err(|_| "sequence has too many atom expressions".to_owned())
            })
            .transpose()?;
        self.transitions.push(IrSequenceTransition {
            from,
            to,
            delay: IrSequenceRange {
                min: delay.min,
                max: delay.max,
            },
            clock_signal: clock.map(|clock| clock.signal),
            clock_posedge: clock.is_some_and(|clock| clock.posedge),
            atom,
            match_start: None,
            match_count: 0,
            enter_scope: None,
            exit_scope: None,
            enter_join: None,
            exit_join: None,
        });
        Ok(())
    }

    fn epsilon_with_clock(
        &mut self,
        from: u32,
        to: u32,
        delay: AssertionRange,
        clock: Option<SampledClock>,
    ) -> Result<(), String> {
        self.edge_with_clock(from, to, delay, None, clock)
    }

    /// Attach source-order match items to every path entering a fragment's
    /// endpoint. The fragment has just been lowered, so no enclosing
    /// concatenation edge has been emitted yet; this keeps each side effect at
    /// the sequence point where its match item is prescribed.
    fn attach_match_items(&mut self, endpoint: u32, items: Vec<IrExpr>) -> Result<(), String> {
        if items.is_empty() {
            return Ok(());
        }
        let start = u32::try_from(self.match_items.len())
            .map_err(|_| "sequence has too many match-item expressions".to_owned())?;
        let count = u32::try_from(items.len())
            .map_err(|_| "sequence has too many match-item expressions".to_owned())?;
        let mut attached = false;
        for transition in &mut self.transitions {
            if transition.to != endpoint {
                continue;
            }
            attached = true;
            if transition.match_count == 0 {
                transition.match_start = Some(start);
                transition.match_count = count;
                continue;
            }
            let existing_start = transition
                .match_start
                .ok_or_else(|| "sequence transition has an invalid match-item range".to_owned())?;
            let existing_end = existing_start
                .checked_add(transition.match_count)
                .ok_or_else(|| "sequence match-item range overflows".to_owned())?;
            if existing_end != start {
                return Err(
                    "nested sequence match items do not have a contiguous source-order range"
                        .to_owned(),
                );
            }
            transition.match_count = transition
                .match_count
                .checked_add(count)
                .ok_or_else(|| "sequence match-item range overflows".to_owned())?;
        }
        if !attached {
            return Err("sequence match item has no reachable endpoint".to_owned());
        }
        self.match_items.extend(items);
        Ok(())
    }

    fn finish(
        self,
        fragment: SequenceFragment,
        locals: Vec<IrSequenceLocal>,
        initializers: Vec<IrExpr>,
        initializer_slots: Vec<u32>,
    ) -> Result<IrSequence, String> {
        IrSequence::new(
            self.next_state,
            fragment.start,
            fragment.accept,
            self.transitions,
            self.atoms,
            self.first_match,
            self.first_match_states,
            locals,
            self.match_items,
            initializers,
            initializer_slots,
            fragment.empty,
            self.joins,
            fragment.leading_clock.map(|clock| clock.signal),
            fragment.leading_clock.is_some_and(|clock| clock.posedge),
            fragment.trailing_clock.map(|clock| clock.signal),
            fragment.trailing_clock.is_some_and(|clock| clock.posedge),
        )
        .map_err(|error| error.to_string())
    }

    fn concatenate(
        &mut self,
        left: SequenceFragment,
        right: SequenceFragment,
        delay: AssertionRange,
    ) -> Result<SequenceFragment, String> {
        let start = self.state()?;
        let accept = self.state()?;
        let clock = right.leading_clock.or(left.trailing_clock);
        if let (Some(a), Some(b)) = (left.trailing_clock, right.leading_clock) {
            if a.signal != b.signal || a.posedge != b.posedge {
                if !matches!((delay.min, delay.max), (0, Some(0)) | (1, Some(1))) {
                    return Err(
                        "cross-clock sequence boundaries require an exact ##0 or ##1".to_owned(),
                    );
                }
                if left.empty || right.empty {
                    return Err(
                        "multiclock sequence segments must not admit empty matches".to_owned()
                    );
                }
            }
        }
        self.epsilon_with_clock(start, left.start, zero_range(), left.leading_clock)?;
        self.epsilon_with_clock(left.accept, right.start, delay.clone(), clock)?;
        self.epsilon_with_clock(right.accept, accept, zero_range(), right.trailing_clock)?;
        // Empty endpoints lie before the starting tick. Normalize them before
        // emission, so no runtime token must travel backwards in sampled time.
        if delay.max.is_none_or(|max| max >= 1) {
            let reduced = AssertionRange {
                min: delay.min.max(1) - 1,
                max: delay.max.map(|max| max - 1),
            };
            if left.empty {
                self.epsilon_with_clock(start, right.start, reduced.clone(), clock)?;
            }
            if right.empty {
                self.epsilon_with_clock(left.accept, accept, reduced, left.trailing_clock)?;
            }
        }
        if left.empty && right.empty && delay.max.is_none_or(|max| max >= 2) {
            self.epsilon_with_clock(
                start,
                accept,
                AssertionRange {
                    min: delay.min.max(2) - 2,
                    max: delay.max.map(|max| max - 2),
                },
                clock,
            )?;
        }
        Ok(SequenceFragment {
            empty: left.empty
                && right.empty
                && delay.min <= 1
                && delay.max.is_none_or(|max| max >= 1),
            start,
            accept,
            leading_clock: left.leading_clock.or(right.leading_clock),
            trailing_clock: right.trailing_clock.or(left.trailing_clock),
        })
    }

    fn first_match(&mut self, inner: SequenceFragment) -> Result<SequenceFragment, String> {
        let start = self.state()?;
        let accept = self.state()?;
        if !inner.empty {
            let scope = self.next_scope;
            self.next_scope = scope
                .checked_add(1)
                .ok_or_else(|| "too many first_match scopes".to_owned())?;
            self.epsilon_with_clock(start, inner.start, zero_range(), inner.leading_clock)?;
            self.transitions.last_mut().unwrap().enter_scope = Some(scope);
            self.epsilon_with_clock(inner.accept, accept, zero_range(), inner.trailing_clock)?;
            self.transitions.last_mut().unwrap().exit_scope = Some(scope);
        }
        // An admitted empty alternative is always the earliest endpoint.
        Ok(SequenceFragment {
            empty: inner.empty,
            start,
            accept,
            leading_clock: inner.leading_clock,
            trailing_clock: inner.trailing_clock,
        })
    }

    /// Join two operands that start on the same tick. The runtime forks one
    /// thread per operand at the enter edge and pairs operand endpoints at
    /// the exit edges by `kind`; operand empty words are part of the plan so
    /// `and` can treat them as already matched.
    fn join(
        &mut self,
        kind: IrSequenceJoinKind,
        left: SequenceFragment,
        right: SequenceFragment,
    ) -> Result<SequenceFragment, String> {
        let clocks_differ = |a: Option<SampledClock>, b: Option<SampledClock>| matches!((a, b), (Some(a), Some(b)) if a.signal != b.signal || a.posedge != b.posedge);
        if clocks_differ(left.leading_clock, right.leading_clock)
            || clocks_differ(left.trailing_clock, right.trailing_clock)
            || clocks_differ(left.leading_clock, left.trailing_clock)
            || clocks_differ(right.leading_clock, right.trailing_clock)
        {
            return Err(
                "and/intersect/within/throughout operands must share one clock; multiclocked \
                 operands are not supported"
                    .to_owned(),
            );
        }
        let index = u32::try_from(self.joins.len())
            .map_err(|_| "sequence has too many joins".to_owned())?;
        let start = self.state()?;
        let accept = self.state()?;
        self.joins.push(IrSequenceJoin {
            kind,
            left_start: left.start,
            right_start: right.start,
            left_empty: left.empty,
            right_empty: right.empty,
        });
        self.epsilon_with_clock(start, left.start, zero_range(), left.leading_clock)?;
        self.transitions.last_mut().unwrap().enter_join = Some(index);
        self.epsilon_with_clock(left.accept, accept, zero_range(), left.trailing_clock)?;
        self.transitions.last_mut().unwrap().exit_join = Some(index);
        self.epsilon_with_clock(right.accept, accept, zero_range(), right.trailing_clock)?;
        self.transitions.last_mut().unwrap().exit_join = Some(index);
        Ok(SequenceFragment {
            empty: left.empty && right.empty,
            start,
            accept,
            leading_clock: left.leading_clock.or(right.leading_clock),
            trailing_clock: left.trailing_clock.or(right.trailing_clock),
        })
    }
}

fn unbounded_star() -> AssertionRepetition {
    AssertionRepetition {
        kind: AssertionRepetitionKind::Consecutive,
        range: AssertionRange { min: 0, max: None },
    }
}

/// The constant `1'b1` sequence atom used by the `within` padding.
fn true_atom() -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(crate::sim::ir::IrConst {
            bits: vec![1],
            x: vec![0],
            z: vec![0],
            width: 1,
            signed: false,
            real: None,
            fill: None,
        }),
        1,
        false,
        None,
    )
}

fn zero_range() -> AssertionRange {
    AssertionRange {
        min: 0,
        max: Some(0),
    }
}

#[allow(clippy::too_many_arguments)]
fn repetition_step(
    builder: &mut SequenceBuilder,
    from: u32,
    to: u32,
    delay: AssertionRange,
    atom: &IrExpr,
    negative: &IrExpr,
    kind: AssertionRepetitionKind,
    clock: Option<SampledClock>,
) -> Result<(), String> {
    builder.edge_with_clock(from, to, delay.clone(), Some(atom.clone()), clock)?;
    if kind != AssertionRepetitionKind::Consecutive {
        let waiting = builder.state()?;
        let unit = AssertionRange {
            min: 1,
            max: Some(1),
        };
        builder.edge_with_clock(from, waiting, delay, Some(negative.clone()), clock)?;
        builder.edge_with_clock(
            waiting,
            waiting,
            unit.clone(),
            Some(negative.clone()),
            clock,
        )?;
        builder.edge_with_clock(waiting, to, unit, Some(atom.clone()), clock)?;
    }
    Ok(())
}

fn repetition_accept(
    builder: &mut SequenceBuilder,
    from: u32,
    accept: u32,
    negative: &IrExpr,
    kind: AssertionRepetitionKind,
    clock: Option<SampledClock>,
) -> Result<(), String> {
    builder.epsilon_with_clock(from, accept, zero_range(), clock)?;
    if kind == AssertionRepetitionKind::Nonconsecutive {
        let tail = builder.state()?;
        let unit = AssertionRange {
            min: 1,
            max: Some(1),
        };
        builder.edge_with_clock(from, tail, unit.clone(), Some(negative.clone()), clock)?;
        builder.edge_with_clock(tail, tail, unit, Some(negative.clone()), clock)?;
        builder.epsilon_with_clock(tail, accept, zero_range(), clock)?;
    }
    Ok(())
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
        let parts = self.lower_property(path, property, assertion)?;
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
            ConcurrentAssertionKind::CoverSequence => IrConcurrentAssertionKind::CoverSequence,
            ConcurrentAssertionKind::Expect => IrConcurrentAssertionKind::Expect,
        };
        let abort_condition = parts.abort_condition.clone();
        let abort_reject = parts.abort_reject;
        let abort_sync = parts.abort_sync;
        if let Some(consequent) = parts.consequent {
            let assertion = IrAssertion::new(
                assertion.index() as u64,
                path.to_owned(),
                label,
                location,
                kind,
                parts.clock_signal,
                parts.posedge,
                parts.disable_signal,
                parts.antecedent,
                consequent,
                parts.overlapped,
                pass_action,
                fail_action,
            );
            self.model.assertions.push(match abort_condition {
                Some(condition) => {
                    assertion.with_abort_control(condition, abort_reject, abort_sync)
                }
                None => assertion,
            });
        } else if let Some(consequent) = parts.consequent_sequence {
            let assertion = IrAssertion::new_sequence(
                assertion.index() as u64,
                path.to_owned(),
                label,
                location,
                kind,
                parts.clock_signal,
                parts.posedge,
                parts.disable_signal,
                parts.antecedent_sequence,
                consequent,
                parts.overlapped,
                pass_action,
                fail_action,
            );
            self.model.assertions.push(match abort_condition {
                Some(condition) => {
                    assertion.with_abort_control(condition, abort_reject, abort_sync)
                }
                None => assertion,
            });
        } else {
            return Err(format!(
                "concurrent assertion has no lowered consequent at {path}"
            ));
        }
        // The assertion registration is shared with ordinary concurrent
        // assertions. The enclosing statement lowerer emits the procedural
        // arm/wait marker for `expect` after this model entry is complete.
        Ok(())
    }

    fn lower_property(
        &mut self,
        path: &str,
        root: NodeId,
        origin: NodeId,
    ) -> Result<PropertyParts, String> {
        self.lower_property_inner(path, root, None, None, origin)
    }

    fn assertion_location(&self, node: NodeId, origin: NodeId) -> String {
        let location = self.source_location(node);
        if location == "<unknown>:0:0" {
            self.source_location(origin)
        } else {
            location
        }
    }

    /// Lower one property expression while carrying metadata inherited from
    /// an enclosing named property/sequence instance. Slang has already
    /// expanded assertion actuals into the owned instance body, so recursive
    /// lowering preserves the binding semantics without reinterpreting source
    /// text or formal names.
    fn lower_property_inner(
        &mut self,
        path: &str,
        root: NodeId,
        inherited_clock: Option<(usize, bool)>,
        inherited_disable: Option<usize>,
        origin: NodeId,
    ) -> Result<PropertyParts, String> {
        let mut current = root;
        let mut clock = inherited_clock;
        let mut disable = inherited_disable;
        let mut abort = None;
        loop {
            match self.kind(current) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    signal,
                    posedge,
                    expr,
                    ..
                }) => {
                    let signal = self.lower_assertion_signal(path, *signal, "clock")?;
                    match clock {
                        None => clock = Some((signal, *posedge)),
                        Some((existing, existing_posedge))
                            if existing == signal && existing_posedge == *posedge => {}
                        Some(_) => {
                            return Err(format!(
                                "multiple clocks in concurrent assertion at {}",
                                self.assertion_location(current, origin)
                            ));
                        }
                    }
                    current = *expr;
                }
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff {
                    condition, expr, ..
                }) => {
                    let signal = self.lower_assertion_signal(path, *condition, "disable iff")?;
                    match disable {
                        None => disable = Some(signal),
                        Some(existing) if existing == signal => {}
                        Some(_) => {
                            return Err(format!(
                                "multiple disable iff conditions in concurrent assertion at {}",
                                self.assertion_location(current, origin)
                            ));
                        }
                    }
                    current = *expr;
                }
                NodeKind::AssertionExpr(AssertionExprKind::Abort {
                    condition,
                    expr,
                    reject,
                    sync,
                }) => {
                    if abort.is_some() {
                        return Err(format!(
                            "nested accept_on/reject_on controls are not supported in concurrent assertion at {}",
                            self.assertion_location(current, origin)
                        ));
                    }
                    abort = Some((*condition, *reject, *sync));
                    current = *expr;
                }
                _ => break,
            }
        }

        // An assertion instance's body is an owned, already-bound assertion
        // expression. Re-enter it with the clock/disable metadata collected
        // above. This admits named sequence/property use while retaining the
        // exact declaration argument/default expansion performed by Slang.
        if let Some(instance) = match self.kind(current) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { .. }) => Some(*expr),
                _ => None,
            },
            _ => None,
        } {
            let capture_at_attempt_entry = self.assertion_instance_depth == 0;
            let mut parts = self
                .lower_assertion_instance(instance, capture_at_attempt_entry, |this, body| {
                    this.lower_property_inner(path, body, clock, disable, origin)
                })?
                .ok_or_else(|| "assertion instance body is missing".to_owned())?;
            if let Some((condition_node, reject, sync)) = abort {
                if parts.abort_condition.is_some() {
                    return Err(format!(
                        "nested accept_on/reject_on controls are not supported in concurrent assertion at {}",
                        self.assertion_location(current, origin)
                    ));
                }
                let condition = self.lower_abort_condition(
                    path,
                    condition_node,
                    parts.clock_signal,
                    parts.posedge,
                )?;
                parts.abort_condition = Some(condition);
                parts.abort_reject = reject;
                parts.abort_sync = sync;
            }
            return Ok(parts);
        }

        if clock.is_none() {
            clock = self
                .infer_leading_assertion_clock(path, current)?
                .map(|clock| (clock.signal, clock.posedge));
        }
        if clock.is_none() {
            clock = self
                .lower_default_sampled_clock(path)?
                .map(|clock| (clock.signal, clock.posedge));
        }

        let (clock_signal, posedge) = clock.ok_or_else(|| {
            format!(
                "concurrent assertions require one explicit signal clock at {} ({path})",
                self.assertion_location(current, origin)
            )
        })?;
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
            // Conditional properties are admitted in the bounded one-cycle
            // subset below.  Keeping the whole conditional as the consequent
            // preserves its sampled condition and avoids treating the two
            // branches as independent assertion instances.
            NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. }) => (None, current, true),
            NodeKind::AssertionExpr(_) => (None, current, true),
            _ => {
                return Err(format!(
                    "unsupported concurrent assertion property at {}",
                    self.assertion_location(current, origin)
                ))
            }
        };
        let previous_clock = self.sampled_clock;
        self.sampled_clock = Some(SampledClock {
            signal: clock_signal,
            posedge,
            gate: None,
        });
        let use_engine = !matches!(
            self.kind(current),
            NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. })
        ) && (antecedent_node
            .is_some_and(|node| self.sequence_requires_engine(node))
            || self.sequence_requires_engine(consequent_node));
        let expressions = (|| {
            if use_engine {
                let antecedent = antecedent_node
                    .map(|node| self.lower_sequence(path, node, "antecedent"))
                    .transpose()?;
                if let Some(antecedent) = &antecedent {
                    if let Some(signal) = antecedent.trailing_clock {
                        self.sampled_clock = Some(SampledClock {
                            signal,
                            posedge: antecedent.trailing_posedge,
                            gate: None,
                        });
                    }
                }
                let consequent = self.lower_sequence(path, consequent_node, "consequent")?;
                Ok::<_, String>((None, None, antecedent, Some(consequent)))
            } else {
                let antecedent = antecedent_node
                    .map(|node| self.lower_simple_assertion_expr(path, node, "antecedent"))
                    .transpose()?;
                let consequent = if matches!(
                    self.kind(consequent_node),
                    NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. })
                ) {
                    self.lower_one_cycle_assertion(path, consequent_node, "consequent")?
                } else {
                    self.lower_simple_assertion_expr(path, consequent_node, "consequent")?
                };
                Ok::<_, String>((antecedent, Some(consequent), None, None))
            }
        })();
        self.sampled_clock = previous_clock;
        let (antecedent, consequent, antecedent_sequence, consequent_sequence) = expressions
            .map_err(|error| format!("{error} (assertion at {})", self.source_location(origin)))?;
        let (abort_condition, abort_reject, abort_sync) = match abort {
            Some((condition_node, reject, sync)) => (
                Some(self.lower_abort_condition(path, condition_node, clock_signal, posedge)?),
                reject,
                sync,
            ),
            None => (None, false, false),
        };
        Ok(PropertyParts {
            clock_signal,
            posedge,
            disable_signal: disable,
            antecedent,
            consequent,
            antecedent_sequence,
            consequent_sequence,
            overlapped,
            abort_condition,
            abort_reject,
            abort_sync,
        })
    }

    /// Find the declaration clock that leads an assertion expression. A
    /// property may omit its own clock when its first sequence element is a
    /// named sequence with an explicit event control; the owned Slang graph
    /// retains that event in the named body. This metadata-only walk resolves
    /// that inherited domain before lowering starts, without traversing native
    /// Slang AST pointers.
    fn infer_leading_assertion_clock(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<SampledClock>, String> {
        let mut seen = HashSet::new();
        self.infer_leading_assertion_clock_inner(path, node, &mut seen)
    }

    fn infer_leading_assertion_clock_inner(
        &mut self,
        path: &str,
        node: NodeId,
        seen: &mut HashSet<NodeId>,
    ) -> Result<Option<SampledClock>, String> {
        if !seen.insert(node) {
            return Ok(None);
        }
        let first = |this: &mut Self,
                     nodes: &[NodeId],
                     seen: &mut HashSet<NodeId>|
         -> Result<Option<SampledClock>, String> {
            for node in nodes {
                if let Some(clock) = this.infer_leading_assertion_clock_inner(path, *node, seen)? {
                    return Ok(Some(clock));
                }
            }
            Ok(None)
        };
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                signal, posedge, ..
            }) => {
                let signal = self.lower_assertion_signal(path, *signal, "clock")?;
                Ok(Some(SampledClock {
                    signal,
                    posedge: *posedge,
                    gate: None,
                }))
            }
            NodeKind::AssertionExpr(AssertionExprKind::Simple { expr, .. }) => {
                match self.kind(*expr) {
                    NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                        self.infer_leading_assertion_clock_inner(path, *body, seen)
                    }
                    NodeKind::AssertionExpr(_) => {
                        self.infer_leading_assertion_clock_inner(path, *expr, seen)
                    }
                    _ => Ok(None),
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, .. }) => {
                first(self, elements, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceWithMatch { expr, .. }) => {
                self.infer_leading_assertion_clock_inner(path, *expr, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::FirstMatch { sequence, .. }) => {
                self.infer_leading_assertion_clock_inner(path, *sequence, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Unary { expr, .. })
            | NodeKind::AssertionExpr(AssertionExprKind::StrongWeak { expr, .. })
            | NodeKind::AssertionExpr(AssertionExprKind::DisableIff { expr, .. }) => {
                self.infer_leading_assertion_clock_inner(path, *expr, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Abort { expr, .. }) => {
                self.infer_leading_assertion_clock_inner(path, *expr, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Binary { left, right, .. }) => {
                first(self, &[*left, *right], seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Conditional {
                if_expr, else_expr, ..
            }) => {
                let mut branches = vec![*if_expr];
                if let Some(else_expr) = else_expr {
                    branches.push(*else_expr);
                }
                first(self, &branches, seen)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Case {
                items,
                default_case,
                ..
            }) => {
                let mut branches = items.iter().map(|item| item.body).collect::<Vec<_>>();
                if let Some(default_case) = default_case {
                    branches.push(*default_case);
                }
                first(self, &branches, seen)
            }
            _ => Ok(None),
        }
    }

    fn lower_abort_condition(
        &mut self,
        path: &str,
        node: NodeId,
        signal: usize,
        posedge: bool,
    ) -> Result<IrExpr, String> {
        let previous_clock = self.sampled_clock;
        self.sampled_clock = Some(SampledClock {
            signal,
            posedge,
            gate: None,
        });
        let result = self.lower_boolean_expr(path, node);
        self.sampled_clock = previous_clock;
        let condition = result?;
        if condition.is_real() || !sampled_compatible(&condition) {
            return Err(format!(
                "accept_on/reject_on condition must be a packed sampled expression in `{path}`"
            ));
        }
        Ok(condition)
    }

    /// Lower a sampled-value clocking event argument (SV 16.9.3). Any legal
    /// event expression is admitted: a single edge of a direct packed signal
    /// is detected at that signal's write, and every other event list
    /// (`or`, `edge`, value changes, named or clocking-block events,
    /// expression edges) becomes an event clock.
    pub(super) fn lower_sampled_clock_event(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClockSource, String> {
        let NodeKind::Expr(ExprKind::ClockingEvent { specs }) = self.kind(node) else {
            return Err(format!(
                "sampled-value clock must be a clocking event in `{path}`"
            ));
        };
        let specs = specs.clone();
        self.sampled_clock_source(path, specs)
    }

    fn sampled_clock_source(
        &mut self,
        path: &str,
        specs: Vec<EventSpec>,
    ) -> Result<SampledClockSource, String> {
        if let [spec] = specs.as_slice() {
            if let Some((event, posedge, gate)) = flatten_clock_spec(spec) {
                if let Some(signal) = self.direct_sampled_clock_signal(path, event) {
                    return Ok(SampledClockSource::Edge(SampledClock {
                        signal,
                        posedge,
                        gate,
                    }));
                }
            }
        }
        if specs.is_empty() {
            return Err(format!("sampled-value clock has no event in `{path}`"));
        }
        Ok(SampledClockSource::Events(specs))
    }

    /// The active packed signal an edge names directly, if any; expression
    /// edges and other storage are waited on through an event clock instead.
    fn direct_sampled_clock_signal(&mut self, path: &str, event: NodeId) -> Option<usize> {
        let expression = self.lower_expr(path, event).ok()?;
        let IrExprKind::SigRead(signal) = expression.kind() else {
            return None;
        };
        matches!(
            self.model.signals.get(*signal),
            Some(IrSignal {
                fixed_default: None,
                ty: IrType::Packed { .. },
                omit: false,
                ..
            })
        )
        .then_some(*signal)
    }

    /// Resolve the single global clocking block used by the 2009 global
    /// sampled-value functions.
    pub(super) fn lower_global_sampled_clock(
        &mut self,
        path: &str,
    ) -> Result<SampledClockSource, String> {
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
        let specs = block.event_specs.clone();
        self.sampled_clock_source(path, specs)
    }

    /// The default clocking event visible in the current elaborated instance:
    /// a `default clocking` block or declaration of the nearest enclosing
    /// scope (SV 14.12), so identically named blocks in sibling instances stay
    /// independent.
    pub(super) fn default_sampled_clock_source(
        &mut self,
        path: &str,
    ) -> Result<Option<SampledClockSource>, String> {
        let Some(block) = self.default_clocking_block(self.inst) else {
            return Ok(None);
        };
        let Some(info) = self.db.clocking_block(block) else {
            return Ok(None);
        };
        let specs = info.event_specs.clone();
        self.sampled_clock_source(path, specs).map(Some)
    }

    /// The default clock of a concurrent assertion, which still needs one
    /// direct signal edge.
    pub(super) fn lower_default_sampled_clock(
        &mut self,
        path: &str,
    ) -> Result<Option<SampledClock>, String> {
        match self.default_sampled_clock_source(path)? {
            None => Ok(None),
            Some(SampledClockSource::Edge(clock)) => Ok(Some(clock)),
            Some(SampledClockSource::Events(_)) => Err(format!(
                "concurrent assertion default clocking block must use one direct edge event in `{path}`"
            )),
        }
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

    /// Intern a sampled-value clock. `gate` is the `$past` gating
    /// expression; an edge's own `iff` joins it, and both read current values
    /// when the clock occurs (`ev iff expression2`, SV 16.9.3). Identical
    /// clocks share one entry and therefore one set of ticks.
    pub(super) fn intern_sampled_clock(
        &mut self,
        path: &str,
        source: SampledClockSource,
        gate: Option<IrExpr>,
    ) -> Result<usize, String> {
        match source {
            SampledClockSource::Edge(clock) => {
                let mut gate = gate;
                if let Some(node) = clock.gate {
                    let iff = self.lower_boolean_expr(path, node)?;
                    if iff.is_real() || !sampled_compatible(&iff) {
                        return Err(format!(
                            "sampled clock `iff` condition must be a static packed expression in `{path}`"
                        ));
                    }
                    gate = Some(match gate {
                        Some(gate) => IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::LogAnd,
                                a: Box::new(iff),
                                b: Box::new(gate),
                            },
                            1,
                            false,
                            None,
                        ),
                        None => iff,
                    });
                }
                let kind = IrSampledClockKind::Edge {
                    signal: clock.signal,
                    posedge: clock.posedge,
                };
                let key = format!("{kind:?} {gate:?}");
                if let Some(index) = self.sampled_clock_keys.get(&key) {
                    return Ok(*index);
                }
                let index = self.model.sampled_clocks.len();
                self.model
                    .sampled_clocks
                    .push(IrSampledClock::new(kind, gate));
                self.sampled_clock_keys.insert(key, index);
                Ok(index)
            }
            SampledClockSource::Events(specs) => {
                let inst = self.inst;
                let (specs, pre_fns) = {
                    let mut ctx = EmitCtx::new(self, path.to_owned(), inst, "0", None, None, false);
                    let specs = ctx.lower_event_specs(&specs)?;
                    (specs, std::mem::take(&mut ctx.pre_fns))
                };
                let key = format!("{specs:?} {gate:?}");
                if let Some(index) = self.sampled_clock_keys.get(&key) {
                    return Ok(*index);
                }
                let index = self.model.sampled_clocks.len();
                self.model
                    .sampled_clocks
                    .push(IrSampledClock::new(IrSampledClockKind::Event, gate));
                self.sampled_clock_keys.insert(key, index);
                self.sampled_event_clocks.push(PendingSampledEventClock {
                    clock: index,
                    path: path.to_owned(),
                    specs,
                    pre_fns,
                });
                Ok(index)
            }
        }
    }

    /// Intern the history of `sample` on `clock`, retaining the deepest
    /// `$past` tick count any sharing call reads (at least 1 for the value
    /// change functions).
    pub(super) fn intern_sampled_domain(
        &mut self,
        clock: usize,
        sample: IrExpr,
        history_ticks: u64,
    ) -> usize {
        let history_ticks = history_ticks.max(1);
        let key = format!("{clock} {sample:?}");
        if let Some(index) = self.sampled_domain_keys.get(&key) {
            let domain = &mut self.model.sampled_domains[*index];
            domain.history_ticks = domain.history_ticks.max(history_ticks);
            return *index;
        }
        let index = self.model.sampled_domains.len();
        self.model
            .sampled_domains
            .push(IrSampledDomain::new(clock, sample, history_ticks));
        self.sampled_domain_keys.insert(key, index);
        index
    }

    /// One looping process per event clock: wait on the clocking event, then
    /// record the tick. Returns the process names so they start first.
    pub(super) fn emit_sampled_clock_processes(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        for pending in std::mem::take(&mut self.sampled_event_clocks) {
            let name = self.new_fn_name(&pending.path, &format!("sampled_clock_{}", pending.clock));
            self.model.processes.push(IrProcess::new_with_origin(
                name.clone(),
                format!("{}.sampled_clock", pending.path),
                IrShape::Loop,
                pending.pre_fns,
                vec![
                    IrStmt::WaitEvents {
                        specs: pending.specs,
                    },
                    IrStmt::SampledClockTick {
                        clock: pending.clock,
                    },
                ],
                crate::sim::semantic::Origin::Synthetic {
                    reason: format!("sampled-value clock {} in {}", pending.clock, pending.path),
                },
            ));
            names.push(name);
        }
        names
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
            fixed_default: None,
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
        let NodeKind::AssertionExpr(AssertionExprKind::Simple {
            expr,
            repeated,
            repetition,
        }) = self.kind(node)
        else {
            return Err(format!(
                "concurrent assertion {role} must be a simple sequence at {path}"
            ));
        };
        if *repeated || repetition.is_some() {
            return Err(format!(
                "sequence repetition requires automaton lowering in concurrent assertion {role} at {path}"
            ));
        }
        let expression = self.lower_boolean_expr(path, *expr)?;
        if expression.is_real() || !sampled_compatible(&expression) {
            return Err(format!(
                "unsupported sampled {role} expression in concurrent assertion at {path}"
            ));
        }
        Ok(expression)
    }

    fn sequence_requires_engine(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repetition,
                repeated,
            }) => {
                repetition.is_some()
                    || *repeated
                    || match self.kind(*expr) {
                        NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                            self.sequence_requires_engine(*body)
                        }
                        // A sequence `.matched` endpoint is a sequence
                        // fragment, not an ordinary sampled Boolean. Keep it
                        // in the NFA so multi-cycle receivers retain their
                        // endpoint timing and local-attempt state.
                        NodeKind::MethodCall { name, .. } if name == "matched" => true,
                        _ => false,
                    }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { .. })
            | NodeKind::AssertionExpr(AssertionExprKind::SequenceWithMatch { .. })
            | NodeKind::AssertionExpr(AssertionExprKind::Unary { .. })
            | NodeKind::AssertionExpr(AssertionExprKind::Binary { .. })
            | NodeKind::AssertionExpr(AssertionExprKind::FirstMatch { .. }) => true,
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. })
            | NodeKind::AssertionExpr(AssertionExprKind::DisableIff { expr, .. }) => {
                self.sequence_requires_engine(*expr)
            }
            _ => true,
        }
    }

    fn lower_sequence(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<IrSequence, String> {
        let previous_locals = self.assertion_local_bindings.take();
        let previous_initializers = self.assertion_local_initializers.take();
        self.assertion_local_bindings = Some(HashMap::new());
        self.assertion_local_initializers = Some(HashMap::new());
        let mut builder = SequenceBuilder::new();
        let result = self
            .lower_sequence_fragment(path, node, &mut builder, role)
            .and_then(|fragment| {
                let mut local_bindings = self
                    .assertion_local_bindings
                    .as_ref()
                    .into_iter()
                    .flat_map(|bindings| bindings.iter().map(|(node, binding)| (*node, *binding)))
                    .collect::<Vec<_>>();
                local_bindings.sort_by_key(|(_, binding)| binding.slot);
                let locals = local_bindings
                    .iter()
                    .map(|(node, binding)| IrSequenceLocal {
                        declaration: node.index() as u64 + 1,
                        width: binding.width,
                        signed: binding.signed,
                        two_state: binding.two_state,
                    })
                    .collect();
                let mut initializers = Vec::new();
                if let Some(initializer_map) = self.assertion_local_initializers.as_ref() {
                    for (target, initializer) in initializer_map {
                        let Some(binding) = self
                            .assertion_local_bindings
                            .as_ref()
                            .and_then(|bindings| bindings.get(target))
                        else {
                            return Err("assertion local initializer has no local slot".to_owned());
                        };
                        initializers.push((binding.slot, initializer.clone()));
                    }
                }
                initializers.sort_by_key(|(slot, _)| *slot);
                let initializer_slots = initializers.iter().map(|(slot, _)| *slot).collect();
                let initializers = initializers
                    .into_iter()
                    .map(|(_, initializer)| initializer)
                    .collect();
                builder.finish(fragment, locals, initializers, initializer_slots)
            });
        self.assertion_local_bindings = previous_locals;
        self.assertion_local_initializers = previous_initializers;
        result
    }

    fn lower_nested_clock(
        &mut self,
        path: &str,
        signal: NodeId,
        posedge: bool,
    ) -> Result<SampledClock, String> {
        let signal = self.lower_assertion_signal(path, signal, "clock")?;
        // A direct clocking wrapper is allowed to switch the sampled domain
        // for a non-empty sequence segment. The Slang-owned analyzer has
        // already rejected illegal empty/ambiguous multiclocked forms; the
        // runtime receives the explicit edge on each generated transition.
        Ok(SampledClock {
            signal,
            posedge,
            gate: None,
        })
    }

    fn lower_sequence_fragment(
        &mut self,
        path: &str,
        node: NodeId,
        builder: &mut SequenceBuilder,
        role: &str,
    ) -> Result<SequenceFragment, String> {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated,
                repetition,
            }) => {
                if !*repeated && repetition.is_none() {
                    let matched_receiver = match self.kind(*expr) {
                        NodeKind::MethodCall {
                            name: method,
                            receiver: Some(receiver),
                            ..
                        } if method == "matched" => Some(*receiver),
                        _ => None,
                    };
                    if let Some(receiver) = matched_receiver {
                        let capture_at_attempt_entry =
                            self.assertion_instance_depth == 0 && builder.next_state == 0;
                        let result = self.lower_assertion_instance(
                            receiver,
                            capture_at_attempt_entry,
                            |this, body| {
                                this.lower_sequence_fragment(path, body, builder, role)
                            },
                        )?;
                        return result.ok_or_else(|| {
                            format!(
                                "sequence `.matched` receiver is not an assertion instance in {role} at {path}"
                            )
                        });
                    }
                    let capture_at_attempt_entry =
                        self.assertion_instance_depth == 0 && builder.next_state == 0;
                    if let Some(result) = self.lower_assertion_instance(
                        *expr,
                        capture_at_attempt_entry,
                        |this, body| this.lower_sequence_fragment(path, body, builder, role),
                    )? {
                        return Ok(result);
                    }
                }
                if let (Some(repetition), NodeKind::Expr(ExprKind::AssertionInstance { body, .. })) =
                    (repetition, self.kind(*expr))
                {
                    if !self.one_cycle_form(*body) {
                        // A repeated multi-cycle named sequence instance.
                        let instance = *expr;
                        let mut lower_copy = |this: &mut Self, builder: &mut SequenceBuilder| {
                            this.lower_assertion_instance(instance, false, |this, body| {
                                this.lower_sequence_fragment(path, body, builder, role)
                            })?
                            .ok_or_else(|| "assertion instance body is missing".to_owned())
                        };
                        let first = lower_copy(self, builder)?;
                        return self.lower_sequence_repetition(
                            path,
                            builder,
                            first,
                            repetition,
                            &mut lower_copy,
                        );
                    }
                }
                let atom = self.lower_sequence_atom(path, *expr, role)?;
                if let Some(repetition) = repetition {
                    if !*repeated {
                        return Err(format!(
                            "invalid sequence repetition metadata in concurrent assertion {role} at {path}"
                        ));
                    }
                    self.lower_repetition(builder, atom, repetition, self.sampled_clock)
                } else {
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    let clock = self.sampled_clock;
                    builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                    Ok(SequenceFragment {
                        empty: false,
                        start,
                        accept,
                        leading_clock: clock,
                        trailing_clock: clock,
                    })
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, delays }) => {
                let mut result: Option<SequenceFragment> = None;
                for (index, element) in elements.iter().enumerate() {
                    let previous_clock = self.sampled_clock;
                    self.sampled_clock = result.as_ref().and_then(|fragment| fragment.trailing_clock)
                        .or(previous_clock);
                    let fragment = self.lower_sequence_fragment(path, *element, builder, role);
                    self.sampled_clock = previous_clock;
                    let fragment = fragment?;
                    let delay = delays.get(index).cloned().unwrap_or_else(zero_range);
                    result = Some(if let Some(left) = result {
                        builder.concatenate(left, fragment, delay)?
                    } else if delay.min == 0 && delay.max == Some(0) {
                        fragment
                    } else {
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        builder.epsilon_with_clock(start, accept, zero_range(), self.sampled_clock)?;
                        builder.concatenate(SequenceFragment { empty: false, start, accept,
                            leading_clock: self.sampled_clock, trailing_clock: self.sampled_clock },
                            fragment, delay)?
                    });
                }
                if let Some(result) = result { Ok(result) } else {
                    Ok(SequenceFragment { empty: true, start: builder.state()?, accept: builder.state()?,
                        leading_clock: self.sampled_clock, trailing_clock: self.sampled_clock })
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceWithMatch {
                expr,
                match_items,
                repetition,
                ..
            }) => {
                let fragment = if matches!(self.kind(*expr), NodeKind::AssertionExpr(_)) {
                    self.lower_sequence_fragment(path, *expr, builder, role)?
                } else {
                    let atom = self.lower_sequence_atom(path, *expr, role)?;
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    let clock = self.sampled_clock;
                    builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                    SequenceFragment {
                        empty: false,
                        start,
                        accept,
                        leading_clock: clock,
                        trailing_clock: clock,
                    }
                };
                let previous_clock = self.sampled_clock;
                self.sampled_clock = fragment.trailing_clock.or(previous_clock);
                let lowered_match_items = match_items
                    .iter()
                    .map(|item| self.lower_assertion_match_item(path, *item, role))
                    .collect::<Result<Vec<_>, _>>();
                self.sampled_clock = previous_clock;
                let lowered_match_items = lowered_match_items?;
                if fragment.empty && !lowered_match_items.is_empty() {
                    return Err("sequence match items cannot attach to an empty match".to_owned());
                }
                if !lowered_match_items.is_empty() {
                    builder.attach_match_items(fragment.accept, lowered_match_items)?;
                }
                if let Some(repetition) = repetition {
                    if builder
                        .transitions
                        .iter()
                        .any(|transition| transition.to == fragment.accept
                            && transition.match_count != 0)
                    {
                        return Err(format!(
                            "repeated sequence match items are not supported in concurrent assertion {role} at {path}"
                        ));
                    }
                    // A sequence-with-match repetition is represented by a
                    // direct repeated body in Slang's owned graph. Match
                    // items run when the enclosing graph reaches its endpoint.
                    // A single-atom body keeps the Boolean repetition
                    // automaton; any other body is repeated as a sequence.
                    if let Ok(atom) = self.sequence_fragment_atom(builder, &fragment) {
                        return self.lower_repetition(
                            builder,
                            atom,
                            repetition,
                            fragment.trailing_clock.or(self.sampled_clock),
                        );
                    }
                    let body = *expr;
                    self.lower_sequence_repetition(
                        path,
                        builder,
                        fragment,
                        repetition,
                        &mut |this, builder| {
                            this.lower_sequence_fragment(path, body, builder, role)
                        },
                    )
                } else {
                    Ok(fragment)
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::FirstMatch {
                sequence,
                match_items,
            }) => {
                let fragment = self.lower_sequence_fragment(path, *sequence, builder, role)?;
                let previous_clock = self.sampled_clock;
                self.sampled_clock = fragment.trailing_clock.or(previous_clock);
                let lowered = match_items
                    .iter()
                    .map(|item| self.lower_assertion_match_item(path, *item, role))
                    .collect::<Result<Vec<_>, _>>();
                self.sampled_clock = previous_clock;
                let lowered = lowered?;
                if fragment.empty && !lowered.is_empty() {
                    return Err("sequence match items cannot attach to an empty match".to_owned());
                }
                builder.attach_match_items(fragment.accept, lowered)?;
                builder.first_match(fragment)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                expr,
                ranged: false,
                range: None,
            }) => {
                let atom = self.lower_one_cycle_assertion(path, *expr, role)?;
                let start = builder.state()?;
                let accept = builder.state()?;
                let clock = self.sampled_clock;
                builder.edge_with_clock(
                    start,
                    accept,
                    zero_range(),
                    Some(IrExpr::new(
                        IrExprKind::Un {
                            op: IrUnOp::LogNot,
                            a: Box::new(atom),
                        },
                        1,
                        false,
                        None,
                    )),
                    clock,
                )?;
                Ok(SequenceFragment {
                    empty: false,
                    start,
                    accept,
                    leading_clock: clock,
                    trailing_clock: clock,
                })
            }
            NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. }) => {
                let atom = self.lower_one_cycle_assertion(path, node, role)?;
                let start = builder.state()?;
                let accept = builder.state()?;
                let clock = self.sampled_clock;
                builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                Ok(SequenceFragment {
                    empty: false,
                    start,
                    accept,
                    leading_clock: clock,
                    trailing_clock: clock,
                })
            }
            NodeKind::AssertionExpr(AssertionExprKind::Binary { op, left, right }) => {
                match op {
                    AssertionBinaryOp::Or => {
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        let left = self.lower_sequence_fragment(path, *left, builder, role)?;
                        let right = self.lower_sequence_fragment(path, *right, builder, role)?;
                        builder.epsilon_with_clock(
                            start,
                            left.start,
                            zero_range(),
                            left.leading_clock.or(self.sampled_clock),
                        )?;
                        builder.epsilon_with_clock(
                            start,
                            right.start,
                            zero_range(),
                            right.leading_clock.or(self.sampled_clock),
                        )?;
                        builder.epsilon_with_clock(
                            left.accept,
                            accept,
                            zero_range(),
                            left.trailing_clock.or(self.sampled_clock),
                        )?;
                        builder.epsilon_with_clock(
                            right.accept,
                            accept,
                            zero_range(),
                            right.trailing_clock.or(self.sampled_clock),
                        )?;
                        Ok(SequenceFragment {
                            empty: left.empty || right.empty,
                            start,
                            accept,
                            leading_clock: left
                                .leading_clock
                                .or(right.leading_clock)
                                .or(self.sampled_clock),
                            trailing_clock: left
                                .trailing_clock
                                .or(right.trailing_clock)
                                .or(self.sampled_clock),
                        })
                    }
                    AssertionBinaryOp::And
                    | AssertionBinaryOp::Intersect
                    | AssertionBinaryOp::Throughout
                    | AssertionBinaryOp::Within
                        if self.one_cycle_form(*left) && self.one_cycle_form(*right) =>
                    {
                        // Every match of a one-cycle operand has length one,
                        // so all four operators reduce to one sampled `&&`
                        // atom with no runtime join.
                        let left = self.lower_one_cycle_assertion(path, *left, role)?;
                        let right = self.lower_one_cycle_assertion(path, *right, role)?;
                        let atom = IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::LogAnd,
                                a: Box::new(left),
                                b: Box::new(right),
                            },
                            1,
                            false,
                            None,
                        );
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        let clock = self.sampled_clock;
                        builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                        Ok(SequenceFragment {
                            empty: false,
                            start,
                            accept,
                            leading_clock: clock,
                            trailing_clock: clock,
                        })
                    }
                    AssertionBinaryOp::And
                    | AssertionBinaryOp::Intersect
                    | AssertionBinaryOp::Throughout
                    | AssertionBinaryOp::Within => {
                        let op = *op;
                        let (left, right) = (*left, *right);
                        self.lower_sequence_join(path, node, op, left, right, builder, role)
                    }
                    AssertionBinaryOp::Iff | AssertionBinaryOp::Implies => {
                        let atom = self.lower_one_cycle_assertion(path, node, role)?;
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        let clock = self.sampled_clock;
                        builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                        Ok(SequenceFragment {
                            empty: false,
                            start,
                            accept,
                            leading_clock: clock,
                            trailing_clock: clock,
                        })
                    }
                    unsupported => Err(format!(
                        "assertion binary operator {unsupported:?} is not supported in concurrent assertion {role} at {} ({path})",
                        self.source_location(node)
                    )),
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                signal,
                posedge,
                expr,
                ..
            }) => {
                let clock = self.lower_nested_clock(path, *signal, *posedge)?;
                let previous_clock = self.sampled_clock;
                self.sampled_clock = Some(clock);
                let result = self.lower_sequence_fragment(path, *expr, builder, role);
                self.sampled_clock = previous_clock;
                result
            }
            NodeKind::AssertionExpr(AssertionExprKind::DisableIff { .. }) => Err(format!(
                "nested disable iff is not supported in concurrent assertion {role} at {} ({path})",
                self.source_location(node)
            )),
            NodeKind::AssertionExpr(kind) => Err(format!(
                "assertion sequence form {kind:?} is not supported in concurrent assertion {role} at {path}"
            )),
            _ => Err(format!(
                "concurrent assertion {role} is not a sequence expression at {path}"
            )),
        }
    }

    /// Whether every match of `node` has length one, so that it can be
    /// lowered as one sampled Boolean by [`Self::lower_one_cycle_assertion`].
    /// This is a static per-site choice: such operands never need a runtime
    /// join thread.
    fn one_cycle_form(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                    self.one_cycle_form(*body)
                }
                NodeKind::MethodCall { name, .. } if name == "matched" => false,
                _ => true,
            },
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) => {
                self.one_cycle_form(*expr)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                expr,
                ranged: false,
                range: None,
            }) => self.one_cycle_form(*expr),
            NodeKind::AssertionExpr(AssertionExprKind::Binary { op, left, right }) => {
                matches!(
                    op,
                    AssertionBinaryOp::And
                        | AssertionBinaryOp::Or
                        | AssertionBinaryOp::Iff
                        | AssertionBinaryOp::Implies
                ) && self.one_cycle_form(*left)
                    && self.one_cycle_form(*right)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Conditional {
                if_expr,
                else_expr: Some(else_expr),
                ..
            }) => self.one_cycle_form(*if_expr) && self.one_cycle_form(*else_expr),
            _ => false,
        }
    }

    /// Lower `and`, `intersect`, `throughout` and `within` with multi-cycle
    /// operands to one runtime join (IEEE 1800-2009 16.9.5-16.9.10 and the
    /// Annex F reductions `e throughout s = e[*0:$] intersect s` and
    /// `s1 within s2 = (1[*0:$] ##1 s1 ##1 1[*0:$]) intersect s2`).
    #[allow(clippy::too_many_arguments)]
    fn lower_sequence_join(
        &mut self,
        path: &str,
        node: NodeId,
        op: AssertionBinaryOp,
        left: NodeId,
        right: NodeId,
        builder: &mut SequenceBuilder,
        role: &str,
    ) -> Result<SequenceFragment, String> {
        let items_before = builder.match_items.len();
        let clock = self.sampled_clock;
        let left = match op {
            AssertionBinaryOp::Throughout => {
                let atom = self.lower_one_cycle_assertion(path, left, role)?;
                self.lower_repetition(builder, atom, &unbounded_star(), clock)?
            }
            AssertionBinaryOp::Within => {
                let inner = self.lower_sequence_fragment(path, left, builder, role)?;
                let unit = AssertionRange {
                    min: 1,
                    max: Some(1),
                };
                let lead = self.lower_repetition(builder, true_atom(), &unbounded_star(), clock)?;
                let lead = builder.concatenate(lead, inner, unit.clone())?;
                let tail = self.lower_repetition(builder, true_atom(), &unbounded_star(), clock)?;
                builder.concatenate(lead, tail, unit)?
            }
            _ => self.lower_sequence_fragment(path, left, builder, role)?,
        };
        let right = self.lower_sequence_fragment(path, right, builder, role)?;
        if builder.match_items.len() != items_before {
            let name = match op {
                AssertionBinaryOp::And => "and",
                AssertionBinaryOp::Intersect => "intersect",
                AssertionBinaryOp::Throughout => "throughout",
                _ => "within",
            };
            return Err(format!(
                "local variable assignments inside `{name}` operands are not supported in concurrent assertion {role} at {} ({path})",
                self.source_location(node)
            ));
        }
        let kind = if op == AssertionBinaryOp::And {
            IrSequenceJoinKind::And
        } else {
            IrSequenceJoinKind::Intersect
        };
        builder.join(kind, left, right).map_err(|error| {
            format!(
                "{error} in concurrent assertion {role} at {} ({path})",
                self.source_location(node)
            )
        })
    }

    /// Consecutive repetition of a general sequence. The body is lowered
    /// once per required copy (`first` is the already-lowered first copy);
    /// copies are chained with `##1`, copies from the lower bound on may
    /// end the repetition, and an unbounded upper bound loops the last copy
    /// back with `##1`. A body admitting the empty word lets every copy end
    /// the repetition, because empty iterations collapse (Annex F `R[*0]`).
    fn lower_sequence_repetition(
        &mut self,
        path: &str,
        builder: &mut SequenceBuilder,
        first: SequenceFragment,
        repetition: &AssertionRepetition,
        relower: &mut dyn FnMut(
            &mut Self,
            &mut SequenceBuilder,
        ) -> Result<SequenceFragment, String>,
    ) -> Result<SequenceFragment, String> {
        if repetition.kind != AssertionRepetitionKind::Consecutive {
            return Err(format!(
                "goto and nonconsecutive repetition apply only to Boolean operands at {path}"
            ));
        }
        let min = repetition.range.min;
        let max = repetition.range.max;
        if max.is_some_and(|max| max < min) {
            return Err("sequence repetition range is inverted".to_owned());
        }
        let start = builder.state()?;
        let accept = builder.state()?;
        let copies = max.unwrap_or(min.max(1));
        let unit = AssertionRange {
            min: 1,
            max: Some(1),
        };
        let mut previous: Option<SequenceFragment> = None;
        let mut current = first;
        for copy in 1..=copies {
            if copy > 1 {
                current = relower(self, builder)?;
            }
            match previous {
                None => builder.epsilon_with_clock(
                    start,
                    current.start,
                    zero_range(),
                    current.leading_clock,
                )?,
                Some(previous) => builder.epsilon_with_clock(
                    previous.accept,
                    current.start,
                    unit.clone(),
                    current.leading_clock.or(previous.trailing_clock),
                )?,
            }
            if copy >= min || first.empty {
                builder.epsilon_with_clock(
                    current.accept,
                    accept,
                    zero_range(),
                    current.trailing_clock,
                )?;
            }
            previous = Some(current);
        }
        if max.is_none() {
            builder.epsilon_with_clock(
                current.accept,
                current.start,
                unit,
                current.trailing_clock,
            )?;
        }
        Ok(SequenceFragment {
            empty: min == 0 || first.empty,
            start,
            accept,
            leading_clock: first.leading_clock,
            trailing_clock: first.trailing_clock,
        })
    }

    fn lower_sequence_atom(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<IrExpr, String> {
        let capture_at_attempt_entry = self.assertion_instance_depth == 0;
        if let Some(result) =
            self.lower_assertion_instance(node, capture_at_attempt_entry, |this, body| {
                this.lower_one_cycle_assertion(path, body, role)
            })?
        {
            return Ok(result);
        }
        let expression = self.lower_boolean_expr(path, node)?;
        if expression.is_real() || !sampled_compatible(&expression) {
            return Err(format!(
                "unsupported sampled {role} expression in concurrent assertion at {path}"
            ));
        }
        Ok(expression)
    }

    /// Lower a property expression that is known to complete on the current
    /// sampled tick. This is deliberately narrower than the full SVA
    /// property algebra: temporal implications and unbounded operators need
    /// attempt-level state and remain explicit fail-closed boundaries. The
    /// admitted boolean forms are useful for named property/sequence bodies,
    /// `not`, and one-cycle Boolean compositions already represented by the
    /// H22 automaton.
    fn lower_one_cycle_assertion(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<IrExpr, String> {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => self.lower_sequence_atom(path, *expr, role).map(property_truth),
            NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                signal,
                posedge,
                expr,
                ..
            }) => {
                let clock = self.lower_nested_clock(path, *signal, *posedge)?;
                let previous_clock = self.sampled_clock;
                self.sampled_clock = Some(clock);
                let result = self.lower_one_cycle_assertion(path, *expr, role);
                self.sampled_clock = previous_clock;
                result
            }
            NodeKind::AssertionExpr(AssertionExprKind::DisableIff { .. }) => Err(format!(
                "nested disable iff is not supported in concurrent assertion {role} at {} ({path})",
                self.source_location(node)
            )),
            NodeKind::AssertionExpr(AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                expr,
                ranged: false,
                range: None,
            }) => {
                let value = self.lower_one_cycle_assertion(path, *expr, role)?;
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(value),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::AssertionExpr(AssertionExprKind::Binary { op, left, right })
                if matches!(
                    op,
                    AssertionBinaryOp::And
                        | AssertionBinaryOp::Or
                        | AssertionBinaryOp::Iff
                        | AssertionBinaryOp::Implies
                ) =>
            {
                let left = self.lower_one_cycle_assertion(path, *left, role)?;
                let right = self.lower_one_cycle_assertion(path, *right, role)?;
                let op = match op {
                    AssertionBinaryOp::And => IrBinOp::LogAnd,
                    AssertionBinaryOp::Or => IrBinOp::LogOr,
                    AssertionBinaryOp::Iff => IrBinOp::LogEquiv,
                    AssertionBinaryOp::Implies => IrBinOp::LogImpl,
                    _ => {
                        return Err(format!(
                            "property expression operator {op:?} is not a one-cycle boolean form in {role} at {} ({path})",
                            self.source_location(node)
                        ));
                    }
                };
                Ok(IrExpr::new(
                    IrExprKind::Bin {
                        op,
                        a: Box::new(left),
                        b: Box::new(right),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::AssertionExpr(AssertionExprKind::Conditional {
                condition,
                if_expr,
                else_expr,
            }) => {
                let condition = property_truth(self.lower_boolean_expr(path, *condition)?);
                let if_value = self.lower_one_cycle_assertion(path, *if_expr, role)?;
                let else_value = else_expr
                    .map(|expr| self.lower_one_cycle_assertion(path, expr, role))
                    .transpose()?
                    .ok_or_else(|| {
                        format!(
                            "conditional property without an else branch is not supported in {role} at {path}"
                        )
                    })?;
                if condition.is_real()
                    || if_value.is_real()
                    || else_value.is_real()
                    || !sampled_compatible(&condition)
                {
                    return Err(format!(
                        "conditional property requires packed sampled expressions in {role} at {path}"
                    ));
                }
                let width = if_value.width.max(else_value.width);
                let signed = if_value.signed && else_value.signed;
                let if_value = IrExpr::resize_to(if_value, width, signed);
                let else_value = IrExpr::resize_to(else_value, width, signed);
                Ok(IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(condition),
                        a: Box::new(if_value),
                        b: Box::new(else_value),
                    },
                    width,
                    signed,
                    None,
                ))
            }
            other => Err(format!(
                "property expression {other:?} is not a one-cycle boolean form in {role} at {} ({path})",
                self.source_location(node)
            )),
        }
    }

    fn sequence_fragment_atom(
        &self,
        builder: &SequenceBuilder,
        fragment: &SequenceFragment,
    ) -> Result<IrExpr, String> {
        let mut candidate = None;
        for transition in &builder.transitions {
            if transition.from == fragment.start
                && transition.to == fragment.accept
                && transition.delay
                    == (IrSequenceRange {
                        min: 0,
                        max: Some(0),
                    })
            {
                if let Some(atom) = transition.atom {
                    candidate = builder.atoms.get(atom as usize).cloned();
                }
            }
        }
        candidate
            .ok_or_else(|| "sequence repetition requires a single sampled sequence atom".to_owned())
    }

    /// Match items are evaluated at their lowered sequence endpoint, in source
    /// order. Their bounded form is an assignment/increment to a local
    /// assertion variable or a subroutine call. A function result is discarded
    /// at this statement position, while arbitrary global writes remain
    /// fail-closed because they would cross the sampled callback's ownership
    /// boundary.
    fn lower_assertion_match_item(
        &mut self,
        path: &str,
        node: NodeId,
        role: &str,
    ) -> Result<IrExpr, String> {
        let previous = self.lowering_assertion_match_item;
        self.lowering_assertion_match_item = true;
        let expression = self.lower_expr(path, node);
        self.lowering_assertion_match_item = previous;
        let expression = expression?;
        match expression.kind() {
            IrExprKind::Mutation(mutation)
                if matches!(
                    &mutation.lhs,
                    IrLhs::WholeRef { addr, .. }
                        if addr.starts_with("llg_sequence_local_addr(")
                ) =>
            {
                Ok(expression)
            }
            IrExprKind::CallFn(_) => Ok(expression),
            IrExprKind::Mutation(_) => Err(format!(
                "sequence match item must assign a local assertion variable in {role} at {path}"
            )),
            _ => Err(format!(
                "unsupported sequence match item in concurrent assertion {role} at {path}"
            )),
        }
    }

    fn lower_repetition(
        &mut self,
        builder: &mut SequenceBuilder,
        atom: IrExpr,
        repetition: &AssertionRepetition,
        clock: Option<SampledClock>,
    ) -> Result<SequenceFragment, String> {
        let min = repetition.range.min;
        let max = repetition.range.max;
        if max.is_some_and(|max| max < min) {
            return Err("sequence repetition range is inverted".to_owned());
        }
        let start = builder.state()?;
        let accept = builder.state()?;
        let negative = IrExpr::new(
            IrExprKind::Un {
                op: IrUnOp::LogNot,
                a: Box::new(atom.clone()),
            },
            1,
            false,
            None,
        );
        let unit = AssertionRange {
            min: 1,
            max: Some(1),
        };
        let mut cursor = start;
        let mut count = 0u32;
        // Positive paths only. An empty match is carried by the fragment,
        // never by an ordinary current-tick epsilon transition.
        let finite = max.unwrap_or(min.max(1));
        while count < finite {
            let next = builder.state()?;
            let delay = if count == 0 {
                zero_range()
            } else {
                unit.clone()
            };
            repetition_step(
                builder,
                cursor,
                next,
                delay,
                &atom,
                &negative,
                repetition.kind,
                clock,
            )?;
            count += 1;
            cursor = next;
            if count >= min {
                repetition_accept(builder, cursor, accept, &negative, repetition.kind, clock)?;
            }
        }
        if max.is_none() {
            repetition_step(
                builder,
                cursor,
                cursor,
                unit.clone(),
                &atom,
                &negative,
                repetition.kind,
                clock,
            )?;
        }
        if min == 0 && repetition.kind == AssertionRepetitionKind::Nonconsecutive {
            let tail = builder.state()?;
            builder.edge_with_clock(start, tail, zero_range(), Some(negative.clone()), clock)?;
            builder.edge_with_clock(tail, tail, unit, Some(negative), clock)?;
            builder.epsilon_with_clock(tail, accept, zero_range(), clock)?;
        }
        Ok(SequenceFragment {
            empty: min == 0,
            start,
            accept,
            leading_clock: clock,
            trailing_clock: clock,
        })
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
        let writes = self.ir_process_writes(self.collect_process_writes(statement)?);
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
        // Sequence-local reads are backed by the private attempt frame and
        // are sampled atom values just like signal reads. Other evaluator
        // locals (for example a procedural capture) must not leak into an
        // assertion's sampled expression.
        IrExprKind::LocalRead(name) if name.starts_with("llg_sequence_local_read(") => true,
        IrExprKind::Bin { a, b, .. } => sampled_compatible(a) && sampled_compatible(b),
        IrExprKind::Un { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::StreamToFixed { a } => sampled_compatible(a),
        IrExprKind::Mux { sel, a, b }
        | IrExprKind::ArrayMux { sel, a, b, .. }
        | IrExprKind::StructMux { sel, a, b, .. } => {
            sampled_compatible(sel) && sampled_compatible(a) && sampled_compatible(b)
        }
        IrExprKind::UdpEval { inputs: parts, .. }
        | IrExprKind::Predicate { clauses: parts }
        | IrExprKind::Concat { parts }
        | IrExprKind::Replicate { parts, .. } => parts.iter().all(sampled_compatible),
        // Pattern bindings are procedural state updates and are not sampled
        // assertion expressions. Conditional pattern support is confined to
        // executable procedural predicates.
        IrExprKind::Pattern(_) => false,
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
                    crate::sim::ir::IrInsideItem::Container { .. }
                    | crate::sim::ir::IrInsideItem::Cells(_) => false,
                    crate::sim::ir::IrInsideItem::FixedArray { value, .. } => {
                        sampled_compatible(value)
                    }
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
        IrExprKind::RealBin { a, b, .. } => sampled_compatible(a) && sampled_compatible(b),
        IrExprKind::RealUn { a, .. } | IrExprKind::CastToReal { a, .. } => sampled_compatible(a),
        IrExprKind::SysFunc(function) => match &**function {
            // A history call reads its domain, whose sample expression was
            // admitted when the domain was created (including a real
            // argument's 64-bit image); only `$sampled` evaluates in place.
            crate::sim::ir::IrSysFunc::Sampled(call) => {
                call.domain.is_some() || sampled_compatible(&call.argument)
            }
            _ => false,
        },
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

/// A Boolean sequence atom succeeds only when its expression is logically true.
/// Normalize before property composition: property `not X` succeeds, unlike `!X`.
fn property_truth(value: IrExpr) -> IrExpr {
    let negated = IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::LogNot,
            a: Box::new(value),
        },
        1,
        false,
        None,
    );
    let logical = IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::LogNot,
            a: Box::new(negated),
        },
        1,
        false,
        None,
    );
    IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::CaseEq,
            a: Box::new(logical),
            b: Box::new(IrExpr::resize_to(lhs_integer_expr(1), 1, false)),
        },
        1,
        false,
        None,
    )
}

/// Signals a sampled expression reads (the forms [`sampled_compatible`]
/// admits). Each one needs a Preponed snapshot outside an assertion.
pub(super) fn sampled_signal_reads(
    expression: &IrExpr,
    reads: &mut std::collections::BTreeSet<usize>,
) {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression.kind() {
            IrExprKind::SigRead(signal) => {
                reads.insert(*signal);
            }
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                pending.extend([&**a, &**b]);
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a }
            | IrExprKind::StreamToFixed { a }
            | IrExprKind::BitStreamCast { a, .. } => pending.push(a),
            IrExprKind::Mux { sel, a, b }
            | IrExprKind::ArrayMux { sel, a, b, .. }
            | IrExprKind::StructMux { sel, a, b, .. } => pending.extend([&**sel, &**a, &**b]),
            IrExprKind::UdpEval { inputs: parts, .. }
            | IrExprKind::Predicate { clauses: parts }
            | IrExprKind::Concat { parts }
            | IrExprKind::Replicate { parts, .. } => pending.extend(parts.iter()),
            IrExprKind::Stream { value, .. } => pending.push(value),
            IrExprKind::Inside { value, items } => {
                pending.push(value);
                for item in items {
                    match item {
                        crate::sim::ir::IrInsideItem::Value(value)
                        | crate::sim::ir::IrInsideItem::FixedArray { value, .. } => {
                            pending.push(value)
                        }
                        crate::sim::ir::IrInsideItem::Range { low, high } => {
                            pending.extend([low, high])
                        }
                        crate::sim::ir::IrInsideItem::OpenRange { low, high } => {
                            pending.extend(low.iter().chain(high.iter()))
                        }
                        crate::sim::ir::IrInsideItem::Container { .. }
                        | crate::sim::ir::IrInsideItem::Cells(_) => {}
                    }
                }
            }
            IrExprKind::BitSel { base, idx } => pending.extend([&**base, &**idx]),
            IrExprKind::PartSel { base, .. } => pending.push(base),
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => pending.extend([&**base, &**base_idx, &**width_expr]),
            IrExprKind::SysFunc(function) => {
                if let crate::sim::ir::IrSysFunc::Sampled(call) = &**function {
                    pending.push(&call.argument);
                }
            }
            _ => {}
        }
    }
}
