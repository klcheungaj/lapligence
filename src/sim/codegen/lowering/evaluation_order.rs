//! Operand evaluation order where IEEE 1800-2009 leaves it open.
//!
//! The standard leaves undefined the order of assignment operations within
//! an expression (11.4.2), of the members of a set (11.4.13) and of the
//! actuals of a call (13.5). llg evaluates operands left to right and
//! follows the industry-standard behaviour where it differs, but only where
//! the difference can be observed: in a statement whose operands call a user
//! subroutine or hold an assignment operator ([`Codegen::evaluation_effect`]).
//! Statements without such an effect lower as before.
//!
//! An assignment operator (`=`, `op=`) nested in an expression runs before
//! the other operands of the expression it is part of, left to right and an
//! assignment after the ones its own operands hold, and the expression reads
//! the value it assigned (decision EO-D1). The expression is the operands a
//! statement evaluates when it starts, or one evaluation of a loop
//! condition. An operand that may be skipped or that is evaluated per element
//! (of `?:`, `&&`, `||`, `->`, `inside`, an array method's `with` clause)
//! keeps the assignments it holds in place. Increments and decrements run in
//! place.

use super::*;
use crate::sim::ir::{IrObjectStmt, IrStringExpr};

impl Codegen<'_> {
    /// Whether `node`, once its operands are evaluated, has an effect that
    /// another operand can observe: a call of a user subroutine or an
    /// assignment operator (also an increment, a decrement or the copy-out
    /// of an output actual).
    pub(in crate::sim::codegen) fn evaluation_effect(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::FuncCall {
                callee: Some(callee),
                ..
            }
            | NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => matches!(self.kind(*callee), NodeKind::FuncTask { .. }),
            NodeKind::Expr(ExprKind::Operation { op, assignment, .. }) => {
                *assignment || Self::assignment_operator(*op)
            }
            _ => false,
        }
    }

    /// Whether evaluating `root` has an [effect](Self::evaluation_effect)
    /// below its root. A called subroutine at the root counts: it runs
    /// before its output actuals are copied out. Nested statements are not
    /// operands.
    pub(in crate::sim::codegen) fn has_evaluation_effect(&self, root: NodeId) -> bool {
        let mut pending: Vec<NodeId> = match self.kind(root) {
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => vec![root],
            _ => self.node(root).children.clone(),
        };
        while let Some(node) = pending.pop() {
            if matches!(self.kind(node), NodeKind::Stmt(_)) {
                continue;
            }
            if self.evaluation_effect(node) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether `node` or an operand below it has an
    /// [effect](Self::evaluation_effect).
    pub(in crate::sim::codegen) fn contains_evaluation_effect(&self, node: NodeId) -> bool {
        self.evaluation_effect(node) || self.has_evaluation_effect(node)
    }

    /// Whether `op` writes its first operand: an assignment, an increment or
    /// a decrement, or an overloaded update.
    pub(in crate::sim::codegen) fn assignment_operator(op: Operation) -> bool {
        matches!(
            op,
            Operation::Assignment
                | Operation::PostIncrement
                | Operation::PreIncrement
                | Operation::PostDecrement
                | Operation::PreDecrement
                | Operation::OverloadUpdate
                | Operation::OverloadPostUpdate
        )
    }

    /// The operands statement `h` evaluates once when it starts, in which
    /// nested assignments run first (decision EO-D1): both sides of an
    /// assignment, the actuals of a call or of an immediate system task, the
    /// operands of an expression statement, a returned value, the condition
    /// of an `if` without `&&&`, a `case` selector and a `repeat` count. A
    /// loop condition is hoisted per evaluation by its loop.
    pub(in crate::sim::codegen) fn statement_hoisting_operands(&self, h: NodeId) -> Vec<NodeId> {
        let children = &self.node(h).children;
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { .. }) => children.iter().take(2).copied().collect(),
            NodeKind::Stmt(StmtKind::Return { value }) => value.iter().copied().collect(),
            NodeKind::Stmt(StmtKind::IfElse { predicate, .. }) => {
                match predicate.clauses.as_slice() {
                    [clause] if clause.pattern.is_none() => vec![clause.expression],
                    _ => Vec::new(),
                }
            }
            NodeKind::Stmt(StmtKind::Case { .. }) => {
                children.first().copied().into_iter().collect()
            }
            NodeKind::Stmt(StmtKind::Repeat { cond, .. }) => vec![*cond],
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => children.clone(),
            NodeKind::SysCall { name } if !super::statements::deferred_display_task(name) => {
                children.clone()
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands.clone(),
            _ => Vec::new(),
        }
    }

    /// The nested assignment operators that the evaluation of `operands`
    /// runs first (decision EO-D1), in the order they run.
    pub(in crate::sim::codegen) fn hoisted_assignment_nodes(
        &self,
        operands: &[NodeId],
    ) -> Vec<NodeId> {
        let mut out = Vec::new();
        // Post-order, left to right: an assignment after those its own
        // operands hold. `true` marks a node whose operands are done.
        let mut pending: Vec<(NodeId, bool)> =
            operands.iter().rev().map(|node| (*node, false)).collect();
        while let Some((node, done)) = pending.pop() {
            if done {
                if self.hoistable_assignment(node) {
                    out.push(node);
                }
                continue;
            }
            let operands = self.hoisting_operands(node);
            pending.push((node, true));
            pending.extend(operands.into_iter().rev().map(|operand| (operand, false)));
        }
        out
    }

    /// The operands of `node` that the walk of
    /// [`Self::hoisted_assignment_nodes`] enters: none of a nested statement
    /// or of an operator that may skip or repeat its operands, the receiver
    /// alone of an array method with a `with` clause, and the target of a
    /// copied-out actual.
    fn hoisting_operands(&self, node: NodeId) -> Vec<NodeId> {
        match self.kind(node) {
            NodeKind::Stmt(_)
            | NodeKind::Expr(ExprKind::Conditional { .. })
            | NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::LogicalAnd
                    | Operation::LogicalOr
                    | Operation::Imply
                    | Operation::Conditional
                    | Operation::Inside
                    | Operation::OverloadUpdate
                    | Operation::OverloadPostUpdate,
                ..
            }) => Vec::new(),
            NodeKind::Expr(ExprKind::Operation { operands, .. }) if self.copy_out_wrapper(node) => {
                operands.first().copied().into_iter().collect()
            }
            NodeKind::MethodCall { receiver, .. } if self.db.method_call_has_with_clause(node) => {
                receiver.iter().copied().collect()
            }
            _ => self.node(node).children.clone(),
        }
    }

    /// Whether `node` is the assignment from the formal that the frontend
    /// wraps an output or inout actual in (its value is an empty argument).
    fn copy_out_wrapper(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if operands.get(1).is_some_and(|value| {
                self.db.semantic_detail(*value) == Some("EmptyArgument")
            })
        )
    }

    /// Whether `node` is the target of a copied-out actual.
    pub(in crate::sim::codegen) fn copy_out_wrapper_target(&self, node: NodeId) -> bool {
        self.node(node)
            .parent
            .is_some_and(|parent| self.copy_out_wrapper(parent))
    }

    /// Whether `node` is a simple or compound assignment operator whose
    /// target holds a packed or real value, the forms an expression can
    /// yield.
    fn hoistable_assignment(&self, node: NodeId) -> bool {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            assignment,
            operands,
            ..
        }) = self.kind(node)
        else {
            return false;
        };
        if !(*op == Operation::Assignment || *assignment)
            || matches!(
                op,
                Operation::PreIncrement
                    | Operation::PostIncrement
                    | Operation::PreDecrement
                    | Operation::PostDecrement
                    | Operation::OverloadUpdate
                    | Operation::OverloadPostUpdate
            )
            || self.copy_out_wrapper(node)
        {
            return false;
        }
        let Some(target) = operands.first() else {
            return false;
        };
        match self
            .query_descriptor(*target)
            .map(|descriptor| &descriptor.shape)
        {
            None | Some(TypeShape::PackedAtom { .. } | TypeShape::Real { .. }) => true,
            Some(TypeShape::Aggregate(layout)) => matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::PackedUnion
            ),
            Some(_) => false,
        }
    }

    /// Run the nested assignments `nodes` (from
    /// [`Self::hoisted_assignment_nodes`]) into temporaries that later reads
    /// of the nodes return. The caller ends them with
    /// [`Self::end_hoisted_assignments`] once the operands are lowered.
    pub(in crate::sim::codegen) fn hoist_assignments(
        &mut self,
        path: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = Vec::with_capacity(nodes.len());
        for node in nodes {
            let value = self.lower_expr(path, *node)?;
            let (width, signed) = (value.width, value.signed);
            let name = format!("_llg_na{}", node.0);
            statements.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            });
            self.hoisted_values
                .insert(*node, HoistedValue::Packed(name, width, signed));
            self.hoisted_assignments.insert(*node, false);
        }
        Ok(statements)
    }

    /// Note that a lowering read the value nested assignment `node` left.
    pub(in crate::sim::codegen) fn note_hoisted_read(&mut self, node: NodeId) {
        if let Some(read) = self.hoisted_assignments.get_mut(&node) {
            *read = true;
        }
    }

    /// Forget the nested assignments `nodes`. One no lowering read would
    /// have run twice had another path lowered it in place, so it is an
    /// error rather than a dropped value.
    pub(in crate::sim::codegen) fn end_hoisted_assignments(
        &mut self,
        nodes: &[NodeId],
    ) -> Result<(), String> {
        let mut unread = None;
        for node in nodes {
            self.hoisted_values.remove(node);
            if self.hoisted_assignments.remove(node) == Some(false) && unread.is_none() {
                unread = Some(*node);
            }
        }
        match unread {
            Some(node) => Err(self.locate_error(
                node,
                "an assignment operator in this operand of an expression is not supported; assign the value in a statement of its own first".to_owned(),
            )),
            None => Ok(()),
        }
    }

    /// Lower `node`, an expression evaluated as a whole each time it runs (a
    /// loop condition), with the nested assignments it holds run before its
    /// other operands (decision EO-D1). The handles its class properties
    /// select through are evaluated before them, as a statement's are.
    pub(in crate::sim::codegen) fn lower_hoisting_expression(
        &mut self,
        path: &str,
        node: NodeId,
        lower: impl FnOnce(&mut Self) -> Result<IrExpr, String>,
    ) -> Result<IrExpr, String> {
        let nodes = self.hoisted_assignment_nodes(&[node]);
        if nodes.is_empty() {
            return lower(self);
        }
        let receivers = self.enter_receiver_root(node);
        let mut statements = receivers.clone().unwrap_or_default();
        let value = self.hoist_assignments(path, &nodes).and_then(|hoisted| {
            statements.extend(hoisted);
            lower(self)
        });
        let ended = self.end_hoisted_assignments(&nodes);
        if let Some(receivers) = &receivers {
            self.leave_receiver_root(node, receivers);
        }
        let value = value?;
        ended?;
        Ok(super::expressions::after_statements(statements, value))
    }
}

/// An input actual a call evaluates into a temporary before the copy-ins
/// and bindings of the formals after it (decision EO-D3), as the call reads
/// it.
pub(in crate::sim::codegen) enum EarlyActual {
    Packed(IrExpr),
    String(IrStringExpr),
    Handle(IrChandleExpr),
}

impl EarlyActual {
    /// The argument the call passes.
    pub(in crate::sim::codegen) fn argument(self) -> IrCallArg {
        match self {
            EarlyActual::Packed(value) => IrCallArg::Val(value),
            EarlyActual::String(value) => IrCallArg::StringVal(value),
            EarlyActual::Handle(value) => IrCallArg::ChandleVal(value),
        }
    }
}

impl Codegen<'_> {
    /// The actuals of a call evaluate left to right, an inout actual copied
    /// in and a `ref` actual bound at its place (decision EO-D3). A call
    /// copies in and binds when it starts and evaluates its inputs then, so
    /// where an input with an [effect](Self::evaluation_effect) precedes
    /// such an actual, the actuals before the last such actual are
    /// evaluated first, in order, into temporaries: the index of that
    /// actual, or `None` when the call's own order already is the order of
    /// its actuals.
    pub(in crate::sim::codegen) fn early_actuals_until(
        &self,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Option<usize> {
        let mut effect = false;
        let mut last = None;
        for (idx, (io, _)) in formals.iter().enumerate() {
            let NodeKind::FuncArg { direction, .. } = self.kind(*io) else {
                continue;
            };
            match direction {
                DbDirection::Inout | DbDirection::Ref if effect => last = Some(idx),
                DbDirection::Input
                    if bound.get(idx).is_some_and(|arg| {
                        !arg.is_default && self.contains_evaluation_effect(arg.expr)
                    }) =>
                {
                    effect = true
                }
                _ => {}
            }
        }
        last
    }

    /// Evaluate input actual `idx` of a call into temporary `name` when
    /// `until` (from [`Self::early_actuals_until`]) follows it (decision
    /// EO-D3): the statement that does, and what the call reads instead.
    /// Only a caller-given packed, real, string or handle value is
    /// evaluated early; an aggregate or container input is evaluated where
    /// the call evaluates it. `arg_irs` records the formal's value for the
    /// defaults of later formals, as [`Self::lower_bound_arg`] does.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim::codegen) fn early_input_actual(
        &mut self,
        path: &str,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        idx: usize,
        until: Option<usize>,
        arg_irs: &mut Vec<Option<IrExpr>>,
        name: String,
    ) -> Result<Option<(IrStmt, EarlyActual)>, String> {
        let io = formals[idx].0;
        let early = until.is_some_and(|until| idx < until)
            && matches!(
                self.kind(io),
                NodeKind::FuncArg {
                    direction: DbDirection::Input,
                    ..
                }
            )
            && !bound[idx].is_default
            && !bound[idx].is_event;
        if !early {
            return Ok(None);
        }
        let handle =
            matches!(self.kind(io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind));
        let singular = handle
            || bound[idx].string
            || matches!(
                self.db
                    .type_descriptor(io)
                    .map(|descriptor| &descriptor.shape),
                Some(TypeShape::PackedAtom { .. } | TypeShape::Real { .. })
            );
        if !singular {
            return Ok(None);
        }
        if handle {
            let value = self.lower_handle_argument(path, io, bound[idx].expr)?;
            return Ok(Some((
                IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))),
                EarlyActual::Handle(IrChandleExpr::LocalRead(name)),
            )));
        }
        self.early_value_actual(path, formals, bound, idx, arg_irs, name)
            .map(Some)
    }

    fn early_value_actual(
        &mut self,
        path: &str,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        idx: usize,
        arg_irs: &mut Vec<Option<IrExpr>>,
        name: String,
    ) -> Result<(IrStmt, EarlyActual), String> {
        if bound[idx].string {
            let value = self.lower_string(path, bound[idx].expr)?;
            return Ok((
                IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(value),
                },
                EarlyActual::String(IrStringExpr::LocalRead(name)),
            ));
        }
        let value = self.lower_bound_arg(path, formals, bound, idx, arg_irs)?;
        let (width, signed) = (value.width, value.signed);
        arg_irs[idx] = Some(IrExpr::new(
            IrExprKind::LocalRead(crate::sim::ir::call_argument_name(idx)),
            width,
            signed,
            None,
        ));
        Ok((
            IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            },
            EarlyActual::Packed(IrExpr::new(
                IrExprKind::LocalRead(name),
                width,
                signed,
                None,
            )),
        ))
    }

    /// Evaluate the copy-in value `value` of an inout actual into temporary
    /// `name` (decision EO-D3), returning its read.
    pub(in crate::sim::codegen) fn early_copy_in(
        value: IrExpr,
        name: String,
        before: &mut Vec<IrStmt>,
    ) -> IrExpr {
        let (width, signed) = (value.width, value.signed);
        before.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(value)),
        });
        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
    }
}

impl Codegen<'_> {
    /// Whether `node` is `root` or lies below it.
    pub(in crate::sim::codegen) fn below(&self, mut node: NodeId, root: NodeId) -> bool {
        loop {
            if node == root {
                return true;
            }
            match self.node(node).parent {
                Some(parent) => node = parent,
                None => return false,
            }
        }
    }

    /// The target of blocking assignment `h` when the computed handles it
    /// is selected through are evaluated late (decision EO-D4): after the
    /// nested assignments the statement runs first, and for an element
    /// target after the right-hand side ([`Self::source_before_target`]).
    pub(in crate::sim::codegen) fn late_located_target(&self, h: NodeId) -> Option<NodeId> {
        let NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            delay: None,
            ..
        }) = self.kind(h)
        else {
            return None;
        };
        let hoists = matches!(&self.pending_hoists, Some((statement, _)) if *statement == h);
        (hoists || self.source_before_target(h).is_some())
            .then(|| self.node(h).children.first().copied())
            .flatten()
    }

    /// The right-hand side of `h` when `h` is a simple blocking assignment
    /// whose target is an element, a bit or a part of something (a select)
    /// and whose right-hand side has an [effect](Self::evaluation_effect):
    /// such a target is located after the right-hand side (decision EO-D4),
    /// whatever handle it is selected through. A statement that stages
    /// container members around it keeps the handles of its target before
    /// it.
    pub(in crate::sim::codegen) fn source_before_target(&self, h: NodeId) -> Option<NodeId> {
        let NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            op: Operation::Assignment,
            delay: None,
        }) = self.kind(h)
        else {
            return None;
        };
        let [lhs, rhs] = self.node(h).children.as_slice() else {
            return None;
        };
        let selected = matches!(
            self.kind(*lhs),
            NodeKind::Expr(
                ExprKind::ArraySelect { .. }
                    | ExprKind::BitSelect { .. }
                    | ExprKind::PartSelect { .. }
                    | ExprKind::IndexedPartSelect { .. }
            )
        );
        (selected
            && self.contains_evaluation_effect(*rhs)
            && !(self.element_staging_possible && self.stages_members(h)))
        .then_some(*rhs)
    }

    /// The computed handle (`ca[k]`, `f()`) that the whole-property target
    /// of blocking assignment `h` is selected through (`ca[k].x`,
    /// `f().d.x`), when its right-hand side has an
    /// [effect](Self::evaluation_effect): such a target is located before
    /// the right-hand side (decision EO-D4), so the handle is evaluated
    /// first. A container or record property has a computed receiver of
    /// its own, which the statement evaluates first already.
    pub(in crate::sim::codegen) fn target_handle_first(&self, h: NodeId) -> Option<NodeId> {
        let NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            delay: None,
            ..
        }) = self.kind(h)
        else {
            return None;
        };
        let [lhs, rhs] = self.node(h).children.as_slice() else {
            return None;
        };
        if !self.contains_evaluation_effect(*rhs)
            || self.computed_receiver_aliases.contains_key(lhs)
        {
            return None;
        }
        let mut node = *lhs;
        loop {
            node = match self.kind(node) {
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    match refs.first().copied().flatten() {
                        Some(first)
                            if matches!(
                                self.kind(first),
                                NodeKind::Expr(_)
                                    | NodeKind::FuncCall { .. }
                                    | NodeKind::MethodCall { .. }
                            ) =>
                        {
                            first
                        }
                        _ => return None,
                    }
                }
                NodeKind::Expr(ExprKind::MemberSelect { base, .. }) => *base,
                _ => break,
            };
        }
        let computed = matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
                | NodeKind::FuncCall { .. }
                | NodeKind::MethodCall { .. }
        );
        let class = matches!(
            self.query_descriptor(node).map(|descriptor| &descriptor.shape),
            Some(TypeShape::Opaque { kind }) if kind == "Class"
        );
        (node != *lhs && computed && class).then_some(node)
    }

    /// Evaluate handle `node` ([`Self::target_handle_first`]) into a
    /// temporary that every lowering of `node` reads until
    /// [`Self::end_target_handle`].
    pub(in crate::sim::codegen) fn hoist_target_handle(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let name = format!("_llg_th{}", node.0);
        let handle = self.lower_chandle(path, node)?;
        self.hoisted_values
            .insert(node, HoistedValue::Handle(name.clone()));
        self.hoisted_assignments.insert(node, false);
        Ok(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
            name,
            Some(handle),
        ))))
    }

    /// Forget the handle [`Self::hoist_target_handle`] evaluated. One that
    /// no lowering read would have been evaluated twice.
    pub(in crate::sim::codegen) fn end_target_handle(
        &mut self,
        node: NodeId,
    ) -> Result<(), String> {
        self.hoisted_values.remove(&node);
        match self.hoisted_assignments.remove(&node) {
            Some(true) => Ok(()),
            _ => Err(self.locate_error(
                node,
                "a handle that the target of this assignment is selected through is not supported here; assign it to a variable first",
            )),
        }
    }
}
