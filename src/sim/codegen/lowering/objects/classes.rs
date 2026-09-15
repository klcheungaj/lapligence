//! Classes.

use super::*;

impl Codegen<'_> {

    pub(super) fn class_field_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|target| {
                self.class_fields.contains_key(target)
                    || self.class_static_signals.contains_key(target)
                    || self.class_static_objects.contains_key(target)
            }),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().rev().flatten().copied().find(|target| {
                    self.class_fields.contains_key(target)
                        || self.class_static_signals.contains_key(target)
                        || self.class_static_objects.contains_key(target)
                })
            }
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
            return self.lower_chandle("class method call", explicit).map(Some);
        }
        self.class_init_receiver
            .clone()
            .or_else(|| {
                self.func.as_ref().and_then(|function| function.class_receiver.clone())
            })
            .map(Some)
            .ok_or_else(|| "class method call has no receiver".to_owned())
    }

    fn class_receiver_code(&self, receiver: &IrChandleExpr) -> String {
        match receiver {
            IrChandleExpr::Null => "NULL".to_owned(),
            IrChandleExpr::Verbatim(code) => code.clone(),
            IrChandleExpr::Read(index) => self.model.objects[*index].c_name.clone(),
            IrChandleExpr::LocalRead(name) => name.clone(),
            IrChandleExpr::FormalRead(index) => {
                let Some(function) = self.func.as_ref() else {
                    return format!("a{index}");
                };
                let Some((formal, _)) = function
                    .chandle_read
                    .iter()
                    .find(|(_, value)| matches!(value, IrChandleExpr::FormalRead(formal) if formal == index))
                else {
                    return format!("a{index}");
                };
                if let Some(ChandleTarget::Local(name)) = function.chandle_write.get(formal) {
                    name.clone()
                } else {
                    format!("*r{index}")
                }
            }
            IrChandleExpr::ContainerGet { .. }
            | IrChandleExpr::ContainerGetNested { .. }
            | IrChandleExpr::AssociativeGet { .. }
            | IrChandleExpr::Call { .. } => "NULL".to_owned(),
        }
    }

    fn class_receiver_for(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) {
            if let Some(base) = refs
                .iter()
                .take_while(|reference| **reference != Some(field))
                .flatten()
                .find(|base| self.object_of(path, **base).is_some())
            {
                return self.lower_chandle(path, *base);
            }
        }
        self.class_init_receiver
            .clone()
            .or_else(|| {
                self.func.as_ref().and_then(|function| function.class_receiver.clone())
            })
            .ok_or_else(|| {
                format!(
                    "class property `{}` has no receiver in `{path}`",
                    self.node(field).name
                )
            })
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
        match self.kind(field) {
            NodeKind::Var { ty } if is_real_kind(&ty.kind) => Ok((0, false, false)),
            NodeKind::Var { ty } => Ok((
                ty.width.ok_or_else(|| {
                    format!("class property `{}` has no width", self.node(field).name)
                })?,
                ty.signed,
                self.db.is_two_state_type(field) || is_two_state_kind(&ty.kind),
            )),
            _ => Err(format!(
                "class field target `{}` is not a variable",
                self.node(field).name
            )),
        }
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
            return Ok(None);
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
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind) || ty.kind == "string") {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let (width, signed, _two_state) = self.class_field_value_shape(field)?;
        let field_name = class_field.c_name.clone();
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        let code = format!(
            "((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{}",
            self.node(field).full_name,
            field_name
        );
        Ok(Some(IrExpr::new(
            IrExprKind::Verbatim {
                code,
                width,
                signed,
            },
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
            return Ok(None);
        };
        if let Some(info) = self.class_static_signals.get(&field).cloned() {
            return Ok(Some(IrLhs::Whole(info.ir)));
        }
        if self.class_static_objects.contains_key(&field) {
            return Ok(None);
        }
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind) || ty.kind == "string") {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let (width, signed, two_state) = self.class_field_value_shape(field)?;
        let field_name = class_field.c_name.clone();
        let shortreal = matches!(
            class_field.ty,
            crate::sim::ir::IrClassFieldType::Real { shortreal: true }
        );
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        let addr = format!(
            "&(((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{})",
            self.node(field).full_name,
            field_name
        );
        Ok(Some(IrLhs::WholeRef {
            addr,
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
            return Ok(None);
        };
        if !matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let field_name = class_field.c_name.clone();
        let field_full_name = self.node(field).full_name.clone();
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        Ok(Some(format!(
            "((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{}",
            field_full_name, field_name
        )))
    }

    pub(in super::super) fn class_field_string_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<String>, String> {
        let Some(field) = self.class_field_target(node) else {
            return Ok(None);
        };
        if !matches!(self.kind(field), NodeKind::Var { ty } if ty.kind == "string") {
            return Ok(None);
        }
        let Some((class, _, layout)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let name = layout.c_name.clone();
        let receiver = self.class_receiver_for(path, node, field)?;
        Ok(Some(format!(
            "((llg_class_{class}_t*)llg_class_require({}, \"string property\"))->{}",
            self.class_receiver_code(&receiver), name,
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
            return Ok(IrChandleExpr::Verbatim(format!(
                "llg_semaphore_new({})",
                self.render_ir_code(&keys)?
            )));
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
        let mut code = format!(
            "({{ llg_class_{class}_t *{object_name} = (llg_class_{class}_t*)calloc(1, sizeof(llg_class_{class}_t)); "
        );
        code.push_str(&format!(
            "if (!{object_name}) {{ fprintf(stderr, \"llg: class allocation failed\\n\"); exit(EXIT_FAILURE); }} {object_name}->_llg_class_id = {class}; "
        ));
        // Initialize representation metadata, not explicit language defaults.
        // Base constructors may access as-yet uninitialized derived properties;
        // those reads must never encounter a zero-width/unallocated sv4 cell.
        for field in &self.model.classes[class].fields {
            if let IrClassFieldType::Packed { width, signed, two_state } = field.ty {
                let value = if two_state {
                    format!("sv4_from_u64(0, {width}, {})", signed as u8)
                } else {
                    format!("sv4_x({width}, {})", signed as u8)
                };
                code.push_str(&format!("{object_name}->{} = {value}; ", field.c_name));
            }
        }
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
            code.push_str(&format!("{}; ", self.render_ir_code(&call)?));
        } else {
            let statements = self.lower_implicit_class_construction(path, class_node, receiver)?;
            for statement in statements {
                code.push_str(&crate::sim::emit_c::render_stmt(&self.render_ctx(), &statement)?);
            }
        }
        code.push_str(&format!("(void*){object_name}; }})"));
        Ok(IrChandleExpr::Verbatim(code))
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
            let mut fields = self.class_fields.iter()
                .filter(|(_, (owner, _))| *owner == class)
                .map(|(node, (_, index))| (*node, *index))
                .collect::<Vec<_>>();
            fields.sort_by_key(|(_, index)| *index);
            let mut statements = Vec::new();
            for (node, index) in fields {
                let field = self.model.classes[class].fields[index].clone();
                let target = format!("((llg_class_{class}_t*)({}))->{}",
                    self.class_receiver_code(&receiver), field.c_name);
                let initializer = self.db.var_initializer(node);
                match field.ty {
                    IrClassFieldType::Packed { width, signed, two_state } => {
                        let value = if let Some(initializer) = initializer {
                            let value = self.lower_expr(path, initializer)?;
                            ir_to_storage(value, width, signed, two_state)?
                        } else {
                            IrExpr::new(IrExprKind::Verbatim {
                                code: if two_state { format!("sv4_from_u64(0, {width}, {})", signed as u8) }
                                      else { format!("sv4_x({width}, {})", signed as u8) },
                                width, signed,
                            }, width, signed, None)
                        };
                        statements.push(IrStmt::Assign {
                            lhs: IrLhs::WholeRef { addr: format!("&({target})"),
                                width, signed, two_state, shortreal: false },
                            rhs: value,
                            nba: false,
                        });
                    }
                    IrClassFieldType::Real { shortreal } => {
                        let value = initializer.map(|node| self.lower_expr(path, node)).transpose()?
                            .unwrap_or_else(|| IrExpr::new(IrExprKind::Verbatim { code: "0.0".to_owned(), width: 0, signed: true }, 0, true, None));
                        statements.push(IrStmt::Assign {
                            lhs: IrLhs::WholeRef { addr: format!("&({target})"),
                                width: 0, signed: false, two_state: false, shortreal },
                            rhs: IrExpr::new(IrExprKind::CastToReal { a: Box::new(value), shortreal }, 0, true, None),
                            nba: false,
                        });
                    }
                    IrClassFieldType::String => {
                        if let Some(initializer) = initializer {
                            let value = self.lower_string(path, initializer)?;
                            statements.push(IrStmt::Object(IrObjectStmt::StringAssignLocal(target, value)));
                        }
                    }
                    IrClassFieldType::Chandle => {
                        if let Some(initializer) = initializer {
                            let value = self.lower_chandle(path, initializer)?;
                            statements.push(IrStmt::Object(IrObjectStmt::ChandleAssignLocal(target, value)));
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
        let Some(metadata) = self.db.class_metadata(class) else { return Ok(Vec::new()); };
        let Some(base) = metadata.base else { return Ok(Vec::new()); };
        let captured_call = metadata.base_constructor;
        let constructor = captured_call.and_then(|call| match self.kind(call) {
            NodeKind::FuncCall { callee, .. } => *callee,
            _ => None,
        }).or_else(|| self.class_constructor(base));
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
