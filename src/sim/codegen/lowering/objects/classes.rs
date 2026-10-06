//! Classes.

use super::*;

impl Codegen<'_> {
    pub(in super::super) fn class_field_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|target| {
                self.class_fields.contains_key(target)
                    || self.class_static_signals.contains_key(target)
                    || self.class_static_objects.contains_key(target)
            }),
            // The last property on the path decides; a native record
            // property (`n.next.rec.s`) is not packed or object storage.
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .find(|target| {
                    self.class_fields.contains_key(target)
                        || self.class_static_signals.contains_key(target)
                        || self.class_static_objects.contains_key(target)
                        || self.class_native_fields.contains_key(target)
                })
                .filter(|target| !self.class_native_fields.contains_key(target)),
            NodeKind::Var { .. } if self.class_fields.contains_key(&node) => Some(node),
            _ => None,
        }
    }

    pub(in super::super) fn is_class_method_call(&self, node: NodeId) -> bool {
        let callee = match self.kind(node) {
            NodeKind::MethodCall { callee, .. } | NodeKind::FuncCall { callee, .. } => *callee,
            _ => None,
        };
        callee.is_some_and(|callee| self.class_method_owner(callee).is_some())
    }

    /// Find the class owning a method declaration. Out-of-block methods are
    /// represented by a method-prototype wrapper between the executable
    /// subroutine and its class, so checking only the immediate parent loses
    /// the class receiver for those calls.
    pub(in super::super) fn class_method_owner(&self, callee: NodeId) -> Option<NodeId> {
        let mut current = Some(callee);
        while let Some(node) = current {
            if self.class_nodes.contains_key(&node) {
                return Some(node);
            }
            current = self.node(node).parent;
        }
        None
    }

    /// Whether a class call should use the runtime override selected by its
    /// receiver's nominal class.  `super` explicitly suppresses virtual
    /// dispatch and therefore binds to the declaring base implementation.
    pub(in super::super) fn class_method_virtual_dispatch(&self, node: NodeId) -> bool {
        let (callee, receiver, is_super) = match self.kind(node) {
            NodeKind::MethodCall {
                callee, receiver, ..
            } => (*callee, *receiver, false),
            NodeKind::FuncCall {
                callee, is_super, ..
            } => (*callee, None, *is_super),
            _ => return false,
        };
        if is_super {
            return false;
        }
        if receiver.is_some_and(|receiver| {
            matches!(
                self.kind(receiver),
                NodeKind::Expr(ExprKind::NewClass {
                    is_super_class: true,
                    ..
                })
            )
        }) {
            return false;
        }
        callee.is_some_and(|callee| {
            matches!(
                self.kind(callee),
                NodeKind::FuncTask {
                    is_virtual: true,
                    ..
                }
            ) && !matches!(
                self.kind(callee),
                NodeKind::FuncTask {
                    is_static: true,
                    ..
                }
            )
        })
    }

    /// Resolve the receiver for a class subroutine call. Explicit receivers
    /// are evaluated at the call site; an omitted receiver means `this` when
    /// lowering a method body (or a fresh allocation's constructor body).
    pub(in super::super) fn class_method_receiver(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        let (callee, explicit) = match self.kind(node) {
            NodeKind::MethodCall {
                callee, receiver, ..
            } => (*callee, *receiver),
            NodeKind::FuncCall { callee, .. } => (*callee, None),
            _ => return Ok(None),
        };
        let Some(callee) = callee else {
            return Err("class method call has no resolved callee".to_owned());
        };
        if self.class_method_owner(callee).is_none() {
            return Ok(None);
        }
        if matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_static: true,
                ..
            }
        ) {
            return Ok(None);
        }
        if let Some(explicit) = explicit {
            let handle = self.lower_chandle(path, explicit)?;
            return Ok(Some(self.required_handle(
                handle,
                "method call",
                path,
                node,
            )));
        }
        self.class_init_receiver
            .clone()
            .or_else(|| {
                self.func
                    .as_ref()
                    .and_then(|function| function.class_receiver.clone())
            })
            .map(Some)
            .ok_or_else(|| "class method call has no receiver".to_owned())
    }

    /// `handle` checked against null where `node` uses it (SV 8.4), with
    /// the run-time error addressed to the use's scope, line and column.
    /// The implicit `this` of a method body is checked at method entry.
    fn required_handle(
        &self,
        handle: IrChandleExpr,
        what: &str,
        path: &str,
        node: NodeId,
    ) -> IrChandleExpr {
        if matches!(
            handle,
            IrChandleExpr::Required { .. }
                | IrChandleExpr::Construct(_)
                | IrChandleExpr::CopyClass { .. }
        ) {
            return handle;
        }
        let source = self.node(node);
        let site = if source.line == 0 {
            format!("{what} at {path}")
        } else {
            format!("{what} at {path}:{}:{}", source.line, source.col)
        };
        IrChandleExpr::Required {
            handle: Box::new(handle),
            site,
        }
    }

    pub(super) fn native_access_symbol(
        &mut self,
        receiver: IrChandleExpr,
        kind: crate::sim::ir::IrNativeAccessKind,
    ) -> String {
        self.native_access_symbol_at(receiver, kind, None)
    }

    pub(super) fn native_access_symbol_at(
        &mut self,
        receiver: IrChandleExpr,
        kind: crate::sim::ir::IrNativeAccessKind,
        site: Option<String>,
    ) -> String {
        let name = format!("_llg_access_{}", self.model.native_accesses.len());
        self.model
            .native_accesses
            .push(crate::sim::ir::IrNativeAccess {
                name: name.clone(),
                receiver,
                kind,
                site,
                item_path: Vec::new(),
                function: self.cur_fn_ir,
            });
        name
    }

    fn class_receiver_for(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if let Some(receiver) = self.explicit_class_receiver(path, node, field)? {
            return Ok(receiver);
        }
        self.class_init_receiver
            .clone()
            .or_else(|| {
                self.func
                    .as_ref()
                    .and_then(|function| function.class_receiver.clone())
            })
            .ok_or_else(|| {
                format!(
                    "class property `{}` has no receiver in `{path}`",
                    self.node(field).name
                )
            })
    }

    /// Before lowering statement `h`, give every class container property
    /// its operands select through an explicit handle (`h.q`, `n.next.q`) a
    /// receiver-qualified alias of the property's per-object container
    /// (SIM-011). Nested statements are prepared when they are lowered.
    pub(in super::super) fn qualify_class_containers(
        &mut self,
        path: &str,
        h: NodeId,
    ) -> Result<(), String> {
        let mut pending = vec![h];
        let mut visited = HashSet::new();
        while let Some(node) = pending.pop() {
            // Operand expressions only: nested statements and declarations
            // are prepared in their own context.
            if !visited.insert(node)
                || (node != h
                    && !matches!(
                        self.kind(node),
                        NodeKind::Expr(_)
                            | NodeKind::MethodCall { .. }
                            | NodeKind::FuncCall { .. }
                            | NodeKind::SysCall { .. }
                    ))
            {
                continue;
            }
            pending.extend(self.node(node).children.iter().copied());
            let mut references = Vec::new();
            self.kind(node).append_references(&mut references);
            pending.extend(references);
            let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) else {
                continue;
            };
            if self.receiver_containers.contains_key(&node)
                || self.receiver_native_values.contains_key(&node)
            {
                continue;
            }
            // A native record property: the selection may continue into
            // its members (`n.rec.s`).
            let native = refs
                .iter()
                .rev()
                .flatten()
                .find_map(|target| self.class_native_fields.get(target).map(|v| (*target, *v)));
            if let Some((field, value)) = native {
                let Some(receiver) = self.explicit_class_receiver(path, node, field)? else {
                    continue;
                };
                if !receiver.is_plain_receiver() {
                    return Err(format!(
                        "class record property `{}` in `{path}` is selected through a handle that is not a variable, formal or property; assign the handle to a variable first (SIM-011)",
                        self.node(field).name
                    ));
                }
                let alias = self.class_native_alias(value, receiver);
                self.receiver_native_values.insert(node, alias);
                continue;
            }
            let Some(field) = refs.last().copied().flatten() else {
                continue;
            };
            let Some(container) = self
                .container_globals
                .get(&field)
                .map(|info| info.ir)
                .filter(|ir| self.model.containers[*ir].class_field.is_some())
            else {
                continue;
            };
            let Some(receiver) = self.explicit_class_receiver(path, node, field)? else {
                continue;
            };
            if !receiver.is_plain_receiver() {
                return Err(format!(
                    "class container property `{}` in `{path}` is selected through a handle that is not a variable, formal or property; assign the handle to a variable first (SIM-011)",
                    self.node(field).name
                ));
            }
            let alias = self.class_container_alias(container, receiver);
            self.receiver_containers.insert(node, alias);
        }
        Ok(())
    }

    /// A receiver-qualified alias of native record class property `value`.
    pub(in super::super) fn class_native_alias(
        &mut self,
        value: usize,
        receiver: IrChandleExpr,
    ) -> usize {
        let alias = self.model.native_values.len();
        let mut storage = self.model.native_values[value].clone();
        storage.c_name = format!("{}_via_{alias}", storage.c_name);
        storage.receiver = Some(receiver);
        self.model.native_values.push(storage);
        if let Some(declaration) = self.native_value_layouts.get(&value).copied() {
            self.native_value_layouts.insert(alias, declaration);
        }
        alias
    }

    /// A receiver-qualified alias of class container property `container`.
    fn class_container_alias(&mut self, container: usize, receiver: IrChandleExpr) -> usize {
        let alias = self.model.containers.len();
        let mut storage = self.model.containers[container].clone();
        storage.c_name = format!("{}_via_{alias}", storage.c_name);
        storage.receiver = Some(receiver);
        self.model.containers.push(storage);
        if let Some(range) = self.fixed_view_ranges.get(&container).copied() {
            self.fixed_view_ranges.insert(alias, range);
        }
        alias
    }

    /// The receiver of class property `field` that path expression `node`
    /// names explicitly, or `None` for the enclosing method's `this`.
    fn explicit_class_receiver(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        if let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) {
            // The property is the last path component naming it: a chain
            // such as `n.next.next` names the same declaration twice.
            let end = refs
                .iter()
                .rposition(|reference| *reference == Some(field))
                .unwrap_or(refs.len());
            // A property selected through a handle property (`n.next.val`,
            // SV 8.4) has that property's value as its receiver, not the
            // first handle variable of the path. The frontend keeps the
            // selected handle expression as the access's base child.
            if end > 0
                && refs[end - 1].is_some_and(|previous| {
                    (self.class_fields.contains_key(&previous)
                        || self.class_static_objects.contains_key(&previous))
                        && matches!(self.kind(previous), NodeKind::Var { ty } if is_handle_kind(&ty.kind))
                })
            {
                // The frontend nests each member access in the next: descend
                // to the access that ends at the handle property.
                let mut base = None;
                let mut current = node;
                while let [child] = self.node(current).children.as_slice() {
                    let child = *child;
                    match self.kind(child) {
                        NodeKind::Expr(ExprKind::HierPath { refs: prefix, .. })
                            if prefix.len() > end =>
                        {
                            current = child;
                        }
                        _ => {
                            base = Some(child);
                            break;
                        }
                    }
                }
                let Some(base) = base else {
                    return Err(format!(
                        "class property `{}` selected through a handle property has no base expression in `{path}`",
                        self.node(field).name
                    ));
                };
                let handle = self.lower_chandle(path, base)?;
                return Ok(Some(self.required_handle(handle, "property access", path, node)));
            }
            if let Some(base) = refs[..end].iter().flatten().find(|base| {
                self.object_of(path, **base).is_some()
                    || self.is_container_chandle_expr(**base)
                    || self.is_subroutine_handle_local(**base)
            }) {
                let handle = self.lower_chandle(path, *base)?;
                return Ok(Some(self.required_handle(
                    handle,
                    "property access",
                    path,
                    node,
                )));
            }
            if let Some(receiver) = self.record_handle_receiver(refs, node, field)? {
                return Ok(Some(self.required_handle(
                    receiver,
                    "property access",
                    path,
                    node,
                )));
            }
            // A handle member of a record element (`q[i].h.v`): the base
            // expression is the frontend's child of the property access.
            if let [base] = self.node(node).children.as_slice() {
                let base = *base;
                if self.element_leaf_kind(base) == Some(crate::sim::ir::IrClassFieldType::Chandle) {
                    let handle = self.lower_chandle(path, base)?;
                    return Ok(Some(self.required_handle(
                        handle,
                        "property access",
                        path,
                        node,
                    )));
                }
            }
        }
        Ok(None)
    }

    /// A class-handle formal or local of the subroutine being lowered, or
    /// one a fork branch captured from its parent: `lower_chandle` resolves
    /// its activation storage by declaration identity.
    fn is_subroutine_handle_local(&self, declaration: NodeId) -> bool {
        self.capture_locals.contains_key(&declaration)
            || self
                .func
                .as_ref()
                .is_some_and(|function| function.chandle_read.contains_key(&declaration))
    }

    /// A class handle stored as a record member (`r.h.v`, SIM-007): the
    /// handle leaf of a module record or native subroutine value named by
    /// the path between the record and the property.
    fn record_handle_receiver(
        &mut self,
        refs: &[Option<NodeId>],
        node: NodeId,
        field: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, .. }) = self.kind(node) else {
            return Ok(None);
        };
        let Some(end) = refs.iter().position(|reference| *reference == Some(field)) else {
            return Ok(None);
        };
        let parts = parts.clone();
        for (index, reference) in refs[..end].iter().enumerate() {
            let Some(root) = reference else {
                continue;
            };
            let member_path: Vec<AggregatePathPart> = parts[index + 1..end]
                .iter()
                .cloned()
                .map(AggregatePathPart::Member)
                .collect();
            if member_path.is_empty() {
                continue;
            }
            if let Some(value) = self.native_roots.get(root).copied() {
                return self.native_handle_leaf(value, &member_path);
            }
            if let Some(aggregate) = self.unpacked_aggregates.get(root) {
                return Ok(aggregate
                    .leaves
                    .iter()
                    .find(|leaf| leaf.path == member_path)
                    .and_then(|leaf| leaf.object)
                    .filter(|object| {
                        self.model.objects[*object].ty == crate::sim::ir::IrObjectType::Chandle
                    })
                    .map(IrChandleExpr::Read));
            }
        }
        Ok(None)
    }

    fn class_field_layout(
        &self,
        field: NodeId,
    ) -> Option<(usize, usize, &crate::sim::ir::IrClassField)> {
        let (class, index) = self.class_fields.get(&field).copied()?;
        Some((
            class,
            index,
            self.model.classes.get(class)?.fields.get(index)?,
        ))
    }

    fn class_field_value_shape(&self, field: NodeId) -> Result<(u32, bool, bool), String> {
        match self
            .class_field_layout(field)
            .map(|(_, _, layout)| layout.ty)
        {
            Some(IrClassFieldType::Real { .. }) => Ok((0, false, false)),
            Some(IrClassFieldType::Packed {
                width,
                signed,
                two_state,
            }) => Ok((width, signed, two_state)),
            _ => Err(format!(
                "class property `{}` is not packed or real storage",
                self.node(field).name
            )),
        }
    }

    /// Packed instance property `node` names, possibly through a handle,
    /// as fixed-value storage: its read, its whole target, its type and the
    /// member names selected after it (`n.ps.a`), so member, bit and part
    /// selections project into the property (SIM-011).
    #[allow(clippy::type_complexity)]
    pub(in super::super) fn class_field_cell(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(IrExpr, IrLhs, TypeDescriptor, Vec<String>)>, String> {
        let (field, members) = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self.class_fields.contains_key(target) => (*target, Vec::new()),
            // A bare declaration has only the implicit `this` receiver.
            NodeKind::Var { .. }
                if self.class_fields.contains_key(&node)
                    && (self.class_init_receiver.is_some()
                        || self
                            .func
                            .as_ref()
                            .is_some_and(|function| function.class_receiver.is_some())) =>
            {
                (node, Vec::new())
            }
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let Some(field) = self
                    .class_field_target(node)
                    .filter(|field| self.class_fields.contains_key(field))
                else {
                    return Ok(None);
                };
                let Some(end) = refs.iter().rposition(|reference| *reference == Some(field)) else {
                    return Ok(None);
                };
                (field, parts[end + 1..].to_vec())
            }
            _ => return Ok(None),
        };
        let Some((class, index, layout)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let IrClassFieldType::Packed {
            width,
            signed,
            two_state,
        } = layout.ty
        else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(field).cloned() else {
            return Ok(None);
        };
        let receiver = self.class_receiver_for(path, node, field)?;
        let name = self.native_access_symbol(
            receiver,
            crate::sim::ir::IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        );
        Ok(Some((
            IrExpr::new(IrExprKind::LocalRead(name.clone()), width, signed, None),
            IrLhs::WholeRef {
                addr: format!("&{name}"),
                width,
                signed,
                two_state,
                shortreal: false,
            },
            descriptor,
            members,
        )))
    }

    pub(in super::super) fn node_contains_super_constructor(&self, root: NodeId) -> bool {
        let mut pending = vec![root];
        let mut visited = HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::NewClass {
                    is_super_class: true,
                    ..
                })
            ) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    fn class_constructor(&self, class: NodeId) -> Option<NodeId> {
        self.class_method_nodes(class).into_iter().find(|child| {
            matches!(
                self.kind(*child),
                NodeKind::FuncTask {
                    is_constructor: true,
                    ..
                }
            )
        })
    }

    pub(in super::super) fn class_field_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(field) = self.class_field_target(node) else {
            return self.native_leaf_expr(path, node);
        };
        if let Some(info) = self.class_static_signals.get(&field).cloned() {
            return Ok(Some(self.signal_read_expr(&info)?));
        }
        if self.class_static_objects.contains_key(&field) {
            return Err(format!(
                "string/class object static property `{}` is not a packed expression",
                self.node(field).name
            ));
        }
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind) || ty.kind == "string")
        {
            return Ok(None);
        }
        let Some((class, index, _)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let (width, signed, _) = self.class_field_value_shape(field)?;
        let receiver = self.class_receiver_for(path, node, field)?;
        let name = self.native_access_symbol(
            receiver,
            crate::sim::ir::IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        );
        Ok(Some(IrExpr::new(
            IrExprKind::LocalRead(name),
            width,
            signed,
            None,
        )))
    }

    pub(in super::super) fn class_field_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some(field) = self.class_field_target(node) else {
            return self.native_leaf_target(path, node);
        };
        if let Some(info) = self.class_static_signals.get(&field).cloned() {
            return Ok(Some(IrLhs::Whole(info.ir)));
        }
        if self.class_static_objects.contains_key(&field) {
            return Ok(None);
        }
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind) || ty.kind == "string")
        {
            return Ok(None);
        }
        // A member selected after the property (`n.ps.a`) is a fixed-value
        // projection, not the whole property.
        if let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) {
            if refs.last().copied().flatten() != Some(field) {
                return Ok(None);
            }
        }
        let Some((class, index, layout)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let shortreal = matches!(layout.ty, IrClassFieldType::Real { shortreal: true });
        let (width, signed, two_state) = self.class_field_value_shape(field)?;
        let receiver = self.class_receiver_for(path, node, field)?;
        let name = self.native_access_symbol(
            receiver,
            crate::sim::ir::IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        );
        Ok(Some(IrLhs::WholeRef {
            addr: format!("&{name}"),
            width,
            signed,
            two_state,
            shortreal,
        }))
    }

    pub(super) fn class_field_chandle_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<String>, String> {
        let Some(field) = self.class_field_target(node) else {
            return self.native_object_leaf(path, node, false);
        };
        if !matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
            return Ok(None);
        }
        let Some((class, index, _)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let receiver = self.class_receiver_for(path, node, field)?;
        Ok(Some(self.native_access_symbol(
            receiver,
            crate::sim::ir::IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        )))
    }

    pub(in super::super) fn class_field_string_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<String>, String> {
        let Some(field) = self.class_field_target(node) else {
            return self.native_object_leaf(path, node, true);
        };
        if !matches!(self.kind(field), NodeKind::Var { ty } if ty.kind == "string") {
            return Ok(None);
        }
        let Some((class, index, _)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let receiver = self.class_receiver_for(path, node, field)?;
        Ok(Some(self.native_access_symbol(
            receiver,
            crate::sim::ir::IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        )))
    }

    pub(super) fn lower_new_class(
        &mut self,
        path: &str,
        node: NodeId,
        class_name: Option<&str>,
        class_type: Option<crate::core::db::TypeId>,
        constructor: Option<NodeId>,
        is_super_class: bool,
    ) -> Result<IrChandleExpr, String> {
        if is_super_class {
            return self
                .func
                .as_ref()
                .and_then(|function| function.class_receiver.clone())
                .or_else(|| self.class_init_receiver.clone())
                .ok_or_else(|| format!("super constructor has no receiver in `{path}"));
        }
        if class_name == Some("semaphore") {
            let args = constructor
                .map(|constructor| self.call_argument_nodes(constructor))
                .unwrap_or_default();
            let keys = match args.as_slice() {
                [] => lhs_integer_expr(0),
                [value] => self.semaphore_key_argument(path, *value)?,
                _ => {
                    return Err(format!(
                        "semaphore constructor takes zero or one key-count argument in `{path}`"
                    ))
                }
            };
            return Ok(IrChandleExpr::SemaphoreNew(Box::new(keys)));
        }
        let name = class_name.unwrap_or("<specialized class>");
        let class_node = class_type
            .and_then(|type_id| self.db.class_for_type(type_id))
            .or_else(|| {
                self.db
                    .classes()
                    .iter()
                    .find(|class| self.node(**class).name == name)
                    .copied()
            });
        let class_node = class_node
            .ok_or_else(|| format!("class `{name}` has no captured layout in `{path}"))?;
        if let Some(metadata) = self.db.class_metadata(class_node) {
            if metadata.is_abstract || metadata.is_interface {
                return Err(format!(
                    "cannot construct {} class `{name}` in `{path}`",
                    if metadata.is_interface {
                        "interface"
                    } else {
                        "abstract"
                    }
                ));
            }
        }
        let class = self
            .class_nodes
            .get(&class_node)
            .copied()
            .ok_or_else(|| format!("class `{name}` has no captured layout in `{path}`"))?;
        // Actual arguments remain in the caller's environment. Each constructor
        // initializes its own layer after calling super; allocation initializes no
        // language properties ahead of that sequence.
        let object_name = format!("_llg_obj_{}", node.index());
        let receiver = IrChandleExpr::LocalRead(object_name.clone());
        let mut body = Vec::new();
        if let Some(constructor) = constructor {
            let (name, callee) = match self.kind(constructor) {
                NodeKind::FuncCall { name, callee, .. } => (name.clone(), *callee),
                _ => return Err("class constructor edge does not name a function call".to_owned()),
            };
            let mut call = self.lower_func_call_expr(path, constructor, &name, callee)?;
            if let IrExprKind::CallFn(call) = &mut call.kind {
                call.receiver = Some(receiver);
                call.virtual_dispatch = false;
            }
            body.push(IrStmt::PlusArg(call));
        } else {
            body = self.lower_implicit_class_construction(path, class_node, receiver)?;
        }
        let index = self.model.class_allocations.len();
        self.model
            .class_allocations
            .push(crate::sim::ir::IrClassAllocation {
                class,
                local: object_name,
                body,
                function: self.cur_fn_ir,
            });
        Ok(IrChandleExpr::Construct(index))
    }

    /// Shallow copy `new h` (SV 8.11) of the source expression's class type.
    /// A null source is a run-time error at the copy's site, like a property
    /// access. Built-in classes have no property layout to copy.
    pub(super) fn lower_class_copy(
        &mut self,
        path: &str,
        node: NodeId,
        source: NodeId,
        class_type: Option<crate::core::db::TypeId>,
    ) -> Result<IrChandleExpr, String> {
        let class = class_type
            .and_then(|type_id| self.db.class_for_type(type_id))
            .and_then(|class_node| self.class_nodes.get(&class_node).copied())
            .ok_or_else(|| {
                format!(
                    "shallow copy of a built-in or unlayouted class handle `{}` is not supported in `{path}`",
                    self.node(source).name
                )
            })?;
        let handle = self.lower_chandle(path, source)?;
        Ok(IrChandleExpr::CopyClass {
            class,
            source: Box::new(self.required_handle(handle, "class copy", path, node)),
        })
    }

    /// Property defaults belong to the new instance, not an enclosing factory
    /// method. Only fields declared by this inheritance layer are initialized.
    pub(in super::super) fn lower_class_initializers(
        &mut self,
        path: &str,
        class_node: NodeId,
        receiver: IrChandleExpr,
    ) -> Result<Vec<IrStmt>, String> {
        let previous = self.class_init_receiver.replace(receiver.clone());
        let result = (|| {
            let class = self.class_nodes[&class_node];
            let mut fields = self
                .class_fields
                .iter()
                .filter(|(_, (owner, _))| *owner == class)
                .map(|(node, (_, index))| (*node, *index))
                .collect::<Vec<_>>();
            // Container properties of this layer with an initializer, by
            // their declaration: they initialize in field order with the rest.
            let containers = self
                .container_globals
                .iter()
                .filter_map(|(node, info)| {
                    let storage = &self.model.containers[info.ir];
                    match storage.class_field {
                        Some((owner, index)) if owner == class && storage.receiver.is_none() => {
                            Some((*node, index, info.ir))
                        }
                        _ => None,
                    }
                })
                .filter(|(node, _, _)| {
                    self.db
                        .array_meta(*node)
                        .is_some_and(|meta| meta.initializer().is_some())
                })
                .collect::<Vec<_>>();
            fields.extend(containers.iter().map(|(node, index, _)| (*node, *index)));
            // Native record properties of this layer with an initializer.
            let natives = self
                .class_native_fields
                .iter()
                .filter_map(|(node, value)| {
                    let (owner, index) = self.model.native_values[*value].class_field?;
                    (owner == class && self.db.var_initializer(*node).is_some())
                        .then_some((*node, index, *value))
                })
                .collect::<Vec<_>>();
            fields.extend(natives.iter().map(|(node, index, _)| (*node, *index)));
            fields.sort_by_key(|(_, index)| *index);
            let mut statements = Vec::new();
            for (node, index) in fields {
                if let Some((_, _, value)) = natives
                    .iter()
                    .find(|(declaration, _, _)| *declaration == node)
                {
                    let initializer = self
                        .db
                        .var_initializer(node)
                        .ok_or_else(|| "class record initializer disappeared".to_owned())?;
                    // The new object's value, bound while its initializer lowers.
                    let alias = self.class_native_alias(*value, receiver.clone());
                    let previous = self.native_roots.insert(node, alias);
                    let lowered = self.lower_native_value_assignment(
                        path,
                        node,
                        initializer,
                        false,
                        Operation::Assignment,
                    );
                    match previous {
                        Some(previous) => self.native_roots.insert(node, previous),
                        None => self.native_roots.remove(&node),
                    };
                    statements
                        .push(lowered?.ok_or("class record initializer has no record assignment")?);
                    continue;
                }
                if let Some((_, _, container)) = containers
                    .iter()
                    .find(|(declaration, _, _)| *declaration == node)
                {
                    let initializer = self
                        .db
                        .array_meta(node)
                        .and_then(|meta| meta.initializer())
                        .ok_or_else(|| "class container initializer disappeared".to_owned())?;
                    let alias = self.class_container_alias(*container, receiver.clone());
                    statements.push(self.lower_container_into(path, node, alias, initializer)?);
                    continue;
                }
                let field = self.model.classes[class].fields[index].clone();
                let target = self.native_access_symbol(
                    receiver.clone(),
                    crate::sim::ir::IrNativeAccessKind::ClassField {
                        class,
                        field: index,
                    },
                );
                let initializer = self.db.var_initializer(node);
                match field.ty {
                    IrClassFieldType::Packed {
                        width,
                        signed,
                        two_state,
                    } => {
                        let value = if let Some(initializer) = initializer {
                            let value = self.lower_expr(path, initializer)?;
                            ir_to_storage(value, width, signed, two_state)?
                        } else if let Some(default) = self.fixed_default_literal(node) {
                            // An unpacked record image keeps its two-state
                            // leaves at zero (SV 6.8, Table 6-7).
                            IrExpr::new(
                                IrExprKind::Const(default.clone()),
                                default.width,
                                default.signed,
                                default.fill,
                            )
                        } else {
                            IrExpr::new(
                                IrExprKind::Fill(if two_state { 0 } else { 2 }),
                                width,
                                signed,
                                None,
                            )
                        };
                        statements.push(IrStmt::Assign {
                            lhs: IrLhs::WholeRef {
                                addr: format!("&{target}"),
                                width,
                                signed,
                                two_state,
                                shortreal: false,
                            },
                            rhs: value,
                            nba: false,
                        });
                    }
                    IrClassFieldType::Real { shortreal } => {
                        let value = initializer
                            .map(|node| self.lower_expr(path, node))
                            .transpose()?
                            .unwrap_or_else(|| {
                                // Real expressions are signed, as the
                                // constant folder's real results are.
                                IrExpr::new(
                                    IrExprKind::CastToReal {
                                        a: Box::new(lhs_integer_expr(0)),
                                        shortreal,
                                    },
                                    0,
                                    true,
                                    None,
                                )
                            });
                        statements.push(IrStmt::Assign {
                            lhs: IrLhs::WholeRef {
                                addr: format!("&{target}"),
                                width: 0,
                                signed: false,
                                two_state: false,
                                shortreal,
                            },
                            rhs: IrExpr::new(
                                IrExprKind::CastToReal {
                                    a: Box::new(value),
                                    shortreal,
                                },
                                0,
                                true,
                                None,
                            ),
                            nba: false,
                        });
                    }
                    IrClassFieldType::String => {
                        if let Some(initializer) = initializer {
                            let value = self.lower_string(path, initializer)?;
                            statements.push(IrStmt::Object(Box::new(
                                IrObjectStmt::StringAssignLocal(target, value),
                            )));
                        }
                    }
                    IrClassFieldType::Chandle => {
                        if let Some(initializer) = initializer {
                            let value = self.lower_chandle(path, initializer)?;
                            statements.push(IrStmt::Object(Box::new(
                                IrObjectStmt::ChandleAssignLocal(target, value),
                            )));
                        }
                    }
                }
            }
            Ok(statements)
        })();
        self.class_init_receiver = previous;
        result
    }

    /// An implicit constructor still performs every inherited construction layer.
    pub(in super::super) fn lower_implicit_class_construction(
        &mut self,
        path: &str,
        class: NodeId,
        receiver: IrChandleExpr,
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = self.lower_class_base_construction(path, class, receiver.clone())?;
        statements.extend(self.lower_class_initializers(path, class, receiver)?);
        Ok(statements)
    }

    pub(in super::super) fn lower_class_base_construction(
        &mut self,
        path: &str,
        class: NodeId,
        receiver: IrChandleExpr,
    ) -> Result<Vec<IrStmt>, String> {
        let Some(metadata) = self.db.class_metadata(class) else {
            return Ok(Vec::new());
        };
        let Some(base) = metadata.base else {
            return Ok(Vec::new());
        };
        let captured_call = metadata.base_constructor;
        let constructor = captured_call
            .and_then(|call| match self.kind(call) {
                NodeKind::FuncCall { callee, .. } => *callee,
                _ => None,
            })
            .or_else(|| self.class_constructor(base));
        if let Some(constructor) = constructor {
            let mut call = if let Some(call) = captured_call {
                self.lower_func_call_expr(path, call, "new", Some(constructor))?
            } else {
                self.lower_func_call_expr_with_args(path, class, "new", Some(constructor), &[])?
            };
            if let IrExprKind::CallFn(call) = &mut call.kind {
                call.receiver = Some(receiver);
                call.virtual_dispatch = false;
            }
            Ok(vec![IrStmt::PlusArg(call)])
        } else {
            self.lower_implicit_class_construction(path, base, receiver)
        }
    }
}
