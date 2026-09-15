//! Handles.

use super::*;

impl Codegen<'_> {
    /// Resolve a chandle lvalue to its native pointer identity and its
    /// caller-owned pointer slot. The address is never an integer encoding.
    pub(in super::super) fn lower_chandle_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(ChandleTarget, IrChandleExpr), String> {
        let read = self.lower_chandle(path, node)?;
        if let Some(address) = self.class_field_chandle_lvalue(path, node)? {
            return Ok((ChandleTarget::Local(format!("*({address})")), read));
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        let mapped = self.func.as_ref().and_then(|function| {
            target
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .chandle_write
                            .iter()
                            .find(|(target, _)| self.node(**target).name == self.node(node).name)
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        if let Some(target) = mapped {
            return Ok((target, read));
        }
        if let Some(target) = target.and_then(|target| {
            self.proc_mailbox_local_name(target)
                .map(|name| ChandleTarget::Local(name.to_owned()))
        }) {
            return Ok((target, read));
        }
        if let Some(index) = target.and_then(|target| {
            self.proc_mailbox_static_objects
                .get(&(self.inst, target))
                .copied()
        }) {
            return Ok((ChandleTarget::Object(index), read));
        }
        if let Some((_, name)) = self.lexical_proc_mailbox_local(node) {
            return Ok((ChandleTarget::Local(name.to_owned()), read));
        }
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return Ok((ChandleTarget::Object(index), read));
            }
        }
        if let Some(target) = self.semaphore_lvalue_target(path, node)? {
            return Ok((target, read));
        }
        Err("chandle output/ref actual must be a named chandle lvalue".to_owned())
    }

    /// Resolve a procedural semaphore's pointer slot for assignments and
    /// output/ref actuals. Procedural automatic storage is emitted as a C
    /// local, while static storage is represented by a runtime-owned object.
    pub(super) fn semaphore_lvalue_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<ChandleTarget>, String> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        let declaration = self.lexical_proc_semaphore_decl(node).or_else(|| {
            target.filter(|target| {
                matches!(
                    self.kind(*target),
                    NodeKind::Var { ty }
                        if ty.kind == "class"
                            && ty.type_name.as_deref() == Some("semaphore")
                )
            })
        });
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        if let Some(name) = self.proc_semaphore_local_name(declaration) {
            return Ok(Some(ChandleTarget::Local(name.to_owned())));
        }
        if self.db.variable_lifetime(declaration) == VariableLifetime::Static {
            return Ok(Some(ChandleTarget::Object(
                self.collect_semaphore_static_object(path, declaration)?,
            )));
        }
        Ok(None)
    }

    pub(in super::super) fn chandle_target_address(&self, target: &ChandleTarget) -> String {
        match target {
            ChandleTarget::Object(index) => format!("&{}", self.model.objects[*index].c_name),
            ChandleTarget::Local(name) if name.starts_with('*') => format!("&({name})"),
            ChandleTarget::Local(name) => format!("&{name}"),
        }
    }

    pub(in super::super) fn lower_chandle(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if let Some(value) = self.lower_container_chandle_query(path, node)? {
            return Ok(value);
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if is_handle_kind(&ty.kind) {
                return self.lower_chandle(path, *operand);
            }
        }
        if let NodeKind::Expr(ExprKind::NewClass {
            class_name,
            class_type,
            constructor,
            is_super_class,
        }) = self.kind(node)
        {
            return self.lower_new_class(
                path,
                node,
                class_name.as_deref(),
                *class_type,
                *constructor,
                *is_super_class,
            );
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrChandleExpr::Null);
        }
        if let NodeKind::Expr(ExprKind::ScopeRef { target }) = self.kind(node) {
            if let Some((descriptor, instance)) = self.virtual_interface_instances.get(target) {
                let env = self
                    .model
                    .virtual_interfaces
                    .get(*descriptor)
                    .and_then(|interface| interface.instances.get(*instance))
                    .ok_or_else(|| {
                        format!(
                            "virtual interface target `{}` has no runtime environment in `{path}`",
                            self.node(*target).full_name
                        )
                    })?;
                return Ok(IrChandleExpr::Verbatim(format!("(void *)&{}", env.c_name)));
            }
            if matches!(
                self.kind(*target),
                NodeKind::ModuleInst {
                    is_interface: true,
                    ..
                }
            ) {
                return Err(format!(
                    "virtual interface target `{}` has no compatible runtime descriptor in `{path}`",
                    self.node(*target).full_name
                ));
            }
        }
        if let Some(address) = self.class_field_chandle_lvalue(path, node)? {
            return Ok(IrChandleExpr::Verbatim(address));
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(captured) = self
            .capture_target(node)
            .or_else(|| target.filter(|target| self.capture_locals.contains_key(target)))
        {
            let binding = self
                .capture_binding(captured)
                .expect("capture target must have a binding");
            if binding.storage.kind() == StorageKind::Opaque {
                return Ok(IrChandleExpr::LocalRead(Codegen::capture_local_name(
                    binding.storage,
                )));
            }
        }
        if let Some(target) = target.and_then(|target| self.proc_mailbox_local_name(target)) {
            return Ok(IrChandleExpr::LocalRead(target.to_owned()));
        }
        if let Some(index) = target.and_then(|target| {
            self.proc_mailbox_static_objects
                .get(&(self.inst, target))
                .copied()
        }) {
            return Ok(IrChandleExpr::Read(index));
        }
        if let Some((_, name)) = self.lexical_proc_mailbox_local(node) {
            return Ok(IrChandleExpr::LocalRead(name.to_owned()));
        }
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.chandle_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .chandle_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        // Module/class object storage is collected before procedural locals.
        // Resolve it first so a top-level semaphore is not mistaken for a
        // second hidden procedural object for the same declaration.
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return Ok(IrChandleExpr::Read(index));
            }
        }
        if let Some(variable) = self.lexical_proc_semaphore_decl(node) {
            if let Some(name) = self.proc_semaphore_local_name(variable) {
                return Ok(IrChandleExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_semaphore_static_object(path, variable)?;
                return Ok(IrChandleExpr::Read(index));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
            )
        }) {
            if let Some(name) = self.proc_semaphore_local_name(variable) {
                return Ok(IrChandleExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_semaphore_static_object(path, variable)?;
                return Ok(IrChandleExpr::Read(index));
            }
        }
        let callable = match self.kind(node) {
            NodeKind::FuncCall {
                is_task: false,
                callee,
                ..
            } => Some(*callee),
            NodeKind::MethodCall { callee, .. } if self.is_class_method_call(node) => Some(*callee),
            _ => None,
        };
        if let Some(callee) = callable {
            let (ft, _callee_inst) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, callee)?;
            let meta = self
                .func_meta
                .get(&ft)
                .cloned()
                .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
            if !meta.ret_chandle {
                return Err(format!(
                    "function `{}` does not return chandle",
                    self.node(ft).name
                ));
            }
            let args = self.call_argument_nodes(node);
            let bound = self.bind_call_args(self.inst, &meta.formals, &args)?;
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            let mut arg_codes = vec![None; meta.formals.len()];
            let mut arg_irs = vec![None; meta.formals.len()];
            for (idx, (formal, is_out)) in meta.formals.iter().enumerate() {
                let is_chandle = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
                );
                let is_ref = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    if !is_chandle {
                        return Err(format!(
                            "chandle function `{}` does not support packed output/ref formals",
                            self.node(ft).name
                        ));
                    }
                    let (target, _) = self.lower_chandle_lvalue(path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                } else if is_chandle {
                    in_args.push(IrCallArg::ChandleVal(
                        self.lower_chandle(path, bound[idx].expr)?,
                    ));
                } else {
                    let (_, value) = self.lower_bound_arg_code(
                        path,
                        &meta.formals,
                        &bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(value));
                }
            }
            out_args.extend(in_args);
            return Ok(IrChandleExpr::Call {
                receiver: self.class_method_receiver(node)?.map(Box::new),
                virtual_dispatch: self.class_method_virtual_dispatch(node),
                function: meta.ir,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
            });
        }
        Err(format!(
            "chandle values can only be copied from chandle or null (node {:?} `{}` in `{path}`)",
            self.kind(node),
            self.node(node).name
        ))
    }
}
