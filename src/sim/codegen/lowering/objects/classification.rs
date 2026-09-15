//! Classification.

use super::*;

impl Codegen<'_> {
    pub(in super::super) fn is_process_self_call(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::FuncCall { name, .. } if name == "self")
    }

    pub(in super::super) fn is_semaphore_constructor_call(&self, node: NodeId) -> bool {
        if !matches!(self.kind(node), NodeKind::FuncCall { name, .. } if name == "new") {
            return false;
        }
        self.db.node_ids().any(|owner| {
            matches!(
                self.kind(owner),
                NodeKind::Expr(ExprKind::NewClass {
                    class_name: Some(name),
                    constructor: Some(constructor),
                    ..
                }) if name == "semaphore" && *constructor == node
            )
        })
    }

    pub(super) fn is_process_rng_receiver(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::FuncCall { .. } if self.is_process_self_call(node) => true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.node(node).name == "self" || self.node(*target).name == "self",
            _ => self.node(node).name == "self",
        }
    }

    pub(super) fn object_int_argument(
        &mut self,
        path: &str,
        node: NodeId,
        width: u32,
    ) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, width, true, true)
    }

    pub(super) fn semaphore_key_argument(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        if value.is_real() {
            return Err(format!("semaphore key count must be integral in `{path}`"));
        }
        // Built-in semaphore methods take a signed 32-bit `int` keyCount.
        // Keep X/Z bits intact for the runtime boundary instead of applying
        // the two-state conversion used by ordinary array indices.
        ir_to_storage(value, 32, true, false)
    }
    pub(in super::super) fn lower_boolean_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if self.is_mailbox_expr(path, node) {
            let equal = object_query(
                IrObjectQuery::MailboxEq(
                    self.lower_mailbox_expr(path, node, IrMailboxElement::Untyped)?,
                    IrMailboxExpr::Null,
                ),
                1,
                false,
            );
            return Ok(IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(equal),
                },
                1,
                false,
                None,
            ));
        }
        if self.is_chandle_expr(path, node) {
            let equal = object_query(
                IrObjectQuery::ChandleEq(self.lower_chandle(path, node)?, IrChandleExpr::Null),
                1,
                false,
            );
            return Ok(IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(equal),
                },
                1,
                false,
                None,
            ));
        }
        self.lower_expr(path, node)
    }
    pub(in super::super) fn collect_object(&mut self, path: &str, node: NodeId) -> Result<bool, String> {
        let is_mailbox = self.is_mailbox_expr(path, node);
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => match ty.kind.as_str() {
                "string" => IrObjectType::String,
                "chandle" => IrObjectType::Chandle,
                "class" if ty.type_name.as_deref() == Some("process") => IrObjectType::Process,
                "class" if ty.type_name.as_deref() == Some("semaphore") => IrObjectType::Semaphore,
                "class" => IrObjectType::Chandle,
                "virtual_interface" => IrObjectType::Chandle,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if self.object_globals.contains_key(&node) {
            return Ok(true);
        }
        let name = self.node(node).name.clone();
        let index = self.model.objects.len();
        self.model.objects.push(IrObject {
            c_name: format!("O_{}_{}", ident(path), ident(&name)),
            ty,
            initial: None,
        });
        self.object_globals.insert(node, index);
        self.scope_object_names
            .entry(path.to_owned())
            .or_default()
            .insert(name, index);
        if let Some(init) = self.db.var_initializer(node) {
            match ty {
                IrObjectType::String => {
                    self.model.objects[index].initial = Some(self.lower_string(path, init)?)
                }
                IrObjectType::Chandle => {
                    if is_mailbox {
                        self.mailbox_object_initializers
                            .push((node, index, init, path.to_owned()));
                    } else if matches!(self.kind(node), NodeKind::Var { ty } if ty.kind == "virtual_interface")
                    {
                        if !matches!(
                            self.kind(init),
                            NodeKind::Expr(ExprKind::Constant {
                                const_type: ConstantType::Null,
                                ..
                            })
                        ) {
                            self.validate_virtual_interface_assignment(node, init, path)?;
                            self.class_object_initializers.push((
                                node,
                                index,
                                init,
                                path.to_owned(),
                            ));
                        }
                    } else {
                        let is_class =
                            matches!(self.kind(node), NodeKind::Var { ty } if ty.kind == "class");
                        if is_class {
                            self.class_object_initializers.push((
                                node,
                                index,
                                init,
                                path.to_owned(),
                            ));
                        } else if self.lower_chandle(path, init)? != IrChandleExpr::Null {
                            return Err("chandle declaration initializer must be null".to_owned());
                        }
                    }
                }
                IrObjectType::Semaphore => {
                    if matches!(self.kind(init), NodeKind::Expr(ExprKind::NewClass { .. })) {
                        self.semaphore_initializers
                            .push((node, index, init, path.to_owned()));
                    } else if self.lower_chandle(path, init)? != IrChandleExpr::Null {
                        return Err(
                            "semaphore declaration initializer must be new or null".to_owned()
                        );
                    }
                }
                IrObjectType::Process => {
                    if self.lower_process(path, init)? != IrProcessExpr::Null {
                        return Err("process declaration initializer must be null".to_owned());
                    }
                }
            }
        }
        Ok(true)
    }

    pub(in super::super) fn object_of(&self, path: &str, node: NodeId) -> Option<usize> {
        if let Some((target, _kind, member)) = self.unpacked_member_info(node) {
            if let Some(index) = member.object {
                return Some(self.reference_object(index));
            }
            // A recursive aggregate leaf is still declaration-owned even when
            // its representation is a packed/real signal. Do not fall
            // through to display-name lookup for those values.
            if self.unpacked_aggregates.contains_key(&target) {
                return None;
            }
        }
        if let Some(index) = self.object_globals.get(&node) {
            return Some(self.reference_object(*index));
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            return self
                .object_globals
                .get(target)
                .copied()
                .map(|index| self.reference_object(index));
        }
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) {
            return None;
        }
        self.scope_object_names
            .get(path)?
            .get(&self.node(node).name)
            .copied()
            .map(|index| self.reference_object(index))
    }

    pub(in super::super) fn is_string_expr(&self, path: &str, node: NodeId) -> bool {
        if self.class_field_target(node).is_some_and(|field| {
            matches!(self.kind(field), NodeKind::Var { ty } if ty.kind == "string")
        }) {
            return true;
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if self.func.as_ref().is_some_and(|function| {
            target.is_some_and(|target| function.string_read.contains_key(&target))
                || matches!(
                    self.kind(node),
                    NodeKind::Expr(ExprKind::Ref { target: None })
                ) && function
                    .string_read
                    .keys()
                    .any(|target| self.node(*target).name == self.node(node).name)
        }) {
            return true;
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) && self.func.as_ref().is_some_and(|function| {
            let name = &self.node(node).name;
            function
                .arg_ir
                .keys()
                .chain(function.locals.keys())
                .any(|target| self.node(*target).name == *name)
                || function.ret.as_ref().is_some_and(|ret| {
                    ret.node
                        .is_some_and(|target| self.node(target).name == *name)
                })
        }) {
            return false;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            if self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_string)
            {
                return true;
            }
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::String)
        {
            return true;
        }
        if self.lexical_proc_string_local(node).is_some() {
            return true;
        }
        if self.is_container_string_expr(node) {
            return true;
        }
        match self.kind(node) {
            NodeKind::SysCall { name } if name == "$typename" || name == "$sformatf" => true,
            NodeKind::Param { ty, .. } => ty.kind == "string",
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => matches!(self.kind(*target),NodeKind::Param{ty,..} if ty.kind=="string"),
            NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty.kind == "string",
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => {
                (matches!(name.as_str(), "toupper" | "tolower" | "substr")
                    && self.is_string_expr(path, *receiver))
                    || (name == "get_randstate" && self.is_process_rng_receiver(*receiver))
                    || (name == "name"
                        && self
                            .enum_metadata_for_expr(*receiver)
                            .is_some_and(|_| self.node(node).children.len() == 1))
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat | Operation::MultiConcat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.is_string_expr(path, *operand)),
            _ => false,
        }
    }

    /// Return whether an expression is a native chandle without requiring it
    /// to be backed by a model-global object. Function formals, automatic
    /// locals, and chandle-returning calls all live in the function context.
    pub(in super::super) fn is_chandle_expr(&self, path: &str, node: NodeId) -> bool {
        if self.class_field_target(node).is_some_and(|field| {
            matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind))
        }) {
            return true;
        }
        if self.is_mailbox_expr(path, node) {
            return false;
        }
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::NewClass { .. })) {
            return true;
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if is_handle_kind(&ty.kind) {
                return true;
            }
            if self.is_chandle_expr(path, *operand) {
                return false;
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.chandle_read.contains_key(&target)) {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function
                .chandle_read
                .keys()
                .any(|target| self.node(*target).name == self.node(node).name)
            {
                return true;
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return true;
            }
        }
        if self.is_container_chandle_expr(node) {
            return true;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            return self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_chandle);
        }
        false
    }

    /// Return whether an expression is specifically a SystemVerilog
    /// semaphore handle.  Semaphores share the native pointer ABI with
    /// chandles, but their methods must lower to the blocking runtime service.
    pub(in super::super) fn is_semaphore_expr(&self, path: &str, node: NodeId) -> bool {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::NewClass {
                class_name: Some(name),
                ..
            }) if name == "semaphore"
        ) {
            return true;
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore") {
                return true;
            }
            if self.is_semaphore_expr(path, *operand) {
                return true;
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if target.is_some_and(|target| {
            matches!(
                self.kind(target),
                NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
            )
        }) {
            return true;
        }
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.chandle_read.contains_key(&target))
                && target.is_some_and(|target| {
                    matches!(
                        self.kind(target),
                        NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                            if ty.kind == "class"
                                && ty.type_name.as_deref() == Some("semaphore")
                    )
                })
            {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function.chandle_read.keys().any(|target| {
                self.node(*target).name == self.node(node).name
                    && matches!(
                        self.kind(*target),
                        NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                            if ty.kind == "class"
                                && ty.type_name.as_deref() == Some("semaphore")
                    )
            }) {
                return true;
            }
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::Semaphore)
        {
            return true;
        }
        if self.lexical_proc_semaphore_decl(node).is_some() {
            return true;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            if self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .is_some_and(|(function, _)| {
                    matches!(
                        self.kind(function),
                        NodeKind::FuncTask {
                            ret: Some(ty), ..
                        } if ty.kind == "class"
                            && ty.type_name.as_deref() == Some("semaphore")
                    )
                })
            {
                return true;
            }
        }
        false
    }

    /// Return whether an expression denotes a SystemVerilog `process`
    /// handle. Process values are intentionally kept out of packed lowering;
    /// callers must route them through [`lower_process`].
    pub(in super::super) fn is_process_expr(&self, path: &str, node: NodeId) -> bool {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) || self.is_process_self_call(node)
        {
            return true;
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.process_read.contains_key(&target)) {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function
                .process_read
                .keys()
                .any(|target| self.node(*target).name == self.node(node).name)
            {
                return true;
            }
        }
        if self.lexical_proc_process_decl(node).is_some() {
            return true;
        }
        if target.is_some_and(|target| {
            matches!(
                self.kind(target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            return true;
        }
        self.object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::Process)
    }
}
