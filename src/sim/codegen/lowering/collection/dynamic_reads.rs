//! Storage that a wait reaches through a handle: class properties selected
//! through a class handle (`h.x`, `n.next.v`, `this.x`) and virtual-interface
//! members (`v.sig`). Their storage changes when the handle is rebound, so a
//! wait names them by an [`IrDependency::NativeAccess`] that the waiting
//! process resolves again every time it arms.
//!
//! Only `wait` conditions and event expressions observe such storage (SV
//! 9.4.2: `@(p.status)` follows the object `p` names). `always_comb` adds
//! nothing for class objects (SV 9.2.2.2.1), and virtual-interface members
//! cannot be used in sensitivity lists (SV 25.9), so implicit sensitivity
//! keeps only the handle variables it reads.

use super::*;

/// Longest handle-property chain (`a.b.c.d.v`) whose links a wait follows;
/// deeper links reject at the emitter's own access-depth guard first.
const MAX_HANDLE_PROPERTY_DEPTH: usize = 256;

impl Codegen<'_> {
    /// The instance property `node` selects, when it is packed or real
    /// storage held directly in the object.
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
        (layout.container.is_none()
            && layout.native_value.is_none()
            && matches!(
                layout.ty,
                IrClassFieldType::Packed { .. } | IrClassFieldType::Real { .. }
            ))
        .then_some(field)
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
    /// virtual-interface handle, directly or through a class method. Other
    /// callee bodies are not inspected: their receivers are formals, bound
    /// only inside the call.
    pub(in super::super) fn reads_dynamic_storage(&self, root: NodeId) -> bool {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if self.dynamic_class_property(node).is_some()
                || self.dynamic_interface_member(node)
                || self.class_method_call(node).is_some()
            {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Properties of the receiver that method `method` reads through its
    /// implicit `this`, including through the nonvirtual methods it calls on
    /// that receiver: `(class layout, field index)` pairs.
    fn method_receiver_reads(&self, method: NodeId) -> Vec<(usize, usize)> {
        let mut fields = Vec::new();
        let mut methods = vec![method];
        let mut visited = HashSet::new();
        while let Some(method) = methods.pop() {
            if !visited.insert(method) {
                continue;
            }
            let Some(body) = self.func_body(method) else {
                continue;
            };
            let mut pending = vec![body];
            while let Some(node) = pending.pop() {
                pending.extend(self.node(node).children.iter().copied());
                let field = match self.kind(node) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => Some(*target),
                    NodeKind::Expr(ExprKind::HierPath { parts, refs })
                        if parts.first().is_some_and(|part| part == "this") && refs.len() == 2 =>
                    {
                        refs[1]
                    }
                    NodeKind::FuncCall {
                        callee: Some(callee),
                        ..
                    }
                    | NodeKind::MethodCall {
                        receiver: None,
                        callee: Some(callee),
                        ..
                    } if self.class_method_owner(*callee).is_some()
                        && matches!(
                            self.kind(*callee),
                            NodeKind::FuncTask {
                                is_task: false,
                                is_virtual: false,
                                is_static: false,
                                ..
                            }
                        ) =>
                    {
                        methods.push(*callee);
                        None
                    }
                    _ => None,
                };
                let Some((class, index)) = field.and_then(|field| self.class_fields.get(&field))
                else {
                    continue;
                };
                let observable = self
                    .model
                    .classes
                    .get(*class)
                    .and_then(|layout| layout.fields.get(*index))
                    .is_some_and(|layout| {
                        layout.container.is_none()
                            && layout.native_value.is_none()
                            && matches!(
                                layout.ty,
                                IrClassFieldType::Packed { .. } | IrClassFieldType::Real { .. }
                            )
                    });
                if observable && !fields.contains(&(*class, *index)) {
                    fields.push((*class, *index));
                }
            }
        }
        fields
    }

    /// Give every class-property or interface-member read in `root` its
    /// native access, keyed by the reading node, for the read walker. A
    /// nonvirtual method call contributes the properties its body reads
    /// through `this`, selected through the call's receiver.
    fn prepare_dynamic_reads(&mut self, path: &str, root: NodeId) -> Result<(), String> {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            pending.extend(self.node(node).children.iter().copied());
            if let Some((method, receiver)) = self.class_method_call(node) {
                let fields = self.method_receiver_reads(method);
                if fields.is_empty() {
                    continue;
                }
                let handle = self.lower_chandle(path, receiver)?;
                let dependencies = fields
                    .into_iter()
                    .map(|(class, field)| {
                        IrDependency::NativeAccess(self.native_access_symbol(
                            handle.clone(),
                            crate::sim::ir::IrNativeAccessKind::ClassField { class, field },
                        ))
                    })
                    .collect();
                self.dynamic_reads.insert(node, dependencies);
                continue;
            }
            let read = if self.dynamic_class_property(node).is_some() {
                match self.class_field_cell(path, node)? {
                    Some((read, ..)) => Some(read),
                    None => self.class_field_expr(path, node)?,
                }
            } else if self.dynamic_interface_member(node) {
                self.virtual_interface_member_expr(path, node)?
            } else {
                None
            };
            let Some(IrExprKind::LocalRead(name)) = read.map(|read| read.kind().clone()) else {
                continue;
            };
            // Only accesses a waiting process can resolve again qualify.
            if self.model.native_accesses.iter().any(|access| {
                access.name == name
                    && matches!(
                        access.kind,
                        crate::sim::ir::IrNativeAccessKind::ClassField { .. }
                            | crate::sim::ir::IrNativeAccessKind::InterfaceMember { .. }
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
