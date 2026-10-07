//! Processes.

use super::*;

/// Statements before a call, the formal's argument and the copy-back after it.
type ProcessFormalBinding = (Vec<IrStmt>, IrCallArg, Option<IrStmt>);

impl Codegen<'_> {
    fn process_target_node(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        }
    }

    /// Lower a process handle expression while preserving its stable runtime
    /// identity. Only null, self, declared process objects and mapped locals
    /// are legal sources at this stage.
    pub(in super::super) fn lower_process(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrProcessExpr, String> {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrProcessExpr::Null);
        }
        if self.is_process_self_call(node) {
            return Ok(IrProcessExpr::SelfHandle);
        }
        if self.is_plain_process_storage(node) {
            return Ok(IrProcessExpr::Handle(Box::new(
                self.lower_chandle(path, node)?,
            )));
        }
        // Slang wraps `null` and other class-typed sources in an implicit
        // conversion to the `process` type; the handle identity is unchanged.
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if is_handle_kind(&ty.kind) {
                return self.lower_process(path, *operand);
            }
        }
        if let Some(handle) = self.lower_container_process_query(path, node)? {
            return Ok(IrProcessExpr::Handle(Box::new(handle)));
        }
        let target = self.process_target_node(node);
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.process_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .process_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        if let Some(variable) = self.lexical_proc_process_decl(node) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok(IrProcessExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok(IrProcessExpr::Read(index));
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Process {
                return Ok(IrProcessExpr::Read(index));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok(IrProcessExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok(IrProcessExpr::Read(index));
            }
        }
        // Any other process-typed source (a class property, a non-input
        // formal, a function result) lives in plain handle storage; the
        // value written there was pinned (see `IrChandleExpr::PinnedProcess`).
        if self.is_process_typed(node) {
            return Ok(IrProcessExpr::Handle(Box::new(
                self.lower_chandle(path, node)?,
            )));
        }
        Err(format!(
            "process handle `{}` cannot be resolved in `{path}`",
            self.node(node).name
        ))
    }

    /// Whether `node` reads a process handle that `lower_process` resolves
    /// to counted storage or `process::self()`, as opposed to plain handle
    /// storage (class properties, non-input formals, function results).
    pub(in super::super) fn is_counted_process_source(&self, path: &str, node: NodeId) -> bool {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) || self.is_plain_process_storage(node)
        {
            return false;
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            return is_handle_kind(&ty.kind) && self.is_counted_process_source(path, *operand);
        }
        self.is_process_expr(path, node)
    }

    /// Whether `node` names process-typed plain handle storage: an instance
    /// property, or a subprogram variable or formal held in a handle slot
    /// (fork-shared, static, non-input). Such storage keeps no count.
    pub(in super::super) fn is_plain_process_storage(&self, node: NodeId) -> bool {
        let is_process = |target: NodeId| {
            matches!(self.kind(target), NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("process"))
        };
        if self
            .class_field_target(node)
            .is_some_and(|field| self.class_fields.contains_key(&field) && is_process(field))
        {
            return true;
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => None,
        };
        target.is_some_and(|target| {
            is_process(target)
                && self.func.as_ref().is_some_and(|function| {
                    function.chandle_read.contains_key(&target)
                        && !function.process_read.contains_key(&target)
                })
        })
    }

    /// Bind an output, inout or `ref` process formal at a statement call.
    ///
    /// A `ref` formal aliases the caller's counted slot, so the callee's
    /// writes retain and release through it; only counted process variables
    /// qualify. An output or inout bound to a counted variable goes through a
    /// handle temporary that the callee fills with pinned values, then a
    /// counted assignment after the call. Other actuals (plain handle
    /// storage, container elements) return `None` and keep the generic
    /// handle paths, which store pinned values.
    pub(in super::super) fn process_formal_binding(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
        is_ref: bool,
        copy_in: bool,
        temporary: &str,
    ) -> Result<Option<ProcessFormalBinding>, String> {
        let counted = self.is_counted_process_source(path, actual);
        if is_ref {
            if !counted || !self.is_whole_process_variable(actual) {
                return Err(format!(
                    "ref actual of process formal `{}` in `{}` must be a process variable (SIM-015)",
                    self.node(formal).name,
                    self.source_path(path)
                ));
            }
            let (target, _) = self.lower_process_lvalue(path, actual)?;
            return Ok(Some((
                Vec::new(),
                IrCallArg::ChandleRefAddr(self.process_target_address(&target)),
                None,
            )));
        }
        if !counted || !self.is_whole_process_variable(actual) {
            return Ok(None);
        }
        let (target, _) = self.lower_process_lvalue(path, actual)?;
        let initial = if copy_in {
            Some(self.lower_chandle(path, actual)?)
        } else {
            None
        };
        let value = IrProcessExpr::Handle(Box::new(IrChandleExpr::LocalRead(temporary.to_owned())));
        let store = match target {
            ProcessTarget::Object(index) => IrObjectStmt::ProcessAssign(index, value),
            ProcessTarget::Local(name) => IrObjectStmt::ProcessAssignLocal(name, value),
        };
        Ok(Some((
            vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                temporary.to_owned(),
                initial,
            )))],
            IrCallArg::ChandleAddr(format!("&{temporary}")),
            Some(IrStmt::Object(Box::new(store))),
        )))
    }

    /// Bind an output, inout or `ref` process formal of a call evaluated
    /// inside an expression, which has no statement to run a copy-back.
    pub(in super::super) fn process_formal_expression_binding(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
        is_ref: bool,
    ) -> Result<Option<IrCallArg>, String> {
        match self.process_formal_binding(path, formal, actual, is_ref, false, "_pout")? {
            Some((before, argument, None)) if before.is_empty() => Ok(Some(argument)),
            Some(_) => Err(format!(
                "output or inout process formal `{}` bound to a process variable in a call inside an expression in `{}` is not supported; call the subroutine as a statement (SIM-015)",
                self.node(formal).name,
                self.source_path(path)
            )),
            None => Ok(None),
        }
    }

    /// Whether `node` names a whole counted process variable that
    /// `lower_process_lvalue` can bind (not `self()`, `null` or an element).
    fn is_whole_process_variable(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { .. }) => true,
            NodeKind::Expr(ExprKind::HierPath { .. }) => !self.is_container_process_expr(node),
            _ => false,
        }
    }

    pub(in super::super) fn process_target_address(&self, target: &ProcessTarget) -> String {
        match target {
            ProcessTarget::Object(index) => format!("&{}", self.model.objects[*index].c_name),
            ProcessTarget::Local(name) if name.starts_with('*') => name[1..].to_owned(),
            ProcessTarget::Local(name) => format!("&{name}"),
        }
    }

    /// Reject a process `status()` read in a wait, event or implicit
    /// sensitivity expression: status changes publish no change marker, so
    /// such a wait would never wake. `await()` waits for termination.
    pub(in super::super) fn reject_process_status_wait(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<(), String> {
        let mut pending = vec![node];
        while let Some(current) = pending.pop() {
            if let NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } = self.kind(current)
            {
                if name == "status" && self.is_process_value(path, *receiver) {
                    return Err(format!(
                        "process status() in a wait, event or sensitivity expression in `{}` is not supported; status changes are not change events (use await()) (SIM-015)",
                        self.source_path(path)
                    ));
                }
            }
            pending.extend(self.node(current).children.iter().copied());
        }
        Ok(())
    }

    /// Whether `node` is any process-handle expression: counted storage,
    /// `self()`, `null` in a process context, or plain handle storage of
    /// process type.
    pub(in super::super) fn is_process_value(&self, path: &str, node: NodeId) -> bool {
        self.is_process_expr(path, node) || self.is_process_typed(node)
    }

    /// Whether the static type of `node` is the built-in `process` class.
    pub(in super::super) fn is_process_typed(&self, node: NodeId) -> bool {
        self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Class")
                && descriptor.name == "process"
        }) || matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                if matches!(self.kind(*target),
                    NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                        if ty.kind == "class" && ty.type_name.as_deref() == Some("process"))
        )
    }

    /// Resolve a process lvalue and retain the same storage identity for
    /// assignment, method control, and later status/await operations.
    pub(in super::super) fn lower_process_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(ProcessTarget, IrProcessExpr), String> {
        let read = self.lower_process(path, node)?;
        let target = self.process_target_node(node);
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.process_write.get(&target)) {
                return Ok((value.clone(), read));
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .process_write
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok((value.clone(), read));
                }
            }
        }
        if let Some(variable) = self.lexical_proc_process_decl(node) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok((ProcessTarget::Local(name.to_owned()), read));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Process {
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok((ProcessTarget::Local(name.to_owned()), read));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        Err("process output/ref actual must be a named process lvalue".to_owned())
    }
}
