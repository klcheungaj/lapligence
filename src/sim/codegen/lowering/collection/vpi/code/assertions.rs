//! Assertion statement objects of the VPI catalog (IEEE 1800-2009 37.45,
//! 37.46, 37.48, 37.51): the clocking event, disable condition, property
//! and action statements of each assertion item.
//!
//! An item's entry is not a handle of its own (the assertion handle is the
//! coverage item); it holds the relations of 37.46 and 37.51 as children.
//! Sequence and property operators are operations with the operands of
//! 37.48 and 37.50 (decision A19-D59).

use super::*;
use crate::core::db::{
    AssertionBinaryOp, AssertionExprKind, AssertionRange, AssertionRepetition,
    AssertionRepetitionKind, AssertionUnaryOp,
};
use crate::sim::codegen::lowering::assertions::ProcedureClock;

/// Where the clocking event of an assertion comes from (37.46 detail 1: the
/// actual clocking event, explicit or inferred).
pub(super) enum ClockSource<'a> {
    /// An event list written in the source: an assertion clocking event or
    /// a default clocking block.
    Written(&'a [EventSpec], NodeId),
    /// The edge inferred from the enclosing procedure (16.15.6).
    Procedure(EventSpec, NodeId),
}

impl<'c, 'a> CodeTables<'c, 'a> {
    /// The top clocking event of property `node`: the one that clocks the
    /// whole property, reached through `disable iff`, asynchronous aborts
    /// and the body of a sequence or property instance. `None` when the
    /// clock is inherited from the context.
    fn top_clocking(&self, mut node: NodeId) -> Option<(&'a [EventSpec], NodeId)> {
        let cg = self.cg;
        loop {
            match cg.kind(node) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    control,
                    signal,
                    specs,
                    ..
                }) => {
                    // `$inferred_clock` stands for the context clock.
                    if matches!(cg.kind(*signal), NodeKind::SysCall { .. }) {
                        return None;
                    }
                    return Some((specs.as_slice(), *control));
                }
                NodeKind::AssertionExpr(
                    AssertionExprKind::DisableIff { expr, .. }
                    | AssertionExprKind::Abort {
                        expr, sync: false, ..
                    },
                ) => node = *expr,
                _ => node = self.instance_body(node)?,
            }
        }
    }

    /// The `disable iff` written in the statement whose property is `node`
    /// (under its clocking event). A condition inside a named property or
    /// a `default disable iff` is not the statement's (decision A17-D4).
    fn statement_disable(&self, mut node: NodeId) -> Option<NodeId> {
        loop {
            match self.cg.kind(node) {
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, .. }) => {
                    return Some(*condition)
                }
                NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) => node = *expr,
                _ => return None,
            }
        }
    }

    /// The instance a property expression consists of, if it is exactly
    /// one sequence or property instance.
    pub(super) fn sole_instance(&self, node: NodeId) -> Option<NodeId> {
        let cg = self.cg;
        let instance = match cg.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => *expr,
            _ => node,
        };
        matches!(
            cg.kind(instance),
            NodeKind::Expr(ExprKind::AssertionInstance { .. })
        )
        .then_some(instance)
    }

    /// The declaration an instance names and its kind (`"Sequence"` or
    /// `"Property"`, as the frontend details it; `""` otherwise).
    pub(super) fn instance_declaration(&self, instance: NodeId) -> (NodeId, &'a str) {
        let cg = self.cg;
        let target = match cg.kind(instance) {
            NodeKind::Expr(ExprKind::AssertionInstance { target, .. }) => *target,
            _ => instance,
        };
        (target, cg.db.semantic_detail(target).unwrap_or(""))
    }

    fn instance_body(&self, node: NodeId) -> Option<NodeId> {
        match self.cg.kind(self.sole_instance(node)?) {
            NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => Some(*body),
            _ => None,
        }
    }

    /// The actual clocking event of the concurrent assertion `statement`
    /// with property `property`, and whether it is inferred: written at the
    /// top of the property, else the procedure's edge, else the default
    /// clocking (16.15.6, 16.17).
    pub(super) fn assertion_clock(
        &self,
        statement: NodeId,
        property: NodeId,
    ) -> Option<(ClockSource<'a>, bool)> {
        let cg = self.cg;
        if let Some((specs, at)) = self.top_clocking(property) {
            return Some((ClockSource::Written(specs, at), false));
        }
        let mut current = cg.node(statement).parent;
        while let Some(node) = current {
            if matches!(cg.kind(node), NodeKind::Process { .. }) {
                let body = cg.node(node).children.first().copied();
                if let Some(ProcedureClock::Edge(spec)) =
                    body.and_then(|body| cg.infer_procedure_clock(body))
                {
                    return Some((ClockSource::Procedure(spec, statement), true));
                }
                break;
            }
            current = cg.node(node).parent;
        }
        let instance = cg.owning_inst(statement)?;
        let block = cg.default_clocking_block(instance)?;
        let info = cg.db.clocking_block(block)?;
        Some((ClockSource::Written(&info.event_specs, info.event), true))
    }

    /// The entry of assertion statement `id`: its type and the children of
    /// 37.46 (concurrent) or 37.51 (immediate).
    pub(super) fn assertion(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        // An empty action `;` is no action statement (A17-D4).
        let stmt = |this: &Self, role, id: Option<NodeId>| -> Option<(&'static str, Item<'a>)> {
            let id = id.filter(|id| !matches!(cg.kind(*id), NodeKind::Stmt(StmtKind::Empty)))?;
            Some((role, this.statement_item(id)?))
        };
        match cg.kind(id) {
            NodeKind::Stmt(StmtKind::ImmediateAssertion {
                kind,
                cond,
                if_true,
                if_false,
                ..
            }) => {
                let type_code = match kind {
                    crate::core::db::ImmediateAssertionKind::Assert => "vpiImmediateAssert",
                    crate::core::db::ImmediateAssertionKind::Assume => "vpiImmediateAssume",
                    crate::core::db::ImmediateAssertionKind::Cover => "vpiImmediateCover",
                };
                let code = self.located(type_code, id);
                let mut children = vec![("vpiExpr", Item::Expr(*cond))];
                children.extend(stmt(self, "vpiStmt", *if_true));
                children.extend(stmt(self, "vpiElseStmt", *if_false));
                (code, children)
            }
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind,
                property,
                if_true,
                if_false,
                ..
            }) => {
                use crate::core::db::ConcurrentAssertionKind as Kind;
                let type_code = match kind {
                    Kind::Assume => "vpiAssume",
                    Kind::Cover | Kind::CoverSequence => "vpiCover",
                    _ => "vpiAssert",
                };
                let mut code = self.located(type_code, id);
                let mut children = Vec::new();
                if let Some((clock, inferred)) = self.assertion_clock(id, *property) {
                    if inferred {
                        code.flags |= IrVpiCode::INFERRED;
                    }
                    children.push(("vpiClockingEvent", Item::Clock(clock)));
                }
                if let Some(condition) = self.statement_disable(*property) {
                    children.push(("vpiDisableCondition", Item::Expr(condition)));
                }
                children.push(("vpiProperty", Item::Property(*property, id)));
                children.extend(stmt(self, "vpiStmt", *if_true));
                children.extend(stmt(self, "vpiElseStmt", *if_false));
                (code, children)
            }
            _ => self.opaque("vpiAssert", id),
        }
    }

    /// The `vpiProperty` of a concurrent assertion: a property instance
    /// when the statement's property is exactly one, else a property
    /// specification (37.46) with the clocking event and disable condition
    /// written in the statement and the property expression (37.48).
    pub(super) fn property(
        &mut self,
        property: NodeId,
        statement: NodeId,
    ) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        if let Some(instance) = self
            .sole_instance(property)
            .filter(|instance| self.instance_declaration(*instance).1 == "Property")
        {
            return self.assertion_expression(instance);
        }
        let code = self.located("vpiPropertySpec", statement);
        let mut children: Children<'a> = Vec::new();
        let mut inner = property;
        loop {
            match cg.kind(inner) {
                NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                    control,
                    signal,
                    specs,
                    expr,
                    ..
                }) if !children.iter().any(|(role, _)| *role == "vpiClockingEvent") => {
                    if !matches!(cg.kind(*signal), NodeKind::SysCall { .. }) {
                        children.push((
                            "vpiClockingEvent",
                            Item::Clock(ClockSource::Written(specs, *control)),
                        ));
                    }
                    inner = *expr;
                }
                NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, expr })
                    if !children
                        .iter()
                        .any(|(role, _)| *role == "vpiDisableCondition") =>
                {
                    children.push(("vpiDisableCondition", Item::Expr(*condition)));
                    inner = *expr;
                }
                _ => break,
            }
        }
        children.push(("vpiPropertyExpr", Item::AssertionExpr(inner)));
        (code, children)
    }

    /// A sequence or property expression (37.48, 37.50): a Boolean
    /// expression is an expression object, a named sequence or property
    /// an instance object and an operator an operation with its operands.
    pub(super) fn assertion_expression(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        if let Some(instance) = self.sole_instance(id) {
            return self.instance(instance);
        }
        match cg.kind(id) {
            NodeKind::AssertionExpr(kind) => self.assertion_operator(id, kind),
            _ => self.expression(id),
        }
    }

    /// The operation of sequence or property operator `id`. The operands
    /// follow the BNF except where 37.48 detail 2 and 37.50 detail 3 put
    /// the ranges last; `vpiOpStrong` applies to the temporal operators of
    /// 37.48 detail 3 and to sequence operations (decision A19-D59).
    fn assertion_operator(
        &mut self,
        id: NodeId,
        kind: &'a AssertionExprKind,
    ) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let operation = |this: &mut Self, op: &'static str, flags: u32| {
            let mut code = this.assertion_located("vpiOperation", id);
            code.subtype = op;
            code.flags |= IrVpiCode::TEMPORAL | flags;
            code
        };
        let strength =
            |strong: bool| IrVpiCode::STRENGTH | if strong { IrVpiCode::STRONG } else { 0 };
        let operand = |item| ("vpiOperand", item);
        match kind {
            AssertionExprKind::Simple {
                expr,
                repetition: None,
                ..
            } => self.assertion_expression(*expr),
            AssertionExprKind::Simple {
                expr,
                repetition: Some(repetition),
                ..
            } => self.repetition(id, Item::AssertionExpr(*expr), repetition),
            AssertionExprKind::SequenceWithMatch {
                repetition: Some(repetition),
                ..
            } => self.repetition(id, Item::Matched(id), repetition),
            AssertionExprKind::SequenceWithMatch { .. } => self.matched(id),
            AssertionExprKind::SequenceConcat { elements, .. } => self.concat(id, elements.len()),
            AssertionExprKind::Unary {
                op, expr, range, ..
            } => {
                let (name, flags) = match op {
                    AssertionUnaryOp::Not => ("vpiNotOp", 0),
                    AssertionUnaryOp::NextTime => ("vpiNexttimeOp", strength(false)),
                    AssertionUnaryOp::SNextTime => ("vpiNexttimeOp", strength(true)),
                    AssertionUnaryOp::Always => ("vpiAlwaysOp", strength(false)),
                    AssertionUnaryOp::SAlways => ("vpiAlwaysOp", strength(true)),
                    AssertionUnaryOp::Eventually => ("vpiEventuallyOp", strength(false)),
                    AssertionUnaryOp::SEventually => ("vpiEventuallyOp", strength(true)),
                };
                let code = operation(self, name, flags);
                let mut children = vec![operand(Item::AssertionExpr(*expr))];
                match (op, range) {
                    (AssertionUnaryOp::Not, _) | (_, None) => {}
                    // 37.48 detail 2: the constant only when it is not 1.
                    (AssertionUnaryOp::NextTime | AssertionUnaryOp::SNextTime, Some(range)) => {
                        if range.min != 1 {
                            children.push(operand(Item::Bound(Some(range.min), id)));
                        }
                    }
                    (_, Some(range)) => {
                        children.push(operand(Item::Bound(Some(range.min), id)));
                        children.push(operand(Item::Bound(range.max, id)));
                    }
                }
                (code, children)
            }
            AssertionExprKind::Binary { op, left, right } => {
                let (name, flags) = match op {
                    AssertionBinaryOp::And | AssertionBinaryOp::Or => {
                        let sequence = self.is_sequence(*left) && self.is_sequence(*right);
                        let name = if *op == AssertionBinaryOp::And {
                            "vpiCompAndOp"
                        } else {
                            "vpiCompOrOp"
                        };
                        (name, if sequence { strength(false) } else { 0 })
                    }
                    AssertionBinaryOp::Intersect => ("vpiIntersectOp", strength(false)),
                    AssertionBinaryOp::Throughout => ("vpiThroughoutOp", strength(false)),
                    AssertionBinaryOp::Within => ("vpiWithinOp", strength(false)),
                    AssertionBinaryOp::Iff => ("vpiIffOp", 0),
                    AssertionBinaryOp::Until => ("vpiUntilOp", strength(false)),
                    AssertionBinaryOp::SUntil => ("vpiUntilOp", strength(true)),
                    AssertionBinaryOp::UntilWith => ("vpiUntilWithOp", strength(false)),
                    AssertionBinaryOp::SUntilWith => ("vpiUntilWithOp", strength(true)),
                    AssertionBinaryOp::Implies => ("vpiImpliesOp", 0),
                    AssertionBinaryOp::OverlappedImplication => ("vpiOverlapImplyOp", 0),
                    AssertionBinaryOp::NonOverlappedImplication => ("vpiNonOverlapImplyOp", 0),
                    AssertionBinaryOp::OverlappedFollowedBy => ("vpiOverlapFollowedByOp", 0),
                    AssertionBinaryOp::NonOverlappedFollowedBy => ("vpiNonOverlapFollowedByOp", 0),
                };
                let code = operation(self, name, flags);
                let children = vec![
                    operand(Item::AssertionExpr(*left)),
                    operand(Item::AssertionExpr(*right)),
                ];
                (code, children)
            }
            AssertionExprKind::FirstMatch {
                sequence,
                match_items,
            } => {
                let code = operation(self, "vpiFirstMatchOp", strength(false));
                // The match items belong to the sequence they follow.
                let inner = if match_items.is_empty() {
                    Item::AssertionExpr(*sequence)
                } else {
                    Item::Matched(id)
                };
                (code, vec![operand(inner)])
            }
            AssertionExprKind::Clocking {
                signal,
                control,
                specs,
                expr,
                ..
            } => {
                // `$inferred_clock` stands for the context clock.
                if matches!(cg.kind(*signal), NodeKind::SysCall { .. }) {
                    return self.assertion_expression(*expr);
                }
                let (type_code, role) = if self.is_sequence(*expr) {
                    ("vpiClockedSeq", "vpiExpr")
                } else {
                    ("vpiClockedProp", "vpiPropertyExpr")
                };
                let code = self.assertion_located(type_code, id);
                let children = vec![
                    (
                        "vpiClockingEvent",
                        Item::Clock(ClockSource::Written(specs, *control)),
                    ),
                    (role, Item::AssertionExpr(*expr)),
                ];
                (code, children)
            }
            AssertionExprKind::StrongWeak { expr, strong } => {
                // 37.48 detail 3: `strong(s)` is sequence `s` with
                // vpiOpStrong TRUE, `weak(s)` with FALSE.
                let (mut code, children) = self.assertion_expression(*expr);
                if code.flags & IrVpiCode::STRENGTH != 0 && *strong {
                    code.flags |= IrVpiCode::STRONG;
                }
                (code, children)
            }
            AssertionExprKind::Abort {
                condition,
                expr,
                reject,
                sync,
            } => {
                let name = match (reject, sync) {
                    (false, false) => "vpiAcceptOnOp",
                    (true, false) => "vpiRejectOnOp",
                    (false, true) => "vpiSyncAcceptOnOp",
                    (true, true) => "vpiSyncRejectOnOp",
                };
                let code = operation(self, name, 0);
                let children = vec![
                    operand(Item::Expr(*condition)),
                    operand(Item::AssertionExpr(*expr)),
                ];
                (code, children)
            }
            AssertionExprKind::Conditional {
                condition,
                if_expr,
                else_expr,
            } => {
                let name = if else_expr.is_some() {
                    "vpiIfElseOp"
                } else {
                    "vpiIfOp"
                };
                let code = operation(self, name, 0);
                let mut children = vec![
                    operand(Item::Expr(*condition)),
                    operand(Item::AssertionExpr(*if_expr)),
                ];
                children.extend(else_expr.map(|id| operand(Item::AssertionExpr(id))));
                (code, children)
            }
            AssertionExprKind::Case {
                expr,
                items,
                default_case,
            } => {
                let code = self.assertion_located("vpiCaseProperty", id);
                let mut children = vec![("vpiCondition", Item::Expr(*expr))];
                children.extend(items.iter().map(|item| {
                    (
                        "vpiCasePropertyItem",
                        Item::CaseBranch(&item.expressions, item.body),
                    )
                }));
                // 37.48 detail 5: the default item has no expressions.
                children.extend(
                    default_case.map(|body| ("vpiCasePropertyItem", Item::CaseBranch(&[], body))),
                );
                (code, children)
            }
            AssertionExprKind::DisableIff { .. } | AssertionExprKind::Invalid { .. } => {
                let mut code = IrVpiCode::new("vpiOperation");
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
        }
    }

    /// An entry of `type_code` at sequence or property expression `id`. The
    /// frontend gives an operator node no position of its own: it is at
    /// the first node under it that has one.
    fn assertion_located(&mut self, type_code: &'static str, id: NodeId) -> IrVpiCode {
        let at = self.positioned(id).unwrap_or(id);
        self.located(type_code, at)
    }

    /// `id`, or the first node under it with a source position.
    fn positioned(&self, id: NodeId) -> Option<NodeId> {
        let cg = self.cg;
        if cg.node(id).line != 0 {
            return Some(id);
        }
        let NodeKind::AssertionExpr(kind) = cg.kind(id) else {
            return None;
        };
        let mut nodes = Vec::new();
        kind.referenced_nodes(&mut nodes);
        nodes.into_iter().find_map(|node| self.positioned(node))
    }

    /// Whether assertion expression `id` is a sequence (16.7), not a
    /// property: it decides between a clocked sequence and a clocked
    /// property and whether `and` and `or` are sequence operations.
    fn is_sequence(&self, id: NodeId) -> bool {
        let cg = self.cg;
        if let Some(instance) = self.sole_instance(id) {
            return self.instance_declaration(instance).1 != "Property";
        }
        match cg.kind(id) {
            NodeKind::AssertionExpr(kind) => match kind {
                AssertionExprKind::Simple { expr, .. } => self.is_sequence(*expr),
                AssertionExprKind::SequenceConcat { .. }
                | AssertionExprKind::SequenceWithMatch { .. }
                | AssertionExprKind::FirstMatch { .. } => true,
                AssertionExprKind::Binary { op, left, right } => match op {
                    AssertionBinaryOp::Intersect
                    | AssertionBinaryOp::Throughout
                    | AssertionBinaryOp::Within => true,
                    AssertionBinaryOp::And | AssertionBinaryOp::Or => {
                        self.is_sequence(*left) && self.is_sequence(*right)
                    }
                    _ => false,
                },
                AssertionExprKind::Clocking { expr, .. } => self.is_sequence(*expr),
                _ => false,
            },
            _ => true,
        }
    }

    /// A repetition (`[*]`, `[=]`, `[->]`) of `operand`: the sequence, then
    /// the bounds (37.50 detail 3).
    fn repetition(
        &mut self,
        id: NodeId,
        operand: Item<'a>,
        repetition: &AssertionRepetition,
    ) -> (IrVpiCode, Children<'a>) {
        let mut code = self.assertion_located("vpiOperation", id);
        code.subtype = match repetition.kind {
            AssertionRepetitionKind::Consecutive => "vpiConsecutiveRepeatOp",
            AssertionRepetitionKind::Nonconsecutive => "vpiRepeatOp",
            AssertionRepetitionKind::GoTo => "vpiGotoRepeatOp",
        };
        code.flags |= IrVpiCode::TEMPORAL | IrVpiCode::STRENGTH;
        let mut children = vec![("vpiOperand", operand)];
        children.extend(self.bounds(&repetition.range, id));
        (code, children)
    }

    /// The bound operands of a delay or repetition range: the left bound,
    /// and the right one only when it differs (37.50 detail 3).
    fn bounds(&self, range: &AssertionRange, at: NodeId) -> Children<'a> {
        let mut children = vec![("vpiOperand", Item::Bound(Some(range.min), at))];
        if range.max != Some(range.min) {
            children.push(("vpiOperand", Item::Bound(range.max, at)));
        }
        children
    }

    /// A range bound: an integer constant, or `$` (an unbounded constant).
    pub(super) fn bound(&mut self, bound: Option<u32>, at: NodeId) -> IrVpiCode {
        let mut code = self.assertion_located("vpiConstant", at);
        match bound {
            Some(count) => {
                code.subtype = "vpiDecConst";
                code.flags |= IrVpiCode::SIGNED;
                code.width = 32;
                code.text = Some(format!("{count:032b}"));
            }
            None => {
                code.subtype = "vpiUnboundedConst";
                code.flags |= IrVpiCode::OPAQUE;
                code.text = Some("$".to_owned());
            }
        }
        code
    }

    /// The first `count` elements of sequence concatenation `id`, left
    /// associated: `a ##1 b ##2 c` is `(a ##1 b) ##2 c`. A concatenation
    /// that starts with a delay starts with a unary cycle delay. Operands:
    /// the sequences, then the delay's bounds (37.50 detail 3).
    pub(super) fn concat(&mut self, id: NodeId, count: usize) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, delays }) =
            cg.kind(id)
        else {
            return self.opaque("vpiOperation", id);
        };
        let (Some(element), Some(delay)) = (elements.get(count - 1), delays.get(count - 1)) else {
            return self.opaque("vpiOperation", id);
        };
        let mut code = self.assertion_located("vpiOperation", id);
        code.flags |= IrVpiCode::TEMPORAL | IrVpiCode::STRENGTH;
        let mut children = Vec::new();
        if count == 1 {
            if !self.leading_delay(id) {
                return self.assertion_expression(*element);
            }
            code.subtype = "vpiUnaryCycleDelayOp";
        } else {
            code.subtype = "vpiCycleDelayOp";
            children.push(("vpiOperand", Item::Concat(id, count - 1)));
        }
        children.push(("vpiOperand", Item::AssertionExpr(*element)));
        children.extend(self.bounds(delay, id));
        (code, children)
    }

    /// Whether concatenation `id` starts with a cycle delay (`##1 a`): the
    /// frontend gives its first element a `##0` delay otherwise.
    fn leading_delay(&mut self, id: NodeId) -> bool {
        let cg = self.cg;
        let NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, delays }) =
            cg.kind(id)
        else {
            return false;
        };
        if elements.len() == 1 || delays.first().is_some_and(|delay| delay.max != Some(0)) {
            return true;
        }
        let node = cg.node(id);
        self.source_at(id, node.line, node.col)
            .is_some_and(|text| text.starts_with("##"))
    }

    /// The source of `id` from `(line, column)` (1-based) to the end of its
    /// file.
    fn source_at(&mut self, id: NodeId, line: u32, column: u32) -> Option<&'a str> {
        let file = self.cg.node(id).file()?;
        let source = self.cg.db.source_text(file)?;
        let starts = self.line_starts.entry(file.to_owned()).or_insert_with(|| {
            std::iter::once(0)
                .chain(source.match_indices('\n').map(|(at, _)| at + 1))
                .collect()
        });
        let start = *starts.get(usize::try_from(line).ok()?.checked_sub(1)?)?
            + usize::try_from(column).ok()?.checked_sub(1)?;
        source.get(start..)
    }

    /// The sequence of a sequence with match items or a `first_match`
    /// (`id`) with its match items (37.50 vpiMatchItem). A Boolean
    /// expression that is a catalog object cannot hold them: it stands
    /// without them.
    pub(super) fn matched(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let (expr, match_items) = match cg.kind(id) {
            NodeKind::AssertionExpr(
                AssertionExprKind::SequenceWithMatch {
                    expr, match_items, ..
                }
                | AssertionExprKind::FirstMatch {
                    sequence: expr,
                    match_items,
                },
            ) => (*expr, match_items),
            _ => return self.opaque("vpiOperation", id),
        };
        let (code, mut children) = self.assertion_expression(expr);
        let own = code.flags & (IrVpiCode::ALIAS | IrVpiCode::LINK | IrVpiCode::OPAQUE) == 0;
        if own && matches!(code.type_code, "vpiOperation" | "vpiSequenceInst") {
            children.extend(
                match_items
                    .iter()
                    .map(|item| ("vpiMatchItem", Item::ExprStmt(*item))),
            );
        }
        (code, children)
    }

    /// A case property item: its expressions and its property (37.48).
    pub(super) fn case_property_item(
        &mut self,
        expressions: &'a [NodeId],
        body: NodeId,
    ) -> (IrVpiCode, Children<'a>) {
        let code = self.assertion_located("vpiCasePropertyItem", body);
        let mut children: Children<'a> = expressions
            .iter()
            .map(|expr| ("vpiExpr", Item::Expr(*expr)))
            .collect();
        children.push(("vpiPropertyExpr", Item::AssertionExpr(body)));
        (code, children)
    }

    /// The event expression of a clocking event.
    pub(super) fn clock(
        &mut self,
        clock: ClockSource<'a>,
        scope: Option<usize>,
    ) -> (IrVpiCode, Children<'a>, Option<usize>) {
        match clock {
            ClockSource::Written(specs, at) => self.build(Item::Events(specs, at), scope),
            ClockSource::Procedure(spec, at) => self.build(Item::OwnedEvent(spec, at), scope),
        }
    }

    /// A sequence or property instance (37.50, 37.52): the name of its
    /// declaration, its actual arguments in formal order (defaults
    /// included) and a link to the declaration.
    fn instance(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let NodeKind::Expr(ExprKind::AssertionInstance { bindings, .. }) = cg.kind(id) else {
            return self.opaque("vpiOperation", id);
        };
        let (declaration, kind) = self.instance_declaration(id);
        let (type_code, relation) = match kind {
            "Sequence" => ("vpiSequenceInst", "vpiSequenceDecl"),
            "Property" => ("vpiPropertyInst", "vpiPropertyDecl"),
            _ => return self.opaque("vpiOperation", id),
        };
        let mut code = self.located(type_code, id);
        code.span = self.instance_span(id);
        code.text = Some(cg.node(declaration).name.clone());
        let formals = &cg.node(declaration).children;
        let mut actuals: Vec<_> = bindings.iter().collect();
        actuals.sort_by_key(|binding| formals.iter().position(|formal| *formal == binding.formal));
        let mut children: Children<'a> = actuals
            .into_iter()
            .map(|binding| ("vpiArgument", Item::AssertionExpr(binding.actual)))
            .collect();
        if let Some(entry) = self.declarations.get(&declaration) {
            children.push((relation, Item::Link(*entry)));
        }
        // 37.52: a property instance has the disable condition its
        // declaration writes under the clocking event (decision A19-D31).
        if kind == "Property" {
            if let NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) = cg.kind(id) {
                if let Some(condition) = self.statement_disable(*body) {
                    children.push(("vpiDisableCondition", Item::Expr(condition)));
                }
            }
        }
        (code, children)
    }

    /// The 37.45 location of instance `id`: the frontend spans only its
    /// name, so the end is the parenthesis that closes its argument list in
    /// the source, when there is one.
    fn instance_span(&mut self, id: NodeId) -> Option<(u32, u32, u32)> {
        let (column, end_line, last) = self.span(id)?;
        let end = self
            .argument_list_end(id, end_line, last + 1)
            .unwrap_or((end_line, last));
        Some((column, end.0, end.1))
    }

    /// The `(line, column)` of the `)` closing the argument list that
    /// follows `(line, column)` (1-based, the column of the character after
    /// a name) in the source of `id`, skipping white space, nested
    /// parentheses and string literals; `None` without an argument list.
    fn argument_list_end(&mut self, id: NodeId, line: u32, column: u32) -> Option<(u32, u32)> {
        let file = self.cg.node(id).file()?;
        let source = self.cg.db.source_text(file)?;
        let starts = self.line_starts.entry(file.to_owned()).or_insert_with(|| {
            std::iter::once(0)
                .chain(source.match_indices('\n').map(|(at, _)| at + 1))
                .collect()
        });
        let start = *starts.get(usize::try_from(line).ok()?.checked_sub(1)?)?
            + usize::try_from(column).ok()?.checked_sub(1)?;
        let bytes = source.as_bytes();
        let mut at = start;
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        if bytes.get(at) != Some(&b'(') {
            return None;
        }
        let (mut depth, mut quoted) = (0u32, false);
        while let Some(byte) = bytes.get(at) {
            match (quoted, byte) {
                (true, b'\\') => at += 1,
                (true, b'"') | (false, b'"') => quoted = !quoted,
                (false, b'(') => depth += 1,
                (false, b')') => {
                    depth -= 1;
                    if depth == 0 {
                        let row = starts.partition_point(|line_start| *line_start <= at);
                        let column = at - starts[row - 1] + 1;
                        return Some((u32::try_from(row).ok()?, u32::try_from(column).ok()?));
                    }
                }
                _ => {}
            }
            at += 1;
        }
        None
    }

    /// The 37.45 location of `id` after its start line: its start column
    /// and where it ends. The frontend's end column is the one after the
    /// last character; 37.45 reports the last character's (A17-D4).
    pub(super) fn span(&self, id: NodeId) -> Option<(u32, u32, u32)> {
        let node = self.cg.node(id);
        (node.line != 0 && node.end_line != 0)
            .then(|| (node.col, node.end_line, node.end_col.saturating_sub(1)))
    }

    /// Add the sequence or property declaration `id` declared in catalog
    /// scope `scope` as a root (37.47, 37.49), once.
    pub(in super::super) fn declaration_root(&mut self, id: NodeId, scope: Option<usize>) {
        if !self.declarations.contains_key(&id) {
            let entry = self.root(Item::Declaration(id), scope);
            self.declarations.insert(id, entry);
        }
    }

    /// A declaration: its name and its formals in declaration order.
    pub(super) fn declaration(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let (type_code, formal) = match cg.db.semantic_detail(id) {
            Some("Sequence") => ("vpiSequenceDecl", "vpiSeqFormalDecl"),
            _ => ("vpiPropertyDecl", "vpiPropFormalDecl"),
        };
        let mut code = self.located(type_code, id);
        code.text = Some(cg.node(id).name.clone());
        let children = cg
            .node(id)
            .children
            .iter()
            .filter(|child| cg.db.semantic_detail(**child) == Some("AssertionPort"))
            .map(|child| (formal, Item::AssertionFormal(*child, formal)))
            .collect();
        (code, children)
    }

    /// A formal argument (37.47 details 2 and 4, 37.49 details 2 and 4): its
    /// name, its direction (`vpiNoDirection` unless it is a local variable
    /// argument) and the typespec of a typed formal.
    pub(super) fn formal(&mut self, id: NodeId, type_code: &'static str) -> IrVpiCode {
        let cg = self.cg;
        let mut code = self.located(type_code, id);
        code.text = Some(cg.node(id).name.clone());
        let NodeKind::FuncArg { direction, ty, .. } = cg.kind(id) else {
            return code;
        };
        code.subtype = match direction {
            Direction::Input => "vpiInput",
            Direction::Output => "vpiOutput",
            Direction::Inout => "vpiInout",
            _ => "vpiNoDirection",
        };
        if ty.type_name.as_deref() != Some("untyped") {
            code.typespec = cg
                .db
                .type_descriptor(id)
                .and_then(|descriptor| cg.vpi_typespec(self.typespecs, descriptor, None, 0));
        }
        code
    }
}
