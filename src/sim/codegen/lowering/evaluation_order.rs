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
//! place. Where nothing else of the expression reads storage or calls a
//! subroutine, the assignments run in place, which is the same order. A
//! packed or real target is written by one mutation expression; a string or
//! handle target, a target that expression does not reach (a container
//! element, a class property select) and an element target of a value with
//! an effect are written by the assignment statement form from a temporary
//! of the value, which the expression reads; an unpacked array, record or
//! container target is written by the statement form and read again
//! ([`NestedAssignment`]). These run first wherever the expression runs its
//! nested assignments first and are rejected in an operand that may be
//! skipped or repeated.
//!
//! A set with a member whose effect runs at its place is scanned member by
//! member (EO-D2, `Codegen::lower_inside_in_order`). An input actual with an effect
//! before an inout or `ref` actual is evaluated before that actual is copied
//! in or bound (EO-D3, [`Codegen::early_actuals_until`]). A computed handle a
//! class property is selected through is evaluated where its use starts, an
//! element target after the right-hand side and a whole-property one before
//! it (EO-D4, [`Codegen::source_before_target`],
//! [`Codegen::target_handle_first`]); the uses after an effect are found by
//! the walk the views of interface members share
//! (`Codegen::uses_after_effects_with`). The rules and the forms they leave
//! out are recorded in `docs/lrm_decisions.md`.

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
    pub(in crate::sim::codegen) fn statement_hoisting_operands(&self, h: NodeId) -> &[NodeId] {
        let children = &self.node(h).children;
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { .. }) => &children[..children.len().min(2)],
            NodeKind::Stmt(StmtKind::Return { value }) => value.as_slice(),
            NodeKind::Stmt(StmtKind::IfElse { predicate, .. }) => {
                match predicate.clauses.as_slice() {
                    [clause] if clause.pattern.is_none() => {
                        std::slice::from_ref(&clause.expression)
                    }
                    _ => &[],
                }
            }
            NodeKind::Stmt(StmtKind::Case { .. }) => children.get(..1).unwrap_or_default(),
            NodeKind::Stmt(StmtKind::Repeat { cond, .. }) => std::slice::from_ref(cond),
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => children,
            NodeKind::SysCall { name } if !super::statements::deferred_display_task(name) => {
                children
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands,
            _ => &[],
        }
    }

    /// The nested assignment operators that the evaluation of `operands`
    /// runs first (decision EO-D1), in the order they run.
    pub(in crate::sim::codegen) fn hoisted_assignment_nodes(
        &self,
        operands: &[NodeId],
    ) -> Vec<NodeId> {
        // Most statements hold none: find one before ordering them.
        let mut pending = operands.to_vec();
        loop {
            let Some(node) = pending.pop() else {
                return Vec::new();
            };
            if self.nested_assignment(node).is_some() {
                break;
            }
            pending.extend_from_slice(self.hoisting_operands(node));
        }
        let mut out = Vec::new();
        // Post-order, left to right: an assignment after those its own
        // operands hold. `true` marks a node whose operands are done.
        let mut pending: Vec<(NodeId, bool)> =
            operands.iter().rev().map(|node| (*node, false)).collect();
        while let Some((node, done)) = pending.pop() {
            if done {
                if self.nested_assignment(node).is_some() {
                    out.push(node);
                }
                continue;
            }
            pending.push((node, true));
            let operands = self.hoisting_operands(node);
            pending.extend(operands.iter().rev().map(|operand| (*operand, false)));
        }
        out
    }

    /// The operands of `node` that the walk of
    /// [`Self::hoisted_assignment_nodes`] enters: none of a nested statement
    /// or of an operator that may skip or repeat its operands, the receiver
    /// alone of an array method with a `with` clause, and the target of a
    /// copied-out actual.
    fn hoisting_operands(&self, node: NodeId) -> &[NodeId] {
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
            }) => &[],
            NodeKind::Expr(ExprKind::Operation { operands, .. }) if self.copy_out_wrapper(node) => {
                operands.get(..1).unwrap_or_default()
            }
            NodeKind::MethodCall { receiver, .. } if self.db.method_call_has_with_clause(node) => {
                receiver.as_slice()
            }
            _ => &self.node(node).children,
        }
    }

    /// Whether `node` is the assignment from the formal that the frontend
    /// wraps an output or inout actual in (its value is an empty argument).
    fn copy_out_wrapper(&self, node: NodeId) -> bool {
        // The value is the formal's, converted to the actual's type when
        // the two differ.
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if operands.get(1).is_some_and(|value| {
                self.db.semantic_detail(self.p30_unwrap_cast(*value)) == Some("EmptyArgument")
            })
        )
    }

    /// `hoists` (of `operands`) if running them first can be observed:
    /// something else the operands read or call lies outside one of them.
    /// `target` is a variable the statement only writes, as is the variable
    /// an assignment of `hoists` writes. Otherwise they run in place, which
    /// is the same order.
    pub(in crate::sim::codegen) fn observable_hoists(
        &self,
        operands: &[NodeId],
        target: Option<NodeId>,
        hoists: Vec<NodeId>,
    ) -> Vec<NodeId> {
        // Only a mutation has an in-place form.
        if hoists.is_empty()
            || hoists
                .iter()
                .any(|hoist| self.nested_assignment(*hoist) != Some(NestedAssignment::Mutation))
        {
            return hoists;
        }
        let mut pending = operands.to_vec();
        while let Some(node) = pending.pop() {
            if Some(node) == target {
                continue;
            }
            let children = &self.node(node).children;
            match self.kind(node) {
                NodeKind::Stmt(_)
                | NodeKind::Expr(
                    ExprKind::Constant { .. } | ExprKind::DataType | ExprKind::Unbounded,
                ) => {}
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(declaration),
                }) if matches!(self.kind(*declaration), NodeKind::Param { .. }) => {}
                NodeKind::Expr(ExprKind::Operation { operands, .. }) if hoists.contains(&node) => {
                    let written = operands.first().filter(|first| {
                        matches!(self.kind(**first), NodeKind::Expr(ExprKind::Ref { .. }))
                    });
                    pending.extend(operands.iter().filter(|operand| Some(*operand) != written));
                }
                NodeKind::Expr(
                    ExprKind::Ref { .. } | ExprKind::HierPath { .. } | ExprKind::ScopeRef { .. },
                )
                | NodeKind::FuncCall { .. }
                | NodeKind::MethodCall { .. }
                | NodeKind::SysCall { .. } => {
                    if hoists
                        .iter()
                        .any(|hoist| !self.nesting().contains(*hoist, node))
                    {
                        return hoists;
                    }
                    pending.extend(children.iter().copied());
                }
                _ => pending.extend(children.iter().copied()),
            }
        }
        Vec::new()
    }

    /// How nested assignment operator `node` runs when its full expression
    /// runs it first (decision EO-D1), or `None` for a node that is not one
    /// (an increment, a decrement, an overloaded update, the copy-out of an
    /// output actual) or whose target has no such form (an event).
    pub(in crate::sim::codegen) fn nested_assignment(
        &self,
        node: NodeId,
    ) -> Option<NestedAssignment> {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            assignment,
            operands,
            ..
        }) = self.kind(node)
        else {
            return None;
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
            return None;
        }
        let target = *operands.first()?;
        let simple = *op == Operation::Assignment;
        let shape = self
            .query_descriptor(target)
            .map(|descriptor| &descriptor.shape);
        let packed = match shape {
            None | Some(TypeShape::PackedAtom { .. } | TypeShape::Real { .. }) => true,
            Some(TypeShape::Aggregate(layout)) => matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::PackedUnion
            ),
            Some(_) => false,
        };
        if packed {
            let value = simple.then(|| operands.get(1)).flatten();
            return Some(self.packed_nested_assignment(target, value));
        }
        match shape? {
            TypeShape::String => simple.then_some(NestedAssignment::Store(StoredValue::String)),
            TypeShape::Opaque { kind } if kind == "Class" || kind == "Chandle" => {
                simple.then_some(NestedAssignment::Store(StoredValue::Handle))
            }
            TypeShape::Aggregate(_)
            | TypeShape::FixedArray { .. }
            | TypeShape::Container { .. } => simple.then_some(NestedAssignment::Whole),
            _ => None,
        }
    }

    /// The form of a nested assignment to packed or real `target`, of
    /// `value` for a simple one: one mutation expression where that locates
    /// the target as the statement form would, otherwise the statement
    /// form, which stores a value evaluated first. That is an element, bit
    /// or part target of a value with an [effect](Self::evaluation_effect),
    /// located after it as a statement's is (decisions EO-D1, EO-D4), and
    /// a target the mutation does not reach: an element or member of a
    /// queue, dynamic or associative array, or a select of a class
    /// property.
    fn packed_nested_assignment(&self, target: NodeId, value: Option<&NodeId>) -> NestedAssignment {
        let selected = matches!(
            self.kind(target),
            NodeKind::Expr(
                ExprKind::ArraySelect { .. }
                    | ExprKind::BitSelect { .. }
                    | ExprKind::PartSelect { .. }
                    | ExprKind::IndexedPartSelect { .. }
            )
        );
        if (selected && value.is_some_and(|value| self.contains_evaluation_effect(*value)))
            || !self.mutation_reaches(target)
        {
            NestedAssignment::Store(StoredValue::Packed)
        } else {
            NestedAssignment::Mutation
        }
    }

    /// Whether the mutation expression locates packed or real `target`:
    /// not an element or member of a queue, dynamic or associative array,
    /// nor a select of a class property.
    fn mutation_reaches(&self, target: NodeId) -> bool {
        let mut node = target;
        loop {
            node = match self.kind(node) {
                NodeKind::Expr(
                    ExprKind::ArraySelect { base, .. }
                    | ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. }
                    | ExprKind::MemberSelect { base, .. },
                ) => *base,
                _ => return true,
            };
            let container = matches!(
                self.query_descriptor(node)
                    .map(|descriptor| &descriptor.shape),
                Some(TypeShape::Container { .. })
            );
            if container
                || self.container_of(node).is_some()
                || self.class_field_target(node).is_some()
            {
                return false;
            }
        }
    }

    /// The error for nested assignment `node` lowered where it is evaluated
    /// (an operand that may be skipped or repeated), when only the
    /// statement form it runs first in has its target: `None` for one the
    /// mutation expression lowers there.
    pub(in crate::sim::codegen) fn nested_assignment_in_place_error(
        &self,
        node: NodeId,
    ) -> Option<String> {
        let target = match self.nested_assignment(node)? {
            NestedAssignment::Mutation => return None,
            NestedAssignment::Store(StoredValue::Packed) => {
                let NodeKind::Expr(ExprKind::Operation { operands, .. }) = self.kind(node) else {
                    return None;
                };
                *operands.first()?
            }
            NestedAssignment::Store(_) | NestedAssignment::Whole => node,
        };
        if target != node && self.mutation_reaches(target) {
            return None;
        }
        Some(self.locate_error(
            node,
            "an assignment operator to a string, a handle, an unpacked value, a container element or a class property select is supported in an operand that its full expression evaluates once (decision EO-D1), not in one of `?:`, `&&`, `||`, `->`, `inside` or an array method's `with` clause; assign the value in a statement of its own first".to_owned(),
        ))
    }

    /// Run the nested assignments `nodes` (from
    /// [`Self::hoisted_assignment_nodes`]) into temporaries that later reads
    /// of the nodes return, or for an unpacked target into the target that
    /// later reads name ([`Self::assigned_target`]). The caller ends them
    /// with [`Self::end_hoisted_assignments`] once the operands are lowered.
    pub(in crate::sim::codegen) fn hoist_assignments(
        &mut self,
        path: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = Vec::with_capacity(nodes.len());
        for node in nodes {
            let form = self.nested_assignment(*node).ok_or_else(|| {
                self.locate_error(*node, "nested assignment without a hoisted form".to_owned())
            })?;
            let name = format!("_llg_na{}", node.0);
            match form {
                NestedAssignment::Mutation => {
                    let value = self.lower_expr(path, *node)?;
                    let (width, signed) = (value.width, value.signed);
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
                NestedAssignment::Store(kind) => {
                    let hoisted =
                        self.store_nested_assignment(path, *node, kind, name, &mut statements)?;
                    self.hoisted_values.insert(*node, hoisted);
                    self.hoisted_assignments.insert(*node, false);
                }
                NestedAssignment::Whole => {
                    statements.push(self.run_whole_nested_assignment(path, *node)?);
                    self.hoisted_values.insert(*node, HoistedValue::Target);
                    // Reads name the target; none is an evaluation of its own.
                    self.hoisted_assignments.insert(*node, true);
                }
            }
        }
        Ok(statements)
    }

    /// Nested assignment `node` in its statement form: its value into
    /// temporary `name` of the target's kind, then the target, located
    /// after the value as a statement's element target is, assigned that
    /// temporary. The value of a compound assignment reads the target, so
    /// its selectors must have no effect. Returns what reads of `node` read.
    fn store_nested_assignment(
        &mut self,
        path: &str,
        node: NodeId,
        kind: StoredValue,
        name: String,
        statements: &mut Vec<IrStmt>,
    ) -> Result<HoistedValue, String> {
        let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = self.kind(node) else {
            unreachable!("nested assignment is an operation");
        };
        let (op, operands) = (*op, operands.clone());
        let [target, value_node] = operands[..] else {
            return Err(
                self.locate_error(node, "nested assignment without two operands".to_owned())
            );
        };
        let hoisted = match kind {
            StoredValue::String => {
                statements.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(self.lower_string(path, value_node)?),
                });
                HoistedValue::String(name)
            }
            StoredValue::Handle => {
                if !self.is_chandle_expr(path, target) {
                    return Err(self.locate_error(
                        node,
                        format!("an assignment operator to this handle in an operand of an expression in `{path}` is not supported; assign the value in a statement of its own first"),
                    ));
                }
                let value = self.lower_chandle(path, value_node)?;
                statements.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))));
                HoistedValue::Handle(name)
            }
            StoredValue::Packed => {
                let simple = op == Operation::Assignment;
                if !simple && self.has_evaluation_effect(target) {
                    return Err(self.locate_error(
                        node,
                        format!("a compound assignment operator in an operand of an expression whose target in `{path}` is selected with a call or an assignment is not supported; assign the value in a statement of its own first"),
                    ));
                }
                let descriptor = self.query_descriptor(target).map(|descriptor| {
                    (
                        descriptor.two_state,
                        descriptor.info.width,
                        descriptor.info.signed,
                    )
                });
                let two_state = descriptor.is_some_and(|(two_state, _, _)| two_state);
                let value = if simple {
                    // The frontend converts the value to the target's type;
                    // the target is located after it, so it is not lowered
                    // here.
                    let value = self.lower_expr(path, value_node)?;
                    match descriptor
                        .filter(|_| !value.is_real())
                        .and_then(|(_, width, signed)| Some((width?, signed)))
                    {
                        Some((width, signed)) if (width, signed) != (value.width, value.signed) => {
                            IrExpr::convert_to(value, width, signed)
                        }
                        _ => value,
                    }
                } else {
                    // A compound target is located before its value, and its
                    // current value is read there.
                    if let Some(declarations) = self.take_receiver_root(target) {
                        statements.extend(declarations);
                    }
                    let current = self.lower_expr(path, target)?;
                    let value = self.lower_expr(path, value_node)?;
                    let value = lower_compound_expr_ir(path, op, current.clone(), value)?;
                    if current.is_real() || value.is_real() {
                        value
                    } else {
                        IrExpr::convert_to(value, current.width, current.signed)
                    }
                };
                let value = if two_state && !value.is_real() {
                    IrExpr::to_two_state(value)
                } else {
                    value
                };
                let (width, signed) = (value.width, value.signed);
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                });
                HoistedValue::Packed(name, width, signed)
            }
        };
        // The store reads the temporary where it would read the value.
        let outer = self.hoisted_values.insert(value_node, hoisted.clone());
        let store = self.lower_nested_store(path, target, value_node);
        match outer {
            Some(outer) => self.hoisted_values.insert(value_node, outer),
            None => self.hoisted_values.remove(&value_node),
        };
        statements.push(store?);
        Ok(hoisted)
    }

    /// Nested assignment `node` to an unpacked array, record or container,
    /// as a statement; reads of `node` read the target again, which must
    /// therefore be selected without an effect.
    fn run_whole_nested_assignment(&mut self, path: &str, node: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Expr(ExprKind::Operation { operands, .. }) = self.kind(node) else {
            unreachable!("nested assignment is an operation");
        };
        let [target, value_node] = operands[..] else {
            return Err(
                self.locate_error(node, "nested assignment without two operands".to_owned())
            );
        };
        if self.has_evaluation_effect(target) {
            return Err(self.locate_error(
                node,
                format!("an assignment operator to an unpacked value selected with a call or an assignment in an operand of an expression in `{path}` is not supported; assign the value in a statement of its own first"),
            ));
        }
        self.lower_nested_store(path, target, value_node)
    }

    /// The blocking assignment statement of `target` from `value`, after
    /// the computed handles of the target that the statement evaluates at
    /// the target (decision EO-D4).
    fn lower_nested_store(
        &mut self,
        path: &str,
        target: NodeId,
        value: NodeId,
    ) -> Result<IrStmt, String> {
        let mut statements = self.take_receiver_root(target).unwrap_or_default();
        let function = self.func.clone();
        let depth = self.depth_arg.clone();
        let instance = self.inst;
        statements.push(
            EmitCtx::new(self, path.to_owned(), instance, &depth, function, false)
                .lower_assignment_operands(target, value, true, Operation::Assignment, false)?,
        );
        Ok(match statements.len() {
            1 => statements.remove(0),
            _ => IrStmt::Block(statements),
        })
    }

    /// The target whose value stands for assignment `node`: the target of
    /// a copied-out actual, or of a nested assignment to an unpacked value
    /// that already ran ([`Self::hoist_assignments`]).
    pub(in crate::sim::codegen) fn assigned_target(&self, node: NodeId) -> Option<NodeId> {
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Assignment,
            operands,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        (matches!(self.hoisted_value(node), Some(HoistedValue::Target))
            || self.copy_out_wrapper(node))
        .then(|| operands.first().copied())
        .flatten()
    }

    /// The target of simple assignment `node` nested in an expression,
    /// whose type its value has.
    pub(in crate::sim::codegen) fn nested_assignment_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !self.copy_out_wrapper(node) => operands.first().copied(),
            _ => None,
        }
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
        let nodes = self.observable_hoists(&[node], None, self.hoisted_assignment_nodes(&[node]));
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

/// How a nested assignment operator runs first (decision EO-D1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim::codegen) enum NestedAssignment {
    /// One mutation expression locates, writes and yields a packed or real
    /// target, into a temporary or in place.
    Mutation,
    /// The value goes to a temporary that the assignment statement form
    /// stores and later reads read.
    Store(StoredValue),
    /// An unpacked array, record or container target is assigned by the
    /// statement form, and later reads read the target.
    Whole,
}

/// The kind of temporary a [`NestedAssignment::Store`] evaluates its value
/// into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim::codegen) enum StoredValue {
    Packed,
    String,
    Handle,
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
        let direction = |io: NodeId| match self.kind(io) {
            NodeKind::FuncArg { direction, .. } => Some(*direction),
            _ => None,
        };
        // Only the inputs before the last inout or `ref` actual matter, or
        // before an output actual selected through a computed handle that
        // the call evaluates at its place (decision EO-D4).
        let bindings =
            formals
                .iter()
                .enumerate()
                .rposition(|(idx, (io, _))| match direction(*io) {
                    Some(DbDirection::Inout | DbDirection::Ref) => true,
                    Some(DbDirection::Output) => bound.get(idx).is_some_and(|arg| {
                        self.receiver_roots
                            .contains_key(&self.unwrap_output_actual(arg.expr))
                    }),
                    _ => false,
                })?;
        formals[..bindings]
            .iter()
            .enumerate()
            .any(|(idx, (io, _))| {
                direction(*io) == Some(DbDirection::Input)
                    && bound.get(idx).is_some_and(|arg| {
                        !arg.is_default && self.contains_evaluation_effect(arg.expr)
                    })
            })
            .then_some(bindings)
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
    /// whatever handle it is selected through, also in a statement that
    /// stages container members around it.
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
        (selected && self.contains_evaluation_effect(*rhs)).then_some(*rhs)
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
        if self.computed_receiver_aliases.contains_key(lhs) {
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
        (node != *lhs && computed && class && self.contains_evaluation_effect(*rhs)).then_some(node)
    }

    /// The selectors of the record element of a queue, dynamic or
    /// associative array whose packed or real member the blocking
    /// assignment `h` writes whole (`rq[k].v = f()`, `rq[k].v += f()`), when
    /// its right-hand side has an [effect](Self::evaluation_effect): such a
    /// target is located before the right-hand side (decision EO-D4), so
    /// they are evaluated first. A selector with `$` is evaluated with the
    /// target.
    pub(in crate::sim::codegen) fn member_target_selectors_first(
        &self,
        h: NodeId,
    ) -> Option<Vec<NodeId>> {
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
        if !matches!(
            self.kind(*lhs),
            NodeKind::Expr(ExprKind::MemberSelect { .. } | ExprKind::HierPath { .. })
        ) {
            return None;
        }
        let selectors = self.member_target_selectors(*lhs)?;
        if !self.contains_evaluation_effect(*rhs) {
            return None;
        }
        let unbounded = |root: NodeId| {
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                if matches!(self.kind(node), NodeKind::Expr(ExprKind::Unbounded)) {
                    return true;
                }
                pending.extend(self.node(node).children.iter().copied());
            }
            false
        };
        (!selectors.iter().any(|selector| unbounded(*selector))).then_some(selectors)
    }

    /// Evaluate the selectors `nodes` ([`Self::member_target_selectors_first`])
    /// into temporaries that every lowering of them reads until
    /// [`Self::end_target_selectors`].
    pub(in crate::sim::codegen) fn hoist_target_selectors(
        &mut self,
        path: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = Vec::with_capacity(nodes.len());
        for node in nodes {
            let value = self.lower_expr(path, *node)?;
            let (width, signed) = (value.width, value.signed);
            let name = format!("_llg_ts{}", node.0);
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

    /// Forget the selectors [`Self::hoist_target_selectors`] evaluated. One
    /// that no lowering read would have been evaluated twice.
    pub(in crate::sim::codegen) fn end_target_selectors(
        &mut self,
        nodes: &[NodeId],
    ) -> Result<(), String> {
        let mut unread = None;
        for node in nodes {
            self.hoisted_values.remove(node);
            if self.hoisted_assignments.remove(node) != Some(true) && unread.is_none() {
                unread = Some(*node);
            }
        }
        match unread {
            Some(node) => Err(self.locate_error(
                node,
                "a selector of the record element this assignment writes is not supported here; assign the element's index to a variable first",
            )),
            None => Ok(()),
        }
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
