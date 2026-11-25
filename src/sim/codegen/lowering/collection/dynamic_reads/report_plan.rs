//! Whether a `$monitor` argument that calls a function or method can be
//! watched exactly ([`ReportReadPlan::Exact`]).
//!
//! The exact plan reuses the wait planner's callee walk
//! (`callee_dependencies`): the storage the callee body reads through its
//! receiver, its class-handle formals, handle properties of those, static
//! handles and automatic handle locals that hold one such handle. That walk
//! lowers handles and interns native accesses, and a read it cannot bind is
//! a located error, so the plan is decided here first from the same static
//! facts, without lowering anything. A call this check does not accept is
//! watched at every settled slot; a call it accepts that the walk then
//! cannot bind is a located error, never a quiet change of plan.
//!
//! The check is at least as strict as the walk: a root actual or receiver
//! binds only when it names a static handle variable or a chain of handle
//! properties from one, a handle property read binds as a new handle every
//! time (as `property_handle` interns a new access), and a callee local binds
//! only when every write stores the same bound handle.

use super::*;

/// A handle a callee body reads through, as `callee_handle` would bind it.
#[derive(Clone, PartialEq, Eq)]
enum PlannedHandle {
    /// A static handle variable named by the report's own call: as its
    /// receiver (`h.get()`, null-checked) or as a handle actual (`f(h)`).
    /// A receiver never equals an actual naming the same variable, because
    /// only the receiver carries the null check.
    Root { declaration: NodeId, receiver: bool },
    /// A static handle variable named inside a callee body.
    Static(usize),
    /// A handle property; every read is a handle of its own.
    Property(usize),
}

/// The handles one call binds, as [`CalleeReceivers`] holds them.
#[derive(Clone, Default, PartialEq)]
struct PlannedReceivers {
    formals: HashMap<NodeId, Option<PlannedHandle>>,
    this: Option<PlannedHandle>,
}

impl Codegen<'_> {
    /// Whether `node` calls a virtual method: its implementation is chosen at
    /// run time, so no callee body names what it reads (user decision: such
    /// a monitor stays at every settled slot).
    pub(super) fn calls_virtual_method(&self, node: NodeId) -> bool {
        self.called_subroutine(node).is_some_and(|callee| {
            matches!(
                self.kind(callee),
                NodeKind::FuncTask {
                    is_virtual: true,
                    ..
                }
            )
        })
    }

    /// Whether the dependencies of report argument call `call` to `callee`
    /// can be bound exactly by `report_dependencies`.
    pub(super) fn report_call_binds(&self, call: NodeId, callee: NodeId) -> bool {
        if self.calls_virtual_method(call) {
            return false;
        }
        let mut receivers = PlannedReceivers::default();
        let mut serial = 0;
        if self.class_method_owner(callee).is_some() {
            // An omitted receiver (`get()` in a method) is the enclosing
            // method's `this`, which this check does not follow.
            receivers.this = match self.kind(call) {
                NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } => self.planned_root_handle(*receiver, true, &mut serial),
                _ => None,
            };
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self.planned_root_handle(actual, false, &mut serial);
            receivers.formals.insert(formal, handle);
        }
        self.planned_callee_binds(callee, receivers, &mut CalleeWalk::default(), &mut serial)
    }

    /// A handle actual or receiver of the report's own call that the wait
    /// walk evaluates again whenever the monitor arms: a static handle
    /// variable (a module or static subroutine variable, a static class
    /// property) or a chain of handle properties from one.
    fn planned_root_handle(
        &self,
        node: NodeId,
        receiver: bool,
        serial: &mut usize,
    ) -> Option<PlannedHandle> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self
                .planned_static_object(*target)
                .map(|_| PlannedHandle::Root {
                    declaration: *target,
                    receiver,
                }),
            // `h.n`: a chain of handle properties from a static handle.
            NodeKind::Expr(ExprKind::HierPath { refs, .. })
                if refs.len() > 1
                    && refs.iter().all(Option::is_some)
                    && refs[0].is_some_and(|root| self.planned_static_object(root).is_some())
                    && refs[1..].iter().flatten().all(|field| {
                        self.class_fields.contains_key(field)
                            && self.direct_field_type(*field) == Some(IrClassFieldType::Chandle)
                    }) =>
            {
                *serial += 1;
                Some(PlannedHandle::Property(*serial))
            }
            _ => None,
        }
    }

    /// The handle object a static handle declaration names.
    fn planned_static_object(&self, target: NodeId) -> Option<usize> {
        let object = self
            .class_static_objects
            .get(&target)
            .or_else(|| self.object_globals.get(&target))
            .map(|object| self.reference_object(*object))?;
        (self.model.objects.get(object)?.ty == crate::sim::ir::IrObjectType::Chandle)
            .then_some(object)
    }

    /// Mirror of `callee_dependencies`: whether every dynamic read of
    /// `callee`'s body, and of the bodies it calls, binds.
    fn planned_callee_binds(
        &self,
        callee: NodeId,
        receivers: PlannedReceivers,
        walk: &mut CalleeWalk<PlannedReceivers>,
        serial: &mut usize,
    ) -> bool {
        let Some(body) = self.func_body(callee) else {
            return true;
        };
        if walk.cuts(callee) {
            return !self.reads_dynamic_storage_in(body, &mut IdSet::new());
        }
        if walk.has_walked(callee, &receivers) {
            return true;
        }
        let entered = receivers.clone();
        let receivers = self.planned_bind_locals(body, receivers, serial);
        let cuts = walk.cuts;
        walk.stack.push(callee);
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            if self.reject_unobservable_read("", node).is_err() {
                return false;
            }
            if self.static_class_property(node).is_some() {
                continue;
            }
            if self.mailbox_num_call(node) {
                let bound = match self.kind(node) {
                    NodeKind::MethodCall {
                        receiver: Some(receiver),
                        ..
                    } => self.planned_handle(*receiver, &receivers, serial),
                    _ => None,
                };
                if bound.is_none() {
                    return false;
                }
                pending.extend(self.node(node).children.iter().copied());
                continue;
            }
            if self.dynamic_interface_member(node) {
                // Virtual-interface handles inside callee bodies are not
                // followed by this check.
                return false;
            }
            if self.class_container_property(node).is_some() {
                let bound = self.container_property_declaration(node).and_then(|field| {
                    self.planned_property_receiver(node, field, &receivers, serial)
                });
                if bound.is_none() {
                    return false;
                }
                continue;
            }
            if let Some(field) = self.class_storage_field(node) {
                let stored = self.class_native_fields.contains_key(&field)
                    || self.direct_field_type(field).is_some();
                if !stored
                    || self
                        .planned_property_receiver(node, field, &receivers, serial)
                        .is_none()
                {
                    return false;
                }
                continue;
            }
            if let Some(next) = self.called_subroutine(node) {
                if self.calls_virtual_method(node) {
                    return false;
                }
                let bound = self.planned_nested_receivers(node, next, &receivers, serial);
                if !self.planned_callee_binds(next, bound, walk, serial) {
                    return false;
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        walk.leave(callee, entered, cuts);
        true
    }

    /// Mirror of `nested_call_receivers`.
    fn planned_nested_receivers(
        &self,
        call: NodeId,
        callee: NodeId,
        outer: &PlannedReceivers,
        serial: &mut usize,
    ) -> PlannedReceivers {
        let mut receivers = PlannedReceivers::default();
        let is_static = matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_static: true,
                ..
            }
        );
        if self.class_method_owner(callee).is_some() && !is_static {
            receivers.this = match self.kind(call) {
                NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } => self.planned_handle(*receiver, outer, serial),
                _ => outer.this.clone(),
            };
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self.planned_handle(actual, outer, serial);
            receivers.formals.insert(formal, handle);
        }
        receivers
    }

    /// Mirror of `bind_callee_locals`.
    fn planned_bind_locals(
        &self,
        body: NodeId,
        mut receivers: PlannedReceivers,
        serial: &mut usize,
    ) -> PlannedReceivers {
        let mut locals = Vec::new();
        let mut parents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut assigned: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            if let NodeKind::Stmt(StmtKind::VariableDecl { declaration }) = self.kind(node) {
                if matches!(self.kind(*declaration), NodeKind::Var { ty } if ty.kind == "class")
                    && self.db.variable_lifetime(*declaration)
                        == crate::core::db::VariableLifetime::Automatic
                {
                    locals.push((
                        self.node(*declaration).line,
                        self.node(*declaration).col,
                        *declaration,
                    ));
                    if let Some(initializer) = self.db.var_initializer(*declaration) {
                        assigned.entry(*declaration).or_default().push(initializer);
                    }
                }
            }
            if let NodeKind::Stmt(StmtKind::Assign { .. }) = self.kind(node) {
                if let [lhs, rhs, ..] = self.node(node).children.as_slice() {
                    if let NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) = self.kind(*lhs)
                    {
                        assigned.entry(*target).or_default().push(*rhs);
                    }
                }
            }
            for child in &self.node(node).children {
                parents.entry(*child).or_default().push(node);
                pending.push(*child);
            }
        }
        locals.sort_by_key(|(line, col, _)| (*line, *col));
        for (_, _, local) in locals {
            if !self.local_written_only_by_assignment(local, body, &parents) {
                continue;
            }
            let mut bound: Option<PlannedHandle> = None;
            let mut consistent = true;
            for value in assigned.get(&local).into_iter().flatten().copied() {
                match self.planned_handle(value, &receivers, serial) {
                    Some(handle) if bound.as_ref().is_none_or(|bound| *bound == handle) => {
                        bound = Some(handle);
                    }
                    _ => consistent = false,
                }
            }
            if consistent {
                if let Some(handle) = bound {
                    receivers.formals.insert(local, Some(handle));
                }
            }
        }
        receivers
    }

    /// Mirror of `property_handle`: a new handle for every read.
    fn planned_property_handle(
        &self,
        receiver: Option<PlannedHandle>,
        field: NodeId,
        serial: &mut usize,
    ) -> Option<PlannedHandle> {
        receiver?;
        if self.direct_field_type(field) != Some(IrClassFieldType::Chandle)
            || !self.class_fields.contains_key(&field)
        {
            return None;
        }
        *serial += 1;
        Some(PlannedHandle::Property(*serial))
    }

    /// Mirror of `callee_root_handle`.
    fn planned_callee_root(
        &self,
        target: NodeId,
        receivers: &PlannedReceivers,
        serial: &mut usize,
    ) -> Option<PlannedHandle> {
        if let Some(bound) = receivers.formals.get(&target) {
            return bound.clone();
        }
        if self.class_fields.contains_key(&target) {
            return self.planned_property_handle(receivers.this.clone(), target, serial);
        }
        self.planned_static_object(target)
            .map(PlannedHandle::Static)
    }

    /// Mirror of `callee_handle`.
    fn planned_handle(
        &self,
        node: NodeId,
        receivers: &PlannedReceivers,
        serial: &mut usize,
    ) -> Option<PlannedHandle> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.planned_callee_root(*target, receivers, serial),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                if refs.len() == 1
                    && refs[0].is_none()
                    && parts.first().is_some_and(|part| part == "this")
                {
                    return receivers.this.clone();
                }
                let field = refs.iter().rev().flatten().copied().next()?;
                if refs.len() == 1 {
                    return self.planned_callee_root(field, receivers, serial);
                }
                let receiver = self.planned_property_receiver(node, field, receivers, serial);
                self.planned_property_handle(receiver, field, serial)
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "class" => {
                self.planned_handle(*operand, receivers, serial)
            }
            _ => None,
        }
    }

    /// Mirror of `callee_property_receiver`.
    fn planned_property_receiver(
        &self,
        node: NodeId,
        field: NodeId,
        receivers: &PlannedReceivers,
        serial: &mut usize,
    ) -> Option<PlannedHandle> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return receivers.this.clone();
        };
        let end = refs
            .iter()
            .rposition(|reference| *reference == Some(field))
            .unwrap_or(refs.len());
        if end == 0 {
            return receivers.this.clone();
        }
        if refs[end - 1].is_some_and(|previous| {
            self.direct_field_type(previous) == Some(IrClassFieldType::Chandle)
        }) && end > 1
        {
            let mut current = node;
            while let [child] = self.node(current).children.as_slice() {
                let child = *child;
                match self.kind(child) {
                    NodeKind::Expr(ExprKind::HierPath { refs: prefix, .. })
                        if prefix.len() > end =>
                    {
                        current = child;
                    }
                    _ => return self.planned_handle(child, receivers, serial),
                }
            }
            return None;
        }
        if end == 1 {
            return match refs[0] {
                Some(root) => self.planned_callee_root(root, receivers, serial),
                None if parts.first().is_some_and(|part| part == "this") => receivers.this.clone(),
                None => None,
            };
        }
        None
    }
}
