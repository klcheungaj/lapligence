//! Storage that a wait reaches through a handle: class properties selected
//! through a class handle (`h.x`, `n.next.v`, `this.x`, `h.s`) and
//! virtual-interface members (`v.sig`). Their storage changes when the
//! handle is rebound, so a wait names them by an
//! [`IrDependency::NativeAccess`] that the waiting process resolves again
//! every time it arms.
//!
//! Only `wait` conditions and event expressions observe such storage (SV
//! 9.4.2: `@(p.status)` follows the object `p` names, and "changing the
//! value of object data members ... referenced by a method or function
//! shall cause the event expression to be reevaluated"). The bodies of the
//! functions and nonvirtual methods such an expression calls are walked
//! with their class-handle formals bound to the caller's actuals, so a read
//! of `c.x` in `function int f(C c)` called as `f(h)` waits on `h.x`. A read
//! whose changes cannot be observed this way is rejected with its location:
//! a missed dependency would leave the wait blocked forever.
//!
//! `always_comb` adds nothing for class objects (SV 9.2.2.2.1), and
//! virtual-interface members cannot be used in sensitivity lists (SV 25.9),
//! so implicit sensitivity keeps only the handle variables it reads.

use super::*;
use crate::sim::ir::IrNativeAccessKind;

/// Longest handle-property chain (`a.b.c.d.v`) whose links a wait follows;
/// deeper links reject at the emitter's own access-depth guard first.
const MAX_HANDLE_PROPERTY_DEPTH: usize = 256;

/// Deepest nest of calls whose bodies a wait's dependency walk follows.
const MAX_CALLEE_DEPTH: usize = 64;

/// Class handles a callee body can read through, bound for one call: the
/// handle formals to the caller's actuals and `this` to the receiver. An
/// unbound handle (`None`) cannot be evaluated again when the wait arms.
#[derive(Clone, Default)]
struct CalleeReceivers {
    formals: HashMap<NodeId, Option<IrChandleExpr>>,
    this: Option<IrChandleExpr>,
}

impl Codegen<'_> {
    /// The instance property `node` selects, when it is packed, real, string
    /// or handle storage held directly in the object.
    fn dynamic_class_property(&self, node: NodeId) -> Option<NodeId> {
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        ) {
            return None;
        }
        let field = self.class_field_target(node)?;
        let (class, index) = self.class_fields.get(&field).copied()?;
        let layout = self.model.classes.get(class)?.fields.get(index)?;
        (layout.container.is_none() && layout.native_value.is_none()).then_some(field)
    }

    /// The layout type of instance property `field`, when it is held
    /// directly in the object.
    fn direct_field_type(&self, field: NodeId) -> Option<IrClassFieldType> {
        let (class, index) = self.class_fields.get(&field).copied()?;
        let layout = self.model.classes.get(class)?.fields.get(index)?;
        (layout.container.is_none() && layout.native_value.is_none()).then_some(layout.ty)
    }

    /// Whether `node` names a member through a virtual-interface handle.
    fn dynamic_interface_member(&self, node: NodeId) -> bool {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return false;
        };
        refs.iter().enumerate().any(|(position, target)| {
            target.is_some_and(|target| self.virtual_interface_spelling(target).is_some())
                && parts.iter().skip(position + 1).any(|part| !part.is_empty())
        })
    }

    /// The nonvirtual class function a method call `node` invokes, with its
    /// receiver expression (SV 9.4.2 admits only those in event controls).
    fn class_method_call(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            callee: Some(method),
            ..
        } = self.kind(node)
        else {
            return None;
        };
        (matches!(
            self.kind(*method),
            NodeKind::FuncTask {
                is_task: false,
                is_virtual: false,
                ..
            }
        ) && self.class_method_owner(*method).is_some())
        .then_some((*method, *receiver))
    }

    /// Whether evaluating `root` reads storage selected through a class or
    /// virtual-interface handle, a static class property or a built-in
    /// synchronization object, directly or in the body of a function or
    /// method it calls: the waiting process must evaluate such an expression
    /// and re-arm on the storage the handles name after every wake.
    pub(in super::super) fn reads_dynamic_storage(&self, root: NodeId) -> bool {
        self.reads_dynamic_storage_in(root, &mut HashSet::new())
    }

    fn reads_dynamic_storage_in(&self, root: NodeId, visited: &mut HashSet<NodeId>) -> bool {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if self.class_storage_field(node).is_some()
                || self.static_class_property(node).is_some()
                || self.dynamic_interface_member(node)
                || self.class_method_call(node).is_some()
                || self.synchronization_method_call(node)
            {
                return true;
            }
            if let Some(callee) = self.called_subroutine(node) {
                if visited.insert(callee)
                    && self
                        .func_body(callee)
                        .is_some_and(|body| self.reads_dynamic_storage_in(body, visited))
                {
                    return true;
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// The last class property `node` names, including container and native
    /// record properties, when `node` reads object storage.
    fn class_storage_field(&self, node: NodeId) -> Option<NodeId> {
        let is_field = |target: &NodeId| {
            self.class_fields.contains_key(target) || self.class_native_fields.contains_key(target)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => is_field(target).then_some(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .find(|target| is_field(target)),
            _ => None,
        }
    }

    /// The signal of the static class property `node` names.
    fn static_class_property(&self, node: NodeId) -> Option<&SignalInfo> {
        self.class_static_signals
            .get(&self.class_field_target(node)?)
    }

    /// Whether `node` calls a method of a mailbox or semaphore: their state
    /// (`num()`, `try_get()`) publishes no change marker.
    fn synchronization_method_call(&self, node: NodeId) -> bool {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        else {
            return false;
        };
        self.query_descriptor(*receiver).is_some_and(|descriptor| {
            descriptor.name == "semaphore" || descriptor.name.starts_with("mailbox")
        })
    }

    /// The user function or method `node` calls, when its body is walked
    /// for wait dependencies.
    fn called_subroutine(&self, node: NodeId) -> Option<NodeId> {
        let callee = match self.kind(node) {
            NodeKind::FuncCall {
                callee: Some(callee),
                is_task: false,
                ..
            } if !self.is_mailbox_constructor_call(node)
                && !self.is_semaphore_constructor_call(node)
                && !self.is_process_self_call(node) =>
            {
                *callee
            }
            NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => *callee,
            _ => return None,
        };
        matches!(self.kind(callee), NodeKind::FuncTask { is_task: false, .. }).then_some(callee)
    }

    /// A located rejection of a read whose changes no wait can observe.
    fn unobservable_read(&self, path: &str, node: NodeId, what: &str) -> String {
        let site = self.node(node);
        format!(
            "{what} at {}:{}:{} cannot be observed by a wait or event expression in `{}`: llg publishes no change for it, so the wait could never resume (IEEE 1800-2009 9.4.2)",
            site.file.as_deref().unwrap_or("<unknown>"),
            site.line,
            site.col,
            self.source_path(path)
        )
    }

    /// Reject reads in a wait or event expression that publish no change.
    fn reject_unobservable_read(&self, path: &str, node: NodeId) -> Result<(), String> {
        if self.synchronization_method_call(node) {
            return Err(self.unobservable_read(path, node, "a mailbox or semaphore method call"));
        }
        // Per-object containers have no change marker a wait can name.
        if self.container_of(node).is_some_and(|container| {
            self.model
                .containers
                .get(container.ir)
                .is_some_and(|container| container.class_field.is_some())
        }) {
            return Err(self.unobservable_read(
                path,
                node,
                &format!("container class property `{}`", self.node(node).name),
            ));
        }
        Ok(())
    }

    /// Whether a handle expression can be evaluated again whenever the wait
    /// arms without side effects.
    fn rearmable_handle(handle: &IrChandleExpr) -> bool {
        match handle {
            IrChandleExpr::Null
            | IrChandleExpr::Read(_)
            | IrChandleExpr::LocalRead(_)
            | IrChandleExpr::FormalRead(_)
            | IrChandleExpr::InterfaceInstance { .. } => true,
            IrChandleExpr::Required { handle, .. } => Self::rearmable_handle(handle),
            _ => false,
        }
    }

    /// Whether `formal` passes a class handle.
    fn is_class_handle_formal(&self, formal: NodeId) -> bool {
        matches!(self.kind(formal), NodeKind::FuncArg { ty, .. }
            if ty.kind == "class" && ty.type_name.as_deref() != Some("process"))
    }

    /// The formals of `callee` paired with the actuals of `call`.
    fn handle_formal_actuals(&self, call: NodeId, callee: NodeId) -> Vec<(NodeId, NodeId)> {
        let formals = self
            .node(callee)
            .children
            .iter()
            .copied()
            .filter(|child| matches!(self.kind(*child), NodeKind::FuncArg { .. }));
        formals
            .zip(self.call_argument_nodes(call))
            .filter(|(formal, _)| self.is_class_handle_formal(*formal))
            .collect()
    }

    fn reject_virtual_callee(
        &self,
        path: &str,
        call: NodeId,
        callee: NodeId,
    ) -> Result<(), String> {
        if matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_virtual: true,
                ..
            }
        ) {
            return Err(self.unobservable_read(
                path,
                call,
                "the storage a virtual method reads (its implementation is chosen at run time)",
            ));
        }
        Ok(())
    }

    /// Bind the handles a call in the wait expression itself passes to its
    /// callee, lowered in the waiting process's own context.
    fn root_call_receivers(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
    ) -> Result<CalleeReceivers, String> {
        self.reject_virtual_callee(path, call, callee)?;
        let mut receivers = CalleeReceivers::default();
        if self.class_method_owner(callee).is_some() {
            receivers.this = self
                .class_method_receiver(path, call)
                .ok()
                .flatten()
                .filter(Self::rearmable_handle);
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self
                .lower_chandle(path, actual)
                .ok()
                .filter(Self::rearmable_handle);
            receivers.formals.insert(formal, handle);
        }
        Ok(receivers)
    }

    /// Bind the handles a call inside a callee body passes to its callee.
    fn nested_call_receivers(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
        outer: &CalleeReceivers,
    ) -> Result<CalleeReceivers, String> {
        self.reject_virtual_callee(path, call, callee)?;
        let mut receivers = CalleeReceivers::default();
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
                } => self.callee_handle(*receiver, outer),
                _ => outer.this.clone(),
            };
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self.callee_handle(actual, outer);
            receivers.formals.insert(formal, handle);
        }
        Ok(receivers)
    }

    /// A class-handle property access on `receiver`, as a handle value.
    fn property_handle(&mut self, receiver: IrChandleExpr, field: NodeId) -> Option<IrChandleExpr> {
        if self.direct_field_type(field) != Some(IrClassFieldType::Chandle) {
            return None;
        }
        let (class, index) = self.class_fields.get(&field).copied()?;
        Some(IrChandleExpr::LocalRead(self.native_access_symbol(
            receiver,
            IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        )))
    }

    /// The handle a declaration names inside a callee body: a bound formal,
    /// a handle property of `this`, or a static handle variable.
    fn callee_root_handle(
        &mut self,
        target: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        if let Some(bound) = receivers.formals.get(&target) {
            return bound.clone();
        }
        if self.class_fields.contains_key(&target) {
            let this = receivers.this.clone()?;
            return self.property_handle(this, target);
        }
        let object = self
            .class_static_objects
            .get(&target)
            .or_else(|| self.object_globals.get(&target))
            .map(|object| self.reference_object(*object))?;
        (self.model.objects.get(object)?.ty == crate::sim::ir::IrObjectType::Chandle)
            .then_some(IrChandleExpr::Read(object))
    }

    /// The handle value expression `node` evaluates to inside a callee body,
    /// when it can be evaluated again whenever the wait arms.
    fn callee_handle(
        &mut self,
        node: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.callee_root_handle(*target, receivers),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                if refs.len() == 1
                    && refs[0].is_none()
                    && parts.first().is_some_and(|part| part == "this")
                {
                    return receivers.this.clone();
                }
                let field = refs.iter().rev().flatten().copied().next()?;
                if refs.len() == 1 {
                    return self.callee_root_handle(field, receivers);
                }
                let receiver = self.callee_property_receiver(node, field, receivers)?;
                self.property_handle(receiver, field)
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "class" => {
                self.callee_handle(*operand, receivers)
            }
            _ => None,
        }
    }

    /// The receiver of property `field` that `node` selects inside a callee
    /// body (see `explicit_class_receiver` for the path shapes).
    fn callee_property_receiver(
        &mut self,
        node: NodeId,
        field: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            // A bare property name inside a method: the implicit `this`.
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
            // The frontend nests each member access in the next: descend to
            // the access that ends at the handle property.
            let mut current = node;
            while let [child] = self.node(current).children.as_slice() {
                let child = *child;
                match self.kind(child) {
                    NodeKind::Expr(ExprKind::HierPath { refs: prefix, .. })
                        if prefix.len() > end =>
                    {
                        current = child;
                    }
                    _ => return self.callee_handle(child, receivers),
                }
            }
            return None;
        }
        if end == 1 {
            return match refs[0] {
                Some(root) => self.callee_root_handle(root, receivers),
                None if parts.first().is_some_and(|part| part == "this") => receivers.this.clone(),
                None => None,
            };
        }
        None
    }

    /// Dependencies of one class property read inside a callee body.
    fn callee_property_dependencies(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
        receivers: &CalleeReceivers,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        let name = self.node(field).name.clone();
        if self.direct_field_type(field).is_none() {
            return Err(self.unobservable_read(
                path,
                node,
                &format!("container or record class property `{name}` read by a called subroutine"),
            ));
        }
        let Some(receiver) = self.callee_property_receiver(node, field, receivers) else {
            return Err(self.unobservable_read(
                path,
                node,
                &format!(
                    "class property `{name}` read by a called subroutine through a handle other than a class-handle input formal, `this`, a handle property or a static handle"
                ),
            ));
        };
        let Some((class, index)) = self.class_fields.get(&field).copied() else {
            return Ok(());
        };
        let access = self.native_access_symbol(
            receiver,
            IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        );
        out.push(IrDependency::NativeAccess(access.clone()));
        self.handle_property_receivers(&access, out);
        Ok(())
    }

    /// Collect the dependencies of the class storage a called function or
    /// method body reads (and, for a method, the module storage it reads,
    /// which the ordinary read walk does not follow into method bodies).
    fn callee_dependencies(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
        receivers: CalleeReceivers,
        stack: &mut Vec<NodeId>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        let Some(body) = self.func_body(callee) else {
            return Ok(());
        };
        if stack.contains(&callee) || stack.len() >= MAX_CALLEE_DEPTH {
            if self.reads_dynamic_storage_in(body, &mut HashSet::new()) {
                return Err(self.unobservable_read(
                    path,
                    call,
                    "the class storage a recursive call reads",
                ));
            }
            return Ok(());
        }
        stack.push(callee);
        if self.class_method_owner(callee).is_some() {
            for dependency in self.collect_read_signals(path, body)? {
                if !self.activation_dependency(&dependency) {
                    out.push(dependency);
                }
            }
        }
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            self.reject_unobservable_read(path, node)?;
            if let Some(info) = self.static_class_property(node) {
                out.push(self.signal_dependency(info));
                continue;
            }
            if self.dynamic_interface_member(node) {
                return Err(self.unobservable_read(
                    path,
                    node,
                    "a virtual-interface member read by a called subroutine",
                ));
            }
            if let Some(field) = self.class_storage_field(node) {
                self.callee_property_dependencies(path, node, field, &receivers, out)?;
                continue;
            }
            if let Some(next) = self.called_subroutine(node) {
                let bound = self.nested_call_receivers(path, node, next, &receivers)?;
                self.callee_dependencies(path, node, next, bound, stack, out)?;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        stack.pop();
        Ok(())
    }

    /// Give every class-property or interface-member read in `root` its
    /// native access, keyed by the reading node, for the read walker. A call
    /// contributes the class storage its callee's body reads, through the
    /// handles the call binds (see [`CalleeReceivers`]).
    fn prepare_dynamic_reads(&mut self, path: &str, root: NodeId) -> Result<(), String> {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            pending.extend(self.node(node).children.iter().copied());
            self.reject_unobservable_read(path, node)?;
            if let Some(info) = self.static_class_property(node) {
                let dependency = self.signal_dependency(info);
                self.dynamic_reads.insert(node, vec![dependency]);
                continue;
            }
            if let Some(callee) = self.called_subroutine(node) {
                let receivers = self.root_call_receivers(path, node, callee)?;
                let mut dependencies = Vec::new();
                self.callee_dependencies(
                    path,
                    node,
                    callee,
                    receivers,
                    &mut Vec::new(),
                    &mut dependencies,
                )?;
                if !dependencies.is_empty() {
                    self.dynamic_reads.insert(node, dependencies);
                }
                continue;
            }
            let read = match self.dynamic_class_property(node) {
                Some(field)
                    if matches!(
                        self.direct_field_type(field),
                        Some(IrClassFieldType::String | IrClassFieldType::Chandle)
                    ) =>
                {
                    // String and handle properties have no packed cell: name
                    // the property itself.
                    let (class, index) = self.class_fields[&field];
                    let receiver = self.class_receiver_for(path, node, field)?;
                    Some(IrExprKind::LocalRead(self.native_access_symbol(
                        receiver,
                        IrNativeAccessKind::ClassField {
                            class,
                            field: index,
                        },
                    )))
                }
                Some(_) => match self.class_field_cell(path, node)? {
                    Some((read, ..)) => Some(read.kind().clone()),
                    None => self
                        .class_field_expr(path, node)?
                        .map(|read| read.kind().clone()),
                },
                None if self.dynamic_interface_member(node) => self
                    .virtual_interface_member_expr(path, node)?
                    .map(|read| read.kind().clone()),
                None => None,
            };
            let Some(IrExprKind::LocalRead(name)) = read else {
                continue;
            };
            // Only accesses a waiting process can resolve again qualify.
            if self.model.native_accesses.iter().any(|access| {
                access.name == name
                    && matches!(
                        access.kind,
                        IrNativeAccessKind::ClassField { .. }
                            | IrNativeAccessKind::InterfaceMember { .. }
                    )
            }) {
                let mut dependencies = vec![IrDependency::NativeAccess(name.clone())];
                self.handle_property_receivers(&name, &mut dependencies);
                self.dynamic_reads.insert(node, dependencies);
            }
        }
        Ok(())
    }

    /// The handle properties a native access selects its receiver through
    /// (`n.next` for `n.next.v`): rebinding one of them moves the access.
    fn handle_property_receivers(&self, name: &str, out: &mut Vec<IrDependency>) {
        let mut current = name.to_owned();
        while let Some(access) = self
            .model
            .native_accesses
            .iter()
            .find(|access| access.name == current)
        {
            let mut receiver = &access.receiver;
            while let IrChandleExpr::Required { handle, .. } = receiver {
                receiver = handle;
            }
            let IrChandleExpr::LocalRead(inner) = receiver else {
                return;
            };
            let handle_property = self.model.native_accesses.iter().any(|access| {
                access.name == *inner
                    && matches!(
                        access.kind,
                        crate::sim::ir::IrNativeAccessKind::ClassField { class, field }
                            if self.model.classes.get(class).and_then(|class| class.fields.get(field)).is_some_and(|field| {
                                field.ty == IrClassFieldType::Chandle
                                    && field.container.is_none()
                                    && field.native_value.is_none()
                            })
                    )
            });
            if !handle_property || out.len() >= MAX_HANDLE_PROPERTY_DEPTH {
                return;
            }
            out.push(IrDependency::NativeAccess(inner.clone()));
            current = inner.clone();
        }
    }

    /// Run `collect` (a read walk over `root`) with the class-property and
    /// interface-member reads of `root` resolved to native-access
    /// dependencies. Use it only where the consumer arms its wait again
    /// after every wake.
    pub(in super::super) fn with_dynamic_reads<T>(
        &mut self,
        path: &str,
        root: NodeId,
        collect: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let prepared = self.prepare_dynamic_reads(path, root);
        let result = prepared.and_then(|()| collect(self));
        self.dynamic_reads.clear();
        result
    }
}
