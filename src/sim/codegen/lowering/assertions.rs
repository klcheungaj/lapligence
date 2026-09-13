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
    IrSampledDomain, IrSequence, IrSequenceLocal, IrSequenceRange, IrSequenceTransition, IrShape,
    IrUnOp,
};

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
    transitions: Vec<IrSequenceTransition>,
    atoms: Vec<IrExpr>,
    first_match: bool,
    first_match_states: Vec<u32>,
    match_items: Vec<IrExpr>,
}

struct SequenceFragment {
    start: u32,
    accept: u32,
    leading_clock: Option<SampledClock>,
    trailing_clock: Option<SampledClock>,
}

impl SequenceBuilder {
    fn new() -> Self {
        Self {
            next_state: 0,
            transitions: Vec::new(),
            atoms: Vec::new(),
            first_match: false,
            first_match_states: Vec::new(),
            match_items: Vec::new(),
        }
    }

    fn state(&mut self) -> Result<u32, String> {
        let state = self.next_state;
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
        )
        .map_err(|error| error.to_string())
    }

    fn mark_first_match(&mut self, state: u32) {
        if !self.first_match_states.contains(&state) {
            self.first_match_states.push(state);
        }
    }
}

fn zero_range() -> AssertionRange {
    AssertionRange {
        min: 0,
        max: Some(0),
    }
}

fn one_or_more_range(kind: AssertionRepetitionKind) -> AssertionRange {
    match kind {
        AssertionRepetitionKind::Consecutive => AssertionRange {
            min: 1,
            max: Some(1),
        },
        AssertionRepetitionKind::Nonconsecutive | AssertionRepetitionKind::GoTo => {
            AssertionRange { min: 1, max: None }
        }
    }
}

fn repetition_first_range(kind: AssertionRepetitionKind) -> AssertionRange {
    match kind {
        AssertionRepetitionKind::Consecutive => zero_range(),
        AssertionRepetitionKind::Nonconsecutive | AssertionRepetitionKind::GoTo => {
            AssertionRange { min: 0, max: None }
        }
    }
}

fn repetition_endpoint_range(kind: AssertionRepetitionKind) -> AssertionRange {
    match kind {
        AssertionRepetitionKind::Consecutive | AssertionRepetitionKind::GoTo => zero_range(),
        // Nonconsecutive repetition may complete at or after its final
        // occurrence; retaining this endpoint range lets a following
        // concatenation choose any later sampled tick.
        AssertionRepetitionKind::Nonconsecutive => AssertionRange { min: 0, max: None },
    }
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
        let mut blocks = Vec::new();
        let mut scope = Some(self.inst);
        while let Some(current) = scope {
            for child in self.node(current).children.iter().copied() {
                if let Some(info) = self.db.clocking_block(child).filter(|info| info.is_default) {
                    blocks.push((child, info));
                }
            }
            // A default clocking declaration is inherited from the nearest
            // enclosing scope. Do not let a sibling module's declaration
            // become visible merely because it has the same source name.
            if !blocks.is_empty() {
                break;
            }
            scope = self.node(current).parent;
        }
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
                    .flat_map(|bindings| bindings.values().copied())
                    .collect::<Vec<_>>();
                local_bindings.sort_by_key(|binding| binding.slot);
                let locals = local_bindings
                    .iter()
                    .map(|binding| IrSequenceLocal {
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
                let initializers = initializers
                    .into_iter()
                    .map(|(_, initializer)| initializer)
                    .collect();
                builder.finish(fragment, locals, initializers)
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
                        start,
                        accept,
                        leading_clock: clock,
                        trailing_clock: clock,
                    })
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, delays }) => {
                if elements.is_empty() {
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    let clock = self.sampled_clock;
                    builder.epsilon_with_clock(start, accept, zero_range(), clock)?;
                    return Ok(SequenceFragment {
                        start,
                        accept,
                        leading_clock: clock,
                        trailing_clock: clock,
                    });
                }
                let start = builder.state()?;
                let accept = builder.state()?;
                let mut cursor = start;
                let mut leading_clock: Option<SampledClock> = None;
                let mut trailing_clock: Option<SampledClock> = None;
                for (index, element) in elements.iter().enumerate() {
                    let fragment = self.lower_sequence_fragment(path, *element, builder, role)?;
                    let delay = delays.get(index).cloned().unwrap_or_else(zero_range);
                    let clock = fragment.leading_clock.or(self.sampled_clock);
                    if let (Some(previous), Some(next)) = (trailing_clock, clock) {
                        let different_domain = previous.signal != next.signal
                            || previous.posedge != next.posedge;
                        if different_domain
                            && !matches!((delay.min, delay.max), (0, Some(0)) | (1, Some(1)))
                        {
                            return Err(format!(
                                "cross-clock sequence boundaries require an exact ##0 or ##1 delay in concurrent assertion {role} at {path}"
                            ));
                        }
                    }
                    builder.epsilon_with_clock(cursor, fragment.start, delay, clock)?;
                    if leading_clock.is_none() {
                        leading_clock = clock;
                    }
                    trailing_clock = fragment.trailing_clock.or(clock);
                    cursor = fragment.accept;
                }
                let final_clock = trailing_clock.or(self.sampled_clock);
                builder.epsilon_with_clock(cursor, accept, zero_range(), final_clock)?;
                Ok(SequenceFragment {
                    start,
                    accept,
                    leading_clock: leading_clock.or(self.sampled_clock),
                    trailing_clock: final_clock,
                })
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
                        start,
                        accept,
                        leading_clock: clock,
                        trailing_clock: clock,
                    }
                };
                let previous_clock = self.sampled_clock;
                self.sampled_clock = fragment.trailing_clock.or(previous_clock);
                let lowered_match_items = (|| {
                    match_items
                        .iter()
                        .map(|item| self.lower_assertion_match_item(path, *item, role))
                        .collect::<Result<Vec<_>, _>>()
                })();
                self.sampled_clock = previous_clock;
                let lowered_match_items = lowered_match_items?;
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
                    let atom = self.sequence_fragment_atom(builder, &fragment)?;
                    self.lower_repetition(
                        builder,
                        atom,
                        repetition,
                        fragment.trailing_clock.or(self.sampled_clock),
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
                builder.first_match = true;
                builder.mark_first_match(fragment.accept);
                let previous_clock = self.sampled_clock;
                self.sampled_clock = fragment.trailing_clock.or(previous_clock);
                let lowered = (|| {
                    match_items
                        .iter()
                        .map(|item| self.lower_assertion_match_item(path, *item, role))
                        .collect::<Result<Vec<_>, _>>()
                })();
                self.sampled_clock = previous_clock;
                let lowered = lowered?;
                builder.attach_match_items(fragment.accept, lowered)?;
                Ok(fragment)
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
                    | AssertionBinaryOp::Within => {
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
                            start,
                            accept,
                            leading_clock: clock,
                            trailing_clock: clock,
                        })
                    }
                    AssertionBinaryOp::Iff | AssertionBinaryOp::Implies => {
                        let atom = self.lower_one_cycle_assertion(path, node, role)?;
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        let clock = self.sampled_clock;
                        builder.edge_with_clock(start, accept, zero_range(), Some(atom), clock)?;
                        Ok(SequenceFragment {
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
            }) => self.lower_sequence_atom(path, *expr, role),
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
                let condition = self.lower_boolean_expr(path, *condition)?;
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
        if repetition.range.max.is_some_and(|max| max < min) {
            return Err("sequence repetition range is inverted".to_owned());
        }
        let start = builder.state()?;
        let accept = builder.state()?;
        if repetition.kind == AssertionRepetitionKind::GoTo {
            // Goto repetition ends at the first endpoint that satisfies its
            // occurrence count; retaining the waiting source would create
            // later endpoints that the LRM's goto form does not admit.
            builder.mark_first_match(accept);
        }
        if min == 0 {
            builder.epsilon_with_clock(
                start,
                accept,
                repetition_endpoint_range(repetition.kind),
                clock,
            )?;
        }
        let mut cursor = start;
        for count in 0..min {
            let next = builder.state()?;
            builder.edge_with_clock(
                cursor,
                next,
                if count == 0 {
                    repetition_first_range(repetition.kind)
                } else {
                    one_or_more_range(repetition.kind)
                },
                Some(atom.clone()),
                clock,
            )?;
            cursor = next;
        }
        match repetition.range.max {
            Some(max) => {
                for count in min..max {
                    builder.epsilon_with_clock(
                        cursor,
                        accept,
                        repetition_endpoint_range(repetition.kind),
                        clock,
                    )?;
                    let next = builder.state()?;
                    builder.edge_with_clock(
                        cursor,
                        next,
                        if count == 0 {
                            repetition_first_range(repetition.kind)
                        } else {
                            one_or_more_range(repetition.kind)
                        },
                        Some(atom.clone()),
                        clock,
                    )?;
                    cursor = next;
                }
                builder.epsilon_with_clock(
                    cursor,
                    accept,
                    repetition_endpoint_range(repetition.kind),
                    clock,
                )?;
            }
            None => {
                builder.epsilon_with_clock(
                    cursor,
                    accept,
                    repetition_endpoint_range(repetition.kind),
                    clock,
                )?;
                let loop_state = builder.state()?;
                builder.edge_with_clock(
                    cursor,
                    loop_state,
                    if min == 0 {
                        repetition_first_range(repetition.kind)
                    } else {
                        one_or_more_range(repetition.kind)
                    },
                    Some(atom.clone()),
                    clock,
                )?;
                builder.edge_with_clock(
                    loop_state,
                    loop_state,
                    one_or_more_range(repetition.kind),
                    Some(atom),
                    clock,
                )?;
                builder.epsilon_with_clock(
                    loop_state,
                    accept,
                    repetition_endpoint_range(repetition.kind),
                    clock,
                )?;
            }
        }
        Ok(SequenceFragment {
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
