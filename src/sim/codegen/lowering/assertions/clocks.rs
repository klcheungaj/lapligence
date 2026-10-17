//! Clock resolution and clock flow of concurrent assertions (IEEE 1800-2009
//! 16.14.3, 16.17, 16.17.1 and the Annex F.5.1 rewrite rules).
//!
//! Clock flow is structural: a clocking event clocks the subexpression it
//! prefixes and nothing outside it ("the scope of a clocking event does not
//! flow out of enclosing parentheses", 16.14.3), and `T_p((r |-> p), c)`
//! clocks the consequent by the clock flowing into the implication, not by
//! the clock the antecedent ends on (F.5.1.2). The assertion is registered on
//! its semantic leading clock (16.17.1).

use super::*;

/// Semantic leading clocks of a sequence or property (16.17.1): the explicit
/// clocks, and whether a leading position inherits the clock flowing in.
#[derive(Clone, Default)]
pub(super) struct LeadingClocks {
    inherited: bool,
    clocks: Vec<SampledClock>,
}

pub(super) fn same_clock(a: &SampledClock, b: &SampledClock) -> bool {
    a.signal == b.signal && a.posedge == b.posedge && a.gate == b.gate
}

impl LeadingClocks {
    fn inherited() -> Self {
        Self {
            inherited: true,
            clocks: Vec::new(),
        }
    }

    fn add(&mut self, clock: SampledClock) {
        if !self.clocks.iter().any(|known| same_clock(known, &clock)) {
            self.clocks.push(clock);
        }
    }

    fn union(mut self, other: Self) -> Self {
        self.inherited |= other.inherited;
        for clock in other.clocks {
            self.add(clock);
        }
        self
    }

    /// The single explicit leading clock, when no leading position
    /// inherits one.
    pub(super) fn explicit(&self) -> Option<SampledClock> {
        match self.clocks.as_slice() {
            [clock] if !self.inherited => Some(*clock),
            _ => None,
        }
    }

    /// `@(c) q`: c replaces an inherited leading clock of q.
    fn under(mut self, clock: SampledClock) -> Self {
        if self.inherited {
            self.inherited = false;
            self.add(clock);
        }
        self
    }
}

/// Whether a clocking event is juxtaposed with an inner one, which nullifies
/// it (`@(d) @(c) x` is `@(c) x`, 16.14.3): its body reaches another
/// clocking event through operators that take no clock of their own.
fn transparent_body(kind: &AssertionExprKind) -> Option<NodeId> {
    match kind {
        AssertionExprKind::DisableIff { expr, .. }
        | AssertionExprKind::Abort {
            expr, sync: false, ..
        } => Some(*expr),
        _ => None,
    }
}

/// The resolved clocks of one assertion: the clock flowing into its top
/// property (written at its top, inferred from the procedure or the default,
/// 16.17 a)-d)) and its semantic leading clock, which starts its attempts.
#[derive(Clone, Copy)]
pub(super) struct AssertionClocks {
    pub(super) context: Option<SampledClock>,
    pub(super) leading: SampledClock,
}

impl Codegen<'_> {
    /// The clock of an assertion clocking event, as an identity for clock
    /// resolution: one edge of a direct signal (with its `iff`) is detected
    /// at that signal's write; every other legal clocking event becomes a
    /// planned pulse clock.
    pub(super) fn assertion_event_clock(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        if let Some(clock) = self.assertion_inferred_resolved.get(&node) {
            return Ok(*clock);
        }
        let NodeKind::AssertionExpr(AssertionExprKind::Clocking { specs, .. }) = self.kind(node)
        else {
            return Err(format!(
                "assertion clocking event expected at {}",
                self.source_location(node)
            ));
        };
        let specs = specs.clone();
        if let [EventSpec::Named(event) | EventSpec::AnyChange { sig: event }] = specs.as_slice() {
            if matches!(self.kind(*event), NodeKind::SysCall { name } if name == "$inferred_clock")
            {
                let clock = self.inferred_assertion_clock(path, node)?;
                self.assertion_inferred_resolved.insert(node, clock);
                return Ok(clock);
            }
        }
        self.assertion_clock_from_specs(path, &specs, 0)
    }

    /// `$inferred_clock` (IEEE 1800-2009 16.15.7): the clock that flows to
    /// the instance whose formal default it replaced, or, for an instance
    /// that is the top-level property, the clock inferred at the assertion
    /// statement (its procedure's clock, else default clocking).
    fn inferred_assertion_clock(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        let context = match self.assertion_inferred_clocks.last().copied().flatten() {
            Some(clock) => Some(clock),
            None => {
                let procedure = match self.assertion_inferred_origin {
                    Some(origin) => self.procedure_assertion_clock(path, origin)?,
                    None => None,
                };
                match procedure {
                    Some(clock) => Some(clock),
                    None => self.lower_default_sampled_clock(path)?,
                }
            }
        };
        context.ok_or_else(|| {
            format!(
                "$inferred_clock at {} has no current resolved event expression (IEEE 1800-2009 16.15.7)",
                self.source_location(node)
            )
        })
    }

    /// The clock a clocking event switches the evaluation to while a
    /// property or sequence is lowered. A gated edge that is not the
    /// assertion's leading clock, and an edge of the leading clock's signal
    /// and polarity under a different gate, are pulse clocks: the runtime
    /// gates a direct signal edge only by the leading clock's `iff`.
    pub(super) fn assertion_flow_clock(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        let clock = self.assertion_event_clock(path, node)?;
        let Some(leading) = self.assertion_leading_clock else {
            return if clock.gate.is_some() {
                self.assertion_clock_pulse_of(path, node)
            } else {
                Ok(clock)
            };
        };
        if same_clock(&clock, &leading) {
            return Ok(leading);
        }
        let same_edge = clock.signal == leading.signal && clock.posedge == leading.posedge;
        if clock.gate.is_some() || (same_edge && leading.gate.is_some()) {
            return self.assertion_clock_pulse_of(path, node);
        }
        Ok(clock)
    }

    /// The pulse clock of a clocking event node, regardless of its form.
    fn assertion_clock_pulse_of(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        let NodeKind::AssertionExpr(AssertionExprKind::Clocking { specs, .. }) = self.kind(node)
        else {
            return Err(format!(
                "assertion clocking event expected at {}",
                self.source_location(node)
            ));
        };
        let specs = specs.clone();
        self.assertion_pulse_clock(path, &specs)
    }

    /// Resolve a clocking event list to an assertion clock. A clocking
    /// block event (`@(cb)`, 14.13) and `$global_clock` (14.14) stand for
    /// their block's clocking event, so a block clocked on one direct edge
    /// keeps the direct form.
    pub(super) fn assertion_clock_from_specs(
        &mut self,
        path: &str,
        specs: &[EventSpec],
        depth: usize,
    ) -> Result<SampledClock, String> {
        if let [spec] = specs {
            if let Some((event, posedge, gate)) = flatten_clock_spec(spec) {
                if let Some(signal) = self.direct_sampled_clock_signal(path, event) {
                    return Ok(SampledClock {
                        signal,
                        posedge,
                        gate,
                    });
                }
            }
            if let EventSpec::Named(event) | EventSpec::AnyChange { sig: event } = spec {
                if depth < 8 {
                    if let Some(block) = self.assertion_clocking_block_events(path, *event)? {
                        return self.assertion_clock_from_specs(path, &block, depth + 1);
                    }
                }
            }
        }
        if specs.is_empty() {
            return Err(format!("assertion clocking event has no event in `{path}`"));
        }
        self.assertion_pulse_clock(path, specs)
    }

    /// The event list of the clocking block an event expression names
    /// (`@(cb)`), or of the global clocking block for `$global_clock`.
    fn assertion_clocking_block_events(
        &mut self,
        path: &str,
        event: NodeId,
    ) -> Result<Option<Vec<EventSpec>>, String> {
        if let NodeKind::SysCall { name } = self.kind(event) {
            if name == "$global_clock" {
                return self.global_clocking_events(path).map(Some);
            }
            return Ok(None);
        }
        let Some(target) = self.event_target_of(event) else {
            return Ok(None);
        };
        if !target.indices.is_empty() {
            return Ok(None);
        }
        Ok(self
            .db
            .clocking_block(target.declaration)
            .map(|block| block.event_specs.clone()))
    }

    /// A planned assertion clock: a hidden one-bit signal that a synthetic
    /// process pulses each time `specs` occurs (at most once per time step).
    /// Identical lowered event lists share one signal.
    pub(super) fn assertion_pulse_clock(
        &mut self,
        path: &str,
        specs: &[EventSpec],
    ) -> Result<SampledClock, String> {
        let pulse = |signal: usize| SampledClock {
            signal,
            posedge: true,
            gate: None,
        };
        // The owned event list identifies one written clocking event in one
        // instance; equal lowered lists without evaluated qualifiers are
        // the same event wherever they are written.
        let source_key = format!("source {:?} {specs:?}", self.inst);
        if let Some(signal) = self.assertion_pulse_keys.get(&source_key) {
            return Ok(pulse(*signal));
        }
        self.ensure_event_spec_endpoints(path, specs)?;
        let inst = self.inst;
        let (lowered, pre_fns) = {
            let mut ctx = EmitCtx::new(self, path.to_owned(), inst, "0", None, false);
            let lowered = ctx.lower_event_specs(specs)?;
            (lowered, std::mem::take(&mut ctx.pre_fns))
        };
        let lowered_key = pre_fns.is_empty().then(|| format!("lowered {lowered:?}"));
        if let Some(signal) = lowered_key
            .as_ref()
            .and_then(|key| self.assertion_pulse_keys.get(key))
            .copied()
        {
            self.assertion_pulse_keys.insert(source_key, signal);
            return Ok(pulse(signal));
        }
        let signal = self.model.signals.len();
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: format!("llg_assertion_clock_{signal}"),
            hdl_name: None,
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.assertion_pulse_keys.insert(source_key, signal);
        if let Some(key) = lowered_key {
            self.assertion_pulse_keys.insert(key, signal);
        }
        self.sampled_event_clocks.push(PendingSampledEventClock {
            clock: signal,
            pulse: true,
            path: path.to_owned(),
            specs: lowered,
            pre_fns,
        });
        Ok(pulse(signal))
    }

    /// The event list of the design's one global clocking block (IEEE
    /// 1800-2009 14.14: at most one in the elaborated model).
    pub(in crate::sim::codegen) fn global_clocking_events(
        &self,
        path: &str,
    ) -> Result<Vec<EventSpec>, String> {
        let mut blocks = self
            .db
            .node_ids()
            .filter_map(|id| self.db.clocking_block(id).filter(|info| info.is_global));
        match (blocks.next(), blocks.next()) {
            (Some(block), None) => Ok(block.event_specs.clone()),
            (None, _) => Err(format!(
                "$global_clock requires a global clocking declaration in the elaborated model (IEEE 1800-2009 14.14) in `{path}`"
            )),
            (Some(_), Some(_)) => Err(format!(
                "$global_clock found more than one global clocking declaration (IEEE 1800-2009 14.14) in `{path}`"
            )),
        }
    }

    /// The body of an assertion instance expression, if `node` is one used
    /// as a sequence or property.
    pub(super) fn assertion_instance_body(&self, node: NodeId) -> Option<NodeId> {
        let NodeKind::AssertionExpr(AssertionExprKind::Simple {
            expr,
            repeated: false,
            repetition: None,
        }) = self.kind(node)
        else {
            return None;
        };
        match self.kind(*expr) {
            NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => Some(*body),
            _ => None,
        }
    }

    /// The temporal operands of an assertion node: the sequences and
    /// properties it combines, with instance bodies, but not Boolean
    /// conditions or clocking-event expressions.
    pub(super) fn assertion_operands(&self, node: NodeId) -> Vec<NodeId> {
        let NodeKind::AssertionExpr(kind) = self.kind(node) else {
            return Vec::new();
        };
        match kind {
            AssertionExprKind::Simple { expr, .. } => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => vec![*body],
                _ => Vec::new(),
            },
            AssertionExprKind::SequenceConcat { elements, .. } => elements.clone(),
            AssertionExprKind::SequenceWithMatch { expr, .. }
            | AssertionExprKind::Unary { expr, .. }
            | AssertionExprKind::Clocking { expr, .. }
            | AssertionExprKind::StrongWeak { expr, .. }
            | AssertionExprKind::Abort { expr, .. }
            | AssertionExprKind::DisableIff { expr, .. } => vec![*expr],
            AssertionExprKind::FirstMatch { sequence, .. } => vec![*sequence],
            AssertionExprKind::Binary { left, right, .. } => vec![*left, *right],
            AssertionExprKind::Conditional {
                if_expr, else_expr, ..
            } => std::iter::once(*if_expr).chain(*else_expr).collect(),
            AssertionExprKind::Case {
                items,
                default_case,
                ..
            } => items
                .iter()
                .map(|item| item.body)
                .chain(*default_case)
                .collect(),
            AssertionExprKind::Invalid { .. } => Vec::new(),
        }
    }

    /// Whether a clocking event is nullified by an inner one juxtaposed with
    /// it, through instance bodies and clockless wrappers.
    fn clocking_nullified(&self, node: NodeId) -> bool {
        let NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) = self.kind(node)
        else {
            return false;
        };
        let mut current = *expr;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            if let Some(body) = self.assertion_instance_body(current) {
                current = body;
                continue;
            }
            match self.kind(current) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking { .. }) => return true,
                NodeKind::AssertionExpr(kind) => match transparent_body(kind) {
                    Some(body) => current = body,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// The semantic leading clocks of `node` (16.17.1).
    pub(super) fn semantic_leading_clocks(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<LeadingClocks, String> {
        let mut seen = HashSet::new();
        self.semantic_leading_clocks_inner(path, node, &mut seen)
    }

    fn semantic_leading_clocks_inner(
        &mut self,
        path: &str,
        node: NodeId,
        seen: &mut HashSet<NodeId>,
    ) -> Result<LeadingClocks, String> {
        // A recursive instance re-enters a body already being examined; its
        // clocks are those of the enclosing expansion.
        if !seen.insert(node) {
            return Ok(LeadingClocks::default());
        }
        let result = self.semantic_leading_clocks_node(path, node, seen);
        seen.remove(&node);
        result
    }

    fn semantic_leading_clocks_node(
        &mut self,
        path: &str,
        node: NodeId,
        seen: &mut HashSet<NodeId>,
    ) -> Result<LeadingClocks, String> {
        let NodeKind::AssertionExpr(kind) = self.kind(node) else {
            return Ok(LeadingClocks::inherited());
        };
        match kind {
            AssertionExprKind::Clocking { expr, .. } => {
                let expr = *expr;
                let clock = self.assertion_event_clock(path, node)?;
                // Leading positions below flow this clock to any instance.
                self.assertion_inferred_clocks.push(Some(clock));
                let inner = self.semantic_leading_clocks_inner(path, expr, seen);
                self.assertion_inferred_clocks.pop();
                Ok(inner?.under(clock))
            }
            AssertionExprKind::Simple { expr, .. } => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                    let body = *body;
                    self.semantic_leading_clocks_inner(path, body, seen)
                }
                _ => Ok(LeadingClocks::inherited()),
            },
            AssertionExprKind::SequenceConcat { elements, delays } => {
                // A leading delay (`##1 s`) is `1 ##1 s`, whose leading
                // clock is inherited.
                let leading_delay = delays
                    .first()
                    .is_some_and(|delay| delay.min != 0 || delay.max != Some(0));
                match elements.first() {
                    Some(first) if !leading_delay => {
                        let first = *first;
                        self.semantic_leading_clocks_inner(path, first, seen)
                    }
                    _ => Ok(LeadingClocks::inherited()),
                }
            }
            AssertionExprKind::SequenceWithMatch { expr, .. }
            | AssertionExprKind::StrongWeak { expr, .. }
            | AssertionExprKind::DisableIff { expr, .. }
            | AssertionExprKind::Abort {
                expr, sync: false, ..
            } => {
                let expr = *expr;
                self.semantic_leading_clocks_inner(path, expr, seen)
            }
            AssertionExprKind::FirstMatch { sequence, .. } => {
                let sequence = *sequence;
                self.semantic_leading_clocks_inner(path, sequence, seen)
            }
            AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                expr,
                ..
            } => {
                let expr = *expr;
                self.semantic_leading_clocks_inner(path, expr, seen)
            }
            AssertionExprKind::Binary { op, left, right } => {
                let (op, left, right) = (*op, *left, *right);
                match op {
                    AssertionBinaryOp::OverlappedImplication
                    | AssertionBinaryOp::NonOverlappedImplication
                    | AssertionBinaryOp::OverlappedFollowedBy
                    | AssertionBinaryOp::NonOverlappedFollowedBy => {
                        self.semantic_leading_clocks_inner(path, left, seen)
                    }
                    AssertionBinaryOp::Until
                    | AssertionBinaryOp::SUntil
                    | AssertionBinaryOp::UntilWith
                    | AssertionBinaryOp::SUntilWith => Ok(LeadingClocks::inherited()),
                    _ => {
                        let left = self.semantic_leading_clocks_inner(path, left, seen)?;
                        let right = self.semantic_leading_clocks_inner(path, right, seen)?;
                        Ok(left.union(right))
                    }
                }
            }
            _ => Ok(LeadingClocks::inherited()),
        }
    }

    /// The first clocking event anywhere in `node`, the pre-16.17.1 fallback
    /// for an assertion whose leading positions all inherit a clock that no
    /// context supplies (`(@(c) a) and b` without default clocking).
    fn first_assertion_clock(
        &mut self,
        path: &str,
        node: NodeId,
        seen: &mut HashSet<NodeId>,
    ) -> Result<Option<SampledClock>, String> {
        if !seen.insert(node) {
            return Ok(None);
        }
        if matches!(
            self.kind(node),
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { .. })
        ) {
            return self.assertion_event_clock(path, node).map(Some);
        }
        for operand in self.assertion_operands(node) {
            if let Some(clock) = self.first_assertion_clock(path, operand, seen)? {
                return Ok(Some(clock));
            }
        }
        Ok(None)
    }

    /// Resolve the clocks of an assertion whose property is `root`.
    /// `explicit` is the clock already written above `root` (an enclosing
    /// instance's or end point's context). The context clock is consulted
    /// only when some position inherits it, so a procedure clock that cannot
    /// clock an assertion is reported only when it is used.
    pub(super) fn resolve_assertion_clocks(
        &mut self,
        path: &str,
        root: NodeId,
        explicit: Option<SampledClock>,
        origin: NodeId,
    ) -> Result<AssertionClocks, String> {
        let leading = self.semantic_leading_clocks(path, root)?;
        let context = match explicit {
            Some(clock) => Some(clock),
            None if leading.inherited || leading.clocks.is_empty() => {
                // 16.15.6: a procedural assertion takes its procedure's
                // inferred clock before default clocking (16.17 a), c)).
                match self.procedure_assertion_clock(path, origin)? {
                    Some(clock) => Some(clock),
                    None => self.lower_default_sampled_clock(path)?,
                }
            }
            None => None,
        };
        let mut clocks = leading.clocks.clone();
        if leading.inherited {
            if let Some(context) = context {
                if !clocks.iter().any(|known| same_clock(known, &context)) {
                    clocks.push(context);
                }
            }
        }
        let leading = match clocks.as_slice() {
            [clock] => *clock,
            [] => {
                let mut seen = HashSet::new();
                self.first_assertion_clock(path, root, &mut seen)?
                    .ok_or_else(|| {
                        format!(
                            "concurrent assertions require one explicit signal clock at {} ({path})",
                            self.assertion_location(root, origin)
                        )
                    })?
            }
            _ => {
                return Err(format!(
                    "the property at {} has no unique semantic leading clock (IEEE 1800-2009 16.17 e), 16.17.1)",
                    self.assertion_location(root, origin)
                ))
            }
        };
        Ok(AssertionClocks { context, leading })
    }

    /// Record every clocking event inside `root` whose clock differs from the
    /// clock flowing into it (16.14.2: such a property is multiclocked).
    /// Clocking events on the top chain (through instance bodies, `disable
    /// iff` and asynchronous aborts) replace the context clock instead
    /// (16.17 a), d)).
    pub(super) fn collect_clock_changes(
        &mut self,
        path: &str,
        root: NodeId,
        context: Option<SampledClock>,
    ) -> Result<HashSet<NodeId>, String> {
        let mut changes = HashSet::new();
        let mut active = HashSet::new();
        self.collect_clock_changes_inner(
            path,
            root,
            context,
            (true, None),
            &mut active,
            &mut changes,
        )?;
        Ok(changes)
    }

    fn collect_clock_changes_inner(
        &mut self,
        path: &str,
        node: NodeId,
        flowing: Option<SampledClock>,
        (top, inferred): (bool, Option<SampledClock>),
        active: &mut HashSet<NodeId>,
        changes: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        if !active.insert(node) {
            return Ok(());
        }
        let mut flowing = flowing;
        let mut top = top;
        // The clock the innermost clocking event above `node` flows to it,
        // even one nullified by a juxtaposed inner event; `None` when the
        // clock is the one inferred at the assertion statement (16.15.7).
        let mut inferred = inferred;
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { .. }) => {
                let clock = self.assertion_event_clock(path, node)?;
                if !self.clocking_nullified(node) {
                    if !top && flowing.is_some_and(|flowing| !same_clock(&flowing, &clock)) {
                        changes.insert(node);
                    }
                    flowing = Some(clock);
                }
                inferred = Some(clock);
            }
            NodeKind::AssertionExpr(kind) => {
                let keeps_top = transparent_body(kind).is_some();
                top &= keeps_top || self.assertion_instance_body(node).is_some();
            }
            _ => top = false,
        }
        // An instance passes the clock flowing to it to `$inferred_clock`
        // defaults in its body (16.15.7).
        let instance = self.assertion_instance_body(node).is_some();
        if instance {
            self.assertion_inferred_clocks.push(inferred);
        }
        let mut result = Ok(());
        for operand in self.assertion_operands(node) {
            result = self.collect_clock_changes_inner(
                path,
                operand,
                flowing,
                (top, inferred),
                active,
                changes,
            );
            if result.is_err() {
                break;
            }
        }
        if instance {
            self.assertion_inferred_clocks.pop();
        }
        active.remove(&node);
        result
    }

    /// Differently clocked or multiclocked sequence operands combine only
    /// through `##1` and `##0` (IEEE 1800-2009 16.14.1), so a repetition,
    /// `first_match` or sequence `and`/`or`/`intersect`/`throughout`/`within`
    /// over an operand that changes clock is illegal.
    pub(super) fn check_multiclocked_sequence_operator(&self, node: NodeId) -> Result<(), String> {
        let NodeKind::AssertionExpr(kind) = self.kind(node) else {
            return Ok(());
        };
        let operator = match kind {
            AssertionExprKind::Simple {
                repeated,
                repetition,
                ..
            }
            | AssertionExprKind::SequenceWithMatch {
                repeated,
                repetition,
                ..
            } if *repeated || repetition.is_some() => "repetition",
            AssertionExprKind::FirstMatch { .. } => "first_match",
            AssertionExprKind::Binary { op, .. } => match op {
                AssertionBinaryOp::And => "and",
                AssertionBinaryOp::Or => "or",
                AssertionBinaryOp::Intersect => "intersect",
                AssertionBinaryOp::Throughout => "throughout",
                AssertionBinaryOp::Within => "within",
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };
        if self
            .assertion_operands(node)
            .into_iter()
            .any(|operand| self.changes_clock_within(operand))
        {
            return Err(format!(
                "sequence operator {operator}{} combines differently clocked or multiclocked sequence operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)",
                self.known_location_suffix(node)
            ));
        }
        Ok(())
    }

    /// Checks one `##` boundary of a sequence concatenation whose sides are
    /// differently clocked or multiclocked (IEEE 1800-2009 16.14.1): the
    /// delay must be `##1` or `##0`, and the maximal singly clocked
    /// subsequences on both sides must not admit an empty match.
    pub(super) fn check_multiclocked_concat(
        &self,
        node: NodeId,
        left: NodeId,
        right: NodeId,
        delay: &AssertionRange,
        empty_side: bool,
    ) -> Result<(), String> {
        if !self.changes_clock_within(left) && !self.changes_clock_within(right) {
            return Ok(());
        }
        let location = self.known_location_suffix(node);
        if delay.max != Some(delay.min) || delay.min > 1 {
            return Err(format!(
                "differently clocked or multiclocked sequences{location} may be concatenated only with ##1 or ##0 (IEEE 1800-2009 16.14.1)"
            ));
        }
        if empty_side {
            return Err(format!(
                "a maximal singly clocked subsequence of the multiclocked sequence{location} admits an empty match (IEEE 1800-2009 16.14.1)"
            ));
        }
        Ok(())
    }

    /// ` at file:line:col` when the node has a source location; the caller
    /// appends the assertion's own location otherwise.
    fn known_location_suffix(&self, node: NodeId) -> String {
        let location = self.source_location(node);
        if location == "<unknown>:0:0" {
            String::new()
        } else {
            format!(" at {location}")
        }
    }

    /// Whether some clocking event inside `node` changes the clock of the
    /// assertion being lowered.
    pub(super) fn changes_clock_within(&self, node: NodeId) -> bool {
        if self.assertion_clock_changes.is_empty() {
            return false;
        }
        let mut stack = vec![node];
        let mut seen = HashSet::new();
        while let Some(node) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            if self.assertion_clock_changes.contains(&node) {
                return true;
            }
            stack.extend(self.assertion_operands(node));
        }
        false
    }
}
