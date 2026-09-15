//! Processes.

use super::*;

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
        Err(format!(
            "process handle `{}` cannot be resolved in `{path}`",
            self.node(node).name
        ))
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
