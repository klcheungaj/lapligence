//! Concurrent assertion lowering.
//!
//! The H22 sequence lowerer uses one shared sampled-clock automaton for
//! concatenation, repetition, and the admitted sequence combinators. H23 adds
//! bounded one-cycle property composition and recursive use of owned named
//! sequence/property bodies. Forms outside the executable subset fail closed
//! here rather than becoming an untimed immediate assertion.

use super::*;
use crate::core::db::{
    AssertionBinaryOp, AssertionExprKind, AssertionRange, AssertionRepetition,
    AssertionRepetitionKind, AssertionUnaryOp, ConcurrentAssertionKind, EventSpec,
};
use crate::sim::ir::{
    IrAssertion, IrBinOp, IrConcurrentAssertionKind, IrExpr, IrExprKind, IrProcess,
    IrSampledDomain, IrSequence, IrSequenceRange, IrSequenceTransition, IrShape, IrUnOp,
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
}

struct SequenceBuilder {
    next_state: u32,
    transitions: Vec<IrSequenceTransition>,
    atoms: Vec<IrExpr>,
    first_match: bool,
    first_match_states: Vec<u32>,
}

struct SequenceFragment {
    start: u32,
    accept: u32,
}

impl SequenceBuilder {
    fn new() -> Self {
        Self {
            next_state: 0,
            transitions: Vec::new(),
            atoms: Vec::new(),
            first_match: false,
            first_match_states: Vec::new(),
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

    fn edge(
        &mut self,
        from: u32,
        to: u32,
        delay: AssertionRange,
        atom: Option<IrExpr>,
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
            atom,
        });
        Ok(())
    }

    fn epsilon(&mut self, from: u32, to: u32, delay: AssertionRange) -> Result<(), String> {
        self.edge(from, to, delay, None)
    }

    fn finish(self, fragment: SequenceFragment) -> Result<IrSequence, String> {
        IrSequence::new(
            self.next_state,
            fragment.start,
            fragment.accept,
            self.transitions,
            self.atoms,
            self.first_match,
            self.first_match_states,
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
        };
        if let Some(consequent) = parts.consequent {
            self.model.assertions.push(IrAssertion::new(
                assertion.index() as u64,
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
            ));
        } else if let Some(consequent) = parts.consequent_sequence {
            self.model.assertions.push(IrAssertion::new_sequence(
                assertion.index() as u64,
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
            ));
        } else {
            return Err(format!(
                "concurrent assertion has no lowered consequent at {path}"
            ));
        }
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
                _ => break,
            }
        }

        // An assertion instance's body is an owned, already-bound assertion
        // expression. Re-enter it with the clock/disable metadata collected
        // above. This admits named sequence/property use while retaining the
        // exact declaration argument/default expansion performed by Slang.
        if let Some(body) = match self.kind(current) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => Some(*body),
                _ => None,
            },
            _ => None,
        } {
            return self.lower_property_inner(path, body, clock, disable, origin);
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
        let use_engine = antecedent_node.is_some_and(|node| self.sequence_requires_engine(node))
            || self.sequence_requires_engine(consequent_node);
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
                let consequent =
                    self.lower_simple_assertion_expr(path, consequent_node, "consequent")?;
                Ok::<_, String>((antecedent, Some(consequent), None, None))
            }
        })();
        self.sampled_clock = previous_clock;
        let (antecedent, consequent, antecedent_sequence, consequent_sequence) = expressions
            .map_err(|error| format!("{error} (assertion at {})", self.source_location(origin)))?;
        Ok(PropertyParts {
            clock_signal,
            posedge,
            disable_signal: disable,
            antecedent,
            consequent,
            antecedent_sequence,
            consequent_sequence,
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
        let mut builder = SequenceBuilder::new();
        let fragment = self.lower_sequence_fragment(path, node, &mut builder, role)?;
        builder.finish(fragment)
    }

    fn validate_nested_clock(
        &mut self,
        path: &str,
        node: NodeId,
        signal: NodeId,
        posedge: bool,
        role: &str,
    ) -> Result<(), String> {
        let signal = self.lower_assertion_signal(path, signal, "clock")?;
        let Some(expected) = self.sampled_clock else {
            return Err(format!(
                "nested assertion clock has no enclosing sampled domain in {role} at {} ({path})",
                self.source_location(node)
            ));
        };
        if signal != expected.signal || posedge != expected.posedge {
            return Err(format!(
                "nested assertion clock conflicts with the enclosing sampled domain in {role} at {} ({path})",
                self.source_location(node)
            ));
        }
        Ok(())
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
                    if let NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) = self.kind(*expr)
                    {
                        return self.lower_sequence_fragment(path, *body, builder, role);
                    }
                }
                let atom = self.lower_sequence_atom(path, *expr, role)?;
                if let Some(repetition) = repetition {
                    if !*repeated {
                        return Err(format!(
                            "invalid sequence repetition metadata in concurrent assertion {role} at {path}"
                        ));
                    }
                    self.lower_repetition(builder, atom, repetition)
                } else {
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    builder.edge(start, accept, zero_range(), Some(atom))?;
                    Ok(SequenceFragment { start, accept })
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, delays }) => {
                if elements.is_empty() {
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    builder.epsilon(start, accept, zero_range())?;
                    return Ok(SequenceFragment { start, accept });
                }
                let start = builder.state()?;
                let accept = builder.state()?;
                let mut cursor = start;
                for (index, element) in elements.iter().enumerate() {
                    let fragment = self.lower_sequence_fragment(path, *element, builder, role)?;
                    let delay = delays.get(index).cloned().unwrap_or_else(zero_range);
                    builder.epsilon(cursor, fragment.start, delay)?;
                    cursor = fragment.accept;
                }
                builder.epsilon(cursor, accept, zero_range())?;
                Ok(SequenceFragment { start, accept })
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceWithMatch {
                expr,
                match_items,
                repetition,
                ..
            }) => {
                if !match_items.is_empty() {
                    return Err(format!(
                        "sequence match items are not supported in concurrent assertion {role} at {path}"
                    ));
                }
                let fragment = if matches!(self.kind(*expr), NodeKind::AssertionExpr(_)) {
                    self.lower_sequence_fragment(path, *expr, builder, role)?
                } else {
                    let atom = self.lower_sequence_atom(path, *expr, role)?;
                    let start = builder.state()?;
                    let accept = builder.state()?;
                    builder.edge(start, accept, zero_range(), Some(atom))?;
                    SequenceFragment { start, accept }
                };
                if let Some(repetition) = repetition {
                    // A sequence-with-match repetition is represented by a
                    // direct repeated body in Slang's owned graph. Match
                    // items remain outside this bounded H23 subset because
                    // their per-thread mutable state needs a richer engine.
                    let atom = self.sequence_fragment_atom(builder, fragment)?;
                    self.lower_repetition(builder, atom, repetition)
                } else {
                    Ok(fragment)
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::FirstMatch {
                sequence,
                match_items,
            }) => {
                if !match_items.is_empty() {
                    return Err(format!(
                        "first_match match items are not supported in concurrent assertion {role} at {path}"
                    ));
                }
                let fragment = self.lower_sequence_fragment(path, *sequence, builder, role)?;
                builder.first_match = true;
                builder.mark_first_match(fragment.accept);
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
                builder.edge(
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
                )?;
                Ok(SequenceFragment { start, accept })
            }
            NodeKind::AssertionExpr(AssertionExprKind::Binary { op, left, right }) => {
                match op {
                    AssertionBinaryOp::Or => {
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        let left = self.lower_sequence_fragment(path, *left, builder, role)?;
                        let right = self.lower_sequence_fragment(path, *right, builder, role)?;
                        builder.epsilon(start, left.start, zero_range())?;
                        builder.epsilon(start, right.start, zero_range())?;
                        builder.epsilon(left.accept, accept, zero_range())?;
                        builder.epsilon(right.accept, accept, zero_range())?;
                        Ok(SequenceFragment { start, accept })
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
                        builder.edge(start, accept, zero_range(), Some(atom))?;
                        Ok(SequenceFragment { start, accept })
                    }
                    AssertionBinaryOp::Iff | AssertionBinaryOp::Implies => {
                        let atom = self.lower_one_cycle_assertion(path, node, role)?;
                        let start = builder.state()?;
                        let accept = builder.state()?;
                        builder.edge(start, accept, zero_range(), Some(atom))?;
                        Ok(SequenceFragment { start, accept })
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
                self.validate_nested_clock(path, node, *signal, *posedge, role)?;
                self.lower_sequence_fragment(path, *expr, builder, role)
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
        if let NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) = self.kind(node) {
            return self.lower_one_cycle_assertion(path, *body, role);
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
                self.validate_nested_clock(path, node, *signal, *posedge, role)?;
                self.lower_one_cycle_assertion(path, *expr, role)
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
            other => Err(format!(
                "property expression {other:?} is not a one-cycle boolean form in {role} at {} ({path})",
                self.source_location(node)
            )),
        }
    }

    fn sequence_fragment_atom(
        &self,
        builder: &SequenceBuilder,
        fragment: SequenceFragment,
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

    fn lower_repetition(
        &mut self,
        builder: &mut SequenceBuilder,
        atom: IrExpr,
        repetition: &AssertionRepetition,
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
            builder.epsilon(start, accept, repetition_endpoint_range(repetition.kind))?;
        }
        let mut cursor = start;
        for count in 0..min {
            let next = builder.state()?;
            builder.edge(
                cursor,
                next,
                if count == 0 {
                    repetition_first_range(repetition.kind)
                } else {
                    one_or_more_range(repetition.kind)
                },
                Some(atom.clone()),
            )?;
            cursor = next;
        }
        match repetition.range.max {
            Some(max) => {
                for count in min..max {
                    builder.epsilon(cursor, accept, repetition_endpoint_range(repetition.kind))?;
                    let next = builder.state()?;
                    builder.edge(
                        cursor,
                        next,
                        if count == 0 {
                            repetition_first_range(repetition.kind)
                        } else {
                            one_or_more_range(repetition.kind)
                        },
                        Some(atom.clone()),
                    )?;
                    cursor = next;
                }
                builder.epsilon(cursor, accept, repetition_endpoint_range(repetition.kind))?;
            }
            None => {
                builder.epsilon(cursor, accept, repetition_endpoint_range(repetition.kind))?;
                let loop_state = builder.state()?;
                builder.edge(
                    cursor,
                    loop_state,
                    if min == 0 {
                        repetition_first_range(repetition.kind)
                    } else {
                        one_or_more_range(repetition.kind)
                    },
                    Some(atom.clone()),
                )?;
                builder.edge(
                    loop_state,
                    loop_state,
                    one_or_more_range(repetition.kind),
                    Some(atom),
                )?;
                builder.epsilon(
                    loop_state,
                    accept,
                    repetition_endpoint_range(repetition.kind),
                )?;
            }
        }
        Ok(SequenceFragment { start, accept })
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
