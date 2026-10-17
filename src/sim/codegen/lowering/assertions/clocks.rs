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
    /// The clock of an assertion clocking event.
    pub(super) fn assertion_event_clock(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<SampledClock, String> {
        let NodeKind::AssertionExpr(AssertionExprKind::Clocking {
            signal,
            posedge,
            gate,
            ..
        }) = self.kind(node)
        else {
            return Err(format!(
                "assertion clocking event expected at {}",
                self.source_location(node)
            ));
        };
        let (signal, posedge, gate) = (*signal, *posedge, *gate);
        let signal = self.lower_assertion_signal(path, signal, "clock")?;
        Ok(SampledClock {
            signal,
            posedge,
            gate,
        })
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
                Ok(self
                    .semantic_leading_clocks_inner(path, expr, seen)?
                    .under(clock))
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
        self.collect_clock_changes_inner(path, root, context, true, &mut active, &mut changes)?;
        Ok(changes)
    }

    fn collect_clock_changes_inner(
        &mut self,
        path: &str,
        node: NodeId,
        flowing: Option<SampledClock>,
        top: bool,
        active: &mut HashSet<NodeId>,
        changes: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        if !active.insert(node) {
            return Ok(());
        }
        let mut flowing = flowing;
        let mut top = top;
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { .. }) => {
                if !self.clocking_nullified(node) {
                    let clock = self.assertion_event_clock(path, node)?;
                    if !top && flowing.is_some_and(|flowing| !same_clock(&flowing, &clock)) {
                        changes.insert(node);
                    }
                    flowing = Some(clock);
                }
            }
            NodeKind::AssertionExpr(kind) => {
                let keeps_top = transparent_body(kind).is_some();
                top &= keeps_top || self.assertion_instance_body(node).is_some();
            }
            _ => top = false,
        }
        for operand in self.assertion_operands(node) {
            self.collect_clock_changes_inner(path, operand, flowing, top, active, changes)?;
        }
        active.remove(&node);
        Ok(())
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
